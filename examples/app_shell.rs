use std::time::Duration;

use ratatui::{Frame, layout::Rect};
use tuicore::{
    ActivationMode, AnimationSettings, DataView, DataViewTypedEvent, EventCtx, EventOutcome,
    EventRoute, Flex, FlexItem, FocusCtx, FocusId, FocusTarget, LayoutCtx, LayoutProposal,
    LayoutResult, LayoutSizeHint, LifecycleCtx, Panel, PanelHost, Paragraph, RenderCtx,
    SelectionMode, SelectionTrigger, Split, StatusBar, Tab, Tabs, TickResult, TuiEvent, TuiNode,
};

#[derive(Clone)]
struct Service {
    id: u8,
    name: &'static str,
    status: &'static str,
    version: &'static str,
}

type ServicesView = Split<PanelHost<DataView<Service, u8>>, PanelHost<Paragraph>>;

struct MasterDetail {
    services: Vec<Service>,
    view: ServicesView,
}

impl MasterDetail {
    fn new(services: Vec<Service>) -> Self {
        let detail = service_detail(services.first());
        let service_list = DataView::list(
            services.clone(),
            |service| service.id,
            |service| service.name.to_string(),
        )
        .action_bar(true)
        .activation_mode(ActivationMode::OnNavigate)
        .selection_mode(SelectionMode::Single)
        .selection_trigger(SelectionTrigger::OnNavigate)
        .selected([1]);
        let view = Split::horizontal(
            Panel::new().top_left("Services").host(service_list),
            Panel::new()
                .top_left("Details")
                .host(Paragraph::new(detail)),
        )
        .ratio(2, 3);

        Self { services, view }
    }

    fn drain_data_view_events(&mut self, ctx: &mut EventCtx<()>) {
        for event in self.view.first_mut().child_mut().drain_events() {
            if let DataViewTypedEvent::HighlightChanged { row_id } = event {
                let service = row_id.and_then(|id| self.services.iter().find(|item| item.id == id));
                self.view
                    .second_mut()
                    .child_mut()
                    .set_text(service_detail(service));
                ctx.request_redraw();
            }
        }
    }
}

impl TuiNode for MasterDetail {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        <ServicesView as TuiNode>::measure(&self.view, proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        <ServicesView as TuiNode>::layout(&mut self.view, area, ctx)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        <ServicesView as TuiNode>::render(&self.view, frame, area, ctx);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<()>) -> EventOutcome {
        let outcome = self.view.event(event, ctx);
        self.drain_data_view_events(ctx);
        outcome
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<()>,
    ) -> EventOutcome {
        let outcome = self.view.dispatch_event(route, event, ctx);
        self.drain_data_view_events(ctx);
        outcome
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        <ServicesView as TuiNode>::tick(&mut self.view, dt, settings)
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

fn service_detail(service: Option<&Service>) -> String {
    service.map_or_else(
        || "No service selected".to_string(),
        |service| {
            format!(
                "{}\n\nStatus: {}\nVersion: {}\n\nUse ↑/↓ to browse services and / to search.",
                service.name, service.status, service.version
            )
        },
    )
}

fn main() -> tuicore::Result<()> {
    tuicore::init();

    let services = [
        Service {
            id: 1,
            name: "API server",
            status: "healthy",
            version: "0.12.0",
        },
        Service {
            id: 2,
            name: "Worker",
            status: "processing jobs",
            version: "0.11.4",
        },
        Service {
            id: 3,
            name: "Scheduler",
            status: "healthy",
            version: "0.12.0",
        },
    ];
    let services_page = MasterDetail::new(services.to_vec());

    let tabs = Tabs::new(vec![
        Tab::<()>::new("Services", services_page).hotkey("s"),
        Tab::text(
            "About",
            "Hello from tuicore.\n\nTab moves focus, and Ctrl+Q exits.",
        )
        .hotkey("a"),
    ]);
    let footer = StatusBar::new();
    let app = Flex::column().child("main", tabs, FlexItem::fill(1)).child(
        "footer",
        footer,
        FlexItem::fixed(1),
    );

    tuicore::TreeApp::new(app).run()
}
