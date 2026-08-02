use std::time::Duration;

use ratatui::{Frame, layout::Rect};
use tuicore::{
    AnimationSettings, Dropdown, EventCtx, EventOutcome, EventRoute, FocusCtx, FocusId,
    FocusTarget, LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint, LifecycleCtx, MenuButton,
    MenuItem, Panel, PanelHost, Paragraph, RenderCtx, Split, TickResult, TuiEvent, TuiNode,
};

#[derive(Clone)]
struct Environment {
    id: &'static str,
    label: &'static str,
}

type DropdownView = PanelHost<
    Split<Dropdown<Environment, &'static str>, Split<MenuButton<&'static str>, Paragraph>>,
>;

struct DropdownExample {
    view: DropdownView,
    selected: Option<&'static str>,
}

impl DropdownExample {
    fn new() -> Self {
        let environments = [
            Environment {
                id: "dev",
                label: "Development",
            },
            Environment {
                id: "stage",
                label: "Staging",
            },
            Environment {
                id: "prod",
                label: "Production",
            },
        ];
        let dropdown = Dropdown::single(
            environments,
            |environment| environment.id,
            |environment| environment.label.to_string(),
        )
        .label("Environment")
        .placeholder("Choose environment")
        .selected_one("stage");
        let commands = MenuButton::new(
            "Commands",
            [
                MenuItem::new("deploy", "Deploy selected environment"),
                MenuItem::new("logs", "Open logs"),
                MenuItem::new("cancel", "Cancel deployment"),
            ],
        )
        .hotkey("m")
        .visible_items(5);
        let controls = Split::vertical(
            dropdown,
            Split::vertical(
                commands,
                Paragraph::new("Selected: stage • Menu: no command yet"),
            )
            .ratio(1, 2)
            .gap(1),
        )
        .ratio(1, 2)
        .gap(1);
        let view = Panel::new()
            .top_left("Dropdown and command menu")
            .bottom_left("Enter opens • popup owns focus • Esc returns focus • Ctrl+Q quits")
            .host(controls);

        Self {
            view,
            selected: Some("stage"),
        }
    }

    fn consume_outputs(&mut self, ctx: &mut EventCtx<()>) {
        let selected = self.view.child().first().selected_id();
        let commands = self
            .view
            .child_mut()
            .second_mut()
            .first_mut()
            .take_activated();
        if selected == self.selected && commands.is_empty() {
            return;
        }

        self.selected = selected;
        let selection = selected.unwrap_or("none");
        let command = commands.last().copied().unwrap_or("no command yet");
        self.view
            .child_mut()
            .second_mut()
            .second_mut()
            .set_text(format!("Selected: {selection} • Menu: {command}"));
        ctx.request_redraw();
    }
}

impl TuiNode for DropdownExample {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        self.view.measure(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.view.layout(area, ctx)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        self.view.render(frame, area, ctx);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<()>) -> EventOutcome {
        let outcome = self.view.event(event, ctx);
        self.consume_outputs(ctx);
        outcome
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<()>,
    ) -> EventOutcome {
        let outcome = self.view.dispatch_event(route, event, ctx);
        self.consume_outputs(ctx);
        outcome
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.view.tick(dt, settings)
    }

    fn focus(&mut self, target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<()>) {
        self.view.focus(target, focused, ctx);
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<()>) {
        self.view.dispatch_focus(target, focused, ctx);
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<()>) {
        self.view.init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<()>) {
        self.view.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<()>) {
        self.view.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<()>) {
        self.view.destroy(ctx);
    }
}

fn main() -> tuicore::Result<()> {
    tuicore::init();
    tuicore::TreeApp::new(DropdownExample::new()).run()
}
