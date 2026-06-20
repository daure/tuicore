use std::{
    fmt,
    path::Path,
    sync::{OnceLock, RwLock},
};

use crate::preset::PresetError;
use crate::theme::ThemeError;
use crate::{AnimationSettings, KeyBindings, KeyBindingsError, Preset, Theme};

#[derive(Debug, Clone, Default)]
struct UiSettings {
    theme: Theme,
    keybindings: KeyBindings,
    preset: Preset,
}

static SETTINGS: OnceLock<RwLock<UiSettings>> = OnceLock::new();

pub fn init() {
    replace_settings(UiSettings {
        theme: Theme::load().unwrap_or_default(),
        keybindings: KeyBindings::load(),
        preset: Preset::load().unwrap_or_default(),
    });
}

pub fn try_init() -> Result<(), UiInitError> {
    replace_settings(UiSettings {
        theme: Theme::load()?,
        keybindings: KeyBindings::try_load()?,
        preset: Preset::load()?,
    });
    Ok(())
}

pub fn init_from_dir(path: impl AsRef<Path>) {
    let path = path.as_ref();
    replace_settings(UiSettings {
        theme: Theme::load_from_path(path.join("tui.toml")).unwrap_or_default(),
        keybindings: KeyBindings::load_from_path(path.join("keybindings.toml")).unwrap_or_default(),
        preset: Preset::load_from_path(path.join("tui.toml")).unwrap_or_default(),
    });
}

pub fn try_init_from_dir(path: impl AsRef<Path>) -> Result<(), UiInitError> {
    let path = path.as_ref();
    replace_settings(UiSettings {
        theme: Theme::load_from_path(path.join("tui.toml"))?,
        keybindings: KeyBindings::try_load_from_path(path.join("keybindings.toml"))?,
        preset: Preset::load_from_path(path.join("tui.toml"))?,
    });
    Ok(())
}

pub fn theme() -> Theme {
    settings()
        .read()
        .expect("tuicore settings lock poisoned")
        .theme
        .clone()
}

pub fn keybindings() -> KeyBindings {
    settings()
        .read()
        .expect("tuicore settings lock poisoned")
        .keybindings
        .clone()
}

pub fn preset() -> Preset {
    settings()
        .read()
        .expect("tuicore settings lock poisoned")
        .preset
        .clone()
}

pub fn animation_settings() -> AnimationSettings {
    preset().animation()
}

pub fn set_theme(theme: Theme) {
    settings()
        .write()
        .expect("tuicore settings lock poisoned")
        .theme = theme;
}

pub fn set_keybindings(keybindings: KeyBindings) {
    settings()
        .write()
        .expect("tuicore settings lock poisoned")
        .keybindings = keybindings;
}

pub fn set_preset(preset: Preset) {
    settings()
        .write()
        .expect("tuicore settings lock poisoned")
        .preset = preset;
}

fn settings() -> &'static RwLock<UiSettings> {
    SETTINGS.get_or_init(|| RwLock::new(UiSettings::default()))
}

fn replace_settings(next: UiSettings) {
    *settings().write().expect("tuicore settings lock poisoned") = next;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiInitError {
    Theme(ThemeError),
    KeyBindings(KeyBindingsError),
    Preset(PresetError),
}

impl fmt::Display for UiInitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Theme(error) => write!(f, "{error}"),
            Self::KeyBindings(error) => write!(f, "{error}"),
            Self::Preset(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for UiInitError {}

impl From<ThemeError> for UiInitError {
    fn from(value: ThemeError) -> Self {
        Self::Theme(value)
    }
}

impl From<KeyBindingsError> for UiInitError {
    fn from(value: KeyBindingsError) -> Self {
        Self::KeyBindings(value)
    }
}

impl From<PresetError> for UiInitError {
    fn from(value: PresetError) -> Self {
        Self::Preset(value)
    }
}
