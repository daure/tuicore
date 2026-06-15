use tuirealm::event::KeyEvent;

#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub enum Id {
    ComponentList,
    DataViewList,
    DataViewTable,
    DataViewListTree,
    DataViewTableTree,
    DataViewSingleSelect,
    DataViewMultiSelect,
    DataViewChecklistTree,
    DataViewActivateOnNavigate,
    Panel,
    ScrollAnimated,
    Spinner,
    Tabs,
}

pub fn focus_list_key(key: KeyEvent) -> bool {
    let bindings = tuicore::keybindings();
    let focus = bindings.focus();
    focus.next_matches(key) || focus.previous_matches(key)
}

pub fn focus_nav_message(key: KeyEvent) -> Option<Msg> {
    let bindings = tuicore::keybindings();
    let focus = bindings.focus();
    if focus.next_matches(key) {
        Some(Msg::FocusNext)
    } else if focus.previous_matches(key) {
        Some(Msg::FocusPrevious)
    } else {
        None
    }
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    Quit,
    FocusNext,
    FocusPrevious,
    FocusList,
    Selected(ComponentKind),
    Redraw,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum ComponentKind {
    Tabs,
    Panel,
    ScrollAnimated,
    Spinner,
    DataView,
    DataViewList,
    DataViewTable,
    DataViewListTree,
    DataViewTableTree,
    DataViewSingleSelect,
    DataViewMultiSelect,
    DataViewChecklistTree,
    DataViewActivateOnNavigate,
}

impl ComponentKind {
    pub const ALL: [Self; 13] = [
        Self::Tabs,
        Self::Panel,
        Self::ScrollAnimated,
        Self::Spinner,
        Self::DataView,
        Self::DataViewList,
        Self::DataViewTable,
        Self::DataViewListTree,
        Self::DataViewTableTree,
        Self::DataViewSingleSelect,
        Self::DataViewMultiSelect,
        Self::DataViewChecklistTree,
        Self::DataViewActivateOnNavigate,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Tabs => "Tabs",
            Self::Panel => "Panels",
            Self::ScrollAnimated => "Scroll: animated",
            Self::Spinner => "Spinner",
            Self::DataView => "DataView",
            Self::DataViewList => "DataView: list",
            Self::DataViewTable => "DataView: table",
            Self::DataViewListTree => "DataView: list tree",
            Self::DataViewTableTree => "DataView: table tree",
            Self::DataViewSingleSelect => "DataView: single select",
            Self::DataViewMultiSelect => "DataView: multi select",
            Self::DataViewChecklistTree => "DataView: tree checklist",
            Self::DataViewActivateOnNavigate => "DataView: activate on navigate",
        }
    }

    pub fn preview_id(self) -> Id {
        match self {
            Self::Tabs => Id::Tabs,
            Self::Panel => Id::Panel,
            Self::ScrollAnimated => Id::ScrollAnimated,
            Self::Spinner => Id::Spinner,
            Self::DataView => Id::DataViewList,
            Self::DataViewList => Id::DataViewList,
            Self::DataViewTable => Id::DataViewTable,
            Self::DataViewListTree => Id::DataViewListTree,
            Self::DataViewTableTree => Id::DataViewTableTree,
            Self::DataViewSingleSelect => Id::DataViewSingleSelect,
            Self::DataViewMultiSelect => Id::DataViewMultiSelect,
            Self::DataViewChecklistTree => Id::DataViewChecklistTree,
            Self::DataViewActivateOnNavigate => Id::DataViewActivateOnNavigate,
        }
    }

    pub fn parent(self) -> Option<Self> {
        match self {
            Self::DataViewList
            | Self::DataViewTable
            | Self::DataViewListTree
            | Self::DataViewTableTree
            | Self::DataViewSingleSelect
            | Self::DataViewMultiSelect
            | Self::DataViewChecklistTree
            | Self::DataViewActivateOnNavigate => Some(Self::DataView),
            _ => None,
        }
    }
}
