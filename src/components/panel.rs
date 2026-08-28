use std::{fmt, rc::Rc, time::Duration};

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::event::{HotkeyEvent, Key, KeyEvent, MouseButton, MouseEventKind, TuiEvent};
use crate::{
    Animated, AnimationSettings, AnimationSpec, BorderKind, ColorTween, ScrollAxes, ScrollBehavior,
    ScrollDelta, ScrollGeometry, ScrollLayout, ScrollOffset, ScrollOutcome, ScrollSize,
    ScrollState, TickResult, border_chars, border_set, hotkey_badge_width, hotkey_edge_spans,
    hotkey_sequence_to_event, hotkey_underline_style, line_width, paragraph_scroll, preset, theme,
};

const PANEL_FOCUS: &str = "panel";
use crate::{
    ChildKey, EventCtx, EventOutcome, EventRoute, FocusCtx, FocusId, FocusTarget, HotkeyMatch,
    HotkeySequenceMatcher, LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint, LifecycleCtx,
    TreePath, TuiNode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelTitlePosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum PanelTone {
    #[default]
    Normal,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PanelTitle {
    text: String,
    line: Option<Line<'static>>,
}

struct PanelActionHotkey<M> {
    sequence: String,
    on_trigger: Rc<dyn Fn() -> M>,
}

impl<M> Clone for PanelActionHotkey<M> {
    fn clone(&self) -> Self {
        Self {
            sequence: self.sequence.clone(),
            on_trigger: Rc::clone(&self.on_trigger),
        }
    }
}

impl<M> fmt::Debug for PanelActionHotkey<M> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PanelActionHotkey")
            .field("sequence", &self.sequence)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
pub struct Panel<M = ()> {
    top_left: Option<PanelTitle>,
    top_right: Option<PanelTitle>,
    bottom_left: Option<PanelTitle>,
    bottom_right: Option<PanelTitle>,
    hotkey: Option<String>,
    action_hotkeys: Vec<PanelActionHotkey<M>>,
    hotkey_matcher: HotkeySequenceMatcher,
    border: Option<BorderKind>,
    one_row: bool,
    tone: PanelTone,
    content: Vec<String>,
    scroll: Option<ScrollState>,
    focused: bool,
    border_color: ColorTween,
    title_color: ColorTween,
    area: Rect,
    layout_path: TreePath,
    pending_hotkey_prefix: Option<String>,
}

pub struct PanelHost<C, M = ()> {
    panel: Panel<M>,
    child: C,
    child_area: Rect,
}

impl<M> Default for Panel<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl Panel<()> {
    pub fn inner_area(area: Rect) -> Rect {
        panel_inner_area(area)
    }
}

impl<M> Panel<M> {
    pub fn new() -> Self {
        let theme = theme();
        Self {
            top_left: None,
            top_right: None,
            bottom_left: None,
            bottom_right: None,
            hotkey: None,
            action_hotkeys: Vec::new(),
            hotkey_matcher: HotkeySequenceMatcher::default(),
            border: None,
            one_row: false,
            tone: PanelTone::Normal,
            content: Vec::new(),
            scroll: None,
            focused: false,
            border_color: ColorTween::idle(theme.border_fg()),
            title_color: ColorTween::idle(theme.muted_fg()),
            area: Rect::default(),
            layout_path: TreePath::new(),
            pending_hotkey_prefix: None,
        }
    }

    pub fn top_left(mut self, title: impl Into<String>) -> Self {
        self.top_left = Some(PanelTitle::standard(title));
        self
    }

    pub fn set_top_left(&mut self, title: impl Into<String>) {
        self.top_left = Some(PanelTitle::standard(title));
    }

    pub fn top_right(mut self, title: impl Into<String>) -> Self {
        self.top_right = Some(PanelTitle::standard(title));
        self
    }

    pub fn top_right_line(mut self, title: Line<'static>) -> Self {
        self.top_right = Some(PanelTitle::styled(title));
        self
    }

    pub fn set_top_right(&mut self, title: impl Into<String>) {
        self.top_right = Some(PanelTitle::standard(title));
    }

    pub fn bottom_left(mut self, title: impl Into<String>) -> Self {
        self.bottom_left = Some(PanelTitle::standard(title));
        self
    }

    pub fn set_bottom_left(&mut self, title: impl Into<String>) {
        self.bottom_left = Some(PanelTitle::standard(title));
    }

    pub fn bottom_right(mut self, title: impl Into<String>) -> Self {
        self.bottom_right = Some(PanelTitle::standard(title));
        self
    }

    pub fn set_bottom_right(&mut self, title: impl Into<String>) {
        self.bottom_right = Some(PanelTitle::standard(title));
    }

    pub fn title(mut self, position: PanelTitlePosition, title: impl Into<String>) -> Self {
        self.set_title(position, title);
        self
    }

    pub fn set_title(&mut self, position: PanelTitlePosition, title: impl Into<String>) {
        *self.title_slot_mut(position) = Some(PanelTitle::standard(title));
    }

    pub fn clear_title(&mut self, position: PanelTitlePosition) {
        *self.title_slot_mut(position) = None;
    }

    pub fn title_text(&self, position: PanelTitlePosition) -> Option<&str> {
        self.title_slot(position).map(|title| title.text.as_str())
    }

    pub fn hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.set_hotkey(hotkey);
        self
    }

    pub fn set_hotkey(&mut self, hotkey: impl Into<String>) {
        let hotkey = hotkey.into();
        self.hotkey = Some(hotkey.clone());
        self.rebuild_hotkey_matcher();
    }

    pub fn clear_hotkey(&mut self) {
        self.hotkey = None;
        self.rebuild_hotkey_matcher();
    }

    pub fn action_hotkey(
        mut self,
        sequence: impl Into<String>,
        on_trigger: impl Fn() -> M + 'static,
    ) -> Self {
        self.action_hotkeys.push(PanelActionHotkey {
            sequence: sequence.into(),
            on_trigger: Rc::new(on_trigger),
        });
        self.rebuild_hotkey_matcher();
        self
    }

    pub(crate) fn set_hotkey_badge(&mut self, hotkey: Option<String>) {
        self.hotkey = hotkey;
        self.hotkey_matcher = HotkeySequenceMatcher::default();
    }

    pub(crate) fn set_pending_hotkey_prefix(&mut self, prefix: Option<String>) {
        self.pending_hotkey_prefix = prefix;
    }

    pub fn border(mut self, border: BorderKind) -> Self {
        self.set_border(border);
        self
    }

    pub fn set_border(&mut self, border: BorderKind) {
        self.border = Some(border);
    }

    pub fn clear_border(&mut self) {
        self.border = None;
    }

    pub fn one_row(mut self, one_row: bool) -> Self {
        self.one_row = one_row;
        self
    }

    pub fn set_one_row(&mut self, one_row: bool) {
        self.one_row = one_row;
    }

    pub fn tone(mut self, tone: PanelTone) -> Self {
        self.set_tone(tone);
        self
    }

    pub fn set_tone(&mut self, tone: PanelTone) {
        self.tone = tone;
    }

    pub fn current_tone(&self) -> PanelTone {
        self.tone
    }

    pub fn content(mut self, lines: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.content = lines.into_iter().map(Into::into).collect();
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        let theme = theme();
        self.border_color.snap_to(if focused {
            theme.accent_fg()
        } else {
            theme.border_fg()
        });
        self.title_color.snap_to(if focused {
            theme.accent_fg()
        } else {
            theme.muted_fg()
        });
        self
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn set_focused(&mut self, focused: bool, settings: AnimationSettings) {
        if self.focused == focused {
            return;
        }
        self.focused = focused;
        self.start_focus_color_transition(focused, settings);
    }

    pub fn host<C>(self, child: C) -> PanelHost<C, M> {
        PanelHost {
            panel: self,
            child,
            child_area: Rect::default(),
        }
    }

    pub fn scrollable(mut self, axes: ScrollAxes) -> Self {
        self.scroll = Some(match self.scroll.take() {
            Some(scroll) => scroll.with_axes(axes),
            None => ScrollState::from_preset(axes, preset().scroll()),
        });
        self
    }

    pub fn scroll_behavior(mut self, behavior: ScrollBehavior) -> Self {
        if let Some(scroll) = self.scroll.take() {
            self.scroll = Some(scroll.behavior(behavior));
        } else {
            self.scroll = Some(
                ScrollState::from_preset(ScrollAxes::Vertical, preset().scroll())
                    .behavior(behavior),
            );
        }
        self
    }

    pub fn content_size(&self) -> ScrollSize {
        let width = self
            .content
            .iter()
            .map(|line| line_width(&Line::from(line.as_str())))
            .max()
            .unwrap_or(0);
        ScrollSize::new(width, self.content.len())
    }

    pub fn scroll_geometry(&self, area: Rect) -> ScrollGeometry {
        let inner = self.content_area(area);
        let content = self.content_size();
        if let Some(scroll) = &self.scroll {
            scroll.geometry(inner, content)
        } else {
            let layout = ScrollLayout {
                outer: inner,
                viewport: inner,
                vertical_bar: None,
                horizontal_bar: None,
                corner: None,
            };
            ScrollGeometry {
                layout,
                viewport: ScrollSize::from_area(inner),
                content,
            }
        }
    }

    pub fn on_key(
        &mut self,
        key: impl Into<KeyEvent>,
        area: Rect,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        let key = key.into();
        let geometry = self.scroll_geometry(area);
        let Some(scroll) = &mut self.scroll else {
            return ScrollOutcome::idle();
        };
        scroll.on_key(key, geometry.viewport, geometry.content, settings)
    }

    pub fn scroll_by(
        &mut self,
        delta: ScrollDelta,
        area: Rect,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        let geometry = self.scroll_geometry(area);
        let Some(scroll) = &mut self.scroll else {
            return ScrollOutcome::idle();
        };
        scroll.scroll_by(delta, geometry.viewport, geometry.content, settings)
    }

    pub fn scroll_to(
        &mut self,
        offset: ScrollOffset,
        area: Rect,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        let geometry = self.scroll_geometry(area);
        let Some(scroll) = &mut self.scroll else {
            return ScrollOutcome::idle();
        };
        scroll.scroll_to(offset, geometry.viewport, geometry.content, settings)
    }

    pub fn clamp_scroll(&mut self, area: Rect, settings: AnimationSettings) -> ScrollOutcome {
        let geometry = self.scroll_geometry(area);
        let Some(scroll) = &mut self.scroll else {
            return ScrollOutcome::idle();
        };
        scroll.clamp_to(geometry.viewport, geometry.content, settings)
    }

    fn content_area(&self, area: Rect) -> Rect {
        if self.one_row {
            Rect::new(
                area.x,
                area.y.saturating_add(1),
                area.width,
                area.height.saturating_sub(1),
            )
        } else {
            panel_inner_area(area)
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let border = self.border.unwrap_or_else(|| preset().border());
        let border_style = Style::default().fg(self.visible_border_color());

        let block = Block::default()
            .borders(if self.one_row {
                Borders::TOP
            } else {
                Borders::ALL
            })
            .border_set(border_set(border))
            .border_style(border_style);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        self.render_panel_title(frame, area, border, PanelTitlePosition::TopLeft);
        self.render_panel_title(frame, area, border, PanelTitlePosition::TopRight);
        if !self.one_row {
            self.render_panel_title(frame, area, border, PanelTitlePosition::BottomLeft);
            self.render_panel_title(frame, area, border, PanelTitlePosition::BottomRight);
            self.render_hotkey(frame, area, border);
        }

        if !inner.is_empty() {
            let lines = self
                .content
                .iter()
                .map(|line| Line::from(line.clone()))
                .collect::<Vec<_>>();
            if let Some(scroll) = &self.scroll {
                let geometry = scroll.geometry(inner, self.content_size());
                let offset = scroll.offset();
                let lines = if offset.x > u16::MAX as usize || offset.y > u16::MAX as usize {
                    visible_lines(&self.content, offset, geometry.viewport)
                } else {
                    lines
                };
                let paragraph = Paragraph::new(lines).alignment(Alignment::Left).scroll(
                    if offset.x > u16::MAX as usize || offset.y > u16::MAX as usize {
                        (0, 0)
                    } else {
                        paragraph_scroll(offset)
                    },
                );
                frame.render_widget(paragraph, geometry.layout.viewport);
                scroll.render_scrollbars(frame, geometry.layout, geometry.content, self.focused);
            } else {
                frame.render_widget(Paragraph::new(lines).alignment(Alignment::Left), inner);
            }
        }
    }

    fn render_panel_title(
        &self,
        frame: &mut Frame,
        area: Rect,
        border: BorderKind,
        position: PanelTitlePosition,
    ) {
        let Some(title) = self.title_slot(position) else {
            return;
        };
        match position {
            PanelTitlePosition::TopLeft | PanelTitlePosition::TopRight => {
                self.render_title(frame, area, title, position)
            }
            PanelTitlePosition::BottomLeft | PanelTitlePosition::BottomRight => {
                self.render_inset_title(frame, area, border, title, position)
            }
        }
    }

    fn render_hotkey(&self, frame: &mut Frame, area: Rect, border: BorderKind) {
        let Some(hotkey) = self.display_hotkey() else {
            return;
        };
        if area.width <= 4 {
            return;
        }

        let border_style = Style::default().fg(self.visible_border_color());
        let title_style = Style::default().fg(self.visible_title_color());
        let width = hotkey_badge_width(&hotkey).min(u16::MAX as usize) as u16;
        let x = area.x + area.width.saturating_sub(width);
        let y = title_y(area, PanelTitlePosition::BottomRight);
        let line = Line::from(hotkey_edge_spans(
            &hotkey,
            self.pending_hotkey_prefix.as_deref(),
            border,
            border_style,
            title_style,
            hotkey_underline_style(title_style),
        ));

        frame.render_widget(Paragraph::new(line), Rect::new(x, y, width, 1));
    }

    fn render_title(
        &self,
        frame: &mut Frame,
        area: Rect,
        title: &PanelTitle,
        position: PanelTitlePosition,
    ) {
        if area.width <= 4 {
            return;
        }

        let max_width = area.width.saturating_sub(4) as usize;
        let style = Style::default()
            .fg(self.visible_title_color())
            .add_modifier(Modifier::BOLD);
        let line = title.line(max_width, style);
        let width = line_width(&line).min(u16::MAX as usize) as u16;
        if width == 0 {
            return;
        }

        let x = match title_alignment(position) {
            Alignment::Left => area.x.saturating_add(2),
            Alignment::Center => area.x + area.width.saturating_sub(width) / 2,
            Alignment::Right => area.x + area.width.saturating_sub(width).saturating_sub(2),
        };
        let y = title_y(area, position);
        frame.render_widget(Paragraph::new(line), Rect::new(x, y, width, 1));
    }

    fn render_inset_title(
        &self,
        frame: &mut Frame,
        area: Rect,
        border: BorderKind,
        title: &PanelTitle,
        position: PanelTitlePosition,
    ) {
        if area.width <= 4 {
            return;
        }

        let chars = border_chars(border);
        let border_style = Style::default().fg(self.visible_border_color());
        let title_style = Style::default().fg(self.visible_title_color());
        let reserved_right = self.reserved_bottom_right_width(position);
        if area.width <= 4 + reserved_right {
            return;
        }

        let title = bounded_title(
            &title.text,
            area.width.saturating_sub(5 + reserved_right) as usize,
        );
        let title_width = line_width(&Line::from(title.as_str())).min(area.width as usize);
        if title_width == 0 {
            return;
        }

        let line = Line::from(vec![
            Span::styled(chars.right_join, border_style),
            Span::styled(title, title_style),
            Span::styled(chars.left_join, border_style),
        ]);
        let width = (title_width + 2).min(u16::MAX as usize) as u16;
        let x = match title_alignment(position) {
            Alignment::Left | Alignment::Center => area.x.saturating_add(1),
            Alignment::Right => area.x.saturating_add(
                area.width
                    .saturating_sub(width)
                    .saturating_sub(1 + reserved_right),
            ),
        };
        let y = title_y(area, position);

        frame.render_widget(Paragraph::new(line), Rect::new(x, y, width, 1));
    }
}

impl PanelTitle {
    fn standard(title: impl Into<String>) -> Self {
        Self {
            text: title.into(),
            line: None,
        }
    }

    fn styled(line: Line<'static>) -> Self {
        let text = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        Self {
            text,
            line: Some(line),
        }
    }

    fn line(&self, max_width: usize, style: Style) -> Line<'static> {
        if let Some(line) = &self.line
            && line_width(line).saturating_add(2) <= max_width
        {
            let mut spans = Vec::with_capacity(line.spans.len() + 2);
            spans.push(Span::raw(" "));
            spans.extend(line.spans.iter().cloned());
            spans.push(Span::raw(" "));
            return Line::from(spans).style(style);
        }
        Line::from(Span::styled(bounded_title(&self.text, max_width), style))
    }
}

impl<M> Panel<M> {
    fn visible_border_color(&self) -> ratatui::style::Color {
        if self.tone == PanelTone::Error {
            return theme().error_fg();
        }
        if self.border_color.is_active() {
            return self.border_color.value();
        }

        let theme = theme();
        if self.focused {
            theme.accent_fg()
        } else {
            theme.border_fg()
        }
    }

    fn visible_title_color(&self) -> ratatui::style::Color {
        if self.tone == PanelTone::Error {
            return theme().error_fg();
        }
        if self.title_color.is_active() {
            return self.title_color.value();
        }

        let theme = theme();
        if self.focused {
            theme.accent_fg()
        } else {
            theme.muted_fg()
        }
    }

    fn title_slot(&self, position: PanelTitlePosition) -> Option<&PanelTitle> {
        match position {
            PanelTitlePosition::TopLeft => self.top_left.as_ref(),
            PanelTitlePosition::TopRight => self.top_right.as_ref(),
            PanelTitlePosition::BottomLeft => self.bottom_left.as_ref(),
            PanelTitlePosition::BottomRight => self.bottom_right.as_ref(),
        }
    }

    fn title_slot_mut(&mut self, position: PanelTitlePosition) -> &mut Option<PanelTitle> {
        match position {
            PanelTitlePosition::TopLeft => &mut self.top_left,
            PanelTitlePosition::TopRight => &mut self.top_right,
            PanelTitlePosition::BottomLeft => &mut self.bottom_left,
            PanelTitlePosition::BottomRight => &mut self.bottom_right,
        }
    }

    fn reserved_bottom_right_width(&self, position: PanelTitlePosition) -> u16 {
        if position != PanelTitlePosition::BottomRight {
            return 0;
        }
        self.display_hotkey()
            .map(|hotkey| hotkey_badge_width(&hotkey).min(u16::MAX as usize) as u16)
            .unwrap_or(0)
    }

    fn display_hotkey(&self) -> Option<String> {
        let hotkeys = self
            .hotkey
            .iter()
            .cloned()
            .chain(
                self.action_hotkeys
                    .iter()
                    .map(|action| action.sequence.clone()),
            )
            .collect::<Vec<_>>();
        (!hotkeys.is_empty()).then(|| hotkeys.join("·"))
    }
}

fn panel_inner_area(area: Rect) -> Rect {
    Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    )
}

fn title_alignment(position: PanelTitlePosition) -> Alignment {
    match position {
        PanelTitlePosition::TopLeft | PanelTitlePosition::BottomLeft => Alignment::Left,
        PanelTitlePosition::TopRight | PanelTitlePosition::BottomRight => Alignment::Right,
    }
}

fn title_y(area: Rect, position: PanelTitlePosition) -> u16 {
    match position {
        PanelTitlePosition::TopLeft | PanelTitlePosition::TopRight => area.y,
        PanelTitlePosition::BottomLeft | PanelTitlePosition::BottomRight => {
            area.y + area.height.saturating_sub(1)
        }
    }
}

impl<M> Animated for Panel<M> {
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        let hotkey_tick = if self.hotkey_matcher.tick(dt) {
            TickResult::CHANGED
        } else {
            TickResult::IDLE
        };
        let scroll = self
            .scroll
            .as_mut()
            .map(|scroll| scroll.tick(dt, settings))
            .unwrap_or(TickResult::IDLE);

        scroll
            .merge(self.border_color.tick(dt, settings))
            .merge(self.title_color.tick(dt, settings))
            .merge(hotkey_tick)
    }
}

impl<M> TuiNode<M> for Panel<M> {
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        let width_changed = self.area.width != 0 && self.area.width != area.width;
        self.area = area;
        self.layout_path = ctx.current_path();
        ctx.register_hit_region(crate::HitRegion::new(self.layout_path.clone(), area));
        if width_changed && let Some(scroll) = &mut self.scroll {
            scroll.snap_horizontal_to_start();
        }
        let hotkeys = self.hotkey_sequences();
        if !hotkeys.is_empty() {
            ctx.register_focusable_with_hotkey_sequences(
                FocusId::new(PANEL_FOCUS),
                area,
                true,
                hotkeys,
            );
        } else {
            ctx.register_focusable(FocusId::new(PANEL_FOCUS), area, true);
        }
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _ctx: &mut crate::RenderCtx<'_>) {
        Self::render(self, frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        if let TuiEvent::Hotkey(hotkey) = event {
            return self.on_hotkey_event(hotkey, ctx);
        }
        if let TuiEvent::Key(key) = event
            && let Some(outcome) = self.handle_hotkey_key(*key, ctx)
        {
            return outcome;
        }
        if let TuiEvent::Mouse(mouse) = event
            && (mouse.column < self.area.x
                || mouse.column >= self.area.x.saturating_add(self.area.width)
                || mouse.row < self.area.y
                || mouse.row >= self.area.y.saturating_add(self.area.height))
        {
            return EventOutcome::Ignored;
        }
        if matches!(
            event,
            TuiEvent::Mouse(mouse) if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
        ) {
            ctx.focus(crate::FocusRequest::TargetAt {
                path: self.layout_path.clone(),
                id: FocusId::new(PANEL_FOCUS),
            });
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        let outcome = match event {
            TuiEvent::Key(key) => self.on_key(*key, self.area, ctx.animation()),
            TuiEvent::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => {
                    self.scroll_by(ScrollDelta::new(0, -1), self.area, ctx.animation())
                }
                MouseEventKind::ScrollDown => {
                    self.scroll_by(ScrollDelta::new(0, 1), self.area, ctx.animation())
                }
                MouseEventKind::ScrollLeft => {
                    self.scroll_by(ScrollDelta::new(-1, 0), self.area, ctx.animation())
                }
                MouseEventKind::ScrollRight => {
                    self.scroll_by(ScrollDelta::new(1, 0), self.area, ctx.animation())
                }
                _ => return EventOutcome::Ignored,
            },
            _ => return EventOutcome::Ignored,
        };
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
        self.set_focused(focused, ctx.animation());
        ctx.request_redraw();
    }
}

impl<C, M> PanelHost<C, M> {
    pub fn panel(&self) -> &Panel<M> {
        &self.panel
    }

    pub fn panel_mut(&mut self) -> &mut Panel<M> {
        &mut self.panel
    }

    pub fn child(&self) -> &C {
        &self.child
    }

    pub fn child_mut(&mut self) -> &mut C {
        &mut self.child
    }

    pub fn child_area(&self) -> Rect {
        self.child_area
    }
}

impl<C, M> TuiNode<M> for PanelHost<C, M>
where
    C: TuiNode<M>,
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let child = self.child.measure(proposal);
        let horizontal_border_pad = if self.panel.one_row { 0 } else { 2 };
        let vertical_border_pad = if self.panel.one_row { 1 } else { 2 };
        LayoutSizeHint::content(
            child.preferred.width.saturating_add(horizontal_border_pad),
            child.preferred.height.saturating_add(vertical_border_pad),
        )
        .normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        let width_changed = self.panel.area.width != 0 && self.panel.area.width != area.width;
        self.panel.area = area;
        self.panel.layout_path = ctx.current_path();
        ctx.register_hit_region(crate::HitRegion::new(self.panel.layout_path.clone(), area));
        if width_changed && let Some(scroll) = &mut self.panel.scroll {
            scroll.snap_horizontal_to_start();
        }
        let inner = self.panel.content_area(area);
        self.child_area = inner;
        let hotkeys = self.panel.hotkey_sequences();
        let fallback_inserted = if hotkeys.is_empty() {
            ctx.with_focus_fallback_status(FocusId::new(PANEL_FOCUS), area, |ctx| {
                ctx.push_slot(ChildKey::body(), inner, |ctx| {
                    self.child.layout(inner, ctx);
                });
            })
            .1
        } else {
            ctx.with_focus_fallback_hotkey_sequences_status(
                FocusId::new(PANEL_FOCUS),
                area,
                hotkeys,
                |ctx| {
                    ctx.push_slot(ChildKey::body(), inner, |ctx| {
                        self.child.layout(inner, ctx);
                    });
                },
            )
            .1
        };
        if !fallback_inserted {
            ctx.register_focusable(FocusId::new(PANEL_FOCUS), area, true);
        }
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut crate::RenderCtx<'a>) {
        self.panel.render(frame, area);
        self.child.render(frame, self.child_area, ctx);
        crate::separator::patch_border_joins(
            frame,
            area,
            self.child_area,
            self.panel.border.unwrap_or_else(|| preset().border()),
            Style::default().fg(self.panel.visible_border_color()),
        );
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        self.panel.event(event, ctx)
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        if route.path.is_empty() {
            return self.event(event, ctx);
        }

        if let TuiEvent::Key(key) = event
            && let Some(outcome) = self.panel.handle_hotkey_key(*key, ctx)
        {
            return outcome;
        }

        let body = ChildKey::body();
        let child = route
            .path
            .without_first_if(&body)
            .map(EventRoute::new)
            .map(|route| self.child.dispatch_event(&route, event, ctx))
            .unwrap_or(EventOutcome::Ignored);
        child.bubble(ctx, |ctx| self.event(event, ctx))
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(&mut self.panel, dt, settings).merge(self.child.tick(dt, settings))
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        if target.path.is_empty() && target.id.as_str() == PANEL_FOCUS {
            self.panel.set_focused(focused, ctx.animation());
            ctx.request_redraw();
            return;
        }

        let body = ChildKey::body();
        if let Some(child_target) = target.for_child(&body) {
            self.panel.set_focused(focused, ctx.animation());
            self.child.dispatch_focus(&child_target, focused, ctx);
            ctx.request_redraw();
        }
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.child.init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.child.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.child.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.child.destroy(ctx);
    }
}

impl<M> Panel<M> {
    fn start_focus_color_transition(&mut self, focused: bool, settings: AnimationSettings) {
        let theme = theme();
        self.border_color.start(
            if focused {
                theme.accent_fg()
            } else {
                theme.border_fg()
            },
            settings,
            focus_color_animation(),
        );
        self.title_color.start(
            if focused {
                theme.accent_fg()
            } else {
                theme.muted_fg()
            },
            settings,
            focus_color_animation(),
        );
    }

    fn hotkey_event(&self) -> Option<KeyEvent> {
        self.hotkey.as_deref().and_then(hotkey_sequence_to_event)
    }

    fn hotkey_matches(&self, key: KeyEvent) -> bool {
        self.hotkey_event()
            .is_some_and(|hotkey| panel_hotkey_matches(hotkey, key))
    }

    fn handle_hotkey_key(&mut self, key: KeyEvent, ctx: &mut EventCtx<M>) -> Option<EventOutcome> {
        match self.hotkey_matcher.on_key(key) {
            HotkeyMatch::Matched(index) => {
                if let Some(action) = self.action_for_match_index(index) {
                    ctx.emit((action.on_trigger)());
                    ctx.request_redraw();
                }
                ctx.stop_propagation();
                return Some(EventOutcome::Handled);
            }
            HotkeyMatch::Pending | HotkeyMatch::Canceled => {
                ctx.stop_propagation();
                return Some(EventOutcome::Handled);
            }
            HotkeyMatch::Ignored => {}
        }

        if self.hotkey_matches(key) {
            ctx.stop_propagation();
            Some(EventOutcome::Handled)
        } else {
            None
        }
    }

    fn on_hotkey_event(&mut self, hotkey: &HotkeyEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        match hotkey {
            HotkeyEvent::Pending(prefix) => {
                if self.hotkey_has_prefix(prefix) {
                    self.pending_hotkey_prefix = Some(prefix.clone());
                    ctx.request_redraw();
                }
                EventOutcome::Ignored
            }
            HotkeyEvent::Canceled => {
                if self.pending_hotkey_prefix.take().is_some() {
                    ctx.request_redraw();
                }
                EventOutcome::Ignored
            }
            HotkeyEvent::Commit(sequence) => {
                self.pending_hotkey_prefix = None;
                if let Some(action) = self.action_for_sequence(sequence) {
                    ctx.emit((action.on_trigger)());
                    ctx.request_redraw();
                    ctx.stop_propagation();
                    return EventOutcome::Handled;
                }
                if self.hotkey_matches_sequence(sequence) {
                    ctx.request_redraw();
                    ctx.stop_propagation();
                    EventOutcome::Handled
                } else {
                    EventOutcome::Ignored
                }
            }
        }
    }

    fn hotkey_has_prefix(&self, prefix: &str) -> bool {
        let prefix = crate::hotkey::normalize_hotkey(prefix);
        self.hotkey_sequences()
            .iter()
            .any(|hotkey| crate::hotkey::normalize_hotkey(hotkey).starts_with(&prefix))
    }

    fn hotkey_matches_sequence(&self, sequence: &str) -> bool {
        self.hotkey.as_deref().is_some_and(|hotkey| {
            crate::hotkey::normalize_hotkey(hotkey) == crate::hotkey::normalize_hotkey(sequence)
        })
    }

    fn hotkey_sequences(&self) -> Vec<String> {
        self.hotkey
            .iter()
            .cloned()
            .chain(
                self.action_hotkeys
                    .iter()
                    .map(|action| action.sequence.clone()),
            )
            .collect()
    }

    fn rebuild_hotkey_matcher(&mut self) {
        self.hotkey_matcher = HotkeySequenceMatcher::new(self.hotkey_sequences());
    }

    fn action_for_match_index(&self, index: usize) -> Option<&PanelActionHotkey<M>> {
        let action_index = index.checked_sub(self.hotkey.is_some() as usize)?;
        self.action_hotkeys.get(action_index)
    }

    fn action_for_sequence(&self, sequence: &str) -> Option<&PanelActionHotkey<M>> {
        self.action_hotkeys.iter().find(|action| {
            crate::hotkey::normalize_hotkey(&action.sequence)
                == crate::hotkey::normalize_hotkey(sequence)
        })
    }
}

fn focus_color_animation() -> AnimationSpec {
    AnimationSpec::default()
}

fn panel_hotkey_matches(hotkey: KeyEvent, key: KeyEvent) -> bool {
    if hotkey.modifiers != key.modifiers {
        return false;
    }
    match (hotkey.code, key.code) {
        (Key::Char(a), Key::Char(b)) => a.eq_ignore_ascii_case(&b),
        (a, b) => a == b,
    }
}

fn bounded_title(title: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    let mut value = format!(" {title} ");
    if line_width(&Line::from(value.as_str())) > max_width {
        value = truncate_cells(&value, max_width);
    }
    value
}

fn visible_lines(
    lines: &[String],
    offset: ScrollOffset,
    viewport: ScrollSize,
) -> Vec<Line<'static>> {
    lines
        .iter()
        .skip(offset.y)
        .take(viewport.height)
        .map(|line| Line::from(trim_cells(line, offset.x, viewport.width)))
        .collect()
}

fn trim_cells(line: &str, skip: usize, width: usize) -> String {
    let end = skip.saturating_add(width);
    let mut cursor = 0;
    let mut trimmed = String::new();

    for ch in line.chars() {
        let ch_width = char_width(ch);
        let next = cursor + ch_width;
        if ch_width == 0 {
            if cursor >= skip && cursor <= end {
                trimmed.push(ch);
            }
        } else if cursor >= skip && next <= end {
            trimmed.push(ch);
        }
        cursor = next;
        if cursor >= end && ch_width > 0 {
            break;
        }
    }

    trimmed
}

fn truncate_cells(value: &str, max_width: usize) -> String {
    let mut width = 0;
    let mut truncated = String::new();

    for ch in value.chars() {
        let ch_width = char_width(ch);
        if ch_width > 0 && width + ch_width > max_width {
            break;
        }
        width += ch_width;
        truncated.push(ch);
    }

    truncated
}

fn char_width(ch: char) -> usize {
    let mut value = String::new();
    value.push(ch);
    line_width(&Line::from(value))
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;

    use crate::{
        EventCtx, EventRoute, Flex, FlexItem, FocusCtx, FocusManager, Key, KeyEvent, LayoutCtx,
        ScrollbarConfig, ScrollbarGutter, ScrollbarStyle, ScrollbarVisibility, TreePath, TuiEvent,
        TuiNode, animation_settings,
    };

    use super::super::{PasswordInput, TextInput, TextareaInput};
    use super::*;

    #[test]
    fn empty_scrollable_panel_still_renders_scrollbars() {
        let mut panel = Panel::<()>::new();
        panel.scroll = Some(
            ScrollState::new(ScrollAxes::Both).scrollbars(ScrollbarConfig {
                vertical: ScrollbarVisibility::Always,
                horizontal: ScrollbarVisibility::Always,
                gutter: ScrollbarGutter::Reserve,
                style: ScrollbarStyle::ThinTrack,
            }),
        );
        let mut terminal = Terminal::new(TestBackend::new(6, 4)).expect("terminal should build");

        terminal
            .draw(|frame| panel.render(frame, frame.area()))
            .expect("panel should render");

        let buffer = terminal.backend().buffer();
        assert_ne!(buffer.cell((4, 2)).unwrap().fg, Color::Reset);
    }

    #[test]
    fn mouse_event_outside_layout_is_ignored() {
        let mut panel = Panel::<()>::new();
        let mut layout = LayoutCtx::new();
        panel.layout(Rect::new(4, 2, 10, 3), &mut layout);
        let mut ctx = EventCtx::default();

        let outcome = panel.event(
            &TuiEvent::Mouse(crate::MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 0,
                row: 0,
                modifiers: crate::KeyModifiers::NONE,
            }),
            &mut ctx,
        );

        assert_eq!(outcome, EventOutcome::Ignored);
        assert_eq!(ctx.focus_request(), None);
    }

    #[test]
    fn panel_click_focuses_its_registered_layout_path() {
        let mut panel = Panel::<()>::new();
        let host_path = TreePath::from_keys([ChildKey::new("host")]);
        let mut layout = LayoutCtx::new();
        layout.push_slot(ChildKey::new("host"), Rect::new(0, 0, 10, 3), |ctx| {
            panel.layout(Rect::new(0, 0, 10, 3), ctx);
        });
        let mut ctx = EventCtx::new_at_path(
            AnimationSettings::default(),
            host_path.child(ChildKey::body()),
        );

        let outcome = panel.event(
            &TuiEvent::Mouse(crate::MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 1,
                row: 1,
                modifiers: crate::KeyModifiers::NONE,
            }),
            &mut ctx,
        );

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(
            ctx.focus_request(),
            Some(&crate::FocusRequest::TargetAt {
                path: host_path,
                id: FocusId::new(PANEL_FOCUS),
            })
        );
    }

    #[test]
    fn clamp_scroll_clamps_offset_after_content_shrinks() {
        let mut settings = AnimationSettings::default();
        settings.enabled = false;
        let area = Rect::new(0, 0, 10, 5);
        let mut panel = Panel::<()>::new()
            .content((0..20).map(|line| format!("line {line}")))
            .scrollable(ScrollAxes::Vertical);

        panel.scroll_to(ScrollOffset::new(0, 99), area, settings);
        panel.content = vec![String::from("line")];
        let outcome = panel.clamp_scroll(area, settings);

        assert!(outcome.changed);
        assert_eq!(
            panel.scroll.as_ref().unwrap().offset(),
            ScrollOffset::new(0, 0)
        );
    }

    #[test]
    fn handled_scroll_key_stops_propagation() {
        let mut panel = Panel::new()
            .content((0..20).map(|line| format!("line {line}")))
            .scrollable(ScrollAxes::Vertical);
        let area = Rect::new(0, 0, 10, 5);
        let mut layout = LayoutCtx::new();
        <Panel as TuiNode<()>>::layout(&mut panel, area, &mut layout);
        let mut ctx = EventCtx::<()>::default();

        let outcome = panel.event(&TuiEvent::Key(KeyEvent::from(crate::Key::Down)), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.propagation(), crate::Propagation::Stopped);
    }

    #[test]
    fn focus_changes_start_color_transitions() {
        let mut panel = Panel::<()>::new().focused(true);

        panel.set_focused(false, animation_settings());

        assert!(panel.border_color.is_active());
        assert!(panel.title_color.is_active());
    }

    #[test]
    fn focus_changes_snap_when_global_animations_are_disabled() {
        let mut animation = AnimationSettings::default();
        animation.enabled = false;

        let mut panel = Panel::<()>::new().focused(true);
        panel.start_focus_color_transition(false, animation);

        let theme = theme();
        assert_eq!(panel.border_color.value(), theme.border_fg());
        assert_eq!(panel.title_color.value(), theme.muted_fg());
        assert!(!panel.border_color.is_active());
        assert!(!panel.title_color.is_active());
    }

    #[test]
    fn render_uses_current_theme_instead_of_stale_idle_colors() {
        let stale_theme = crate::Theme::named(crate::ThemeName::Dracula);
        let mut panel = Panel::<()>::new();
        panel.border_color.snap_to(stale_theme.border_fg());
        panel.title_color.snap_to(stale_theme.muted_fg());
        let expected = theme().border_fg();
        let mut terminal = Terminal::new(TestBackend::new(12, 4)).expect("terminal should build");

        terminal
            .draw(|frame| panel.render(frame, frame.area()))
            .expect("panel should render");

        assert_eq!(
            terminal.backend().buffer().cell((0, 0)).unwrap().fg,
            expected
        );
    }

    #[test]
    fn error_tone_colors_border_and_title_semantically() {
        let panel = Panel::<()>::new().top_left("Email").tone(PanelTone::Error);
        let mut terminal = Terminal::new(TestBackend::new(16, 3)).expect("terminal should build");

        terminal
            .draw(|frame| panel.render(frame, frame.area()))
            .expect("panel should render");

        let buffer = terminal.backend().buffer();
        let error = theme().error_fg();
        assert_eq!(buffer.cell((0, 0)).unwrap().fg, error);
        assert_eq!(buffer.cell((2, 0)).unwrap().fg, error);
    }

    #[test]
    fn error_tone_overrides_active_focus_animation() {
        let mut panel = Panel::<()>::new().top_left("Email").focused(true);
        panel.set_focused(false, animation_settings());
        assert!(panel.border_color.is_active());
        panel.set_tone(PanelTone::Error);
        let mut terminal = Terminal::new(TestBackend::new(16, 3)).expect("terminal should build");

        terminal
            .draw(|frame| panel.render(frame, frame.area()))
            .expect("panel should render");

        let buffer = terminal.backend().buffer();
        let error = theme().error_fg();
        assert_eq!(buffer.cell((0, 0)).unwrap().fg, error);
        assert_eq!(buffer.cell((2, 0)).unwrap().fg, error);
    }

    #[test]
    fn one_row_panel_renders_only_top_border() {
        let panel = Panel::<()>::new()
            .one_row(true)
            .top_left("Title")
            .bottom_left("Hidden")
            .hotkey("p")
            .border(BorderKind::Plain)
            .content(["Body"]);
        let mut terminal = Terminal::new(TestBackend::new(20, 4)).expect("terminal should build");

        terminal
            .draw(|frame| panel.render(frame, frame.area()))
            .expect("panel should render");

        let buffer = terminal.backend().buffer();
        let row = |y| -> String {
            (0..20)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        };

        assert!(row(0).contains("Title"), "{}", row(0));
        assert!(row(0).contains('─'), "{}", row(0));
        assert!(!row(0).contains('┌'), "{}", row(0));
        assert!(!row(0).contains('┐'), "{}", row(0));
        assert!(row(1).starts_with("Body"), "{}", row(1));
        assert!(!row(1).contains('│'), "{}", row(1));
        assert_eq!(row(3), " ".repeat(20));
    }

    #[derive(Debug, PartialEq, Eq)]
    enum Msg {
        Submit(String),
    }

    struct StaticBody;

    impl TuiNode<()> for StaticBody {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(
            &self,
            _frame: &mut ratatui::Frame,
            _area: Rect,
            _ctx: &mut crate::RenderCtx<'_>,
        ) {
        }
    }

    #[test]
    fn panel_host_registers_fallback_focus_when_child_has_none() {
        let mut host = Panel::new().top_left("Preview").host(StaticBody);
        let mut layout = LayoutCtx::new();

        host.layout(Rect::new(0, 0, 20, 4), &mut layout);

        assert_eq!(layout.focus_targets().len(), 1);
        let target = layout.focus_targets()[0].clone();
        assert_eq!(target.id.as_str(), "panel");
        assert!(target.path.is_empty());

        let mut focus = FocusCtx::new(AnimationSettings::default());
        host.dispatch_focus(&target, true, &mut focus);

        assert!(host.panel().is_focused());
        assert!(focus.redraw_requested());
    }

    #[test]
    fn panel_host_keeps_child_hit_regions_above_its_host_region() {
        let mut host = Panel::new().host(StaticBody);
        let host_path = TreePath::from_keys([ChildKey::new("host")]);
        let child_path = host_path.child(ChildKey::body());
        let mut layout = LayoutCtx::new();
        layout.push_slot(ChildKey::new("host"), Rect::new(0, 0, 10, 3), |ctx| {
            host.layout(Rect::new(0, 0, 10, 3), ctx);
        });

        assert_eq!(layout.hit_regions()[1].path, host_path);
        assert_eq!(layout.hit_regions()[2].path, child_path);

        let mut ctx = EventCtx::new_at_path(AnimationSettings::default(), child_path);
        let outcome = host.dispatch_event(
            &EventRoute::new(TreePath::from_keys([ChildKey::body()])),
            &TuiEvent::Mouse(crate::MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 1,
                row: 1,
                modifiers: crate::KeyModifiers::NONE,
            }),
            &mut ctx,
        );

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(
            ctx.focus_request(),
            Some(&crate::FocusRequest::TargetAt {
                path: host_path,
                id: FocusId::new(PANEL_FOCUS),
            })
        );
    }

    #[test]
    fn one_row_panel_host_gives_child_full_width_below_top_border() {
        let mut host = Panel::new().one_row(true).host(StaticBody);
        let mut layout = LayoutCtx::new();

        host.layout(Rect::new(0, 0, 20, 4), &mut layout);

        assert_eq!(host.child_area(), Rect::new(0, 1, 20, 3));
    }

    #[test]
    fn panel_host_preserves_hotkey_on_fallback_focus() {
        let mut host = Panel::new().hotkey("p").host(StaticBody);
        let mut layout = LayoutCtx::new();

        host.layout(Rect::new(0, 0, 20, 4), &mut layout);

        assert_eq!(
            layout.focus_targets()[0].hotkey,
            Some(KeyEvent::from(Key::Char('p')))
        );
    }

    #[test]
    fn panel_host_attaches_hotkey_to_child_focus_target() {
        let mut host = Panel::new().hotkey("p").host(TextInput::<()>::new());
        let mut layout = LayoutCtx::new();

        host.layout(Rect::new(0, 0, 20, 4), &mut layout);

        assert_eq!(layout.focus_targets().len(), 2);
        assert_eq!(
            layout.focus_targets()[0].hotkey,
            Some(KeyEvent::from(Key::Char('p')))
        );
        assert_eq!(
            layout.focus_targets()[0].path,
            TreePath::from_keys([ChildKey::body()])
        );
        assert_eq!(layout.focus_targets()[1].id.as_str(), "panel");
        assert!(layout.focus_targets()[1].path.is_empty());
    }

    #[test]
    fn panel_host_attaches_all_action_hotkeys_to_child_focus_target() {
        let mut host = Panel::new()
            .hotkey("p")
            .action_hotkey("ra", || ())
            .action_hotkey("ca", || ())
            .host(TextInput::<()>::new());
        let mut layout = LayoutCtx::new();

        host.layout(Rect::new(0, 0, 20, 4), &mut layout);

        assert_eq!(
            layout.focus_targets()[0].hotkey_sequences,
            vec!["p", "ra", "ca"]
        );
    }

    #[test]
    fn panel_host_fallback_siblings_traverse_once_each() {
        let mut flex = Flex::row()
            .child("first", Panel::new().host(StaticBody), FlexItem::fixed(10))
            .child("second", Panel::new().host(StaticBody), FlexItem::fixed(10));
        let mut layout = LayoutCtx::new();

        flex.layout(Rect::new(0, 0, 20, 4), &mut layout);

        assert_eq!(layout.focus_targets().len(), 2);
        let mut focus = FocusManager::new();
        focus.validate(layout.focus_targets());
        assert_eq!(
            focus.current().unwrap().path,
            TreePath::from_keys([ChildKey::new("first")])
        );

        focus.next(layout.focus_targets());

        assert_eq!(
            focus.current().unwrap().path,
            TreePath::from_keys([ChildKey::new("second")])
        );
    }

    #[test]
    fn panel_host_routes_focus_keys_submit_redraw_and_tick() {
        let mut host = Panel::new().top_left("Filter").host(
            TextInput::new()
                .placeholder("Search…")
                .on_submit(Msg::Submit),
        );
        let area = Rect::new(0, 0, 20, 3);
        let mut layout = LayoutCtx::new();

        host.layout(area, &mut layout);
        let target = layout.focus_targets()[0].clone();
        let route = EventRoute::new(target.path.clone());
        let mut focus = FocusCtx::new(AnimationSettings::default());
        host.dispatch_focus(&target, true, &mut focus);

        assert!(focus.redraw_requested());
        assert!(host.panel.border_color.is_active());

        let mut enter_insert = EventCtx::new(AnimationSettings::default());
        let enter_outcome = host.dispatch_event(
            &route,
            &TuiEvent::Key(KeyEvent::from(Key::Enter)),
            &mut enter_insert,
        );
        let mut key = EventCtx::new(AnimationSettings::default());
        let key_outcome = host.dispatch_event(
            &route,
            &TuiEvent::Key(KeyEvent::from(Key::Char('x'))),
            &mut key,
        );

        assert!(enter_outcome.handled());
        assert_eq!(
            enter_insert.drain_messages().collect::<Vec<_>>(),
            vec![Msg::Submit(String::new())]
        );
        assert!(enter_insert.redraw_requested());
        assert!(key_outcome.handled());
        assert_eq!(key.propagation(), crate::Propagation::Stopped);
        assert!(key.redraw_requested());
        assert_eq!(host.child().current_value(), "x");

        let mut submit = EventCtx::new(AnimationSettings::default());
        host.dispatch_event(
            &route,
            &TuiEvent::Key(KeyEvent::from(Key::Enter)),
            &mut submit,
        );

        assert!(submit.drain_messages().next().is_none());
        assert!(submit.redraw_requested());
        assert!(
            TuiNode::tick(
                &mut host,
                Duration::from_millis(16),
                AnimationSettings::default()
            )
            .active
        );
    }

    #[test]
    fn panel_host_hotkey_is_consumed_before_child_input() {
        let mut host = Panel::new().hotkey("p").host(TextInput::<()>::new());
        let mut layout = LayoutCtx::new();

        host.layout(Rect::new(0, 0, 20, 3), &mut layout);
        let route = EventRoute::new(layout.focus_targets()[0].path.clone());
        let mut ctx = EventCtx::<()>::default();

        let outcome = host.dispatch_event(
            &route,
            &TuiEvent::Key(KeyEvent::from(Key::Char('p'))),
            &mut ctx,
        );

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.propagation(), crate::Propagation::Stopped);
        assert_eq!(host.child().current_value(), "");
    }

    #[test]
    fn panel_host_multiletter_hotkey_is_consumed_before_child_input() {
        let mut host = Panel::new().hotkey("pa").host(TextInput::<()>::new());
        let mut layout = LayoutCtx::new();

        host.layout(Rect::new(0, 0, 20, 3), &mut layout);
        let route = EventRoute::new(layout.focus_targets()[0].path.clone());
        let mut ctx = EventCtx::<()>::default();

        let pending = host.dispatch_event(
            &route,
            &TuiEvent::Key(KeyEvent::from(Key::Char('p'))),
            &mut ctx,
        );
        let matched = host.dispatch_event(
            &route,
            &TuiEvent::Key(KeyEvent::from(Key::Char('a'))),
            &mut ctx,
        );

        assert_eq!(pending, EventOutcome::Handled);
        assert_eq!(matched, EventOutcome::Handled);
        assert_eq!(ctx.propagation(), crate::Propagation::Stopped);
        assert_eq!(host.child().current_value(), "");
    }

    #[test]
    fn focused_panel_multiletter_hotkey_is_consumed_from_key_events() {
        let mut panel = Panel::new().hotkey("pa");
        let mut ctx = EventCtx::<()>::default();

        let pending = panel.event(&TuiEvent::Key(KeyEvent::from(Key::Char('p'))), &mut ctx);
        let matched = panel.event(&TuiEvent::Key(KeyEvent::from(Key::Char('a'))), &mut ctx);

        assert_eq!(pending, EventOutcome::Handled);
        assert_eq!(matched, EventOutcome::Handled);
        assert_eq!(ctx.propagation(), crate::Propagation::Stopped);
    }

    #[test]
    fn top_titles_always_render_standard() {
        let panel = Panel::<()>::new()
            .top_left("Processes")
            .border(BorderKind::Plain)
            .content(["✖ No processes running"]);
        let mut terminal = Terminal::new(TestBackend::new(24, 4)).expect("terminal should build");

        terminal
            .draw(|frame| panel.render(frame, frame.area()))
            .expect("panel should render");

        let buffer = terminal.backend().buffer();
        let top = (0..24)
            .map(|x| buffer.cell((x, 0)).unwrap().symbol())
            .collect::<String>();
        assert_eq!(top, "┌─ Processes ──────────┐");
    }

    #[test]
    fn bottom_left_title_and_bottom_right_hotkey_render_inset() {
        let panel = Panel::<()>::new()
            .bottom_left("Left")
            .hotkey("r")
            .border(BorderKind::Plain);
        let mut terminal = Terminal::new(TestBackend::new(24, 4)).expect("terminal should build");

        terminal
            .draw(|frame| panel.render(frame, frame.area()))
            .expect("panel should render");

        let buffer = terminal.backend().buffer();
        let bottom = (0..24)
            .map(|x| buffer.cell((x, 3)).unwrap().symbol())
            .collect::<String>();
        assert_eq!(bottom, "└┤ Left ├────────────┤r│");
    }

    #[test]
    fn panel_bottom_right_title_and_hotkey_align_with_border_snapshot() {
        let panel = Panel::<()>::new()
            .top_left("Services")
            .bottom_right("Ready")
            .hotkey("run")
            .border(BorderKind::Plain)
            .content(["Body"]);
        let mut terminal = Terminal::new(TestBackend::new(36, 5)).expect("terminal should build");

        terminal
            .draw(|frame| panel.render(frame, frame.area()))
            .expect("panel should render");

        let buffer = terminal.backend().buffer();
        let rendered = (0..5)
            .map(|y| {
                (0..36)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        let expected = [
            "┌─ Services ───────────────────────┐",
            "│Body                              │",
            "│                                  │",
            "│                                  │",
            "└────────────────────┤ Ready ├─┤run│",
        ]
        .join("\n");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn panel_bottom_right_title_slot_is_independent_from_hotkey() {
        let mut panel = Panel::<()>::new().bottom_right("State").hotkey("r");

        panel.clear_title(PanelTitlePosition::BottomRight);

        assert!(panel.title_slot(PanelTitlePosition::BottomRight).is_none());
        assert_eq!(panel.hotkey.as_deref(), Some("r"));
    }

    #[test]
    fn panel_registers_hotkey_with_focus_target() {
        let mut panel = Panel::new().hotkey("p");
        let mut ctx = LayoutCtx::new();

        <Panel as TuiNode<()>>::layout(&mut panel, Rect::new(0, 0, 20, 4), &mut ctx);

        assert_eq!(
            ctx.focus_targets()[0].hotkey,
            Some(KeyEvent::from(Key::Char('p')))
        );
    }

    #[test]
    fn panel_action_hotkeys_render_register_and_emit_messages() {
        let mut panel = Panel::new()
            .hotkey("p")
            .action_hotkey("ra", || "refresh")
            .action_hotkey("ca", || "clear")
            .border(BorderKind::Plain);
        let area = Rect::new(0, 0, 24, 4);
        let mut layout = LayoutCtx::new();
        panel.layout(area, &mut layout);
        let mut terminal = Terminal::new(TestBackend::new(24, 4)).expect("terminal should build");

        terminal
            .draw(|frame| panel.render(frame, frame.area()))
            .expect("panel should render");
        let bottom = (0..24)
            .map(|x| terminal.backend().buffer().cell((x, 3)).unwrap().symbol())
            .collect::<String>();

        assert_eq!(
            layout.focus_targets()[0].hotkey_sequences,
            vec!["p", "ra", "ca"]
        );
        assert!(bottom.ends_with("┤p·ra·ca│"), "{bottom}");

        let mut ctx = EventCtx::default();
        let outcome = panel.event(
            &TuiEvent::Hotkey(HotkeyEvent::Commit("ra".to_string())),
            &mut ctx,
        );

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.messages(), &["refresh"]);

        let mut ctx = EventCtx::default();
        assert_eq!(
            panel.event(&TuiEvent::Key(KeyEvent::from(Key::Char('c'))), &mut ctx),
            EventOutcome::Handled
        );
        assert_eq!(
            panel.event(&TuiEvent::Key(KeyEvent::from(Key::Char('a'))), &mut ctx),
            EventOutcome::Handled
        );
        assert_eq!(ctx.messages(), &["clear"]);
    }

    #[test]
    fn panel_hotkey_event_is_consumed_when_focused() {
        let mut panel = Panel::new().hotkey("p");
        let mut ctx = EventCtx::<()>::default();

        let outcome = panel.event(&TuiEvent::Key(KeyEvent::from(Key::Char('p'))), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.propagation(), crate::Propagation::Stopped);
    }

    #[test]
    fn panel_hotkey_pending_prefix_is_render_state() {
        let mut panel = Panel::new().hotkey("pa");
        let mut ctx = EventCtx::<()>::default();

        let outcome = panel.event(
            &TuiEvent::Hotkey(HotkeyEvent::Pending("p".into())),
            &mut ctx,
        );

        assert_eq!(outcome, EventOutcome::Ignored);
        assert_eq!(panel.pending_hotkey_prefix.as_deref(), Some("p"));
        assert!(ctx.redraw_requested());
    }

    #[test]
    fn panel_host_hotkey_commit_clears_pending_prefix_and_stops_propagation() {
        let mut host = Panel::new().hotkey("pa").host(StaticBody);
        let mut layout = LayoutCtx::new();

        host.layout(Rect::new(0, 0, 20, 3), &mut layout);
        let route = EventRoute::new(layout.focus_targets()[0].path.clone());
        let mut pending = EventCtx::<()>::default();
        host.dispatch_event(
            &route,
            &TuiEvent::Hotkey(HotkeyEvent::Pending("p".into())),
            &mut pending,
        );

        let mut commit = EventCtx::<()>::default();
        let outcome = host.dispatch_event(
            &route,
            &TuiEvent::Hotkey(HotkeyEvent::Commit("pa".into())),
            &mut commit,
        );

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(host.panel().pending_hotkey_prefix, None);
        assert_eq!(commit.propagation(), crate::Propagation::Stopped);
        assert!(commit.redraw_requested());
    }

    #[test]
    fn panel_hotkey_commit_activates_nested_text_input() {
        let mut host = Panel::new().hotkey("p").host(TextInput::<()>::new());
        let mut layout = LayoutCtx::new();

        host.layout(Rect::new(0, 0, 20, 3), &mut layout);
        let route = EventRoute::new(layout.focus_targets()[0].path.clone());
        let mut commit = EventCtx::<()>::default();
        let commit_outcome = host.dispatch_event(
            &route,
            &TuiEvent::Hotkey(HotkeyEvent::Commit("p".into())),
            &mut commit,
        );
        let mut key = EventCtx::<()>::default();
        let key_outcome = host.dispatch_event(
            &route,
            &TuiEvent::Key(KeyEvent::from(Key::Char('x'))),
            &mut key,
        );

        assert_eq!(commit_outcome, EventOutcome::Handled);
        assert_eq!(key_outcome, EventOutcome::Handled);
        assert_eq!(host.child().current_value(), "x");
        assert!(commit.layout_requested());
        assert_eq!(commit.propagation(), crate::Propagation::Stopped);
    }

    #[test]
    fn panel_hotkey_commit_activates_nested_textarea_input() {
        let mut host = Panel::new().hotkey("p").host(TextareaInput::<()>::new());
        let mut layout = LayoutCtx::new();

        host.layout(Rect::new(0, 0, 20, 3), &mut layout);
        let route = EventRoute::new(layout.focus_targets()[0].path.clone());
        host.dispatch_event(
            &route,
            &TuiEvent::Hotkey(HotkeyEvent::Commit("p".into())),
            &mut EventCtx::<()>::default(),
        );
        let key_outcome = host.dispatch_event(
            &route,
            &TuiEvent::Key(KeyEvent::from(Key::Char('x'))),
            &mut EventCtx::<()>::default(),
        );

        assert_eq!(key_outcome, EventOutcome::Handled);
        assert_eq!(host.child().current_value(), "x");
    }

    #[test]
    fn panel_hotkey_commit_activates_nested_password_input() {
        let mut host = Panel::new().hotkey("p").host(PasswordInput::<()>::new());
        let mut layout = LayoutCtx::new();

        host.layout(Rect::new(0, 0, 20, 3), &mut layout);
        let route = EventRoute::new(layout.focus_targets()[0].path.clone());
        host.dispatch_event(
            &route,
            &TuiEvent::Hotkey(HotkeyEvent::Commit("p".into())),
            &mut EventCtx::<()>::default(),
        );
        let key_outcome = host.dispatch_event(
            &route,
            &TuiEvent::Key(KeyEvent::from(Key::Char('x'))),
            &mut EventCtx::<()>::default(),
        );

        assert_eq!(key_outcome, EventOutcome::Handled);
        assert_eq!(host.child().current_value(), "x");
    }
}
