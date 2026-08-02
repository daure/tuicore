use std::time::Duration;

use ratatui::{
    Frame,
    layout::{Constraint, Rect},
};
use tuicore::{
    AnimationSettings, Column, DataView, DataViewTypedEvent, EventCtx, EventOutcome, EventRoute,
    FocusCtx, FocusId, FocusTarget, LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint,
    LifecycleCtx, Panel, PanelHost, RenderCtx, SearchMode, SelectionMode, SelectionTrigger,
    SortDirection, TickResult, TuiEvent, TuiNode,
};

#[derive(Clone)]
struct Task {
    id: u16,
    title: &'static str,
    owner: &'static str,
    status: &'static str,
}

type TaskView = PanelHost<DataView<Task, u16>>;

struct TaskTable {
    view: TaskView,
}

impl TaskTable {
    fn new() -> Self {
        let tasks = vec![
            Task {
                id: 104,
                title: "Ship calendar example",
                owner: "Ada",
                status: "Ready",
            },
            Task {
                id: 101,
                title: "Review component API",
                owner: "Lin",
                status: "Active",
            },
            Task {
                id: 108,
                title: "Write release notes",
                owner: "Mia",
                status: "Blocked",
            },
            Task {
                id: 103,
                title: "Verify terminal themes",
                owner: "Noor",
                status: "Ready",
            },
        ];
        let columns = vec![
            Column::text("id", "ID", Constraint::Length(8), |task: &Task| {
                format!("TC-{}", task.id)
            })
            .sortable(|task| task.id),
            Column::text("title", "Task", Constraint::Fill(1), |task: &Task| {
                task.title.to_string()
            })
            .sortable(|task| task.title.to_string())
            .search_key(|task| task.title.to_string()),
            Column::text("owner", "Owner", Constraint::Length(12), |task: &Task| {
                task.owner.to_string()
            })
            .sortable(|task| task.owner.to_string())
            .search_key(|task| task.owner.to_string()),
            Column::text("status", "Status", Constraint::Length(12), |task: &Task| {
                task.status.to_string()
            })
            .sortable(|task| task.status.to_string())
            .search_key(|task| task.status.to_string()),
        ];
        let table = DataView::new(tasks, |task| task.id)
            .columns(columns)
            .headers(true)
            .action_bar(true)
            .search_mode(SearchMode::Fuzzy)
            .sorted_by("id", SortDirection::Ascending)
            .selection_mode(SelectionMode::Single)
            .selection_trigger(SelectionTrigger::OnActivate);
        let view = Panel::new()
            .top_left("Tasks")
            .top_right("/ search · Enter select")
            .host(table);
        Self { view }
    }

    fn drain_events(&mut self, ctx: &mut EventCtx<()>) {
        for event in self.view.child_mut().drain_events() {
            let status = match event {
                DataViewTypedEvent::Activated { row_id } => format!("activated TC-{row_id}"),
                DataViewTypedEvent::SelectionChanged { selected, .. } => {
                    format!("selected {selected:?}")
                }
                DataViewTypedEvent::TransformChanged { state } => {
                    format!(
                        "search {:?} · filters {}",
                        state.search,
                        state.filters.len()
                    )
                }
                DataViewTypedEvent::HighlightChanged { row_id } => format!("row {row_id:?}"),
            };
            self.view.panel_mut().set_top_right(status);
            ctx.request_redraw();
        }
    }
}

impl TuiNode for TaskTable {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        <TaskView as TuiNode>::measure(&self.view, proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        <TaskView as TuiNode>::layout(&mut self.view, area, ctx)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        <TaskView as TuiNode>::render(&self.view, frame, area, ctx);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<()>) -> EventOutcome {
        let outcome = self.view.event(event, ctx);
        self.drain_events(ctx);
        outcome
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<()>,
    ) -> EventOutcome {
        let outcome = self.view.dispatch_event(route, event, ctx);
        self.drain_events(ctx);
        outcome
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        <TaskView as TuiNode>::tick(&mut self.view, dt, settings)
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
    tuicore::TreeApp::new(TaskTable::new()).run()
}
