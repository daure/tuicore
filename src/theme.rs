use std::{collections::BTreeMap, fmt, fs, path::PathBuf, str::FromStr};

use ratatui::style::Color;

use crate::keybindings::config_dir;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThemeName {
    Amoled,
    Aura,
    Ayu,
    Carbonfox,
    Catppuccin,
    CatppuccinFrappe,
    CatppuccinMacchiato,
    Cobalt2,
    Cursor,
    Dracula,
    Everforest,
    Flexoki,
    Github,
    Gruvbox,
    Kanagawa,
    LucentOrng,
    Material,
    Matrix,
    Mercury,
    Monokai,
    NightOwl,
    Nord,
    Oc2,
    OneDark,
    Onedarkpro,
    Opencode,
    Orng,
    OsakaJade,
    Palenight,
    RosePine,
    ShadesOfPurple,
    Solarized,
    Synthwave84,
    TokyoNight,
    Vercel,
    Vesper,
    Zenburn,
}

impl ThemeName {
    pub const ALL: [Self; 37] = [
        Self::Amoled,
        Self::Aura,
        Self::Ayu,
        Self::Carbonfox,
        Self::Catppuccin,
        Self::CatppuccinFrappe,
        Self::CatppuccinMacchiato,
        Self::Cobalt2,
        Self::Cursor,
        Self::Dracula,
        Self::Everforest,
        Self::Flexoki,
        Self::Github,
        Self::Gruvbox,
        Self::Kanagawa,
        Self::LucentOrng,
        Self::Material,
        Self::Matrix,
        Self::Mercury,
        Self::Monokai,
        Self::NightOwl,
        Self::Nord,
        Self::Oc2,
        Self::OneDark,
        Self::Onedarkpro,
        Self::Opencode,
        Self::Orng,
        Self::OsakaJade,
        Self::Palenight,
        Self::RosePine,
        Self::ShadesOfPurple,
        Self::Solarized,
        Self::Synthwave84,
        Self::TokyoNight,
        Self::Vercel,
        Self::Vesper,
        Self::Zenburn,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Amoled => "amoled",
            Self::Aura => "aura",
            Self::Ayu => "ayu",
            Self::Carbonfox => "carbonfox",
            Self::Catppuccin => "catppuccin",
            Self::CatppuccinFrappe => "catppuccin_frappe",
            Self::CatppuccinMacchiato => "catppuccin_macchiato",
            Self::Cobalt2 => "cobalt2",
            Self::Cursor => "cursor",
            Self::Dracula => "dracula",
            Self::Everforest => "everforest",
            Self::Flexoki => "flexoki",
            Self::Github => "github",
            Self::Gruvbox => "gruvbox",
            Self::Kanagawa => "kanagawa",
            Self::LucentOrng => "lucent_orng",
            Self::Material => "material",
            Self::Matrix => "matrix",
            Self::Mercury => "mercury",
            Self::Monokai => "monokai",
            Self::NightOwl => "nightowl",
            Self::Nord => "nord",
            Self::Oc2 => "oc_2",
            Self::OneDark => "one_dark",
            Self::Onedarkpro => "onedarkpro",
            Self::Opencode => "opencode",
            Self::Orng => "orng",
            Self::OsakaJade => "osaka_jade",
            Self::Palenight => "palenight",
            Self::RosePine => "rosepine",
            Self::ShadesOfPurple => "shadesofpurple",
            Self::Solarized => "solarized",
            Self::Synthwave84 => "synthwave84",
            Self::TokyoNight => "tokyonight",
            Self::Vercel => "vercel",
            Self::Vesper => "vesper",
            Self::Zenburn => "zenburn",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Amoled => "Amoled",
            Self::Aura => "Aura",
            Self::Ayu => "Ayu",
            Self::Carbonfox => "Carbonfox",
            Self::Catppuccin => "Catppuccin",
            Self::CatppuccinFrappe => "Catppuccin Frappé",
            Self::CatppuccinMacchiato => "Catppuccin Macchiato",
            Self::Cobalt2 => "Cobalt2",
            Self::Cursor => "Cursor",
            Self::Dracula => "Dracula",
            Self::Everforest => "Everforest",
            Self::Flexoki => "Flexoki",
            Self::Github => "GitHub",
            Self::Gruvbox => "Gruvbox",
            Self::Kanagawa => "Kanagawa",
            Self::LucentOrng => "Lucent Orng",
            Self::Material => "Material",
            Self::Matrix => "Matrix",
            Self::Mercury => "Mercury",
            Self::Monokai => "Monokai",
            Self::NightOwl => "Night Owl",
            Self::Nord => "Nord",
            Self::Oc2 => "OC-2",
            Self::OneDark => "One Dark",
            Self::Onedarkpro => "OneDark Pro",
            Self::Opencode => "Opencode",
            Self::Orng => "Orng",
            Self::OsakaJade => "Osaka Jade",
            Self::Palenight => "Palenight",
            Self::RosePine => "Rosé Pine",
            Self::ShadesOfPurple => "Shades of Purple",
            Self::Solarized => "Solarized",
            Self::Synthwave84 => "Synthwave '84",
            Self::TokyoNight => "Tokyo Night",
            Self::Vercel => "Vercel",
            Self::Vesper => "Vesper",
            Self::Zenburn => "Zenburn",
        }
    }
}

impl Default for ThemeName {
    fn default() -> Self {
        Self::Vercel
    }
}

impl FromStr for ThemeName {
    type Err = ThemeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "amoled" => Ok(Self::Amoled),
            "aura" => Ok(Self::Aura),
            "ayu" | "ayu_dark" => Ok(Self::Ayu),
            "carbonfox" => Ok(Self::Carbonfox),
            "catppuccin" | "catppuccin_mocha" | "catpuccin" => Ok(Self::Catppuccin),
            "catppuccin_frappe" => Ok(Self::CatppuccinFrappe),
            "catppuccin_macchiato" => Ok(Self::CatppuccinMacchiato),
            "cobalt2" => Ok(Self::Cobalt2),
            "cursor" => Ok(Self::Cursor),
            "dracula" => Ok(Self::Dracula),
            "everforest" | "everforest_dark" => Ok(Self::Everforest),
            "flexoki" => Ok(Self::Flexoki),
            "github" => Ok(Self::Github),
            "gruvbox" | "gruvbox_dark" => Ok(Self::Gruvbox),
            "kanagawa" => Ok(Self::Kanagawa),
            "lucent_orng" => Ok(Self::LucentOrng),
            "material" => Ok(Self::Material),
            "matrix" => Ok(Self::Matrix),
            "mercury" => Ok(Self::Mercury),
            "monokai" => Ok(Self::Monokai),
            "nightowl" | "night_owl" => Ok(Self::NightOwl),
            "nord" => Ok(Self::Nord),
            "oc_2" | "oc2" => Ok(Self::Oc2),
            "one_dark" | "onedark" => Ok(Self::OneDark),
            "onedarkpro" | "one_dark_pro" => Ok(Self::Onedarkpro),
            "opencode" => Ok(Self::Opencode),
            "orng" => Ok(Self::Orng),
            "osaka_jade" => Ok(Self::OsakaJade),
            "palenight" | "pale_night" => Ok(Self::Palenight),
            "rose_pine" | "rosepine" => Ok(Self::RosePine),
            "shadesofpurple" | "shades_of_purple" => Ok(Self::ShadesOfPurple),
            "solarized" | "solarized_dark" => Ok(Self::Solarized),
            "synthwave84" | "synthwave_84" => Ok(Self::Synthwave84),
            "tokyo_night" | "tokyonight" | "tira_dark" => Ok(Self::TokyoNight),
            "vercel" => Ok(Self::Vercel),
            "vesper" => Ok(Self::Vesper),
            "zenburn" => Ok(Self::Zenburn),
            other => Err(ThemeError(format!("Unknown theme `{other}`"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    name: ThemeName,
    selected_fg: Color,
    selected_bg: Color,
    text_fg: Color,
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
            text_fg: palette.text,
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
    pub fn text_fg(&self) -> Color {
        self.text_fg
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
            "text_fg" => self.text_fg = color,
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
            [32, 32, 32],
            [242, 242, 242],
            [196, 196, 196],
            [128, 128, 128],
            [86, 156, 214],
            [78, 201, 176],
            [152, 195, 121],
            [229, 192, 123],
            [224, 108, 117],
        ),
        ThemeName::Aura => palette(
            [21, 18, 27],
            [36, 31, 49],
            [69, 58, 94],
            [237, 233, 254],
            [178, 165, 209],
            [122, 109, 156],
            [130, 170, 255],
            [132, 235, 209],
            [167, 233, 175],
            [255, 203, 107],
            [255, 103, 149],
        ),
        ThemeName::Ayu => palette(
            [11, 18, 24],
            [15, 29, 39],
            [36, 49, 62],
            [191, 199, 213],
            [171, 180, 194],
            [94, 104, 117],
            [57, 186, 230],
            [95, 210, 229],
            [195, 232, 141],
            [255, 204, 102],
            [255, 51, 102],
        ),
        ThemeName::Carbonfox => palette(
            [22, 25, 30],
            [42, 45, 53],
            [82, 88, 100],
            [242, 244, 248],
            [196, 203, 211],
            [109, 114, 124],
            [120, 169, 255],
            [63, 203, 212],
            [66, 200, 142],
            [190, 156, 63],
            [255, 125, 125],
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
        ThemeName::CatppuccinFrappe => palette(
            [48, 52, 70],
            [65, 69, 89],
            [115, 121, 148],
            [198, 208, 245],
            [181, 191, 226],
            [140, 145, 172],
            [140, 170, 238],
            [153, 209, 219],
            [166, 209, 137],
            [229, 200, 144],
            [231, 130, 132],
        ),
        ThemeName::CatppuccinMacchiato => palette(
            [36, 39, 58],
            [54, 58, 79],
            [110, 115, 141],
            [202, 211, 245],
            [184, 192, 224],
            [128, 135, 162],
            [138, 173, 244],
            [145, 215, 227],
            [166, 218, 149],
            [238, 212, 159],
            [237, 135, 150],
        ),
        ThemeName::Cobalt2 => palette(
            [25, 36, 76],
            [31, 44, 92],
            [63, 83, 161],
            [255, 255, 255],
            [187, 205, 255],
            [124, 150, 222],
            [0, 194, 255],
            [96, 218, 251],
            [61, 214, 140],
            [255, 214, 10],
            [255, 98, 140],
        ),
        ThemeName::Cursor => palette(
            [27, 31, 39],
            [41, 47, 58],
            [73, 83, 100],
            [230, 236, 241],
            [182, 191, 202],
            [122, 132, 145],
            [87, 164, 255],
            [106, 227, 255],
            [110, 203, 132],
            [244, 191, 117],
            [242, 112, 122],
        ),
        ThemeName::Dracula => palette(
            [40, 42, 54],
            [68, 71, 90],
            [98, 114, 164],
            [248, 248, 242],
            [189, 147, 249],
            [98, 114, 164],
            [139, 233, 253],
            [139, 233, 253],
            [80, 250, 123],
            [241, 250, 140],
            [255, 85, 85],
        ),
        ThemeName::Everforest => palette(
            [45, 53, 59],
            [52, 63, 68],
            [75, 86, 91],
            [211, 198, 170],
            [168, 176, 162],
            [127, 137, 125],
            [127, 187, 179],
            [131, 192, 146],
            [167, 192, 128],
            [219, 188, 127],
            [230, 126, 128],
        ),
        ThemeName::Flexoki => palette(
            [16, 15, 15],
            [28, 27, 26],
            [64, 62, 60],
            [206, 205, 195],
            [185, 173, 146],
            [135, 124, 99],
            [67, 133, 190],
            [58, 169, 159],
            [102, 128, 11],
            [173, 131, 1],
            [209, 77, 65],
        ),
        ThemeName::Github => palette(
            [13, 17, 23],
            [22, 27, 34],
            [48, 54, 61],
            [230, 237, 243],
            [139, 148, 158],
            [110, 118, 129],
            [121, 192, 255],
            [57, 211, 83],
            [63, 185, 80],
            [210, 153, 34],
            [248, 81, 73],
        ),
        ThemeName::Gruvbox => palette(
            [40, 40, 40],
            [60, 56, 54],
            [80, 73, 69],
            [235, 219, 178],
            [213, 196, 161],
            [146, 131, 116],
            [131, 165, 152],
            [142, 192, 124],
            [184, 187, 38],
            [250, 189, 47],
            [251, 73, 52],
        ),
        ThemeName::Kanagawa => palette(
            [31, 31, 40],
            [42, 42, 55],
            [84, 84, 109],
            [220, 215, 186],
            [200, 192, 147],
            [114, 113, 105],
            [126, 156, 216],
            [112, 192, 183],
            [152, 187, 108],
            [230, 195, 132],
            [224, 105, 99],
        ),
        ThemeName::LucentOrng => palette(
            [24, 21, 18],
            [38, 32, 27],
            [83, 68, 55],
            [247, 240, 231],
            [219, 197, 173],
            [157, 132, 108],
            [94, 163, 255],
            [88, 205, 176],
            [140, 201, 118],
            [255, 176, 84],
            [255, 110, 85],
        ),
        ThemeName::Material => palette(
            [38, 50, 56],
            [55, 71, 79],
            [84, 110, 122],
            [238, 255, 255],
            [176, 190, 197],
            [120, 144, 156],
            [130, 170, 255],
            [137, 221, 255],
            [195, 232, 141],
            [255, 203, 107],
            [240, 113, 120],
        ),
        ThemeName::Matrix => palette(
            [4, 12, 4],
            [9, 25, 9],
            [23, 58, 23],
            [166, 255, 166],
            [108, 201, 108],
            [63, 140, 63],
            [61, 214, 140],
            [61, 214, 140],
            [89, 255, 89],
            [178, 255, 89],
            [255, 89, 89],
        ),
        ThemeName::Mercury => palette(
            [26, 29, 33],
            [39, 43, 48],
            [79, 86, 94],
            [233, 238, 242],
            [195, 203, 211],
            [132, 142, 151],
            [110, 163, 255],
            [98, 212, 208],
            [120, 208, 146],
            [255, 199, 99],
            [255, 119, 127],
        ),
        ThemeName::Monokai => palette(
            [39, 40, 34],
            [49, 51, 45],
            [73, 72, 62],
            [248, 248, 242],
            [230, 219, 116],
            [117, 113, 94],
            [102, 217, 239],
            [102, 217, 239],
            [166, 226, 46],
            [230, 219, 116],
            [249, 38, 114],
        ),
        ThemeName::NightOwl => palette(
            [1, 22, 39],
            [10, 34, 57],
            [18, 54, 86],
            [214, 222, 235],
            [127, 219, 202],
            [99, 119, 119],
            [130, 170, 255],
            [127, 219, 202],
            [173, 219, 103],
            [250, 208, 0],
            [239, 83, 80],
        ),
        ThemeName::Nord => palette(
            [46, 52, 64],
            [59, 66, 82],
            [76, 86, 106],
            [216, 222, 233],
            [229, 233, 240],
            [129, 161, 193],
            [94, 129, 172],
            [136, 192, 208],
            [163, 190, 140],
            [235, 203, 139],
            [191, 97, 106],
        ),
        ThemeName::Oc2 => palette(
            [20, 22, 26],
            [32, 35, 42],
            [70, 76, 89],
            [235, 236, 240],
            [187, 192, 199],
            [124, 131, 143],
            [97, 175, 239],
            [86, 182, 194],
            [152, 195, 121],
            [229, 192, 123],
            [224, 108, 117],
        ),
        ThemeName::OneDark => palette(
            [40, 44, 52],
            [49, 54, 63],
            [92, 99, 112],
            [171, 178, 191],
            [190, 195, 202],
            [92, 99, 112],
            [97, 175, 239],
            [86, 182, 194],
            [152, 195, 121],
            [229, 192, 123],
            [224, 108, 117],
        ),
        ThemeName::Onedarkpro => palette(
            [34, 37, 44],
            [43, 47, 58],
            [79, 86, 103],
            [213, 218, 227],
            [171, 178, 191],
            [101, 109, 126],
            [97, 175, 239],
            [86, 182, 194],
            [152, 195, 121],
            [229, 192, 123],
            [224, 108, 117],
        ),
        ThemeName::Opencode => palette(
            [17, 20, 26],
            [28, 33, 42],
            [61, 70, 87],
            [230, 236, 245],
            [182, 190, 202],
            [120, 130, 147],
            [102, 163, 255],
            [79, 209, 197],
            [126, 211, 140],
            [255, 195, 102],
            [255, 107, 107],
        ),
        ThemeName::Orng => palette(
            [25, 22, 19],
            [39, 33, 29],
            [82, 67, 59],
            [244, 236, 229],
            [221, 188, 161],
            [160, 129, 108],
            [92, 159, 255],
            [99, 205, 177],
            [153, 205, 102],
            [255, 183, 77],
            [255, 101, 84],
        ),
        ThemeName::OsakaJade => palette(
            [22, 29, 27],
            [33, 43, 40],
            [63, 82, 77],
            [223, 235, 231],
            [177, 204, 195],
            [116, 151, 141],
            [108, 164, 255],
            [93, 202, 182],
            [133, 201, 129],
            [226, 190, 109],
            [226, 124, 111],
        ),
        ThemeName::Palenight => palette(
            [41, 45, 62],
            [54, 58, 79],
            [103, 114, 229],
            [166, 172, 205],
            [149, 157, 203],
            [103, 114, 149],
            [130, 170, 255],
            [137, 221, 255],
            [195, 232, 141],
            [255, 203, 107],
            [240, 113, 120],
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
            [156, 207, 216],
            [246, 193, 119],
            [235, 111, 146],
        ),
        ThemeName::ShadesOfPurple => palette(
            [43, 18, 68],
            [62, 32, 93],
            [104, 74, 137],
            [255, 255, 255],
            [199, 187, 255],
            [149, 131, 214],
            [130, 170, 255],
            [94, 236, 255],
            [173, 255, 47],
            [255, 183, 77],
            [255, 99, 132],
        ),
        ThemeName::Solarized => palette(
            [0, 43, 54],
            [7, 54, 66],
            [88, 110, 117],
            [131, 148, 150],
            [147, 161, 161],
            [101, 123, 131],
            [38, 139, 210],
            [42, 161, 152],
            [133, 153, 0],
            [181, 137, 0],
            [220, 50, 47],
        ),
        ThemeName::Synthwave84 => palette(
            [38, 24, 67],
            [53, 33, 92],
            [107, 74, 145],
            [255, 255, 255],
            [241, 223, 255],
            [170, 123, 255],
            [54, 247, 255],
            [54, 247, 255],
            [114, 255, 184],
            [255, 209, 102],
            [255, 111, 145],
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
        ThemeName::Vercel => palette(
            [0, 0, 0],
            [20, 20, 20],
            [44, 44, 44],
            [255, 255, 255],
            [170, 170, 170],
            [112, 112, 112],
            [0, 112, 243],
            [0, 166, 255],
            [0, 204, 136],
            [247, 181, 0],
            [255, 0, 80],
        ),
        ThemeName::Vesper => palette(
            [16, 18, 24],
            [27, 30, 38],
            [54, 60, 74],
            [245, 245, 245],
            [185, 188, 193],
            [116, 122, 136],
            [91, 157, 255],
            [95, 205, 228],
            [110, 204, 136],
            [255, 190, 98],
            [255, 110, 114],
        ),
        ThemeName::Zenburn => palette(
            [63, 63, 63],
            [76, 76, 76],
            [98, 98, 98],
            [220, 220, 204],
            [181, 189, 104],
            [127, 159, 127],
            [140, 208, 211],
            [147, 224, 227],
            [95, 126, 93],
            [240, 223, 175],
            [204, 147, 147],
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

    #[test]
    fn default_theme_is_vercel() {
        assert_eq!(ThemeName::default(), ThemeName::Vercel);
        assert_eq!(Theme::default().name(), ThemeName::Vercel);
    }

    #[test]
    fn built_in_theme_ids_round_trip() {
        assert_eq!(ThemeName::ALL.len(), 37);

        for name in ThemeName::ALL {
            assert_eq!(ThemeName::from_str(name.id()).unwrap(), name);
        }
    }
}
