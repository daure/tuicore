use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::{CalendarEntryRole, CalendarKeyBindings, calendar_entry_style};
use crate::components::{
    Column, DataViewTypedEvent, ListControl, ListControlDisplayKeyBindings, ListControlEvent,
    SelectionMode,
};
use crate::{
    AnimationSettings, EventCtx, EventOutcome, EventRoute, FocusCtx, FocusTarget, LayoutCtx,
    LayoutProposal, LayoutResult, LayoutSizeHint, LifecycleCtx, TickResult, TuiEvent, TuiNode,
    theme,
};

pub(super) const DAY_ENTRIES_SLOT: &str = "day-entries";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct CalendarDayRowKey(u64);

#[derive(Clone)]
pub(super) struct CalendarDayRow {
    pub(super) key: CalendarDayRowKey,
    pub(super) entry_index: usize,
    rank: usize,
    pub(super) time_prefix: Option<String>,
    pub(super) marker_prefix: String,
    pub(super) entry: Line<'static>,
    continuation_indent: usize,
    pub(super) role: Option<CalendarEntryRole>,
    scope: Vec<CalendarDayRowKey>,
}

impl CalendarDayRow {
    pub(super) fn new(
        key: CalendarDayRowKey,
        entry_index: usize,
        rank: usize,
        time_prefix: Option<String>,
        marker_prefix: String,
        entry: Line<'static>,
        continuation_indent: usize,
        role: Option<CalendarEntryRole>,
        scope: Vec<CalendarDayRowKey>,
    ) -> Self {
        Self {
            key,
            entry_index,
            rank,
            time_prefix,
            marker_prefix,
            entry,
            continuation_indent,
            role,
            scope,
        }
    }
}

pub(super) enum CalendarDayEvent<Id> {
    Highlighted(Option<Id>),
    Activated(Id),
    Reordered(Vec<Id>),
}

pub(super) struct CalendarDayList<Id, M> {
    control: ListControl<CalendarDayRow, CalendarDayRowKey, M>,
    registry: Vec<(Id, CalendarDayRowKey)>,
    next_key: u64,
    rows: Vec<CalendarDayRow>,
    keys: CalendarKeyBindings,
    wrap_cells: bool,
    reorder_enabled: bool,
}

impl<Id, M: 'static> CalendarDayList<Id, M>
where
    Id: Clone + Eq,
{
    pub(super) fn new(keys: CalendarKeyBindings) -> Self {
        Self {
            control: Self::control(Vec::new(), &keys, false, false),
            registry: Vec::new(),
            next_key: 0,
            rows: Vec::new(),
            keys,
            wrap_cells: false,
            reorder_enabled: false,
        }
    }

    fn control(
        rows: Vec<CalendarDayRow>,
        keys: &CalendarKeyBindings,
        wrap_cells: bool,
        reorder_enabled: bool,
    ) -> ListControl<CalendarDayRow, CalendarDayRowKey, M> {
        let control = ListControl::display(rows, |row: &CalendarDayRow| row.key)
            .selection_mode(SelectionMode::Single)
            .column(
                Column::rich(
                    "entry",
                    "",
                    Constraint::Percentage(100),
                    |row: &CalendarDayRow, _| {
                        let mut spans = Vec::new();
                        if let Some(time_prefix) = &row.time_prefix {
                            spans.push(Span::styled(
                                time_prefix.clone(),
                                Style::default().fg(theme().accent_fg()),
                            ));
                        }
                        spans.push(Span::styled(
                            row.marker_prefix.clone(),
                            Style::default().fg(theme().text_fg()),
                        ));
                        spans.extend(row.entry.spans.clone());
                        Line {
                            spans,
                            style: row.role.map_or(row.entry.style, |role| {
                                row.entry
                                    .style
                                    .patch(calendar_entry_style(Some(role), false))
                            }),
                            alignment: row.entry.alignment,
                        }
                    },
                )
                .wrap_continuation_indent_by(|row| row.continuation_indent)
                .reorderable(|row| row.rank, |row, rank| row.rank = rank),
            )
            .headers(false)
            .filter_controls(false)
            .empty_message("No entries")
            .display_keybindings(Self::display_keybindings(keys));
        let control = if reorder_enabled {
            control.reorderable_by_scoped("entry", |left, right| left.scope.contains(&right.key))
        } else {
            control
        };
        if wrap_cells {
            control.wrap_cells()
        } else {
            control
        }
    }

    fn display_keybindings(keys: &CalendarKeyBindings) -> ListControlDisplayKeyBindings {
        ListControlDisplayKeyBindings::default()
            .line_up(keys.up.clone())
            .line_down(keys.down.clone())
            .page_up(keys.page_up.clone())
            .page_down(keys.page_down.clone())
            .top(keys.home.clone())
            .top_prefix(keys.top_prefix.clone())
            .bottom(keys.end.iter().chain(&keys.bottom).cloned())
            .activate(keys.activate.clone())
            .reorder(keys.reorder.clone())
    }

    pub(super) fn set_keybindings(&mut self, keys: CalendarKeyBindings) {
        self.control
            .set_display_keybindings(Self::display_keybindings(&keys));
        self.keys = keys;
    }

    pub(super) fn set_wrap_cells(&mut self, wrap_cells: bool) {
        if self.wrap_cells != wrap_cells {
            self.wrap_cells = wrap_cells;
            self.control.set_wrap_cells(wrap_cells);
        }
    }

    pub(super) fn set_reorderable(&mut self, enabled: bool) {
        if self.reorder_enabled != enabled {
            self.reorder_enabled = enabled;
            self.rebuild();
        }
    }

    fn rebuild(&mut self) {
        let highlighted = self.control.data_view().highlighted_id();
        self.control = Self::control(
            self.rows.clone(),
            &self.keys,
            self.wrap_cells,
            self.reorder_enabled,
        );
        if let Some(key) = highlighted {
            self.control.data_view_mut().highlight_id(&key);
            self.control.take_data_view_events();
        }
    }

    pub(super) fn reconcile_ids(&mut self, ids: impl IntoIterator<Item = Id>) {
        let ids = ids.into_iter().collect::<Vec<_>>();
        assert!(
            ids.iter()
                .enumerate()
                .all(|(index, id)| !ids[..index].contains(id)),
            "Calendar entry IDs must be unique"
        );
        self.registry.retain(|(id, _)| ids.contains(id));
        for id in ids {
            if !self.registry.iter().any(|(known, _)| known == &id) {
                let key = CalendarDayRowKey(self.next_key);
                self.next_key = self
                    .next_key
                    .checked_add(1)
                    .expect("calendar row key overflow");
                self.registry.push((id, key));
            }
        }
    }

    pub(super) fn key_for(&self, id: &Id) -> CalendarDayRowKey {
        self.registry
            .iter()
            .find(|(known, _)| known == id)
            .map(|(_, key)| *key)
            .expect("calendar row key exists after entry reconciliation")
    }

    pub(super) fn replace_rows(&mut self, rows: Vec<CalendarDayRow>) {
        self.rows = rows.clone();
        self.control.set_rows(rows);
    }

    pub(super) fn set_highlighted(&mut self, id: Option<&Id>) {
        if let Some(id) = id {
            let key = self.key_for(id);
            self.control.set_highlighted_id(&key);
        }
        self.control.take_data_view_events();
    }

    pub(super) fn highlighted_entry_index(&self) -> Option<usize> {
        let key = self.control.data_view().highlighted_id()?;
        self.rows
            .iter()
            .find(|row| row.key == key)
            .map(|row| row.entry_index)
    }

    pub(super) fn transient_selected_ids(&self) -> Vec<Id> {
        self.control
            .transient_selected_ids()
            .into_iter()
            .filter_map(|key| self.id_for_key(key))
            .collect()
    }

    pub(super) fn is_transient_selected(&self, id: &Id) -> bool {
        let key = self.key_for(id);
        self.control.transient_selected_ids().contains(&key)
    }

    pub(super) fn set_display_focused(&mut self, focused: bool) {
        self.control.set_display_focused(focused);
    }

    pub(super) fn clear_display_pending_top_prefix(&mut self) {
        self.control.clear_display_pending_top_prefix();
    }

    pub(super) fn clear_transient_selection(&mut self) {
        self.control.clear_transient_selection();
    }

    pub(super) fn is_reordering(&self) -> bool {
        self.control.is_reordering()
    }

    pub(super) fn can_start_reorder(&self) -> bool {
        self.control
            .data_view()
            .highlighted_id()
            .and_then(|key| self.rows.iter().find(|row| row.key == key))
            .is_some_and(|row| row.scope.len() > 1)
    }

    pub(super) fn event(
        &mut self,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> (EventOutcome, Vec<CalendarDayEvent<Id>>) {
        let outcome = self.control.event(event, ctx);
        (outcome, self.take_events())
    }

    pub(super) fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> (EventOutcome, Vec<CalendarDayEvent<Id>>) {
        let outcome = self.control.dispatch_event(route, event, ctx);
        (outcome, self.take_events())
    }

    fn take_events(&mut self) -> Vec<CalendarDayEvent<Id>> {
        let mut events = self
            .control
            .take_data_view_events()
            .into_iter()
            .filter_map(|event| match event {
                DataViewTypedEvent::HighlightChanged { row_id } => Some(
                    CalendarDayEvent::Highlighted(row_id.and_then(|key| self.id_for_key(key))),
                ),
                DataViewTypedEvent::Activated { row_id } => {
                    self.id_for_key(row_id).map(CalendarDayEvent::Activated)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        events.extend(self.control.take_events().into_iter().filter_map(|event| {
            match event {
                ListControlEvent::Reordered { row_ids } => Some(CalendarDayEvent::Reordered(
                    row_ids
                        .into_iter()
                        .filter_map(|key| self.id_for_key(key))
                        .collect(),
                )),
                _ => None,
            }
        }));
        events
    }

    fn id_for_key(&self, key: CalendarDayRowKey) -> Option<Id> {
        self.registry
            .iter()
            .find(|(_, known)| *known == key)
            .map(|(id, _)| id.clone())
    }

    pub(super) fn render(&self, frame: &mut Frame, area: Rect) {
        self.control.data_view().render(frame, area);
    }

    #[cfg(test)]
    pub(super) fn selection_overlay_active_for_test(&self) -> bool {
        self.control.data_view().selection_overlay_active_for_test()
    }

    #[cfg(test)]
    pub(super) fn rows(&self) -> &[CalendarDayRow] {
        self.control.items()
    }

    #[cfg(test)]
    pub(super) fn row_has_reorder_highlight(&self, entry_index: &usize) -> bool {
        self.rows()
            .iter()
            .find(|row| &row.entry_index == entry_index)
            .is_some_and(|row| self.control.data_view().row_has_reorder_highlight(&row.key))
    }
}

impl<Id, M: 'static> TuiNode<M> for CalendarDayList<Id, M>
where
    Id: Clone + Eq,
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        self.control.measure(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.control.layout(area, ctx)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut crate::RenderCtx<'a>) {
        self.control.render(frame, area, ctx);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        self.control.event(event, ctx)
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        self.control.dispatch_event(route, event, ctx)
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        self.control.dispatch_focus(target, focused, ctx);
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.control.tick(dt, settings)
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.control.init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.control.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.control.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.control.destroy(ctx);
    }
}
