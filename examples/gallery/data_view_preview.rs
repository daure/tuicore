use tuicore::{
    ActivationMode, Animated, Column, DataView, DataViewTypedEvent, Panel, SelectionGlyphs,
    SelectionMode, SelectionPropagation, SelectionTrigger, TreeAdapter, TreeGlyphs,
};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, NoUserEvent};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::ratatui::Frame;
use tuirealm::ratatui::layout::{Constraint, Direction, Layout, Rect};
use tuirealm::ratatui::style::{Modifier, Style};
use tuirealm::ratatui::text::{Line, Span};
use tuirealm::ratatui::widgets::Paragraph;
use tuirealm::state::State;

use crate::shared::{Msg, focus_list_key};

pub struct DataViewPreview {
    mode: DataViewMode,
    panel: Panel,
    data_view: DataView<DemoRow, usize>,
    area: Rect,
    focused: bool,
    status: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataViewMode {
    List,
    Table,
    ListTree,
    TableTree,
    SingleSelect,
    MultiSelect,
    ChecklistTree,
    ActivateOnNavigate,
}

impl DataViewPreview {
    pub fn new(mode: DataViewMode) -> Self {
        Self {
            mode,
            panel: Panel::new().top_left(mode.title()),
            data_view: mode.data_view(),
            area: Rect::default(),
            focused: false,
            status: String::from("No event yet"),
        }
    }

    fn layout(area: Rect) -> [Rect; 2] {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Fill(1)])
            .areas(area)
    }

    fn record_events(&mut self) {
        let statuses = self
            .data_view
            .take_events()
            .into_iter()
            .map(Self::event_status)
            .collect::<Vec<_>>();
        if !statuses.is_empty() {
            self.status = statuses.join(" • ");
        }
    }

    fn event_status(event: DataViewTypedEvent<usize>) -> String {
        match event {
            DataViewTypedEvent::HighlightChanged { row_id } => format!("highlight → {:?}", row_id),
            DataViewTypedEvent::Activated { row_id } => format!("activated #{row_id}"),
            DataViewTypedEvent::SelectionChanged { selected, .. } => {
                format!("selected {:?}", selected)
            }
        }
    }
}

impl DataViewMode {
    fn title(self) -> &'static str {
        match self {
            Self::List => "DataView: list",
            Self::Table => "DataView: table",
            Self::ListTree => "DataView: list tree",
            Self::TableTree => "DataView: table tree",
            Self::SingleSelect => "DataView: single select",
            Self::MultiSelect => "DataView: multi select",
            Self::ChecklistTree => "DataView: tree checklist",
            Self::ActivateOnNavigate => "DataView: activate on navigate",
        }
    }

    fn help(self) -> String {
        let bindings = tuicore::keybindings();
        let data_keys = bindings.data_view();
        let scroll_keys = format!(
            "{}/{}",
            bindings.line_up_label(),
            bindings.line_down_label()
        );
        let all_tree_keys = format!(
            "{}/{}",
            data_keys.collapse_all_label(),
            data_keys.expand_all_label()
        );
        match self {
            Self::List => format!(
                "100 rows • one column • no header • {scroll_keys} scroll • {} activates row",
                data_keys.activate_label()
            ),
            Self::Table => {
                format!(
                    "100 rows • headers + rich cells • {scroll_keys} scroll • s sorts task column"
                )
            }
            Self::ListTree => format!(
                "100 rows • {} node • {all_tree_keys} collapse/expand all • using tree glyphs /",
                data_keys.toggle_expansion_label()
            ),
            Self::TableTree => format!(
                "100 rows • rich cells • {} node • {all_tree_keys} all • s sorts • using tree glyphs /",
                data_keys.toggle_expansion_label()
            ),
            Self::SingleSelect => format!(
                "{} toggles row • {} selects + activates • single selected ID",
                data_keys.toggle_selection_label(),
                data_keys.activate_label()
            ),
            Self::MultiSelect => format!(
                "{} or {} toggles rows • selected IDs stay in source order",
                data_keys.activate_label(),
                data_keys.toggle_selection_label()
            ),
            Self::ChecklistTree => format!(
                "{} or {} cascades descendants • Nerd Font mixed icon",
                data_keys.activate_label(),
                data_keys.toggle_selection_label()
            ),
            Self::ActivateOnNavigate => format!(
                "{scroll_keys} changes active + selected row immediately • dropdown-style preview"
            ),
        }
    }

    fn data_view(self) -> DataView<DemoRow, usize> {
        let rows = demo_rows();
        let expanded = rows
            .iter()
            .filter(|row| row.parent.is_none() || (1..4).contains(&(row.id % 10)))
            .map(|row| row.id)
            .collect::<Vec<_>>();

        match self {
            Self::List => DataView::list(rows, |row| row.id, |row| row.name.clone()),
            Self::Table => DataView::new(rows, |row| row.id)
                .headers(true)
                .columns(demo_columns()),
            Self::ListTree => DataView::list(rows, |row| row.id, |row| row.name.clone())
                .tree(TreeAdapter::parent_id(|row: &DemoRow| row.parent))
                .tree_glyphs(TreeGlyphs::NERD_FONT)
                .expanded(expanded),
            Self::TableTree => DataView::new(rows, |row| row.id)
                .headers(true)
                .columns(demo_columns())
                .tree(TreeAdapter::parent_id(|row: &DemoRow| row.parent))
                .tree_glyphs(TreeGlyphs::NERD_FONT)
                .expanded(expanded),
            Self::SingleSelect => DataView::list(rows, |row| row.id, |row| row.name.clone())
                .selection_mode(SelectionMode::Single)
                .selection_trigger(SelectionTrigger::OnActivate),
            Self::MultiSelect => DataView::new(rows, |row| row.id)
                .headers(true)
                .columns(demo_columns())
                .selection_mode(SelectionMode::Multi)
                .selection_trigger(SelectionTrigger::OnActivate),
            Self::ChecklistTree => DataView::list(rows, |row| row.id, |row| row.name.clone())
                .tree(TreeAdapter::parent_id(|row: &DemoRow| row.parent))
                .tree_glyphs(TreeGlyphs::NERD_FONT)
                .selection_mode(SelectionMode::Multi)
                .selection_trigger(SelectionTrigger::OnActivate)
                .selection_propagation(SelectionPropagation::CascadeDescendants)
                .selection_glyphs(SelectionGlyphs::NERD_FONT)
                .expanded(expanded),
            Self::ActivateOnNavigate => DataView::list(rows, |row| row.id, |row| row.name.clone())
                .activation_mode(ActivationMode::OnNavigate)
                .selection_mode(SelectionMode::Single)
                .selection_trigger(SelectionTrigger::OnNavigate),
        }
    }
}

impl Component for DataViewPreview {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        self.panel.view(frame, area);
        let [help, body] = Self::layout(Panel::inner_area(area));
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw(self.mode.help()),
                Span::raw("\n"),
                Span::raw(self.status.clone()),
            ])),
            help,
        );
        self.data_view.view(frame, body);
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        match attr {
            Attribute::Focus => Some(QueryResult::Owned(AttrValue::Flag(self.focused))),
            _ => self.panel.query(attr),
        }
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        match (attr, value) {
            (Attribute::Focus, AttrValue::Flag(focused)) => {
                self.focused = focused;
                self.panel.attr(Attribute::Focus, AttrValue::Flag(focused));
                self.data_view
                    .attr(Attribute::Focus, AttrValue::Flag(focused));
            }
            (Attribute::Width, AttrValue::Size(width)) => self.area.width = width,
            (Attribute::Height, AttrValue::Size(height)) => self.area.height = height,
            _ => {}
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        CmdResult::Invalid(cmd)
    }
}

impl AppComponent<Msg, NoUserEvent> for DataViewPreview {
    fn on(&mut self, event: &Event<NoUserEvent>) -> Option<Msg> {
        match event {
            Event::Keyboard(KeyEvent { code: Key::Esc, .. })
            | Event::Keyboard(KeyEvent {
                code: Key::Char('q'),
                ..
            }) => Some(Msg::Quit),
            Event::Keyboard(key) if focus_list_key(*key) => Some(Msg::FocusList),
            Event::Keyboard(KeyEvent {
                code: Key::Char('s'),
                modifiers: KeyModifiers::NONE,
            }) if matches!(self.mode, DataViewMode::Table | DataViewMode::TableTree) => {
                self.data_view.toggle_sort("task");
                self.record_events();
                Some(Msg::Redraw)
            }
            Event::Keyboard(key) => {
                let [_, body] = Self::layout(Panel::inner_area(self.area));
                let outcome = self.data_view.on_key(*key, body);
                self.record_events();
                outcome.needs_redraw().then_some(Msg::Redraw)
            }
            Event::Tick => {
                let settings = tuicore::animation_settings();
                let tick = self
                    .panel
                    .tick(settings.frame_duration(), settings)
                    .merge(self.data_view.tick(settings.frame_duration(), settings));
                tick.changed.then_some(Msg::Redraw)
            }
            _ => Some(Msg::Redraw),
        }
    }
}

#[derive(Debug, Clone)]
struct DemoRow {
    id: usize,
    parent: Option<usize>,
    name: String,
    owner: &'static str,
    status: Status,
    progress: u8,
}

#[derive(Debug, Clone, Copy)]
enum Status {
    Ready,
    Active,
    Blocked,
}

fn demo_columns() -> Vec<Column<DemoRow, usize>> {
    vec![
        Column::text(
            "task",
            "Task",
            Constraint::Percentage(45),
            |row: &DemoRow| row.name.clone(),
        )
        .sortable(|row| row.name.clone()),
        Column::text(
            "owner",
            "Owner",
            Constraint::Percentage(20),
            |row: &DemoRow| row.owner.to_string(),
        )
        .sortable(|row| row.owner.to_string()),
        Column::rich(
            "status",
            "Status",
            Constraint::Percentage(20),
            |row: &DemoRow, _| {
                let theme = tuicore::theme();
                let (label, color) = match row.status {
                    Status::Ready => ("READY", theme.success_fg()),
                    Status::Active => ("ACTIVE", theme.accent_fg()),
                    Status::Blocked => ("BLOCKED", theme.error_fg()),
                };
                Line::from(Span::styled(
                    format!(" {label} "),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ))
            },
        ),
        Column::rich(
            "progress",
            "Progress",
            Constraint::Percentage(15),
            |row: &DemoRow, _| {
                let theme = tuicore::theme();
                let bars = (row.progress / 20) as usize;
                Line::from(vec![
                    Span::styled("█".repeat(bars), Style::default().fg(theme.accent_fg())),
                    Span::styled(
                        "░".repeat(5_usize.saturating_sub(bars)),
                        Style::default().fg(theme.subtle_fg()),
                    ),
                ])
            },
        ),
    ]
}

fn demo_rows() -> Vec<DemoRow> {
    let owners = ["Ada", "Lin", "Ken", "Mia", "Noor"];
    let mut rows = Vec::with_capacity(100);
    for group in 0..10 {
        let parent_id = group * 10;
        rows.push(DemoRow {
            id: parent_id,
            parent: None,
            name: format!("Module {:02}", group + 1),
            owner: "Core",
            status: status_for(group),
            progress: progress_for(group),
        });
        for section in 1..4 {
            let id = parent_id + section;
            rows.push(DemoRow {
                id,
                parent: Some(parent_id),
                name: format!("Module {:02} / section {:02}", group + 1, section),
                owner: owners[id % owners.len()],
                status: status_for(id),
                progress: progress_for(id),
            });
        }
        for task in 4..10 {
            let id = parent_id + task;
            let section_id = parent_id + 1 + ((task - 4) / 2);
            rows.push(DemoRow {
                id,
                parent: Some(section_id),
                name: format!("Module {:02} / task {:02}", group + 1, task - 3),
                owner: owners[id % owners.len()],
                status: status_for(id),
                progress: progress_for(id),
            });
        }
    }
    rows
}

fn status_for(index: usize) -> Status {
    match index % 5 {
        0 => Status::Ready,
        1 | 2 => Status::Active,
        _ => Status::Blocked,
    }
}

fn progress_for(index: usize) -> u8 {
    ((index * 17) % 101) as u8
}
