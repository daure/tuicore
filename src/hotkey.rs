use std::time::Duration;

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

use crate::{Key, KeyEvent, KeyModifiers, border_chars, line_width};

pub const HOTKEY_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyLabelMode {
    Inline,
    PreferMnemonic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotkeyMatch {
    Ignored,
    Pending,
    Matched(usize),
    Canceled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeySequenceMatcher {
    hotkeys: Vec<String>,
    prefix: String,
    pending_match: Option<usize>,
    elapsed: Duration,
    timeout: Duration,
}

impl Default for HotkeySequenceMatcher {
    fn default() -> Self {
        Self::new(Vec::<String>::new())
    }
}

impl HotkeySequenceMatcher {
    pub fn new(hotkeys: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            hotkeys: hotkeys
                .into_iter()
                .map(|hotkey| normalize_hotkey(&hotkey.into()))
                .collect(),
            prefix: String::new(),
            pending_match: None,
            elapsed: Duration::ZERO,
            timeout: HOTKEY_TIMEOUT,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn set_hotkeys(&mut self, hotkeys: impl IntoIterator<Item = impl Into<String>>) {
        self.hotkeys = hotkeys
            .into_iter()
            .map(|hotkey| normalize_hotkey(&hotkey.into()))
            .collect();
        self.refresh_pending_match();
    }

    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    pub fn is_pending(&self) -> bool {
        !self.prefix.is_empty()
    }

    pub(crate) fn can_commit_pending(&self) -> bool {
        self.pending_match.is_some()
    }

    pub fn remaining_timeout(&self) -> Option<Duration> {
        self.is_pending()
            .then(|| self.timeout.saturating_sub(self.elapsed))
    }

    pub fn on_key(&mut self, key: KeyEvent) -> HotkeyMatch {
        if is_cancel_key(key) {
            return self.cancel();
        }
        if key.code == Key::Enter && key.modifiers == KeyModifiers::NONE {
            let Some(index) = self.pending_match else {
                if self.is_pending() {
                    self.clear();
                    return HotkeyMatch::Canceled;
                }
                return HotkeyMatch::Ignored;
            };
            self.clear();
            return HotkeyMatch::Matched(index);
        }
        if key.code == Key::Char(' ') && key.modifiers == KeyModifiers::NONE && self.is_pending() {
            self.elapsed = Duration::ZERO;
            return HotkeyMatch::Pending;
        }
        let Some(ch) = plain_char(key) else {
            return HotkeyMatch::Ignored;
        };

        let mut candidate = self.prefix.clone();
        candidate.push(ch.to_ascii_lowercase());

        let exact = self.hotkeys.iter().position(|hotkey| hotkey == &candidate);
        let has_longer = self
            .hotkeys
            .iter()
            .any(|hotkey| hotkey.starts_with(&candidate) && hotkey.len() > candidate.len());

        match (exact, has_longer) {
            (Some(index), false) => {
                self.clear();
                HotkeyMatch::Matched(index)
            }
            (exact, true) => {
                self.prefix = candidate;
                self.pending_match = exact;
                self.elapsed = Duration::ZERO;
                HotkeyMatch::Pending
            }
            (None, false) if self.prefix.is_empty() => HotkeyMatch::Ignored,
            (None, false) => {
                self.clear();
                HotkeyMatch::Canceled
            }
        }
    }

    pub fn tick(&mut self, dt: Duration) -> bool {
        if self.prefix.is_empty() {
            return false;
        }
        self.elapsed += dt;
        if self.elapsed >= self.timeout {
            self.clear();
            true
        } else {
            false
        }
    }

    pub fn cancel(&mut self) -> HotkeyMatch {
        if self.prefix.is_empty() {
            HotkeyMatch::Ignored
        } else {
            self.clear();
            HotkeyMatch::Canceled
        }
    }

    fn clear(&mut self) {
        self.prefix.clear();
        self.pending_match = None;
        self.elapsed = Duration::ZERO;
    }

    fn refresh_pending_match(&mut self) {
        if self.prefix.is_empty() {
            return;
        }

        let exact = self
            .hotkeys
            .iter()
            .position(|hotkey| hotkey == &self.prefix);
        let has_longer = self
            .hotkeys
            .iter()
            .any(|hotkey| hotkey.starts_with(&self.prefix) && hotkey.len() > self.prefix.len());

        if has_longer {
            self.pending_match = exact;
        } else {
            self.clear();
        }
    }
}

pub fn hotkey_label_spans(
    label: &str,
    hotkey: Option<&str>,
    mode: HotkeyLabelMode,
    active_prefix: Option<&str>,
    base_style: Style,
    hotkey_style: Style,
) -> Vec<Span<'static>> {
    let Some(hotkey) = hotkey else {
        return vec![Span::styled(label.to_owned(), base_style)];
    };
    let hotkey = normalize_hotkey(hotkey);
    if hotkey.is_empty() {
        return vec![Span::styled(label.to_owned(), base_style)];
    }

    let active_prefix = active_prefix
        .map(normalize_hotkey)
        .filter(|prefix| !prefix.is_empty() && badge_contains_prefix(&hotkey, prefix));

    let active_prefix_is_partial = active_prefix
        .as_deref()
        .is_some_and(|prefix| prefix.len() < hotkey.len());

    if mode == HotkeyLabelMode::PreferMnemonic
        && !active_prefix_is_partial
        && let Some(highlight) = active_prefix.as_deref().or(Some(&hotkey))
        && let Some((start, end)) = find_case_insensitive(label, highlight)
    {
        return split_label(label, start, end, base_style, hotkey_style);
    }

    let mut spans = vec![Span::styled(label.to_owned(), base_style)];
    spans.push(Span::styled(" ", base_style));
    spans.push(Span::styled("|", base_style));
    if let Some(highlight) = active_prefix.as_deref() {
        spans.extend(split_hotkey_badge(
            &hotkey,
            highlight,
            base_style,
            hotkey_style,
        ));
    } else {
        spans.push(Span::styled(hotkey, base_style));
    }
    spans.push(Span::styled("|", base_style));
    spans
}

pub fn hotkey_underline_style(style: Style) -> Style {
    style.add_modifier(Modifier::UNDERLINED)
}

pub fn hotkey_badge_spans(
    hotkey: &str,
    active_prefix: Option<&str>,
    border: crate::BorderKind,
    border_style: Style,
    hotkey_style: Style,
    highlight_style: Style,
) -> Vec<Span<'static>> {
    let chars = border_chars(border);
    let hotkey = normalize_hotkey(hotkey);
    let active_prefix = active_prefix
        .map(normalize_hotkey)
        .filter(|prefix| !prefix.is_empty() && badge_contains_prefix(&hotkey, prefix));

    let mut spans = vec![Span::styled(chars.right_join, border_style)];
    if let Some(highlight) = active_prefix.as_deref() {
        spans.extend(split_hotkey_badge(
            &hotkey,
            highlight,
            hotkey_style,
            highlight_style,
        ));
    } else {
        spans.push(Span::styled(hotkey, hotkey_style));
    }
    spans.push(Span::styled(chars.left_join, border_style));
    spans
}

pub fn hotkey_edge_spans(
    hotkey: &str,
    active_prefix: Option<&str>,
    border: crate::BorderKind,
    border_style: Style,
    hotkey_style: Style,
    highlight_style: Style,
) -> Vec<Span<'static>> {
    let chars = border_chars(border);
    let hotkey = normalize_hotkey(hotkey);
    let active_prefix = active_prefix
        .map(normalize_hotkey)
        .filter(|prefix| !prefix.is_empty() && badge_contains_prefix(&hotkey, prefix));

    let mut spans = vec![Span::styled(chars.right_join, border_style)];
    if let Some(highlight) = active_prefix.as_deref() {
        spans.extend(split_hotkey_badge(
            &hotkey,
            highlight,
            hotkey_style,
            highlight_style,
        ));
    } else {
        spans.push(Span::styled(hotkey, hotkey_style));
    }
    spans.push(Span::styled(chars.vertical, border_style));
    spans
}

pub fn hotkey_badge_width(hotkey: &str) -> usize {
    line_width(&ratatui::text::Line::from(normalize_hotkey(hotkey))) + 2
}

pub fn hotkey_sequence_to_event(hotkey: &str) -> Option<KeyEvent> {
    let normalized = normalize_hotkey(hotkey);
    let mut chars = normalized.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    Some(KeyEvent {
        code: Key::Char(c),
        modifiers: KeyModifiers::NONE,
    })
}

pub fn hotkey_starts_with_event(hotkey: &str, key: KeyEvent) -> bool {
    plain_char(key)
        .map(|ch| normalize_hotkey(hotkey).starts_with(ch.to_ascii_lowercase()))
        .unwrap_or(false)
}

pub fn normalize_hotkey(hotkey: &str) -> String {
    hotkey
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

fn split_label(
    label: &str,
    start: usize,
    end: usize,
    base_style: Style,
    hotkey_style: Style,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    if start > 0 {
        spans.push(Span::styled(label[..start].to_owned(), base_style));
    }
    spans.push(Span::styled(label[start..end].to_owned(), hotkey_style));
    if end < label.len() {
        spans.push(Span::styled(label[end..].to_owned(), base_style));
    }
    spans
}

fn split_hotkey(
    hotkey: &str,
    highlight: &str,
    base_style: Style,
    hotkey_style: Style,
) -> Vec<Span<'static>> {
    let highlight_len = highlight.len().min(hotkey.len());
    let mut spans = Vec::new();
    if highlight_len > 0 {
        spans.push(Span::styled(
            hotkey[..highlight_len].to_owned(),
            hotkey_style,
        ));
    }
    if highlight_len < hotkey.len() {
        spans.push(Span::styled(hotkey[highlight_len..].to_owned(), base_style));
    }
    spans
}

fn split_hotkey_badge(
    hotkey: &str,
    highlight: &str,
    base_style: Style,
    hotkey_style: Style,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (index, sequence) in hotkey.split('·').enumerate() {
        if index > 0 {
            spans.push(Span::styled("·", base_style));
        }
        if sequence.starts_with(highlight) {
            spans.extend(split_hotkey(sequence, highlight, base_style, hotkey_style));
        } else {
            spans.push(Span::styled(sequence.to_owned(), base_style));
        }
    }
    spans
}

fn badge_contains_prefix(hotkey: &str, prefix: &str) -> bool {
    hotkey
        .split('·')
        .any(|sequence| sequence.starts_with(prefix))
}

fn find_case_insensitive(value: &str, needle: &str) -> Option<(usize, usize)> {
    let normalized_value = value.to_ascii_lowercase();
    let start = normalized_value.find(needle)?;
    let end = start + needle.len();
    Some((start, end))
}

fn plain_char(key: KeyEvent) -> Option<char> {
    if key.modifiers != KeyModifiers::NONE {
        return None;
    }
    let Key::Char(ch) = key.code else {
        return None;
    };
    Some(ch)
}

fn is_cancel_key(key: KeyEvent) -> bool {
    key.code == Key::Esc
        || (key.code == Key::Char('[') && key.modifiers.contains(KeyModifiers::CONTROL))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefer_mnemonic_highlights_existing_letter_without_suffix() {
        let spans = hotkey_label_spans(
            "Overview",
            Some("o"),
            HotkeyLabelMode::PreferMnemonic,
            None,
            Style::default(),
            Style::default().add_modifier(ratatui::style::Modifier::BOLD),
        );

        assert_eq!(
            spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>(),
            "Overview"
        );
        assert!(
            spans[0]
                .style
                .add_modifier
                .contains(ratatui::style::Modifier::BOLD)
        );
    }

    #[test]
    fn prefer_mnemonic_falls_back_to_inline_suffix() {
        let spans = hotkey_label_spans(
            "Run",
            Some("x"),
            HotkeyLabelMode::PreferMnemonic,
            None,
            Style::default(),
            Style::default(),
        );

        assert_eq!(
            spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>(),
            "Run |x|"
        );
    }

    #[test]
    fn prefer_mnemonic_keeps_suffix_for_partial_multiletter_prefix() {
        let base_style = Style::default();
        let hotkey_style = Style::default().add_modifier(ratatui::style::Modifier::UNDERLINED);
        let spans = hotkey_label_spans(
            "Open real tabs-as-dialog overlay",
            Some("td"),
            HotkeyLabelMode::PreferMnemonic,
            Some("t"),
            base_style,
            hotkey_style,
        );

        assert_eq!(
            spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>(),
            "Open real tabs-as-dialog overlay |td|"
        );
        let underlined_t = spans
            .iter()
            .find(|span| span.content.as_ref() == "t")
            .expect("partial prefix should be rendered in hotkey suffix");
        assert!(
            underlined_t
                .style
                .add_modifier
                .contains(ratatui::style::Modifier::UNDERLINED)
        );
    }

    #[test]
    fn inline_mode_does_not_highlight_without_active_prefix() {
        let base_style = Style::default();
        let hotkey_style = Style::default().add_modifier(ratatui::style::Modifier::BOLD);
        let spans = hotkey_label_spans(
            "Overview",
            Some("o"),
            HotkeyLabelMode::Inline,
            None,
            base_style,
            hotkey_style,
        );

        assert_eq!(
            spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>(),
            "Overview |o|"
        );
        assert!(spans.iter().all(|span| span.style == base_style));
    }

    #[test]
    fn matcher_waits_for_multiletter_completion() {
        let mut matcher = HotkeySequenceMatcher::new(["op", "ov"]);

        assert_eq!(
            matcher.on_key(KeyEvent::from(Key::Char('o'))),
            HotkeyMatch::Pending
        );
        assert_eq!(matcher.prefix(), "o");
        assert_eq!(
            matcher.on_key(KeyEvent::from(Key::Char('v'))),
            HotkeyMatch::Matched(1)
        );
        assert_eq!(matcher.prefix(), "");
    }

    #[test]
    fn matcher_times_out_pending_prefix() {
        let mut matcher = HotkeySequenceMatcher::new(["ov"]);

        matcher.on_key(KeyEvent::from(Key::Char('o')));

        assert!(matcher.tick(HOTKEY_TIMEOUT));
        assert_eq!(matcher.prefix(), "");
    }

    #[test]
    fn matcher_reports_remaining_timeout_for_pending_prefix() {
        let mut matcher = HotkeySequenceMatcher::new(["ov"]);

        assert_eq!(matcher.remaining_timeout(), None);
        matcher.on_key(KeyEvent::from(Key::Char('o')));
        matcher.tick(Duration::from_millis(500));

        assert_eq!(
            matcher.remaining_timeout(),
            Some(HOTKEY_TIMEOUT - Duration::from_millis(500))
        );
    }

    #[test]
    fn matcher_commits_ambiguous_exact_match_on_enter() {
        let mut matcher = HotkeySequenceMatcher::new(["s", "sa"]);

        assert_eq!(
            matcher.on_key(KeyEvent::from(Key::Char('s'))),
            HotkeyMatch::Pending
        );
        assert!(matcher.can_commit_pending());
        assert_eq!(
            matcher.on_key(KeyEvent::from(Key::Enter)),
            HotkeyMatch::Matched(0)
        );
    }

    #[test]
    fn matcher_cancels_incomplete_prefix_on_enter() {
        let mut matcher = HotkeySequenceMatcher::new(["v", "sa", "ta"]);

        assert_eq!(
            matcher.on_key(KeyEvent::from(Key::Char('t'))),
            HotkeyMatch::Pending
        );
        assert!(!matcher.can_commit_pending());
        assert_eq!(
            matcher.on_key(KeyEvent::from(Key::Enter)),
            HotkeyMatch::Canceled
        );
        assert!(!matcher.is_pending());
    }

    #[test]
    fn matcher_keeps_multiletter_exact_pending_when_longer_exists() {
        let mut matcher = HotkeySequenceMatcher::new(["ta", "tat"]);

        assert_eq!(
            matcher.on_key(KeyEvent::from(Key::Char('t'))),
            HotkeyMatch::Pending
        );
        assert_eq!(
            matcher.on_key(KeyEvent::from(Key::Char('a'))),
            HotkeyMatch::Pending
        );
        assert_eq!(
            matcher.on_key(KeyEvent::from(Key::Char('t'))),
            HotkeyMatch::Matched(1)
        );
    }

    #[test]
    fn matcher_cancels_pending_sequence_on_invalid_continuation() {
        let mut matcher = HotkeySequenceMatcher::new(["ta", "tat"]);

        assert_eq!(
            matcher.on_key(KeyEvent::from(Key::Char('t'))),
            HotkeyMatch::Pending
        );
        assert_eq!(
            matcher.on_key(KeyEvent::from(Key::Char('s'))),
            HotkeyMatch::Canceled
        );
        assert_eq!(matcher.prefix(), "");
    }

    #[test]
    fn matcher_ignores_space_during_pending_sequence() {
        let mut matcher = HotkeySequenceMatcher::new(["ma", "mam"]);

        assert_eq!(
            matcher.on_key(KeyEvent::from(Key::Char('m'))),
            HotkeyMatch::Pending
        );
        assert_eq!(
            matcher.on_key(KeyEvent::from(Key::Char(' '))),
            HotkeyMatch::Pending
        );
        assert_eq!(matcher.prefix(), "m");
        assert_eq!(
            matcher.on_key(KeyEvent::from(Key::Char('a'))),
            HotkeyMatch::Pending
        );
        assert_eq!(
            matcher.on_key(KeyEvent::from(Key::Char('m'))),
            HotkeyMatch::Matched(1)
        );
    }

    #[test]
    fn matcher_recomputes_pending_match_after_hotkey_reorder() {
        let mut matcher = HotkeySequenceMatcher::new(["s", "sa"]);

        assert_eq!(
            matcher.on_key(KeyEvent::from(Key::Char('s'))),
            HotkeyMatch::Pending
        );
        matcher.set_hotkeys(["sa", "s"]);

        assert_eq!(
            matcher.on_key(KeyEvent::from(Key::Enter)),
            HotkeyMatch::Matched(1)
        );
    }

    #[test]
    fn matcher_clears_pending_prefix_when_updated_hotkeys_no_longer_match() {
        let mut matcher = HotkeySequenceMatcher::new(["s", "sa"]);

        assert_eq!(
            matcher.on_key(KeyEvent::from(Key::Char('s'))),
            HotkeyMatch::Pending
        );
        matcher.set_hotkeys(["x", "xa"]);

        assert_eq!(matcher.prefix(), "");
        assert_eq!(
            matcher.on_key(KeyEvent::from(Key::Enter)),
            HotkeyMatch::Ignored
        );
    }

    #[test]
    fn hotkey_sequence_to_event_only_returns_single_char_hotkeys() {
        assert_eq!(
            hotkey_sequence_to_event(" S "),
            Some(KeyEvent::from(Key::Char('s')))
        );
        assert_eq!(hotkey_sequence_to_event("g g"), None);
    }
}
