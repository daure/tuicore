use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub(crate) fn password_input_showcase_layout(area: Rect) -> [Rect; 3] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(11),
            Constraint::Length(1),
            Constraint::Length(2),
        ])
        .areas(area)
}

pub(crate) fn text_input_showcase_layout(area: Rect) -> [Rect; 3] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(11),
            Constraint::Length(1),
            Constraint::Length(5),
        ])
        .areas(area)
}

pub(crate) fn toggle_layout(area: Rect) -> [Rect; 3] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(area)
}

pub(crate) fn button_layout(area: Rect) -> [Rect; 3] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(area)
}

pub(crate) fn textarea_showcase_layout(area: Rect) -> [Rect; 3] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(11),
            Constraint::Length(4),
            Constraint::Length(7),
        ])
        .areas(area)
}

pub(crate) fn typography_showcase_layout(area: Rect) -> [Rect; 5] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(area)
}
