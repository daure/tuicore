use ratatui::layout::{Constraint, Direction, Layout, Rect};
use tuicore::{Flex, FlexItem, Header, Paragraph, ParagraphOverflow};

pub(crate) fn typography_showcase<M: 'static>() -> Flex<M> {
    Flex::column()
        .child(
            "instructions",
            Paragraph::new(
                "Typography renders semantic text primitives.\n\
                 Headers support plain labels and Nerd Font icons.\n\
                 Paragraphs can wrap, clip, or ellipsize overflowing copy.",
            )
            .wrap(false),
            FlexItem::fit_content(),
        )
        .child("spacing", Paragraph::new(""), FlexItem::fit_content())
        .child(
            "plain-header",
            Header::new("Release Notes"),
            FlexItem::fit_content(),
        )
        .child(
            "icon-header",
            Header::new("Settings").icon(""),
            FlexItem::fit_content(),
        )
        .child(
            "paragraph-label",
            Paragraph::new("Paragraph with ellipsis:").wrap(false),
            FlexItem::fit_content(),
        )
        .child(
            "paragraph",
            Paragraph::new(
                "Paragraphs render wrapped body copy for explanatory text, help panels, and quiet content blocks. This one is intentionally longer than its preview box so the ellipsis overflow behavior is visible without needing a separate gallery entry.",
            )
            .overflow(ParagraphOverflow::Ellipsis)
            .max_lines(1),
            FlexItem::fit_content(),
        )
}

pub(crate) fn password_input_showcase_layout(area: Rect) -> [Rect; 4] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(11),
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Length(2),
        ])
        .areas(area)
}

pub(crate) fn text_input_showcase_layout(area: Rect) -> [Rect; 4] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(11),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(3),
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

pub(crate) fn chip_layout(area: Rect) -> [Rect; 3] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .areas(area)
}

pub(crate) fn textarea_showcase_layout(area: Rect) -> [Rect; 4] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(11),
            Constraint::Length(5),
            Constraint::Length(6),
            Constraint::Length(4),
        ])
        .areas(area)
}

pub(crate) fn date_time_showcase_layout(area: Rect) -> [Rect; 6] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(13),
            Constraint::Length(1),
            Constraint::Length(11),
            Constraint::Length(3),
            Constraint::Length(2),
        ])
        .areas(area)
}
