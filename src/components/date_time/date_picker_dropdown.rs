use std::time::Duration as StdDuration;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use time::Date;

use crate::{
    EventCtx, EventOutcome, FocusCtx, FocusId, HotkeyEvent, HotkeyMatch, HotkeySequenceMatcher,
    LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint, OverlayId, OverlayLayer, OverlaySpec,
    TickResult, TuiEvent, TuiNode, hotkey_label_spans, hotkey_underline_style, keybindings,
    line_width, theme,
};

use crate::components::{InputChrome, Panel};

use super::{DATE_PICKER_DROPDOWN_FOCUS, DatePicker, finish_event, picker_size_hint};

const DATE_PICKER_DROPDOWN_OVERLAY_NAMESPACE: u64 = 0x4441_5445_5049_434b;

pub struct DatePickerDropdown<M = ()> {
    picker: DatePicker<M>,
    open: bool,
    focused: bool,
    field_area: Rect,
    overlay_bounds: Rect,
    placeholder: String,
    hotkey: Option<String>,
    hotkey_matcher: HotkeySequenceMatcher,
    pending_hotkey_prefix: Option<String>,
    on_select: Option<Box<dyn Fn(Date) -> M>>,
    chrome: InputChrome,
    panel: Panel,
}

impl<M> DatePickerDropdown<M> {
    pub fn new() -> Self {
        Self {
            picker: DatePicker::new(),
            open: false,
            focused: false,
            field_area: Rect::default(),
            overlay_bounds: Rect::default(),
            placeholder: String::from("Select date"),
            hotkey: None,
            hotkey_matcher: HotkeySequenceMatcher::default(),
            pending_hotkey_prefix: None,
            on_select: None,
            chrome: InputChrome::Plain,
            panel: Panel::new(),
        }
    }

    pub fn value(mut self, value: Option<Date>) -> Self {
        self.picker.set_value(value);
        self
    }

    pub fn today(mut self, today: Date) -> Self {
        self.picker = self.picker.today(today);
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.set_hotkey(hotkey);
        self
    }

    pub fn set_hotkey(&mut self, hotkey: impl Into<String>) {
        let hotkey = hotkey.into();
        self.hotkey = Some(hotkey.clone());
        self.hotkey_matcher = HotkeySequenceMatcher::new([hotkey]);
        self.sync_panel();
    }

    pub fn style(mut self, chrome: InputChrome) -> Self {
        self.set_style(chrome);
        self
    }

    pub fn panel(mut self, title: impl Into<String>) -> Self {
        self.set_style(InputChrome::panel(title));
        self
    }

    pub fn set_style(&mut self, chrome: InputChrome) {
        self.chrome = chrome;
        self.sync_panel();
    }

    pub fn on_select(mut self, handler: impl Fn(Date) -> M + 'static) -> Self {
        self.on_select = Some(Box::new(handler));
        self
    }

    pub fn current_value(&self) -> Option<Date> {
        self.picker.current_value()
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn set_open(&mut self, open: bool) {
        self.open = open;
        self.picker.set_focused(open && self.focused);
    }

    pub fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut crate::RenderCtx<'a>) {
        self.render_field(frame, area);
        if self.open {
            let bounds = self.overlay_bounds;
            ctx.push_portal(OverlayLayer::Popover, 0, bounds, |frame, bounds| {
                self.render_portal_popup(frame, bounds);
            });
        }
    }

    pub fn render_field(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }
        let field = self.render_chrome(frame, area);
        frame.render_widget(Paragraph::new(self.field_line(field.width)), field);
    }

    fn sync_panel(&mut self) {
        let mut panel = match &self.chrome {
            InputChrome::Plain => Panel::new(),
            InputChrome::Panel(panel) => panel.panel(self.focused, self.hotkey.as_deref()),
        };
        panel.set_pending_hotkey_prefix(self.pending_hotkey_prefix.clone());
        self.panel = panel;
    }

    fn render_chrome(&self, frame: &mut Frame, area: Rect) -> Rect {
        match self.chrome {
            InputChrome::Plain => Rect::new(area.x, area.y, area.width, 1.min(area.height)),
            InputChrome::Panel(_) => {
                self.panel.render(frame, area);
                Panel::inner_area(area)
            }
        }
    }

    fn content_area(&self, area: Rect) -> Rect {
        match self.chrome {
            InputChrome::Plain => Rect::new(area.x, area.y, area.width, 1.min(area.height)),
            InputChrome::Panel(_) => Panel::inner_area(area),
        }
    }

    fn measure_size(&self) -> (u16, u16) {
        match self.chrome {
            InputChrome::Plain => (24, 1),
            InputChrome::Panel(_) => (26, 3),
        }
    }

    fn render_portal_popup(&self, frame: &mut Frame, bounds: Rect) {
        if !self.open || bounds.is_empty() {
            return;
        }
        let popup = self.popup_area(bounds);
        if !popup.is_empty() {
            frame.render_widget(Clear, popup);
            self.picker.render(frame, popup);
        }
    }

    pub fn popup_area(&self, bounds: Rect) -> Rect {
        let field = self.effective_field_area(bounds);
        let width = field.width.min(24).min(bounds.width);
        let below_space = bounds
            .y
            .saturating_add(bounds.height)
            .saturating_sub(field.y.saturating_add(field.height));
        let above_space = field.y.saturating_sub(bounds.y);
        let place_below = below_space >= 10 || below_space >= above_space;
        let available_height = if place_below {
            below_space
        } else {
            above_space
        };
        let height = 10.min(available_height);
        if width == 0 || height == 0 {
            return Rect::default();
        }
        let y = if place_below {
            field.y.saturating_add(field.height)
        } else {
            field.y.saturating_sub(height)
        };
        let max_x = bounds.x.saturating_add(bounds.width.saturating_sub(width));
        let x = field.x.min(max_x).max(bounds.x);
        Rect::new(x, y, width, height)
    }

    fn effective_field_area(&self, bounds: Rect) -> Rect {
        if self.field_area.is_empty() {
            Rect::new(bounds.x, bounds.y, bounds.width, 1.min(bounds.height))
        } else {
            self.field_area
        }
    }

    fn field_line(&self, width: u16) -> Line<'static> {
        let t = theme();
        let style = if self.focused {
            Style::default().fg(t.highlight_fg()).bg(t.highlight_bg())
        } else {
            Style::default().fg(t.text_fg())
        };
        let value = self
            .current_value()
            .map(|date| date.to_string())
            .unwrap_or_else(|| self.placeholder.clone());
        let icon = " ";
        let mut spans = vec![Span::styled(icon.to_string(), style)];
        spans.extend(hotkey_label_spans(
            &value,
            self.inline_hotkey(),
            crate::HotkeyLabelMode::PreferMnemonic,
            self.pending_hotkey_prefix.as_deref(),
            style,
            hotkey_underline_style(style),
        ));
        let used = line_width(&Line::from(spans.clone()));
        if width as usize > used {
            spans.push(Span::styled(" ".repeat(width as usize - used), style));
        }
        Line::from(spans)
    }

    fn inline_hotkey(&self) -> Option<&str> {
        match self.chrome {
            InputChrome::Plain => self.hotkey.as_deref(),
            InputChrome::Panel(_) => None,
        }
    }

    fn sync_pending_hotkey_prefix_from_matcher(&mut self) {
        self.pending_hotkey_prefix = self
            .hotkey_matcher
            .is_pending()
            .then(|| self.hotkey_matcher.prefix().to_owned());
        self.sync_panel();
    }

    fn handle_hotkey(&mut self, hotkey: &HotkeyEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        match hotkey {
            HotkeyEvent::Pending(prefix) => {
                self.pending_hotkey_prefix = Some(prefix.clone());
                self.sync_panel();
                ctx.request_redraw();
                EventOutcome::Ignored
            }
            HotkeyEvent::Canceled => {
                self.pending_hotkey_prefix = None;
                self.sync_panel();
                ctx.request_redraw();
                EventOutcome::Ignored
            }
            HotkeyEvent::Commit(sequence) => {
                self.pending_hotkey_prefix = None;
                self.sync_panel();
                if self.hotkey.as_deref().is_some_and(|hotkey| {
                    crate::hotkey::normalize_hotkey(hotkey)
                        == crate::hotkey::normalize_hotkey(sequence)
                }) {
                    self.set_open(true);
                    ctx.request_layout();
                    ctx.request_redraw();
                    ctx.stop_propagation();
                    EventOutcome::Handled
                } else {
                    EventOutcome::Ignored
                }
            }
        }
    }
}

impl<M> Default for DatePickerDropdown<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: 'static> TuiNode<M> for DatePickerDropdown<M> {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let (width, height) = self.measure_size();
        picker_size_hint(width, height).normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.field_area = self.content_area(area);
        self.overlay_bounds = ctx.overlay_bounds();
        if let Some(hotkey) = self.hotkey.clone() {
            ctx.register_focusable_with_hotkey_sequences(
                FocusId::new(DATE_PICKER_DROPDOWN_FOCUS),
                self.field_area,
                true,
                vec![hotkey],
            );
        } else {
            ctx.register_focusable(
                FocusId::new(DATE_PICKER_DROPDOWN_FOCUS),
                self.field_area,
                true,
            );
        }
        ctx.set_focus_receives_events_before_global_hotkeys(
            FocusId::new(DATE_PICKER_DROPDOWN_FOCUS),
            true,
        );
        ctx.set_focus_suppresses_global_hotkeys(
            FocusId::new(DATE_PICKER_DROPDOWN_FOCUS),
            self.open,
        );
        if self.open {
            let popup = self.popup_area(self.overlay_bounds);
            let mut spec = OverlaySpec::new(
                OverlayId::for_path(DATE_PICKER_DROPDOWN_OVERLAY_NAMESPACE, &ctx.current_path()),
                self.field_area,
                popup,
            );
            let path = ctx.current_path();
            spec.owner_path = Some(path.clone());
            spec.route_path = Some(path);
            spec.bounds = Some(self.overlay_bounds);
            spec.layer = OverlayLayer::Popover;
            ctx.register_overlay(spec);
        }
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut crate::RenderCtx<'a>) {
        self.render(frame, area, ctx);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        if let TuiEvent::Hotkey(hotkey) = event {
            return self.handle_hotkey(hotkey, ctx);
        }
        if let TuiEvent::ExternalEditor(response) = event {
            let was_open = self.open;
            let outcome = self.picker.apply_external_editor_response(response);
            self.set_open(false);
            if outcome.selected
                && let Some(value) = self.picker.current_value()
                && let Some(on_select) = &self.on_select
            {
                ctx.emit(on_select(value));
            }
            ctx.request_clear();
            if was_open {
                ctx.request_layout();
            }
            ctx.request_redraw();
            return finish_event(ctx, outcome);
        }
        if matches!(event, TuiEvent::Yank) {
            ctx.copy_to_clipboard(self.picker.cursor().to_string());
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };
        let bindings = keybindings();
        let focus_keys = bindings.focus();
        if bindings.date_time_picker().external_editor_matches(*key) {
            let value = self.picker.cursor().to_string();
            ctx.request_external_editor(value.clone(), 1, value.len() + 1);
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        if !self.open {
            match self.hotkey_matcher.on_key(*key) {
                HotkeyMatch::Matched(_) => self.sync_pending_hotkey_prefix_from_matcher(),
                HotkeyMatch::Pending | HotkeyMatch::Canceled => {
                    self.sync_pending_hotkey_prefix_from_matcher();
                    ctx.request_redraw();
                }
                HotkeyMatch::Ignored => {}
            }
            if keybindings().button().press_matches(*key) {
                self.set_open(true);
                ctx.request_layout();
                ctx.request_redraw();
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
            return EventOutcome::Ignored;
        }
        if focus_keys.next_matches(*key) {
            self.set_open(false);
            ctx.focus_next();
            ctx.request_layout();
            ctx.request_redraw();
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        if focus_keys.previous_matches(*key) {
            self.set_open(false);
            ctx.focus_previous();
            ctx.request_layout();
            ctx.request_redraw();
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        let outcome = self.picker.on_key(*key);
        if outcome.selected {
            self.set_open(false);
            if let Some(value) = self.picker.current_value()
                && let Some(on_select) = &self.on_select
            {
                ctx.emit(on_select(value));
            }
            ctx.request_layout();
            ctx.request_redraw();
        }
        if outcome.canceled {
            self.set_open(false);
            ctx.request_layout();
            ctx.request_redraw();
        }
        finish_event(ctx, outcome)
    }

    fn focus(&mut self, _target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        self.focused = focused;
        self.sync_panel();
        if !focused && self.open {
            self.set_open(false);
        }
        self.picker.set_focused(focused && self.open);
        ctx.request_redraw();
    }

    fn tick(&mut self, dt: StdDuration, _settings: crate::AnimationSettings) -> TickResult {
        if self.hotkey_matcher.tick(dt) {
            self.sync_pending_hotkey_prefix_from_matcher();
            TickResult::CHANGED
        } else {
            TickResult::IDLE
        }
    }
}

#[cfg(test)]
mod tests;
