use crate::KeySpec;
use crate::event::Key;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DropdownFocusRegion {
    Field,
    Search,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropdownSearchMode {
    None,
    Contains,
    Fuzzy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropdownCommitMode {
    Explicit,
    Immediate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DropdownVariant {
    #[default]
    Bordered,
    Filled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DropdownLabelPosition {
    #[default]
    Top,
    Inline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DropdownPopupDirection {
    #[default]
    Down,
    Up,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropdownActionKeys {
    pub open: Vec<KeySpec>,
    pub commit: Vec<KeySpec>,
    pub toggle: Vec<KeySpec>,
}

impl Default for DropdownActionKeys {
    fn default() -> Self {
        Self {
            open: vec![KeySpec::key(Key::Enter), KeySpec::plain(' ')],
            commit: vec![KeySpec::key(Key::Enter)],
            toggle: vec![KeySpec::plain(' ')],
        }
    }
}

impl DropdownActionKeys {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DropdownOutcome {
    pub handled: bool,
    pub changed: bool,
    pub opened: bool,
    pub closed: bool,
    pub committed: bool,
    pub canceled: bool,
}

impl DropdownOutcome {
    pub const IDLE: Self = Self {
        handled: false,
        changed: false,
        opened: false,
        closed: false,
        committed: false,
        canceled: false,
    };

    pub const HANDLED: Self = Self {
        handled: true,
        changed: false,
        opened: false,
        closed: false,
        committed: false,
        canceled: false,
    };

    pub(super) fn changed() -> Self {
        Self {
            handled: true,
            changed: true,
            ..Self::IDLE
        }
    }
}
