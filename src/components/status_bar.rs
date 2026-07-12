use std::hash::Hash;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use std::{env, thread};

use futures::StreamExt;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use rig::agent::{MultiTurnStreamItem, Text as RigText};
use rig::client::CompletionClient;
use rig::providers::chatgpt;
use rig::streaming::{StreamedAssistantContent, StreamingPrompt};

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
use crate::{
    Animated, AnimationSettings, ChildKey, EventCtx, EventOutcome, EventRoute, FocusCtx, FocusId,
    FocusRequest, FocusTarget, LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint,
    LifecycleCtx, Theme, ThemeName, TickResult, TreePath, TuiEvent, TuiNode,
    hotkey_underline_style, keybindings, line_width, set_theme, theme,
};
use crate::{KeyEvent, KeySpec};

const MENU_ICON: &str = "󰍜";
const AI_ICON: &str = "";
const DEFAULT_MENU_HOTKEY: &str = "`";
const DEFAULT_AI_HOTKEY: &str = "'";

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
            Self::Theme => "Theme",
            Self::WeatherForecast => "Weather forecast",
            Self::StoreView => "Store view",
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBarKeyBindings {
    menu_toggle: Vec<KeySpec>,
    ai_open: Vec<KeySpec>,
    menu_hotkey: String,
    ai_hotkey: String,
}

impl Default for StatusBarKeyBindings {
    fn default() -> Self {
        Self {
            menu_toggle: vec![KeySpec::plain('`')],
            ai_open: vec![KeySpec::plain('\'')],
            menu_hotkey: DEFAULT_MENU_HOTKEY.to_string(),
            ai_hotkey: DEFAULT_AI_HOTKEY.to_string(),
        }
    }
}

impl StatusBarKeyBindings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_menu_toggle(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.menu_toggle = keys.into_iter().collect();
    }

    pub fn with_menu_toggle(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_menu_toggle(keys);
        self
    }

    pub fn set_ai_open(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.ai_open = keys.into_iter().collect();
    }

    pub fn with_ai_open(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_ai_open(keys);
        self
    }

    pub fn set_menu_hotkey(&mut self, hotkey: impl Into<String>) {
        self.menu_hotkey = hotkey.into();
    }

    pub fn with_menu_hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.set_menu_hotkey(hotkey);
        self
    }

    pub fn set_ai_hotkey(&mut self, hotkey: impl Into<String>) {
        self.ai_hotkey = hotkey.into();
    }

    pub fn with_ai_hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.set_ai_hotkey(hotkey);
        self
    }

    pub fn menu_toggle_matches(&self, key: impl Into<KeyEvent>) -> bool {
        let key = key.into();
        self.menu_toggle
            .iter()
            .copied()
            .any(|binding| binding.matches(key))
    }

    pub fn ai_open_matches(&self, key: impl Into<KeyEvent>) -> bool {
        let key = key.into();
        self.ai_open
            .iter()
            .copied()
            .any(|binding| binding.matches(key))
    }

    pub fn menu_hotkey(&self) -> &str {
        &self.menu_hotkey
    }

    pub fn ai_hotkey(&self) -> &str {
        &self.ai_hotkey
    }
}

pub struct StatusBar<M = ()> {
    menu_trigger: Button<M>,
    menu: Menu<StatusBarMenuItem>,
    menu_items: Vec<StatusBarMenuItem>,
    theme_dropdown: Dropdown<ThemeChoice, ThemeName>,
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

    pub fn weather_report(mut self, report: WeatherReport) -> Self {
        self.set_weather_report(report);
        self
    }

    pub fn set_weather_report(&mut self, report: WeatherReport) {
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
        ctx.push_slot(status_bar_ai_key(), self.areas.ai, |ctx| {
            self.ai.layout(self.areas.ai, ctx);
        });
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
                },
            );
        } else {
            self.weather_dialog_path = ctx.current_path().child(status_bar_weather_dialog_key());
        }
        if self.ai_dock_open {
            self.ai_dock_area = bottom_dock_area(overlay_bounds, 80, 80);
            self.ai_dock_path = ctx.current_path().child(status_bar_ai_dock_key());
            ctx.push_slot(status_bar_ai_dock_key(), self.ai_dock_area, |ctx| {
                <AiDock<M> as TuiNode<M>>::layout(&mut self.ai_dock, self.ai_dock_area, ctx);
            });
        } else {
            self.ai_dock_path = ctx.current_path().child(status_bar_ai_dock_key());
        }
        LayoutResult::new(area)
    }

    fn layout_areas(&self, area: Rect, overlay_bounds: Rect) -> StatusBarAreas {
        let menu_width = measured_width(&self.menu_trigger).min(area.width);
        let ai_width = measured_width(&self.ai).min(area.width.saturating_sub(menu_width));
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
                self.theme_dropdown.open_with_context(ctx);
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
        TickResult::ACTIVE
    }

    fn drain_weather_fetch(&mut self) -> TickResult {
        let Some(fetch) = self.weather_fetch.take() else {
            return TickResult::IDLE;
        };
        match fetch.try_recv() {
            Ok(Ok(report)) => {
                self.set_weather_report(report);
                TickResult::CHANGED
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
                TickResult::CHANGED
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
                TickResult::CHANGED
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
            + measured_width(&self.ai)
            + STATUS_ACTION_TAIL_WIDTH
            + measured_width(&self.weather)
            + measured_width(&self.time);
        LayoutSizeHint::content(width, 1).normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.layout_with_current_bounds(area, ctx)
    }

    fn render<'a>(&'a self, frame: &mut Frame, _area: Rect, ctx: &mut crate::RenderCtx<'a>) {
        let action_bg = theme().border_fg();
        self.menu_trigger
            .render_with_inactive_background(frame, self.areas.menu, action_bg);
        self.ai
            .render_with_inactive_background(frame, self.areas.ai, action_bg);
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
            <WeatherForecastDialog<M> as TuiNode<M>>::render(
                &self.weather_dialog,
                frame,
                self.weather_dialog_area,
                ctx,
            );
        }
        if self.ai_dock_open {
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
            return self.theme_dropdown.dispatch_event(&route, event, ctx);
        }

        if let Some(route) = route
            .path
            .without_first_if(&status_bar_weather_dialog_key())
            .map(EventRoute::new)
        {
            return self.weather_dialog.dispatch_event(&route, event, ctx);
        }

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
            return outcome;
        }

        EventOutcome::Ignored
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        if self.ai_dock_open {
            let outcome = self.ai_dock.event(event, ctx);
            self.close_ai_dock_if_requested(ctx);
            if outcome.handled() {
                return outcome;
            }
            if ai_dock_close_event(event) {
                self.close_ai_dock(ctx);
                return EventOutcome::Handled;
            }
        }
        if self.weather_dialog_open {
            return self.handle_weather_dialog_event(None, event, ctx);
        }
        if status_menu_hotkey(event, &self.keybindings) {
            return self.toggle_menu(ctx);
        }
        if status_ai_hotkey(event, &self.keybindings) {
            let outcome = self.ai.event(event, ctx);
            if !self.custom_ai_open {
                self.open_ai_dock(ctx);
            }
            return outcome;
        }
        if self.theme_dropdown.is_open() {
            let outcome = self.theme_dropdown.event(event, ctx);
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
        } else if let Some(target) = target.for_child(&status_bar_ai_key()) {
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
        } else if let Some(target) = target.for_child(&status_bar_ai_dock_key()) {
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
            .merge(Animated::tick(&mut self.ai, dt, settings))
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
            .merge(if self.ai_dock_open {
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

fn default_ai_runner(
    prompt: String,
    history: Vec<rig::message::Message>,
    sender: mpsc::Sender<LlmEvent>,
    request_id: u64,
    provider: String,
    model: String,
) {
    thread::spawn(move || {
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = sender.send(LlmEvent::error(
                    request_id,
                    format!("Tokio runtime error: {error}"),
                ));
                return;
            }
        };

        runtime.block_on(async move {
            if !provider.is_empty() && provider != "openai" {
                let _ = sender.send(LlmEvent::error(
                    request_id,
                    format!("Unsupported default AI provider: {provider}"),
                ));
                return;
            }

            let model = resolve_chatgpt_model(model);
            let status_sender = sender.clone();
            let token_dir = chatgpt_token_dir();
            let client = match chatgpt::Client::builder()
                .oauth()
                .token_dir(token_dir.clone())
                .on_device_code(move |code| {
                    let _ = status_sender.send(LlmEvent::status(
                        request_id,
                        format!(
                            "OAuth: Open {} and enter code {}",
                            code.verification_uri, code.user_code
                        ),
                    ));
                })
                .build()
            {
                Ok(client) => client,
                Err(error) => {
                    let _ = sender.send(LlmEvent::error(
                        request_id,
                        format!("Failed to build ChatGPT client: {error}"),
                    ));
                    return;
                }
            };

            let _ = sender.send(LlmEvent::status(request_id, "Authorizing..."));
            if let Err(error) = client.authorize().await {
                let _ = sender.send(LlmEvent::error(
                    request_id,
                    format!("ChatGPT OAuth failed: {error}"),
                ));
                return;
            }

            let model_name = model.strip_prefix("openai/").unwrap_or(&model).to_string();
            let agent = client
                .agent(&model_name)
                .preamble("You are a concise assistant inside a terminal UI. Help with the current app workflow and keep answers practical.")
                .build();

            let _ = sender.send(LlmEvent::status(
                request_id,
                format!("Calling {model_name}..."),
            ));
            let mut stream = agent
                .stream_prompt(prompt)
                .with_history(history)
                .multi_turn(4)
                .await;

            let mut output = String::new();
            let mut updated_history = Vec::new();
            let mut usage = rig::completion::Usage::new();

            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(MultiTurnStreamItem::StreamAssistantItem(
                        StreamedAssistantContent::Text(RigText { text, .. }),
                    )) => {
                        output.push_str(&text);
                        let _ = sender.send(LlmEvent::chunk(request_id, text));
                    }
                    Ok(MultiTurnStreamItem::FinalResponse(final_response)) => {
                        usage = final_response
                            .completion_calls()
                            .last()
                            .map(|call| call.usage)
                            .unwrap_or_else(|| final_response.usage());
                        usage.total_tokens =
                            usage.input_tokens.saturating_add(usage.output_tokens);
                        if let Some(history) = final_response.history() {
                            updated_history = history.to_vec();
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(LlmEvent::error(
                            request_id,
                            format!("Stream error: {error}"),
                        ));
                        return;
                    }
                    _ => {}
                }
            }

            let _ = sender.send(LlmEvent::complete_with_usage(
                request_id,
                updated_history,
                output,
                usage,
            ));
        });
    });
}

fn resolve_chatgpt_model(model: String) -> String {
    if model.is_empty() {
        env::var("LLM_MODEL").unwrap_or_else(|_| "openai/gpt-5.5".to_string())
    } else if model.contains('/') {
        model
    } else {
        format!("openai/{model}")
    }
}

fn chatgpt_token_dir() -> PathBuf {
    if let Ok(dir) = env::var("TUICORE_CHATGPT_TOKEN_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(dir) = env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(dir).join("tuicore").join("rig-chatgpt");
    }
    if let Ok(dir) = env::var("APPDATA") {
        return PathBuf::from(dir).join("tuicore").join("rig-chatgpt");
    }
    if let Ok(home) = env::var("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("tuicore")
            .join("rig-chatgpt");
    }
    env::temp_dir().join("tuicore").join("rig-chatgpt")
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
    .search_mode(DropdownSearchMode::Contains)
    .commit_mode(DropdownCommitMode::Immediate)
    .centered(true)
    .tab_stop(false)
    .max_popup_height(12)
    .on_select(|ids| {
        if let Some(name) = ids.first() {
            set_theme(Theme::named(*name));
        }
    })
}

fn measured_width<M, N>(node: &N) -> u16
where
    N: TuiNode<M>,
{
    node.measure(LayoutProposal::unbounded()).preferred.width
}

fn status_segment_width(label: &str) -> u16 {
    line_width(&Line::from(format!(" {label} "))).min(u16::MAX as usize) as u16
}

const STATUS_ACTION_TAIL_WIDTH: u16 = 1;

fn status_action_tail() -> Line<'static> {
    Line::from(Span::styled("", Style::default().fg(theme().border_fg())))
}

fn status_segment_line(
    label_spans: Vec<Span<'static>>,
    focused: bool,
    segment_bg: Color,
    separator_bg: Option<Color>,
) -> Line<'static> {
    let theme = theme();
    let background = if focused {
        theme.highlight_bg()
    } else {
        segment_bg
    };
    let mut separator_style = Style::default().fg(background);
    if let Some(separator_bg) = separator_bg {
        separator_style = separator_style.bg(separator_bg);
    }
    let mut spans = vec![
        Span::styled("", separator_style),
        Span::styled(" ", status_segment_text_style(focused, segment_bg)),
    ];
    spans.extend(label_spans);
    spans.push(Span::styled(
        " ",
        status_segment_text_style(focused, segment_bg),
    ));
    Line::from(spans)
}

fn status_segment_text_style(focused: bool, segment_bg: Color) -> Style {
    let theme = theme();
    if focused {
        Style::default()
            .fg(theme.highlight_fg())
            .bg(theme.highlight_bg())
    } else {
        Style::default().fg(theme.background_bg()).bg(segment_bg)
    }
}

fn centered_field_area(area: Rect, width: u16) -> Rect {
    let width = width.min(area.width);
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(1) / 2,
        width,
        1,
    )
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
    use ratatui::layout::Rect;

    use super::*;
    use crate::components::weather_provider::WeatherFetchError;
    use crate::{FocusId, FocusRequest, Key, Propagation, TreePath, TuiEvent};

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
    fn default_status_bar_enables_builtin_weather_provider() {
        let status = StatusBar::<()>::new();

        assert!(status.weather_provider.is_enabled());
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
        let report = open_meteo_test_report();
        assert_eq!(
            report
                .raw()
                .lines()
                .filter(|line| line.starts_with('┌'))
                .count(),
            7
        );
        tx.send(Ok(report)).expect("test receiver should be alive");

        let result = status.tick(Duration::from_millis(16), AnimationSettings::default());

        assert!(result.changed);
        assert!(!result.active);
        assert!(status.weather_fetch.is_none());
        assert!(status.weather.label().contains("21(23) °C"));
        assert!(status.weather.label().contains("Sunny"));
    }

    fn open_meteo_test_report() -> WeatherReport {
        let now =
            time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
        let dates = (0..7)
            .map(|offset| {
                now.date()
                    .saturating_add(time::Duration::days(offset))
                    .to_string()
            })
            .collect::<Vec<_>>();
        let hourly_times = dates
            .iter()
            .flat_map(|date| {
                ["00:00", "06:00", "12:00", "18:00"].map(move |hour| format!("{date}T{hour}"))
            })
            .collect::<Vec<_>>();
        let hourly_len = hourly_times.len();
        let json = serde_json::json!({
            "hourly": {
                "time": hourly_times,
                "temperature_2m": vec![21.0; hourly_len],
                "apparent_temperature": vec![23.0; hourly_len],
                "weather_code": vec![0; hourly_len],
                "wind_speed_10m": vec![8.0; hourly_len],
                "precipitation": vec![0.0; hourly_len],
                "precipitation_probability": vec![1.0; hourly_len],
                "visibility": vec![10000.0; hourly_len]
            },
            "daily": {
                "time": dates,
                "weather_code": [0, 0, 0, 0, 0, 0, 0],
                "temperature_2m_max": [24.0, 24.0, 24.0, 24.0, 24.0, 24.0, 24.0],
                "temperature_2m_min": [12.0, 12.0, 12.0, 12.0, 12.0, 12.0, 12.0]
            }
        });
        WeatherReport::from_open_meteo_json("Here", json.to_string())
            .expect("test report should parse")
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
        assert!(!result.active);
        assert!(status.weather_fetch.is_none());
        assert!(status.weather.label().contains("Weather unavailable"));
    }
}
