use std::marker::PhantomData;
use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::{
    AnimationSettings, AxisProposal, ChildKey, EventCtx, EventOutcome, EventRoute, FocusCtx,
    FocusTarget, LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint, LifecycleCtx, RenderCtx,
    TickResult, TuiEvent, TuiNode, theme,
};

use super::{Panel, PanelTone};

pub struct FormField<C, M = ()> {
    panel: Panel,
    child: C,
    error: Option<String>,
    panel_area: Rect,
    child_area: Rect,
    feedback_area: Rect,
    embedded: bool,
    message: PhantomData<fn() -> M>,
}

impl<C, M> FormField<C, M> {
    pub fn new(label: impl Into<String>, child: C) -> Self {
        Self {
            panel: Panel::new().top_left(label),
            child,
            error: None,
            panel_area: Rect::default(),
            child_area: Rect::default(),
            feedback_area: Rect::default(),
            embedded: false,
            message: PhantomData,
        }
    }

    pub fn child(&self) -> &C {
        &self.child
    }

    pub fn embedded(mut self) -> Self {
        self.embedded = true;
        self
    }

    pub fn child_mut(&mut self) -> &mut C {
        &mut self.child
    }

    pub fn set_error(&mut self, error: Option<String>) {
        self.panel.set_tone(if error.is_some() {
            PanelTone::Error
        } else {
            PanelTone::Normal
        });
        self.error = error;
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

impl<C, M> TuiNode<M> for FormField<C, M>
where
    C: TuiNode<M>,
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let feedback_height = u16::from(self.error.is_some());
        let chrome = if self.embedded { 0 } else { 2 };
        let child_proposal = LayoutProposal {
            width: subtract_proposal(proposal.width, chrome),
            height: subtract_proposal(proposal.height, chrome + feedback_height),
        };
        let child = self.child.measure(child_proposal);
        LayoutSizeHint::content(
            child.preferred.width.saturating_add(chrome),
            child
                .preferred
                .height
                .saturating_add(chrome + feedback_height),
        )
        .normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        let feedback_height = u16::from(self.error.is_some() && area.height > 0);
        self.panel_area = Rect::new(
            area.x,
            area.y,
            area.width,
            area.height.saturating_sub(feedback_height),
        );
        self.feedback_area = Rect::new(
            area.x,
            area.y.saturating_add(self.panel_area.height),
            area.width,
            feedback_height,
        );
        self.child_area = if self.embedded {
            self.panel_area
        } else {
            Panel::inner_area(self.panel_area)
        };
        ctx.push_slot(ChildKey::body(), self.child_area, |ctx| {
            self.child.layout(self.child_area, ctx);
        });
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut Frame, _area: Rect, ctx: &mut RenderCtx<'a>) {
        if !self.embedded {
            self.panel.render(frame, self.panel_area);
        }
        if !self.child_area.is_empty() {
            self.child.render(frame, self.child_area, ctx);
        }
        if let Some(error) = &self.error
            && !self.feedback_area.is_empty()
        {
            frame.render_widget(
                Paragraph::new(Line::styled(
                    error.as_str(),
                    Style::default().fg(theme().error_fg()),
                )),
                self.feedback_area,
            );
        }
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        self.child.event(event, ctx)
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
        route
            .path
            .without_first_if(&body)
            .map(EventRoute::new)
            .map(|route| self.child.dispatch_event(&route, event, ctx))
            .unwrap_or(EventOutcome::Ignored)
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        crate::Animated::tick(&mut self.panel, dt, settings).merge(self.child.tick(dt, settings))
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        let body = ChildKey::body();
        if let Some(child_target) = target.for_child(&body) {
            if !self.embedded {
                self.panel.set_focused(focused, ctx.animation());
            }
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

fn subtract_proposal(proposal: AxisProposal, amount: u16) -> AxisProposal {
    match proposal {
        AxisProposal::Unbounded => AxisProposal::Unbounded,
        AxisProposal::AtMost(value) => AxisProposal::AtMost(value.saturating_sub(amount)),
        AxisProposal::Exact(value) => AxisProposal::Exact(value.saturating_sub(amount)),
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::widgets::Paragraph;

    use super::*;
    use crate::{FocusId, Key, KeyEvent, LayoutSize, TreePath};

    #[derive(Default)]
    struct Probe {
        area: Rect,
        events: usize,
        ticks: usize,
        focused: bool,
        lifecycle: Vec<&'static str>,
    }

    impl TuiNode<()> for Probe {
        fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
            LayoutSizeHint::content(10, 2).normalized(proposal)
        }

        fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
            self.area = area;
            ctx.register_focusable(FocusId::new("probe"), area, true);
            LayoutResult::new(area)
        }

        fn render(&self, frame: &mut Frame, area: Rect, _ctx: &mut RenderCtx<'_>) {
            frame.render_widget(Paragraph::new("child"), area);
        }

        fn event(&mut self, _event: &TuiEvent, _ctx: &mut EventCtx<()>) -> EventOutcome {
            self.events += 1;
            EventOutcome::Handled
        }

        fn tick(&mut self, _dt: Duration, _settings: AnimationSettings) -> TickResult {
            self.ticks += 1;
            TickResult::CHANGED
        }

        fn focus(&mut self, _target: Option<&FocusId>, focused: bool, _ctx: &mut FocusCtx<()>) {
            self.focused = focused;
        }

        fn init(&mut self, _ctx: &mut LifecycleCtx<()>) {
            self.lifecycle.push("init");
        }

        fn mount(&mut self, _ctx: &mut LifecycleCtx<()>) {
            self.lifecycle.push("mount");
        }

        fn unmount(&mut self, _ctx: &mut LifecycleCtx<()>) {
            self.lifecycle.push("unmount");
        }

        fn destroy(&mut self, _ctx: &mut LifecycleCtx<()>) {
            self.lifecycle.push("destroy");
        }
    }

    #[test]
    fn measurement_consumes_feedback_height_only_while_error_is_visible() {
        let mut field = FormField::<_, ()>::new("Name", Probe::default());

        assert_eq!(
            field.measure(LayoutProposal::unbounded()).preferred,
            LayoutSize::new(12, 4)
        );

        field.set_error(Some("Required".into()));
        assert_eq!(
            field.measure(LayoutProposal::unbounded()).preferred,
            LayoutSize::new(12, 5)
        );

        field.set_error(None);
        assert_eq!(
            field.measure(LayoutProposal::unbounded()).preferred,
            LayoutSize::new(12, 4)
        );
    }

    #[test]
    fn embedded_field_skips_panel_and_only_adds_visible_feedback_row() {
        let mut field = FormField::<_, ()>::new("Ignored", Probe::default()).embedded();

        assert_eq!(
            field.measure(LayoutProposal::unbounded()).preferred,
            LayoutSize::new(10, 2)
        );
        field.set_error(Some("Required".into()));
        assert_eq!(
            field.measure(LayoutProposal::unbounded()).preferred,
            LayoutSize::new(10, 3)
        );
        field.layout(Rect::new(0, 0, 16, 3), &mut LayoutCtx::new());
        assert_eq!(field.child().area, Rect::new(0, 0, 16, 2));
    }

    #[test]
    fn routes_layout_event_focus_tick_and_lifecycle_through_one_child_slot() {
        let mut field = FormField::<_, ()>::new("Name", Probe::default());
        let mut layout = LayoutCtx::new();
        field.layout(Rect::new(2, 3, 14, 6), &mut layout);

        assert_eq!(field.child().area, Rect::new(3, 4, 12, 4));
        assert_eq!(layout.focus_targets().len(), 1);
        assert_eq!(
            layout.focus_targets()[0].path,
            TreePath::from_keys([ChildKey::body()])
        );

        let target = layout.focus_targets()[0].clone();
        let route = EventRoute::new(target.path.clone());
        assert_eq!(
            field.dispatch_event(
                &route,
                &TuiEvent::Key(KeyEvent::from(Key::Enter)),
                &mut EventCtx::default(),
            ),
            EventOutcome::Handled
        );
        field.dispatch_focus(&target, true, &mut FocusCtx::default());
        assert!(
            field
                .tick(Duration::from_millis(16), AnimationSettings::default())
                .active
        );

        let mut lifecycle = LifecycleCtx::default();
        field.init(&mut lifecycle);
        field.mount(&mut lifecycle);
        field.unmount(&mut lifecycle);
        field.destroy(&mut lifecycle);

        assert_eq!(field.child().events, 1);
        assert_eq!(field.child().ticks, 1);
        assert!(field.child().focused);
        assert_eq!(
            field.child().lifecycle,
            vec!["init", "mount", "unmount", "destroy"]
        );
    }

    #[test]
    fn error_colors_border_title_and_message_and_places_message_below_panel() {
        let mut field = FormField::<_, ()>::new("Name", Probe::default());
        field.set_error(Some("Required".into()));
        let mut layout = LayoutCtx::new();
        field.layout(Rect::new(0, 0, 16, 6), &mut layout);
        assert_eq!(field.child().area, Rect::new(1, 1, 14, 3));
        assert_eq!(
            field.measure(LayoutProposal::unbounded()).preferred,
            LayoutSize::new(12, 5)
        );
        let mut terminal = Terminal::new(TestBackend::new(16, 6)).expect("terminal should build");

        terminal
            .draw(|frame| field.render(frame, frame.area(), &mut RenderCtx::new()))
            .expect("field should render");

        let buffer = terminal.backend().buffer();
        let error = theme().error_fg();
        assert_eq!(buffer.cell((0, 0)).unwrap().fg, error);
        assert_eq!(buffer.cell((2, 0)).unwrap().fg, error);
        assert_eq!(buffer.cell((0, 5)).unwrap().symbol(), "R");
        assert_eq!(buffer.cell((0, 5)).unwrap().fg, error);
        assert_eq!(buffer.cell((0, 4)).unwrap().symbol(), "╰");
    }

    #[test]
    fn normal_field_uses_full_area_without_blank_feedback_reservation() {
        let mut field = FormField::<_, ()>::new("Name", Probe::default());
        field.layout(Rect::new(0, 0, 16, 6), &mut LayoutCtx::new());
        let mut terminal = Terminal::new(TestBackend::new(16, 6)).expect("terminal should build");

        terminal
            .draw(|frame| field.render(frame, frame.area(), &mut RenderCtx::new()))
            .expect("field should render");

        let buffer = terminal.backend().buffer();
        assert_eq!(buffer.cell((0, 0)).unwrap().fg, theme().border_fg());
        assert_eq!(buffer.cell((0, 5)).unwrap().symbol(), "╰");
        assert_eq!(buffer.cell((0, 5)).unwrap().fg, theme().border_fg());
        assert_eq!(field.child().area, Rect::new(1, 1, 14, 4));
    }

    #[test]
    fn focused_error_keeps_semantic_error_color() {
        let mut field = FormField::<_, ()>::new("Name", Probe::default());
        field.set_error(Some("Required".into()));
        let mut layout = LayoutCtx::new();
        field.layout(Rect::new(0, 0, 16, 6), &mut layout);
        field.dispatch_focus(
            &layout.focus_targets()[0].clone(),
            true,
            &mut FocusCtx::default(),
        );
        let mut terminal = Terminal::new(TestBackend::new(16, 6)).expect("terminal should build");

        terminal
            .draw(|frame| field.render(frame, frame.area(), &mut RenderCtx::new()))
            .expect("field should render");

        assert_eq!(
            terminal.backend().buffer().cell((0, 0)).unwrap().fg,
            theme().error_fg()
        );
    }

    #[test]
    fn tiny_areas_are_safe() {
        let mut field = FormField::<_, ()>::new("Name", Probe::default());
        field.set_error(Some("Required".into()));
        let mut terminal = Terminal::new(TestBackend::new(1, 1)).expect("terminal should build");

        for area in [Rect::default(), Rect::new(0, 0, 1, 1)] {
            field.layout(area, &mut LayoutCtx::new());
            terminal
                .draw(|frame| field.render(frame, area, &mut RenderCtx::new()))
                .expect("field should render");
        }
    }
}
