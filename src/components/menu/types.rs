use crate::KeySpec;
use crate::event::Key;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem<Id> {
    pub id: Id,
    pub label: String,
}

impl<Id> MenuItem<Id> {
    pub fn new(id: Id, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuSearchMode {
    None,
    Contains,
    Fuzzy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MenuPopupDirection {
    #[default]
    Down,
    Up,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuActionKeys {
    pub activate: Vec<KeySpec>,
}

impl Default for MenuActionKeys {
    fn default() -> Self {
        Self {
            activate: vec![KeySpec::key(Key::Enter)],
        }
    }
}

impl MenuActionKeys {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuOutcome {
    pub handled: bool,
    pub changed: bool,
    pub opened: bool,
    pub closed: bool,
    pub activated: bool,
}

impl MenuOutcome {
    pub const IDLE: Self = Self {
        handled: false,
        changed: false,
        opened: false,
        closed: false,
        activated: false,
    };

    pub const HANDLED: Self = Self {
        handled: true,
        changed: false,
        opened: false,
        closed: false,
        activated: false,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MenuFocusRegion {
    Search,
    Panel,
}
