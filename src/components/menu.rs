use std::cell::{Cell, RefCell};
use std::hash::Hash;
use std::rc::Rc;
use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::components::{Column, DataView, SelectionMode, TextInput};
use crate::event::KeyEvent;
use crate::search::{MatchSpan, SearchMode, search_match, search_ranked};
use crate::{
    Animated, AnimationSettings, AnimationSpec, EventCtx, EventOutcome, FocusCtx, FocusId,
    FocusRequest, FocusTarget, HintSource, HotkeyMatch, HotkeySequenceMatcher, LayoutCtx,
    LayoutProposal, LayoutResult, LayoutSize, LayoutSizeHint, OverlayId, OverlayLayer, OverlaySpec,
    TickResult, TreePath, TuiEvent, TuiNode, Tween, border_set, keybindings, line_width, preset,
    theme,
};

mod types;
use types::MenuFocusRegion;
pub use types::{MenuActionKeys, MenuItem, MenuOutcome, MenuPopupDirection, MenuSearchMode};

const SEARCH_FOCUS: &str = "search";
const PANEL_FOCUS: &str = "panel";
const POPUP_BORDER_HEIGHT: u16 = 2;
const DEFAULT_VISIBLE_ITEMS: u16 = 10;
const MENU_BACKDROP_AMOUNT: f64 = 0.55;
const MENU_OVERLAY_NAMESPACE: u64 = 0x4d45_4e55_504f_5055;

fn highlighted_label_line(
    label: String,
    query: &str,
    search_mode: MenuSearchMode,
) -> Line<'static> {
    if query.is_empty() || search_mode == MenuSearchMode::None {
        return Line::from(label);
    }

    let Some(mode) = search_mode_for_highlight(search_mode) else {
        return Line::from(label);
    };
    let Some(matched) = search_match(query, &label, mode) else {
        return Line::from(label);
    };
    highlighted_spans(label, &matched.spans)
}

fn search_mode_for_highlight(search_mode: MenuSearchMode) -> Option<SearchMode> {
    match search_mode {
        MenuSearchMode::None => None,
        MenuSearchMode::Contains => Some(SearchMode::Contains),
        MenuSearchMode::Fuzzy => Some(SearchMode::Fuzzy),
    }
}

fn highlighted_spans(label: String, spans: &[MatchSpan]) -> Line<'static> {
    if spans.is_empty() {
        return Line::from(label);
    }

    let mut rendered = Vec::new();
    let mut cursor = 0;
    for span in spans {
        if span.start > cursor {
            rendered.push(Span::raw(label[cursor..span.start].to_string()));
        }
        rendered.push(Span::styled(
            label[span.start..span.end].to_string(),
            search_match_style(),
        ));
        cursor = span.end;
    }
    if cursor < label.len() {
        rendered.push(Span::raw(label[cursor..].to_string()));
    }

    Line::from(rendered)
}

fn search_match_style() -> Style {
    Style::default()
        .fg(theme().accent_fg())
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
}

fn clip_rect(area: Rect, bounds: Rect) -> Rect {
    let x = area.x.max(bounds.x);
    let y = area.y.max(bounds.y);
    let right = area
        .x
        .saturating_add(area.width)
        .min(bounds.x.saturating_add(bounds.width));
    let bottom = area
        .y
        .saturating_add(area.height)
        .min(bounds.y.saturating_add(bounds.height));
    Rect::new(x, y, right.saturating_sub(x), bottom.saturating_sub(y))
}

fn matches_any(bindings: &[crate::KeySpec], key: KeyEvent) -> bool {
    bindings.iter().any(|binding| binding.matches(key))
}

fn target_index(current: usize, delta: isize, len: usize) -> usize {
    let last = len.saturating_sub(1);
    if delta.is_negative() {
        current.saturating_sub(delta.unsigned_abs())
    } else {
        current.saturating_add(delta as usize).min(last)
    }
}

pub struct Menu<Id> {
    data_view: DataView<MenuItem<Id>, Id>,
    search_input: TextInput,
    search_render_query: Rc<RefCell<String>>,
    search_render_mode: Rc<Cell<MenuSearchMode>>,
    ids: Vec<Id>,
    labels: Vec<String>,
    filtered: Vec<Id>,
    open: bool,
    search_mode: MenuSearchMode,
    max_popup_height: Option<u16>,
    visible_items: u16,
    min_popup_width: u16,
    popup_direction: MenuPopupDirection,
    trigger_area: Rect,
    overlay_bounds: Rect,
    focus_path: TreePath,
    focus_region: Option<MenuFocusRegion>,
    scroll_highlight_on_next_layout: bool,
    action_keys: MenuActionKeys,
    trigger_hotkey: Option<String>,
    trigger_hotkey_matcher: HotkeySequenceMatcher,
    backdrop_tween: Tween,
    return_focus: Option<(TreePath, FocusId)>,
    on_activate: Option<Box<dyn Fn(Id) + 'static>>,
    activated: Vec<Id>,
}

impl<Id> Menu<Id>
where
    Id: Clone + Eq + Hash + 'static,
{
    pub fn new(rows: impl IntoIterator<Item = MenuItem<Id>>) -> Self {
        let rows = rows.into_iter().collect::<Vec<_>>();
        let ids = rows.iter().map(|row| row.id.clone()).collect::<Vec<_>>();
        let labels = rows.iter().map(|row| row.label.clone()).collect::<Vec<_>>();
        let search_render_query = Rc::new(RefCell::new(String::new()));
        let search_render_mode = Rc::new(Cell::new(MenuSearchMode::Fuzzy));
        let data_view_search_query = Rc::clone(&search_render_query);
        let data_view_search_mode = Rc::clone(&search_render_mode);
        let data_view = DataView::new(rows, |row: &MenuItem<Id>| row.id.clone())
            .column(Column::rich(
                "label",
                "",
                Constraint::Percentage(100),
                move |row: &MenuItem<Id>, _| {
                    highlighted_label_line(
                        row.label.clone(),
                        &data_view_search_query.borrow(),
                        data_view_search_mode.get(),
                    )
                },
            ))
            .selection_mode(SelectionMode::Single)
            .focused(false);

        Self {
            data_view,
            search_input: TextInput::new().placeholder("Search..."),
            search_render_query,
            search_render_mode,
            filtered: ids.clone(),
            ids,
            labels,
            open: false,
            search_mode: MenuSearchMode::Fuzzy,
            max_popup_height: None,
            visible_items: DEFAULT_VISIBLE_ITEMS,
            min_popup_width: 24,
            popup_direction: MenuPopupDirection::Down,
            trigger_area: Rect::default(),
            overlay_bounds: Rect::default(),
            focus_path: TreePath::default(),
            focus_region: None,
            scroll_highlight_on_next_layout: false,
            action_keys: MenuActionKeys::default(),
            trigger_hotkey: None,
            trigger_hotkey_matcher: HotkeySequenceMatcher::default(),
            backdrop_tween: Tween::idle(0.0),
            return_focus: None,
            on_activate: None,
            activated: Vec::new(),
        }
    }

    pub fn search_mode(mut self, mode: MenuSearchMode) -> Self {
        self.search_mode = mode;
        self.search_render_mode.set(mode);
        self.refresh_filter();
        self
    }

    pub fn max_popup_height(mut self, height: u16) -> Self {
        self.max_popup_height = Some(height.max(1));
        self
    }

    pub fn visible_items(mut self, count: u16) -> Self {
        self.visible_items = count.max(1);
        self
    }

    pub fn min_popup_width(mut self, width: u16) -> Self {
        self.min_popup_width = width.max(1);
        self
    }

    pub fn popup_direction(mut self, direction: MenuPopupDirection) -> Self {
        self.popup_direction = direction;
        self
    }

    pub fn action_keys(mut self, keys: MenuActionKeys) -> Self {
        self.action_keys = keys;
        self
    }

    /// Mirrors the external trigger hotkey while the popup owns focus.
    ///
    /// Use this with custom triggers that have a hotkey so pressing the same
    /// key can close the open menu instead of typing into search.
    pub fn trigger_hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.set_trigger_hotkey(hotkey);
        self
    }

    pub fn set_trigger_hotkey(&mut self, hotkey: impl Into<String>) {
        let hotkey = hotkey.into();
        self.trigger_hotkey = Some(hotkey.clone());
        self.trigger_hotkey_matcher = HotkeySequenceMatcher::new([hotkey]);
    }

    pub fn clear_trigger_hotkey(&mut self) {
        self.trigger_hotkey = None;
        self.trigger_hotkey_matcher = HotkeySequenceMatcher::default();
    }

    /// Requests focus on `path` + `id` when context-driven close completes.
    pub fn return_focus_to(mut self, path: TreePath, id: FocusId) -> Self {
        self.set_return_focus_to(path, id);
        self
    }

    pub fn set_return_focus_to(&mut self, path: TreePath, id: FocusId) {
        self.return_focus = Some((path, id));
    }

    pub fn on_activate(mut self, handler: impl Fn(Id) + 'static) -> Self {
        self.on_activate = Some(Box::new(handler));
        self
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn search_query(&self) -> &str {
        self.search_input.current_value()
    }

    pub fn take_activated(&mut self) -> Vec<Id> {
        std::mem::take(&mut self.activated)
    }

    pub fn open(&mut self) -> MenuOutcome {
        if self.open {
            return MenuOutcome::HANDLED;
        }

        self.open = true;
        self.backdrop_tween.snap_to(MENU_BACKDROP_AMOUNT);
        self.refresh_filter();
        self.highlight_first_filtered();
        self.focus_region = Some(if self.search_enabled() {
            MenuFocusRegion::Search
        } else {
            MenuFocusRegion::Panel
        });
        self.sync_child_focus();
        self.scroll_highlight_on_next_layout = true;
        MenuOutcome {
            handled: true,
            changed: true,
            opened: true,
            ..MenuOutcome::IDLE
        }
    }

    /// Opens the menu and requests layout, redraw, and focus through `ctx`.
    ///
    /// External triggers should prefer this over `open()` so the runtime can
    /// place and focus the popup immediately.
    pub fn open_with_context<M>(&mut self, ctx: &mut EventCtx<M>) -> MenuOutcome {
        let outcome = self.open();
        if outcome.opened {
            self.backdrop_tween.snap_to(0.0);
            self.start_backdrop_tween(true, ctx.animation());
        }
        self.apply_open_close_context(outcome, ctx);
        outcome
    }

    fn start_backdrop_tween(&mut self, active: bool, settings: AnimationSettings) {
        let target = if active { MENU_BACKDROP_AMOUNT } else { 0.0 };
        let resolved = settings.resolve(AnimationSpec::default());
        if !resolved.enabled {
            self.backdrop_tween.snap_to(target);
            return;
        }
        self.backdrop_tween.start(
            self.backdrop_tween.value(),
            target,
            resolved.duration,
            resolved.easing,
        );
    }

    pub fn close(&mut self) -> MenuOutcome {
        if !self.open {
            return MenuOutcome::HANDLED;
        }

        self.open = false;
        self.backdrop_tween.snap_to(0.0);
        self.clear_search_query();
        self.focus_region = None;
        self.sync_child_focus();
        MenuOutcome {
            handled: true,
            changed: true,
            closed: true,
            ..MenuOutcome::IDLE
        }
    }

    /// Closes the menu and requests layout/redraw through `ctx`.
    ///
    /// External triggers should prefer this over `close()` when user-visible
    /// popup state changes need runtime updates.
    pub fn close_with_context<M>(&mut self, ctx: &mut EventCtx<M>) -> MenuOutcome {
        let outcome = self.close();
        self.apply_open_close_context(outcome, ctx);
        outcome
    }

    pub fn toggle(&mut self) -> MenuOutcome {
        if self.open { self.close() } else { self.open() }
    }

    /// Toggles the menu and requests layout, redraw, and open focus through `ctx`.
    ///
    /// External triggers should prefer this over `toggle()` so the runtime can
    /// place, redraw, and focus the popup consistently.
    pub fn toggle_with_context<M>(&mut self, ctx: &mut EventCtx<M>) -> MenuOutcome {
        let outcome = self.toggle();
        self.apply_open_close_context(outcome, ctx);
        outcome
    }

    pub fn on_key(&mut self, key: impl Into<KeyEvent>, area: Rect) -> MenuOutcome {
        let key = key.into();
        if !self.open {
            return MenuOutcome::IDLE;
        }

        match self.trigger_hotkey_matcher.on_key(key) {
            HotkeyMatch::Matched(_) => return self.toggle(),
            HotkeyMatch::Pending | HotkeyMatch::Canceled => return MenuOutcome::HANDLED,
            HotkeyMatch::Ignored => {}
        }

        if keybindings().focus().unfocus_matches(key) {
            return self.close();
        }
        if matches_any(&self.action_keys.activate, key)
            || keybindings().dropdown().select_matches(key)
        {
            return self.activate_highlighted();
        }
        if keybindings().dropdown().next_matches(key) {
            return self.navigate_by(1, area);
        }
        if keybindings().dropdown().previous_matches(key) {
            return self.navigate_by(-1, area);
        }
        if keybindings().dropdown().page_next_matches(key) {
            return self.navigate_by(self.list_viewport_height(area) as isize, area);
        }
        if keybindings().dropdown().page_previous_matches(key) {
            return self.navigate_by(-(self.list_viewport_height(area) as isize), area);
        }

        if self.search_enabled() {
            let input = self.search_input.on_key(key);
            if input.changed {
                self.refresh_filter();
                self.highlight_first_filtered();
            }
            if input.needs_redraw() {
                return MenuOutcome {
                    handled: true,
                    changed: input.changed,
                    ..MenuOutcome::IDLE
                };
            }
        }

        MenuOutcome::IDLE
    }

    fn layout_with_current_bounds<M>(
        &mut self,
        trigger_area: Rect,
        ctx: &mut LayoutCtx,
    ) -> LayoutResult {
        self.trigger_area = trigger_area;
        let overlay_bounds = ctx.overlay_bounds();
        self.overlay_bounds = overlay_bounds;
        self.focus_path = ctx.current_path();
        if !self.open {
            return LayoutResult::new(trigger_area);
        }

        let popup_area = self.popup_area_for(trigger_area, overlay_bounds);
        let mut spec = OverlaySpec::new(
            OverlayId::for_path(MENU_OVERLAY_NAMESPACE, &self.focus_path),
            trigger_area,
            popup_area,
        );
        spec.owner_path = Some(self.focus_path.clone());
        spec.route_path = Some(self.focus_path.clone());
        spec.bounds = Some(overlay_bounds);
        spec.layer = OverlayLayer::Popover;
        ctx.register_overlay(spec);

        let [search_area, list_area] = self.popup_inner_areas(popup_area);
        if self.search_enabled() {
            ctx.register_text_entry_focusable(FocusId::new(SEARCH_FOCUS), search_area, true, true);
            ctx.set_focus_tab_stop(FocusId::new(SEARCH_FOCUS), true);
        } else {
            ctx.register_focusable(FocusId::new(PANEL_FOCUS), popup_area, true);
            ctx.set_focus_tab_stop(FocusId::new(PANEL_FOCUS), true);
        }
        if self.scroll_highlight_on_next_layout {
            self.data_view.snap_highlight_centered(list_area);
            self.scroll_highlight_on_next_layout = false;
        }
        let mut child_ctx = LayoutCtx::new();
        <DataView<MenuItem<Id>, Id> as TuiNode<M>>::layout(
            &mut self.data_view,
            list_area,
            &mut child_ctx,
        );
        LayoutResult::new(trigger_area)
    }

    fn render_portal_popup(&self, frame: &mut Frame, bounds: Rect) {
        if !self.open || bounds.is_empty() {
            return;
        }
        let popup_area = self.popup_area_for(self.effective_trigger_area(bounds), bounds);
        if popup_area.is_empty() {
            return;
        }

        let trigger_area = self.effective_trigger_area(bounds);
        let backdrop = self.backdrop_tween.value();
        if backdrop > 0.0 {
            super::dialog_layer::dim_backdrop_buffer_except(
                frame,
                bounds,
                backdrop,
                &[trigger_area, popup_area],
            );
        }
        self.render_popup(frame, popup_area);
    }

    pub fn render<'a>(&'a self, _frame: &mut Frame, _area: Rect, ctx: &mut crate::RenderCtx<'a>) {
        if self.open {
            let bounds = self.overlay_bounds;
            ctx.push_portal(OverlayLayer::Popover, 0, bounds, |frame, bounds| {
                self.render_portal_popup(frame, bounds);
            });
        }
    }

    fn apply_open_close_context<M>(&self, outcome: MenuOutcome, ctx: &mut EventCtx<M>) {
        if outcome.opened || outcome.closed {
            ctx.request_layout();
        }
        if outcome.opened {
            self.request_open_focus(ctx);
        }
        if outcome.closed {
            self.request_return_focus(ctx);
        }
        if outcome.handled || outcome.changed {
            ctx.request_redraw();
        }
    }

    fn request_open_focus<M>(&self, ctx: &mut EventCtx<M>) {
        let id = if self.search_enabled() {
            SEARCH_FOCUS
        } else {
            PANEL_FOCUS
        };
        ctx.focus(FocusRequest::TargetAt {
            path: self.focus_path.clone(),
            id: FocusId::new(id),
        });
    }

    fn request_return_focus<M>(&self, ctx: &mut EventCtx<M>) {
        if let Some((path, id)) = &self.return_focus {
            ctx.focus(FocusRequest::TargetAt {
                path: path.clone(),
                id: id.clone(),
            });
        } else {
            ctx.focus(FocusRequest::Last);
        }
    }

    fn clear_search_query(&mut self) {
        if !self.search_input.current_value().is_empty() {
            self.search_input.set_value("");
            self.refresh_filter();
        }
    }

    fn activate_highlighted(&mut self) -> MenuOutcome {
        let Some(id) = self.data_view.highlighted_id() else {
            return MenuOutcome::HANDLED;
        };
        self.activated.push(id.clone());
        if let Some(on_activate) = &self.on_activate {
            on_activate(id);
        }
        let mut outcome = self.close();
        outcome.activated = true;
        outcome.changed = true;
        outcome
    }

    fn navigate_by(&mut self, delta: isize, area: Rect) -> MenuOutcome {
        if self.filtered.is_empty() {
            return MenuOutcome::HANDLED;
        }

        let current = self.data_view.highlighted_id();
        let current_index = current
            .as_ref()
            .and_then(|id| self.filtered.iter().position(|known| known == id))
            .unwrap_or(0);
        let target_index = target_index(current_index, delta, self.filtered.len());
        let target_id = self.filtered[target_index].clone();
        let outcome = self.data_view.highlight_id(&target_id);
        let scroll = self.data_view.snap_highlight_centered(self.list_area(area));
        MenuOutcome {
            handled: true,
            changed: outcome.changed || scroll.changed,
            ..MenuOutcome::IDLE
        }
    }

    fn list_viewport_height(&self, area: Rect) -> usize {
        let list_area = self.list_area(area);
        let height = list_area.height.saturating_sub(u16::from(
            self.needs_horizontal_scrollbar(list_area.width.saturating_add(2)),
        ));
        usize::from(height.max(1))
    }

    fn refresh_filter(&mut self) {
        let query = self.search_input.current_value();
        let query_empty = query.is_empty();
        self.search_render_query.replace(query.to_string());
        self.search_render_mode.set(self.search_mode);
        let filtered = match self.search_mode {
            MenuSearchMode::None => self.ids.clone(),
            MenuSearchMode::Contains if query_empty => self.ids.clone(),
            MenuSearchMode::Fuzzy if query_empty => self.ids.clone(),
            MenuSearchMode::Contains => self.search(SearchMode::Contains),
            MenuSearchMode::Fuzzy => self.search(SearchMode::Fuzzy),
        };

        self.filtered = filtered;
        if self.search_mode == MenuSearchMode::None || query_empty {
            self.data_view.clear_visible_row_ids();
        } else {
            self.data_view.set_visible_row_ids(self.filtered.clone());
        }
    }

    fn highlight_first_filtered(&mut self) {
        let Some(first) = self.filtered.first() else {
            return;
        };
        self.data_view.highlight_id(first);
    }

    fn search(&self, mode: SearchMode) -> Vec<Id> {
        search_ranked(self.search_input.current_value(), &self.labels, mode)
            .into_iter()
            .map(|matched| self.ids[matched.index].clone())
            .collect()
    }

    fn search_enabled(&self) -> bool {
        self.search_mode != MenuSearchMode::None
    }

    fn sync_child_focus(&mut self) {
        let search_focused = self.open && self.focus_region == Some(MenuFocusRegion::Search);
        self.search_input.set_focused(search_focused);
        self.search_input.set_insert_mode(search_focused);
        self.data_view
            .set_focused(self.open && self.focus_region.is_some());
    }

    fn popup_inner_areas(&self, popup_area: Rect) -> [Rect; 2] {
        if popup_area.is_empty() {
            return [popup_area, popup_area];
        }
        let popup_inner = Block::default().borders(Borders::ALL).inner(popup_area);
        let search_height = u16::from(self.search_enabled());
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(search_height), Constraint::Fill(1)])
            .areas(popup_inner)
    }

    fn effective_trigger_area(&self, bounds: Rect) -> Rect {
        if self.trigger_area.is_empty() {
            Rect::new(
                bounds.x,
                bounds.y,
                self.min_popup_width.min(bounds.width),
                1,
            )
        } else {
            self.trigger_area
        }
    }

    fn popup_area_for(&self, trigger_area: Rect, bounds: Rect) -> Rect {
        if !self.open || trigger_area.is_empty() || bounds.is_empty() {
            return Rect::default();
        }

        let width = self
            .measured_popup_width()
            .max(trigger_area.width)
            .min(bounds.width);
        let desired_height = self
            .popup_content_height(width)
            .min(self.effective_max_popup_height(width));
        let x = trigger_area
            .x
            .min(bounds.x.saturating_add(bounds.width).saturating_sub(width));
        let (y, available_height) = match self.popup_direction {
            MenuPopupDirection::Down => {
                let y = trigger_area.y.saturating_add(trigger_area.height);
                let available = bounds.y.saturating_add(bounds.height).saturating_sub(y);
                (y, available)
            }
            MenuPopupDirection::Up => {
                let available = trigger_area.y.saturating_sub(bounds.y);
                let height = desired_height.min(available);
                (trigger_area.y.saturating_sub(height), available)
            }
        };
        let popup_area = Rect::new(x, y, width, desired_height.min(available_height));
        clip_rect(popup_area, bounds)
    }

    fn popup_content_height(&self, width: u16) -> u16 {
        POPUP_BORDER_HEIGHT
            .saturating_add(u16::from(self.search_enabled()))
            .saturating_add(self.visible_popup_rows())
            .saturating_add(u16::from(self.needs_horizontal_scrollbar(width)))
    }

    fn visible_popup_rows(&self) -> u16 {
        self.filtered.len().max(1).min(usize::from(u16::MAX)) as u16
    }

    fn needs_horizontal_scrollbar(&self, width: u16) -> bool {
        let viewport_width = width.saturating_sub(2);
        let content_width = self
            .filtered
            .iter()
            .filter_map(|id| self.label_for(id))
            .map(|label| line_width(&Line::from(label)))
            .max()
            .unwrap_or_else(|| line_width(&Line::from("No results")));
        content_width > viewport_width as usize
    }

    fn measured_popup_width(&self) -> u16 {
        let label_width = self
            .labels
            .iter()
            .map(|label| line_width(&Line::from(label.as_str())))
            .max()
            .unwrap_or(0)
            .saturating_add(2)
            .min(u16::MAX as usize) as u16;
        self.min_popup_width.max(label_width)
    }

    fn effective_max_popup_height(&self, width: u16) -> u16 {
        self.max_popup_height
            .unwrap_or_else(|| {
                POPUP_BORDER_HEIGHT
                    .saturating_add(u16::from(self.search_enabled()))
                    .saturating_add(self.visible_items)
                    .saturating_add(u16::from(self.needs_horizontal_scrollbar(width)))
            })
            .max(1)
    }

    fn list_area(&self, area: Rect) -> Rect {
        let popup_area = self.popup_area_for(self.effective_trigger_area(area), area);
        if popup_area.is_empty() {
            return popup_area;
        }
        let [_, list_area] = self.popup_inner_areas(popup_area);
        list_area
    }

    fn label_for(&self, id: &Id) -> Option<String> {
        self.ids
            .iter()
            .position(|known| known == id)
            .map(|index| self.labels[index].clone())
    }

    fn render_popup(&self, frame: &mut Frame, area: Rect) {
        let theme = theme();
        frame.render_widget(Clear, area);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border_set(preset().border()))
            .border_style(Style::default().fg(if self.focus_region.is_some() {
                theme.accent_fg()
            } else {
                theme.border_fg()
            }));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let [search_area, list_area] = self.popup_inner_areas(area);
        if self.search_enabled() {
            self.search_input.render(frame, search_area);
        }

        if self.filtered.is_empty() {
            let line = Line::styled(
                "No results",
                Style::default()
                    .fg(theme.muted_fg())
                    .add_modifier(Modifier::ITALIC),
            );
            frame.render_widget(Paragraph::new(line), list_area);
        } else {
            let list_area = Rect::new(inner.x, list_area.y, inner.width, list_area.height);
            self.data_view.render_with_row_style(frame, list_area, None);
        }
    }
}

impl<Id, M> TuiNode<M> for Menu<Id>
where
    Id: Clone + Eq + Hash + 'static,
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        LayoutSizeHint {
            source: HintSource::Measured,
            min: LayoutSize::new(0, 0),
            preferred: LayoutSize::new(0, 0),
            expand: crate::AxisExpand::default(),
        }
        .normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.layout_with_current_bounds::<M>(area, ctx)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut crate::RenderCtx<'a>) {
        self.render(frame, area, ctx);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };

        let bindings = keybindings();
        let focus_keys = bindings.focus();
        if self.open && focus_keys.next_matches(*key) {
            self.close();
            ctx.focus_next();
            ctx.request_layout();
            ctx.request_redraw();
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        if self.open && focus_keys.previous_matches(*key) {
            self.close();
            ctx.focus_previous();
            ctx.request_layout();
            ctx.request_redraw();
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }

        let outcome = self.on_key(*key, self.overlay_bounds);
        self.apply_open_close_context(outcome, ctx);
        if outcome.handled {
            ctx.stop_propagation();
            EventOutcome::Handled
        } else {
            EventOutcome::Ignored
        }
    }

    fn dispatch_event(
        &mut self,
        route: &crate::EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        if !route.path.is_empty() {
            return EventOutcome::Ignored;
        }
        self.event(event, ctx)
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(self, dt, settings)
    }

    fn focus(&mut self, target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        if focused && target.is_some_and(|id| id.as_str() == SEARCH_FOCUS) {
            self.focus_region = Some(MenuFocusRegion::Search);
            self.sync_child_focus();
        } else if focused && target.is_some_and(|id| id.as_str() == PANEL_FOCUS) {
            self.focus_region = Some(MenuFocusRegion::Panel);
            self.sync_child_focus();
        } else if !focused && self.open {
            self.close();
        } else if !focused {
            self.focus_region = None;
            self.sync_child_focus();
        }
        ctx.request_redraw();
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        if target.path.is_empty() {
            self.focus(Some(&target.id), focused, ctx);
        }
    }
}

impl<Id> Animated for Menu<Id>
where
    Id: Clone + Eq + Hash,
{
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        let hotkey_tick = if self.trigger_hotkey_matcher.tick(dt) {
            TickResult::CHANGED
        } else {
            TickResult::IDLE
        };
        hotkey_tick
            .merge(Animated::tick(&mut self.data_view, dt, settings))
            .merge(Animated::tick(&mut self.search_input, dt, settings))
            .merge(self.backdrop_tween.tick(dt, settings))
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::{Modifier, Style};

    use super::*;

    #[test]
    fn open_popup_dims_background_outside_trigger_and_popup() {
        let mut menu = Menu::new([MenuItem::new(1, "Alpha"), MenuItem::new(2, "Beta")]);
        menu.open();
        let mut layout = LayoutCtx::new();
        layout.with_overlay_bounds(Rect::new(0, 0, 20, 8), |ctx| {
            <Menu<_> as TuiNode<()>>::layout(&mut menu, Rect::new(4, 1, 5, 1), ctx);
        });
        let mut terminal = Terminal::new(TestBackend::new(20, 8)).expect("terminal should build");

        terminal
            .draw(|frame| {
                frame
                    .buffer_mut()
                    .set_string(0, 0, "background", Style::default());
                let mut render = crate::RenderCtx::new();
                <Menu<_> as TuiNode<()>>::render(&menu, frame, Rect::new(4, 1, 5, 1), &mut render);
                render.flush(frame);
            })
            .expect("menu should render");

        let cell = terminal.backend().buffer().cell((0, 0)).unwrap();
        assert!(cell.modifier.contains(Modifier::DIM));
    }

    #[test]
    fn close_restores_focus_held_before_menu_opened() {
        let mut menu = Menu::new([MenuItem::new(1, "Alpha")]);
        let mut open_ctx = EventCtx::<()>::default();
        menu.open_with_context(&mut open_ctx);
        let mut close_ctx = EventCtx::<()>::default();

        menu.close_with_context(&mut close_ctx);

        assert_eq!(close_ctx.focus_request(), Some(&FocusRequest::Last));
    }
}
