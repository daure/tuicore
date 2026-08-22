use ansi_to_tui::IntoText;
pub use lumis::languages::Language;
use lumis::{TerminalBuilder, formatters::Formatter, themes};
use ratatui::{Frame, layout::Rect, text::Text, widgets::Paragraph};
use std::time::Duration;

use crate::{
    Animated, AnimationSettings, AxisProposal, EventCtx, EventOutcome, EventRoute, FocusCtx,
    FocusId, FocusTarget, KeyEvent, LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint,
    RenderCtx, ScrollGeometry, ScrollOutcome, ScrollSize, ScrollState, ThemeName, TickResult,
    TuiEvent, TuiNode, keybindings, paragraph_scroll, theme,
};

const SYNTAX_FOCUS: &str = "syntax-highlighter";

#[derive(Debug, Clone)]
pub struct SyntaxHighlighter {
    code: String,
    language: Language,
    cached_text: Option<Text<'static>>,
    last_theme: Option<ThemeName>,
    scroll: ScrollState,
    content_size: ScrollSize,
    area: Rect,
    focused: bool,
    selected_line: Option<usize>,
    pending_top_prefix: bool,
}

impl SyntaxHighlighter {
    pub fn new(code: impl Into<String>, language: Language) -> Self {
        Self {
            code: code.into(),
            language,
            cached_text: None,
            last_theme: None,
            scroll: ScrollState::default(),
            content_size: ScrollSize::default(),
            area: Rect::default(),
            focused: false,
            selected_line: None,
            pending_top_prefix: false,
        }
    }

    pub fn code(mut self, code: impl Into<String>) -> Self {
        self.set_code(code);
        self
    }

    pub fn set_code(&mut self, code: impl Into<String>) {
        self.code = code.into();
        self.cached_text = None; // Invalidate cache
        self.selected_line = None;
        self.scroll = ScrollState::default();
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
        highlight_text(&self.code, self.language, theme_name)
    }

    fn scroll_geometry(&self, area: Rect) -> ScrollGeometry {
        self.scroll.geometry(area, self.content_size)
    }

    fn clamp_scroll(&mut self) {
        let geometry = self.scroll_geometry(self.area);
        self.scroll.clamp_to(
            geometry.viewport,
            geometry.content,
            AnimationSettings {
                enabled: false,
                ..crate::animation_settings()
            },
        );
    }

    fn center_selection(&mut self, area: Rect, settings: AnimationSettings) -> ScrollOutcome {
        let Some(selected) = self.selected_line else {
            return ScrollOutcome::idle();
        };
        let geometry = self.scroll_geometry(area);
        let viewport = geometry.viewport.height.max(1);
        let y = selected.saturating_sub(viewport / 2);
        self.scroll.scroll_to(
            crate::ScrollOffset::new(self.scroll.target_offset().x, y),
            geometry.viewport,
            geometry.content,
            settings,
        )
    }

    fn select_index(
        &mut self,
        index: usize,
        area: Rect,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        let lines_count = self.code.lines().count();
        if lines_count == 0 {
            return ScrollOutcome::idle();
        }
        let index = index.min(lines_count.saturating_sub(1));
        let changed = self.selected_line != Some(index);
        self.selected_line = Some(index);
        let scroll = self.center_selection(area, settings);
        ScrollOutcome {
            handled: true,
            changed: changed || scroll.changed,
            active: scroll.active,
        }
    }

    fn select_relative(
        &mut self,
        direction: isize,
        area: Rect,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        let lines_count = self.code.lines().count();
        if lines_count == 0 {
            return ScrollOutcome::idle();
        }
        let current = self.selected_line.unwrap_or(0);
        let next = if direction.is_negative() {
            current.saturating_sub(direction.unsigned_abs())
        } else {
            current
                .saturating_add(direction as usize)
                .min(lines_count.saturating_sub(1))
        };
        self.select_index(next, area, settings)
    }

    pub fn on_key_with_settings(
        &mut self,
        key: impl Into<KeyEvent>,
        area: Rect,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        let key = key.into();
        let bindings = keybindings();
        let data_keys = bindings.data_view();
        let viewport = self.scroll_geometry(area).viewport.height.max(1);
        let page = (viewport.saturating_mul(3).saturating_add(4) / 5).max(1);

        if data_keys.top_prefix_matches(key) {
            if self.pending_top_prefix {
                self.pending_top_prefix = false;
                let selection = self.select_index(0, area, settings);
                if selection.changed {
                    return selection;
                }
            } else {
                self.pending_top_prefix = true;
                return ScrollOutcome {
                    handled: true,
                    changed: false,
                    active: false,
                };
            }
        } else {
            self.pending_top_prefix = false;
        }

        if bindings.page_up_matches(key) {
            let selection = self.select_relative(-(page as isize), area, settings);
            if selection.changed {
                return selection;
            }
        }
        if bindings.page_down_matches(key) {
            let selection = self.select_relative(page as isize, area, settings);
            if selection.changed {
                return selection;
            }
        }
        if bindings.home_matches(key) {
            let selection = self.select_index(0, area, settings);
            if selection.changed {
                return selection;
            }
        }
        if bindings.end_matches(key) || data_keys.bottom_matches(key) {
            let lines_count = self.code.lines().count();
            let selection = self.select_index(lines_count.saturating_sub(1), area, settings);
            if selection.changed {
                return selection;
            }
        }
        let instant_settings = AnimationSettings {
            enabled: false,
            ..settings
        };
        if bindings.line_up_matches(key) {
            let selection = self.select_relative(-1, area, instant_settings);
            if selection.changed {
                return selection;
            }
        }
        if bindings.line_down_matches(key) {
            let selection = self.select_relative(1, area, instant_settings);
            if selection.changed {
                return selection;
            }
        }

        let geometry = self.scroll_geometry(area);
        self.scroll
            .on_key(key, geometry.viewport, geometry.content, settings)
    }
}

pub(crate) fn highlight_text(
    code: &str,
    language: Language,
    theme_name: ThemeName,
) -> Text<'static> {
    let append_terminal_newline = language == Language::Markdown && !code.ends_with('\n');
    let source = append_terminal_newline
        .then(|| format!("{code}\n"))
        .unwrap_or_else(|| code.to_owned());
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

    let theme =
        themes::get(lumis_theme_name).unwrap_or_else(|_| themes::get("tokyonight_night").unwrap());
    let formatter = TerminalBuilder::new()
        .language(language)
        .theme(Some(theme))
        .background(lumis::TerminalBackground::Inherit)
        .build()
        .unwrap();

    let mut output = Vec::new();
    if formatter.format(&source, &mut output).is_ok() {
        if let Ok(ansi_str) = String::from_utf8(output) {
            if let Ok(mut text) = ansi_str.into_text() {
                if append_terminal_newline
                    && text.lines.last().is_some_and(|line| line.spans.is_empty())
                {
                    text.lines.pop();
                }
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

    Text::raw(code.to_owned())
}

impl<M> TuiNode<M> for SyntaxHighlighter {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let lines = self.code.lines().count() as u16;
        let max_width = self
            .code
            .lines()
            .map(|l| l.chars().count())
            .max()
            .unwrap_or(0) as u16;
        let width = match proposal.width {
            AxisProposal::Unbounded => max_width,
            AxisProposal::AtMost(max) => max_width.min(max),
            AxisProposal::Exact(exact) => exact,
        };
        LayoutSizeHint::content(width, lines).normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        let resized = self.area.width != area.width || self.area.height != area.height;
        self.area = area;

        let lines = self.code.lines().count();
        let max_width = self
            .code
            .lines()
            .map(|l| l.chars().count())
            .max()
            .unwrap_or(0);
        self.content_size = ScrollSize {
            width: max_width,
            height: lines,
        };

        if resized {
            self.center_selection(
                area,
                AnimationSettings {
                    enabled: false,
                    ..crate::animation_settings()
                },
            );
        } else {
            self.clamp_scroll();
        }

        ctx.register_focusable(FocusId::new(SYNTAX_FOCUS), area, true);
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, _ctx: &mut RenderCtx<'a>) {
        if area.is_empty() {
            return;
        }

        let current_theme = theme().name();
        let text = if let Some(cached) = &self.cached_text {
            cached.clone()
        } else {
            self.highlight(current_theme)
        };

        let geometry = self.scroll_geometry(area);
        if !geometry.layout.viewport.is_empty() {
            // Render full-width selection background if focused
            if self.focused {
                if let Some(selected) = self.selected_line {
                    let offset = self.scroll.offset().y;
                    let bottom = offset.saturating_add(geometry.viewport.height as usize);
                    if selected >= offset && selected < bottom {
                        let style = ratatui::style::Style::default()
                            .fg(theme().highlight_fg())
                            .bg(theme().highlight_bg());
                        frame.render_widget(
                            ratatui::widgets::Block::default().style(style),
                            Rect::new(
                                geometry.layout.viewport.x,
                                geometry.layout.viewport.y + (selected - offset) as u16,
                                geometry.layout.viewport.width,
                                1,
                            ),
                        );
                    }
                }
            }

            frame.render_widget(
                Paragraph::new(text).scroll(paragraph_scroll(self.scroll.offset())),
                geometry.layout.viewport,
            );
        }

        self.scroll
            .render_scrollbars(frame, geometry.layout, geometry.content, self.focused);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };
        let outcome = self.on_key_with_settings(*key, self.area, ctx.animation());
        if outcome.needs_redraw() {
            ctx.request_redraw();
        }
        if outcome.handled {
            ctx.stop_propagation();
            EventOutcome::Handled
        } else {
            EventOutcome::Ignored
        }
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        if route.path.is_empty() {
            self.event(event, ctx)
        } else {
            EventOutcome::Ignored
        }
    }

    fn focus(&mut self, _target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        self.focused = focused;
        if focused && self.selected_line.is_none() {
            self.selected_line = Some(self.scroll.offset().y as usize);
        }
        ctx.request_redraw();
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        if target.path.is_empty() {
            self.focus(Some(&target.id), focused, ctx);
        }
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        let current_theme = theme().name();
        let mut result = TickResult::IDLE;

        if self.last_theme != Some(current_theme) || self.cached_text.is_none() {
            self.cached_text = Some(self.highlight(current_theme));
            self.last_theme = Some(current_theme);
            result = TickResult::CHANGED;
        }

        result.merge(Animated::tick(&mut self.scroll, dt, settings))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Key;

    #[test]
    fn line_navigation_scrolls_to_selected_line_without_tweening() {
        let code = (0..20)
            .map(|line| format!("line {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut highlighter = SyntaxHighlighter::new(code, Language::Rust);
        highlighter.content_size = ScrollSize::new(7, 20);
        let area = Rect::new(0, 0, 7, 4);

        for _ in 0..8 {
            highlighter.on_key_with_settings(Key::Char('j'), area, AnimationSettings::default());
        }

        assert_eq!(
            highlighter.scroll.offset(),
            highlighter.scroll.target_offset()
        );
        assert!(!highlighter.scroll.is_active());

        highlighter.on_key_with_settings(Key::Char('k'), area, AnimationSettings::default());

        assert_eq!(
            highlighter.scroll.offset(),
            highlighter.scroll.target_offset()
        );
        assert!(!highlighter.scroll.is_active());
    }
}
