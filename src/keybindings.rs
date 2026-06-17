use std::{env, fs, io, path::PathBuf};

use crate::event::{Key, KeyEvent, KeyModifiers};

// Large cohesive module; config parsing, defaults, and labels stay aligned.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBindings {
    nav: NavKeyBindings,
    focus: FocusKeyBindings,
    tabs: TabsKeyBindings,
    data_view: DataViewKeyBindings,
    dropdown: DropdownKeyBindings,
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
pub struct FocusKeyBindings {
    next: Vec<KeySpec>,
    previous: Vec<KeySpec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabsKeyBindings {
    previous: KeySpec,
    next: KeySpec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataViewKeyBindings {
    activate: Vec<KeySpec>,
    toggle_selection: Vec<KeySpec>,
    toggle_expansion: Vec<KeySpec>,
    next_page: Vec<KeySpec>,
    previous_page: Vec<KeySpec>,
    collapse_all: Vec<KeySpec>,
    expand_all: Vec<KeySpec>,
    top_prefix: Vec<KeySpec>,
    bottom: Vec<KeySpec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropdownKeyBindings {
    next: Vec<KeySpec>,
    previous: Vec<KeySpec>,
    page_next: Vec<KeySpec>,
    page_previous: Vec<KeySpec>,
    select: Vec<KeySpec>,
}

impl Default for NavKeyBindings {
    fn default() -> Self {
        Self {
            line_up: vec![KeySpec::key(Key::Up), KeySpec::plain('k')],
            line_down: vec![KeySpec::key(Key::Down), KeySpec::plain('j')],
            line_left: vec![KeySpec::key(Key::Left), KeySpec::plain('h')],
            line_right: vec![KeySpec::key(Key::Right), KeySpec::plain('l')],
            page_up: vec![
                KeySpec::key(Key::PageUp),
                KeySpec::key_with_modifiers(Key::Char('u'), KeyModifiers::CONTROL),
            ],
            page_down: vec![
                KeySpec::key(Key::PageDown),
                KeySpec::key_with_modifiers(Key::Char('d'), KeyModifiers::CONTROL),
            ],
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

impl Default for FocusKeyBindings {
    fn default() -> Self {
        Self {
            next: vec![KeySpec::key(Key::Tab)],
            previous: vec![KeySpec::key(Key::BackTab)],
        }
    }
}

impl Default for DataViewKeyBindings {
    fn default() -> Self {
        Self {
            activate: vec![KeySpec::key(Key::Enter)],
            toggle_selection: vec![KeySpec::plain('x')],
            toggle_expansion: vec![KeySpec::plain(' ')],
            next_page: vec![KeySpec::plain('n')],
            previous_page: vec![KeySpec::plain('p')],
            collapse_all: vec![KeySpec::plain('z')],
            expand_all: vec![KeySpec::shifted('z')],
            top_prefix: vec![KeySpec::plain('g')],
            bottom: vec![KeySpec::shifted('g')],
        }
    }
}

impl Default for DropdownKeyBindings {
    fn default() -> Self {
        Self {
            next: vec![KeySpec::key_with_modifiers(
                Key::Char('j'),
                KeyModifiers::CONTROL,
            )],
            previous: vec![KeySpec::key_with_modifiers(
                Key::Char('k'),
                KeyModifiers::CONTROL,
            )],
            page_next: vec![KeySpec::key_with_modifiers(
                Key::Char('d'),
                KeyModifiers::CONTROL,
            )],
            page_previous: vec![KeySpec::key_with_modifiers(
                Key::Char('u'),
                KeyModifiers::CONTROL,
            )],
            select: vec![KeySpec::key_with_modifiers(
                Key::Char(' '),
                KeyModifiers::CONTROL,
            )],
        }
    }
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            nav: NavKeyBindings::default(),
            focus: FocusKeyBindings::default(),
            tabs: TabsKeyBindings::default(),
            data_view: DataViewKeyBindings::default(),
            dropdown: DropdownKeyBindings::default(),
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
        set_keys(&value, "focus", "next", &mut bindings.focus.next);
        set_keys(&value, "focus", "previous", &mut bindings.focus.previous);
        set_key(&value, "tabs", "previous_tab", &mut bindings.tabs.previous);
        set_key(&value, "tabs", "next_tab", &mut bindings.tabs.next);
        set_keys(
            &value,
            "data_view",
            "activate",
            &mut bindings.data_view.activate,
        );
        set_keys(
            &value,
            "data_view",
            "toggle_selection",
            &mut bindings.data_view.toggle_selection,
        );
        set_keys(
            &value,
            "data_view",
            "toggle_expansion",
            &mut bindings.data_view.toggle_expansion,
        );
        set_keys(
            &value,
            "data_view",
            "next_page",
            &mut bindings.data_view.next_page,
        );
        set_keys(
            &value,
            "data_view",
            "previous_page",
            &mut bindings.data_view.previous_page,
        );
        set_keys(
            &value,
            "data_view",
            "collapse_all",
            &mut bindings.data_view.collapse_all,
        );
        set_keys(
            &value,
            "data_view",
            "expand_all",
            &mut bindings.data_view.expand_all,
        );
        set_keys(
            &value,
            "data_view",
            "top_prefix",
            &mut bindings.data_view.top_prefix,
        );
        set_keys(
            &value,
            "data_view",
            "bottom",
            &mut bindings.data_view.bottom,
        );
        set_keys(&value, "dropdown", "next", &mut bindings.dropdown.next);
        set_keys(
            &value,
            "dropdown",
            "previous",
            &mut bindings.dropdown.previous,
        );
        set_keys(
            &value,
            "dropdown",
            "page_next",
            &mut bindings.dropdown.page_next,
        );
        set_keys(
            &value,
            "dropdown",
            "page_previous",
            &mut bindings.dropdown.page_previous,
        );
        set_keys(&value, "dropdown", "select", &mut bindings.dropdown.select);

        bindings
    }

    pub fn tabs(&self) -> &TabsKeyBindings {
        &self.tabs
    }

    pub fn focus(&self) -> &FocusKeyBindings {
        &self.focus
    }

    pub fn data_view(&self) -> &DataViewKeyBindings {
        &self.data_view
    }

    pub fn dropdown(&self) -> &DropdownKeyBindings {
        &self.dropdown
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

    pub fn set_focus_next(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.focus.next = keys.into_iter().collect();
    }

    pub fn with_focus_next(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_focus_next(keys);
        self
    }

    pub fn set_focus_previous(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.focus.previous = keys.into_iter().collect();
    }

    pub fn with_focus_previous(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_focus_previous(keys);
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

    pub fn set_data_view_activate(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.data_view.activate = keys.into_iter().collect();
    }

    pub fn with_data_view_activate(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_data_view_activate(keys);
        self
    }

    pub fn set_data_view_toggle_expansion(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.data_view.toggle_expansion = keys.into_iter().collect();
    }

    pub fn set_data_view_toggle_selection(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.data_view.toggle_selection = keys.into_iter().collect();
    }

    pub fn with_data_view_toggle_selection(
        mut self,
        keys: impl IntoIterator<Item = KeySpec>,
    ) -> Self {
        self.set_data_view_toggle_selection(keys);
        self
    }

    pub fn with_data_view_toggle_expansion(
        mut self,
        keys: impl IntoIterator<Item = KeySpec>,
    ) -> Self {
        self.set_data_view_toggle_expansion(keys);
        self
    }

    pub fn set_data_view_next_page(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.data_view.next_page = keys.into_iter().collect();
    }

    pub fn with_data_view_next_page(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_data_view_next_page(keys);
        self
    }

    pub fn set_data_view_previous_page(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.data_view.previous_page = keys.into_iter().collect();
    }

    pub fn with_data_view_previous_page(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_data_view_previous_page(keys);
        self
    }

    pub fn set_data_view_collapse_all(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.data_view.collapse_all = keys.into_iter().collect();
    }

    pub fn with_data_view_collapse_all(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_data_view_collapse_all(keys);
        self
    }

    pub fn set_data_view_expand_all(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.data_view.expand_all = keys.into_iter().collect();
    }

    pub fn with_data_view_expand_all(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_data_view_expand_all(keys);
        self
    }

    pub fn set_data_view_top_prefix(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.data_view.top_prefix = keys.into_iter().collect();
    }

    pub fn with_data_view_top_prefix(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_data_view_top_prefix(keys);
        self
    }

    pub fn set_data_view_bottom(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.data_view.bottom = keys.into_iter().collect();
    }

    pub fn with_data_view_bottom(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_data_view_bottom(keys);
        self
    }

    pub fn set_dropdown_next(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.dropdown.next = keys.into_iter().collect();
    }

    pub fn with_dropdown_next(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_dropdown_next(keys);
        self
    }

    pub fn set_dropdown_previous(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.dropdown.previous = keys.into_iter().collect();
    }

    pub fn with_dropdown_previous(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_dropdown_previous(keys);
        self
    }

    pub fn set_dropdown_page_next(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.dropdown.page_next = keys.into_iter().collect();
    }

    pub fn with_dropdown_page_next(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_dropdown_page_next(keys);
        self
    }

    pub fn set_dropdown_page_previous(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.dropdown.page_previous = keys.into_iter().collect();
    }

    pub fn with_dropdown_page_previous(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_dropdown_page_previous(keys);
        self
    }

    pub fn set_dropdown_select(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.dropdown.select = keys.into_iter().collect();
    }

    pub fn with_dropdown_select(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_dropdown_select(keys);
        self
    }

    pub fn line_up_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_any(&self.nav.line_up, key.into())
    }

    pub fn line_up_label(&self) -> String {
        labels(&self.nav.line_up)
    }

    pub fn line_down_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_any(&self.nav.line_down, key.into())
    }

    pub fn line_down_label(&self) -> String {
        labels(&self.nav.line_down)
    }

    pub fn line_left_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_any(&self.nav.line_left, key.into())
    }

    pub fn line_right_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_any(&self.nav.line_right, key.into())
    }

    pub fn page_up_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_any(&self.nav.page_up, key.into())
    }

    pub fn page_down_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_any(&self.nav.page_down, key.into())
    }

    pub fn home_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_any(&self.nav.home, key.into())
    }

    pub fn end_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_any(&self.nav.end, key.into())
    }
}

impl TabsKeyBindings {
    pub fn previous_matches(&self, key: impl Into<KeyEvent>) -> bool {
        self.previous.matches(key)
    }

    pub fn next_matches(&self, key: impl Into<KeyEvent>) -> bool {
        self.next.matches(key)
    }

    pub fn previous_label(&self) -> String {
        self.previous.label()
    }

    pub fn next_label(&self) -> String {
        self.next.label()
    }
}

impl FocusKeyBindings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_next(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.next = keys.into_iter().collect();
    }

    pub fn with_next(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_next(keys);
        self
    }

    pub fn set_previous(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.previous = keys.into_iter().collect();
    }

    pub fn with_previous(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_previous(keys);
        self
    }

    pub fn next_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_any(&self.next, key.into())
    }

    pub fn previous_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_any(&self.previous, key.into())
    }
}

impl DataViewKeyBindings {
    pub fn activate_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_any(&self.activate, key.into())
    }

    pub fn activate_label(&self) -> String {
        labels(&self.activate)
    }

    pub fn toggle_expansion_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_any(&self.toggle_expansion, key.into())
    }

    pub fn toggle_expansion_label(&self) -> String {
        labels(&self.toggle_expansion)
    }

    pub fn toggle_selection_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_any(&self.toggle_selection, key.into())
    }

    pub fn toggle_selection_label(&self) -> String {
        labels(&self.toggle_selection)
    }

    pub fn next_page_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_any(&self.next_page, key.into())
    }

    pub fn previous_page_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_any(&self.previous_page, key.into())
    }

    pub fn collapse_all_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_any(&self.collapse_all, key.into())
    }

    pub fn collapse_all_label(&self) -> String {
        labels(&self.collapse_all)
    }

    pub fn expand_all_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_any(&self.expand_all, key.into())
    }

    pub fn expand_all_label(&self) -> String {
        labels(&self.expand_all)
    }

    pub fn top_prefix_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_any(&self.top_prefix, key.into())
    }

    pub fn bottom_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_any(&self.bottom, key.into())
    }
}

impl DropdownKeyBindings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_next(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.next = keys.into_iter().collect();
    }

    pub fn with_next(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_next(keys);
        self
    }

    pub fn set_previous(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.previous = keys.into_iter().collect();
    }

    pub fn with_previous(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_previous(keys);
        self
    }

    pub fn set_page_next(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.page_next = keys.into_iter().collect();
    }

    pub fn with_page_next(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_page_next(keys);
        self
    }

    pub fn set_page_previous(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.page_previous = keys.into_iter().collect();
    }

    pub fn with_page_previous(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_page_previous(keys);
        self
    }

    pub fn set_select(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.select = keys.into_iter().collect();
    }

    pub fn with_select(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_select(keys);
        self
    }

    pub fn next_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_any(&self.next, key.into())
    }

    pub fn previous_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_any(&self.previous, key.into())
    }

    pub fn page_next_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_any(&self.page_next, key.into())
    }

    pub fn page_previous_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_any(&self.page_previous, key.into())
    }

    pub fn select_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_any(&self.select, key.into())
    }

    pub fn next_label(&self) -> String {
        labels(&self.next)
    }

    pub fn previous_label(&self) -> String {
        labels(&self.previous)
    }

    pub fn page_next_label(&self) -> String {
        labels(&self.page_next)
    }

    pub fn page_previous_label(&self) -> String {
        labels(&self.page_previous)
    }

    pub fn select_label(&self) -> String {
        labels(&self.select)
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

    pub fn matches(self, key: impl Into<KeyEvent>) -> bool {
        let key = key.into();
        if self.code == Key::BackTab && key.code == Key::BackTab {
            return if self.modifiers == KeyModifiers::NONE {
                key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT
            } else {
                key.modifiers == self.modifiers
            };
        }

        if let KeySpec {
            code: Key::Char(expected),
            modifiers,
        } = self
            && modifiers == KeyModifiers::CONTROL
            && expected.is_ascii_lowercase()
            && let KeySpec {
                code: Key::Char(actual),
                modifiers: actual_modifiers,
            } = KeySpec::from(key)
        {
            let tolerated_uppercase_report = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
            return (actual_modifiers == KeyModifiers::CONTROL
                || actual_modifiers == tolerated_uppercase_report)
                && actual == expected;
        }

        if self == KeySpec::from(key) {
            return true;
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

fn labels(bindings: &[KeySpec]) -> String {
    bindings
        .iter()
        .map(|binding| binding.label())
        .collect::<Vec<_>>()
        .join("/")
}

fn parse_key(value: &str) -> Option<KeySpec> {
    let value = value.trim().to_ascii_lowercase();

    if let Some(rest) = value.strip_prefix("ctrl+") {
        return modified_key(rest, KeyModifiers::CONTROL);
    }

    if let Some(rest) = value.strip_prefix("alt+") {
        return modified_key(rest, KeyModifiers::ALT);
    }

    if let Some(rest) = value.strip_prefix("shift+") {
        if rest == "tab" || rest == "backtab" {
            return Some(KeySpec::key(Key::BackTab));
        }
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

fn modified_key(value: &str, modifiers: KeyModifiers) -> Option<KeySpec> {
    if value == "space" {
        return Some(KeySpec::key_with_modifiers(Key::Char(' '), modifiers));
    }
    single_char(value).map(|key| KeySpec::key_with_modifiers(Key::Char(key), modifiers))
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

            [data_view]
            activate = "enter"
            toggle_selection = "x"
            toggle_expansion = "space"
            next_page = "n"
            previous_page = "p"
            collapse_all = "z"
            expand_all = "shift+z"
            top_prefix = "g"
            bottom = "shift+g"

            [dropdown]
            next = "ctrl+j"
            previous = "ctrl+k"
            page_next = "ctrl+d"
            page_previous = "ctrl+u"
            select = "ctrl+space"
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
        assert!(bindings.data_view().activate_matches(KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.data_view().toggle_selection_matches(KeyEvent {
            code: Key::Char('x'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.data_view().toggle_expansion_matches(KeyEvent {
            code: Key::Char(' '),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.data_view().next_page_matches(KeyEvent {
            code: Key::Char('n'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.data_view().previous_page_matches(KeyEvent {
            code: Key::Char('p'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.data_view().collapse_all_matches(KeyEvent {
            code: Key::Char('z'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.data_view().expand_all_matches(KeyEvent {
            code: Key::Char('Z'),
            modifiers: KeyModifiers::SHIFT,
        }));
        assert!(bindings.data_view().top_prefix_matches(KeyEvent {
            code: Key::Char('g'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.data_view().bottom_matches(KeyEvent {
            code: Key::Char('G'),
            modifiers: KeyModifiers::SHIFT,
        }));
        assert!(bindings.dropdown().next_matches(KeyEvent {
            code: Key::Char('j'),
            modifiers: KeyModifiers::CONTROL,
        }));
        assert!(bindings.dropdown().previous_matches(KeyEvent {
            code: Key::Char('k'),
            modifiers: KeyModifiers::CONTROL,
        }));
        assert!(bindings.dropdown().page_next_matches(KeyEvent {
            code: Key::Char('d'),
            modifiers: KeyModifiers::CONTROL,
        }));
        assert!(bindings.dropdown().page_previous_matches(KeyEvent {
            code: Key::Char('u'),
            modifiers: KeyModifiers::CONTROL,
        }));
        assert!(bindings.dropdown().select_matches(KeyEvent {
            code: Key::Char(' '),
            modifiers: KeyModifiers::CONTROL,
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
            .with_nav_end([KeySpec::shifted('g')])
            .with_data_view_activate([KeySpec::plain('a')])
            .with_data_view_toggle_selection([KeySpec::plain('s')])
            .with_data_view_toggle_expansion([KeySpec::plain('e')])
            .with_data_view_next_page([KeySpec::plain('n')])
            .with_data_view_previous_page([KeySpec::plain('p')])
            .with_data_view_collapse_all([KeySpec::plain('c')])
            .with_data_view_expand_all([KeySpec::plain('x')])
            .with_data_view_top_prefix([KeySpec::plain('t')])
            .with_data_view_bottom([KeySpec::plain('b')])
            .with_dropdown_next([KeySpec::plain('j')])
            .with_dropdown_previous([KeySpec::plain('k')])
            .with_dropdown_page_next([KeySpec::plain('d')])
            .with_dropdown_page_previous([KeySpec::plain('u')])
            .with_dropdown_select([KeySpec::plain(' ')]);

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
        assert!(bindings.data_view().activate_matches(KeyEvent {
            code: Key::Char('a'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.data_view().toggle_selection_matches(KeyEvent {
            code: Key::Char('s'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.data_view().toggle_expansion_matches(KeyEvent {
            code: Key::Char('e'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.data_view().next_page_matches(KeyEvent {
            code: Key::Char('n'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.data_view().previous_page_matches(KeyEvent {
            code: Key::Char('p'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.data_view().collapse_all_matches(KeyEvent {
            code: Key::Char('c'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.data_view().expand_all_matches(KeyEvent {
            code: Key::Char('x'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.data_view().top_prefix_matches(KeyEvent {
            code: Key::Char('t'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.data_view().bottom_matches(KeyEvent {
            code: Key::Char('b'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.dropdown().next_matches(KeyEvent {
            code: Key::Char('j'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.dropdown().previous_matches(KeyEvent {
            code: Key::Char('k'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.dropdown().page_next_matches(KeyEvent {
            code: Key::Char('d'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.dropdown().page_previous_matches(KeyEvent {
            code: Key::Char('u'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.dropdown().select_matches(KeyEvent {
            code: Key::Char(' '),
            modifiers: KeyModifiers::NONE,
        }));
    }

    #[test]
    fn default_dropdown_bindings_split_line_page_and_select_actions() {
        let bindings = KeyBindings::default();

        assert!(bindings.dropdown().next_matches(KeyEvent {
            code: Key::Char('j'),
            modifiers: KeyModifiers::CONTROL,
        }));
        assert!(!bindings.dropdown().next_matches(KeyEvent {
            code: Key::Char('d'),
            modifiers: KeyModifiers::CONTROL,
        }));
        assert!(bindings.dropdown().previous_matches(KeyEvent {
            code: Key::Char('k'),
            modifiers: KeyModifiers::CONTROL,
        }));
        assert!(!bindings.dropdown().previous_matches(KeyEvent {
            code: Key::Char('u'),
            modifiers: KeyModifiers::CONTROL,
        }));
        assert!(bindings.dropdown().page_next_matches(KeyEvent {
            code: Key::Char('d'),
            modifiers: KeyModifiers::CONTROL,
        }));
        assert!(bindings.dropdown().page_previous_matches(KeyEvent {
            code: Key::Char('u'),
            modifiers: KeyModifiers::CONTROL,
        }));
        assert!(bindings.dropdown().select_matches(KeyEvent {
            code: Key::Char(' '),
            modifiers: KeyModifiers::CONTROL,
        }));
    }

    #[test]
    fn focus_bindings_can_be_built_directly() {
        let bindings = FocusKeyBindings::new()
            .with_next([KeySpec::plain('l')])
            .with_previous([KeySpec::plain('h')]);

        assert!(bindings.next_matches(KeyEvent {
            code: Key::Char('l'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.previous_matches(KeyEvent {
            code: Key::Char('h'),
            modifiers: KeyModifiers::NONE,
        }));
        assert!(!bindings.next_matches(KeyEvent {
            code: Key::Tab,
            modifiers: KeyModifiers::NONE,
        }));
    }

    #[test]
    fn default_focus_previous_matches_backtab_with_or_without_shift_modifier() {
        let bindings = FocusKeyBindings::default();

        assert!(bindings.previous_matches(KeyEvent {
            code: Key::BackTab,
            modifiers: KeyModifiers::NONE,
        }));
        assert!(bindings.previous_matches(KeyEvent {
            code: Key::BackTab,
            modifiers: KeyModifiers::SHIFT,
        }));
        assert!(!bindings.previous_matches(KeyEvent {
            code: Key::BackTab,
            modifiers: KeyModifiers::ALT,
        }));
    }

    #[test]
    fn shift_tab_config_maps_to_backtab() {
        let bindings = KeyBindings::from_toml_str(
            r#"
            [focus]
            previous = "shift+tab"
            "#,
        );

        assert!(bindings.focus().previous_matches(KeyEvent {
            code: Key::BackTab,
            modifiers: KeyModifiers::SHIFT,
        }));
    }

    #[test]
    fn ctrl_bindings_reject_unrelated_modifiers() {
        let binding = KeySpec::key_with_modifiers(Key::Char('u'), KeyModifiers::CONTROL);

        assert!(binding.matches(KeyEvent {
            code: Key::Char('U'),
            modifiers: KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        }));
        assert!(!binding.matches(KeyEvent {
            code: Key::Char('u'),
            modifiers: KeyModifiers::CONTROL | KeyModifiers::ALT,
        }));
        assert!(!binding.matches(KeyEvent {
            code: Key::Char('U'),
            modifiers: KeyModifiers::CONTROL | KeyModifiers::SHIFT | KeyModifiers::ALT,
        }));
    }
}
