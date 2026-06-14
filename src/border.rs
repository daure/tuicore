use tuirealm::ratatui::symbols::border::{DOUBLE, PLAIN, ROUNDED, Set, THICK};

use crate::BorderKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorderChars {
    pub top_left: &'static str,
    pub top_right: &'static str,
    pub bottom_left: &'static str,
    pub bottom_right: &'static str,
    pub top_join: &'static str,
    pub bottom_join: &'static str,
    pub left_join: &'static str,
    pub right_join: &'static str,
    pub vertical: &'static str,
    pub horizontal: &'static str,
}

pub fn border_chars(border: BorderKind) -> BorderChars {
    match border {
        BorderKind::Plain => BorderChars {
            top_left: "┌",
            top_right: "┐",
            bottom_left: "└",
            bottom_right: "┘",
            top_join: "┬",
            bottom_join: "┴",
            left_join: "├",
            right_join: "┤",
            vertical: "│",
            horizontal: "─",
        },
        BorderKind::Rounded => BorderChars {
            top_left: "╭",
            top_right: "╮",
            bottom_left: "╰",
            bottom_right: "╯",
            top_join: "┬",
            bottom_join: "┴",
            left_join: "├",
            right_join: "┤",
            vertical: "│",
            horizontal: "─",
        },
        BorderKind::Double => BorderChars {
            top_left: "╔",
            top_right: "╗",
            bottom_left: "╚",
            bottom_right: "╝",
            top_join: "╦",
            bottom_join: "╩",
            left_join: "╠",
            right_join: "╣",
            vertical: "║",
            horizontal: "═",
        },
        BorderKind::Thick => BorderChars {
            top_left: "┏",
            top_right: "┓",
            bottom_left: "┗",
            bottom_right: "┛",
            top_join: "┳",
            bottom_join: "┻",
            left_join: "┣",
            right_join: "┫",
            vertical: "┃",
            horizontal: "━",
        },
    }
}

pub fn border_set(border: BorderKind) -> Set<'static> {
    match border {
        BorderKind::Plain => PLAIN,
        BorderKind::Rounded => ROUNDED,
        BorderKind::Double => DOUBLE,
        BorderKind::Thick => THICK,
    }
}
