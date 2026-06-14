use std::time::Duration;

use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::Component;
use tuirealm::event::KeyEvent;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::ratatui::Frame;
use tuirealm::ratatui::layout::Rect;
use tuirealm::ratatui::style::{Modifier, Style};
use tuirealm::ratatui::text::{Line, Span};
use tuirealm::ratatui::widgets::{List as RatatuiList, ListItem, ListState};
use tuirealm::state::State;

use crate::{
    Animated, AnimationSettings, ScrollAxes, ScrollBehavior, ScrollGeometry, ScrollOffset,
    ScrollOutcome, ScrollSize, ScrollState, ScrollbarConfig, TickResult, animation_settings,
    keybindings, line_width, preset, theme,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListOutcome {
    pub handled: bool,
    pub changed: bool,
    pub active: bool,
}

impl ListOutcome {
    pub const IDLE: Self = Self {
        handled: false,
        changed: false,
        active: false,
    };

    pub const HANDLED: Self = Self {
        handled: true,
        changed: false,
        active: false,
    };

    pub const CHANGED: Self = Self {
        handled: true,
        changed: true,
        active: false,
    };

    pub fn needs_redraw(self) -> bool {
        self.changed || self.active
    }
}

#[derive(Debug, Clone)]
pub struct List {
    items: Vec<String>,
    selected: usize,
    highlight_symbol: String,
    focused: bool,
    scroll: ScrollState,
}

impl Default for List {
    fn default() -> Self {
        Self::new(Vec::<String>::new())
    }
}

impl List {
    pub fn new(items: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            items: items.into_iter().map(Into::into).collect(),
            selected: 0,
            highlight_symbol: String::from("› "),
            focused: false,
            scroll: ScrollState::from_preset(ScrollAxes::Vertical, preset().scroll()),
        }
    }

    pub fn selected(mut self, selected: usize) -> Self {
        self.selected = self.clamp_selected(selected);
        self
    }

    pub fn highlight_symbol(mut self, symbol: impl Into<String>) -> Self {
        self.highlight_symbol = symbol.into();
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn scroll_behavior(mut self, behavior: ScrollBehavior) -> Self {
        self.scroll = self.scroll.behavior(behavior);
        self
    }

    pub fn scrollbars(mut self, config: ScrollbarConfig) -> Self {
        self.scroll = self.scroll.scrollbars(config);
        self
    }

    pub fn items(&self) -> &[String] {
        &self.items
    }

    pub fn selected_index(&self) -> usize {
        self.clamp_selected(self.selected)
    }

    pub fn selected_item(&self) -> Option<&str> {
        self.items.get(self.selected_index()).map(String::as_str)
    }

    pub fn select_index(&mut self, selected: usize) -> ListOutcome {
        let selected = self.clamp_selected(selected);
        let changed = selected != self.selected;
        self.selected = selected;
        ListOutcome {
            handled: true,
            changed,
            active: false,
        }
    }

    pub fn select_index_with_settings(
        &mut self,
        selected: usize,
        area: Rect,
        settings: AnimationSettings,
    ) -> ListOutcome {
        let selected = self.clamp_selected(selected);
        let changed = selected != self.selected;
        self.selected = selected;
        self.ensure_selection_visible(area, settings)
            .into_list_outcome(true, changed)
    }

    pub fn next(&mut self) -> ListOutcome {
        self.select_index(self.selected_index().saturating_add(1))
    }

    pub fn previous(&mut self) -> ListOutcome {
        self.select_index(self.selected_index().saturating_sub(1))
    }

    pub fn page_down(&mut self, page: usize) -> ListOutcome {
        self.select_index(self.selected_index().saturating_add(page.max(1)))
    }

    pub fn page_up(&mut self, page: usize) -> ListOutcome {
        self.select_index(self.selected_index().saturating_sub(page.max(1)))
    }

    pub fn first(&mut self) -> ListOutcome {
        self.select_index(0)
    }

    pub fn last(&mut self) -> ListOutcome {
        self.select_index(self.items.len().saturating_sub(1))
    }

    pub fn on_key(&mut self, key: KeyEvent, viewport: Rect) -> ListOutcome {
        self.on_key_with_settings(key, viewport, animation_settings())
    }

    pub fn on_key_with_settings(
        &mut self,
        key: KeyEvent,
        area: Rect,
        settings: AnimationSettings,
    ) -> ListOutcome {
        let page = self
            .scroll_geometry(area)
            .viewport
            .height
            .saturating_sub(1)
            .max(1);
        let keybindings = keybindings();
        if keybindings.line_up_matches(key) {
            self.select_index_with_settings(self.selected_index().saturating_sub(1), area, settings)
        } else if keybindings.line_down_matches(key) {
            self.select_index_with_settings(self.selected_index().saturating_add(1), area, settings)
        } else if keybindings.page_up_matches(key) {
            self.select_index_with_settings(
                self.selected_index().saturating_sub(page),
                area,
                settings,
            )
        } else if keybindings.page_down_matches(key) {
            self.select_index_with_settings(
                self.selected_index().saturating_add(page),
                area,
                settings,
            )
        } else if keybindings.home_matches(key) {
            self.select_index_with_settings(0, area, settings)
        } else if keybindings.end_matches(key) {
            self.select_index_with_settings(self.items.len().saturating_sub(1), area, settings)
        } else {
            ListOutcome::IDLE
        }
    }

    pub fn content_size(&self) -> ScrollSize {
        let width = self
            .items
            .iter()
            .map(|item| line_width(&Line::from(item.as_str())))
            .max()
            .unwrap_or(0);
        ScrollSize::new(width, self.items.len())
    }

    pub fn scroll_geometry(&self, area: Rect) -> ScrollGeometry {
        self.scroll.geometry(area, self.content_size())
    }

    pub fn clamp_scroll(&mut self, area: Rect, settings: AnimationSettings) -> ScrollOutcome {
        let geometry = self.scroll_geometry(area);
        self.scroll
            .clamp_to(geometry.viewport, geometry.content, settings)
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let theme = theme();
        let geometry = self.scroll_geometry(area);
        let offset = self.visible_offset(geometry.viewport);
        let selected = (!self.items.is_empty()).then_some(self.selected_index());
        let visible_selected = selected.and_then(|selected| {
            (selected >= offset && selected < offset.saturating_add(geometry.viewport.height))
                .then_some(selected - offset)
        });
        let items = self
            .items
            .iter()
            .skip(offset)
            .take(geometry.viewport.height)
            .map(|item| ListItem::new(Line::from(Span::raw(item.as_str()))));
        let list = RatatuiList::new(items)
            .highlight_style(
                Style::default()
                    .fg(theme.selected_fg())
                    .bg(theme.selected_bg())
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(self.highlight_symbol.as_str());
        let mut state = ListState::default().with_selected(visible_selected);

        frame.render_stateful_widget(list, geometry.layout.viewport, &mut state);
        self.scroll
            .render_scrollbars(frame, geometry.layout, geometry.content, self.focused);
    }

    fn clamp_selected(&self, selected: usize) -> usize {
        selected.min(self.items.len().saturating_sub(1))
    }

    fn ensure_selection_visible(
        &mut self,
        area: Rect,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        let geometry = self.scroll_geometry(area);
        let viewport_height = geometry.viewport.height.max(1);
        let selected = self.selected_index();
        let current = self.scroll.target_offset().y;
        let target = if selected < current {
            selected
        } else if selected >= current.saturating_add(viewport_height) {
            selected.saturating_add(1).saturating_sub(viewport_height)
        } else {
            current
        };

        self.scroll.scroll_to(
            ScrollOffset::new(0, target),
            geometry.viewport,
            geometry.content,
            settings,
        )
    }

    fn visible_offset(&self, viewport: ScrollSize) -> usize {
        self.scroll
            .offset()
            .y
            .min(self.items.len().saturating_sub(viewport.height))
    }
}

impl Animated for List {
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.scroll.tick(dt, settings)
    }
}

impl Component for List {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        self.render(frame, area);
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        match attr {
            Attribute::Focus => Some(QueryResult::Owned(AttrValue::Flag(self.focused))),
            _ => None,
        }
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        if attr == Attribute::Focus
            && let AttrValue::Flag(focused) = value
        {
            self.focused = focused;
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        CmdResult::Invalid(cmd)
    }
}

trait ScrollOutcomeExt {
    fn into_list_outcome(self, handled: bool, selection_changed: bool) -> ListOutcome;
}

impl ScrollOutcomeExt for ScrollOutcome {
    fn into_list_outcome(self, handled: bool, selection_changed: bool) -> ListOutcome {
        ListOutcome {
            handled: handled || self.handled,
            changed: selection_changed || self.changed,
            active: self.active,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tuirealm::event::Key;

    #[test]
    fn navigation_clamps_to_items() {
        let mut list = List::new(["one", "two"]);

        assert_eq!(list.selected_index(), 0);
        assert_eq!(list.previous(), ListOutcome::HANDLED);
        assert_eq!(list.selected_index(), 0);
        assert_eq!(list.last(), ListOutcome::CHANGED);
        assert_eq!(list.next(), ListOutcome::HANDLED);
        assert_eq!(list.selected_index(), 1);
    }

    #[test]
    fn empty_list_has_no_selected_item() {
        let list = List::new(Vec::<String>::new()).selected(10);

        assert_eq!(list.selected_index(), 0);
        assert_eq!(list.selected_item(), None);
    }

    #[test]
    fn navigation_scrolls_selection_into_view() {
        let mut list = List::new(["one", "two", "three"]);
        let area = Rect::new(0, 0, 10, 2);

        let outcome = list.on_key_with_settings(
            KeyEvent::from(Key::Down),
            area,
            disabled_animation_settings(),
        );
        assert_eq!(outcome, ListOutcome::CHANGED);
        assert_eq!(list.scroll.target_offset().y, 0);

        let outcome = list.on_key_with_settings(
            KeyEvent::from(Key::Down),
            area,
            disabled_animation_settings(),
        );
        assert_eq!(outcome, ListOutcome::CHANGED);
        assert_eq!(list.scroll.target_offset().y, 1);
    }

    fn disabled_animation_settings() -> AnimationSettings {
        AnimationSettings {
            enabled: false,
            ..AnimationSettings::default()
        }
    }
}
