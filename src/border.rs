use ratatui::symbols::border::{DOUBLE, PLAIN, ROUNDED, Set, THICK};

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
        BorderKind::RoundedDashed => BorderChars {
            top_left: "╭",
            top_right: "╮",
            bottom_left: "╰",
            bottom_right: "╯",
            top_join: "┬",
            bottom_join: "┴",
            left_join: "├",
            right_join: "┤",
            vertical: "╎",
            horizontal: "-",
        },
        BorderKind::AsciiDashed => BorderChars {
            top_left: "┌",
            top_right: "┐",
            bottom_left: "└",
            bottom_right: "┘",
            top_join: "┬",
            bottom_join: "┴",
            left_join: "├",
            right_join: "┤",
            vertical: "╎",
            horizontal: "-",
        },
    }
}

pub fn border_set(border: BorderKind) -> Set<'static> {
    match border {
        BorderKind::Plain => PLAIN,
        BorderKind::Rounded => ROUNDED,
        BorderKind::Double => DOUBLE,
        BorderKind::Thick => THICK,
        BorderKind::RoundedDashed => Set {
            top_left: "╭",
            top_right: "╮",
            bottom_left: "╰",
            bottom_right: "╯",
            vertical_left: "╎",
            vertical_right: "╎",
            horizontal_top: "-",
            horizontal_bottom: "-",
        },
        BorderKind::AsciiDashed => Set {
            top_left: "┌",
            top_right: "┐",
            bottom_left: "└",
            bottom_right: "┘",
            vertical_left: "╎",
            vertical_right: "╎",
            horizontal_top: "-",
            horizontal_bottom: "-",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn border_chars_match_each_border_kind() {
        assert_eq!(border_chars(BorderKind::Plain).top_left, "┌");
        assert_eq!(border_chars(BorderKind::Rounded).top_left, "╭");
        assert_eq!(border_chars(BorderKind::Double).top_left, "╔");
        assert_eq!(border_chars(BorderKind::Thick).top_left, "┏");
        assert_eq!(border_chars(BorderKind::RoundedDashed).top_left, "╭");
        assert_eq!(border_chars(BorderKind::RoundedDashed).bottom_right, "╯");
        assert_eq!(border_chars(BorderKind::RoundedDashed).vertical, "╎");
        assert_eq!(border_chars(BorderKind::RoundedDashed).horizontal, "-");
        assert_eq!(border_set(BorderKind::RoundedDashed).top_left, "╭");
        assert_eq!(border_set(BorderKind::RoundedDashed).horizontal_top, "-");
        assert_eq!(border_chars(BorderKind::AsciiDashed).horizontal, "-");
    }
}
