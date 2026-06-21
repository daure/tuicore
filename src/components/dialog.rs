use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::event::{KeyEvent, TuiEvent};
use crate::{
    Animated, AnimationSettings, AnimationSpec, BorderKind, ChildKey, ColorTween, EventCtx,
    EventOutcome, EventRoute, FocusCtx, FocusId, FocusRequest, FocusTarget, KeySpec, LayoutCtx,
    LayoutResult, LifecycleCtx, ScrollAxes, ScrollBehavior, ScrollDelta, ScrollGeometry,
    ScrollLayout, ScrollOffset, ScrollOutcome, ScrollSize, ScrollState, TickResult, TuiNode,
    border_set, hotkey_edge_spans, keybindings, line_width, paragraph_scroll, preset, theme,
};

pub(crate) const DIALOG_FOCUS: &str = "dialog";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogTitlePosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogCloseReason {
    CloseKey,
    Escape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogKeyBindings {
    pub close: Vec<KeySpec>,
}

impl Default for DialogKeyBindings {
    fn default() -> Self {
        Self {
            close: vec![KeySpec::plain('x')],
        }
    }
}

impl DialogKeyBindings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn close_label(&self) -> Option<String> {
        self.close.first().map(|key| key.label())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DialogTitle {
    text: String,
}

pub struct Dialog<M = ()> {
    top_left: Option<DialogTitle>,
    top_right: Option<DialogTitle>,
    bottom_left: Option<DialogTitle>,
    bottom_right: Option<DialogTitle>,
    border: Option<BorderKind>,
    content: Vec<String>,
    scroll: Option<ScrollState>,
    on_close: Option<Box<dyn Fn(DialogCloseReason) -> M>>,
    focused: bool,
    border_color: ColorTween,
    title_color: ColorTween,
    area: Rect,
    keys: DialogKeyBindings,
}

pub struct DialogHost<C, M = ()> {
    dialog: Dialog<M>,
    child: C,
    child_area: Rect,
}

impl<M> Default for Dialog<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M> Dialog<M> {
    pub fn new() -> Self {
        let theme = theme();
        Self {
            top_left: None,
            top_right: None,
            bottom_left: None,
            bottom_right: None,
            border: None,
            content: Vec::new(),
            scroll: None,
            on_close: None,
            focused: false,
            border_color: ColorTween::idle(theme.border_fg()),
            title_color: ColorTween::idle(theme.muted_fg()),
            area: Rect::default(),
            keys: DialogKeyBindings::default(),
        }
    }

    pub fn top_left(mut self, title: impl Into<String>) -> Self {
        self.set_top_left(title);
        self
    }

    pub fn set_top_left(&mut self, title: impl Into<String>) {
        self.top_left = Some(DialogTitle::standard(title));
    }

    pub fn top_right(mut self, title: impl Into<String>) -> Self {
        self.set_top_right(title);
        self
    }

    pub fn set_top_right(&mut self, title: impl Into<String>) {
        self.top_right = Some(DialogTitle::standard(title));
    }

    pub fn bottom_left(mut self, title: impl Into<String>) -> Self {
        self.set_bottom_left(title);
        self
    }

    pub fn set_bottom_left(&mut self, title: impl Into<String>) {
        self.bottom_left = Some(DialogTitle::standard(title));
    }

    pub fn bottom_right(mut self, title: impl Into<String>) -> Self {
        self.set_bottom_right(title);
        self
    }

    pub fn set_bottom_right(&mut self, title: impl Into<String>) {
        self.bottom_right = Some(DialogTitle::standard(title));
    }

    pub fn title(mut self, position: DialogTitlePosition, title: impl Into<String>) -> Self {
        self.set_title(position, title);
        self
    }

    pub fn set_title(&mut self, position: DialogTitlePosition, title: impl Into<String>) {
        *self.title_slot_mut(position) = Some(DialogTitle::standard(title));
    }

    pub fn clear_title(&mut self, position: DialogTitlePosition) {
        *self.title_slot_mut(position) = None;
    }

    pub fn border(mut self, border: BorderKind) -> Self {
        self.border = Some(border);
        self
    }

    pub fn content(mut self, lines: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.content = lines.into_iter().map(Into::into).collect();
        self
    }

    pub fn set_content(&mut self, lines: impl IntoIterator<Item = impl Into<String>>) {
        self.content = lines.into_iter().map(Into::into).collect();
    }

    pub fn clear_content(&mut self) {
        self.content.clear();
    }

    pub fn on_close(mut self, handler: impl Fn(DialogCloseReason) -> M + 'static) -> Self {
        self.on_close = Some(Box::new(handler));
        self
    }

    pub fn keybindings(mut self, keys: DialogKeyBindings) -> Self {
        self.keys = keys;
        self
    }

    pub fn set_keybindings(&mut self, keys: DialogKeyBindings) {
        self.keys = keys;
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

    pub fn host<C>(self, child: C) -> DialogHost<C, M> {
        DialogHost {
            dialog: self,
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
        let inner = Self::inner_area(area);
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

    pub fn inner_area(area: Rect) -> Rect {
        Rect::new(
            area.x.saturating_add(1),
            area.y.saturating_add(1),
            area.width.saturating_sub(2),
            area.height.saturating_sub(2),
        )
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        frame.render_widget(Clear, area);
        let border = self.border.unwrap_or_else(|| preset().border());
        let border_style = Style::default().fg(self.visible_border_color());
        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border_set(border))
            .border_style(border_style);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        self.render_titles(frame, area, border);

        if !inner.is_empty() {
            let lines = self
                .content
                .iter()
                .map(|line| Line::from(line.clone()))
                .collect::<Vec<_>>();
            if let Some(scroll) = &self.scroll {
                let geometry = scroll.geometry(inner, self.content_size());
                let offset = scroll.offset();
                let paragraph = Paragraph::new(lines)
                    .alignment(Alignment::Left)
                    .scroll(paragraph_scroll(offset));
                frame.render_widget(paragraph, geometry.layout.viewport);
                scroll.render_scrollbars(frame, geometry.layout, geometry.content, self.focused);
            } else {
                frame.render_widget(Paragraph::new(lines).alignment(Alignment::Left), inner);
            }
        }
    }

    fn close(&self, reason: DialogCloseReason, ctx: &mut EventCtx<M>) {
        if let Some(on_close) = &self.on_close {
            ctx.emit(on_close(reason));
        }
        ctx.focus(FocusRequest::Last);
        ctx.stop_propagation();
        ctx.request_redraw();
        ctx.request_layout();
    }

    fn close_reason(&self, key: KeyEvent) -> Option<DialogCloseReason> {
        if matches_any(&self.keys.close, key) {
            Some(DialogCloseReason::CloseKey)
        } else if keybindings().focus().unfocus_matches(key) {
            Some(DialogCloseReason::Escape)
        } else {
            None
        }
    }

    fn render_titles(&self, frame: &mut Frame, area: Rect, border: BorderKind) {
        self.render_top_left_title(frame, area);
        self.render_top_right_title(frame, area);
        self.render_bottom_title(frame, area, DialogTitlePosition::BottomLeft);
        self.render_bottom_title(frame, area, DialogTitlePosition::BottomRight);
        self.render_close_label(frame, area, border);
    }

    fn render_top_left_title(&self, frame: &mut Frame, area: Rect) {
        let Some(title) = self.top_left.as_ref() else {
            return;
        };
        let close_width = self.close_label_width();
        self.render_plain_title(frame, area, title, Alignment::Left, area.y, close_width + 1);
    }

    fn render_top_right_title(&self, frame: &mut Frame, area: Rect) {
        let Some(title) = self.top_right.as_ref() else {
            return;
        };
        let close_width = self.close_label_width();
        self.render_plain_title(
            frame,
            area,
            title,
            Alignment::Right,
            area.y,
            close_width + 1,
        );
    }

    fn close_label_width(&self) -> u16 {
        self.keys
            .close_label()
            .map(|label| close_label_width(&label))
            .unwrap_or_default()
    }

    fn render_bottom_title(&self, frame: &mut Frame, area: Rect, position: DialogTitlePosition) {
        let Some(title) = self.title_slot(position) else {
            return;
        };
        let alignment = match position {
            DialogTitlePosition::BottomLeft => Alignment::Left,
            DialogTitlePosition::BottomRight => Alignment::Right,
            DialogTitlePosition::TopLeft | DialogTitlePosition::TopRight => return,
        };
        self.render_plain_title(
            frame,
            area,
            title,
            alignment,
            area.y + area.height.saturating_sub(1),
            0,
        );
    }

    fn render_plain_title(
        &self,
        frame: &mut Frame,
        area: Rect,
        title: &DialogTitle,
        alignment: Alignment,
        y: u16,
        reserved_right: u16,
    ) {
        if area.width <= 4 + reserved_right {
            return;
        }
        let max_width = area.width.saturating_sub(4 + reserved_right) as usize;
        let title = bounded_title(&title.text, max_width);
        let width = line_width(&Line::from(title.as_str())).min(u16::MAX as usize) as u16;
        if width == 0 {
            return;
        }
        let x = match alignment {
            Alignment::Left => area.x.saturating_add(2),
            Alignment::Center => area.x + area.width.saturating_sub(width) / 2,
            Alignment::Right => area.x.saturating_add(
                area.width
                    .saturating_sub(width)
                    .saturating_sub(2 + reserved_right),
            ),
        };
        let style = Style::default()
            .fg(self.visible_title_color())
            .add_modifier(Modifier::BOLD);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(title, style))),
            Rect::new(x, y, width, 1),
        );
    }

    fn render_close_label(&self, frame: &mut Frame, area: Rect, border: BorderKind) {
        let Some(label) = self.keys.close_label() else {
            return;
        };
        let width = close_label_width(&label);
        if area.width <= width + 2 {
            return;
        }
        let border_style = Style::default().fg(self.visible_border_color());
        let title_style = Style::default()
            .fg(self.visible_title_color())
            .add_modifier(Modifier::BOLD);
        let line = Line::from(hotkey_edge_spans(
            &label,
            None,
            border,
            border_style,
            title_style,
            title_style,
        ));
        let x = area.x + area.width.saturating_sub(width);
        frame.render_widget(Paragraph::new(line), Rect::new(x, area.y, width, 1));
    }

    fn visible_border_color(&self) -> ratatui::style::Color {
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

    fn title_slot(&self, position: DialogTitlePosition) -> Option<&DialogTitle> {
        match position {
            DialogTitlePosition::TopLeft => self.top_left.as_ref(),
            DialogTitlePosition::TopRight => self.top_right.as_ref(),
            DialogTitlePosition::BottomLeft => self.bottom_left.as_ref(),
            DialogTitlePosition::BottomRight => self.bottom_right.as_ref(),
        }
    }

    fn title_slot_mut(&mut self, position: DialogTitlePosition) -> &mut Option<DialogTitle> {
        match position {
            DialogTitlePosition::TopLeft => &mut self.top_left,
            DialogTitlePosition::TopRight => &mut self.top_right,
            DialogTitlePosition::BottomLeft => &mut self.bottom_left,
            DialogTitlePosition::BottomRight => &mut self.bottom_right,
        }
    }

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
}

impl DialogTitle {
    fn standard(title: impl Into<String>) -> Self {
        Self { text: title.into() }
    }
}

impl<M> Animated for Dialog<M> {
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        let scroll = self
            .scroll
            .as_mut()
            .map(|scroll| scroll.tick(dt, settings))
            .unwrap_or(TickResult::IDLE);
        scroll
            .merge(self.border_color.tick(dt, settings))
            .merge(self.title_color.tick(dt, settings))
    }
}

impl<M> TuiNode<M> for Dialog<M> {
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.area = area;
        ctx.register_focusable(FocusId::new(DIALOG_FOCUS), area, true);
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        Self::render(self, frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        let TuiEvent::Key(key) = event else {
            ctx.stop_propagation();
            return EventOutcome::Handled;
        };
        if let Some(reason) = self.close_reason(*key) {
            self.close(reason, ctx);
            return EventOutcome::Handled;
        }
        let outcome = self.on_key(*key, self.area, ctx.animation());
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

impl<C, M> DialogHost<C, M> {
    pub fn dialog(&self) -> &Dialog<M> {
        &self.dialog
    }

    pub fn dialog_mut(&mut self) -> &mut Dialog<M> {
        &mut self.dialog
    }

    pub fn child(&self) -> &C {
        &self.child
    }

    pub fn child_mut(&mut self) -> &mut C {
        &mut self.child
    }
}

impl<C, M> TuiNode<M> for DialogHost<C, M>
where
    C: TuiNode<M>,
{
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.dialog.area = area;
        let inner = Dialog::<M>::inner_area(area);
        self.child_area = inner;
        let fallback_inserted = ctx
            .with_focus_fallback_status(FocusId::new(DIALOG_FOCUS), area, |ctx| {
                ctx.push_slot(ChildKey::body(), inner, |ctx| {
                    self.child.layout(inner, ctx);
                });
            })
            .1;
        if !fallback_inserted {
            ctx.register_focusable(FocusId::new(DIALOG_FOCUS), area, true);
        }
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        self.dialog.render(frame, area);
        self.child.render(frame, self.child_area);
        crate::separator::patch_border_joins(
            frame,
            area,
            self.child_area,
            self.dialog.border.unwrap_or_else(|| preset().border()),
            Style::default().fg(self.dialog.visible_border_color()),
        );
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        self.dialog.event(event, ctx)
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
        let body = ChildKey::body();
        let child = route
            .path
            .without_first_if(&body)
            .map(EventRoute::new)
            .map(|route| self.child.dispatch_event(&route, event, ctx))
            .unwrap_or(EventOutcome::Ignored);
        if is_focus_unfocus_event(event) && ctx.propagation() == crate::Propagation::Stopped {
            return child;
        }
        child.bubble(ctx, |ctx| self.event(event, ctx))
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(&mut self.dialog, dt, settings).merge(self.child.tick(dt, settings))
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        if target.path.is_empty() && target.id.as_str() == DIALOG_FOCUS {
            self.dialog.set_focused(focused, ctx.animation());
            ctx.request_redraw();
            return;
        }
        let body = ChildKey::body();
        if let Some(child_target) = target.for_child(&body) {
            self.dialog.set_focused(focused, ctx.animation());
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

fn focus_color_animation() -> AnimationSpec {
    AnimationSpec::default()
}

fn close_label_width(label: &str) -> u16 {
    line_width(&Line::from(format!("┤{label}│"))).min(u16::MAX as usize) as u16
}

fn is_focus_unfocus_event(event: &TuiEvent) -> bool {
    let TuiEvent::Key(key) = event else {
        return false;
    };
    keybindings().focus().unfocus_matches(*key)
}

fn matches_any(bindings: &[KeySpec], key: KeyEvent) -> bool {
    bindings.iter().any(|binding| binding.matches(key))
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

    use super::*;
    use crate::{Key, TextInput, animation_settings};

    struct StaticBody;

    struct TopRightVerticalBody;

    struct BottomRightVerticalBody;

    impl TuiNode<()> for StaticBody {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut ratatui::Frame, _area: Rect) {}
    }

    impl TuiNode<()> for TopRightVerticalBody {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, frame: &mut ratatui::Frame, area: Rect) {
            frame.buffer_mut().set_string(
                area.right().saturating_sub(1),
                area.y,
                "│",
                Style::default(),
            );
        }
    }

    impl TuiNode<()> for BottomRightVerticalBody {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, frame: &mut ratatui::Frame, area: Rect) {
            frame.buffer_mut().set_string(
                area.right().saturating_sub(1),
                area.bottom().saturating_sub(1),
                "│",
                Style::default(),
            );
        }
    }

    #[test]
    fn dialog_renders_all_title_slots_and_fixed_close_label() {
        let dialog = Dialog::<()>::new()
            .top_left("Title")
            .top_right("State")
            .bottom_left("Help")
            .bottom_right("Enter OK")
            .content(["Body"]);
        let mut terminal = Terminal::new(TestBackend::new(40, 5)).expect("terminal should build");

        terminal
            .draw(|frame| dialog.render(frame, frame.area()))
            .expect("dialog should render");

        let buffer = terminal.backend().buffer();
        let rendered = (0..5)
            .flat_map(|y| (0..40).map(move |x| buffer.cell((x, y)).unwrap().symbol()))
            .collect::<String>();
        assert!(rendered.contains("Title"));
        assert!(rendered.contains("State"));
        assert!(rendered.contains("Help"));
        assert!(rendered.contains("Enter OK"));
        assert!(rendered.contains("┤x│"));
    }

    #[test]
    fn dialog_top_right_close_hotkey_aligns_with_border_snapshot() {
        let dialog = Dialog::<()>::new()
            .top_left("Prompt")
            .top_right("Ready")
            .content(["Body"]);
        let mut terminal = Terminal::new(TestBackend::new(40, 5)).expect("terminal should build");

        terminal
            .draw(|frame| dialog.render(frame, frame.area()))
            .expect("dialog should render");

        let buffer = terminal.backend().buffer();
        let rendered = (0..5)
            .map(|y| {
                (0..40)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        let expected = [
            "╭─ Prompt ───────────────── Ready ───┤x│",
            "│Body                                  │",
            "│                                      │",
            "│                                      │",
            "╰──────────────────────────────────────╯",
        ]
        .join("\n");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn dialog_host_join_patch_does_not_overwrite_close_label() {
        let mut host = Dialog::<()>::new().host(TopRightVerticalBody);
        let mut layout = LayoutCtx::new();
        host.layout(Rect::new(0, 0, 20, 6), &mut layout);
        let mut terminal = Terminal::new(TestBackend::new(20, 6)).expect("terminal should build");

        terminal
            .draw(|frame| host.render(frame, frame.area()))
            .expect("dialog host should render");

        let buffer = terminal.backend().buffer();
        let top_line = (0..20)
            .map(|x| buffer.cell((x, 0)).unwrap().symbol())
            .collect::<String>();
        assert!(top_line.ends_with("┤x│"), "{top_line}");
    }

    #[test]
    fn dialog_host_join_patch_does_not_join_scrollbar_to_bottom_border() {
        let mut host = Dialog::<()>::new().host(BottomRightVerticalBody);
        let mut layout = LayoutCtx::new();
        host.layout(Rect::new(0, 0, 20, 6), &mut layout);
        let mut terminal = Terminal::new(TestBackend::new(20, 6)).expect("terminal should build");

        terminal
            .draw(|frame| host.render(frame, frame.area()))
            .expect("dialog host should render");

        let buffer = terminal.backend().buffer();
        let bottom_line = (0..20)
            .map(|x| buffer.cell((x, 5)).unwrap().symbol())
            .collect::<String>();
        assert!(!bottom_line.contains('┴'), "{bottom_line}");
    }

    #[test]
    fn close_key_emits_close_message_and_stops_propagation() {
        let mut dialog = Dialog::new().on_close(|reason| reason);
        let mut ctx = EventCtx::new(animation_settings());

        let outcome = dialog.event(&TuiEvent::Key(Key::Char('x').into()), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.messages(), &[DialogCloseReason::CloseKey]);
        assert_eq!(ctx.propagation(), crate::Propagation::Stopped);
    }

    #[test]
    fn custom_close_key_replaces_default_close_key() {
        let mut dialog = Dialog::new()
            .keybindings(DialogKeyBindings {
                close: vec![KeySpec::plain('q')],
            })
            .on_close(|reason| reason);
        let mut ctx = EventCtx::new(animation_settings());

        assert_eq!(
            dialog.event(&TuiEvent::Key(Key::Char('x').into()), &mut ctx),
            EventOutcome::Ignored
        );
        assert_eq!(
            dialog.event(&TuiEvent::Key(Key::Char('q').into()), &mut ctx),
            EventOutcome::Handled
        );
        assert_eq!(ctx.messages(), &[DialogCloseReason::CloseKey]);
    }

    #[test]
    fn unfocus_key_closes_dialog_when_dialog_shell_is_focused() {
        let mut dialog = Dialog::new().on_close(|reason| reason);
        let mut ctx = EventCtx::new(animation_settings());

        let outcome = dialog.event(&TuiEvent::Key(Key::Esc.into()), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.messages(), &[DialogCloseReason::Escape]);
        assert_eq!(ctx.propagation(), crate::Propagation::Stopped);
    }

    #[test]
    fn focused_text_input_receives_x_before_dialog_close_policy() {
        let mut host = Dialog::new()
            .on_close(|reason| reason)
            .host(TextInput::<DialogCloseReason>::new());
        let mut layout = LayoutCtx::new();
        let area = Rect::new(0, 0, 24, 5);
        host.layout(area, &mut layout);
        let route = EventRoute::new(layout.focus_targets()[0].path.clone());
        let mut ctx = EventCtx::new(animation_settings());

        let outcome = host.dispatch_event(&route, &TuiEvent::Key(Key::Char('x').into()), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(host.child().current_value(), "x");
        assert!(ctx.messages().is_empty());
        assert_eq!(ctx.propagation(), crate::Propagation::Stopped);
    }

    #[test]
    fn escape_bubbles_from_child_and_closes_dialog() {
        let mut host = Dialog::new()
            .on_close(|reason| reason)
            .host(TextInput::<DialogCloseReason>::new());
        let mut layout = LayoutCtx::new();
        let area = Rect::new(0, 0, 24, 5);
        host.layout(area, &mut layout);
        let route = EventRoute::new(layout.focus_targets()[0].path.clone());
        let mut ctx = EventCtx::new(animation_settings());

        let outcome = host.dispatch_event(&route, &TuiEvent::Key(Key::Esc.into()), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.messages(), &[DialogCloseReason::Escape]);
        assert_eq!(ctx.propagation(), crate::Propagation::Stopped);
    }

    #[test]
    fn dialog_host_registers_single_fallback_focus_when_child_has_none() {
        let mut host = Dialog::<()>::new().host(StaticBody);
        let mut layout = LayoutCtx::new();

        host.layout(Rect::new(0, 0, 24, 5), &mut layout);

        assert_eq!(layout.focus_targets().len(), 1);
        assert_eq!(layout.focus_targets()[0].id.as_str(), "dialog");
        assert!(layout.focus_targets()[0].path.is_empty());
    }
}
