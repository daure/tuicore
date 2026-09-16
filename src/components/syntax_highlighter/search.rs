use ratatui::{
    layout::Rect,
    text::{Line, Span, Text},
    widgets::{Paragraph, Wrap},
};
use unicode_segmentation::UnicodeSegmentation;

use super::SyntaxHighlighter;
use crate::search::{SearchAction, TextMatchRange, search_match_style, text_match_ranges};
use crate::{AnimationSettings, KeyEvent, ScrollOffset, ScrollOutcome};

pub(super) struct SyntaxSearchMatch {
    line: usize,
    range: TextMatchRange,
}

impl SyntaxHighlighter {
    pub(super) fn search_layout_state(&self) -> (bool, bool, usize) {
        (
            self.search.is_active(),
            self.search.is_editing(),
            self.search.query().len(),
        )
    }

    pub(super) fn search_matches(&self) -> Vec<SyntaxSearchMatch> {
        if !self.search.is_active() {
            return Vec::new();
        }
        self.code
            .lines()
            .enumerate()
            .flat_map(|(line, text)| {
                text_match_ranges(self.search.query(), text)
                    .into_iter()
                    .map(move |range| SyntaxSearchMatch { line, range })
            })
            .collect()
    }

    pub(super) fn handle_search_key(
        &mut self,
        key: KeyEvent,
        area: Rect,
        settings: AnimationSettings,
    ) -> Option<ScrollOutcome> {
        if !self.focused {
            return None;
        }
        let previous_matches = self.search_matches();
        let previous_match = self
            .search
            .selected(previous_matches.len())
            .map(|index| &previous_matches[index]);
        let previous_width = self.scroll_geometry(area).layout.viewport.width;
        let action = self.search.handle_key(key);
        if action == SearchAction::Unhandled {
            return None;
        }
        self.pending_top_prefix = false;
        let count = self.search_matches().len();
        match action {
            SearchAction::Next => self.search.select_next(count),
            SearchAction::Previous => self.search.select_previous(count),
            _ => {}
        }
        // The search row can change the scrollbar gutter and therefore word wrapping.
        self.area = area;
        self.refresh_content_size();
        let centered = if action == SearchAction::Cleared {
            if let Some(matched) = previous_match
                && self.selected_line == Some(self.search_match_row(matched, previous_width))
            {
                self.selected_line =
                    Some(self.search_match_row(
                        matched,
                        self.scroll_geometry(area).layout.viewport.width,
                    ));
            }
            self.center_selection(
                area,
                AnimationSettings {
                    enabled: false,
                    ..settings
                },
            )
        } else if action != SearchAction::Handled {
            self.center_search_match(area, settings)
        } else {
            ScrollOutcome::idle()
        };
        Some(ScrollOutcome {
            handled: true,
            changed: true,
            active: centered.active,
        })
    }

    pub(super) fn center_search_match(
        &mut self,
        area: Rect,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        let matches = self.search_matches();
        let Some(selected) = self.search.selected(matches.len()) else {
            self.clamp_scroll();
            return ScrollOutcome::idle();
        };
        let matched = &matches[selected];
        let geometry = self.scroll_geometry(area);
        let line = self.code.lines().nth(matched.line).unwrap_or_default();
        let row = self.search_match_row(matched, geometry.layout.viewport.width);
        self.selected_line = Some(row);
        let start = Line::raw(line.chars().take(matched.range.start).collect::<String>()).width();
        let end = Line::raw(line.chars().take(matched.range.end).collect::<String>()).width();
        let target = self.scroll.target_offset();
        let x = if self.wrap {
            0
        } else if start < target.x || end.saturating_sub(start) > geometry.viewport.width {
            start
        } else if end > target.x.saturating_add(geometry.viewport.width) {
            end.saturating_sub(geometry.viewport.width)
        } else {
            target.x
        };
        self.scroll.scroll_to(
            ScrollOffset::new(x, row.saturating_sub(geometry.viewport.height / 2)),
            geometry.viewport,
            geometry.content,
            settings,
        )
    }

    fn search_match_row(&self, matched: &SyntaxSearchMatch, width: u16) -> usize {
        if !self.wrap {
            return matched.line;
        }
        let mut lines = self.code.lines();
        let preceding = lines
            .by_ref()
            .take(matched.line)
            .map(|line| {
                Paragraph::new(line)
                    .wrap(Wrap { trim: false })
                    .line_count(width)
            })
            .sum::<usize>();
        preceding + wrapped_match_row(lines.next().unwrap_or_default(), matched.range.start, width)
    }

    pub(super) fn highlight_search_matches(&self, text: &mut Text<'static>) {
        let matches = self.search_matches();
        let selected = self.search.selected(matches.len());
        if matches.is_empty() {
            return;
        }
        for (row, line) in text.lines.iter_mut().enumerate() {
            let first = matches.partition_point(|matched| matched.line < row);
            let end = matches.partition_point(|matched| matched.line <= row);
            let row_matches = &matches[first..end];
            if row_matches.is_empty() {
                continue;
            }
            let mut position = 0;
            let mut next_match = 0;
            line.spans = line
                .spans
                .iter()
                .flat_map(|span| {
                    let mut spans: Vec<Span<'static>> = Vec::new();
                    for grapheme in span.content.graphemes(true) {
                        let end = position + grapheme.chars().count();
                        while row_matches
                            .get(next_match)
                            .is_some_and(|matched| matched.range.end <= position)
                        {
                            next_match += 1;
                        }
                        let contains = |index: usize| {
                            let matched = &matches[index];
                            matched.line == row
                                && matched.range.start < end
                                && matched.range.end > position
                        };
                        let current = selected.filter(|index| contains(*index));
                        let matched = current.or_else(|| {
                            row_matches
                                .get(next_match)
                                .filter(|matched| matched.range.start < end)
                                .map(|_| first + next_match)
                        });
                        let style = matched.map_or(span.style, |index| {
                            span.style
                                .patch(search_match_style(selected == Some(index)))
                        });
                        if let Some(last) = spans.last_mut().filter(|last| last.style == style) {
                            last.content.to_mut().push_str(grapheme);
                        } else {
                            spans.push(Span::styled(grapheme.to_owned(), style));
                        }
                        position = end;
                    }
                    spans
                })
                .collect();
        }
    }
}

// Track a source position through Paragraph's untrimmed word wrapping. Counting a
// truncated prefix is insufficient: the rest of a word can push its start to the next row.
fn wrapped_match_row(line: &str, position: usize, width: u16) -> usize {
    use std::collections::VecDeque;

    let width = usize::from(width);
    if width == 0 {
        return 0;
    }
    let mut row = 0;
    let mut line_width = 0;
    let mut word_width = 0;
    let mut space_width = 0;
    let mut line_has_match = false;
    let mut word_has_match = false;
    let mut spaces = VecDeque::new();
    let mut previous_word = false;
    let mut offset = 0;
    for grapheme in line.graphemes(true) {
        let end = offset + grapheme.chars().count();
        let has_match = (offset..end).contains(&position);
        offset = end;
        let size = Span::raw(grapheme).width();
        if size > width {
            continue;
        }
        let whitespace =
            grapheme == "\u{200b}" || grapheme.chars().all(|c| c.is_whitespace() && c != '\u{a0}');
        if previous_word && whitespace || line_width == 0 && word_width + space_width + size > width
        {
            line_width += space_width + word_width;
            line_has_match |= word_has_match || spaces.iter().any(|(_, matched)| *matched);
            word_width = 0;
            space_width = 0;
            word_has_match = false;
            spaces.clear();
        }
        if line_width >= width || size > 0 && line_width + space_width + word_width >= width {
            if line_has_match {
                return row;
            }
            let mut remaining = width.saturating_sub(line_width);
            row += 1;
            line_width = 0;
            line_has_match = false;
            while let Some(&(size, matched)) = spaces.front() {
                if size > remaining {
                    break;
                }
                if matched {
                    return row.saturating_sub(1);
                }
                remaining -= size;
                space_width -= size;
                spaces.pop_front();
            }
            if whitespace && spaces.is_empty() {
                if has_match {
                    return row.saturating_sub(1);
                }
                continue;
            }
        }
        if whitespace {
            space_width += size;
            spaces.push_back((size, has_match));
        } else {
            word_width += size;
            word_has_match |= has_match;
        }
        previous_word = !whitespace;
    }
    row
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{
        buffer::Buffer,
        style::{Modifier, Style},
        widgets::Widget,
    };

    #[test]
    fn wrapped_match_positions_agree_with_rendered_word_boundaries() {
        for source in [
            "prefix needleword suffix",
            "  abc   defghijklmnopqrstuvwxyz  tail",
            "界界 e\u{301}clair 👩‍💻 needle end",
            "a\u{a0}b\u{200b}c needle",
        ] {
            for width in 1..16 {
                let mut position = 0;
                for (byte, grapheme) in source.grapheme_indices(true) {
                    let line = Line::from(vec![
                        Span::raw(&source[..byte]),
                        Span::styled(
                            grapheme,
                            Style::default().add_modifier(Modifier::UNDERLINED),
                        ),
                        Span::raw(&source[byte + grapheme.len()..]),
                    ]);
                    let paragraph = Paragraph::new(line).wrap(Wrap { trim: false });
                    let area = Rect::new(0, 0, width, paragraph.line_count(width) as u16);
                    let mut buffer = Buffer::empty(area);
                    paragraph.render(area, &mut buffer);
                    if let Some(row) = (0..area.height).find(|y| {
                        (0..width).any(|x| buffer[(x, *y)].modifier.contains(Modifier::UNDERLINED))
                    }) {
                        assert_eq!(
                            wrapped_match_row(source, position, width),
                            usize::from(row),
                            "source={source:?}, position={position}, width={width}"
                        );
                    }
                    position += grapheme.chars().count();
                }
            }
        }
    }
}
