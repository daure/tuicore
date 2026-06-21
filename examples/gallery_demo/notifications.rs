use ratatui::layout::{Constraint, Direction, Layout, Rect};
use tuicore::{Button, ChildKey, EventRoute, Notification};

use crate::Msg;

pub(crate) fn notification_buttons() -> [Button<Msg>; 4] {
    [
        Button::new("Info")
            .hotkey("ni")
            .on_press(|| Msg::NotificationTriggered(0)),
        Button::new("Success")
            .hotkey("ns")
            .on_press(|| Msg::NotificationTriggered(1)),
        Button::new("Warning")
            .hotkey("nw")
            .on_press(|| Msg::NotificationTriggered(2)),
        Button::new("Error")
            .hotkey("ne")
            .on_press(|| Msg::NotificationTriggered(3)),
    ]
}

pub(crate) fn notification_for_index(index: usize) -> Notification {
    match index {
        0 => Notification::info("One line", "Background sync completed."),
        1 => Notification::success(
            "Two lines",
            "Profile changes persisted and the account cache refreshed across tabs.",
        ),
        2 => Notification::warning(
            "Three lines",
            "Deploy queue is running behind because two checks are still waiting for runners to become available during release verification.",
        ),
        _ => Notification::error(
            "Ellipsis",
            "Release failed health checks after the canary reported elevated latency, repeated timeout spikes, missing telemetry, rollback safeguards, database migration warnings, and deployment locks that need operator review before continuing.",
        ),
    }
}

pub(crate) fn notification_trigger_layout(area: Rect) -> [Rect; 3] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(7),
            Constraint::Fill(1),
        ])
        .areas(area)
}

pub(crate) fn notification_button_areas(area: Rect) -> [Rect; 4] {
    let [top, bottom] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(3)])
        .areas(area);
    let [info, success] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .areas(top);
    let [warning, error] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .areas(bottom);
    [info, success, warning, error]
}

pub(crate) fn notification_button_child_key(index: usize) -> ChildKey {
    ChildKey::new(format!("notification-button-{index}"))
}

pub(crate) fn notification_button_index(key: &ChildKey) -> Option<usize> {
    key.as_str()
        .strip_prefix("notification-button-")?
        .parse()
        .ok()
        .filter(|index| *index < 4)
}

pub(crate) fn notification_button_child_route(route: &EventRoute) -> Option<(usize, EventRoute)> {
    let first = route.path.first()?;
    let index = notification_button_index(first)?;
    Some((index, EventRoute::new(route.path.without_first())))
}
