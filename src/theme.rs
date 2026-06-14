use std::{collections::BTreeMap, fmt, fs, path::PathBuf, str::FromStr};

use tuirealm::ratatui::style::Color;

use crate::keybindings::config_dir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeName {
    Amoled,
    Catppuccin,
    Github,
    Gruvbox,
    RosePine,
    Solarized,
    TokyoNight,
}

impl ThemeName {
    pub const ALL: [Self; 7] = [
        Self::Amoled,
        Self::Catppuccin,
        Self::Github,
        Self::Gruvbox,
        Self::RosePine,
        Self::Solarized,
        Self::TokyoNight,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Amoled => "amoled",
            Self::Catppuccin => "catppuccin",
            Self::Github => "github",
            Self::Gruvbox => "gruvbox",
            Self::RosePine => "rosepine",
            Self::Solarized => "solarized",
            Self::TokyoNight => "tokyonight",
        }
    }
}

impl Default for ThemeName {
    fn default() -> Self {
        Self::TokyoNight
    }
}

impl FromStr for ThemeName {
    type Err = ThemeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "amoled" => Ok(Self::Amoled),
            "catppuccin" | "catppuccin_mocha" | "catpuccin" => Ok(Self::Catppuccin),
            "github" => Ok(Self::Github),
            "gruvbox" | "gruvbox_dark" => Ok(Self::Gruvbox),
            "rose_pine" | "rosepine" => Ok(Self::RosePine),
            "solarized" | "solarized_dark" => Ok(Self::Solarized),
            "tokyo_night" | "tokyonight" | "tira_dark" => Ok(Self::TokyoNight),
            other => Err(ThemeError(format!("Unknown theme `{other}`"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    name: ThemeName,
    selected_fg: Color,
    selected_bg: Color,
    muted_fg: Color,
    subtle_fg: Color,
    accent_fg: Color,
    success_fg: Color,
    error_fg: Color,
    border_fg: Color,
    highlight_fg: Color,
    highlight_bg: Color,
    key_fg: Color,
    warning_fg: Color,
    overrides: BTreeMap<String, String>,
}

impl Default for Theme {
    fn default() -> Self {
        Self::named(ThemeName::default())
    }
}

impl Theme {
    pub fn named(name: ThemeName) -> Self {
        let palette = palette_for(name);
        Self {
            name,
            selected_fg: palette.green,
            selected_bg: palette.surface,
            muted_fg: palette.muted,
            subtle_fg: palette.subtle,
            accent_fg: palette.cyan,
            success_fg: palette.green,
            error_fg: palette.red,
            border_fg: palette.border,
            highlight_fg: palette.base,
            highlight_bg: palette.yellow,
            key_fg: palette.blue,
            warning_fg: palette.yellow,
            overrides: BTreeMap::new(),
        }
    }

    pub fn load() -> Result<Self, ThemeError> {
        let Some(path) = theme_path() else {
            return Ok(Self::default());
        };
        Self::load_from_path(path)
    }

    pub fn load_from_path(path: impl Into<PathBuf>) -> Result<Self, ThemeError> {
        match fs::read_to_string(path.into()) {
            Ok(text) => Self::from_toml_str(&text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(ThemeError(format!(
                "Theme config could not be opened: {error}"
            ))),
        }
    }

    pub fn from_toml_str(text: &str) -> Result<Self, ThemeError> {
        let file = text
            .parse::<toml::Table>()
            .map_err(|error| ThemeError(format!("Theme config could not be read: {error}")))?;
        let name = file
            .get("theme")
            .and_then(toml::Value::as_str)
            .map(ThemeName::from_str)
            .transpose()?
            .unwrap_or_default();
        let mut theme = Self::named(name);
        if let Some(colors) = file.get("colors").and_then(toml::Value::as_table) {
            for (role, value) in colors {
                if let Some(value) = value.as_str() {
                    theme.set_role(role, value)?;
                }
            }
        }
        Ok(theme)
    }

    pub fn name(&self) -> ThemeName {
        self.name
    }
    pub fn selected_fg(&self) -> Color {
        self.selected_fg
    }
    pub fn selected_bg(&self) -> Color {
        self.selected_bg
    }
    pub fn muted_fg(&self) -> Color {
        self.muted_fg
    }
    pub fn subtle_fg(&self) -> Color {
        self.subtle_fg
    }
    pub fn accent_fg(&self) -> Color {
        self.accent_fg
    }
    pub fn success_fg(&self) -> Color {
        self.success_fg
    }
    pub fn error_fg(&self) -> Color {
        self.error_fg
    }
    pub fn border_fg(&self) -> Color {
        self.border_fg
    }
    pub fn highlight_fg(&self) -> Color {
        self.highlight_fg
    }
    pub fn highlight_bg(&self) -> Color {
        self.highlight_bg
    }
    pub fn key_fg(&self) -> Color {
        self.key_fg
    }
    pub fn warning_fg(&self) -> Color {
        self.warning_fg
    }

    pub fn set_role(&mut self, role: &str, value: &str) -> Result<(), ThemeError> {
        let color = parse_hex_color(value)?;
        match role {
            "selected_fg" => self.selected_fg = color,
            "selected_bg" => self.selected_bg = color,
            "muted_fg" => self.muted_fg = color,
            "subtle_fg" => self.subtle_fg = color,
            "accent_fg" => self.accent_fg = color,
            "success_fg" => self.success_fg = color,
            "error_fg" => self.error_fg = color,
            "border_fg" => self.border_fg = color,
            "highlight_fg" => self.highlight_fg = color,
            "highlight_bg" => self.highlight_bg = color,
            "key_fg" => self.key_fg = color,
            "warning_fg" => self.warning_fg = color,
            _ => return Err(ThemeError(format!("Unknown theme role `{role}`"))),
        }
        self.overrides.insert(role.to_string(), value.to_string());
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeError(pub String);

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ThemeError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Palette {
    base: Color,
    surface: Color,
    border: Color,
    text: Color,
    subtle: Color,
    muted: Color,
    blue: Color,
    cyan: Color,
    green: Color,
    yellow: Color,
    red: Color,
}

fn palette_for(name: ThemeName) -> Palette {
    match name {
        ThemeName::Amoled => palette(
            [0, 0, 0],
            [12, 12, 12],
            [48, 48, 48],
            [238, 238, 238],
            [180, 180, 180],
            [120, 120, 120],
            [122, 162, 247],
            [125, 207, 255],
            [158, 206, 106],
            [224, 175, 104],
            [247, 118, 142],
        ),
        ThemeName::Catppuccin => palette(
            [30, 30, 46],
            [49, 50, 68],
            [69, 71, 90],
            [205, 214, 244],
            [166, 173, 200],
            [108, 112, 134],
            [137, 180, 250],
            [137, 220, 235],
            [166, 227, 161],
            [249, 226, 175],
            [243, 139, 168],
        ),
        ThemeName::Github => palette(
            [13, 17, 23],
            [22, 27, 34],
            [48, 54, 61],
            [230, 237, 243],
            [139, 148, 158],
            [110, 118, 129],
            [88, 166, 255],
            [121, 192, 255],
            [63, 185, 80],
            [210, 153, 34],
            [248, 81, 73],
        ),
        ThemeName::Gruvbox => palette(
            [40, 40, 40],
            [60, 56, 54],
            [80, 73, 69],
            [235, 219, 178],
            [189, 174, 147],
            [146, 131, 116],
            [131, 165, 152],
            [142, 192, 124],
            [184, 187, 38],
            [250, 189, 47],
            [251, 73, 52],
        ),
        ThemeName::RosePine => palette(
            [25, 23, 36],
            [31, 29, 46],
            [64, 61, 82],
            [224, 222, 244],
            [144, 140, 170],
            [110, 106, 134],
            [49, 116, 143],
            [156, 207, 216],
            [196, 167, 231],
            [246, 193, 119],
            [235, 111, 146],
        ),
        ThemeName::Solarized => palette(
            [0, 43, 54],
            [7, 54, 66],
            [88, 110, 117],
            [238, 232, 213],
            [147, 161, 161],
            [101, 123, 131],
            [38, 139, 210],
            [42, 161, 152],
            [133, 153, 0],
            [181, 137, 0],
            [220, 50, 47],
        ),
        ThemeName::TokyoNight => palette(
            [26, 27, 38],
            [41, 46, 66],
            [65, 72, 104],
            [192, 202, 245],
            [169, 177, 214],
            [86, 95, 137],
            [122, 162, 247],
            [125, 207, 255],
            [158, 206, 106],
            [224, 175, 104],
            [247, 118, 142],
        ),
    }
}

fn palette(
    base: [u8; 3],
    surface: [u8; 3],
    border: [u8; 3],
    text: [u8; 3],
    subtle: [u8; 3],
    muted: [u8; 3],
    blue: [u8; 3],
    cyan: [u8; 3],
    green: [u8; 3],
    yellow: [u8; 3],
    red: [u8; 3],
) -> Palette {
    Palette {
        base: rgb(base),
        surface: rgb(surface),
        border: rgb(border),
        text: rgb(text),
        subtle: rgb(subtle),
        muted: rgb(muted),
        blue: rgb(blue),
        cyan: rgb(cyan),
        green: rgb(green),
        yellow: rgb(yellow),
        red: rgb(red),
    }
}

const fn rgb(value: [u8; 3]) -> Color {
    Color::Rgb(value[0], value[1], value[2])
}

fn parse_hex_color(value: &str) -> Result<Color, ThemeError> {
    let value = value.trim();
    let hex = value
        .strip_prefix('#')
        .ok_or_else(|| ThemeError(format!("Theme color `{value}` must start with #")))?;
    if hex.len() != 6 {
        return Err(ThemeError(format!("Theme color `{value}` must be #RRGGBB")));
    }
    let red = parse_hex_pair(&hex[0..2], value)?;
    let green = parse_hex_pair(&hex[2..4], value)?;
    let blue = parse_hex_pair(&hex[4..6], value)?;
    Ok(Color::Rgb(red, green, blue))
}

fn parse_hex_pair(pair: &str, original: &str) -> Result<u8, ThemeError> {
    u8::from_str_radix(pair, 16)
        .map_err(|_| ThemeError(format!("Theme color `{original}` contains invalid hex")))
}

fn theme_path() -> Option<PathBuf> {
    config_dir().map(|path| path.join("tui.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_role_updates_public_theme_color() {
        let mut theme = Theme::default();

        theme
            .set_role("accent_fg", "#112233")
            .expect("role should update");

        assert_eq!(theme.accent_fg(), Color::Rgb(0x11, 0x22, 0x33));
    }
}
