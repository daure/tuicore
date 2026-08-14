mod markdown;

use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Gauge, Paragraph, Wrap};
use unicode_segmentation::UnicodeSegmentation;

use crate::event::{Key, KeyEvent, TuiEvent};
use crate::{
    Animated, AnimationSettings, AxisExpand, EventCtx, EventOutcome, FocusId, HintSource, KeySpec,
    LayoutCtx, LayoutProposal, LayoutResult, LayoutSize, LayoutSizeHint, TickResult, TuiNode,
    line_width, theme,
};

use super::dialog::{Dialog, DialogCloseReason, DialogHost};

const DEFAULT_WPM: u16 = 300;
const MIN_WPM: u16 = 100;
const MAX_WPM: u16 = 1_000;
const WPM_STEP: u16 = 25;
const PREFERRED_WIDTH: u16 = 64;
const PREFERRED_HEIGHT: u16 = 8;
const SPEED_READER_FOCUS: &str = "speed-reader";
const DEFAULT_TITLE: &str = "Speed reader";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedReaderInputMode {
    Plain,
    Markdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedReaderState {
    Empty,
    Paused,
    Playing,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpeedReaderOutcome {
    pub handled: bool,
    pub changed: bool,
}

impl SpeedReaderOutcome {
    pub const IGNORED: Self = Self {
        handled: false,
        changed: false,
    };

    const CHANGED: Self = Self {
        handled: true,
        changed: true,
    };

    const HANDLED: Self = Self {
        handled: true,
        changed: false,
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeedReaderKeyBindings {
    pub toggle: Vec<KeySpec>,
    pub faster: Vec<KeySpec>,
    pub slower: Vec<KeySpec>,
    pub previous_word: Vec<KeySpec>,
    pub next_word: Vec<KeySpec>,
    pub previous_block: Vec<KeySpec>,
    pub next_block: Vec<KeySpec>,
    pub restart: Vec<KeySpec>,
}

impl Default for SpeedReaderKeyBindings {
    fn default() -> Self {
        Self {
            toggle: vec![KeySpec::plain(' ')],
            faster: vec![KeySpec::plain('+'), KeySpec::plain('=')],
            slower: vec![KeySpec::plain('-')],
            previous_word: vec![KeySpec::plain('h'), KeySpec::key(Key::Left)],
            next_word: vec![KeySpec::plain('l'), KeySpec::key(Key::Right)],
            previous_block: vec![KeySpec::plain('k'), KeySpec::key(Key::Up)],
            next_block: vec![KeySpec::plain('j'), KeySpec::key(Key::Down)],
            restart: vec![KeySpec::plain('r'), KeySpec::key(Key::Home)],
        }
    }
}

impl SpeedReaderKeyBindings {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct InlineMarks {
    emphasis: bool,
    strong: bool,
    link: bool,
}

#[derive(Debug, Clone)]
struct ReaderToken {
    text: String,
    fragments: Vec<(String, InlineMarks)>,
    prefix: String,
    block: usize,
    heading: bool,
    boundary_after: bool,
}

#[derive(Debug, Clone)]
pub struct SpeedReader {
    title: String,
    source: String,
    mode: SpeedReaderInputMode,
    tokens: Vec<ReaderToken>,
    position: usize,
    state: SpeedReaderState,
    wpm: u16,
    elapsed: Duration,
    natural_pauses: bool,
    keys: SpeedReaderKeyBindings,
    area: Rect,
}

impl SpeedReader {
    pub fn new(source: impl Into<String>) -> Self {
        Self::from_source(source.into(), SpeedReaderInputMode::Plain)
    }

    pub fn markdown(source: impl Into<String>) -> Self {
        Self::from_source(source.into(), SpeedReaderInputMode::Markdown)
    }

    fn from_source(source: String, mode: SpeedReaderInputMode) -> Self {
        let tokens = parse_source(&source, mode);
        let state = if tokens.is_empty() {
            SpeedReaderState::Empty
        } else {
            SpeedReaderState::Paused
        };
        Self {
            title: DEFAULT_TITLE.to_string(),
            source,
            mode,
            tokens,
            position: 0,
            state,
            wpm: DEFAULT_WPM,
            elapsed: Duration::ZERO,
            natural_pauses: true,
            keys: SpeedReaderKeyBindings::default(),
            area: Rect::default(),
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.set_title(title);
        self
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    pub fn title_text(&self) -> &str {
        &self.title
    }

    pub fn dialog<M>(
        self,
        on_close: impl Fn(DialogCloseReason) -> M + 'static,
    ) -> DialogHost<Self, M> {
        let title = self.title.clone();
        Dialog::new().top_left(title).on_close(on_close).host(self)
    }

    pub fn input_mode(mut self, mode: SpeedReaderInputMode) -> Self {
        self.set_input_mode(mode);
        self
    }

    pub fn set_input_mode(&mut self, mode: SpeedReaderInputMode) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        self.reparse();
    }

    pub fn set_source(&mut self, source: impl Into<String>) {
        self.source = source.into();
        self.reparse();
    }

    pub fn wpm(mut self, wpm: u16) -> Self {
        self.set_wpm(wpm);
        self
    }

    pub fn set_wpm(&mut self, wpm: u16) {
        let old_dwell = self.current_dwell();
        self.wpm = wpm.clamp(MIN_WPM, MAX_WPM);
        let progress = if old_dwell.is_zero() {
            0.0
        } else {
            self.elapsed.as_secs_f64() / old_dwell.as_secs_f64()
        };
        self.elapsed = self.current_dwell().mul_f64(progress.clamp(0.0, 1.0));
    }

    pub fn natural_pauses(mut self, enabled: bool) -> Self {
        self.natural_pauses = enabled;
        self
    }

    pub fn keybindings(mut self, keys: SpeedReaderKeyBindings) -> Self {
        self.keys = keys;
        self
    }

    pub fn state(&self) -> SpeedReaderState {
        self.state
    }

    pub fn current_word(&self) -> Option<&str> {
        self.tokens
            .get(self.position)
            .map(|token| token.text.as_str())
    }

    pub fn current_context(&self) -> Option<&str> {
        self.tokens
            .get(self.position)
            .map(|token| token.prefix.as_str())
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn token_count(&self) -> usize {
        self.tokens.len()
    }

    pub fn progress(&self) -> f64 {
        if self.tokens.is_empty() {
            0.0
        } else if self.state == SpeedReaderState::Complete {
            1.0
        } else {
            (self.position + 1) as f64 / self.tokens.len() as f64
        }
    }

    pub fn play(&mut self) {
        if self.state == SpeedReaderState::Complete {
            self.position = 0;
            self.elapsed = Duration::ZERO;
        }
        if !self.tokens.is_empty() {
            self.state = SpeedReaderState::Playing;
        }
    }

    pub fn pause(&mut self) {
        if self.state == SpeedReaderState::Playing {
            self.state = SpeedReaderState::Paused;
        }
    }

    pub fn toggle(&mut self) {
        if self.state == SpeedReaderState::Playing {
            self.pause();
        } else {
            self.play();
        }
    }

    pub fn restart_paused(&mut self) {
        self.position = 0;
        self.elapsed = Duration::ZERO;
        self.state = if self.tokens.is_empty() {
            SpeedReaderState::Empty
        } else {
            SpeedReaderState::Paused
        };
    }

    pub fn restart_playing(&mut self) {
        self.restart_paused();
        self.play();
    }

    pub fn previous(&mut self) {
        if !self.tokens.is_empty() {
            self.set_position(self.position.saturating_sub(1));
            self.elapsed = Duration::ZERO;
            self.state = SpeedReaderState::Paused;
        }
    }

    pub fn next(&mut self) {
        if !self.tokens.is_empty() {
            self.set_position((self.position + 1).min(self.tokens.len() - 1));
            self.elapsed = Duration::ZERO;
            self.state = SpeedReaderState::Paused;
        }
    }

    pub fn on_key(&mut self, key: impl Into<KeyEvent>) -> SpeedReaderOutcome {
        let key = key.into();
        let previous = (self.position, self.state, self.wpm, self.elapsed);
        if matches_any(&self.keys.toggle, key) {
            self.toggle();
        } else if matches_any(&self.keys.faster, key) {
            self.set_wpm(self.wpm.saturating_add(WPM_STEP));
        } else if matches_any(&self.keys.slower, key) {
            self.set_wpm(self.wpm.saturating_sub(WPM_STEP));
        } else if matches_any(&self.keys.previous_word, key) {
            self.previous();
        } else if matches_any(&self.keys.next_word, key) {
            self.next();
        } else if self.mode == SpeedReaderInputMode::Markdown
            && matches_any(&self.keys.previous_block, key)
        {
            self.seek_previous_block();
        } else if self.mode == SpeedReaderInputMode::Markdown
            && matches_any(&self.keys.next_block, key)
        {
            self.seek_next_block();
        } else if matches_any(&self.keys.restart, key) {
            self.restart_paused();
        } else {
            return SpeedReaderOutcome::IGNORED;
        }
        if previous == (self.position, self.state, self.wpm, self.elapsed) {
            SpeedReaderOutcome::HANDLED
        } else {
            SpeedReaderOutcome::CHANGED
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }
        let theme = theme();
        let status = match self.state {
            SpeedReaderState::Empty => "Empty",
            SpeedReaderState::Paused => "Paused",
            SpeedReaderState::Playing => "Reading",
            SpeedReaderState::Complete => "Done",
        };
        frame.render_widget(
            Paragraph::new(Span::styled(
                status,
                Style::default()
                    .fg(if self.state == SpeedReaderState::Complete {
                        theme.success_fg()
                    } else {
                        theme.muted_fg()
                    })
                    .add_modifier(Modifier::BOLD),
            )),
            row(area, 0),
        );
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!("{} WPM", self.wpm),
                Style::default().fg(theme.key_fg()),
            ))
            .alignment(Alignment::Right),
            row(area, 0),
        );

        match self.tokens.get(self.position) {
            Some(token) => self.render_token(frame, area, token),
            None => frame.render_widget(
                Paragraph::new(Span::styled(
                    "No readable text",
                    Style::default().fg(theme.subtle_fg()),
                ))
                .alignment(Alignment::Center),
                row(area, 3),
            ),
        }

        if area.height > 5 {
            frame.render_widget(
                Gauge::default()
                    .ratio(self.progress())
                    .gauge_style(
                        Style::default()
                            .fg(theme.accent_fg())
                            .bg(theme.surface_bg()),
                    )
                    .label(format!(
                        "{} / {}",
                        self.position
                            .saturating_add((!self.tokens.is_empty()) as usize),
                        self.tokens.len()
                    )),
                row(area, 5),
            );
        }
        if area.height > 7 {
            frame.render_widget(
                Paragraph::new(self.help_line()).alignment(Alignment::Center),
                row(area, 7),
            );
        }
    }

    fn render_token(&self, frame: &mut Frame, area: Rect, token: &ReaderToken) {
        let word_area = Rect::new(
            area.x,
            area.y.saturating_add(2),
            area.width,
            3.min(area.height.saturating_sub(2)),
        );
        if word_area.is_empty() {
            return;
        }
        let graphemes = styled_graphemes(token);
        if graphemes.is_empty() {
            return;
        }
        let pivot = recognition_point(graphemes.len());
        let total_width = graphemes
            .iter()
            .map(|(text, _)| cell_width(text))
            .sum::<u16>();
        let available = word_area.width.saturating_sub(4);
        if total_width > available {
            let mut spans = Vec::new();
            if !token.prefix.is_empty() {
                spans.push(context_span(token.prefix.clone()));
                spans.push(Span::raw(" "));
            }
            spans.extend(styled_spans(&graphemes, token.heading, Some(pivot)));
            frame.render_widget(
                Paragraph::new(Line::from(spans))
                    .alignment(Alignment::Center)
                    .wrap(Wrap { trim: false }),
                word_area,
            );
            return;
        }

        let anchor = word_area.x.saturating_add(word_area.width / 2);
        let prefix_width = graphemes[..pivot]
            .iter()
            .map(|(text, _)| cell_width(text))
            .sum::<u16>();
        let pivot_width = cell_width(&graphemes[pivot].0).max(1);
        let suffix_width = total_width.saturating_sub(prefix_width + pivot_width);
        let y = word_area.y.saturating_add(1);
        if !token.prefix.is_empty() {
            let context_width = cell_width(&token.prefix);
            let word_start = anchor.saturating_sub(prefix_width);
            let context_right = word_start.saturating_sub(1);
            let context_x = context_right.saturating_sub(context_width).max(word_area.x);
            frame.render_widget(
                Paragraph::new(context_span(token.prefix.clone())).alignment(Alignment::Right),
                Rect::new(context_x, y, context_right.saturating_sub(context_x), 1),
            );
        }
        if prefix_width > 0 {
            frame.render_widget(
                Paragraph::new(Line::from(styled_spans(
                    &graphemes[..pivot],
                    token.heading,
                    None,
                ))),
                Rect::new(anchor.saturating_sub(prefix_width), y, prefix_width, 1),
            );
        }
        frame.render_widget(
            Paragraph::new(Line::from(styled_spans(
                &graphemes[pivot..=pivot],
                token.heading,
                Some(0),
            ))),
            Rect::new(anchor, y, pivot_width, 1),
        );
        if suffix_width > 0 {
            frame.render_widget(
                Paragraph::new(Line::from(styled_spans(
                    &graphemes[pivot + 1..],
                    token.heading,
                    None,
                ))),
                Rect::new(
                    anchor.saturating_add(pivot_width),
                    y,
                    suffix_width.min(word_area.right().saturating_sub(anchor + pivot_width)),
                    1,
                ),
            );
        }
    }

    fn help_line(&self) -> Line<'static> {
        let theme = theme();
        let toggle = first_label(&self.keys.toggle);
        let faster = first_label(&self.keys.faster);
        let slower = first_label(&self.keys.slower);
        let words = format!(
            "{}/{} words",
            first_label(&self.keys.previous_word),
            first_label(&self.keys.next_word)
        );
        Line::from(vec![
            Span::styled(toggle, Style::default().fg(theme.key_fg())),
            Span::styled(" play/pause  ", Style::default().fg(theme.subtle_fg())),
            Span::styled(
                format!("{slower}/{faster}"),
                Style::default().fg(theme.key_fg()),
            ),
            Span::styled(" speed  ", Style::default().fg(theme.subtle_fg())),
            Span::styled(words, Style::default().fg(theme.key_fg())),
            Span::styled("  Esc close", Style::default().fg(theme.subtle_fg())),
        ])
    }

    fn reparse(&mut self) {
        self.tokens = parse_source(&self.source, self.mode);
        self.restart_paused();
    }

    fn seek_previous_block(&mut self) {
        let Some(current) = self.tokens.get(self.position) else {
            return;
        };
        if current.block == 0 {
            self.restart_paused();
            return;
        }
        let target = current.block - 1;
        if let Some(position) = self.tokens.iter().position(|token| token.block == target) {
            self.set_position(position);
            self.elapsed = Duration::ZERO;
            self.state = SpeedReaderState::Paused;
        }
    }

    fn seek_next_block(&mut self) {
        let Some(current) = self.tokens.get(self.position) else {
            return;
        };
        if let Some(position) = self
            .tokens
            .iter()
            .position(|token| token.block > current.block)
        {
            self.set_position(position);
            self.elapsed = Duration::ZERO;
            self.state = SpeedReaderState::Paused;
        }
    }

    fn current_dwell(&self) -> Duration {
        let base = Duration::from_secs_f64(60.0 / f64::from(self.wpm));
        let Some(token) = self.tokens.get(self.position) else {
            return base;
        };
        base.mul_f64(if self.natural_pauses {
            dwell_factor(token)
        } else {
            1.0
        })
    }

    fn advance_after_dwell(&mut self) {
        if self.position + 1 >= self.tokens.len() {
            self.state = SpeedReaderState::Complete;
            return;
        }
        self.set_position(self.position + 1);
    }

    fn set_position(&mut self, position: usize) {
        self.position = position;
        self.pause_oversized_word();
    }

    fn pause_oversized_word(&mut self) {
        if self.state == SpeedReaderState::Playing
            && self.area.width > 0
            && self.current_word_width() > self.area.width.saturating_sub(4)
        {
            self.state = SpeedReaderState::Paused;
        }
    }

    fn current_word_width(&self) -> u16 {
        self.current_word().map(cell_width).unwrap_or_default()
    }
}

impl Animated for SpeedReader {
    fn tick(&mut self, dt: Duration, _settings: AnimationSettings) -> TickResult {
        if self.state != SpeedReaderState::Playing {
            return TickResult::IDLE;
        }
        self.elapsed += dt;
        let mut changed = false;
        while self.state == SpeedReaderState::Playing {
            let dwell = self.current_dwell();
            if self.elapsed < dwell {
                return TickResult {
                    changed,
                    next_tick: Some(dwell - self.elapsed),
                    ..TickResult::IDLE
                };
            }
            self.elapsed -= dwell;
            self.advance_after_dwell();
            changed = true;
        }
        self.elapsed = Duration::ZERO;
        TickResult {
            changed,
            layout: false,
            active: false,
            next_tick: None,
        }
    }
}

impl<M> TuiNode<M> for SpeedReader {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        LayoutSizeHint {
            source: HintSource::Measured,
            min: LayoutSize::new(28, 6),
            preferred: LayoutSize::new(PREFERRED_WIDTH, PREFERRED_HEIGHT),
            expand: AxisExpand {
                width: false,
                height: false,
            },
        }
        .normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.area = area;
        self.pause_oversized_word();
        ctx.register_focusable(FocusId::new(SPEED_READER_FOCUS), area, true);
        ctx.set_focus_control(FocusId::new(SPEED_READER_FOCUS), true);
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _ctx: &mut crate::RenderCtx<'_>) {
        Self::render(self, frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };
        let outcome = self.on_key(*key);
        if outcome.changed {
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
}

fn parse_source(source: &str, mode: SpeedReaderInputMode) -> Vec<ReaderToken> {
    match mode {
        SpeedReaderInputMode::Plain => parse_plain(source),
        SpeedReaderInputMode::Markdown => markdown::parse_markdown(source),
    }
}

fn parse_plain(source: &str) -> Vec<ReaderToken> {
    let mut tokens: Vec<ReaderToken> = Vec::new();
    let mut block = 0;
    let mut block_has_words = false;
    for line in source.lines() {
        if line.trim().is_empty() {
            if block_has_words {
                if let Some(token) = tokens.last_mut() {
                    token.boundary_after = true;
                }
                block += 1;
                block_has_words = false;
            }
            continue;
        }
        for word in line.split_whitespace() {
            tokens.push(ReaderToken {
                text: word.to_string(),
                fragments: vec![(word.to_string(), InlineMarks::default())],
                prefix: String::new(),
                block,
                heading: false,
                boundary_after: false,
            });
            block_has_words = true;
        }
    }
    if let Some(token) = tokens.last_mut() {
        token.boundary_after = true;
    }
    tokens
}

fn dwell_factor(token: &ReaderToken) -> f64 {
    let punctuation = token
        .text
        .trim_end_matches(['"', '\'', '”', '’', ')', ']', '}']);
    let punctuation_factor: f64 =
        if punctuation.ends_with(['.', '?', '!']) || punctuation.ends_with("…") {
            2.0
        } else if punctuation.ends_with([',', ':', ';', '—', '–']) {
            1.5
        } else {
            1.0
        };
    let boundary_factor: f64 = if token.boundary_after {
        if token.heading || !token.prefix.is_empty() {
            2.0
        } else {
            2.25
        }
    } else {
        1.0
    };
    punctuation_factor.max(boundary_factor)
}

fn styled_graphemes(token: &ReaderToken) -> Vec<(String, InlineMarks)> {
    token
        .fragments
        .iter()
        .flat_map(|(text, marks)| {
            text.graphemes(true)
                .map(|grapheme| (grapheme.to_string(), *marks))
        })
        .collect()
}

fn styled_spans(
    graphemes: &[(String, InlineMarks)],
    heading: bool,
    pivot: Option<usize>,
) -> Vec<Span<'static>> {
    graphemes
        .iter()
        .enumerate()
        .map(|(index, (text, marks))| {
            let mut modifiers = Modifier::empty();
            if heading || marks.strong {
                modifiers |= Modifier::BOLD;
            }
            if marks.emphasis {
                modifiers |= Modifier::ITALIC;
            }
            if marks.link {
                modifiers |= Modifier::UNDERLINED;
            }
            let theme = theme();
            let mut foreground = theme.text_fg();
            if pivot == Some(index) {
                foreground = theme.accent_fg();
                modifiers |= Modifier::BOLD | Modifier::UNDERLINED;
            }
            let style = Style::default().fg(foreground).add_modifier(modifiers);
            Span::styled(text.clone(), style)
        })
        .collect()
}

fn context_span(text: String) -> Span<'static> {
    Span::styled(
        text,
        Style::default()
            .fg(theme().subtle_fg())
            .add_modifier(Modifier::BOLD),
    )
}

fn recognition_point(graphemes: usize) -> usize {
    match graphemes {
        0 | 1 => 0,
        2..=5 => 1,
        6..=9 => 2,
        10..=13 => 3,
        _ => 4,
    }
}

fn cell_width(text: &str) -> u16 {
    line_width(&Line::from(text)).min(u16::MAX as usize) as u16
}

fn row(area: Rect, offset: u16) -> Rect {
    Rect::new(
        area.x,
        area.y.saturating_add(offset),
        area.width,
        (area.height > offset) as u16,
    )
}

fn matches_any(bindings: &[KeySpec], key: KeyEvent) -> bool {
    bindings.iter().any(|binding| binding.matches(key))
}

fn first_label(bindings: &[KeySpec]) -> String {
    bindings
        .first()
        .map(|binding| binding.label())
        .unwrap_or_else(|| "—".to_string())
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;

    #[test]
    fn starts_paused_and_completes_after_final_dwell() {
        let mut reader = SpeedReader::new("one two").wpm(300);
        reader.play();

        Animated::tick(
            &mut reader,
            Duration::from_millis(200),
            AnimationSettings::default(),
        );
        assert_eq!(reader.current_word(), Some("two"));
        assert_eq!(reader.state(), SpeedReaderState::Playing);

        Animated::tick(
            &mut reader,
            Duration::from_millis(450),
            AnimationSettings::default(),
        );
        assert_eq!(reader.current_word(), Some("two"));
        assert_eq!(reader.state(), SpeedReaderState::Complete);
    }

    #[test]
    fn pause_preserves_remaining_dwell_and_speed_change_preserves_fraction() {
        let mut reader = SpeedReader::new("one two").wpm(300);
        reader.play();
        Animated::tick(
            &mut reader,
            Duration::from_millis(100),
            AnimationSettings::default(),
        );
        reader.pause();
        reader.set_wpm(600);
        reader.play();

        Animated::tick(
            &mut reader,
            Duration::from_millis(49),
            AnimationSettings::default(),
        );
        assert_eq!(reader.current_word(), Some("one"));
        Animated::tick(
            &mut reader,
            Duration::from_millis(1),
            AnimationSettings::default(),
        );
        assert_eq!(reader.current_word(), Some("two"));
    }

    #[test]
    fn delayed_tick_consumes_all_elapsed_dwells() {
        let mut reader = SpeedReader::new("one two three")
            .wpm(300)
            .natural_pauses(false);
        reader.play();

        let tick = Animated::tick(
            &mut reader,
            Duration::from_millis(450),
            AnimationSettings::default(),
        );

        assert_eq!(reader.current_word(), Some("three"));
        assert_eq!(reader.state(), SpeedReaderState::Playing);
        assert_eq!(tick.next_tick, Some(Duration::from_millis(150)));
    }

    #[test]
    fn handled_boundary_key_does_not_report_a_change() {
        let mut reader = SpeedReader::new("one").wpm(MAX_WPM);

        let outcome = reader.on_key(Key::Char('+'));

        assert!(outcome.handled);
        assert!(!outcome.changed);
    }

    #[test]
    fn layout_pauses_playback_on_an_oversized_word() {
        let mut reader = SpeedReader::new("extraordinary");
        reader.play();
        let mut layout = LayoutCtx::new();

        <SpeedReader as TuiNode<()>>::layout(&mut reader, Rect::new(0, 0, 8, 8), &mut layout);

        assert_eq!(reader.state(), SpeedReaderState::Paused);
    }

    #[test]
    fn markdown_navigation_moves_by_words_and_blocks() {
        let mut reader = SpeedReader::markdown("# First block\n\n- Second block");
        reader.on_key(Key::Down);
        assert_eq!(reader.current_word(), Some("Second"));
        reader.on_key(Key::Left);
        assert_eq!(reader.current_word(), Some("block"));
        reader.on_key(Key::Up);
        assert_eq!(reader.current_word(), Some("First"));
    }

    #[test]
    fn natural_pauses_extend_sentence_and_block_boundaries() {
        let reader = SpeedReader::new("Wait. Next").wpm(300);
        assert_eq!(reader.current_dwell(), Duration::from_millis(400));

        let mut reader = SpeedReader::new("A paragraph.\n\nNext").wpm(300);
        reader.next();
        assert_eq!(reader.current_dwell(), Duration::from_millis(450));

        let reader = SpeedReader::markdown("# Heading\n\nParagraph").wpm(300);
        assert_eq!(reader.current_dwell(), Duration::from_millis(400));
    }

    #[test]
    fn renders_context_word_progress_and_help() {
        let reader = SpeedReader::markdown("## **Fast** reading");
        let mut terminal = Terminal::new(TestBackend::new(64, 8)).expect("terminal should build");

        terminal
            .draw(|frame| reader.render(frame, frame.area()))
            .expect("reader should render");

        let content = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        let word_row = (0..64)
            .map(|x| terminal.backend().buffer().cell((x, 3)).unwrap().symbol())
            .collect::<String>();
        assert!(content.contains("Paused"));
        assert!(content.contains("##"));
        assert!(content.contains("Fast"));
        assert!(word_row.contains("## Fast"));
        let context_x = word_row.find("## Fast").expect("context should render") as u16;
        assert_eq!(
            terminal.backend().buffer().cell((context_x, 3)).unwrap().fg,
            theme().subtle_fg()
        );
        assert!(content.contains("300 WPM"));
        assert!(content.contains("Space"));
        assert!(!content.contains("┬"));
    }

    #[test]
    fn dialog_uses_configured_title() {
        let host = SpeedReader::new("Plain text")
            .title("Plain text example")
            .dialog::<()>(|_| ());

        assert_eq!(host.child().title_text(), "Plain text example");
    }
}
