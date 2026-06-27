use std::rc::Rc;
use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Borders;

use crate::store::{DispatchOutcome, InspectField, InspectValue, StoreLogEntry, StoreLogPhase};
use crate::{
    AnimationSettings, Column, DataView, EventCtx, EventOutcome, EventRoute, FocusCtx, FocusTarget,
    LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint, LifecycleCtx, TabsVariant, TickResult,
    TuiNode,
};

use super::{ModalCloseReason, Tab, Tabs};

const MAX_STATE_DEPTH: usize = 6;
const MAX_STATE_ROWS: usize = 200;
const MAX_STRING_CHARS: usize = 80;
const MAX_EVENT_ROWS: usize = 200;

pub struct StoreDebugView<M = ()> {
    state: InspectValue,
    events: Vec<StoreLogEntry>,
    tabs: Tabs<M>,
    mode: StoreDebugViewMode,
    on_close: Option<CloseHandler<M>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StoreDebugViewMode {
    Inline,
    Dialog,
}

type CloseHandler<M> = Rc<dyn Fn(ModalCloseReason) -> M>;

impl<M> StoreDebugView<M>
where
    M: 'static,
{
    pub fn new(state: InspectValue, events: Vec<StoreLogEntry>) -> Self {
        Self::with_mode(state, events, StoreDebugViewMode::Inline)
    }

    pub fn dialog(state: InspectValue, events: Vec<StoreLogEntry>) -> Self {
        Self::with_mode(state, events, StoreDebugViewMode::Dialog)
    }

    fn with_mode(
        state: InspectValue,
        events: Vec<StoreLogEntry>,
        mode: StoreDebugViewMode,
    ) -> Self {
        let tabs = store_debug_tabs(&state, &events, 0, mode, None);
        Self {
            state,
            events,
            tabs,
            mode,
            on_close: None,
        }
    }

    pub fn empty() -> Self {
        Self::new(InspectValue::object([]), Vec::new())
    }

    pub fn set_snapshot(&mut self, state: InspectValue, events: Vec<StoreLogEntry>) {
        let selected = self.tabs.selected_index();
        self.state = state;
        self.events = events;
        self.rebuild_tabs(selected);
    }

    pub fn state(&self) -> &InspectValue {
        &self.state
    }

    pub fn events(&self) -> &[StoreLogEntry] {
        &self.events
    }

    pub fn modal(mut self) -> Self {
        self.mode = StoreDebugViewMode::Dialog;
        let selected = self.tabs.selected_index();
        self.rebuild_tabs(selected);
        self
    }

    pub fn on_close(mut self, handler: impl Fn(ModalCloseReason) -> M + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        let selected = self.tabs.selected_index();
        self.rebuild_tabs(selected);
        self
    }

    fn rebuild_tabs(&mut self, selected: usize) {
        self.tabs = store_debug_tabs(
            &self.state,
            &self.events,
            selected,
            self.mode,
            self.on_close.clone(),
        );
    }
}

impl<M> Default for StoreDebugView<M>
where
    M: 'static,
{
    fn default() -> Self {
        Self::empty()
    }
}

impl<M> TuiNode<M> for StoreDebugView<M>
where
    M: 'static,
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        self.tabs.measure(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.tabs.layout(area, ctx)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        self.tabs.render(frame, area);
    }

    fn render_overlay(&self, frame: &mut Frame, area: Rect) {
        self.tabs.render_overlay(frame, area);
    }

    fn event(&mut self, event: &crate::TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        self.tabs.event(event, ctx)
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &crate::TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        self.tabs.dispatch_event(route, event, ctx)
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.tabs.tick(dt, settings)
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        self.tabs.dispatch_focus(target, focused, ctx);
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.tabs.init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.tabs.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.tabs.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.tabs.destroy(ctx);
    }
}

fn store_debug_tabs<M>(
    state: &InspectValue,
    events: &[StoreLogEntry],
    selected: usize,
    mode: StoreDebugViewMode,
    on_close: Option<CloseHandler<M>>,
) -> Tabs<M>
where
    M: 'static,
{
    let tabs = vec![
        Tab::text("State", format_state_snapshot(state)).hotkey("s"),
        Tab::new("Events", event_data_view(events)).hotkey("e"),
    ];

    let tabs = match mode {
        StoreDebugViewMode::Inline => Tabs::new(tabs)
            .variant(TabsVariant::Boxed)
            .edge_borders(Borders::ALL),
        StoreDebugViewMode::Dialog => Tabs::dialog(tabs),
    }
    .selected(selected);

    match on_close {
        Some(handler) => tabs.on_close(move |reason| handler(reason)),
        None => tabs,
    }
}

fn format_state_snapshot(state: &InspectValue) -> String {
    let mut lines = Vec::new();
    push_value_lines(None, state, 0, &mut lines);
    if lines.is_empty() {
        lines.push("<empty state>".to_string());
    }
    lines.join("\n")
}

fn push_value_lines(
    name: Option<&str>,
    value: &InspectValue,
    depth: usize,
    lines: &mut Vec<String>,
) {
    if lines.len() >= MAX_STATE_ROWS {
        return;
    }

    let indent = "  ".repeat(depth);
    match value {
        InspectValue::Object(fields) => {
            if depth >= MAX_STATE_DEPTH {
                push_line(lines, format!("{indent}{}{{…}}", name_prefix(name)));
                return;
            }
            if fields.is_empty() {
                push_line(lines, format!("{indent}{}{{}}", name_prefix(name)));
                return;
            }
            if let Some(name) = name {
                push_line(lines, format!("{indent}{name}:"));
            }
            for field in fields {
                push_field_lines(field, depth + usize::from(name.is_some()), lines);
            }
        }
        InspectValue::List(values) => {
            if depth >= MAX_STATE_DEPTH {
                push_line(lines, format!("{indent}{}[…]", name_prefix(name)));
                return;
            }
            if values.is_empty() {
                push_line(lines, format!("{indent}{}[]", name_prefix(name)));
                return;
            }
            if let Some(name) = name {
                push_line(lines, format!("{indent}{name}:"));
            }
            for (index, value) in values.iter().enumerate() {
                let child_name = format!("[{index}]");
                push_value_lines(
                    Some(&child_name),
                    value,
                    depth + usize::from(name.is_some()),
                    lines,
                );
            }
        }
        _ => push_line(
            lines,
            format!("{indent}{}{}", name_prefix(name), primitive_value(value)),
        ),
    }
}

fn push_field_lines(field: &InspectField, depth: usize, lines: &mut Vec<String>) {
    push_value_lines(Some(&field.name), &field.value, depth, lines);
}

fn push_line(lines: &mut Vec<String>, line: String) {
    if lines.len() + 1 < MAX_STATE_ROWS {
        lines.push(line);
    } else if lines.len() < MAX_STATE_ROWS {
        lines.push("…".to_string());
    }
}

fn name_prefix(name: Option<&str>) -> String {
    name.map(|name| format!("{name}: ")).unwrap_or_default()
}

fn primitive_value(value: &InspectValue) -> String {
    match value {
        InspectValue::String(value) => format!("\"{}\"", truncate_string(value)),
        InspectValue::Number(value) => value.clone(),
        InspectValue::Bool(value) => value.to_string(),
        InspectValue::Null => "null".to_string(),
        InspectValue::Object(_) | InspectValue::List(_) => String::new(),
    }
}

fn truncate_string(value: &str) -> String {
    let mut output = String::new();
    for (index, ch) in value.replace('\n', "⏎").chars().enumerate() {
        if index >= MAX_STRING_CHARS {
            output.push('…');
            break;
        }
        output.push(ch);
    }
    output
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StoreEventRow {
    id: usize,
    sequence: u64,
    phase: StoreLogPhase,
    event_label: String,
    outcome: Option<DispatchOutcome>,
}

fn event_data_view(events: &[StoreLogEntry]) -> DataView<StoreEventRow, usize> {
    DataView::new(event_rows(events), |row: &StoreEventRow| row.id)
        .headers(true)
        .columns(event_columns())
}

fn event_rows(events: &[StoreLogEntry]) -> Vec<StoreEventRow> {
    events
        .iter()
        .take(MAX_EVENT_ROWS)
        .enumerate()
        .map(|(id, entry)| StoreEventRow {
            id,
            sequence: entry.sequence,
            phase: entry.phase,
            event_label: truncate_event_label(&entry.event_label),
            outcome: entry.outcome,
        })
        .collect()
}

fn event_columns() -> Vec<Column<StoreEventRow, usize>> {
    vec![
        Column::text(
            "seq",
            "Seq",
            Constraint::Length(6),
            |row: &StoreEventRow| row.sequence.to_string(),
        ),
        Column::text(
            "phase",
            "Phase",
            Constraint::Length(10),
            |row: &StoreEventRow| match row.phase {
                StoreLogPhase::Received => "Received".to_string(),
                StoreLogPhase::Handled => "Handled".to_string(),
            },
        ),
        Column::text(
            "event",
            "Event",
            Constraint::Percentage(100),
            |row: &StoreEventRow| row.event_label.clone(),
        ),
        Column::rich(
            "changed",
            "Changed",
            Constraint::Length(9),
            |row: &StoreEventRow, _| flag_line(row.outcome.map(|outcome| outcome.changed)),
        ),
        Column::rich(
            "redraw",
            "Redraw",
            Constraint::Length(8),
            |row: &StoreEventRow, _| flag_line(row.outcome.map(|outcome| outcome.redraw)),
        ),
        Column::rich(
            "layout",
            "Layout",
            Constraint::Length(8),
            |row: &StoreEventRow, _| flag_line(row.outcome.map(|outcome| outcome.layout)),
        ),
    ]
}

fn flag_line(value: Option<bool>) -> Line<'static> {
    let theme = crate::theme();
    match value {
        Some(true) => Line::from(Span::styled(
            "yes",
            Style::default()
                .fg(theme.success_fg())
                .add_modifier(Modifier::BOLD),
        )),
        Some(false) => Line::from(Span::styled("no", Style::default().fg(theme.subtle_fg()))),
        None => Line::from(Span::styled("—", Style::default().fg(theme.muted_fg()))),
    }
}

fn truncate_event_label(value: &str) -> String {
    let max = 32;
    let mut output = String::new();
    for (index, ch) in value.chars().enumerate() {
        if index >= max {
            output.push('…');
            break;
        }
        output.push(ch);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{DispatchOutcome, InspectField};
    use crate::{Key, KeyEvent, TuiEvent};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn state_snapshot_renders_nested_tree() {
        let state = InspectValue::object([
            InspectField::new("title", InspectValue::string("Gallery")),
            InspectField::new(
                "items",
                InspectValue::list([InspectValue::bool(true), InspectValue::null()]),
            ),
        ]);

        let text = format_state_snapshot(&state);

        assert!(text.contains("title: \"Gallery\""));
        assert!(text.contains("items:"));
        assert!(text.contains("[0]: true"));
    }

    #[test]
    fn event_rows_keep_composite_ids_and_outcome_flags() {
        let rows = event_rows(&[
            StoreLogEntry {
                sequence: 7,
                event_label: "SelectComponent".to_string(),
                phase: StoreLogPhase::Received,
                outcome: None,
            },
            StoreLogEntry {
                sequence: 7,
                event_label: "SelectComponent".to_string(),
                phase: StoreLogPhase::Handled,
                outcome: Some(DispatchOutcome::layout()),
            },
        ]);

        assert_eq!(rows[0].id, 0);
        assert_eq!(rows[1].id, 1);
        assert_eq!(rows[0].sequence, 7);
        assert_eq!(rows[1].sequence, 7);
        assert_eq!(rows[1].outcome, Some(DispatchOutcome::layout()));
    }

    #[test]
    fn event_view_renders_headers_and_flags() {
        let view = event_data_view(&[StoreLogEntry {
            sequence: 7,
            event_label: "SelectComponent".to_string(),
            phase: StoreLogPhase::Handled,
            outcome: Some(DispatchOutcome::layout()),
        }]);
        let mut terminal = Terminal::new(TestBackend::new(80, 3)).expect("terminal should build");

        terminal
            .draw(|frame| view.render(frame, frame.area()))
            .expect("event view should render");

        let buffer = terminal.backend().buffer();
        let rendered = (0..3)
            .map(|y| {
                (0..80)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("Seq"));
        assert!(rendered.contains("Phase"));
        assert!(rendered.contains("Event"));
        assert!(rendered.contains("Changed"));
        assert!(rendered.contains("Redraw"));
        assert!(rendered.contains("Layout"));
        assert!(rendered.contains("7"));
        assert!(rendered.contains("Handled"));
        assert!(rendered.contains("SelectComponent"));
        assert_eq!(rendered.matches("yes").count(), 3);
    }

    #[test]
    fn event_view_renders_header_when_empty() {
        let view = event_data_view(&[]);
        let mut terminal = Terminal::new(TestBackend::new(80, 2)).expect("terminal should build");

        terminal
            .draw(|frame| view.render(frame, frame.area()))
            .expect("event view should render");

        let buffer = terminal.backend().buffer();
        let header = (0..80)
            .map(|x| buffer.cell((x, 0)).unwrap().symbol())
            .collect::<String>();

        assert!(header.contains("Seq"));
        assert!(header.contains("Phase"));
        assert!(header.contains("Event"));
    }

    #[test]
    fn set_snapshot_preserves_dialog_selection_and_close_handler() {
        let mut view =
            StoreDebugView::dialog(InspectValue::object([]), Vec::new()).on_close(|reason| reason);
        view.tabs.select_index(1);

        view.set_snapshot(
            InspectValue::object([InspectField::new("loaded", InspectValue::bool(true))]),
            vec![StoreLogEntry {
                sequence: 1,
                event_label: "Load".to_string(),
                phase: StoreLogPhase::Handled,
                outcome: Some(DispatchOutcome::changed()),
            }],
        );

        assert_eq!(view.tabs.selected_index(), 1);

        let mut ctx = EventCtx::default();
        let outcome = view.event(&TuiEvent::Key(KeyEvent::from(Key::Esc)), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.messages(), &[ModalCloseReason::Escape]);
    }
}
