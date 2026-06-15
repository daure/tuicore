use std::{fmt, fs, num::NonZeroU32, path::PathBuf, str::FromStr, time::Duration};

use crate::animation::{AnimationSettings, Easing};
use crate::keybindings::config_dir;
use crate::scroll::{ScrollPreset, ScrollbarGutter, ScrollbarStyle, ScrollbarVisibility};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabsVariant {
    Minimal,
    Underline,
    Boxed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderKind {
    Plain,
    Rounded,
    Double,
    Thick,
}

impl Default for BorderKind {
    fn default() -> Self {
        Self::Rounded
    }
}

impl FromStr for BorderKind {
    type Err = PresetError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "plain" => Ok(Self::Plain),
            "rounded" => Ok(Self::Rounded),
            "double" => Ok(Self::Double),
            "thick" => Ok(Self::Thick),
            other => Err(PresetError(format!("Unknown border `{other}`"))),
        }
    }
}

impl Default for TabsVariant {
    fn default() -> Self {
        Self::Boxed
    }
}

impl FromStr for TabsVariant {
    type Err = PresetError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "minimal" => Ok(Self::Minimal),
            "underline" => Ok(Self::Underline),
            "boxed" => Ok(Self::Boxed),
            other => Err(PresetError(format!("Unknown tabs variant `{other}`"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Preset {
    border: BorderKind,
    tabs: TabsPreset,
    data_view: DataViewPreset,
    scroll: ScrollPreset,
    animation: AnimationSettings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabsPreset {
    variant: TabsVariant,
    bordered: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataViewPreset {
    tree_indent_width: usize,
}

impl Default for TabsPreset {
    fn default() -> Self {
        Self {
            variant: TabsVariant::default(),
            bordered: true,
        }
    }
}

impl Default for DataViewPreset {
    fn default() -> Self {
        Self {
            tree_indent_width: 2,
        }
    }
}

impl Preset {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_border(mut self, border: BorderKind) -> Self {
        self.border = border;
        self
    }

    pub fn with_tabs(mut self, tabs: TabsPreset) -> Self {
        self.tabs = tabs;
        self
    }

    pub fn with_data_view(mut self, data_view: DataViewPreset) -> Self {
        self.data_view = data_view;
        self
    }

    pub fn with_scroll(mut self, scroll: ScrollPreset) -> Self {
        self.scroll = scroll;
        self
    }

    pub fn with_animation(mut self, animation: AnimationSettings) -> Self {
        self.animation = animation;
        self
    }

    pub fn load() -> Result<Self, PresetError> {
        let Some(path) = preset_path() else {
            return Ok(Self::default());
        };
        Self::load_from_path(path)
    }

    pub fn load_from_path(path: impl Into<PathBuf>) -> Result<Self, PresetError> {
        match fs::read_to_string(path.into()) {
            Ok(text) => Self::from_toml_str(&text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(PresetError(format!(
                "Preset config could not be opened: {error}"
            ))),
        }
    }

    pub fn from_toml_str(text: &str) -> Result<Self, PresetError> {
        let file = text
            .parse::<toml::Table>()
            .map_err(|error| PresetError(format!("Preset config could not be read: {error}")))?;
        let mut preset = Self::default();

        if let Some(value) = file
            .get("preset")
            .and_then(|section| section.get("border"))
            .and_then(toml::Value::as_str)
        {
            preset.border = BorderKind::from_str(value)?;
        }

        if let Some(value) = file
            .get("preset")
            .and_then(|section| section.get("tabs"))
            .and_then(|section| section.get("variant"))
            .and_then(toml::Value::as_str)
        {
            preset.tabs.variant = TabsVariant::from_str(value)?;
        }

        if let Some(value) = file
            .get("preset")
            .and_then(|section| section.get("tabs"))
            .and_then(|section| section.get("bordered"))
            .and_then(toml::Value::as_bool)
        {
            preset.tabs.bordered = value;
        }

        if let Some(value) = file
            .get("preset")
            .and_then(|section| section.get("data_view"))
            .and_then(|section| section.get("tree_indent_width"))
            .and_then(toml::Value::as_integer)
            .and_then(|value| usize::try_from(value).ok())
        {
            preset.data_view.tree_indent_width = value;
        }

        if let Some(animation) = file
            .get("preset")
            .and_then(|section| section.get("animation"))
        {
            if let Some(enabled) = animation.get("enabled").and_then(toml::Value::as_bool) {
                preset.animation.enabled = enabled;
            }
            if let Some(fps) = animation
                .get("target_fps")
                .and_then(toml::Value::as_integer)
                .and_then(|value| u32::try_from(value).ok())
                .and_then(NonZeroU32::new)
            {
                preset.animation.target_fps =
                    fps.min(NonZeroU32::new(240).expect("240 is non-zero"));
            }
            if let Some(ms) = animation
                .get("max_dt_ms")
                .and_then(toml::Value::as_integer)
                .and_then(|value| u64::try_from(value).ok())
            {
                preset.animation.max_dt = Duration::from_millis(ms.max(1));
            }
            if let Some(ms) = animation
                .get("default_duration_ms")
                .and_then(toml::Value::as_integer)
                .and_then(|value| u64::try_from(value).ok())
            {
                preset.animation.default_duration = Duration::from_millis(ms);
            }
            if let Some(easing) = animation
                .get("default_easing")
                .and_then(toml::Value::as_str)
                .map(parse_easing)
                .transpose()?
            {
                preset.animation.default_easing = easing;
            }
        }

        if let Some(scroll) = file.get("preset").and_then(|section| section.get("scroll")) {
            if let Some(line_step) = scroll
                .get("line_step")
                .and_then(toml::Value::as_integer)
                .and_then(|value| usize::try_from(value).ok())
            {
                preset.scroll.line_step = line_step;
            }
            if let Some(page_overlap) = scroll
                .get("page_overlap")
                .and_then(toml::Value::as_integer)
                .and_then(|value| usize::try_from(value).ok())
            {
                preset.scroll.page_overlap = page_overlap;
            }
            if let Some(visibility) = scroll
                .get("vertical_scrollbar")
                .and_then(toml::Value::as_str)
                .map(parse_scrollbar_visibility)
                .transpose()?
            {
                preset.scroll.vertical_scrollbar = visibility;
            }
            if let Some(visibility) = scroll
                .get("horizontal_scrollbar")
                .and_then(toml::Value::as_str)
                .map(parse_scrollbar_visibility)
                .transpose()?
            {
                preset.scroll.horizontal_scrollbar = visibility;
            }
            if let Some(gutter) = scroll
                .get("gutter")
                .and_then(toml::Value::as_str)
                .map(parse_scrollbar_gutter)
                .transpose()?
            {
                preset.scroll.gutter = gutter;
            }
            if let Some(style) = scroll
                .get("scrollbar_style")
                .or_else(|| scroll.get("style"))
                .and_then(toml::Value::as_str)
                .map(parse_scrollbar_style)
                .transpose()?
            {
                preset.scroll.style = style;
            }
        }

        Ok(preset)
    }

    pub fn border(&self) -> BorderKind {
        self.border
    }

    pub fn tabs(&self) -> &TabsPreset {
        &self.tabs
    }

    pub fn data_view(&self) -> DataViewPreset {
        self.data_view
    }

    pub fn scroll(&self) -> ScrollPreset {
        self.scroll
    }

    pub fn animation(&self) -> AnimationSettings {
        self.animation
    }
}

impl TabsPreset {
    pub fn new(variant: TabsVariant, bordered: bool) -> Self {
        Self { variant, bordered }
    }

    pub fn with_variant(mut self, variant: TabsVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn with_bordered(mut self, bordered: bool) -> Self {
        self.bordered = bordered;
        self
    }

    pub fn variant(&self) -> TabsVariant {
        self.variant
    }

    pub fn bordered(&self) -> bool {
        self.bordered
    }
}

impl DataViewPreset {
    pub fn new(tree_indent_width: usize) -> Self {
        Self { tree_indent_width }
    }

    pub fn with_tree_indent_width(mut self, tree_indent_width: usize) -> Self {
        self.tree_indent_width = tree_indent_width;
        self
    }

    pub fn tree_indent_width(&self) -> usize {
        self.tree_indent_width
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetError(pub String);

impl fmt::Display for PresetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PresetError {}

fn parse_easing(value: &str) -> Result<Easing, PresetError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "linear" => Ok(Easing::Linear),
        "ease_in_out" | "easeinout" | "ease_in_out_cubic" | "easeinoutcubic" => {
            Ok(Easing::EaseInOut)
        }
        "ease_out_quad" | "easeoutquad" => Ok(Easing::EaseOutQuad),
        "ease_out_cubic" | "easeoutcubic" => Ok(Easing::EaseOutCubic),
        other => Err(PresetError(format!("Unknown easing `{other}`"))),
    }
}

fn parse_scrollbar_visibility(value: &str) -> Result<ScrollbarVisibility, PresetError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "auto" => Ok(ScrollbarVisibility::Auto),
        "always" => Ok(ScrollbarVisibility::Always),
        "never" => Ok(ScrollbarVisibility::Never),
        other => Err(PresetError(format!(
            "Unknown scrollbar visibility `{other}`"
        ))),
    }
}

fn parse_scrollbar_gutter(value: &str) -> Result<ScrollbarGutter, PresetError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "reserve" => Ok(ScrollbarGutter::Reserve),
        "overlay" => Ok(ScrollbarGutter::Overlay),
        other => Err(PresetError(format!("Unknown scrollbar gutter `{other}`"))),
    }
}

fn parse_scrollbar_style(value: &str) -> Result<ScrollbarStyle, PresetError> {
    match value
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_")
        .as_str()
    {
        "thin_track" | "thin" => Ok(ScrollbarStyle::ThinTrack),
        "thick_track" | "thick" => Ok(ScrollbarStyle::ThickTrack),
        other => Err(PresetError(format!("Unknown scrollbar style `{other}`"))),
    }
}

fn preset_path() -> Option<PathBuf> {
    config_dir().map(|path| path.join("tui.toml"))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn preset_parses_scroll_settings_and_clamps_zero_max_dt() {
        let preset = Preset::from_toml_str(
            r#"
            [preset.animation]
            max_dt_ms = 0

            [preset.scroll]
            line_step = 3
            page_overlap = 2
            vertical_scrollbar = "always"
            horizontal_scrollbar = "never"
            gutter = "overlay"
            scrollbar_style = "thick_track"

            [preset.data_view]
            tree_indent_width = 3
            "#,
        )
        .expect("preset should parse");

        let scroll = preset.scroll();
        assert_eq!(scroll.line_step, 3);
        assert_eq!(scroll.page_overlap, 2);
        assert_eq!(scroll.vertical_scrollbar, ScrollbarVisibility::Always);
        assert_eq!(scroll.horizontal_scrollbar, ScrollbarVisibility::Never);
        assert_eq!(scroll.gutter, ScrollbarGutter::Overlay);
        assert_eq!(scroll.style, ScrollbarStyle::ThickTrack);
        assert_eq!(preset.data_view().tree_indent_width(), 3);
        assert_eq!(preset.animation().max_dt, Duration::from_millis(1));
    }

    #[test]
    fn preset_builders_customize_public_settings() {
        let mut animation = AnimationSettings::default();
        animation.enabled = false;
        let scroll = ScrollPreset {
            line_step: 4,
            ..ScrollPreset::default()
        };
        let tabs = TabsPreset::new(TabsVariant::Underline, false);
        let data_view = DataViewPreset::new(4);

        let preset = Preset::new()
            .with_border(BorderKind::Double)
            .with_tabs(tabs.clone())
            .with_data_view(data_view)
            .with_scroll(scroll)
            .with_animation(animation);

        assert_eq!(preset.border(), BorderKind::Double);
        assert_eq!(preset.tabs(), &tabs);
        assert_eq!(preset.data_view(), data_view);
        assert_eq!(preset.scroll().line_step, 4);
        assert!(!preset.animation().enabled);
    }
}
