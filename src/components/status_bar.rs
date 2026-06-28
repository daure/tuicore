use std::hash::Hash;
use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

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
use super::{Button, Dropdown, Menu, MenuItem, MenuPopupDirection};
use crate::{
    Animated, AnimationSettings, ChildKey, EventCtx, EventOutcome, EventRoute, FocusCtx,
    FocusTarget, LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint, Theme, ThemeName,
    TickResult, TuiEvent, TuiNode, hotkey_underline_style, line_width, set_theme, theme,
};
use crate::{KeyEvent, KeySpec};

const MENU_ICON: &str = "󰍜";
const AI_ICON: &str = "";
const DEFAULT_MENU_HOTKEY: &str = "`";
const DEFAULT_AI_HOTKEY: &str = "'";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatusBarMenuItem {
    Theme,
    WeatherForecast,
    StoreView,
}

impl StatusBarMenuItem {
    fn label(self) -> &'static str {
        match self {
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
    weather: WeatherIndicator<M>,
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
            weather: WeatherIndicator::new().tab_stop(false),
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
        self.weather = self.weather.report(report);
        self
    }

    pub fn set_weather_report(&mut self, report: WeatherReport) {
        self.weather.set_report(report);
    }

    pub fn weather_refresh_needed(&self) -> bool {
        self.weather.refresh_needed()
    }

    pub fn on_ai_open(mut self, handler: impl Fn() -> M + 'static) -> Self {
        self.ai = self.ai.on_press(handler);
        self
    }

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
            StatusBarMenuItem::Theme => {
                self.theme_dropdown.open_with_context(ctx);
                ctx.stop_propagation();
            }
            StatusBarMenuItem::WeatherForecast => {
                if let Some(on_weather_open) = &self.on_weather_open {
                    ctx.emit(on_weather_open());
                }
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
            return self.ai.dispatch_event(&route, event, ctx);
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

        EventOutcome::Ignored
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        if status_menu_hotkey(event, &self.keybindings) {
            return self.toggle_menu(ctx);
        }
        if status_ai_hotkey(event, &self.keybindings) {
            return self.ai.event(event, ctx);
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
        }
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(&mut self.menu_trigger, dt, settings)
            .merge(Animated::tick(&mut self.ai, dt, settings))
            .merge(<WeatherIndicator<M> as TuiNode<M>>::tick(
                &mut self.weather,
                dt,
                settings,
            ))
            .merge(<DateTimeIndicator<M> as TuiNode<M>>::tick(
                &mut self.time,
                dt,
                settings,
            ))
            .merge(Animated::tick(&mut self.menu, dt, settings))
            .merge(Animated::tick(&mut self.theme_dropdown, dt, settings))
    }
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

fn status_bar_weather_key() -> ChildKey {
    ChildKey::new("status-weather")
}

fn status_bar_time_key() -> ChildKey {
    ChildKey::new("status-time")
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;

    use super::*;
    use crate::{FocusId, FocusRequest, Propagation, TreePath};

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
}
