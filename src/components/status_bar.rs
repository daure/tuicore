use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;
use std::hash::Hash;
use std::time::{Duration, Instant};

#[path = "status_bar_ai.rs"]
mod status_bar_ai;
#[path = "status_bar_keybindings.rs"]
mod status_bar_keybindings;
#[path = "status_bar_style.rs"]
mod status_bar_style;

use status_bar_ai::default_ai_runner;
pub use status_bar_keybindings::StatusBarKeyBindings;
use status_bar_style::{
    STATUS_ACTION_TAIL_WIDTH, centered_field_area, measured_width, status_action_tail,
    status_segment_line, status_segment_text_style, status_segment_width,
};

pub use super::date_time_indicator::{DateTimeIndicator, DateTimeIndicatorFormat};
use super::dropdown::{
    DropdownCommitMode, DropdownLabelPosition, DropdownSearchMode, DropdownVariant,
};
pub use super::weather_forecast_dialog::{
    WeatherForecastDay, WeatherForecastDialog, WeatherForecastError,
};
pub use super::weather_indicator::{
    WeatherIndicator, WeatherReport, WeatherSummary, weather_condition_icon,
};
pub use super::weather_provider::WeatherProviderConfig;
use super::weather_provider::{WeatherFetchReceiver, spawn_weather_fetch};
use super::{AiDock, Button, Dropdown, LlmEvent, Menu, MenuItem, MenuPopupDirection};
use crate::KeySpec;
use crate::{
    Animated, AnimationSettings, ChildKey, EventCtx, EventOutcome, EventRoute, FocusCtx, FocusId,
    FocusRequest, FocusTarget, LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint,
    LifecycleCtx, Theme, ThemeName, TickResult, TreePath, TuiEvent, TuiNode,
    hotkey_underline_style, keybindings, set_theme_and_persist, theme,
};

const MENU_ICON: &str = "󰍜";
const AI_ICON: &str = "";
const DEFAULT_MENU_HOTKEY: &str = ";";
const DEFAULT_AI_HOTKEY: &str = "'";
const WEATHER_DIALOG_BACKDROP_AMOUNT: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatusBarMenuItem {
    Custom {
        id: &'static str,
        label: &'static str,
    },
    Theme,
    WeatherForecast,
    StoreView,
}

impl StatusBarMenuItem {
    fn label(self) -> &'static str {
        match self {
            Self::Custom { label, .. } => label,
            Self::Theme => " Theme",
            Self::WeatherForecast => " Weather forecast",
            Self::StoreView => " Store view",
        }
    }
}

#[derive(Clone)]
struct ThemeChoice {
    name: ThemeName,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct StatusBarAreas {
    menu: Rect,
    ai: Rect,
    action_tail: Rect,
    weather: Rect,
    time: Rect,
    theme: Rect,
}

pub struct StatusBar<M = ()> {
    menu_trigger: Button<M>,
    menu: Menu<StatusBarMenuItem>,
    menu_items: Vec<StatusBarMenuItem>,
    theme_dropdown: Dropdown<ThemeChoice, ThemeName>,
    theme_return_focus: Option<FocusRequest>,
    ai_enabled: bool,
    ai: Button<M>,
    ai_dock: AiDock<M>,
    ai_dock_open: bool,
    ai_dock_area: Rect,
    ai_dock_path: TreePath,
    custom_ai_open: bool,
    weather: WeatherIndicator<M>,
    weather_dialog: WeatherForecastDialog<M>,
    weather_dialog_open: bool,
    weather_dialog_area: Rect,
    weather_dialog_path: TreePath,
    weather_return_focus: Option<FocusRequest>,
    weather_provider: WeatherProviderConfig,
    weather_fetch: Option<WeatherFetchReceiver>,
    weather_last_fetch: Option<Instant>,
    on_custom_menu_item: Option<Box<dyn Fn(&'static str) -> M>>,
    on_weather_open: Option<Box<dyn Fn() -> M>>,
    on_store_view_open: Option<Box<dyn Fn() -> M>>,
    time: DateTimeIndicator<M>,
    areas: StatusBarAreas,
    keybindings: StatusBarKeyBindings,
}

impl<M> StatusBar<M>
where
    M: 'static,
{
    pub fn new() -> Self {
        let keybindings = StatusBarKeyBindings::default();
        let menu_items = default_status_menu_items();
        Self {
            menu_trigger: Button::new(MENU_ICON)
                .hotkey(keybindings.menu_hotkey())
                .tab_stop(false),
            menu: status_menu(menu_items.iter().copied(), keybindings.menu_hotkey()),
            menu_items,
            theme_dropdown: theme_dropdown(),
            theme_return_focus: None,
            ai_enabled: true,
            ai: Button::new(AI_ICON)
                .hotkey(keybindings.ai_hotkey())
                .tab_stop(false),
            ai_dock: default_ai_dock(),
            ai_dock_open: false,
            ai_dock_area: Rect::default(),
            ai_dock_path: TreePath::new(),
            custom_ai_open: false,
            weather: WeatherIndicator::new().tab_stop(false),
            weather_dialog: empty_weather_dialog(),
            weather_dialog_open: false,
            weather_dialog_area: Rect::default(),
            weather_dialog_path: TreePath::from_keys([status_bar_weather_dialog_key()]),
            weather_return_focus: None,
            weather_provider: WeatherProviderConfig::new().enabled(true),
            weather_fetch: None,
            weather_last_fetch: None,
            on_custom_menu_item: None,
            on_weather_open: None,
            on_store_view_open: None,
            time: DateTimeIndicator::new().format(DateTimeIndicatorFormat::DateTime),
            areas: StatusBarAreas::default(),
            keybindings,
        }
    }

    pub fn toggle_menu(&mut self, ctx: &mut EventCtx<M>) -> EventOutcome {
        self.menu.toggle_with_context(ctx);
        ctx.stop_propagation();
        EventOutcome::Handled
    }

    pub fn menu_items(mut self, items: impl IntoIterator<Item = StatusBarMenuItem>) -> Self {
        self.menu_items = items.into_iter().collect();
        self.rebuild_menu();
        self
    }

    pub fn keybindings(mut self, keybindings: StatusBarKeyBindings) -> Self {
        self.set_keybindings(keybindings);
        self
    }

    pub fn set_keybindings(&mut self, keybindings: StatusBarKeyBindings) {
        self.keybindings = keybindings;
        self.menu_trigger.set_hotkey(self.keybindings.menu_hotkey());
        self.ai.set_hotkey(self.keybindings.ai_hotkey());
        self.rebuild_menu();
    }

    pub fn ai_enabled(mut self, enabled: bool) -> Self {
        self.ai_enabled = enabled;
        if !enabled {
            self.ai_dock_open = false;
        }
        self
    }

    pub fn weather_report(mut self, report: WeatherReport) -> Self {
        self.set_weather_report(report);
        self
    }

    pub fn set_weather_report(&mut self, report: WeatherReport) {
        self.weather_provider = self.weather_provider.clone().enabled(false);
        self.weather_fetch = None;
        self.weather_last_fetch = None;
        self.apply_weather_report(report);
    }

    fn apply_weather_report(&mut self, report: WeatherReport) {
        self.weather_dialog.set_report(report.clone());
        self.weather.set_report(report);
    }

    pub fn weather_refresh_needed(&self) -> bool {
        self.weather.refresh_needed()
    }

    pub fn weather_provider(mut self, provider: WeatherProviderConfig) -> Self {
        self.set_weather_provider(provider);
        self
    }

    pub fn set_weather_provider(&mut self, provider: WeatherProviderConfig) {
        self.weather_provider = provider;
        self.weather_fetch = None;
        self.weather_last_fetch = None;
    }

    pub fn on_ai_open(mut self, handler: impl Fn() -> M + 'static) -> Self {
        self.ai = self.ai.on_press(handler);
        self.custom_ai_open = true;
        self
    }

    pub fn on_custom_menu_item(mut self, handler: impl Fn(&'static str) -> M + 'static) -> Self {
        self.on_custom_menu_item = Some(Box::new(handler));
        self
    }

    #[deprecated(
        since = "0.1.0",
        note = "Weather forecast now opens the built-in StatusBar dialog; use `weather_report` or `weather_provider` to configure content"
    )]
    pub fn on_weather_open(mut self, handler: impl Fn() -> M + 'static) -> Self {
        self.on_weather_open = Some(Box::new(handler));
        self
    }

    pub fn on_store_view_open(mut self, handler: impl Fn() -> M + 'static) -> Self {
        self.on_store_view_open = Some(Box::new(handler));
        self.rebuild_menu();
        self
    }

    fn layout_with_current_bounds(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        let overlay_bounds = ctx.overlay_bounds();
        self.areas = self.layout_areas(area, overlay_bounds);
        ctx.push_slot(status_bar_menu_trigger_key(), self.areas.menu, |ctx| {
            self.menu_trigger.layout(self.areas.menu, ctx);
        });
        if self.ai_enabled {
            ctx.push_slot(status_bar_ai_key(), self.areas.ai, |ctx| {
                self.ai.layout(self.areas.ai, ctx);
            });
        }
        ctx.push_slot(status_bar_weather_key(), self.areas.weather, |ctx| {
            self.weather.layout(self.areas.weather, ctx);
        });
        ctx.push_slot(status_bar_time_key(), self.areas.time, |ctx| {
            self.time.layout(self.areas.time, ctx);
        });
        ctx.push_slot(status_bar_menu_panel_key(), self.areas.menu, |ctx| {
            <Menu<StatusBarMenuItem> as TuiNode<M>>::layout(&mut self.menu, self.areas.menu, ctx);
        });
        if self.theme_dropdown.is_open() {
            ctx.push_slot(status_bar_theme_key(), self.areas.theme, |ctx| {
                <Dropdown<ThemeChoice, ThemeName> as TuiNode<M>>::layout(
                    &mut self.theme_dropdown,
                    self.areas.theme,
                    ctx,
                );
            });
        } else {
            let was_disabled = ctx.focus_disabled();
            ctx.set_focus_disabled(true);
            ctx.push_slot_without_hit_region(status_bar_theme_key(), |ctx| {
                <Dropdown<ThemeChoice, ThemeName> as TuiNode<M>>::layout(
                    &mut self.theme_dropdown,
                    self.areas.theme,
                    ctx,
                );
            });
            ctx.set_focus_disabled(was_disabled);
        }
        if self.weather_dialog_open {
            self.weather_dialog_area = overlay_bounds;
            self.weather_dialog_path = ctx.current_path().child(status_bar_weather_dialog_key());
            ctx.push_slot(
                status_bar_weather_dialog_key(),
                self.weather_dialog_area,
                |ctx| {
                    <WeatherForecastDialog<M> as TuiNode<M>>::layout(
                        &mut self.weather_dialog,
                        self.weather_dialog_area,
                        ctx,
                    );
                    let dialog_focus = FocusId::new(crate::components::dialog::DIALOG_FOCUS);
                    ctx.set_focus_receives_events_before_global_hotkeys(dialog_focus.clone(), true);
                    ctx.set_focus_suppresses_global_hotkeys(dialog_focus, true);
                },
            );
        } else {
            self.weather_dialog_path = ctx.current_path().child(status_bar_weather_dialog_key());
        }
        if self.ai_enabled && self.ai_dock_open {
            self.ai_dock_area = bottom_dock_area(overlay_bounds, 80, 80);
            self.ai_dock_path = ctx.current_path().child(status_bar_ai_dock_key());
            ctx.with_global_hotkeys_suppressed(|ctx| {
                ctx.push_slot(status_bar_ai_dock_key(), self.ai_dock_area, |ctx| {
                    <AiDock<M> as TuiNode<M>>::layout(&mut self.ai_dock, self.ai_dock_area, ctx);
                });
            });
        } else {
            self.ai_dock_path = ctx.current_path().child(status_bar_ai_dock_key());
        }
        LayoutResult::new(area)
    }

    fn layout_areas(&self, area: Rect, overlay_bounds: Rect) -> StatusBarAreas {
        let menu_width = measured_width(&self.menu_trigger).min(area.width);
        let ai_width = if self.ai_enabled {
            measured_width(&self.ai).min(area.width.saturating_sub(menu_width))
        } else {
            0
        };
        let action_tail_width = STATUS_ACTION_TAIL_WIDTH.min(
            area.width
                .saturating_sub(menu_width)
                .saturating_sub(ai_width),
        );
        let time_width = status_segment_width(&self.time.label()).min(area.width);
        let weather_width =
            status_segment_width(&self.weather.label()).min(area.width.saturating_sub(time_width));

        let menu = Rect::new(area.x, area.y, menu_width, area.height);
        let ai = Rect::new(
            area.x.saturating_add(menu_width),
            area.y,
            ai_width,
            area.height,
        );
        let action_tail = Rect::new(
            area.x.saturating_add(menu_width).saturating_add(ai_width),
            area.y,
            action_tail_width,
            area.height,
        );
        let time_x = area.x + area.width.saturating_sub(time_width);
        let weather_x = time_x.saturating_sub(weather_width);
        let time = Rect::new(time_x, area.y, time_width, area.height);
        let weather = Rect::new(weather_x, area.y, weather_width, area.height);
        let theme = centered_field_area(overlay_bounds, 36);

        StatusBarAreas {
            menu,
            ai,
            action_tail,
            weather,
            time,
            theme,
        }
    }

    fn activate_menu_item(&mut self, item: StatusBarMenuItem, ctx: &mut EventCtx<M>) {
        match item {
            StatusBarMenuItem::Custom { id, .. } => {
                if let Some(on_custom_menu_item) = &self.on_custom_menu_item {
                    ctx.emit(on_custom_menu_item(id));
                }
                ctx.request_redraw();
                ctx.stop_propagation();
            }
            StatusBarMenuItem::Theme => {
                self.theme_return_focus = ctx.focus_request().cloned();
                self.theme_dropdown.open_immediate_with_context(ctx);
                ctx.stop_propagation();
            }
            StatusBarMenuItem::WeatherForecast => {
                self.open_weather_dialog(ctx);
                ctx.request_redraw();
                ctx.stop_propagation();
            }
            StatusBarMenuItem::StoreView => {
                if let Some(on_store_view_open) = &self.on_store_view_open {
                    ctx.emit(on_store_view_open());
                }
                ctx.request_redraw();
                ctx.stop_propagation();
            }
        }
    }

    fn open_weather_dialog(&mut self, ctx: &mut EventCtx<M>) {
        self.weather_dialog_open = true;
        self.weather_return_focus = ctx.focus_request().cloned();
        if let Some(on_weather_open) = &self.on_weather_open {
            ctx.emit(on_weather_open());
        }
        ctx.request_layout();
        ctx.request_redraw();
        ctx.focus(FocusRequest::TargetAt {
            path: self.weather_dialog_path.clone(),
            id: FocusId::new(crate::components::dialog::DIALOG_FOCUS),
        });
    }

    fn handle_theme_dropdown_event(
        &mut self,
        route: Option<&EventRoute>,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        let was_open = self.theme_dropdown.is_open();
        let outcome = match route {
            Some(route) => self.theme_dropdown.dispatch_event(route, event, ctx),
            None => self.theme_dropdown.event(event, ctx),
        };
        if was_open && !self.theme_dropdown.is_open() {
            ctx.focus(self.theme_return_focus.take().unwrap_or(FocusRequest::Last));
        }
        outcome
    }

    fn open_ai_dock(&mut self, ctx: &mut EventCtx<M>) {
        self.ai_dock_open = true;
        ctx.request_layout();
        ctx.request_redraw();
        ctx.focus(FocusRequest::Path(self.ai_dock_path.clone()));
        ctx.stop_propagation();
    }

    fn close_ai_dock(&mut self, ctx: &mut EventCtx<M>) {
        self.ai_dock_open = false;
        ctx.request_layout();
        ctx.request_redraw();
        ctx.focus(FocusRequest::Last);
        ctx.stop_propagation();
    }

    fn close_ai_dock_if_requested(&mut self, ctx: &mut EventCtx<M>) {
        if self.ai_dock.take_close_requested() {
            self.close_ai_dock(ctx);
        }
    }

    fn close_weather_dialog(&mut self, ctx: &mut EventCtx<M>) {
        self.weather_dialog_open = false;
        ctx.request_layout();
        ctx.request_redraw();
        ctx.focus(
            self.weather_return_focus
                .take()
                .unwrap_or(FocusRequest::Last),
        );
        ctx.stop_propagation();
    }

    fn handle_weather_dialog_event(
        &mut self,
        route: Option<&EventRoute>,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        if weather_dialog_close_event(event) {
            self.close_weather_dialog(ctx);
            return EventOutcome::Handled;
        }
        let outcome = match route {
            Some(route) => self.weather_dialog.dispatch_event(route, event, ctx),
            None => self.weather_dialog.event(event, ctx),
        };
        if outcome.handled() {
            return outcome;
        }
        ctx.stop_propagation();
        EventOutcome::Handled
    }

    fn start_weather_fetch_if_due(&mut self) -> TickResult {
        if !self.weather_provider.is_enabled() || self.weather_fetch.is_some() {
            return TickResult::IDLE;
        }
        let now = Instant::now();
        let refresh_interval = self.weather_provider.refresh_interval_value();
        let due = self.weather_refresh_needed()
            || self
                .weather_last_fetch
                .map(|last| now.duration_since(last) >= refresh_interval)
                .unwrap_or(true);
        if !due {
            return self
                .weather_last_fetch
                .map(|last| {
                    let elapsed = now.duration_since(last);
                    TickResult::scheduled_after(refresh_interval.saturating_sub(elapsed))
                })
                .unwrap_or(TickResult::IDLE);
        }

        self.weather_fetch = Some(spawn_weather_fetch(self.weather_provider.clone()));
        self.weather_last_fetch = Some(now);
        self.weather.set_loading(true);
        self.weather_dialog.set_content([
            "Loading weather forecast…",
            "",
            "Status bar weather is fetching the latest Open-Meteo report.",
        ]);
        TickResult {
            layout: true,
            ..TickResult::ACTIVE
        }
    }

    fn drain_weather_fetch(&mut self) -> TickResult {
        let Some(fetch) = self.weather_fetch.take() else {
            return TickResult::IDLE;
        };
        match fetch.try_recv() {
            Ok(Ok(report)) => {
                self.apply_weather_report(report);
                TickResult {
                    layout: true,
                    ..TickResult::CHANGED
                }
            }
            Ok(Err(error)) => {
                self.weather.set_loading(false);
                self.weather.set_placeholder("Weather unavailable");
                self.weather_dialog.set_content([
                    "Weather unavailable.",
                    "",
                    error.message(),
                    "",
                    "Status bar weather will retry automatically.",
                ]);
                TickResult {
                    layout: true,
                    ..TickResult::CHANGED
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                self.weather_fetch = Some(fetch);
                TickResult::ACTIVE
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.weather.set_loading(false);
                self.weather.set_placeholder("Weather unavailable");
                self.weather_dialog.set_content([
                    "Weather unavailable.",
                    "",
                    "Weather fetch worker disconnected.",
                    "",
                    "Status bar weather will retry automatically.",
                ]);
                TickResult {
                    layout: true,
                    ..TickResult::CHANGED
                }
            }
        }
    }

    fn rebuild_menu(&mut self) {
        self.menu = status_menu(self.effective_menu_items(), self.keybindings.menu_hotkey());
    }

    fn effective_menu_items(&self) -> Vec<StatusBarMenuItem> {
        let mut items = self.menu_items.clone();
        if self.on_store_view_open.is_some() && !items.contains(&StatusBarMenuItem::StoreView) {
            items.push(StatusBarMenuItem::StoreView);
        }
        items
    }
}

impl<M> Default for StatusBar<M>
where
    M: 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<M> TuiNode<M> for StatusBar<M>
where
    M: 'static,
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let width = measured_width(&self.menu_trigger)
            + if self.ai_enabled {
                measured_width(&self.ai)
            } else {
                0
            }
            + STATUS_ACTION_TAIL_WIDTH
            + measured_width(&self.weather)
            + measured_width(&self.time);
        LayoutSizeHint::content(width, 1).normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.layout_with_current_bounds(area, ctx)
    }

    fn render<'a>(&'a self, frame: &mut Frame, _area: Rect, ctx: &mut crate::RenderCtx<'a>) {
        let action_bg = theme().surface_bg();
        self.menu_trigger
            .render_with_inactive_background(frame, self.areas.menu, action_bg);
        if self.ai_enabled {
            self.ai
                .render_with_inactive_background(frame, self.areas.ai, action_bg);
        }
        frame.render_widget(Paragraph::new(status_action_tail()), self.areas.action_tail);
        let weather_bg = theme().weather_sun_fg();
        let time_bg = theme().accent_fg();
        let weather_style = status_segment_text_style(self.weather.is_focused(), weather_bg);
        frame.render_widget(
            Paragraph::new(status_segment_line(
                self.weather
                    .label_spans(weather_style, hotkey_underline_style(weather_style)),
                self.weather.is_focused(),
                weather_bg,
                None,
            )),
            self.areas.weather,
        );
        let time_focused = self.time.is_focused();
        let time_style = status_segment_text_style(time_focused, time_bg);
        frame.render_widget(
            Paragraph::new(status_segment_line(
                self.time
                    .label_spans(time_style, hotkey_underline_style(time_style)),
                time_focused,
                time_bg,
                Some(weather_bg),
            )),
            self.areas.time,
        );
        if self.menu.is_open() {
            <Menu<StatusBarMenuItem> as TuiNode<M>>::render(
                &self.menu,
                frame,
                self.areas.menu,
                ctx,
            );
        }
        if self.theme_dropdown.is_open() {
            <Dropdown<ThemeChoice, ThemeName> as TuiNode<M>>::render(
                &self.theme_dropdown,
                frame,
                self.areas.theme,
                ctx,
            );
        }
        if self.weather_dialog_open {
            super::dialog_layer::dim_backdrop_buffer(
                frame,
                self.weather_dialog_area,
                WEATHER_DIALOG_BACKDROP_AMOUNT,
            );
            <WeatherForecastDialog<M> as TuiNode<M>>::render(
                &self.weather_dialog,
                frame,
                self.weather_dialog_area,
                ctx,
            );
        }
        if self.ai_enabled && self.ai_dock_open {
            <AiDock<M> as TuiNode<M>>::render(&self.ai_dock, frame, self.ai_dock_area, ctx);
        }
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        if route.path.is_empty() {
            return self.event(event, ctx);
        }

        if self.weather_dialog_open {
            let dialog_route = route
                .path
                .without_first_if(&status_bar_weather_dialog_key())
                .map(EventRoute::new);
            return self.handle_weather_dialog_event(dialog_route.as_ref(), event, ctx);
        }

        if let Some(route) = route
            .path
            .without_first_if(&status_bar_menu_trigger_key())
            .map(EventRoute::new)
        {
            let outcome = self.menu_trigger.dispatch_event(&route, event, ctx);
            if outcome.handled() {
                self.menu.toggle_with_context(ctx);
            }
            return outcome;
        }

        if self.ai_enabled {
            if let Some(route) = route
                .path
                .without_first_if(&status_bar_ai_key())
                .map(EventRoute::new)
            {
                let outcome = self.ai.dispatch_event(&route, event, ctx);
                if outcome.handled() && !self.custom_ai_open {
                    self.open_ai_dock(ctx);
                }
                return outcome;
            }
        }

        if let Some(route) = route
            .path
            .without_first_if(&status_bar_weather_key())
            .map(EventRoute::new)
        {
            return self.weather.dispatch_event(&route, event, ctx);
        }

        if let Some(route) = route
            .path
            .without_first_if(&status_bar_time_key())
            .map(EventRoute::new)
        {
            return self.time.dispatch_event(&route, event, ctx);
        }

        if let Some(route) = route
            .path
            .without_first_if(&status_bar_menu_panel_key())
            .map(EventRoute::new)
        {
            let outcome = self.menu.dispatch_event(&route, event, ctx);
            for item in self.menu.take_activated() {
                self.activate_menu_item(item, ctx);
            }
            return outcome;
        }

        if let Some(route) = route
            .path
            .without_first_if(&status_bar_theme_key())
            .map(EventRoute::new)
        {
            return self.handle_theme_dropdown_event(Some(&route), event, ctx);
        }

        if let Some(route) = route
            .path
            .without_first_if(&status_bar_weather_dialog_key())
            .map(EventRoute::new)
        {
            return self.weather_dialog.dispatch_event(&route, event, ctx);
        }

        if self.ai_enabled {
            if let Some(route) = route
                .path
                .without_first_if(&status_bar_ai_dock_key())
                .map(EventRoute::new)
            {
                let outcome = self.ai_dock.dispatch_event(&route, event, ctx);
                self.close_ai_dock_if_requested(ctx);
                if outcome.handled() {
                    return outcome;
                }
                if ai_dock_close_event(event) {
                    self.close_ai_dock(ctx);
                    return EventOutcome::Handled;
                }
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
        }

        EventOutcome::Ignored
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        if self.ai_enabled && self.ai_dock_open {
            let outcome = self.ai_dock.event(event, ctx);
            self.close_ai_dock_if_requested(ctx);
            if outcome.handled() {
                return outcome;
            }
            if ai_dock_close_event(event) {
                self.close_ai_dock(ctx);
                return EventOutcome::Handled;
            }
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        if self.weather_dialog_open {
            return self.handle_weather_dialog_event(None, event, ctx);
        }
        if status_menu_hotkey(event, &self.keybindings) {
            return self.toggle_menu(ctx);
        }
        if self.ai_enabled && status_ai_hotkey(event, &self.keybindings) {
            let outcome = self.ai.event(event, ctx);
            if !self.custom_ai_open {
                self.open_ai_dock(ctx);
            }
            return outcome;
        }
        if self.theme_dropdown.is_open() {
            let outcome = self.handle_theme_dropdown_event(None, event, ctx);
            if outcome.handled() {
                return outcome;
            }
        }
        if self.menu.is_open() {
            let outcome = self.menu.event(event, ctx);
            for item in self.menu.take_activated() {
                self.activate_menu_item(item, ctx);
            }
            if outcome.handled() {
                return outcome;
            }
        }
        EventOutcome::Ignored
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        if let Some(target) = target.for_child(&status_bar_menu_trigger_key()) {
            self.menu_trigger.dispatch_focus(&target, focused, ctx);
        } else if self.ai_enabled
            && let Some(target) = target.for_child(&status_bar_ai_key())
        {
            self.ai.dispatch_focus(&target, focused, ctx);
        } else if let Some(target) = target.for_child(&status_bar_weather_key()) {
            self.weather.dispatch_focus(&target, focused, ctx);
        } else if let Some(target) = target.for_child(&status_bar_time_key()) {
            self.time.dispatch_focus(&target, focused, ctx);
        } else if let Some(target) = target.for_child(&status_bar_menu_panel_key()) {
            self.menu.dispatch_focus(&target, focused, ctx);
        } else if let Some(target) = target.for_child(&status_bar_theme_key()) {
            self.theme_dropdown.dispatch_focus(&target, focused, ctx);
        } else if let Some(target) = target.for_child(&status_bar_weather_dialog_key()) {
            self.weather_dialog.dispatch_focus(&target, focused, ctx);
        } else if self.ai_enabled
            && let Some(target) = target.for_child(&status_bar_ai_dock_key())
        {
            self.ai_dock.dispatch_focus(&target, focused, ctx);
        }
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<M>) {
        let tick = self.start_weather_fetch_if_due();
        if tick.changed {
            ctx.request_redraw();
        }
        if tick.active {
            ctx.request_tick();
        }
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(&mut self.menu_trigger, dt, settings)
            .merge(if self.ai_enabled {
                Animated::tick(&mut self.ai, dt, settings)
            } else {
                TickResult::IDLE
            })
            .merge(self.drain_weather_fetch())
            .merge(<WeatherIndicator<M> as TuiNode<M>>::tick(
                &mut self.weather,
                dt,
                settings,
            ))
            .merge(self.start_weather_fetch_if_due())
            .merge(<DateTimeIndicator<M> as TuiNode<M>>::tick(
                &mut self.time,
                dt,
                settings,
            ))
            .merge(<WeatherForecastDialog<M> as TuiNode<M>>::tick(
                &mut self.weather_dialog,
                dt,
                settings,
            ))
            .merge(if self.ai_enabled && self.ai_dock_open {
                <AiDock<M> as TuiNode<M>>::tick(&mut self.ai_dock, dt, settings)
            } else {
                TickResult::IDLE
            })
            .merge(Animated::tick(&mut self.menu, dt, settings))
            .merge(Animated::tick(&mut self.theme_dropdown, dt, settings))
    }
}

fn empty_weather_dialog<M>() -> WeatherForecastDialog<M> {
    let mut dialog = WeatherForecastDialog::new().content([
        "No weather report loaded.",
        "",
        "Pass `StatusBar::weather_report(...)` to show a forecast here.",
    ]);
    dialog.dialog_mut().set_top_left("Weather forecast");
    dialog
}

fn default_ai_dock<M>() -> AiDock<M>
where
    M: 'static,
{
    AiDock::new(default_ai_runner)
}

fn bottom_dock_area(area: Rect, height_percent: u16, width_percent: u16) -> Rect {
    let width = area.width.saturating_mul(width_percent.min(100)) / 100;
    let height = area.height.saturating_mul(height_percent.min(100)) / 100;
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height),
        width,
        height,
    )
}

fn status_menu(
    items: impl IntoIterator<Item = StatusBarMenuItem>,
    trigger_hotkey: &str,
) -> Menu<StatusBarMenuItem> {
    Menu::new(
        items
            .into_iter()
            .map(|item| MenuItem::new(item, item.label())),
    )
    .visible_items(8)
    .popup_direction(MenuPopupDirection::Up)
    .trigger_hotkey(trigger_hotkey)
}

fn default_status_menu_items() -> Vec<StatusBarMenuItem> {
    vec![StatusBarMenuItem::Theme, StatusBarMenuItem::WeatherForecast]
}

fn status_menu_hotkey(event: &TuiEvent, keybindings: &StatusBarKeyBindings) -> bool {
    matches!(event, TuiEvent::Key(key) if keybindings.menu_toggle_matches(*key))
}

fn status_ai_hotkey(event: &TuiEvent, keybindings: &StatusBarKeyBindings) -> bool {
    matches!(event, TuiEvent::Key(key) if keybindings.ai_open_matches(*key))
}

fn weather_dialog_close_event(event: &TuiEvent) -> bool {
    matches!(event, TuiEvent::Key(key) if KeySpec::plain('x').matches(*key) || keybindings().focus().unfocus_matches(*key))
}

fn ai_dock_close_event(event: &TuiEvent) -> bool {
    matches!(event, TuiEvent::Key(key) if keybindings().focus().unfocus_matches(*key))
}

fn theme_dropdown() -> Dropdown<ThemeChoice, ThemeName> {
    Dropdown::single(
        ThemeName::ALL.map(|name| ThemeChoice { name }),
        |row| row.name,
        |row| row.name.label().to_string(),
    )
    .selected_one(theme().name())
    .variant(DropdownVariant::Filled)
    .label("Theme")
    .label_position(DropdownLabelPosition::Inline)
    .search_mode(DropdownSearchMode::Fuzzy)
    .commit_mode(DropdownCommitMode::Immediate)
    .centered(true)
    .tab_stop(false)
    .max_popup_height(12)
    .on_select(|ids| {
        if let Some(name) = ids.first() {
            let _ = set_theme_and_persist(Theme::named(*name));
        }
    })
}

fn status_bar_menu_trigger_key() -> ChildKey {
    ChildKey::new("status-menu-trigger")
}

fn status_bar_menu_panel_key() -> ChildKey {
    ChildKey::new("status-menu-panel")
}

fn status_bar_theme_key() -> ChildKey {
    ChildKey::new("status-theme")
}

fn status_bar_ai_key() -> ChildKey {
    ChildKey::new("status-ai")
}

fn status_bar_ai_dock_key() -> ChildKey {
    ChildKey::new("status-ai-dock")
}

fn status_bar_weather_key() -> ChildKey {
    ChildKey::new("status-weather")
}

fn status_bar_time_key() -> ChildKey {
    ChildKey::new("status-time")
}

fn status_bar_weather_dialog_key() -> ChildKey {
    ChildKey::new("status-weather-dialog")
}

#[cfg(test)]
mod tests {
    use ratatui::{
        Terminal,
        backend::TestBackend,
        layout::Rect,
        style::{Color, Modifier, Style},
        widgets::Paragraph,
    };

    use super::*;
    use crate::components::weather_provider::WeatherFetchError;
    use crate::{FocusId, FocusRequest, Key, KeyEvent, Propagation, TreePath, TuiEvent};

    #[test]
    fn built_in_menu_items_include_nerd_font_icons() {
        assert_eq!(StatusBarMenuItem::Theme.label(), " Theme");
        assert_eq!(
            StatusBarMenuItem::WeatherForecast.label(),
            " Weather forecast"
        );
        assert_eq!(StatusBarMenuItem::StoreView.label(), " Store view");
    }

    #[test]
    fn action_segments_use_surface_background_role() {
        let mut status = StatusBar::<()>::new();
        let area = Rect::new(0, 0, 80, 1);
        status.layout(area, &mut LayoutCtx::new());
        let mut terminal = Terminal::new(TestBackend::new(area.width, area.height))
            .expect("terminal should build");

        terminal
            .draw(|frame| {
                <StatusBar<()> as TuiNode<()>>::render(
                    &status,
                    frame,
                    area,
                    &mut crate::RenderCtx::new(),
                );
            })
            .expect("status bar should render");

        assert_eq!(
            terminal
                .backend()
                .buffer()
                .cell((status.areas.menu.x, status.areas.menu.y))
                .expect("menu cell should exist")
                .bg,
            theme().surface_bg()
        );
        assert_eq!(
            status_action_tail().spans[0].style.fg,
            Some(theme().surface_bg())
        );
    }

    #[test]
    fn focused_status_segment_is_bold_and_unfocused_segment_is_not() {
        let focused = status_segment_text_style(true, theme().surface_bg());
        let unfocused = status_segment_text_style(false, theme().surface_bg());

        assert_eq!(focused.fg, Some(theme().highlight_fg()));
        assert_eq!(focused.bg, Some(theme().highlight_bg()));
        assert!(focused.add_modifier.contains(Modifier::BOLD));
        assert!(!unfocused.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn footer_hotkeys_are_focus_targets_but_default_weather_is_not_dead_focus() {
        let mut status = StatusBar::<()>::new();
        let mut layout = LayoutCtx::new();

        status.layout(Rect::new(0, 0, 80, 1), &mut layout);

        let target_by_path = |key: ChildKey| {
            layout
                .focus_targets()
                .iter()
                .find(|target| target.path.first() == Some(&key))
                .expect("footer target should be registered")
        };

        let menu = target_by_path(status_bar_menu_trigger_key());
        assert!(!menu.tab_stop);
        assert_eq!(menu.hotkey_sequences, vec![DEFAULT_MENU_HOTKEY]);

        let ai = target_by_path(status_bar_ai_key());
        assert!(!ai.tab_stop);
        assert_eq!(ai.hotkey_sequences, vec![DEFAULT_AI_HOTKEY]);

        assert!(
            layout
                .focus_targets()
                .iter()
                .all(|target| target.path.first() != Some(&status_bar_weather_key()))
        );
    }

    #[test]
    fn disabled_ai_removes_control_hotkey_and_builtin_dock_access() {
        let area = Rect::new(0, 0, 80, 1);
        let enabled_width = StatusBar::<()>::new()
            .measure(LayoutProposal::unbounded())
            .preferred
            .width;
        let mut status = StatusBar::<()>::new().ai_enabled(false);
        let disabled_width = status.measure(LayoutProposal::unbounded()).preferred.width;
        let mut layout = LayoutCtx::new();

        status.layout(area, &mut layout);

        assert_eq!(status.areas.ai.width, 0);
        assert_eq!(enabled_width - disabled_width, measured_width(&status.ai));
        assert!(
            layout
                .focus_targets()
                .iter()
                .all(|target| target.path.first() != Some(&status_bar_ai_key()))
        );
        assert!(
            layout
                .hit_regions()
                .iter()
                .all(|region| region.path.first() != Some(&status_bar_ai_key()))
        );

        let mut hotkey_ctx = EventCtx::default();
        let hotkey_outcome = status.event(
            &TuiEvent::Key(KeyEvent::from(Key::Char('\''))),
            &mut hotkey_ctx,
        );
        assert!(!hotkey_outcome.handled());
        assert!(!status.ai_dock_open);

        let mut routed_ctx = EventCtx::default();
        let routed_outcome = status.dispatch_event(
            &EventRoute::new(TreePath::from_keys([status_bar_ai_dock_key()])),
            &TuiEvent::Key(KeyEvent::from(Key::Char('z'))),
            &mut routed_ctx,
        );
        assert!(!routed_outcome.handled());
        assert_eq!(routed_ctx.propagation(), Propagation::Continue);
    }

    #[test]
    fn closed_theme_dropdown_does_not_register_hit_or_focus_region() {
        let mut status = StatusBar::<()>::new();
        let mut layout = LayoutCtx::new();

        status.layout(Rect::new(0, 0, 80, 1), &mut layout);

        assert!(
            layout
                .hit_regions()
                .iter()
                .all(|region| region.path.first() != Some(&status_bar_theme_key()))
        );
        assert!(
            layout
                .focus_targets()
                .iter()
                .all(|target| target.path.first() != Some(&status_bar_theme_key()))
        );
    }

    #[test]
    fn opening_theme_dropdown_requests_layout_and_targets_visible_search() {
        let mut status = StatusBar::<()>::new();
        let mut layout = LayoutCtx::new();
        status.layout(Rect::new(0, 0, 80, 1), &mut layout);
        let mut ctx = EventCtx::default();

        status.activate_menu_item(StatusBarMenuItem::Theme, &mut ctx);

        assert!(ctx.layout_requested());
        assert!(ctx.redraw_requested());
        assert_eq!(
            ctx.focus_request(),
            Some(&FocusRequest::TargetAt {
                path: TreePath::from_keys([status_bar_theme_key()]),
                id: FocusId::new("input"),
            })
        );
    }

    #[test]
    fn closing_theme_dropdown_restores_focus_held_before_menu_closed() {
        let mut status = StatusBar::<()>::new();
        let return_focus = FocusRequest::TargetAt {
            path: TreePath::from_keys([ChildKey::new("main")]),
            id: FocusId::new("data-view"),
        };
        let mut open_ctx = EventCtx::default();
        open_ctx.focus(return_focus.clone());
        status.activate_menu_item(StatusBarMenuItem::Theme, &mut open_ctx);
        let mut close_ctx = EventCtx::default();

        let outcome = status.event(&TuiEvent::Key(KeyEvent::from(Key::Esc)), &mut close_ctx);

        assert!(outcome.handled());
        assert!(!status.theme_dropdown.is_open());
        assert_eq!(close_ctx.focus_request(), Some(&return_focus));
    }

    #[test]
    fn store_view_menu_item_is_opt_in_or_explicit() {
        let default_status = StatusBar::<()>::new();
        assert!(
            !default_status
                .effective_menu_items()
                .contains(&StatusBarMenuItem::StoreView)
        );

        let callback_status = StatusBar::new().on_store_view_open(|| ());
        assert!(
            callback_status
                .effective_menu_items()
                .contains(&StatusBarMenuItem::StoreView)
        );

        let explicit_status = StatusBar::<()>::new().menu_items([StatusBarMenuItem::StoreView]);
        assert_eq!(
            explicit_status.effective_menu_items(),
            vec![StatusBarMenuItem::StoreView]
        );

        let reordered_status = StatusBar::new()
            .on_store_view_open(|| ())
            .menu_items([StatusBarMenuItem::Theme]);
        assert_eq!(
            reordered_status.effective_menu_items(),
            vec![StatusBarMenuItem::Theme, StatusBarMenuItem::StoreView]
        );
    }

    #[test]
    fn store_view_activation_emits_callback_message() {
        let mut status = StatusBar::new().on_store_view_open(|| "store");
        let mut ctx = EventCtx::default();

        status.activate_menu_item(StatusBarMenuItem::StoreView, &mut ctx);

        assert_eq!(ctx.messages(), &["store"]);
        assert!(ctx.redraw_requested());
        assert_eq!(ctx.propagation(), Propagation::Stopped);
    }

    #[test]
    fn ai_dock_open_focuses_dock_path_not_global_textarea_id() {
        let mut status = StatusBar::<()>::new();
        let mut layout = LayoutCtx::new();
        layout.push_slot(ChildKey::new("footer"), Rect::new(0, 0, 100, 1), |ctx| {
            status.layout(Rect::new(0, 0, 100, 1), ctx);
        });
        let mut ctx = EventCtx::default();

        status.open_ai_dock(&mut ctx);

        assert!(status.ai_dock_open);
        assert_eq!(
            ctx.focus_request(),
            Some(&FocusRequest::Path(TreePath::from_keys([
                ChildKey::new("footer"),
                status_bar_ai_dock_key(),
            ])))
        );
    }

    #[test]
    fn open_ai_dock_isolates_its_focus_targets_from_background_hotkeys() {
        let bounds = Rect::new(0, 0, 100, 40);
        let footer = Rect::new(0, 39, 100, 1);
        let mut status = StatusBar::<()>::new();
        status.open_ai_dock(&mut EventCtx::default());
        let mut layout = LayoutCtx::new();

        layout.with_overlay_bounds(bounds, |ctx| status.layout(footer, ctx));

        let dock_targets = layout
            .focus_targets()
            .iter()
            .filter(|target| target.path.first() == Some(&status_bar_ai_dock_key()))
            .collect::<Vec<_>>();
        assert!(!dock_targets.is_empty());
        assert!(
            dock_targets
                .iter()
                .all(|target| target.suppress_global_hotkeys)
        );
    }

    #[test]
    fn open_ai_dock_consumes_unhandled_keys() {
        let mut status = StatusBar::<()>::new();
        status.open_ai_dock(&mut EventCtx::default());
        let mut ctx = EventCtx::default();

        let outcome = status.event(&TuiEvent::Key(KeyEvent::from(Key::Char('z'))), &mut ctx);

        assert!(outcome.handled());
        assert_eq!(ctx.propagation(), Propagation::Stopped);
        assert!(status.ai_dock_open);
    }

    #[test]
    fn ai_dock_prompt_escape_does_not_close_status_bar_dock() {
        let mut status = StatusBar::<()>::new();
        let mut open_ctx = EventCtx::default();
        status.open_ai_dock(&mut open_ctx);
        let mut layout = LayoutCtx::new();
        layout.with_overlay_bounds(Rect::new(0, 0, 100, 40), |ctx| {
            status.layout(Rect::new(0, 39, 100, 1), ctx);
        });
        let prompt = layout
            .focus_targets()
            .iter()
            .find(|target| target.id.as_str() == "textarea")
            .cloned()
            .expect("AI prompt should register textarea focus");
        let mut focus_ctx = FocusCtx::default();
        status.dispatch_focus(&prompt, true, &mut focus_ctx);

        let mut enter_ctx = EventCtx::default();
        status.dispatch_event(
            &EventRoute::new(prompt.path.clone()),
            &TuiEvent::Key(KeyEvent::from(Key::Enter)),
            &mut enter_ctx,
        );
        let mut escape_ctx = EventCtx::default();
        let outcome = status.dispatch_event(
            &EventRoute::new(prompt.path.clone()),
            &TuiEvent::Key(KeyEvent::from(Key::Esc)),
            &mut escape_ctx,
        );

        assert!(outcome.handled());
        assert!(status.ai_dock_open);
    }

    #[test]
    fn weather_forecast_menu_opens_built_in_dialog_without_callback() {
        let mut status = StatusBar::<()>::new();
        let mut layout = LayoutCtx::new();
        layout.push_slot(ChildKey::new("footer"), Rect::new(0, 0, 100, 1), |ctx| {
            status.layout(Rect::new(0, 0, 100, 1), ctx);
        });
        let mut ctx = EventCtx::default();

        status.activate_menu_item(StatusBarMenuItem::WeatherForecast, &mut ctx);

        assert!(status.weather_dialog_open);
        assert!(ctx.layout_requested());
        assert!(ctx.redraw_requested());
        assert_eq!(ctx.propagation(), Propagation::Stopped);
        assert_eq!(
            ctx.focus_request(),
            Some(&FocusRequest::TargetAt {
                path: TreePath::from_keys([
                    ChildKey::new("footer"),
                    status_bar_weather_dialog_key(),
                ]),
                id: FocusId::new(crate::components::dialog::DIALOG_FOCUS),
            })
        );
    }

    #[test]
    fn weather_dialog_receives_keys_before_background_hotkeys() {
        let bounds = Rect::new(0, 0, 100, 30);
        let footer = Rect::new(0, 29, 100, 1);
        let mut status = StatusBar::<()>::new();
        status.activate_menu_item(StatusBarMenuItem::WeatherForecast, &mut EventCtx::default());
        let mut layout = LayoutCtx::new();

        layout.with_overlay_bounds(bounds, |ctx| status.layout(footer, ctx));

        let dialog = layout
            .focus_targets()
            .iter()
            .find(|target| target.id.as_str() == crate::components::dialog::DIALOG_FOCUS)
            .expect("weather dialog should register focus");
        assert!(dialog.focused_events_before_global_hotkeys);
    }

    #[test]
    fn weather_dialog_dims_content_behind_it() {
        let bounds = Rect::new(0, 0, 100, 30);
        let footer = Rect::new(0, 29, 100, 1);
        let mut status = StatusBar::<()>::new();
        status.activate_menu_item(StatusBarMenuItem::WeatherForecast, &mut EventCtx::default());
        let mut layout = LayoutCtx::new();
        layout.with_overlay_bounds(bounds, |ctx| status.layout(footer, ctx));
        let mut terminal = Terminal::new(TestBackend::new(bounds.width, bounds.height))
            .expect("terminal should build");

        terminal
            .draw(|frame| {
                frame.render_widget(
                    Paragraph::new("background").style(Style::default().fg(Color::White)),
                    bounds,
                );
                <StatusBar<()> as TuiNode<()>>::render(
                    &status,
                    frame,
                    footer,
                    &mut crate::RenderCtx::new(),
                );
            })
            .expect("status bar should render");

        assert!(
            terminal
                .backend()
                .buffer()
                .cell((0, 0))
                .expect("background cell should exist")
                .modifier
                .contains(Modifier::DIM)
        );
    }

    #[test]
    fn default_status_bar_enables_builtin_weather_provider() {
        let status = StatusBar::<()>::new();

        assert!(status.weather_provider.is_enabled());
    }

    #[test]
    fn explicit_weather_report_prevents_default_fetch_on_mount() {
        let mut status =
            StatusBar::<()>::new().weather_report(WeatherReport::custom("21 °C", "Sunny"));
        let mut lifecycle = LifecycleCtx::default();

        status.mount(&mut lifecycle);

        assert!(!status.weather_provider.is_enabled());
        assert!(status.weather_fetch.is_none());
    }

    #[test]
    #[allow(deprecated)]
    fn weather_forecast_callback_is_emitted_when_dialog_opens() {
        let mut status = StatusBar::new().on_weather_open(|| "weather");
        let mut ctx = EventCtx::default();

        status.activate_menu_item(StatusBarMenuItem::WeatherForecast, &mut ctx);

        assert!(status.weather_dialog_open);
        assert_eq!(ctx.messages(), &["weather"]);
    }

    #[test]
    fn built_in_weather_dialog_closes_on_dialog_close_key() {
        let mut status = StatusBar::<()>::new();
        let mut ctx = EventCtx::default();
        status.activate_menu_item(StatusBarMenuItem::WeatherForecast, &mut ctx);
        let mut close_ctx = EventCtx::default();

        let outcome = status.event(
            &TuiEvent::Key(KeyEvent::from(Key::Char('x'))),
            &mut close_ctx,
        );

        assert!(outcome.handled());
        assert!(!status.weather_dialog_open);
        assert!(close_ctx.layout_requested());
        assert!(close_ctx.redraw_requested());
        assert_eq!(close_ctx.focus_request(), Some(&FocusRequest::Last));
    }

    #[test]
    fn built_in_weather_dialog_consumes_unhandled_tab() {
        let mut status = StatusBar::<()>::new();
        let mut open_ctx = EventCtx::default();
        status.activate_menu_item(StatusBarMenuItem::WeatherForecast, &mut open_ctx);
        let mut tab_ctx = EventCtx::default();

        let outcome = status.event(&TuiEvent::Key(KeyEvent::from(Key::Tab)), &mut tab_ctx);

        assert!(outcome.handled());
        assert_eq!(tab_ctx.propagation(), Propagation::Stopped);
        assert_eq!(tab_ctx.focus_request(), None);
        assert!(status.weather_dialog_open);
    }

    #[test]
    fn built_in_weather_dialog_consumes_routed_unhandled_backtab() {
        let mut status = StatusBar::<()>::new();
        let mut open_ctx = EventCtx::default();
        status.activate_menu_item(StatusBarMenuItem::WeatherForecast, &mut open_ctx);
        let mut backtab_ctx = EventCtx::default();
        let route = EventRoute::new(TreePath::from_keys([status_bar_weather_dialog_key()]));

        let outcome = status.dispatch_event(
            &route,
            &TuiEvent::Key(KeyEvent::from(Key::BackTab)),
            &mut backtab_ctx,
        );

        assert!(outcome.handled());
        assert_eq!(backtab_ctx.propagation(), Propagation::Stopped);
        assert_eq!(backtab_ctx.focus_request(), None);
        assert!(status.weather_dialog_open);
    }

    #[test]
    fn built_in_weather_dialog_restores_menu_return_focus_on_close() {
        let mut status = StatusBar::<()>::new();
        let return_focus = FocusRequest::TargetAt {
            path: TreePath::from_keys([ChildKey::new("main")]),
            id: FocusId::new("list"),
        };
        let mut ctx = EventCtx::default();
        ctx.focus(return_focus.clone());
        status.activate_menu_item(StatusBarMenuItem::WeatherForecast, &mut ctx);
        let mut close_ctx = EventCtx::default();

        let outcome = status.event(
            &TuiEvent::Key(KeyEvent::from(Key::Char('x'))),
            &mut close_ctx,
        );

        assert!(outcome.handled());
        assert_eq!(close_ctx.focus_request(), Some(&return_focus));
    }

    #[test]
    fn completed_builtin_weather_fetch_updates_indicator_and_dialog() {
        let mut status =
            StatusBar::<()>::new().weather_provider(WeatherProviderConfig::new().enabled(false));
        let (tx, rx) = std::sync::mpsc::channel();
        status.weather_fetch = Some(rx);
        let report = WeatherReport::custom("21(23) °C", "Sunny");
        tx.send(Ok(report)).expect("test receiver should be alive");

        let result = status.tick(Duration::from_millis(16), AnimationSettings::default());

        assert!(result.changed);
        assert!(result.layout);
        assert!(!result.active);
        assert!(status.weather_fetch.is_none());
        assert!(status.weather.label().contains("21(23) °C"));
        assert!(status.weather.label().contains("Sunny"));
    }

    #[test]
    fn failed_builtin_weather_fetch_shows_unavailable_state() {
        let mut status =
            StatusBar::<()>::new().weather_provider(WeatherProviderConfig::new().enabled(false));
        let (tx, rx) = std::sync::mpsc::channel();
        status.weather_fetch = Some(rx);
        tx.send(Err(WeatherFetchError::new("offline")))
            .expect("test receiver should be alive");

        let result = status.tick(Duration::from_millis(16), AnimationSettings::default());

        assert!(result.changed);
        assert!(result.layout);
        assert!(!result.active);
        assert!(status.weather_fetch.is_none());
        assert!(status.weather.label().contains("Weather unavailable"));
    }
}
