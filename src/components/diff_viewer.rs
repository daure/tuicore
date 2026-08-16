use std::io;
use std::path::Path;
use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use similar::{Algorithm, DiffOp, capture_diff_slices};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    Animated, AnimationSettings, AxisProposal, EventCtx, EventOutcome, FocusCtx, FocusId,
    LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint, ScrollAxes, ScrollBehavior,
    ScrollGeometry, ScrollOutcome, ScrollSize, ScrollState, ScrollbarConfig, TickResult, TuiNode,
    keybindings, line_width, paragraph_scroll, preset, theme,
};
use crate::{KeyEvent, TuiEvent};

mod wrap;

const DIFF_FOCUS: &str = "diff-viewer";

/// User-visible diff layout. "Split" and "unified" are common aliases for
/// [`SideBySide`](Self::SideBySide) and [`Inline`](Self::Inline).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DiffStyle {
    #[default]
    SideBySide,
    Inline,
    Word,
    RawPatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffLocation {
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct DiffViewer {
    old_text: String,
    new_text: String,
    old_label: String,
    new_label: String,
    style: DiffStyle,
    context_lines: usize,
    min_rows: usize,
    max_rows: usize,
    show_headers: bool,
    wrap: bool,
    selected: Option<DiffLocation>,
    rows: Vec<DiffRow>,
    parts: Vec<StyledLine>,
    display_parts: Vec<StyledLine>,
    content: ScrollSize,
    scroll: ScrollState,
    focused: bool,
    area: Rect,
    pending_top_prefix: bool,
}

#[derive(Debug, Clone)]
enum DiffRow {
    Equal {
        old: usize,
        new: usize,
        text: String,
    },
    Pair {
        old: Option<(usize, String)>,
        new: Option<(usize, String)>,
        old_pos: usize,
        new_pos: usize,
    },
    Hunk {
        old_start: usize,
        old_count: usize,
        new_start: usize,
        new_count: usize,
    },
    Gap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiffRole {
    Normal,
    Muted,
    Accent,
    Added,
    Removed,
    AddedEmphasis,
    RemovedEmphasis,
}

#[derive(Debug, Clone)]
struct StyledPart {
    text: String,
    role: DiffRole,
}

#[derive(Debug, Clone)]
struct StyledLine {
    parts: Vec<StyledPart>,
    continuation_indent: usize,
    side_left_content_width: Option<usize>,
    location: Option<DiffLocation>,
}

impl DiffViewer {
    pub fn new(old_text: impl Into<String>, new_text: impl Into<String>) -> Self {
        let mut viewer = Self {
            old_text: normalize_newlines(old_text.into()),
            new_text: normalize_newlines(new_text.into()),
            old_label: "old".to_string(),
            new_label: "new".to_string(),
            style: DiffStyle::default(),
            context_lines: 3,
            min_rows: 1,
            max_rows: 20,
            show_headers: true,
            wrap: true,
            selected: None,
            rows: Vec::new(),
            parts: Vec::new(),
            display_parts: Vec::new(),
            content: ScrollSize::default(),
            scroll: ScrollState::from_preset(ScrollAxes::Vertical, preset().scroll()),
            focused: false,
            area: Rect::default(),
            pending_top_prefix: false,
        };
        viewer.rebuild();
        viewer
    }

    pub fn from_paths(old_path: impl AsRef<Path>, new_path: impl AsRef<Path>) -> io::Result<Self> {
        let old_path = old_path.as_ref();
        let new_path = new_path.as_ref();
        Ok(Self::new(
            std::fs::read_to_string(old_path)?,
            std::fs::read_to_string(new_path)?,
        )
        .labels(
            old_path.display().to_string(),
            new_path.display().to_string(),
        ))
    }

    pub fn labels(mut self, old: impl Into<String>, new: impl Into<String>) -> Self {
        self.old_label = old.into();
        self.new_label = new.into();
        self.rebuild();
        self
    }

    pub fn set_texts(&mut self, old: impl Into<String>, new: impl Into<String>) {
        let old = normalize_newlines(old.into());
        let new = normalize_newlines(new.into());
        if self.old_text == old && self.new_text == new {
            return;
        }
        self.old_text = old;
        self.new_text = new;
        self.selected = None;
        self.rebuild();
    }

    pub fn style(mut self, style: DiffStyle) -> Self {
        self.set_style(style);
        self
    }

    pub fn set_style(&mut self, style: DiffStyle) {
        if self.style != style {
            self.style = style;
            self.rebuild();
        }
    }

    pub fn context_lines(mut self, context_lines: usize) -> Self {
        self.set_context_lines(context_lines);
        self
    }

    pub fn set_context_lines(&mut self, context_lines: usize) {
        if self.context_lines != context_lines {
            self.context_lines = context_lines;
            self.rebuild();
        }
    }

    pub fn show_headers(mut self, show: bool) -> Self {
        self.set_show_headers(show);
        self
    }

    pub fn set_show_headers(&mut self, show: bool) {
        if self.show_headers != show {
            self.show_headers = show;
            self.rebuild();
        }
    }

    pub fn headers_visible(&self) -> bool {
        self.show_headers
    }

    pub fn wrap(mut self, wrap: bool) -> Self {
        self.set_wrap(wrap);
        self
    }

    pub fn set_wrap(&mut self, wrap: bool) {
        if self.wrap != wrap {
            self.wrap = wrap;
            self.scroll = self.scroll.clone().with_axes(if wrap {
                ScrollAxes::Vertical
            } else {
                ScrollAxes::Both
            });
            if wrap {
                self.scroll.snap_horizontal_to_start();
            }
            self.refresh_projection();
            self.clamp_scroll();
        }
    }

    pub fn is_wrapping(&self) -> bool {
        self.wrap
    }

    pub fn selected_location(&self) -> Option<DiffLocation> {
        self.selected
    }

    pub fn min_rows(mut self, min_rows: usize) -> Self {
        self.min_rows = min_rows.max(1);
        self.max_rows = self.max_rows.max(self.min_rows);
        self
    }

    pub fn max_rows(mut self, max_rows: usize) -> Self {
        self.max_rows = max_rows.max(1);
        self.min_rows = self.min_rows.min(self.max_rows);
        self
    }

    pub fn scroll_behavior(mut self, behavior: ScrollBehavior) -> Self {
        self.scroll = self.scroll.behavior(behavior);
        self
    }

    pub fn scrollbars(mut self, config: ScrollbarConfig) -> Self {
        self.scroll = self.scroll.scrollbars(config);
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub fn content_size(&self) -> ScrollSize {
        self.content
    }

    pub fn scroll_geometry(&self, area: Rect) -> ScrollGeometry {
        self.scroll.geometry(area, self.content)
    }

    pub fn on_key(&mut self, key: impl Into<KeyEvent>, area: Rect) -> ScrollOutcome {
        self.on_key_with_settings(key, area, crate::animation_settings())
    }

    pub fn on_key_with_settings(
        &mut self,
        key: impl Into<KeyEvent>,
        area: Rect,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        let key = key.into();
        let bindings = keybindings();
        let data_keys = bindings.data_view();
        let viewport = self.scroll_geometry(area).viewport.height.max(1);
        let page = (viewport.saturating_mul(3).saturating_add(4) / 5).max(1);
        if data_keys.top_prefix_matches(key) {
            if self.pending_top_prefix {
                self.pending_top_prefix = false;
                let selection = self.select_index(0, area, settings, true);
                if selection.changed {
                    return selection;
                }
            } else {
                self.pending_top_prefix = true;
                return ScrollOutcome {
                    handled: true,
                    changed: false,
                    active: false,
                };
            }
        } else {
            self.pending_top_prefix = false;
        }
        if bindings.page_up_matches(key) {
            let selection = self.select_relative(-(page as isize), area, settings, true);
            if selection.changed {
                return selection;
            }
        }
        if bindings.page_down_matches(key) {
            let selection = self.select_relative(page as isize, area, settings, true);
            if selection.changed {
                return selection;
            }
        }
        if bindings.home_matches(key) {
            let selection = self.select_index(0, area, settings, true);
            if selection.changed {
                return selection;
            }
        }
        if bindings.end_matches(key) || data_keys.bottom_matches(key) {
            let selection = self.select_index(
                self.selectable_locations().len().saturating_sub(1),
                area,
                settings,
                true,
            );
            if selection.changed {
                return selection;
            }
        }
        if bindings.line_up_matches(key) {
            let selection = self.move_selection(-1, area, settings);
            if selection.changed {
                return selection;
            }
        }
        if bindings.line_down_matches(key) {
            let selection = self.move_selection(1, area, settings);
            if selection.changed {
                return selection;
            }
        }
        let geometry = self.scroll_geometry(area);
        self.scroll
            .on_key(key, geometry.viewport, geometry.content, settings)
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }
        let geometry = self.scroll_geometry(area);
        if !geometry.layout.viewport.is_empty() {
            self.render_selection_background(frame, geometry);
            let lines = self.styled_lines();
            frame.render_widget(
                Paragraph::new(lines).scroll(paragraph_scroll(self.scroll.offset())),
                geometry.layout.viewport,
            );
        }
        self.scroll
            .render_scrollbars(frame, geometry.layout, geometry.content, self.focused);
    }

    fn rebuild(&mut self) {
        let old_lines = text_lines(&self.old_text);
        let new_lines = text_lines(&self.new_text);
        let ops = capture_diff_slices(Algorithm::Myers, &old_lines, &new_lines);
        let old_missing = !self.old_text.is_empty() && !self.old_text.ends_with('\n');
        let new_missing = !self.new_text.is_empty() && !self.new_text.ends_with('\n');
        let mut rows = rows_from_ops(&ops, &old_lines, &new_lines);
        if old_missing != new_missing {
            if let Some(DiffRow::Equal { old, new, text }) = rows.last().cloned() {
                *rows.last_mut().unwrap() = DiffRow::Pair {
                    old: Some((old, text.clone())),
                    new: Some((new, text)),
                    old_pos: old - 1,
                    new_pos: new - 1,
                };
            }
        }
        self.rows = if self.old_text == self.new_text {
            rows
        } else {
            add_hunk_headers(apply_context(rows, self.context_lines))
        };
        self.parts = self.line_parts();
        self.sync_selection();
        self.refresh_projection();
        self.clamp_scroll();
    }

    fn clamp_scroll(&mut self) {
        let geometry = self.scroll_geometry(self.area);
        self.scroll.clamp_to(
            geometry.viewport,
            geometry.content,
            AnimationSettings {
                enabled: false,
                ..crate::animation_settings()
            },
        );
    }

    fn sync_selection(&mut self) {
        let selectable = self.selectable_locations();
        if self
            .selected
            .is_none_or(|selected| !selectable.contains(&selected))
        {
            self.selected = selectable.first().copied();
        }
    }

    fn selectable_locations(&self) -> Vec<DiffLocation> {
        self.parts.iter().filter_map(|line| line.location).fold(
            Vec::new(),
            |mut locations, location| {
                if locations.last() != Some(&location) {
                    locations.push(location);
                }
                locations
            },
        )
    }

    fn move_selection(
        &mut self,
        direction: isize,
        area: Rect,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        self.select_relative(direction, area, settings, false)
    }

    fn select_relative(
        &mut self,
        direction: isize,
        area: Rect,
        settings: AnimationSettings,
        animate_center: bool,
    ) -> ScrollOutcome {
        let locations = self.selectable_locations();
        let Some(current) = self.selected else {
            return ScrollOutcome::idle();
        };
        let Some(index) = locations.iter().position(|location| *location == current) else {
            return ScrollOutcome::idle();
        };
        let next = if direction.is_negative() {
            index.saturating_sub(direction.unsigned_abs())
        } else {
            index
                .saturating_add(direction as usize)
                .min(locations.len().saturating_sub(1))
        };
        self.select_index(next, area, settings, animate_center)
    }

    fn select_index(
        &mut self,
        index: usize,
        area: Rect,
        settings: AnimationSettings,
        animate_center: bool,
    ) -> ScrollOutcome {
        let locations = self.selectable_locations();
        let Some(location) = locations
            .get(index.min(locations.len().saturating_sub(1)))
            .copied()
        else {
            return ScrollOutcome::idle();
        };
        let changed = self.selected != Some(location);
        self.selected = Some(location);
        let scroll = self.center_selection(
            area,
            AnimationSettings {
                enabled: animate_center && settings.enabled,
                ..settings
            },
        );
        ScrollOutcome {
            handled: true,
            changed: changed || scroll.changed,
            active: scroll.active,
        }
    }

    fn center_selection(&mut self, area: Rect, settings: AnimationSettings) -> ScrollOutcome {
        let Some(selected) = self.selected else {
            return ScrollOutcome::idle();
        };
        let Some(row) = self
            .display_parts
            .iter()
            .position(|line| line.location == Some(selected))
        else {
            return ScrollOutcome::idle();
        };
        let geometry = self.scroll_geometry(area);
        let viewport = geometry.viewport.height.max(1);
        let y = row.saturating_sub(viewport / 2);
        self.scroll.scroll_to(
            crate::ScrollOffset::new(self.scroll.target_offset().x, y),
            geometry.viewport,
            geometry.content,
            settings,
        )
    }

    fn render_selection_background(&self, frame: &mut Frame, geometry: ScrollGeometry) {
        let Some(selected) = self.selected.filter(|_| self.focused) else {
            return;
        };
        let offset = self.scroll.offset().y;
        let bottom = offset.saturating_add(geometry.viewport.height);
        let style = Style::default()
            .fg(theme().highlight_fg())
            .bg(theme().highlight_bg());
        for (index, line) in self.display_parts.iter().enumerate() {
            if line.location != Some(selected) || index < offset || index >= bottom {
                continue;
            }
            frame.render_widget(
                Block::default().style(style),
                Rect::new(
                    geometry.layout.viewport.x,
                    geometry.layout.viewport.y + (index - offset) as u16,
                    geometry.layout.viewport.width,
                    1,
                ),
            );
        }
    }

    #[cfg(test)]
    fn plain_lines(&self) -> Vec<String> {
        self.parts
            .iter()
            .map(|line| line.parts.iter().map(|part| part.text.as_str()).collect())
            .collect()
    }

    #[cfg(test)]
    fn display_plain_lines(&self) -> Vec<String> {
        self.display_parts
            .iter()
            .map(|line| line.parts.iter().map(|part| part.text.as_str()).collect())
            .collect()
    }

    fn styled_lines(&self) -> Vec<Line<'_>> {
        let theme = theme();
        self.display_parts
            .iter()
            .map(|line| {
                let selected = self.focused && line.location == self.selected;
                Line::from(
                    line.parts
                        .iter()
                        .map(|part| {
                            let style = if selected {
                                Style::default()
                                    .fg(theme.highlight_fg())
                                    .bg(theme.highlight_bg())
                            } else {
                                match part.role {
                                    DiffRole::Normal => Style::default().fg(theme.text_fg()),
                                    DiffRole::Muted => Style::default().fg(theme.muted_fg()),
                                    DiffRole::Accent => Style::default().fg(theme.accent_fg()),
                                    DiffRole::Added => Style::default()
                                        .fg(theme.diff_added_fg())
                                        .bg(theme.diff_added_bg()),
                                    DiffRole::Removed => Style::default()
                                        .fg(theme.diff_removed_fg())
                                        .bg(theme.diff_removed_bg()),
                                    DiffRole::AddedEmphasis => Style::default()
                                        .fg(theme.diff_added_fg())
                                        .bg(theme.diff_added_emphasis_bg()),
                                    DiffRole::RemovedEmphasis => Style::default()
                                        .fg(theme.diff_removed_fg())
                                        .bg(theme.diff_removed_emphasis_bg()),
                                }
                            };
                            Span::styled(part.text.as_str(), style)
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .collect()
    }

    fn line_parts(&self) -> Vec<StyledLine> {
        match self.style {
            DiffStyle::SideBySide => self.side_by_side_parts(),
            DiffStyle::Inline => self.unified_parts(false, false),
            DiffStyle::Word => self.unified_parts(true, false),
            DiffStyle::RawPatch if self.old_text == self.new_text => Vec::new(),
            DiffStyle::RawPatch => self.unified_parts(false, true),
        }
    }

    fn side_by_side_parts(&self) -> Vec<StyledLine> {
        let number_width = number_width(&self.rows);
        let left_width = self.side_left_width();
        let divider_column = number_width + 3 + left_width;
        let mut output = if self.show_headers {
            vec![side_styled_line(
                vec![part(
                    format!(
                        "{}{} │ {}",
                        " ".repeat(number_width + 3),
                        pad_to_width(&self.old_label, left_width),
                        self.new_label
                    ),
                    DiffRole::Accent,
                )],
                0,
                number_width + 3 + display_width(&self.old_label),
                None,
            )]
        } else {
            Vec::new()
        };
        for row in &self.rows {
            match row {
                DiffRow::Equal { old, new, text } => output.push(side_styled_line(
                    vec![part(
                        format!(
                            "{old:>number_width$}   {} │ {new:>number_width$}   {text}",
                            pad_to_width(text, left_width)
                        ),
                        DiffRole::Normal,
                    )],
                    number_width + 3,
                    number_width + 3 + display_width(text),
                    Some(DiffLocation {
                        old_line: Some(*old),
                        new_line: Some(*new),
                    }),
                )),
                DiffRow::Pair { old, new, .. } => {
                    let old_number = old
                        .as_ref()
                        .map_or(String::new(), |(line, _)| line.to_string());
                    let new_number = new
                        .as_ref()
                        .map_or(String::new(), |(line, _)| line.to_string());
                    let old_text = old.as_ref().map_or("", |(_, text)| text.as_str());
                    let new_text = new.as_ref().map_or("", |(_, text)| text.as_str());
                    output.push(side_styled_line(
                        vec![
                            part(
                                format!(
                                    "{old_number:>number_width$} - {}",
                                    pad_to_width(old_text, left_width)
                                ),
                                DiffRole::Removed,
                            ),
                            part(" │ ", DiffRole::Muted),
                            part(
                                format!("{new_number:>number_width$} + {new_text}"),
                                DiffRole::Added,
                            ),
                        ],
                        number_width + 3,
                        number_width + 3 + display_width(old_text),
                        Some(DiffLocation {
                            old_line: old.as_ref().map(|(line, _)| *line),
                            new_line: new.as_ref().map(|(line, _)| *line),
                        }),
                    ));
                    let old_missing = old
                        .as_ref()
                        .is_some_and(|(line, _)| self.old_missing_newline(*line));
                    let new_missing = new
                        .as_ref()
                        .is_some_and(|(line, _)| self.new_missing_newline(*line));
                    if old_missing || new_missing {
                        let (parts, content_width) = split_annotation_parts(
                            "\\ No newline at end of file",
                            divider_column,
                            DiffRole::Muted,
                        );
                        output.push(side_styled_line(parts, 0, content_width, None));
                    }
                }
                DiffRow::Hunk {
                    old_start,
                    old_count,
                    new_start,
                    new_count,
                } => {
                    if self.show_headers {
                        let (parts, content_width) = split_annotation_parts(
                            &hunk_header(*old_start, *old_count, *new_start, *new_count),
                            divider_column,
                            DiffRole::Accent,
                        );
                        output.push(side_styled_line(parts, 0, content_width, None));
                    }
                }
                DiffRow::Gap => {}
            }
        }
        output
    }

    fn side_left_width(&self) -> usize {
        self.rows
            .iter()
            .filter_map(|row| match row {
                DiffRow::Equal { text, .. } => Some(text.as_str()),
                DiffRow::Pair { old, .. } => old.as_ref().map(|(_, text)| text.as_str()),
                _ => None,
            })
            .map(|text| line_width(&Line::from(text)))
            .max()
            .unwrap_or(0)
            .max(if self.show_headers {
                display_width(&self.old_label)
            } else {
                0
            })
    }

    fn side_divider_column(&self) -> usize {
        number_width(&self.rows) + 3 + self.side_left_width()
    }

    fn unified_parts(&self, words: bool, raw: bool) -> Vec<StyledLine> {
        let number_width = number_width(&self.rows);
        let indent = if raw { 1 } else { number_width * 2 + 4 };
        let mut output = if !self.show_headers {
            Vec::new()
        } else if raw {
            vec![
                styled_line(
                    vec![part(format!("--- {}", self.old_label), DiffRole::Removed)],
                    0,
                ),
                styled_line(
                    vec![part(format!("+++ {}", self.new_label), DiffRole::Added)],
                    0,
                ),
            ]
        } else {
            vec![styled_line(
                vec![part(
                    format!("{} → {}", self.old_label, self.new_label),
                    DiffRole::Accent,
                )],
                0,
            )]
        };
        for row in &self.rows {
            match row {
                DiffRow::Equal { old, new, text } => {
                    output.push(located_styled_line(
                        vec![part(
                            unified_prefix(raw, ' ', Some(*old), Some(*new), number_width) + text,
                            DiffRole::Normal,
                        )],
                        indent,
                        DiffLocation {
                            old_line: Some(*old),
                            new_line: Some(*new),
                        },
                    ));
                    if self.old_missing_newline(*old) || self.new_missing_newline(*new) {
                        output.push(styled_line(newline_marker_parts(), 0));
                    }
                }
                DiffRow::Pair { old, new, .. } => {
                    let paired = words && old.is_some() && new.is_some();
                    if let Some((line, text)) = old {
                        let mut parts = vec![part(
                            unified_prefix(raw, '-', Some(*line), None, number_width),
                            DiffRole::Removed,
                        )];
                        parts.extend(if paired {
                            word_parts(text, new.as_ref().unwrap().1.as_str(), false)
                        } else {
                            vec![part(text.clone(), DiffRole::Removed)]
                        });
                        output.push(located_styled_line(
                            parts,
                            indent,
                            DiffLocation {
                                old_line: Some(*line),
                                new_line: new.as_ref().map(|(line, _)| *line),
                            },
                        ));
                        if self.old_missing_newline(*line) {
                            output.push(styled_line(newline_marker_parts(), 0));
                        }
                    }
                    if let Some((line, text)) = new {
                        let mut parts = vec![part(
                            unified_prefix(raw, '+', None, Some(*line), number_width),
                            DiffRole::Added,
                        )];
                        parts.extend(if paired {
                            word_parts(old.as_ref().unwrap().1.as_str(), text, true)
                        } else {
                            vec![part(text.clone(), DiffRole::Added)]
                        });
                        output.push(located_styled_line(
                            parts,
                            indent,
                            DiffLocation {
                                old_line: old.as_ref().map(|(line, _)| *line),
                                new_line: Some(*line),
                            },
                        ));
                        if self.new_missing_newline(*line) {
                            output.push(styled_line(newline_marker_parts(), 0));
                        }
                    }
                }
                DiffRow::Hunk {
                    old_start,
                    old_count,
                    new_start,
                    new_count,
                } => {
                    if self.show_headers {
                        output.push(styled_line(
                            vec![part(
                                hunk_header(*old_start, *old_count, *new_start, *new_count),
                                DiffRole::Accent,
                            )],
                            0,
                        ));
                    }
                }
                DiffRow::Gap => {}
            }
        }
        output
    }

    fn old_missing_newline(&self, line: usize) -> bool {
        !self.old_text.ends_with('\n') && line == text_lines(&self.old_text).len()
    }

    fn new_missing_newline(&self, line: usize) -> bool {
        !self.new_text.ends_with('\n') && line == text_lines(&self.new_text).len()
    }
}

impl Animated for DiffViewer {
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.scroll.tick(dt, settings)
    }
}

impl<M> TuiNode<M> for DiffViewer {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let width = self.content.width.min(u16::MAX as usize) as u16;
        let height = self
            .content
            .height
            .max(self.min_rows)
            .min(self.max_rows)
            .min(u16::MAX as usize) as u16;
        let width = match proposal.width {
            AxisProposal::Unbounded => width,
            AxisProposal::AtMost(max) => width.min(max),
            AxisProposal::Exact(exact) => exact,
        };
        LayoutSizeHint::content(width, height).normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        let resized = self.area.width != area.width || self.area.height != area.height;
        self.area = area;
        self.refresh_projection();
        if resized {
            self.center_selection(
                area,
                AnimationSettings {
                    enabled: false,
                    ..crate::animation_settings()
                },
            );
        } else {
            self.clamp_scroll();
        }
        ctx.register_focusable(FocusId::new(DIFF_FOCUS), area, true);
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _ctx: &mut crate::RenderCtx<'_>) {
        Self::render(self, frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };
        let outcome = self.on_key_with_settings(*key, self.area, ctx.animation());
        if outcome.needs_redraw() {
            ctx.request_redraw();
        }
        if outcome.handled {
            ctx.stop_propagation();
            EventOutcome::Handled
        } else {
            EventOutcome::Ignored
        }
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(self, dt, settings)
    }

    fn focus(&mut self, _target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        self.focused = focused;
        ctx.request_redraw();
    }
}

fn normalize_newlines(text: String) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn text_lines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        Vec::new()
    } else {
        text.strip_suffix('\n')
            .unwrap_or(text)
            .split('\n')
            .collect()
    }
}

fn rows_from_ops(ops: &[DiffOp], old: &[&str], new: &[&str]) -> Vec<DiffRow> {
    let mut rows = Vec::new();
    for op in ops {
        match *op {
            DiffOp::Equal {
                old_index,
                new_index,
                len,
            } => {
                for offset in 0..len {
                    rows.push(DiffRow::Equal {
                        old: old_index + offset + 1,
                        new: new_index + offset + 1,
                        text: old[old_index + offset].to_string(),
                    });
                }
            }
            DiffOp::Delete {
                old_index,
                old_len,
                new_index,
            } => {
                push_pairs(&mut rows, old, new, old_index, old_len, new_index, 0);
            }
            DiffOp::Insert {
                old_index,
                new_index,
                new_len,
            } => {
                push_pairs(&mut rows, old, new, old_index, 0, new_index, new_len);
            }
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                push_pairs(&mut rows, old, new, old_index, old_len, new_index, new_len);
            }
        }
    }
    rows
}

fn push_pairs(
    rows: &mut Vec<DiffRow>,
    old: &[&str],
    new: &[&str],
    old_index: usize,
    old_len: usize,
    new_index: usize,
    new_len: usize,
) {
    for offset in 0..old_len.max(new_len) {
        rows.push(DiffRow::Pair {
            old: (offset < old_len)
                .then(|| (old_index + offset + 1, old[old_index + offset].to_string())),
            new: (offset < new_len)
                .then(|| (new_index + offset + 1, new[new_index + offset].to_string())),
            old_pos: old_index + offset.min(old_len),
            new_pos: new_index + offset.min(new_len),
        });
    }
}

fn apply_context(rows: Vec<DiffRow>, context: usize) -> Vec<DiffRow> {
    if !rows.iter().any(|row| matches!(row, DiffRow::Pair { .. })) {
        return rows;
    }
    let mut output = Vec::new();
    let mut index = 0;
    while index < rows.len() {
        if !matches!(rows[index], DiffRow::Equal { .. }) {
            output.push(rows[index].clone());
            index += 1;
            continue;
        }
        let start = index;
        while index < rows.len() && matches!(rows[index], DiffRow::Equal { .. }) {
            index += 1;
        }
        let len = index - start;
        let keep_start = if start == 0 {
            context.min(len)
        } else {
            context.saturating_mul(2).min(len)
        };
        if start == 0 {
            output.extend(rows[index - keep_start..index].iter().cloned());
        } else if index == rows.len() {
            output.extend(rows[start..start + context.min(len)].iter().cloned());
        } else if len > keep_start {
            output.extend(rows[start..start + context].iter().cloned());
            output.push(DiffRow::Gap);
            output.extend(rows[index - context..index].iter().cloned());
        } else {
            output.extend(rows[start..index].iter().cloned());
        }
    }
    output
}

fn row_numbers(row: &DiffRow) -> (usize, usize) {
    match row {
        DiffRow::Equal { old, new, .. } => (*old, *new),
        DiffRow::Pair { old, new, .. } => (
            old.as_ref().map_or(0, |(line, _)| *line),
            new.as_ref().map_or(0, |(line, _)| *line),
        ),
        DiffRow::Hunk {
            old_start,
            new_start,
            ..
        } => (*old_start, *new_start),
        DiffRow::Gap => (0, 0),
    }
}

fn add_hunk_headers(rows: Vec<DiffRow>) -> Vec<DiffRow> {
    let mut output = Vec::new();
    for hunk in rows.split(|row| matches!(row, DiffRow::Gap)) {
        if hunk.is_empty() {
            continue;
        }
        let (old_start, new_start) = row_positions(&hunk[0]);
        let old_count = hunk.iter().filter(|row| row_has_old_line(row)).count();
        let new_count = hunk.iter().filter(|row| row_has_new_line(row)).count();
        output.push(DiffRow::Hunk {
            old_start: unified_start(old_start, old_count),
            old_count,
            new_start: unified_start(new_start, new_count),
            new_count,
        });
        output.extend_from_slice(hunk);
    }
    output
}

fn row_positions(row: &DiffRow) -> (usize, usize) {
    match row {
        DiffRow::Equal { old, new, .. } => (old - 1, new - 1),
        DiffRow::Pair {
            old_pos, new_pos, ..
        } => (*old_pos, *new_pos),
        _ => (0, 0),
    }
}

fn row_has_old_line(row: &DiffRow) -> bool {
    matches!(
        row,
        DiffRow::Equal { .. } | DiffRow::Pair { old: Some(_), .. }
    )
}

fn row_has_new_line(row: &DiffRow) -> bool {
    matches!(
        row,
        DiffRow::Equal { .. } | DiffRow::Pair { new: Some(_), .. }
    )
}

fn unified_start(zero_based_start: usize, count: usize) -> usize {
    zero_based_start + usize::from(count > 0)
}

fn hunk_header(old_start: usize, old_count: usize, new_start: usize, new_count: usize) -> String {
    format!("@@ -{old_start},{old_count} +{new_start},{new_count} @@")
}

fn word_parts(old: &str, new: &str, added: bool) -> Vec<StyledPart> {
    let old_words = old.split_word_bounds().collect::<Vec<_>>();
    let new_words = new.split_word_bounds().collect::<Vec<_>>();
    let ops = capture_diff_slices(Algorithm::Myers, &old_words, &new_words);
    let mut parts = Vec::new();
    for op in ops {
        let (index, len, equal) = match (op, added) {
            (
                DiffOp::Equal {
                    old_index,
                    new_index: _,
                    len,
                },
                false,
            ) => (old_index, len, true),
            (
                DiffOp::Equal {
                    old_index: _,
                    new_index,
                    len,
                },
                true,
            ) => (new_index, len, true),
            (
                DiffOp::Delete {
                    old_index, old_len, ..
                },
                false,
            ) => (old_index, old_len, false),
            (
                DiffOp::Insert {
                    new_index, new_len, ..
                },
                true,
            ) => (new_index, new_len, false),
            (
                DiffOp::Replace {
                    old_index, old_len, ..
                },
                false,
            ) => (old_index, old_len, false),
            (
                DiffOp::Replace {
                    new_index, new_len, ..
                },
                true,
            ) => (new_index, new_len, false),
            _ => continue,
        };
        let words = if added { &new_words } else { &old_words };
        parts.push(part(
            words[index..index + len].concat(),
            match (added, equal) {
                (true, true) => DiffRole::Added,
                (true, false) => DiffRole::AddedEmphasis,
                (false, true) => DiffRole::Removed,
                (false, false) => DiffRole::RemovedEmphasis,
            },
        ));
    }
    parts
}

fn number_width(rows: &[DiffRow]) -> usize {
    rows.iter()
        .map(row_numbers)
        .flat_map(|(old, new)| [old, new])
        .max()
        .unwrap_or(0)
        .to_string()
        .len()
}

fn unified_prefix(
    raw: bool,
    marker: char,
    old: Option<usize>,
    new: Option<usize>,
    width: usize,
) -> String {
    if raw {
        format!("{marker}")
    } else {
        format!(
            "{:>width$} {:>width$} {marker} ",
            old.map_or(String::new(), |line| line.to_string()),
            new.map_or(String::new(), |line| line.to_string()),
        )
    }
}

fn newline_marker_parts() -> Vec<StyledPart> {
    vec![part("\\ No newline at end of file", DiffRole::Muted)]
}

fn split_annotation_parts(
    text: &str,
    divider_column: usize,
    role: DiffRole,
) -> (Vec<StyledPart>, usize) {
    let (prefix, content_width) = if display_width(text) <= divider_column {
        (
            format!(
                "{text}{} │ ",
                " ".repeat(divider_column.saturating_sub(display_width(text)))
            ),
            display_width(text),
        )
    } else {
        (format!("{} │ {text}", " ".repeat(divider_column)), 0)
    };
    (vec![part(prefix, role)], content_width)
}

fn display_width(text: &str) -> usize {
    line_width(&Line::from(text))
}

fn pad_to_width(text: &str, width: usize) -> String {
    format!(
        "{text}{}",
        " ".repeat(width.saturating_sub(display_width(text)))
    )
}

fn part(text: impl Into<String>, role: DiffRole) -> StyledPart {
    StyledPart {
        text: text.into(),
        role,
    }
}

fn styled_line(parts: Vec<StyledPart>, continuation_indent: usize) -> StyledLine {
    StyledLine {
        parts,
        continuation_indent,
        side_left_content_width: None,
        location: None,
    }
}

fn located_styled_line(
    parts: Vec<StyledPart>,
    continuation_indent: usize,
    location: DiffLocation,
) -> StyledLine {
    StyledLine {
        parts,
        continuation_indent,
        side_left_content_width: None,
        location: Some(location),
    }
}

fn styled_line_with_location(
    parts: Vec<StyledPart>,
    continuation_indent: usize,
    location: Option<DiffLocation>,
) -> StyledLine {
    StyledLine {
        parts,
        continuation_indent,
        side_left_content_width: None,
        location,
    }
}

fn side_styled_line(
    parts: Vec<StyledPart>,
    continuation_indent: usize,
    side_left_content_width: usize,
    location: Option<DiffLocation>,
) -> StyledLine {
    StyledLine {
        parts,
        continuation_indent,
        side_left_content_width: Some(side_left_content_width),
        location,
    }
}

fn measure_parts(lines: &[StyledLine]) -> ScrollSize {
    let width = lines
        .iter()
        .map(|line| {
            line_width(&Line::from(
                line.parts
                    .iter()
                    .map(|part| Span::raw(part.text.as_str()))
                    .collect::<Vec<_>>(),
            ))
        })
        .max()
        .unwrap_or(0);
    ScrollSize::new(width, lines.len())
}

#[cfg(test)]
#[path = "diff_viewer_tests.rs"]
mod tests;
