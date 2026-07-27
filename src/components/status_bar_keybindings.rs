use crate::{KeyEvent, KeySpec};

use super::{DEFAULT_AI_HOTKEY, DEFAULT_MENU_HOTKEY};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBarKeyBindings {
    menu_toggle: Vec<KeySpec>,
    ai_open: Vec<KeySpec>,
    menu_hotkey: String,
    ai_hotkey: String,
}

impl Default for StatusBarKeyBindings {
    fn default() -> Self {
        Self {
            menu_toggle: vec![KeySpec::plain(';')],
            ai_open: vec![KeySpec::plain('\'')],
            menu_hotkey: DEFAULT_MENU_HOTKEY.to_string(),
            ai_hotkey: DEFAULT_AI_HOTKEY.to_string(),
        }
    }
}

impl StatusBarKeyBindings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_menu_toggle(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.menu_toggle = keys.into_iter().collect();
    }

    pub fn with_menu_toggle(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_menu_toggle(keys);
        self
    }

    pub fn set_ai_open(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.ai_open = keys.into_iter().collect();
    }

    pub fn with_ai_open(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_ai_open(keys);
        self
    }

    pub fn set_menu_hotkey(&mut self, hotkey: impl Into<String>) {
        self.menu_hotkey = hotkey.into();
    }

    pub fn with_menu_hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.set_menu_hotkey(hotkey);
        self
    }

    pub fn set_ai_hotkey(&mut self, hotkey: impl Into<String>) {
        self.ai_hotkey = hotkey.into();
    }

    pub fn with_ai_hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.set_ai_hotkey(hotkey);
        self
    }

    pub fn menu_toggle_matches(&self, key: impl Into<KeyEvent>) -> bool {
        let key = key.into();
        self.menu_toggle
            .iter()
            .copied()
            .any(|binding| binding.matches(key))
    }

    pub fn ai_open_matches(&self, key: impl Into<KeyEvent>) -> bool {
        let key = key.into();
        self.ai_open
            .iter()
            .copied()
            .any(|binding| binding.matches(key))
    }

    pub fn menu_hotkey(&self) -> &str {
        &self.menu_hotkey
    }

    pub fn ai_hotkey(&self) -> &str {
        &self.ai_hotkey
    }
}
