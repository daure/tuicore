use std::hash::Hash;
use std::rc::Rc;
use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::symbols::border::Set;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::components::{Column, DataView, SelectionMode, TextInput};
use crate::event::{Key, KeyEvent};
use crate::search::{SearchMode, search_ranked};
use crate::{
    Animated, AnimationSettings, ChildKey, EventCtx, EventOutcome, EventRoute, FocusCtx, FocusId,
    FocusTarget, HintSource, LayoutCtx, LayoutProposal, LayoutResult, LayoutSize, LayoutSizeHint,
    TickResult, TuiEvent, TuiNode, border_chars, border_set, keybindings, line_width, preset,
    theme,
};

const FIELD_FOCUS: &str = "field";
const SEARCH_FOCUS: &str = "input";
const SEARCH_SLOT: &str = "search";
const LIST_SLOT: &str = "list";
const POPUP_BORDER_HEIGHT: u16 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DropdownFocusRegion {
    Field,
    Search,
    List,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropdownSearchMode {
    None,
    Contains,
    Fuzzy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropdownCommitMode {
    Explicit,
    Immediate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DropdownVariant {
    #[default]
    Bordered,
    Filled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DropdownOutcome {
    pub handled: bool,
    pub changed: bool,
    pub opened: bool,
    pub closed: bool,
    pub committed: bool,
    pub canceled: bool,
}

impl DropdownOutcome {
    pub const IDLE: Self = Self {
        handled: false,
        changed: false,
        opened: false,
        closed: false,
        committed: false,
        canceled: false,
    };

    pub const HANDLED: Self = Self {
        handled: true,
        changed: false,
        opened: false,
        closed: false,
        committed: false,
        canceled: false,
    };

    fn changed() -> Self {
        Self {
            handled: true,
            changed: true,
            ..Self::IDLE
        }
    }
}

pub struct Dropdown<T, Id> {
    data_view: DataView<T, Id>,
    search_input: TextInput,
    ids: Vec<Id>,
    labels: Vec<String>,
    committed: Vec<Id>,
    draft: Vec<Id>,
    filtered: Vec<Id>,
    multi: bool,
    open: bool,
    search_mode: DropdownSearchMode,
    commit_mode: DropdownCommitMode,
    close_on_select: bool,
    max_popup_height: Option<u16>,
    auto_focus_search: bool,
    placeholder: String,
    variant: DropdownVariant,
    field_area: Rect,
    overlay_bounds: Rect,
    focus_region: Option<DropdownFocusRegion>,
}

impl<T, Id> Dropdown<T, Id>
where
    T: 'static,
    Id: Clone + Eq + Hash + 'static,
{
    pub fn single(
        rows: impl IntoIterator<Item = T>,
        row_id: impl Fn(&T) -> Id + 'static,
        label: impl Fn(&T) -> String + 'static,
    ) -> Self {
        Self::new(rows, row_id, label, false)
    }

    pub fn multi(
        rows: impl IntoIterator<Item = T>,
        row_id: impl Fn(&T) -> Id + 'static,
        label: impl Fn(&T) -> String + 'static,
    ) -> Self {
        Self::new(rows, row_id, label, true)
    }

    fn new(
        rows: impl IntoIterator<Item = T>,
        row_id: impl Fn(&T) -> Id + 'static,
        label: impl Fn(&T) -> String + 'static,
        multi: bool,
    ) -> Self {
        let rows = rows.into_iter().collect::<Vec<_>>();
        let row_id = Rc::new(row_id);
        let label = Rc::new(label);
        let ids = rows.iter().map(|row| row_id(row)).collect::<Vec<_>>();
        let labels = rows.iter().map(|row| label(row)).collect::<Vec<_>>();
        let data_view_row_id = Rc::clone(&row_id);
        let data_view_label = Rc::clone(&label);
        let selection_mode = if multi {
            SelectionMode::Multi
        } else {
            SelectionMode::Single
        };
        let data_view = DataView::new(rows, move |row| data_view_row_id(row))
            .column(Column::text(
                "label",
                "",
                Constraint::Percentage(100),
                move |row| data_view_label(row),
            ))
            .selection_mode(selection_mode)
            .focused(false);

        Self {
            data_view,
            search_input: TextInput::new().placeholder("Search..."),
            filtered: ids.clone(),
            ids,
            labels,
            committed: Vec::new(),
            draft: Vec::new(),
            multi,
            open: false,
            search_mode: DropdownSearchMode::Fuzzy,
            commit_mode: DropdownCommitMode::Explicit,
            close_on_select: !multi,
            max_popup_height: None,
            auto_focus_search: true,
            placeholder: String::from("Select..."),
            variant: DropdownVariant::Bordered,
            field_area: Rect::default(),
            overlay_bounds: Rect::default(),
            focus_region: None,
        }
    }

    pub fn search_mode(mut self, mode: DropdownSearchMode) -> Self {
        self.search_mode = mode;
        self.refresh_filter();
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn commit_mode(mut self, mode: DropdownCommitMode) -> Self {
        self.commit_mode = mode;
        self
    }

    pub fn close_on_select(mut self, close: bool) -> Self {
        self.close_on_select = close;
        self
    }

    pub fn max_popup_height(mut self, height: u16) -> Self {
        self.max_popup_height = Some(height.max(1));
        self
    }

    pub fn auto_focus_search(mut self, auto_focus: bool) -> Self {
        self.auto_focus_search = auto_focus;
        self
    }

    pub fn variant(mut self, variant: DropdownVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn selected(mut self, ids: impl IntoIterator<Item = Id>) -> Self {
        self.committed = self.known_ids(ids);
        if !self.multi {
            self.committed.truncate(1);
        }
        self.draft = self.committed.clone();
        self.sync_view_selection();
        self
    }

    pub fn selected_one(self, id: Id) -> Self {
        self.selected([id])
    }

    pub fn selected_id(&self) -> Option<Id> {
        self.committed.first().cloned()
    }

    pub fn selected_ids(&self) -> Vec<Id> {
        self.committed.clone()
    }

    pub fn search_query(&self) -> &str {
        self.search_input.current_value()
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn is_focused(&self) -> bool {
        self.focus_region.is_some()
    }

    pub fn open(&mut self) -> DropdownOutcome {
        if self.open {
            return DropdownOutcome::HANDLED;
        }

        self.open = true;
        self.draft = self.committed.clone();
        self.highlight_committed();
        if !self.multi && self.draft.is_empty() {
            self.set_single_draft_from_highlight();
        }
        self.sync_focus();
        self.refresh_filter();
        self.sync_view_selection();
        DropdownOutcome {
            handled: true,
            changed: true,
            opened: true,
            ..DropdownOutcome::IDLE
        }
    }

    pub fn close(&mut self) -> DropdownOutcome {
        if !self.open {
            return DropdownOutcome::HANDLED;
        }
        let had_focus = self.is_focused();
        self.open = false;
        self.clear_search_query();
        if had_focus {
            self.sync_focus();
        } else {
            self.clear_focus();
        }
        DropdownOutcome {
            handled: true,
            changed: true,
            closed: true,
            ..DropdownOutcome::IDLE
        }
    }

    fn clear_search_query(&mut self) {
        if !self.search_input.current_value().is_empty() {
            self.search_input.set_value("");
            self.refresh_filter();
        }
    }

    pub fn cancel(&mut self) -> DropdownOutcome {
        self.draft = self.committed.clone();
        self.sync_view_selection();
        let mut outcome = self.close();
        outcome.canceled = true;
        outcome.handled = true;
        outcome.changed = true;
        outcome
    }

    pub fn commit(&mut self) -> DropdownOutcome {
        if !self.multi && self.draft.is_empty() {
            self.set_single_draft_from_highlight();
        }
        let changed = self.committed != self.draft;
        self.committed = self.draft.clone();
        let mut outcome = self.close();
        outcome.committed = true;
        outcome.changed = outcome.changed || changed;
        outcome
    }

    pub fn on_key(&mut self, key: impl Into<KeyEvent>, area: Rect) -> DropdownOutcome {
        let key = key.into();
        if !self.open {
            return self.on_closed_key(key);
        }

        match key.code {
            Key::Esc => return self.cancel(),
            Key::Enter => return self.commit(),
            Key::Char(' ') if self.multi && key.modifiers.is_empty() => {
                return self.toggle_highlighted();
            }
            _ => {}
        }

        let keys = keybindings();
        if keys.dropdown().select_matches(key) {
            return self.select_highlighted();
        }
        if keys.dropdown().next_matches(key) {
            return self.navigate(Key::Down, area);
        }
        if keys.dropdown().previous_matches(key) {
            return self.navigate(Key::Up, area);
        }
        if keys.dropdown().page_next_matches(key) {
            return self.navigate(Key::PageDown, area);
        }
        if keys.dropdown().page_previous_matches(key) {
            return self.navigate(Key::PageUp, area);
        }

        if self.search_enabled() {
            let input = self.search_input.on_key(key);
            if input.changed {
                self.refresh_filter();
                if !self.multi {
                    self.set_single_draft_from_highlight();
                    self.sync_view_selection();
                }
            }
            if input.needs_redraw() {
                return DropdownOutcome {
                    handled: true,
                    changed: input.changed,
                    ..DropdownOutcome::IDLE
                };
            }
        }

        DropdownOutcome::IDLE
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let field_area = self.field_area(area);
        self.render_field(frame, field_area);
    }

    pub fn render_popup_overlay(&self, frame: &mut Frame, bounds: Rect) {
        if !self.open || bounds.is_empty() {
            return;
        }

        let popup_area = self.popup_overlay_area(bounds);
        if !popup_area.is_empty() {
            self.render_popup(frame, popup_area);
        }
    }

    pub fn popup_overlay_area(&self, bounds: Rect) -> Rect {
        let field_area = self.effective_field_area(bounds);
        self.popup_area_for(field_area, bounds)
    }

    pub fn layout_overlay<M>(
        &mut self,
        area: Rect,
        overlay_bounds: Rect,
        ctx: &mut LayoutCtx,
    ) -> LayoutResult {
        self.field_area = self.field_area(area);
        self.overlay_bounds = overlay_bounds;
        if !self.open {
            ctx.register_focusable(FocusId::new(FIELD_FOCUS), self.field_area, true);
            return LayoutResult::new(self.field_area);
        }

        let popup_area = self.popup_area_for(self.field_area, overlay_bounds);
        let [search_area, list_area] = self.popup_inner_areas(popup_area);
        if self.search_enabled() {
            ctx.push_slot(ChildKey::new(SEARCH_SLOT), search_area, |ctx| {
                ctx.register_focusable(FocusId::new(SEARCH_FOCUS), search_area, true);
            });
        }
        ctx.push_slot(ChildKey::new(LIST_SLOT), list_area, |ctx| {
            <DataView<T, Id> as TuiNode<M>>::layout(&mut self.data_view, list_area, ctx);
        });
        LayoutResult::new(self.field_area)
    }

    fn on_closed_key(&mut self, key: KeyEvent) -> DropdownOutcome {
        let keys = keybindings();
        if keys.dropdown().next_matches(key) || keys.dropdown().previous_matches(key) {
            return self.open();
        }

        match key.code {
            Key::Enter | Key::Char(' ') if key.modifiers.is_empty() => self.open(),
            _ => DropdownOutcome::IDLE,
        }
    }

    fn navigate(&mut self, key: Key, area: Rect) -> DropdownOutcome {
        let outcome = self
            .data_view
            .on_key(KeyEvent::from(key), self.list_area(area));
        if !self.multi {
            self.set_single_draft_from_highlight();
            self.sync_view_selection();
            if self.commit_mode == DropdownCommitMode::Immediate {
                self.committed = self.draft.clone();
            }
        }
        DropdownOutcome {
            handled: outcome.handled,
            changed: outcome.changed,
            committed: !self.multi && self.commit_mode == DropdownCommitMode::Immediate,
            ..DropdownOutcome::IDLE
        }
    }

    fn toggle_highlighted(&mut self) -> DropdownOutcome {
        let Some(id) = self.data_view.highlighted_id() else {
            return DropdownOutcome::HANDLED;
        };
        self.data_view.toggle_selected(id);
        self.draft = self.data_view.selected_ids();
        if self.close_on_select {
            self.committed = self.draft.clone();
            let mut outcome = self.close();
            outcome.committed = true;
            return outcome;
        }
        DropdownOutcome::changed()
    }

    fn select_highlighted(&mut self) -> DropdownOutcome {
        if self.multi {
            return self.toggle_highlighted();
        }

        self.set_single_draft_from_highlight();
        self.sync_view_selection();
        self.commit()
    }

    fn refresh_filter(&mut self) {
        let query = self.search_input.current_value();
        let filtered = match self.search_mode {
            DropdownSearchMode::None if query.is_empty() => self.ids.clone(),
            DropdownSearchMode::None => self.ids.clone(),
            DropdownSearchMode::Contains if query.is_empty() => self.ids.clone(),
            DropdownSearchMode::Fuzzy if query.is_empty() => self.ids.clone(),
            DropdownSearchMode::Contains => self.search(SearchMode::Contains),
            DropdownSearchMode::Fuzzy => self.search(SearchMode::Fuzzy),
        };

        self.filtered = filtered;
        if self.search_mode == DropdownSearchMode::None || query.is_empty() {
            self.data_view.clear_visible_row_ids();
        } else {
            self.data_view.set_visible_row_ids(self.filtered.clone());
        }
    }

    fn search(&self, mode: SearchMode) -> Vec<Id> {
        search_ranked(self.search_input.current_value(), &self.labels, mode)
            .into_iter()
            .map(|matched| self.ids[matched.index].clone())
            .collect()
    }

    fn sync_view_selection(&mut self) {
        self.data_view.clear_selection();
        for id in self.draft.clone() {
            self.data_view.select_id(id);
        }
        self.data_view.drain_events();
    }

    fn highlight_committed(&mut self) {
        if let Some(id) = self.committed.first() {
            self.data_view.highlight_id(id);
            self.data_view.drain_events();
        }
    }

    fn set_single_draft_from_highlight(&mut self) {
        let Some(id) = self.data_view.highlighted_id() else {
            self.draft.clear();
            return;
        };
        self.draft = vec![id];
    }

    fn known_ids(&self, ids: impl IntoIterator<Item = Id>) -> Vec<Id> {
        ids.into_iter()
            .filter(|id| self.ids.iter().any(|known| known == id))
            .collect()
    }

    fn search_enabled(&self) -> bool {
        self.search_mode != DropdownSearchMode::None
    }

    fn sync_focus(&mut self) {
        let region = if self.open {
            if self.search_enabled() && self.auto_focus_search {
                Some(DropdownFocusRegion::Search)
            } else {
                Some(DropdownFocusRegion::List)
            }
        } else {
            Some(DropdownFocusRegion::Field)
        };
        self.set_focus_region(region);
    }

    fn clear_focus(&mut self) {
        self.set_focus_region(None);
    }

    fn set_focus_region(&mut self, region: Option<DropdownFocusRegion>) {
        self.focus_region = region;
        self.search_input
            .set_focused(region == Some(DropdownFocusRegion::Search));
        self.data_view.set_focused(matches!(
            region,
            Some(DropdownFocusRegion::Search | DropdownFocusRegion::List)
        ));
    }

    fn popup_inner_areas(&self, popup_area: Rect) -> [Rect; 2] {
        if popup_area.is_empty() {
            return [popup_area, popup_area];
        }
        let popup_inner = if self.popup_has_border() {
            Block::default().borders(Borders::ALL).inner(popup_area)
        } else {
            popup_area
        };
        let search_height = u16::from(self.search_enabled());
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(search_height), Constraint::Fill(1)])
            .areas(popup_inner)
    }

    fn selected_summary(&self) -> String {
        let ids = &self.committed;
        if ids.is_empty() {
            return self.placeholder.clone();
        }
        if self.multi && ids.len() > 1 {
            return format!("{} selected", ids.len());
        }
        self.label_for(&ids[0])
            .unwrap_or_else(|| self.placeholder.clone())
    }

    fn label_for(&self, id: &Id) -> Option<String> {
        self.ids
            .iter()
            .position(|known| known == id)
            .map(|index| self.labels[index].clone())
    }

    #[cfg(test)]
    fn areas(&self, area: Rect) -> [Rect; 2] {
        let field_area = self.field_area(area);
        let popup_area = self.popup_area_for(field_area, area);

        [field_area, popup_area]
    }

    fn field_area(&self, area: Rect) -> Rect {
        let field_height = self.field_height(area);
        Rect::new(area.x, area.y, area.width, field_height)
    }

    fn effective_field_area(&self, bounds: Rect) -> Rect {
        if self.field_area.is_empty() {
            self.field_area(bounds)
        } else {
            self.field_area
        }
    }

    fn popup_area_for(&self, field_area: Rect, bounds: Rect) -> Rect {
        if !self.open || field_area.is_empty() || bounds.is_empty() {
            return Rect::default();
        }

        let popup_y = field_area
            .y
            .saturating_add(field_area.height)
            .saturating_sub(self.popup_overlap());
        let available_height = bounds
            .y
            .saturating_add(bounds.height)
            .saturating_sub(popup_y);
        let popup_height = self
            .popup_content_height(field_area.width)
            .min(self.effective_max_popup_height())
            .min(available_height);
        let popup_area = Rect::new(field_area.x, popup_y, field_area.width, popup_height);

        clip_rect(popup_area, bounds)
    }

    fn field_height(&self, area: Rect) -> u16 {
        match self.variant {
            DropdownVariant::Bordered => area.height.min(3),
            DropdownVariant::Filled => area.height.min(1),
        }
    }

    fn popup_content_height(&self, width: u16) -> u16 {
        self.popup_border_height()
            .saturating_add(u16::from(self.search_enabled()))
            .saturating_add(self.visible_popup_rows())
            .saturating_add(u16::from(self.needs_horizontal_scrollbar(width)))
    }

    fn popup_border_height(&self) -> u16 {
        if self.popup_has_border() {
            POPUP_BORDER_HEIGHT
        } else {
            0
        }
    }

    fn popup_has_border(&self) -> bool {
        self.variant == DropdownVariant::Bordered
    }

    fn popup_overlap(&self) -> u16 {
        u16::from(self.popup_has_border())
    }

    fn visible_popup_rows(&self) -> u16 {
        if self.filtered.is_empty() {
            return 1;
        }
        self.filtered.len().min(usize::from(u16::MAX)) as u16
    }

    fn needs_horizontal_scrollbar(&self, width: u16) -> bool {
        let viewport_width = width.saturating_sub(if self.popup_has_border() { 2 } else { 0 });
        let prefix_width = if self.multi { 2 } else { 0 };
        let content_width = self
            .filtered
            .iter()
            .filter_map(|id| self.label_for(id))
            .map(|label| line_width(&Line::from(label)).saturating_add(prefix_width))
            .max()
            .unwrap_or_else(|| line_width(&Line::from("No results")));
        content_width > viewport_width as usize
    }

    fn effective_max_popup_height(&self) -> u16 {
        self.max_popup_height
            .unwrap_or_else(|| preset().dropdown().max_popup_height())
            .max(1)
    }

    fn list_area(&self, area: Rect) -> Rect {
        let popup_area = self.popup_overlay_area(area);
        if popup_area.is_empty() {
            return popup_area;
        }
        let [_, list_area] = self.popup_inner_areas(popup_area);
        list_area
    }

    fn render_field(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        match self.variant {
            DropdownVariant::Bordered => self.render_bordered_field(frame, area),
            DropdownVariant::Filled => self.render_filled_field(frame, area),
        }
    }

    fn render_bordered_field(&self, frame: &mut Frame, area: Rect) {
        let theme = theme();
        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border_set(preset().border()))
            .border_style(Style::default().fg(if self.is_focused() {
                theme.accent_fg()
            } else {
                theme.border_fg()
            }));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let style = if self.committed.is_empty() {
            Style::default().fg(theme.muted_fg())
        } else {
            Style::default().fg(theme.text_fg())
        };
        frame.render_widget(Paragraph::new(self.selected_summary()).style(style), inner);
    }

    fn render_filled_field(&self, frame: &mut Frame, area: Rect) {
        let theme = theme();
        let base_style = Style::default()
            .fg(theme.highlight_fg())
            .bg(theme.highlight_bg());
        let text_style = if self.committed.is_empty() {
            base_style.add_modifier(Modifier::DIM)
        } else {
            base_style
        };

        frame.render_widget(Paragraph::new("").style(base_style), area);

        let text_area = Rect::new(
            area.x.saturating_add(1),
            area.y,
            area.width.saturating_sub(2),
            1,
        );
        if !text_area.is_empty() {
            frame.render_widget(
                Paragraph::new(self.selected_summary()).style(text_style),
                text_area,
            );
        }

        let arrow_area = Rect::new(area.x + area.width.saturating_sub(1), area.y, 1, 1);
        frame.render_widget(
            Paragraph::new("")
                .style(base_style)
                .alignment(Alignment::Right),
            arrow_area,
        );
    }

    fn render_popup(&self, frame: &mut Frame, area: Rect) {
        let theme = theme();
        frame.render_widget(Clear, area);
        let popup_content_style = self.popup_content_style();
        let inner = if self.popup_has_border() {
            let border = if self.variant == DropdownVariant::Bordered {
                connected_popup_border_set(preset().border())
            } else {
                border_set(preset().border())
            };
            let block = Block::default()
                .borders(Borders::ALL)
                .border_set(border)
                .border_style(Style::default().fg(if self.is_focused() {
                    theme.accent_fg()
                } else {
                    theme.border_fg()
                }));
            let inner = block.inner(area);
            frame.render_widget(block, area);
            inner
        } else {
            frame.render_widget(
                Paragraph::new("").style(popup_content_style.unwrap_or_default()),
                area,
            );
            area
        };

        let search_height = u16::from(self.search_enabled());
        let [search_area, list_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(search_height), Constraint::Fill(1)])
            .areas(inner);
        if self.search_enabled() {
            if let Some(style) = popup_content_style {
                self.search_input
                    .render_with_style(frame, search_area, style);
            } else {
                self.search_input.render(frame, search_area);
            }
        }
        if self.filtered.is_empty() {
            let line = Line::styled(
                "No results",
                popup_content_style
                    .unwrap_or_default()
                    .fg(theme.muted_fg())
                    .add_modifier(Modifier::ITALIC),
            );
            frame.render_widget(
                Paragraph::new(line).style(popup_content_style.unwrap_or_default()),
                list_area,
            );
        } else {
            self.data_view
                .render_with_row_style(frame, list_area, popup_content_style);
        }
    }

    fn popup_content_style(&self) -> Option<Style> {
        (self.variant == DropdownVariant::Filled).then(|| Style::default().bg(theme().border_fg()))
    }
}

impl<T, Id, M> TuiNode<M> for Dropdown<T, Id>
where
    T: 'static,
    Id: Clone + Eq + Hash + 'static,
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let height = match self.variant {
            DropdownVariant::Bordered => 3,
            DropdownVariant::Filled => 1,
        };
        let width = (self.selected_summary().chars().count() as u16).saturating_add(2);
        LayoutSizeHint {
            source: HintSource::Measured,
            min: LayoutSize::new(width, height),
            preferred: LayoutSize::new(width, height),
            expand: crate::AxisExpand {
                width: true,
                height: false,
            },
        }
        .normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.layout_overlay::<M>(area, area, ctx)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        Self::render(self, frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };
        let outcome = self.on_key(*key, self.overlay_bounds);
        if outcome.opened || outcome.closed {
            ctx.request_layout();
        }
        if outcome.handled || outcome.changed {
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
        _route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        self.event(event, ctx)
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(self, dt, settings)
    }

    fn focus(&mut self, target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        if focused && target.is_some_and(|id| id.as_str() == FIELD_FOCUS) {
            self.set_focus_region(Some(DropdownFocusRegion::Field));
        } else if !focused {
            self.clear_focus();
        }
        ctx.request_redraw();
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        if target.path.is_empty() {
            self.focus(Some(&target.id), focused, ctx);
            return;
        }

        let search = ChildKey::new(SEARCH_SLOT);
        if target.for_child(&search).is_some() {
            if focused {
                self.set_focus_region(Some(DropdownFocusRegion::Search));
            } else {
                self.clear_focus();
            }
            ctx.request_redraw();
            return;
        }

        let list = ChildKey::new(LIST_SLOT);
        if let Some(child_target) = target.for_child(&list) {
            if focused {
                self.set_focus_region(Some(DropdownFocusRegion::List));
            } else {
                self.clear_focus();
            }
            self.data_view.dispatch_focus(&child_target, focused, ctx);
            ctx.request_redraw();
        }
    }
}

impl<T, Id> Animated for Dropdown<T, Id>
where
    Id: Clone + Eq + Hash,
{
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(&mut self.data_view, dt, settings).merge(Animated::tick(
            &mut self.search_input,
            dt,
            settings,
        ))
    }
}

fn connected_popup_border_set(border: crate::BorderKind) -> Set<'static> {
    let chars = border_chars(border);
    Set {
        top_left: chars.left_join,
        top_right: chars.right_join,
        bottom_left: chars.bottom_left,
        bottom_right: chars.bottom_right,
        vertical_left: chars.vertical,
        vertical_right: chars.vertical,
        horizontal_top: chars.horizontal,
        horizontal_bottom: chars.horizontal,
    }
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

#[cfg(test)]
mod tests {
    use std::hash::Hash;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;

    use super::*;
    use crate::event::KeyModifiers;
    use crate::{
        ChildKey, EventCtx, Flex, FlexItem, FocusCtx, LayoutCtx, LayoutProposal, Propagation,
        TuiEvent, TuiNode,
    };

    fn single_dropdown() -> Dropdown<&'static str, &'static str> {
        Dropdown::single(ROWS, |row| *row, |row| row.to_string())
    }

    fn multi_dropdown() -> Dropdown<&'static str, &'static str> {
        Dropdown::multi(ROWS, |row| *row, |row| row.to_string())
    }

    fn numeric_dropdown(count: u8) -> Dropdown<u8, u8> {
        Dropdown::single(0..count, |row| *row, |row| row.to_string())
    }

    const ROWS: [&str; 3] = ["Alpha", "Beta", "Gamma"];
    const AREA: Rect = Rect::new(0, 0, 24, 10);

    #[test]
    fn open_clones_committed_selection_to_draft() {
        let mut dropdown = single_dropdown().selected_one("Beta");

        dropdown.open();
        dropdown.on_key(ctrl('j'), AREA);
        dropdown.cancel();

        assert_eq!(dropdown.selected_id(), Some("Beta"));
    }

    #[test]
    fn enter_commits_single_draft() {
        let mut dropdown = single_dropdown();

        dropdown.open();
        dropdown.on_key(ctrl('j'), AREA);
        dropdown.on_key(Key::Enter, AREA);

        assert_eq!(dropdown.selected_id(), Some("Beta"));
        assert!(!dropdown.is_open());
    }

    #[test]
    fn ctrl_d_and_ctrl_u_page_navigate_defaults() {
        let mut dropdown = single_dropdown();

        dropdown.open();
        dropdown.on_key(ctrl('d'), AREA);
        assert_eq!(dropdown.data_view.highlighted_id(), Some("Gamma"));

        dropdown.on_key(ctrl('u'), AREA);
        assert_eq!(dropdown.data_view.highlighted_id(), Some("Alpha"));
    }

    #[test]
    fn closed_ctrl_j_and_ctrl_k_open_without_navigating() {
        for key in [ctrl('j'), ctrl('k')] {
            let mut dropdown = single_dropdown();

            let outcome = dropdown.on_key(key, AREA);

            assert!(outcome.opened);
            assert!(dropdown.is_open());
            assert_eq!(dropdown.data_view.highlighted_id(), Some("Alpha"));
        }
    }

    #[test]
    fn closed_plain_j_and_k_do_not_open() {
        for key in [char_key('j'), char_key('k')] {
            let mut dropdown = single_dropdown();

            let outcome = dropdown.on_key(key, AREA);

            assert!(!outcome.opened);
            assert!(!dropdown.is_open());
        }
    }

    #[test]
    fn ctrl_d_and_ctrl_u_page_navigation_moves_by_visible_page_step() {
        let mut dropdown = numeric_dropdown(20);

        dropdown.open();
        dropdown.on_key(char_key('1'), AREA);
        dropdown.on_key(ctrl('d'), AREA);

        assert_eq!(dropdown.search_query(), "1");
        assert!(dropdown.data_view.highlighted_id().unwrap() > 1);

        dropdown.on_key(ctrl('u'), AREA);
        assert_eq!(dropdown.search_query(), "1");
        assert_eq!(dropdown.data_view.highlighted_id(), Some(1));
    }

    #[test]
    fn escape_rolls_back_single_draft() {
        let mut dropdown = single_dropdown().selected_one("Alpha");

        dropdown.open();
        dropdown.on_key(ctrl('j'), AREA);
        dropdown.on_key(Key::Esc, AREA);

        assert_eq!(dropdown.selected_id(), Some("Alpha"));
        assert!(!dropdown.is_open());
    }

    #[test]
    fn typing_search_filters_rows_before_commit() {
        let mut dropdown = single_dropdown();

        dropdown.open();
        dropdown.on_key(char_key('g'), AREA);
        dropdown.on_key(char_key('a'), AREA);
        dropdown.on_key(Key::Enter, AREA);

        assert_eq!(dropdown.selected_id(), Some("Gamma"));
    }

    #[test]
    fn enter_commit_clears_search_query() {
        let mut dropdown = single_dropdown();

        dropdown.open();
        dropdown.on_key(char_key('g'), AREA);
        dropdown.on_key(Key::Enter, AREA);

        assert_eq!(dropdown.selected_id(), Some("Gamma"));
        assert_eq!(dropdown.search_query(), "");
    }

    #[test]
    fn escape_cancel_clears_search_query_and_filter() {
        let mut dropdown = single_dropdown();

        dropdown.open();
        dropdown.on_key(char_key('g'), AREA);
        dropdown.on_key(Key::Esc, AREA);

        assert_eq!(dropdown.search_query(), "");
        assert_eq!(dropdown.filtered, ROWS.to_vec());
    }

    #[test]
    fn dropdown_navigation_preserves_search_query() {
        let mut dropdown = single_dropdown();

        dropdown.open();
        dropdown.on_key(char_key('a'), AREA);
        dropdown.on_key(ctrl('j'), AREA);

        assert_eq!(dropdown.search_query(), "a");
    }

    #[test]
    fn contains_search_requires_contiguous_match() {
        let mut dropdown = single_dropdown().search_mode(DropdownSearchMode::Contains);

        dropdown.open();
        dropdown.on_key(char_key('m'), AREA);
        dropdown.on_key(char_key('m'), AREA);
        dropdown.on_key(Key::Enter, AREA);

        assert_eq!(dropdown.selected_id(), Some("Gamma"));
    }

    #[test]
    fn disabled_search_ignores_typing() {
        let mut dropdown = single_dropdown().search_mode(DropdownSearchMode::None);

        dropdown.open();
        dropdown.on_key(char_key('g'), AREA);
        dropdown.on_key(Key::Enter, AREA);

        assert_eq!(dropdown.search_query(), "");
        assert_eq!(dropdown.selected_id(), Some("Alpha"));
    }

    #[test]
    fn immediate_commit_updates_selection_while_open() {
        let mut dropdown = single_dropdown()
            .commit_mode(DropdownCommitMode::Immediate)
            .selected_one("Alpha");

        dropdown.open();
        let outcome = dropdown.on_key(ctrl('j'), AREA);

        assert!(outcome.committed);
        assert!(dropdown.is_open());
        assert_eq!(dropdown.selected_id(), Some("Beta"));
    }

    #[test]
    fn explicit_single_keeps_trigger_value_until_commit() {
        let mut dropdown = single_dropdown().selected_one("Alpha");

        dropdown.open();
        dropdown.on_key(ctrl('j'), AREA);

        assert_eq!(dropdown.selected_summary(), "Alpha");
        assert_eq!(dropdown.selected_id(), Some("Alpha"));
    }

    #[test]
    fn open_highlights_committed_selection() {
        let mut dropdown = single_dropdown()
            .search_mode(DropdownSearchMode::None)
            .selected_one("Beta");

        dropdown.open();

        assert_eq!(dropdown.data_view.highlighted_id(), Some("Beta"));
    }

    #[test]
    fn searchable_dropdown_keeps_list_highlight_visible_while_search_focused() {
        let mut dropdown = single_dropdown();

        dropdown.open();

        assert_eq!(dropdown.focus_region, Some(DropdownFocusRegion::Search));
        assert!(dropdown.data_view.focused_for_test());
    }

    #[test]
    fn multi_toggle_then_escape_rolls_back() {
        let mut dropdown = multi_dropdown().selected(["Alpha"]);

        dropdown.open();
        dropdown.on_key(ctrl('j'), AREA);
        dropdown.on_key(Key::Char(' '), AREA);
        dropdown.on_key(Key::Esc, AREA);

        assert_eq!(dropdown.selected_ids(), vec!["Alpha"]);
    }

    #[test]
    fn ctrl_space_toggles_highlighted_multi_row() {
        let mut dropdown = multi_dropdown();

        dropdown.open();
        dropdown.on_key(ctrl('j'), AREA);
        dropdown.on_key(ctrl(' '), AREA);

        assert_eq!(dropdown.draft, vec!["Beta"]);
    }

    #[test]
    fn ctrl_space_commits_highlighted_single_row() {
        let mut dropdown = single_dropdown();

        dropdown.open();
        dropdown.on_key(ctrl('j'), AREA);
        let outcome = dropdown.on_key(ctrl(' '), AREA);

        assert!(outcome.committed);
        assert_eq!(dropdown.selected_id(), Some("Beta"));
        assert!(!dropdown.is_open());
    }

    #[test]
    fn closed_layout_registers_field_focus() {
        let mut dropdown = single_dropdown();
        let mut ctx = LayoutCtx::new();

        <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut ctx);

        let targets = ctx.focus_targets();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].id.as_str(), FIELD_FOCUS);
        assert!(targets[0].path.is_empty());
    }

    #[test]
    fn filled_variant_registers_compact_field_focus() {
        let mut dropdown = single_dropdown().variant(DropdownVariant::Filled);
        let mut ctx = LayoutCtx::new();

        <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut ctx);

        let targets = ctx.focus_targets();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].id.as_str(), FIELD_FOCUS);
        assert_eq!(targets[0].area.height, 1);
    }

    #[test]
    fn bordered_dropdown_measure_reports_field_height() {
        let dropdown = single_dropdown();

        let hint = <Dropdown<_, _> as TuiNode<()>>::measure(&dropdown, LayoutProposal::unbounded());

        assert_eq!(hint.preferred.height, 3);
        assert!(!hint.expand.height);
    }

    #[test]
    fn filled_dropdown_measure_reports_compact_field_height() {
        let dropdown = single_dropdown().variant(DropdownVariant::Filled);

        let hint = <Dropdown<_, _> as TuiNode<()>>::measure(&dropdown, LayoutProposal::unbounded());

        assert_eq!(hint.preferred.height, 1);
        assert!(!hint.expand.height);
    }

    #[test]
    fn flex_fit_content_uses_dropdown_variant_height() {
        let mut bordered: Flex<()> =
            Flex::column().child("dropdown", single_dropdown(), FlexItem::fit_content());
        let mut filled: Flex<()> = Flex::column().child(
            "dropdown",
            single_dropdown().variant(DropdownVariant::Filled),
            FlexItem::fit_content(),
        );
        let mut ctx = LayoutCtx::new();

        bordered.layout(Rect::new(0, 0, 24, 10), &mut ctx);
        filled.layout(Rect::new(0, 0, 24, 10), &mut ctx);

        assert_eq!(
            bordered
                .child_rect(&ChildKey::from("dropdown"))
                .unwrap()
                .height,
            3
        );
        assert_eq!(
            filled
                .child_rect(&ChildKey::from("dropdown"))
                .unwrap()
                .height,
            1
        );
    }

    #[test]
    fn flex_horizontal_fit_content_allocates_width_based_on_text() {
        let mut flex: Flex<()> = Flex::row().child(
            "dropdown",
            single_dropdown().selected_one("Beta"),
            FlexItem::fit_content(),
        );
        let mut ctx = LayoutCtx::new();

        flex.layout(Rect::new(0, 0, 40, 3), &mut ctx);

        // "Beta" is 4 chars, plus 2 chrome = 6 width.
        assert_eq!(
            flex.child_rect(&ChildKey::from("dropdown")).unwrap().width,
            6
        );
    }

    #[test]
    fn open_layout_returns_trigger_field_area_only() {
        let mut dropdown = single_dropdown();
        dropdown.open();
        let mut ctx = LayoutCtx::new();

        let result = dropdown.layout_overlay::<()>(Rect::new(0, 0, 24, 3), AREA, &mut ctx);

        assert_eq!(result.area, Rect::new(0, 0, 24, 3));
    }

    #[test]
    fn filled_variant_renders_filled_trigger_with_nerd_font_chevron() {
        let dropdown = single_dropdown()
            .variant(DropdownVariant::Filled)
            .selected_one("Beta");
        let mut terminal = Terminal::new(TestBackend::new(12, 3)).expect("terminal should build");

        terminal
            .draw(|frame| dropdown.render(frame, frame.area()))
            .expect("dropdown should render");

        let buffer = terminal.backend().buffer();
        assert_eq!(buffer.cell((0, 0)).unwrap().bg, theme().highlight_bg());
        assert_eq!(buffer.cell((1, 0)).unwrap().symbol(), "B");
        assert_eq!(buffer.cell((11, 0)).unwrap().symbol(), "");
    }

    #[test]
    fn focused_bordered_popup_uses_accent_border() {
        let mut dropdown = single_dropdown();
        dropdown.open();
        dropdown.layout_overlay::<()>(
            Rect::new(0, 0, 12, 3),
            Rect::new(0, 0, 12, 8),
            &mut LayoutCtx::new(),
        );
        let mut terminal = Terminal::new(TestBackend::new(12, 8)).expect("terminal should build");

        terminal
            .draw(|frame| {
                dropdown.render(frame, Rect::new(0, 0, 12, 3));
                dropdown.render_popup_overlay(frame, frame.area());
            })
            .expect("dropdown should render");

        let buffer = terminal.backend().buffer();
        assert_eq!(buffer.cell((0, 2)).unwrap().fg, theme().accent_fg());
    }

    #[test]
    fn open_render_draws_trigger_without_inline_popup() {
        let mut dropdown = single_dropdown();
        dropdown.open();
        let mut terminal = Terminal::new(TestBackend::new(12, 8)).expect("terminal should build");

        terminal
            .draw(|frame| dropdown.render(frame, frame.area()))
            .expect("dropdown should render");

        let buffer = terminal.backend().buffer();
        assert_ne!(buffer.cell((0, 2)).unwrap().symbol(), " ");
        assert_eq!(buffer.cell((0, 3)).unwrap().symbol(), " ");
    }

    #[test]
    fn open_layout_registers_search_and_list_focus() {
        let mut dropdown = single_dropdown();
        dropdown.open();
        let mut ctx = LayoutCtx::new();

        <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut ctx);

        let targets = ctx.focus_targets();
        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].id.as_str(), "input");
        assert_eq!(targets[0].path.keys()[0].as_str(), SEARCH_SLOT);
        assert_eq!(targets[1].id.as_str(), "data-view");
        assert_eq!(targets[1].path.keys()[0].as_str(), LIST_SLOT);
    }

    #[test]
    fn open_layout_focus_targets_use_overlay_popup_areas() {
        let mut dropdown = single_dropdown();
        dropdown.open();
        let mut ctx = LayoutCtx::new();

        dropdown.layout_overlay::<()>(Rect::new(0, 0, 24, 3), Rect::new(0, 0, 24, 20), &mut ctx);

        let targets = ctx.focus_targets();
        assert_eq!(targets[0].area, Rect::new(1, 3, 22, 1));
        assert_eq!(targets[1].area, Rect::new(1, 4, 22, 3));
    }

    #[test]
    fn open_layout_sizes_popup_to_visible_items() {
        let mut dropdown = single_dropdown();
        dropdown.open();

        let area = open_list_area(&mut dropdown, Rect::new(0, 0, 24, 20));

        assert_eq!(area.height, 3);
    }

    #[test]
    fn bordered_and_filled_popups_size_to_same_content_with_variant_chrome() {
        let mut bordered = single_dropdown();
        bordered.open();
        let mut filled = single_dropdown().variant(DropdownVariant::Filled);
        filled.open();

        let [_, bordered_popup] = bordered.areas(Rect::new(0, 0, 24, 20));
        let [_, filled_popup] = filled.areas(Rect::new(0, 0, 24, 20));
        let [_, bordered_list] = bordered.popup_inner_areas(bordered_popup);
        let [_, filled_list] = filled.popup_inner_areas(filled_popup);

        assert_eq!(bordered_list.height, 3);
        assert_eq!(filled_list.height, 3);
        assert_eq!(bordered_popup.height, 6);
        assert_eq!(filled_popup.height, 4);
    }

    #[test]
    fn bordered_popup_area_overlaps_field_bottom_row() {
        let mut dropdown = single_dropdown();
        dropdown.open();

        let [field_area, popup_area] = dropdown.areas(AREA);

        assert_eq!(popup_area.y, field_area.y + field_area.height - 1);
    }

    #[test]
    fn overlay_popup_extends_beyond_trigger_field_when_bounds_allow() {
        let mut dropdown = single_dropdown();
        dropdown.open();
        dropdown.layout_overlay::<()>(
            Rect::new(0, 0, 24, 3),
            Rect::new(0, 0, 24, 20),
            &mut LayoutCtx::new(),
        );

        let popup_area = dropdown.popup_overlay_area(Rect::new(0, 0, 24, 20));

        assert_eq!(popup_area, Rect::new(0, 2, 24, 6));
        assert!(popup_area.y + popup_area.height > 3);
    }

    #[test]
    fn filled_popup_layout_has_no_border_offset() {
        let mut dropdown = single_dropdown().variant(DropdownVariant::Filled);
        dropdown.open();

        let [_, popup_area] = dropdown.areas(AREA);
        let [search_area, list_area] = dropdown.popup_inner_areas(popup_area);

        assert_eq!(search_area.y, popup_area.y);
        assert_eq!(search_area.x, popup_area.x);
        assert_eq!(list_area.y, popup_area.y + 1);
        assert_eq!(list_area.x, popup_area.x);
    }

    #[test]
    fn open_layout_sizes_popup_to_no_results_row() {
        let mut dropdown = single_dropdown();
        dropdown.open();
        dropdown.on_key(char_key('z'), Rect::new(0, 0, 24, 20));

        let area = open_list_area(&mut dropdown, Rect::new(0, 0, 24, 20));

        assert_eq!(area.height, 1);
    }

    #[test]
    fn open_layout_caps_popup_at_default_max() {
        let mut dropdown = numeric_dropdown(40);
        dropdown.open();

        let area = open_list_area(&mut dropdown, Rect::new(0, 0, 24, 60));

        assert_eq!(area.height, 27);
    }

    #[test]
    fn max_popup_height_overrides_preset_max() {
        let mut dropdown = numeric_dropdown(40).max_popup_height(5);
        dropdown.open();

        let area = open_list_area(&mut dropdown, Rect::new(0, 0, 24, 60));

        assert_eq!(area.height, 2);
    }

    #[test]
    fn filled_popup_caps_height_without_border_chrome() {
        let mut dropdown = numeric_dropdown(40).variant(DropdownVariant::Filled);
        dropdown.open();

        let [_, popup_area] = dropdown.areas(Rect::new(0, 0, 24, 60));
        let [_, list_area] = dropdown.popup_inner_areas(popup_area);

        assert_eq!(popup_area.height, 30);
        assert_eq!(list_area.height, 29);
    }

    #[test]
    fn filled_popup_applies_background_to_content_rows() {
        let mut dropdown = single_dropdown().variant(DropdownVariant::Filled);
        dropdown.open();
        dropdown.layout_overlay::<()>(
            Rect::new(0, 0, 12, 1),
            Rect::new(0, 0, 12, 6),
            &mut LayoutCtx::new(),
        );
        let mut terminal = Terminal::new(TestBackend::new(12, 6)).expect("terminal should build");

        terminal
            .draw(|frame| {
                dropdown.render(frame, Rect::new(0, 0, 12, 1));
                dropdown.render_popup_overlay(frame, frame.area());
            })
            .expect("dropdown should render");

        let buffer = terminal.backend().buffer();
        assert_eq!(
            dropdown.popup_content_style().unwrap().bg,
            Some(theme().border_fg())
        );
        assert_eq!(buffer.cell((0, 3)).unwrap().symbol(), "B");
        assert_eq!(buffer.cell((0, 3)).unwrap().bg, theme().border_fg());
    }

    #[test]
    fn node_event_opens_and_requests_layout() {
        let mut dropdown = single_dropdown();
        let mut layout = LayoutCtx::new();
        <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut layout);
        let target = layout.focus_targets()[0].clone();
        let mut focus = FocusCtx::<()>::default();
        dropdown.dispatch_focus(&target, true, &mut focus);
        let mut event = EventCtx::<()>::default();

        let outcome = dropdown.event(&TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut event);

        assert_eq!(outcome, EventOutcome::Handled);
        assert!(dropdown.is_open());
        assert!(event.layout_requested());
        assert_eq!(event.propagation(), Propagation::Stopped);
    }

    fn char_key(value: char) -> KeyEvent {
        KeyEvent {
            code: Key::Char(value),
            modifiers: KeyModifiers::NONE,
        }
    }

    fn ctrl(value: char) -> KeyEvent {
        KeyEvent {
            code: Key::Char(value),
            modifiers: KeyModifiers::CONTROL,
        }
    }

    fn open_list_area<T, Id>(dropdown: &mut Dropdown<T, Id>, area: Rect) -> Rect
    where
        T: 'static,
        Id: Clone + Eq + Hash + 'static,
    {
        let mut ctx = LayoutCtx::new();
        <Dropdown<T, Id> as TuiNode<()>>::layout(dropdown, area, &mut ctx);
        ctx.focus_targets()
            .iter()
            .find(|target| target.path.keys()[0].as_str() == LIST_SLOT)
            .expect("list focus target should exist")
            .area
    }
}
