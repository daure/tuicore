#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub enum Id {
    ComponentList,
    Panel,
    ScrollAnimated,
    Spinner,
    Tabs,
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    Quit,
    FocusList,
    FocusPreview,
    Selected(ComponentKind),
    Redraw,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ComponentKind {
    Tabs,
    Panel,
    ScrollAnimated,
    Spinner,
}

impl ComponentKind {
    pub const ALL: [Self; 4] = [Self::Tabs, Self::Panel, Self::ScrollAnimated, Self::Spinner];

    pub fn title(self) -> &'static str {
        match self {
            Self::Tabs => "Tabs",
            Self::Panel => "Panels",
            Self::ScrollAnimated => "Scroll: animated",
            Self::Spinner => "Spinner",
        }
    }

    pub fn preview_id(self) -> Id {
        match self {
            Self::Tabs => Id::Tabs,
            Self::Panel => Id::Panel,
            Self::ScrollAnimated => Id::ScrollAnimated,
            Self::Spinner => Id::Spinner,
        }
    }
}
