use std::{env, fs, io, path::PathBuf};

use tuirealm::event::{Key, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBindings {
    nav: NavKeyBindings,
    tabs: TabsKeyBindings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavKeyBindings {
    line_up: Vec<KeySpec>,
    line_down: Vec<KeySpec>,
    line_left: Vec<KeySpec>,
    line_right: Vec<KeySpec>,
    page_up: Vec<KeySpec>,
    page_down: Vec<KeySpec>,
    home: Vec<KeySpec>,
    end: Vec<KeySpec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabsKeyBindings {
    previous: KeySpec,
    next: KeySpec,
}

impl Default for NavKeyBindings {
    fn default() -> Self {
        Self {
            line_up: vec![KeySpec::key(Key::Up)],
            line_down: vec![KeySpec::key(Key::Down)],
            line_left: vec![KeySpec::key(Key::Left)],
            line_right: vec![KeySpec::key(Key::Right)],
            page_up: vec![KeySpec::key(Key::PageUp)],
            page_down: vec![KeySpec::key(Key::PageDown)],
            home: vec![KeySpec::key(Key::Home)],
            end: vec![KeySpec::key(Key::End)],
        }
    }
}

impl Default for TabsKeyBindings {
    fn default() -> Self {
        Self {
            previous: KeySpec::plain('['),
            next: KeySpec::plain(']'),
        }
    }
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            nav: NavKeyBindings::default(),
            tabs: TabsKeyBindings::default(),
        }
    }
}

impl KeyBindings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load() -> Self {
        let Some(path) = keybindings_path() else {
            return Self::default();
        };

        Self::load_from_path(path).unwrap_or_default()
    }

    pub fn load_from_path(path: impl Into<PathBuf>) -> io::Result<Self> {
        let text = fs::read_to_string(path.into())?;
        Ok(Self::from_toml_str(&text))
    }

    pub fn from_toml_str(text: &str) -> Self {
        let mut bindings = Self::default();
        let Ok(value) = toml::from_str::<toml::Table>(text) else {
            return bindings;
        };

        set_keys(&value, "nav", "line_up", &mut bindings.nav.line_up);
        set_keys(&value, "nav", "line_down", &mut bindings.nav.line_down);
        set_keys(&value, "nav", "line_left", &mut bindings.nav.line_left);
        set_keys(&value, "nav", "line_right", &mut bindings.nav.line_right);
        set_keys(&value, "nav", "page_up", &mut bindings.nav.page_up);
        set_keys(&value, "nav", "page_down", &mut bindings.nav.page_down);
        set_keys(&value, "nav", "home", &mut bindings.nav.home);
        set_keys(&value, "nav", "end", &mut bindings.nav.end);
        set_key(&value, "tabs", "previous_tab", &mut bindings.tabs.previous);
        set_key(&value, "tabs", "next_tab", &mut bindings.tabs.next);

        bindings
    }

    pub fn tabs(&self) -> &TabsKeyBindings {
        &self.tabs
    }

    pub fn set_tabs_previous(&mut self, key: KeySpec) {
        self.tabs.previous = key;
    }

    pub fn with_tabs_previous(mut self, key: KeySpec) -> Self {
        self.set_tabs_previous(key);
        self
    }

    pub fn set_tabs_next(&mut self, key: KeySpec) {
        self.tabs.next = key;
    }

    pub fn with_tabs_next(mut self, key: KeySpec) -> Self {
        self.set_tabs_next(key);
        self
    }

    pub fn set_nav_line_up(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.nav.line_up = keys.into_iter().collect();
    }

    pub fn with_nav_line_up(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_nav_line_up(keys);
        self
    }

    pub fn set_nav_line_down(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.nav.line_down = keys.into_iter().collect();
    }

    pub fn with_nav_line_down(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_nav_line_down(keys);
        self
    }

    pub fn set_nav_line_left(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.nav.line_left = keys.into_iter().collect();
    }

    pub fn with_nav_line_left(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_nav_line_left(keys);
        self
    }

    pub fn set_nav_line_right(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.nav.line_right = keys.into_iter().collect();
    }

    pub fn with_nav_line_right(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_nav_line_right(keys);
        self
    }

    pub fn set_nav_page_up(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.nav.page_up = keys.into_iter().collect();
    }

    pub fn with_nav_page_up(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_nav_page_up(keys);
        self
    }

    pub fn set_nav_page_down(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.nav.page_down = keys.into_iter().collect();
    }

    pub fn with_nav_page_down(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_nav_page_down(keys);
        self
    }

    pub fn set_nav_home(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.nav.home = keys.into_iter().collect();
    }

    pub fn with_nav_home(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_nav_home(keys);
        self
    }

    pub fn set_nav_end(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.nav.end = keys.into_iter().collect();
    }

    pub fn with_nav_end(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_nav_end(keys);
        self
    }

    pub fn line_up_matches(&self, key: KeyEvent) -> bool {
        matches_any(&self.nav.line_up, key)
    }

    pub fn line_down_matches(&self, key: KeyEvent) -> bool {
        matches_any(&self.nav.line_down, key)
    }

    pub fn line_left_matches(&self, key: KeyEvent) -> bool {
        matches_any(&self.nav.line_left, key)
    }

    pub fn line_right_matches(&self, key: KeyEvent) -> bool {
        matches_any(&self.nav.line_right, key)
    }

    pub fn page_up_matches(&self, key: KeyEvent) -> bool {
        matches_any(&self.nav.page_up, key)
    }

    pub fn page_down_matches(&self, key: KeyEvent) -> bool {
        matches_any(&self.nav.page_down, key)
    }

    pub fn home_matches(&self, key: KeyEvent) -> bool {
        matches_any(&self.nav.home, key)
    }

    pub fn end_matches(&self, key: KeyEvent) -> bool {
        matches_any(&self.nav.end, key)
    }
}

impl TabsKeyBindings {
    pub fn previous_matches(&self, key: KeyEvent) -> bool {
        self.previous.matches(key)
    }

    pub fn next_matches(&self, key: KeyEvent) -> bool {
        self.next.matches(key)
    }

    pub fn previous_label(&self) -> String {
        self.previous.label()
    }

    pub fn next_label(&self) -> String {
        self.next.label()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeySpec {
    code: Key,
    modifiers: KeyModifiers,
}

impl KeySpec {
    pub const fn plain(c: char) -> Self {
        Self {
            code: Key::Char(c),
            modifiers: KeyModifiers::NONE,
        }
    }

    pub const fn shifted(c: char) -> Self {
        Self {
            code: Key::Char(c),
            modifiers: KeyModifiers::SHIFT,
        }
    }

    pub const fn key(code: Key) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::NONE,
        }
    }

    pub const fn key_with_modifiers(code: Key, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }

    pub fn matches(self, key: KeyEvent) -> bool {
        if self == KeySpec::from(key) {
            return true;
        }

        if let KeySpec {
            code: Key::Char(expected),
            modifiers,
        } = self
            && modifiers == KeyModifiers::CONTROL
            && expected.is_ascii_lowercase()
            && let Key::Char(actual) = key.code
        {
            return key.modifiers.contains(KeyModifiers::CONTROL)
                && actual.to_ascii_lowercase() == expected;
        }

        matches!(
            (self.code, self.modifiers, key.code, key.modifiers),
            (Key::Char(expected), KeyModifiers::NONE, Key::Char(actual), KeyModifiers::SHIFT)
                if expected == actual && !actual.is_ascii_alphanumeric()
        )
    }

    pub fn label(self) -> String {
        let key = match self.code {
            Key::Char(' ') => String::from("Space"),
            Key::Char(c) => c.to_string(),
            Key::Esc => String::from("Esc"),
            Key::Enter => String::from("Enter"),
            Key::Tab => String::from("Tab"),
            Key::BackTab => String::from("⇧Tab"),
            Key::Backspace => String::from("Backspace"),
            Key::Delete => String::from("Delete"),
            Key::Left => String::from("Left"),
            Key::Right => String::from("Right"),
            Key::Up => String::from("Up"),
            Key::Down => String::from("Down"),
            Key::Home => String::from("Home"),
            Key::End => String::from("End"),
            Key::PageUp => String::from("PageUp"),
            Key::PageDown => String::from("PageDown"),
            _ => format!("{:?}", self.code),
        };

        if self.modifiers.contains(KeyModifiers::CONTROL) {
            format!("⌃{key}")
        } else if self.modifiers.contains(KeyModifiers::ALT) {
            format!("⌥{key}")
        } else if self.modifiers.contains(KeyModifiers::SHIFT) && !matches!(self.code, Key::BackTab)
        {
            shifted_label(self.code, &key)
        } else {
            key
        }
    }
}

impl From<KeyEvent> for KeySpec {
    fn from(value: KeyEvent) -> Self {
        match value.code {
            Key::Char(c) if c.is_ascii_uppercase() => Self {
                code: Key::Char(c.to_ascii_lowercase()),
                modifiers: value.modifiers | KeyModifiers::SHIFT,
            },
            code => Self {
                code,
                modifiers: value.modifiers,
            },
        }
    }
}

fn shifted_label(code: Key, fallback: &str) -> String {
    match code {
        Key::Char(c) if c.is_ascii_alphabetic() => c.to_ascii_uppercase().to_string(),
        Key::Char(c) => c.to_string(),
        _ => format!("Shift+{fallback}"),
    }
}

fn set_key(value: &toml::Table, section: &str, key: &str, destination: &mut KeySpec) {
    if let Some(configured) = value
        .get(section)
        .and_then(|section| section.get(key))
        .and_then(toml::Value::as_str)
        .and_then(parse_key)
    {
        *destination = configured;
    }
}

fn set_keys(value: &toml::Table, section: &str, key: &str, destination: &mut Vec<KeySpec>) {
    let Some(configured) = value.get(section).and_then(|section| section.get(key)) else {
        return;
    };

    let keys = if let Some(text) = configured.as_str() {
        parse_key(text).into_iter().collect::<Vec<_>>()
    } else if let Some(values) = configured.as_array() {
        values
            .iter()
            .filter_map(toml::Value::as_str)
            .filter_map(parse_key)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    if !keys.is_empty() {
        *destination = keys;
    }
}

fn matches_any(bindings: &[KeySpec], key: KeyEvent) -> bool {
    bindings.iter().any(|binding| binding.matches(key))
}

fn parse_key(value: &str) -> Option<KeySpec> {
    let value = value.trim().to_ascii_lowercase();

    if let Some(rest) = value.strip_prefix("ctrl+") {
        return single_char(rest)
            .map(|key| KeySpec::key_with_modifiers(Key::Char(key), KeyModifiers::CONTROL));
    }

    if let Some(rest) = value.strip_prefix("alt+") {
        return single_char(rest)
            .map(|key| KeySpec::key_with_modifiers(Key::Char(key), KeyModifiers::ALT));
    }

    if let Some(rest) = value.strip_prefix("shift+") {
        return single_char(rest).map(KeySpec::shifted);
    }

    let code = match value.as_str() {
        "esc" => Key::Esc,
        "enter" => Key::Enter,
        "tab" => Key::Tab,
        "backtab" => Key::BackTab,
        "backspace" => Key::Backspace,
        "delete" => Key::Delete,
        "left" => Key::Left,
        "right" => Key::Right,
        "up" => Key::Up,
        "down" => Key::Down,
        "pageup" | "page_up" => Key::PageUp,
        "pagedown" | "page_down" => Key::PageDown,
        "home" => Key::Home,
        "end" => Key::End,
        "space" => Key::Char(' '),
        text => return single_char(text).map(KeySpec::plain),
    };

    Some(KeySpec::key(code))
}

fn single_char(value: &str) -> Option<char> {
    let mut chars = value.chars();
    let key = chars.next()?;
    chars.next().is_none().then_some(key)
}

fn keybindings_path() -> Option<PathBuf> {
    config_dir().map(|path| path.join("keybindings.toml"))
}

pub(crate) fn config_dir() -> Option<PathBuf> {
    if let Some(path) = env::var_os("TUICORE_CONFIG_DIR") {
        return Some(PathBuf::from(path));
    }

    let home = env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".tuicore"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_nav_keys_match_scroll_navigation() {
        let bindings = KeyBindings::from_toml_str(
            r#"
            [nav]
            line_up = "k"
            line_down = "j"
            line_left = "h"
            line_right = "l"
            page_up = ["ctrl+u", "pageup"]
            page_down = ["ctrl+d", "pagedown"]
            home = "g"
            end = "shift+g"
            "#,
        );

        assert!(bindings.line_up_matches(KeyEvent {
            code: Key::Char('k'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.line_down_matches(KeyEvent {
            code: Key::Char('j'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.line_left_matches(KeyEvent {
            code: Key::Char('h'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.line_right_matches(KeyEvent {
            code: Key::Char('l'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.page_up_matches(KeyEvent {
            code: Key::Char('u'),
            modifiers: KeyModifiers::CONTROL,
        }));
        assert!(bindings.page_down_matches(KeyEvent {
            code: Key::PageDown,
            modifiers: KeyModifiers::NONE,
        }));
        assert!(!bindings.page_down_matches(KeyEvent {
            code: Key::Char('u'),
            modifiers: KeyModifiers::CONTROL,
        }));
        assert!(bindings.home_matches(KeyEvent {
            code: Key::Char('g'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.end_matches(KeyEvent {
            code: Key::Char('G'),
            modifiers: KeyModifiers::SHIFT,
        }));
    }

    #[test]
    fn builder_overrides_tabs_and_navigation_keys() {
        let bindings = KeyBindings::new()
            .with_tabs_previous(KeySpec::plain('h'))
            .with_tabs_next(KeySpec::plain('l'))
            .with_nav_line_up([KeySpec::plain('k')])
            .with_nav_line_down([KeySpec::plain('j')])
            .with_nav_line_left([KeySpec::plain('h')])
            .with_nav_line_right([KeySpec::plain('l')])
            .with_nav_page_up([KeySpec::plain('u')])
            .with_nav_page_down([KeySpec::plain('d')])
            .with_nav_home([KeySpec::plain('g')])
            .with_nav_end([KeySpec::shifted('g')]);

        assert!(bindings.tabs().previous_matches(KeyEvent {
            code: Key::Char('h'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.tabs().next_matches(KeyEvent {
            code: Key::Char('l'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.line_up_matches(KeyEvent {
            code: Key::Char('k'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.line_down_matches(KeyEvent {
            code: Key::Char('j'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.line_left_matches(KeyEvent {
            code: Key::Char('h'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.line_right_matches(KeyEvent {
            code: Key::Char('l'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.page_up_matches(KeyEvent {
            code: Key::Char('u'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.page_down_matches(KeyEvent {
            code: Key::Char('d'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.home_matches(KeyEvent {
            code: Key::Char('g'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.end_matches(KeyEvent {
            code: Key::Char('G'),
            modifiers: KeyModifiers::NONE,
        }));
    }
}
