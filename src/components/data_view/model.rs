use std::any::Any;
use std::cmp::Ordering;
use std::rc::Rc;

use ratatui::layout::Constraint;
use ratatui::text::{Line, Text};

pub(super) type RowIdFn<T, Id> = dyn Fn(&T) -> Id;
pub(super) type ParentIdFn<T, Id> = dyn Fn(&T) -> Option<Id>;
pub(super) type ParentIdMutFn<T, Id> = dyn Fn(&mut T, Option<Id>);
pub(super) type LevelFn<T> = dyn Fn(&T) -> usize;
type CellFn<T, Id> = dyn Fn(&T, &CellContext<Id>) -> Text<'static>;
pub(super) type SortFn<T> = dyn Fn(&T, &T) -> Ordering;
type TransformKeyFn<T> = dyn Fn(&T) -> String;

pub(super) struct ReorderOps<T> {
    pub compare: Box<SortFn<T>>,
    pub snapshot: Box<dyn Fn(&[T], &[usize]) -> Box<dyn Any>>,
    pub snapshot_matches: Box<dyn Fn(&[T], &[usize], &dyn Any) -> bool>,
    pub apply: Box<dyn Fn(&[T], &[usize], &dyn Any) -> Option<Vec<T>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReorderUnavailableReason {
    Tree,
    VisibleSubset,
    TransformActive,
    Paginated,
    DuplicateRowIds,
    DuplicateRankKeys,
}

pub(crate) struct ReorderSnapshot<Id> {
    pub ids: Vec<Id>,
    pub(super) ranks: Box<dyn Any>,
}

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
pub enum DataViewTypedEvent<Id> {
    HighlightChanged {
        row_id: Option<Id>,
    },
    Activated {
        row_id: Id,
    },
    SelectionChanged {
        selected: Vec<Id>,
        added: Vec<Id>,
        removed: Vec<Id>,
    },
    TransformChanged {
        state: DataViewTransformState,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataViewFilter {
    pub column_id: String,
    pub value: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataViewTransformState {
    pub search: String,
    pub filters: Vec<DataViewFilter>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataViewTransformMode {
    Local,
    External,
}

impl Default for DataViewTransformMode {
    fn default() -> Self {
        Self::Local
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationMode {
    Manual,
    OnActivateKey,
    OnNavigate,
}

impl Default for ActivationMode {
    fn default() -> Self {
        Self::OnActivateKey
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    None,
    Single,
    Multi,
}

impl Default for SelectionMode {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionTrigger {
    Manual,
    OnActivate,
    OnNavigate,
}

impl Default for SelectionTrigger {
    fn default() -> Self {
        Self::Manual
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionPropagation {
    None,
    CascadeDescendants,
}

impl Default for SelectionPropagation {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckState {
    Unchecked,
    Checked,
    Indeterminate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionGlyphs {
    pub unchecked: &'static str,
    pub checked: &'static str,
    pub indeterminate: &'static str,
}

impl SelectionGlyphs {
    pub const ASCII: Self = Self {
        unchecked: "[ ]",
        checked: "[x]",
        indeterminate: "[-]",
    };

    pub const NERD_FONT: Self = Self {
        unchecked: "󰄱",
        checked: "󰱒",
        indeterminate: "󰡖",
    };

    pub(crate) fn glyph(self, state: CheckState) -> &'static str {
        match state {
            CheckState::Unchecked => self.unchecked,
            CheckState::Checked => self.checked,
            CheckState::Indeterminate => self.indeterminate,
        }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColumnSizing {
    #[default]
    Intrinsic,
    Constrained,
}

pub struct Column<T, Id> {
    pub(super) id: String,
    pub(super) header: String,
    pub(super) visible: bool,
    pub(super) width: Constraint,
    pub(super) sizing: ColumnSizing,
    pub(super) renderer: Box<CellFn<T, Id>>,
    pub(super) sort_compare: Option<Box<SortFn<T>>>,
    pub(super) reorder: Option<ReorderOps<T>>,
    pub(super) search_key: Option<Box<TransformKeyFn<T>>>,
    pub(super) filter_key: Option<Box<TransformKeyFn<T>>>,
}

impl<T, Id> Column<T, Id> {
    pub fn text(
        id: impl Into<String>,
        header: impl Into<String>,
        width: Constraint,
        accessor: impl Fn(&T) -> String + 'static,
    ) -> Self {
        let accessor = Rc::new(accessor);
        let renderer_accessor = Rc::clone(&accessor);
        let search_accessor = Rc::clone(&accessor);
        Self {
            id: id.into(),
            header: header.into(),
            visible: true,
            width,
            sizing: ColumnSizing::Intrinsic,
            renderer: Box::new(move |row, _| Text::from(renderer_accessor(row))),
            sort_compare: None,
            reorder: None,
            search_key: Some(Box::new(move |row| search_accessor(row))),
            filter_key: None,
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
            visible: true,
            width,
            sizing: ColumnSizing::Intrinsic,
            renderer: Box::new(move |row, context| Text::from(renderer(row, context))),
            sort_compare: None,
            reorder: None,
            search_key: None,
            filter_key: None,
        }
    }

    /// Creates a column whose cells may contain multiple logical lines.
    ///
    /// DataView clips returned content to each row's configured height; content never increases
    /// row height automatically.
    pub fn multiline<R: Into<Text<'static>>>(
        id: impl Into<String>,
        header: impl Into<String>,
        width: Constraint,
        renderer: impl Fn(&T, &CellContext<Id>) -> R + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            header: header.into(),
            visible: true,
            width,
            sizing: ColumnSizing::Intrinsic,
            renderer: Box::new(move |row, context| renderer(row, context).into()),
            sort_compare: None,
            reorder: None,
            search_key: None,
            filter_key: None,
        }
    }

    pub fn sortable<K: Ord + 'static>(mut self, key: impl Fn(&T) -> K + 'static) -> Self {
        self.sort_compare = Some(Box::new(move |left, right| key(left).cmp(&key(right))));
        self
    }

    pub fn sortable_by(mut self, compare: impl Fn(&T, &T) -> Ordering + 'static) -> Self {
        self.sort_compare = Some(Box::new(compare));
        self
    }

    /// Configures property-backed row ordering.
    ///
    /// The setter must assign the supplied rank key without changing row identity. Commits verify
    /// both the assigned keys and row IDs before reporting success.
    pub fn reorderable<K: Ord + Clone + 'static>(
        mut self,
        getter: impl Fn(&T) -> K + 'static,
        setter: impl Fn(&mut T, K) + 'static,
    ) -> Self
    where
        T: Clone,
    {
        let getter = Rc::new(getter);
        let compare_getter = Rc::clone(&getter);
        let snapshot_getter = Rc::clone(&getter);
        let matches_getter = Rc::clone(&getter);
        self.reorder = Some(ReorderOps {
            compare: Box::new(move |left, right| compare_getter(left).cmp(&compare_getter(right))),
            snapshot: Box::new(move |rows, ordered| {
                Box::new(
                    ordered
                        .iter()
                        .map(|index| snapshot_getter(&rows[*index]))
                        .collect::<Vec<_>>(),
                )
            }),
            snapshot_matches: Box::new(move |rows, ordered, snapshot| {
                let Some(snapshot) = snapshot.downcast_ref::<Vec<K>>() else {
                    return false;
                };
                ordered
                    .iter()
                    .map(|index| matches_getter(&rows[*index]))
                    .eq(snapshot.iter().cloned())
            }),
            apply: Box::new(move |rows, staged, snapshot| {
                let Some(keys) = snapshot.downcast_ref::<Vec<K>>() else {
                    return None;
                };
                if staged.len() != keys.len() {
                    return None;
                }
                let mut candidate = rows.to_vec();
                for (index, key) in staged.iter().copied().zip(keys.iter().cloned()) {
                    setter(&mut candidate[index], key);
                }
                Some(candidate)
            }),
        });
        self
    }

    pub fn search_key(mut self, search_key: impl Fn(&T) -> String + 'static) -> Self {
        self.search_key = Some(Box::new(search_key));
        self
    }

    pub fn filter_key(mut self, filter_key: impl Fn(&T) -> String + 'static) -> Self {
        self.filter_key = Some(Box::new(filter_key));
        self
    }

    pub fn sizing(mut self, sizing: ColumnSizing) -> Self {
        self.sizing = sizing;
        self
    }

    pub fn constrained(self) -> Self {
        self.sizing(ColumnSizing::Constrained)
    }

    pub fn hidden(self) -> Self {
        self.visible(false)
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

pub enum TreeAdapter<T, Id> {
    ParentId(Box<ParentIdFn<T, Id>>),
    MutableParentId {
        parent_id: Box<ParentIdFn<T, Id>>,
        set_parent_id: Box<ParentIdMutFn<T, Id>>,
    },
    Level(Box<LevelFn<T>>),
}

impl<T, Id> TreeAdapter<T, Id> {
    pub fn parent_id(parent_id: impl Fn(&T) -> Option<Id> + 'static) -> Self {
        Self::ParentId(Box::new(parent_id))
    }

    pub fn mutable_parent_id(
        parent_id: impl Fn(&T) -> Option<Id> + 'static,
        set_parent_id: impl Fn(&mut T, Option<Id>) + 'static,
    ) -> Self {
        Self::MutableParentId {
            parent_id: Box::new(parent_id),
            set_parent_id: Box::new(set_parent_id),
        }
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

#[derive(Debug, Clone)]
pub(crate) enum SelectionOverlayPosition<Id> {
    Before(Id),
    After(Id),
}

#[derive(Debug, Clone)]
pub(crate) struct SelectionOverlay<Id> {
    pub selected: Vec<Id>,
    pub position: Option<SelectionOverlayPosition<Id>>,
    pub placeholder_depth: usize,
    pub placeholder_focused: bool,
}

pub(super) enum DisplayRow<'a, T, Id> {
    Data(VisibleRow<'a, T, Id>),
    SelectionPlaceholder {
        count: usize,
        depth: usize,
        focused: bool,
    },
}
