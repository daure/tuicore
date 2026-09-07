use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::{Key, KeyEvent, KeyModifiers, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    Contains,
    Fuzzy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub score: i64,
    pub spans: Vec<MatchSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchSpan {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedSearchMatch {
    pub index: usize,
    pub score: i64,
    pub spans: Vec<MatchSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextMatchRange {
    pub start: usize,
    pub end: usize,
}

impl TextMatchRange {
    pub(crate) fn contains(self, position: usize) -> bool {
        position >= self.start && position < self.end
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SearchAction {
    Started,
    QueryChanged,
    Submitted,
    Cleared,
    Next,
    Previous,
    Handled,
    Unhandled,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SearchState {
    query: String,
    active: bool,
    editing: bool,
    selected: usize,
}

impl SearchState {
    pub(crate) fn query(&self) -> &str {
        &self.query
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active
    }

    pub(crate) fn is_editing(&self) -> bool {
        self.editing
    }

    pub(crate) fn clear(&mut self) -> bool {
        let changed = self.active || self.editing || !self.query.is_empty() || self.selected != 0;
        self.query.clear();
        self.active = false;
        self.editing = false;
        self.selected = 0;
        changed
    }

    pub(crate) fn append(&mut self, value: &str) -> bool {
        if !self.editing {
            return false;
        }
        let previous_len = self.query.len();
        self.query
            .extend(value.chars().filter(|value| !value.is_control()));
        if self.query.len() != previous_len {
            self.selected = 0;
            true
        } else {
            false
        }
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> SearchAction {
        if self.active && cancel_key(key) {
            self.clear();
            return SearchAction::Cleared;
        }

        if self.editing {
            return match key.code {
                Key::Enter if key.modifiers.user_modifiers().is_empty() => {
                    self.editing = false;
                    SearchAction::Submitted
                }
                Key::Backspace if key.modifiers.user_modifiers().is_empty() => {
                    self.query.pop();
                    self.selected = 0;
                    SearchAction::QueryChanged
                }
                Key::Char(value)
                    if !value.is_control()
                        && !key
                            .modifiers
                            .user_modifiers()
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    self.query.push(value);
                    self.selected = 0;
                    SearchAction::QueryChanged
                }
                _ => SearchAction::Handled,
            };
        }

        if plain_char(key, '/') {
            self.query.clear();
            self.active = true;
            self.editing = true;
            self.selected = 0;
            return SearchAction::Started;
        }
        if self.active && plain_char(key, 'n') {
            return SearchAction::Next;
        }
        if self.active && shifted_n(key) {
            return SearchAction::Previous;
        }
        SearchAction::Unhandled
    }

    pub(crate) fn select_next(&mut self, match_count: usize) {
        if match_count > 0 {
            self.selected = (self.selected + 1) % match_count;
        }
    }

    pub(crate) fn select_previous(&mut self, match_count: usize) {
        if match_count > 0 {
            self.selected = (self.selected + match_count - 1) % match_count;
        }
    }

    pub(crate) fn selected(&self, match_count: usize) -> Option<usize> {
        (match_count > 0).then(|| self.selected.min(match_count - 1))
    }

    pub(crate) fn line(&self, match_count: usize) -> Line<'static> {
        let theme = theme();
        let current = self
            .selected(match_count)
            .map_or(0, |selected| selected + 1);
        let mut spans = vec![
            Span::styled("/", Style::default().fg(theme.accent_fg())),
            Span::styled(self.query.clone(), Style::default().fg(theme.text_fg())),
        ];
        if self.editing {
            spans.push(Span::styled(
                " ",
                Style::default()
                    .fg(theme.highlight_fg())
                    .bg(theme.highlight_bg()),
            ));
        }
        spans.push(Span::styled(
            format!("  {current}/{match_count}"),
            Style::default().fg(theme.muted_fg()),
        ));
        Line::from(spans)
    }
}

pub(crate) fn text_match_ranges(query: &str, candidate: &str) -> Vec<TextMatchRange> {
    let query = query.chars().collect::<Vec<_>>();
    if query.is_empty() {
        return Vec::new();
    }
    let candidate = candidate.chars().collect::<Vec<_>>();
    if query.len() > candidate.len() {
        return Vec::new();
    }

    (0..=candidate.len() - query.len())
        .filter(|start| {
            query.iter().enumerate().all(|(offset, expected)| {
                chars_eq_ignore_case(candidate[*start + offset], *expected)
            })
        })
        .map(|start| TextMatchRange {
            start,
            end: start + query.len(),
        })
        .collect()
}

pub(crate) fn split_search_area(area: Rect, active: bool) -> (Rect, Option<Rect>) {
    if !active || area.height == 0 {
        return (area, None);
    }
    let content = Rect::new(area.x, area.y, area.width, area.height.saturating_sub(1));
    let search = Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1);
    (content, Some(search))
}

pub(crate) fn search_match_style(current: bool) -> Style {
    let style = Style::default().fg(theme().accent_fg());
    if current {
        style.add_modifier(Modifier::UNDERLINED | Modifier::BOLD)
    } else {
        style
    }
}

fn cancel_key(key: KeyEvent) -> bool {
    key.code == Key::Esc
        || (key.code == Key::Char('[') && key.modifiers.user_modifiers() == KeyModifiers::CONTROL)
}

fn plain_char(key: KeyEvent, expected: char) -> bool {
    key.code == Key::Char(expected) && key.modifiers.user_modifiers().is_empty()
}

fn shifted_n(key: KeyEvent) -> bool {
    (key.code == Key::Char('N')
        && matches!(
            key.modifiers.user_modifiers(),
            KeyModifiers::NONE | KeyModifiers::SHIFT
        ))
        || (key.code == Key::Char('n') && key.modifiers.user_modifiers() == KeyModifiers::SHIFT)
}

pub fn search_match(query: &str, candidate: &str, mode: SearchMode) -> Option<SearchMatch> {
    match mode {
        SearchMode::Contains => contains_match(query, candidate),
        SearchMode::Fuzzy => fuzzy_match(query, candidate),
    }
}

pub fn search_ranked<I, S>(query: &str, candidates: I, mode: SearchMode) -> Vec<RankedSearchMatch>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut matches = candidates
        .into_iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            search_match(query, candidate.as_ref(), mode).map(|matched| RankedSearchMatch {
                index,
                score: matched.score,
                spans: matched.spans,
            })
        })
        .collect::<Vec<_>>();

    if mode == SearchMode::Fuzzy {
        matches.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.index.cmp(&right.index))
        });
    }

    matches
}

fn contains_match(query: &str, candidate: &str) -> Option<SearchMatch> {
    if query.is_empty() {
        return Some(SearchMatch {
            score: 0,
            spans: Vec::new(),
        });
    }

    let query = query.chars().collect::<Vec<_>>();
    let candidate_chars = candidate.char_indices().collect::<Vec<_>>();
    if query.len() > candidate_chars.len() {
        return None;
    }

    for start in 0..=candidate_chars.len().saturating_sub(query.len()) {
        let matched = query.iter().enumerate().all(|(offset, expected)| {
            chars_eq_ignore_case(candidate_chars[start + offset].1, *expected)
        });
        if matched {
            let start_byte = candidate_chars[start].0;
            let end_index = start + query.len();
            let end_byte = candidate_chars
                .get(end_index)
                .map(|(index, _)| *index)
                .unwrap_or(candidate.len());
            return Some(SearchMatch {
                score: query.len() as i64,
                spans: vec![MatchSpan {
                    start: start_byte,
                    end: end_byte,
                }],
            });
        }
    }

    None
}

fn fuzzy_match(query: &str, candidate: &str) -> Option<SearchMatch> {
    if query.is_empty() {
        return Some(SearchMatch {
            score: 0,
            spans: Vec::new(),
        });
    }

    let query = query.chars().collect::<Vec<_>>();
    let candidate_chars = candidate.char_indices().collect::<Vec<_>>();
    let mut query_index = 0;
    let mut matched_indices = Vec::with_capacity(query.len());

    for (candidate_index, (_, candidate_char)) in candidate_chars.iter().enumerate() {
        if chars_eq_ignore_case(*candidate_char, query[query_index]) {
            matched_indices.push(candidate_index);
            query_index += 1;
            if query_index == query.len() {
                break;
            }
        }
    }

    if query_index != query.len() {
        return None;
    }

    Some(SearchMatch {
        score: fuzzy_score(&candidate_chars, &matched_indices),
        spans: match_spans(candidate, &candidate_chars, &matched_indices),
    })
}

fn fuzzy_score(candidate: &[(usize, char)], matched_indices: &[usize]) -> i64 {
    let mut score = 0;
    let first = matched_indices.first().copied().unwrap_or(0);

    for (position, index) in matched_indices.iter().copied().enumerate() {
        score += 10;
        if position > 0 && matched_indices[position - 1] + 1 == index {
            score += 12;
        }
        if is_boundary(candidate, index) {
            score += 8;
        }
        score -= index as i64;
    }

    score - first as i64
}

fn match_spans(
    candidate: &str,
    candidate_chars: &[(usize, char)],
    matched_indices: &[usize],
) -> Vec<MatchSpan> {
    let mut spans = Vec::new();
    let Some(mut span_start_index) = matched_indices.first().copied() else {
        return spans;
    };
    let mut previous = span_start_index;

    for index in matched_indices.iter().copied().skip(1) {
        if previous + 1 == index {
            previous = index;
            continue;
        }
        spans.push(span_from_indices(
            candidate,
            candidate_chars,
            span_start_index,
            previous,
        ));
        span_start_index = index;
        previous = index;
    }

    spans.push(span_from_indices(
        candidate,
        candidate_chars,
        span_start_index,
        previous,
    ));
    spans
}

fn span_from_indices(
    candidate: &str,
    candidate_chars: &[(usize, char)],
    start: usize,
    end: usize,
) -> MatchSpan {
    MatchSpan {
        start: candidate_chars[start].0,
        end: candidate_chars
            .get(end + 1)
            .map(|(index, _)| *index)
            .unwrap_or(candidate.len()),
    }
}

fn is_boundary(candidate: &[(usize, char)], index: usize) -> bool {
    if index == 0 {
        return true;
    }

    let previous = candidate[index - 1].1;
    let current = candidate[index].1;
    !previous.is_alphanumeric()
        || (previous.is_lowercase() && current.is_uppercase())
        || previous == '_'
}

fn chars_eq_ignore_case(left: char, right: char) -> bool {
    left.to_lowercase().eq(right.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_matches_case_insensitively_and_preserves_order() {
        let matches = search_ranked(
            "ap",
            ["Grape", "apple", "Paper", "pear"],
            SearchMode::Contains,
        );

        assert_eq!(
            matches
                .iter()
                .map(|matched| matched.index)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert_eq!(matches[0].spans, vec![MatchSpan { start: 2, end: 4 }]);
    }

    #[test]
    fn fuzzy_requires_subsequence_and_returns_match_spans() {
        let matched =
            search_match("fb", "foo_bar", SearchMode::Fuzzy).expect("subsequence should match");

        assert_eq!(
            matched.spans,
            vec![
                MatchSpan { start: 0, end: 1 },
                MatchSpan { start: 4, end: 5 },
            ]
        );
        assert!(search_match("fz", "foo_bar", SearchMode::Fuzzy).is_none());
    }

    #[test]
    fn fuzzy_ranks_consecutive_boundary_and_early_matches() {
        let matches = search_ranked("ab", ["xxab", "a-b", "zz_a_b", "ab"], SearchMode::Fuzzy);

        assert_eq!(
            matches
                .iter()
                .map(|matched| matched.index)
                .collect::<Vec<_>>(),
            vec![3, 1, 0, 2]
        );
    }

    #[test]
    fn empty_query_matches_without_spans() {
        let matched =
            search_match("", "anything", SearchMode::Contains).expect("empty query should match");

        assert_eq!(matched.score, 0);
        assert!(matched.spans.is_empty());
    }

    #[test]
    fn text_ranges_find_overlapping_unicode_matches_case_insensitively() {
        assert_eq!(
            text_match_ranges("Éé", "éÉé"),
            vec![
                TextMatchRange { start: 0, end: 2 },
                TextMatchRange { start: 1, end: 3 },
            ]
        );
    }

    #[test]
    fn search_navigation_wraps_in_both_directions() {
        let mut search = SearchState::default();
        assert_eq!(
            search.handle_key(Key::Char('/').into()),
            SearchAction::Started
        );
        assert_eq!(
            search.handle_key(Key::Char('x').into()),
            SearchAction::QueryChanged
        );
        assert_eq!(
            search.handle_key(Key::Enter.into()),
            SearchAction::Submitted
        );

        search.select_previous(3);
        assert_eq!(search.selected(3), Some(2));
        search.select_next(3);
        assert_eq!(search.selected(3), Some(0));
    }

    #[test]
    fn escape_and_control_bracket_clear_search() {
        for key in [
            KeyEvent::from(Key::Esc),
            KeyEvent {
                code: Key::Char('['),
                modifiers: KeyModifiers::CONTROL,
            },
        ] {
            let mut search = SearchState::default();
            search.handle_key(Key::Char('/').into());
            search.handle_key(Key::Char('x').into());

            assert_eq!(search.handle_key(key), SearchAction::Cleared);
            assert!(!search.is_active());
            assert!(search.query().is_empty());
        }
    }
}
