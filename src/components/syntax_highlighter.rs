use ansi_to_tui::IntoText;
use lumis::{formatters::Formatter, languages::Language, themes, TerminalBuilder};
use ratatui::{
    layout::Rect,
    text::Text,
    widgets::Paragraph,
    Frame,
};
use std::time::Duration;

use crate::{
    theme, AnimationSettings, LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint, RenderCtx,
    ThemeName, TickResult, TuiNode,
};

#[derive(Debug, Clone)]
pub struct SyntaxHighlighter {
    code: String,
    language: Language,
    cached_text: Option<Text<'static>>,
    last_theme: Option<ThemeName>,
}

impl SyntaxHighlighter {
    pub fn new(code: impl Into<String>, language: Language) -> Self {
        Self {
            code: code.into(),
            language,
            cached_text: None,
            last_theme: None,
        }
    }

    pub fn code(mut self, code: impl Into<String>) -> Self {
        self.set_code(code);
        self
    }

    pub fn set_code(&mut self, code: impl Into<String>) {
        self.code = code.into();
        self.cached_text = None; // Invalidate cache
    }

    pub fn language(mut self, language: Language) -> Self {
        self.set_language(language);
        self
    }

    pub fn set_language(&mut self, language: Language) {
        if self.language != language {
            self.language = language;
            self.cached_text = None; // Invalidate cache
        }
    }

    fn highlight(&self, theme_name: ThemeName) -> Text<'static> {
        let lumis_theme_name = match theme_name {
            ThemeName::Amoled => "matte_black",
            ThemeName::Aura => "aura_dark",
            ThemeName::Ayu => "ayu_dark",
            ThemeName::Carbonfox => "carbonfox",
            ThemeName::Catppuccin => "catppuccin_mocha",
            ThemeName::CatppuccinFrappe => "catppuccin_frappe",
            ThemeName::CatppuccinMacchiato => "catppuccin_macchiato",
            ThemeName::Cobalt2 => "tokyonight_night",
            ThemeName::Cursor => "vscode_dark",
            ThemeName::Dracula => "dracula",
            ThemeName::Everforest => "everforest_dark",
            ThemeName::Flexoki => "flexoki_dark",
            ThemeName::Github => "github_dark",
            ThemeName::Gruvbox => "gruvbox_dark",
            ThemeName::Kanagawa => "kanagawa_wave",
            ThemeName::LucentOrng => "github_light",
            ThemeName::Material => "material_darker",
            ThemeName::Matrix => "tokyonight_night",
            ThemeName::Mercury => "github_light",
            ThemeName::Monokai => "monokai_pro_dark",
            ThemeName::NightOwl => "tokyonight_night",
            ThemeName::Nord => "nord",
            ThemeName::Oc2 => "tokyonight_night",
            ThemeName::OneDark => "onedark",
            ThemeName::Onedarkpro => "onedarkpro_dark",
            ThemeName::Opencode => "tokyonight_night",
            ThemeName::Orng => "tokyonight_night",
            ThemeName::OsakaJade => "tokyonight_night",
            ThemeName::Palenight => "material_palenight",
            ThemeName::RosePine => "rosepine_dark",
            ThemeName::Solarized => "solarized_autumn_dark",
            ThemeName::Synthwave84 => "tokyonight_night",
            ThemeName::TokyoNight => "tokyonight_night",
            ThemeName::Vercel => "tokyonight_night",
            ThemeName::Vesper => "tokyonight_night",
            ThemeName::Zenburn => "zenburn",
        };

        let theme = themes::get(lumis_theme_name).unwrap_or_else(|_| themes::get("tokyonight_night").unwrap());
        let formatter = TerminalBuilder::new()
            .language(self.language)
            .theme(Some(theme))
            .background(lumis::TerminalBackground::Inherit)
            .build()
            .unwrap();

        let mut output = Vec::new();
        if formatter.format(&self.code, &mut output).is_ok() {
            if let Ok(ansi_str) = String::from_utf8(output) {
                if let Ok(mut text) = ansi_str.into_text() {
                    // ansi-to-tui parses ANSI reset codes as `Color::Reset`.
                    // `Color::Reset` forces the terminal's default background (usually black),
                    // which overrides tuicore's semantic panel backgrounds.
                    // We strip `Color::Reset` so the text inherits the panel's background.
                    for line in &mut text.lines {
                        for span in &mut line.spans {
                            if span.style.bg == Some(ratatui::style::Color::Reset) {
                                span.style.bg = None;
                            }
                            if span.style.fg == Some(ratatui::style::Color::Reset) {
                                span.style.fg = None;
                            }
                        }
                    }
                    return text;
                }
            }
        }
        
        Text::raw(self.code.clone())
    }
}

impl<M> TuiNode<M> for SyntaxHighlighter {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let lines = self.code.lines().count() as u16;
        let max_width = self.code.lines().map(|l| l.chars().count()).max().unwrap_or(0) as u16;
        LayoutSizeHint::content(max_width, lines).normalized(proposal)
    }

    fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, _ctx: &mut RenderCtx<'a>) {
        if let Some(text) = &self.cached_text {
            frame.render_widget(Paragraph::new(text.clone()), area);
        } else {
            let current_theme = theme().name();
            let text = self.highlight(current_theme);
            frame.render_widget(Paragraph::new(text), area);
        }
    }

    fn tick(&mut self, _dt: Duration, _settings: AnimationSettings) -> TickResult {
        let current_theme = theme().name();
        if self.last_theme != Some(current_theme) || self.cached_text.is_none() {
            self.cached_text = Some(self.highlight(current_theme));
            self.last_theme = Some(current_theme);
            TickResult::CHANGED
        } else {
            TickResult::IDLE
        }
    }
}
