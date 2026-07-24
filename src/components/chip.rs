use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::{
    LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint, Theme, TuiNode, line_width, theme,
};

const LEFT_CAP: &str = "";
const RIGHT_CAP: &str = "";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChipColorRole {
    #[default]
    Accent,
    Success,
    Warning,
    Error,
    Selected,
    Highlight,
    Muted,
}

pub struct Chip {
    label: String,
    prepend_icon: Option<String>,
    append_icon: Option<String>,
    color_role: ChipColorRole,
}

impl Chip {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            prepend_icon: None,
            append_icon: None,
            color_role: ChipColorRole::default(),
        }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }

    pub fn prepend_icon(mut self, icon: impl Into<String>) -> Self {
        self.prepend_icon = Some(icon.into());
        self
    }

    pub fn set_prepend_icon(&mut self, icon: impl Into<String>) {
        self.prepend_icon = Some(icon.into());
    }

    pub fn clear_prepend_icon(&mut self) {
        self.prepend_icon = None;
    }

    pub fn append_icon(mut self, icon: impl Into<String>) -> Self {
        self.append_icon = Some(icon.into());
        self
    }

    pub fn set_append_icon(&mut self, icon: impl Into<String>) {
        self.append_icon = Some(icon.into());
    }

    pub fn clear_append_icon(&mut self) {
        self.append_icon = None;
    }

    pub fn color_role(mut self, role: ChipColorRole) -> Self {
        self.color_role = role;
        self
    }

    pub fn set_color_role(&mut self, role: ChipColorRole) {
        self.color_role = role;
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        frame.render_widget(Paragraph::new(self.line()), area);
    }

    pub(crate) fn line(&self) -> Line<'static> {
        let (background, foreground) = self.colors(&theme());
        let cap_style = Style::default().fg(background);
        let content_style = Style::default().fg(foreground).bg(background).add_modifier(
            if self.color_role == ChipColorRole::Highlight {
                ratatui::style::Modifier::BOLD
            } else {
                ratatui::style::Modifier::empty()
            },
        );
        let mut spans = vec![Span::styled(LEFT_CAP, cap_style)];
        spans.push(Span::styled(self.content(), content_style));
        spans.push(Span::styled(RIGHT_CAP, cap_style));
        Line::from(spans)
    }

    fn content(&self) -> String {
        let mut content = String::new();
        if let Some(icon) = &self.prepend_icon {
            content.push_str(icon);
            content.push(' ');
        }
        content.push_str(&self.label);
        if let Some(icon) = &self.append_icon {
            content.push(' ');
            content.push_str(icon);
        }
        content
    }

    fn colors(&self, theme: &Theme) -> (Color, Color) {
        match self.color_role {
            ChipColorRole::Accent => (
                theme.accent_fg(),
                theme.contrast_foreground(theme.accent_fg()),
            ),
            ChipColorRole::Success => (
                theme.success_fg(),
                theme.contrast_foreground(theme.success_fg()),
            ),
            ChipColorRole::Warning => (
                theme.warning_fg(),
                theme.contrast_foreground(theme.warning_fg()),
            ),
            ChipColorRole::Error => (
                theme.error_fg(),
                theme.contrast_foreground(theme.error_fg()),
            ),
            ChipColorRole::Selected => (theme.selected_bg(), theme.selected_fg()),
            ChipColorRole::Highlight => (theme.highlight_bg(), theme.highlight_fg()),
            ChipColorRole::Muted => (theme.surface_bg(), theme.text_fg()),
        }
    }
}

impl<M> TuiNode<M> for Chip {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        LayoutSizeHint::content(line_width(&self.line()).min(u16::MAX as usize) as u16, 1)
            .normalized(proposal)
    }

    fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _ctx: &mut crate::RenderCtx<'_>) {
        Self::render(self, frame, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn muted_chip_uses_surface_background_role() {
        let chip = Chip::new("Muted").color_role(ChipColorRole::Muted);

        assert_eq!(chip.colors(&theme()).0, theme().surface_bg());
    }

    #[test]
    fn semantic_chip_foreground_is_independent_from_focus_role() {
        let theme = Theme::from_toml_str("[colors]\nhighlight_fg = \"#ff00ff\"\n")
            .expect("theme override should parse");
        let chip = Chip::new("Saved").color_role(ChipColorRole::Success);

        assert_eq!(
            chip.colors(&theme),
            (
                theme.success_fg(),
                theme.contrast_foreground(theme.success_fg())
            )
        );
        assert_ne!(chip.colors(&theme).1, theme.highlight_fg());
    }

    #[test]
    fn highlight_chip_is_bold() {
        let line = Chip::new("Focused")
            .color_role(ChipColorRole::Highlight)
            .line();

        assert_eq!(
            line.spans[1].style.add_modifier,
            ratatui::style::Modifier::BOLD
        );
    }

    #[test]
    fn selected_chip_is_not_bold() {
        let line = Chip::new("Selected")
            .color_role(ChipColorRole::Selected)
            .line();

        assert_eq!(
            line.spans[1].style.add_modifier,
            ratatui::style::Modifier::empty()
        );
    }
}
