use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use tuicore::{
    ActivationMode, CellContext, Column, DataView, DataViewTypedEvent, SelectionGlyphs,
    SelectionMode, SelectionPropagation, SelectionTrigger, TreeAdapter, TreeGlyphs,
};

use crate::PreviewKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DataViewMode {
    List,
    Table,
    ListTree,
    TableTree,
    SingleSelect,
    MultiSelect,
    ChecklistTree,
    ActivateOnNavigate,
}

impl DataViewMode {
    pub(crate) fn from_preview(preview: PreviewKind) -> Self {
        match preview {
            PreviewKind::DataTable => Self::Table,
            PreviewKind::DataListTree => Self::ListTree,
            PreviewKind::DataTableTree => Self::TableTree,
            PreviewKind::DataSingleSelect => Self::SingleSelect,
            PreviewKind::DataMultiSelect => Self::MultiSelect,
            PreviewKind::DataChecklistTree => Self::ChecklistTree,
            PreviewKind::DataActivateOnNavigate => Self::ActivateOnNavigate,
            _ => Self::List,
        }
    }

    pub(crate) fn help(self) -> String {
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
                "100 rows • {} search • one column • no header • {scroll_keys} scroll • {} activates row",
                data_keys.search_label(),
                data_keys.activate_label()
            ),
            Self::Table => {
                format!(
                    "100 rows • {} search • {} filter header key • {scroll_keys} scroll • s sorts task column",
                    data_keys.search_label(),
                    data_keys.filter_label()
                )
            }
            Self::ListTree => format!(
                "100 rows • {} search • {} node • {all_tree_keys} collapse/expand all • using tree glyphs /",
                data_keys.search_label(),
                data_keys.toggle_expansion_label()
            ),
            Self::TableTree => format!(
                "100 rows • {} search • {} filter header key • rich cells • {} node • {all_tree_keys} all • s sorts • using tree glyphs /",
                data_keys.search_label(),
                data_keys.filter_label(),
                data_keys.toggle_expansion_label()
            ),
            Self::SingleSelect => format!(
                "{} selects + activates • single selected ID",
                data_keys.activate_label()
            ),
            Self::MultiSelect => format!(
                "{} toggles rows • selected IDs stay in source order",
                data_keys.activate_label()
            ),
            Self::ChecklistTree => format!(
                "{} cascades descendants • Nerd Font mixed icon",
                data_keys.activate_label()
            ),
            Self::ActivateOnNavigate => {
                format!(
                    "{scroll_keys} changes active + selected row immediately • dropdown-style preview"
                )
            }
        }
    }

    pub(crate) fn data_view(self) -> DataView<DemoRow, usize> {
        let rows = demo_rows();
        let expanded = rows
            .iter()
            .filter(|row| row.parent.is_none() || (1..4).contains(&(row.id % 10)))
            .map(|row| row.id)
            .collect::<Vec<_>>();

        match self {
            Self::List => {
                DataView::list(rows, |row| row.id, |row| row.name.clone()).action_bar(true)
            }
            Self::Table => DataView::new(rows, |row| row.id)
                .headers(true)
                .action_bar(true)
                .columns(demo_columns()),
            Self::ListTree => DataView::list(rows, |row| row.id, |row| row.name.clone())
                .action_bar(true)
                .tree(TreeAdapter::parent_id(|row: &DemoRow| row.parent))
                .tree_glyphs(TreeGlyphs::NERD_FONT)
                .expanded(expanded),
            Self::TableTree => DataView::new(rows, |row| row.id)
                .headers(true)
                .action_bar(true)
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

#[derive(Debug, Clone)]
pub(crate) struct DemoRow {
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

pub(crate) fn data_event_status(event: DataViewTypedEvent<usize>) -> String {
    match event {
        DataViewTypedEvent::HighlightChanged { row_id } => format!("highlight → {row_id:?}"),
        DataViewTypedEvent::Activated { row_id } => format!("activated #{row_id}"),
        DataViewTypedEvent::SelectionChanged { selected, .. } => format!("selected {selected:?}"),
        DataViewTypedEvent::TransformChanged { state } => {
            format!(
                "search {:?} • filters {}",
                state.search,
                state.filters.len()
            )
        }
    }
}

fn demo_columns() -> Vec<Column<DemoRow, usize>> {
    vec![
        Column::text(
            "task",
            "Task",
            Constraint::Percentage(45),
            |row: &DemoRow| row.name.clone(),
        )
        .sortable(|row| row.name.clone())
        .search_key(|row| row.name.clone())
        .filter_key(|row| row.name.clone()),
        Column::text(
            "owner",
            "Owner",
            Constraint::Percentage(20),
            |row: &DemoRow| row.owner.to_string(),
        )
        .sortable(|row| row.owner.to_string())
        .search_key(|row| row.owner.to_string())
        .filter_key(|row| row.owner.to_string()),
        Column::rich(
            "status",
            "Status",
            Constraint::Percentage(20),
            |row: &DemoRow, _: &CellContext<usize>| {
                let theme = tuicore::theme();
                let (label, color) = match row.status {
                    Status::Ready => ("READY", theme.success_fg()),
                    Status::Active => ("ACTIVE", theme.accent_fg()),
                    Status::Blocked => ("BLOCKED", theme.error_fg()),
                };
                Line::from(Span::styled(
                    label,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ))
            },
        )
        .search_key(|row| status_label(row.status).to_string())
        .filter_key(|row| status_label(row.status).to_string()),
        Column::rich(
            "progress",
            "Progress",
            Constraint::Percentage(15),
            |row: &DemoRow, _: &CellContext<usize>| {
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
        )
        .search_key(|row| row.progress.to_string())
        .filter_key(|row| progress_bucket(row.progress).to_string()),
    ]
}

fn status_label(status: Status) -> &'static str {
    match status {
        Status::Ready => "READY",
        Status::Active => "ACTIVE",
        Status::Blocked => "BLOCKED",
    }
}

fn progress_bucket(progress: u8) -> &'static str {
    match progress {
        0..=24 => "0-24%",
        25..=49 => "25-49%",
        50..=74 => "50-74%",
        _ => "75-100%",
    }
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

pub(crate) fn data_view_layout(area: Rect) -> [Rect; 2] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Fill(1)])
        .areas(area)
}
