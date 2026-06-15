use tuirealm::ratatui::layout::Constraint;
use tuirealm::ratatui::text::Line;

pub(super) type RowIdFn<T, Id> = dyn Fn(&T) -> Id;
pub(super) type ParentIdFn<T, Id> = dyn Fn(&T) -> Option<Id>;
pub(super) type LevelFn<T> = dyn Fn(&T) -> usize;
type CellFn<T, Id> = dyn Fn(&T, &CellContext<Id>) -> Line<'static>;
type SortFn<T> = dyn Fn(&T) -> String;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataViewOutcome {
    pub handled: bool,
    pub changed: bool,
    pub active: bool,
    pub activated: bool,
}

impl DataViewOutcome {
    pub const IDLE: Self = Self {
        handled: false,
        changed: false,
        active: false,
        activated: false,
    };

    pub const HANDLED: Self = Self {
        handled: true,
        changed: false,
        active: false,
        activated: false,
    };

    pub const CHANGED: Self = Self {
        handled: true,
        changed: true,
        active: false,
        activated: false,
    };

    pub fn needs_redraw(self) -> bool {
        self.changed || self.active || self.activated
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    pub fn reversed(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataViewEvent<Id> {
    pub row_id: Id,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataViewSort {
    pub column_id: String,
    pub direction: SortDirection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataViewPagination {
    pub page_size: usize,
    pub page: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeGlyphs {
    pub expanded: &'static str,
    pub collapsed: &'static str,
    pub leaf: &'static str,
}

impl TreeGlyphs {
    pub const TRIANGLE: Self = Self {
        expanded: "▾",
        collapsed: "▸",
        leaf: " ",
    };

    pub const FILLED_TRIANGLE: Self = Self {
        expanded: "▼",
        collapsed: "▶",
        leaf: " ",
    };

    pub const ASCII: Self = Self {
        expanded: "v",
        collapsed: ">",
        leaf: " ",
    };

    pub const NERD_FONT: Self = Self {
        expanded: "",
        collapsed: "",
        leaf: " ",
    };
}

#[derive(Debug, Clone)]
pub struct CellContext<Id> {
    pub row_id: Id,
    pub column_id: String,
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,
    pub highlighted: bool,
    pub focused: bool,
}

pub struct Column<T, Id> {
    pub(super) id: String,
    pub(super) header: String,
    pub(super) width: Constraint,
    pub(super) renderer: Box<CellFn<T, Id>>,
    pub(super) sort_key: Option<Box<SortFn<T>>>,
}

impl<T, Id> Column<T, Id> {
    pub fn text(
        id: impl Into<String>,
        header: impl Into<String>,
        width: Constraint,
        accessor: impl Fn(&T) -> String + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            header: header.into(),
            width,
            renderer: Box::new(move |row, _| Line::from(accessor(row))),
            sort_key: None,
        }
    }

    pub fn rich(
        id: impl Into<String>,
        header: impl Into<String>,
        width: Constraint,
        renderer: impl Fn(&T, &CellContext<Id>) -> Line<'static> + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            header: header.into(),
            width,
            renderer: Box::new(renderer),
            sort_key: None,
        }
    }

    pub fn sortable(mut self, sort_key: impl Fn(&T) -> String + 'static) -> Self {
        self.sort_key = Some(Box::new(sort_key));
        self
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

pub enum TreeAdapter<T, Id> {
    ParentId(Box<ParentIdFn<T, Id>>),
    Level(Box<LevelFn<T>>),
}

impl<T, Id> TreeAdapter<T, Id> {
    pub fn parent_id(parent_id: impl Fn(&T) -> Option<Id> + 'static) -> Self {
        Self::ParentId(Box::new(parent_id))
    }

    pub fn level(level: impl Fn(&T) -> usize + 'static) -> Self {
        Self::Level(Box::new(level))
    }
}

pub(super) struct VisibleRow<'a, T, Id> {
    pub row: &'a T,
    pub id: Id,
    pub parent_id: Option<Id>,
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,
}
