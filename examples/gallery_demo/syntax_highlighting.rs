use lumis::languages::Language;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};
use tuicore::{
    ChildKey, Dropdown, DropdownSearchMode, EventCtx, EventRoute, FocusCtx, LayoutCtx,
    LayoutProposal, LayoutResult, LayoutSizeHint, RenderCtx, TuiEvent, EventOutcome, FocusTarget,
    TuiNode, SyntaxHighlighter,
};

use super::code_samples::*;

#[derive(Clone)]
pub(crate) struct LanguageItem {
    pub(crate) id: Language,
    pub(crate) label: &'static str,
}

fn language_items() -> Vec<LanguageItem> {
    vec![
        LanguageItem { id: Language::Rust, label: "Rust" },
        LanguageItem { id: Language::JavaScript, label: "JavaScript" },
        LanguageItem { id: Language::Python, label: "Python" },
        LanguageItem { id: Language::HTML, label: "HTML" },
        LanguageItem { id: Language::CSS, label: "CSS" },
        LanguageItem { id: Language::JSON, label: "JSON" },
        LanguageItem { id: Language::Toml, label: "TOML" },
        LanguageItem { id: Language::Bash, label: "Bash" },
        LanguageItem { id: Language::Markdown, label: "Markdown" },
    ]
}

pub(crate) fn language_dropdown() -> Dropdown<LanguageItem, Language> {
    Dropdown::single(language_items(), |row| row.id, |row| row.label.to_string())
        .placeholder("Select language...")
        .label("Language")
        .search_mode(DropdownSearchMode::Contains)
        .selected([Language::Rust])
}

pub(crate) struct SyntaxHighlightingDemo {
    dropdown: Dropdown<LanguageItem, Language>,
    highlighter: SyntaxHighlighter,
}

impl SyntaxHighlightingDemo {
    pub(crate) fn new() -> Self {
        Self {
            dropdown: language_dropdown(),
            highlighter: SyntaxHighlighter::new(RUST_SAMPLE, Language::Rust),
        }
    }
}

impl tuicore::TuiNode<crate::Msg> for SyntaxHighlightingDemo {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let dropdown_hint = TuiNode::<crate::Msg>::measure(&self.dropdown, proposal);
        let highlighter_hint = TuiNode::<crate::Msg>::measure(&self.highlighter, proposal);
        LayoutSizeHint::content(
            dropdown_hint.preferred.width.max(highlighter_hint.preferred.width),
            dropdown_hint.preferred.height + highlighter_hint.preferred.height + 1
        )
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        let [dropdown_area, code_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Fill(1)])
            .areas(area);
            
        ctx.push_slot(ChildKey::new("dropdown"), dropdown_area, |ctx| {
            TuiNode::<crate::Msg>::layout(&mut self.dropdown, dropdown_area, ctx);
        });
        
        ctx.push_slot(ChildKey::new("code"), code_area, |ctx| {
            TuiNode::<crate::Msg>::layout(&mut self.highlighter, code_area, ctx);
        });
        
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        let [dropdown_area, code_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Fill(1)])
            .areas(area);
            
        TuiNode::<crate::Msg>::render(&self.dropdown, frame, dropdown_area, ctx);
        TuiNode::<crate::Msg>::render(&self.highlighter, frame, code_area, ctx);
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<crate::Msg>,
    ) -> EventOutcome {
        if let Some(child_path) = route.path.without_first_if(&ChildKey::new("dropdown")) {
            let child_route = EventRoute::new(child_path);
            let outcome = TuiNode::<crate::Msg>::dispatch_event(&mut self.dropdown, &child_route, event, ctx);
            if let Some(selected) = self.dropdown.selected_ids().first() {
                let source = match selected {
                    Language::Rust => RUST_SAMPLE,
                    Language::JavaScript => JS_SAMPLE,
                    Language::Python => PYTHON_SAMPLE,
                    Language::HTML => HTML_SAMPLE,
                    Language::CSS => CSS_SAMPLE,
                    Language::JSON => JSON_SAMPLE,
                    Language::Toml => TOML_SAMPLE,
                    Language::Bash => BASH_SAMPLE,
                    Language::Markdown => MARKDOWN_SAMPLE,
                    _ => RUST_SAMPLE,
                };
                self.highlighter.set_code(source);
                self.highlighter.set_language(*selected);
            }
            return outcome;
        }
        if let Some(child_path) = route.path.without_first_if(&ChildKey::new("code")) {
            let child_route = EventRoute::new(child_path);
            return TuiNode::<crate::Msg>::dispatch_event(&mut self.highlighter, &child_route, event, ctx);
        }
        EventOutcome::Ignored
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<crate::Msg>) {
        if let Some(child_target) = target.for_child(&ChildKey::new("dropdown")) {
            TuiNode::<crate::Msg>::dispatch_focus(&mut self.dropdown, &child_target, focused, ctx);
        }
        if let Some(child_target) = target.for_child(&ChildKey::new("code")) {
            TuiNode::<crate::Msg>::dispatch_focus(&mut self.highlighter, &child_target, focused, ctx);
        }
    }

    fn tick(&mut self, dt: std::time::Duration, settings: tuicore::AnimationSettings) -> tuicore::TickResult {
        let mut result = TuiNode::<crate::Msg>::tick(&mut self.dropdown, dt, settings);
        result = result.merge(TuiNode::<crate::Msg>::tick(&mut self.highlighter, dt, settings));
        result
    }

    fn init(&mut self, ctx: &mut tuicore::LifecycleCtx<crate::Msg>) {
        TuiNode::<crate::Msg>::init(&mut self.dropdown, ctx);
        TuiNode::<crate::Msg>::init(&mut self.highlighter, ctx);
    }

    fn mount(&mut self, ctx: &mut tuicore::LifecycleCtx<crate::Msg>) {
        TuiNode::<crate::Msg>::mount(&mut self.dropdown, ctx);
        TuiNode::<crate::Msg>::mount(&mut self.highlighter, ctx);
    }

    fn unmount(&mut self, ctx: &mut tuicore::LifecycleCtx<crate::Msg>) {
        TuiNode::<crate::Msg>::unmount(&mut self.dropdown, ctx);
        TuiNode::<crate::Msg>::unmount(&mut self.highlighter, ctx);
    }

    fn destroy(&mut self, ctx: &mut tuicore::LifecycleCtx<crate::Msg>) {
        TuiNode::<crate::Msg>::destroy(&mut self.dropdown, ctx);
        TuiNode::<crate::Msg>::destroy(&mut self.highlighter, ctx);
    }
}
