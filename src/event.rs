use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};

use crossterm::event as crossterm_event;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuiEvent {
    Key(KeyEvent),
    Hotkey(HotkeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Paste(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotkeyEvent {
    Pending(String),
    Canceled,
    Commit(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnsupportedEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Backspace,
    Enter,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Tab,
    BackTab,
    Delete,
    Insert,
    F(u8),
    Char(char),
    Null,
    Esc,
    CapsLock,
    ScrollLock,
    NumLock,
    PrintScreen,
    Pause,
    Menu,
    KeypadBegin,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    pub code: Key,
    pub modifiers: KeyModifiers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct KeyModifiers(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MouseEvent {
    pub kind: MouseEventKind,
    pub column: u16,
    pub row: u16,
    pub modifiers: KeyModifiers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseEventKind {
    Down(MouseButton),
    Up(MouseButton),
    Drag(MouseButton),
    Moved,
    ScrollDown,
    ScrollUp,
    ScrollLeft,
    ScrollRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl KeyModifiers {
    pub const NONE: Self = Self(0);
    pub const SHIFT: Self = Self(0b0000_0001);
    pub const CONTROL: Self = Self(0b0000_0010);
    pub const ALT: Self = Self(0b0000_0100);

    pub const fn empty() -> Self {
        Self::NONE
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub const fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }
}

impl From<Key> for KeyEvent {
    fn from(code: Key) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::NONE,
        }
    }
}

impl From<crossterm_event::KeyEvent> for KeyEvent {
    fn from(value: crossterm_event::KeyEvent) -> Self {
        Self {
            code: value.code.into(),
            modifiers: value.modifiers.into(),
        }
    }
}

impl From<crossterm_event::KeyCode> for Key {
    fn from(value: crossterm_event::KeyCode) -> Self {
        match value {
            crossterm_event::KeyCode::Backspace => Self::Backspace,
            crossterm_event::KeyCode::Enter => Self::Enter,
            crossterm_event::KeyCode::Left => Self::Left,
            crossterm_event::KeyCode::Right => Self::Right,
            crossterm_event::KeyCode::Up => Self::Up,
            crossterm_event::KeyCode::Down => Self::Down,
            crossterm_event::KeyCode::Home => Self::Home,
            crossterm_event::KeyCode::End => Self::End,
            crossterm_event::KeyCode::PageUp => Self::PageUp,
            crossterm_event::KeyCode::PageDown => Self::PageDown,
            crossterm_event::KeyCode::Tab => Self::Tab,
            crossterm_event::KeyCode::BackTab => Self::BackTab,
            crossterm_event::KeyCode::Delete => Self::Delete,
            crossterm_event::KeyCode::Insert => Self::Insert,
            crossterm_event::KeyCode::F(value) => Self::F(value),
            crossterm_event::KeyCode::Char(value) => Self::Char(value),
            crossterm_event::KeyCode::Null => Self::Null,
            crossterm_event::KeyCode::Esc => Self::Esc,
            crossterm_event::KeyCode::CapsLock => Self::CapsLock,
            crossterm_event::KeyCode::ScrollLock => Self::ScrollLock,
            crossterm_event::KeyCode::NumLock => Self::NumLock,
            crossterm_event::KeyCode::PrintScreen => Self::PrintScreen,
            crossterm_event::KeyCode::Pause => Self::Pause,
            crossterm_event::KeyCode::Menu => Self::Menu,
            crossterm_event::KeyCode::KeypadBegin => Self::KeypadBegin,
            crossterm_event::KeyCode::Media(_) | crossterm_event::KeyCode::Modifier(_) => {
                Self::Unknown
            }
        }
    }
}

impl From<crossterm_event::KeyModifiers> for KeyModifiers {
    fn from(value: crossterm_event::KeyModifiers) -> Self {
        let mut modifiers = Self::NONE;
        if value.contains(crossterm_event::KeyModifiers::SHIFT) {
            modifiers |= Self::SHIFT;
        }
        if value.contains(crossterm_event::KeyModifiers::CONTROL) {
            modifiers |= Self::CONTROL;
        }
        if value.contains(crossterm_event::KeyModifiers::ALT) {
            modifiers |= Self::ALT;
        }
        modifiers
    }
}

impl TryFrom<crossterm_event::Event> for TuiEvent {
    type Error = UnsupportedEvent;

    fn try_from(value: crossterm_event::Event) -> Result<Self, Self::Error> {
        match value {
            crossterm_event::Event::Key(value) => {
                if value.kind == crossterm_event::KeyEventKind::Release {
                    Err(UnsupportedEvent)
                } else {
                    Ok(Self::Key(value.into()))
                }
            }
            crossterm_event::Event::Mouse(value) => Ok(Self::Mouse(value.into())),
            crossterm_event::Event::Resize(width, height) => Ok(Self::Resize(width, height)),
            crossterm_event::Event::Paste(value) => Ok(Self::Paste(value)),
            crossterm_event::Event::FocusGained | crossterm_event::Event::FocusLost => {
                Err(UnsupportedEvent)
            }
        }
    }
}

impl From<crossterm_event::MouseEvent> for MouseEvent {
    fn from(value: crossterm_event::MouseEvent) -> Self {
        Self {
            kind: value.kind.into(),
            column: value.column,
            row: value.row,
            modifiers: value.modifiers.into(),
        }
    }
}

impl From<crossterm_event::MouseEventKind> for MouseEventKind {
    fn from(value: crossterm_event::MouseEventKind) -> Self {
        match value {
            crossterm_event::MouseEventKind::Down(button) => Self::Down(button.into()),
            crossterm_event::MouseEventKind::Up(button) => Self::Up(button.into()),
            crossterm_event::MouseEventKind::Drag(button) => Self::Drag(button.into()),
            crossterm_event::MouseEventKind::Moved => Self::Moved,
            crossterm_event::MouseEventKind::ScrollDown => Self::ScrollDown,
            crossterm_event::MouseEventKind::ScrollUp => Self::ScrollUp,
            crossterm_event::MouseEventKind::ScrollLeft => Self::ScrollLeft,
            crossterm_event::MouseEventKind::ScrollRight => Self::ScrollRight,
        }
    }
}

impl From<crossterm_event::MouseButton> for MouseButton {
    fn from(value: crossterm_event::MouseButton) -> Self {
        match value {
            crossterm_event::MouseButton::Left => Self::Left,
            crossterm_event::MouseButton::Right => Self::Right,
            crossterm_event::MouseButton::Middle => Self::Middle,
        }
    }
}

impl BitOr for KeyModifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for KeyModifiers {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for KeyModifiers {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for KeyModifiers {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl Not for KeyModifiers {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossterm_key_conversion_preserves_code_and_modifiers() {
        let key = crossterm_event::KeyEvent::new(
            crossterm_event::KeyCode::Char('x'),
            crossterm_event::KeyModifiers::CONTROL | crossterm_event::KeyModifiers::ALT,
        );

        assert_eq!(
            KeyEvent::from(key),
            KeyEvent {
                code: Key::Char('x'),
                modifiers: KeyModifiers::CONTROL | KeyModifiers::ALT,
            }
        );
    }

    #[test]
    fn crossterm_key_release_events_are_unsupported() {
        let key = crossterm_event::KeyEvent::new_with_kind(
            crossterm_event::KeyCode::Char('x'),
            crossterm_event::KeyModifiers::NONE,
            crossterm_event::KeyEventKind::Release,
        );

        assert_eq!(
            TuiEvent::try_from(crossterm_event::Event::Key(key)),
            Err(UnsupportedEvent)
        );
    }

    #[test]
    fn modifier_contains_and_intersects_match_bitwise_flags() {
        let modifiers = KeyModifiers::CONTROL | KeyModifiers::SHIFT;

        assert!(modifiers.contains(KeyModifiers::CONTROL));
        assert!(modifiers.contains(KeyModifiers::SHIFT));
        assert!(!modifiers.contains(KeyModifiers::ALT));
        assert!(modifiers.intersects(KeyModifiers::SHIFT | KeyModifiers::ALT));
        assert!(!modifiers.intersects(KeyModifiers::ALT));
    }
}
