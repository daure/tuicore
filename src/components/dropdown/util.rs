use ratatui::layout::Rect;
use ratatui::symbols::border::Set;
use ratatui::text::Line;

use crate::event::{Key, KeyEvent};
use crate::{KeySpec, border_chars, line_width};

pub(super) fn connected_popup_border_set(border: crate::BorderKind) -> Set<'static> {
    let chars = border_chars(border);
    Set {
        top_left: chars.left_join,
        top_right: chars.right_join,
        bottom_left: chars.bottom_left,
        bottom_right: chars.bottom_right,
        vertical_left: chars.vertical,
        vertical_right: chars.vertical,
        horizontal_top: chars.horizontal,
        horizontal_bottom: chars.horizontal,
    }
}

pub(super) fn clip_rect(area: Rect, bounds: Rect) -> Rect {
    let x = area.x.max(bounds.x);
    let y = area.y.max(bounds.y);
    let right = area
        .x
        .saturating_add(area.width)
        .min(bounds.x.saturating_add(bounds.width));
    let bottom = area
        .y
        .saturating_add(area.height)
        .min(bounds.y.saturating_add(bounds.height));
    Rect::new(x, y, right.saturating_sub(x), bottom.saturating_sub(y))
}

pub(super) fn bounded_title(title: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    let mut value = format!(" {title} ");
    if line_width(&Line::from(value.as_str())) > max_width {
        value = truncate_cells(&value, max_width);
    }
    value
}

fn truncate_cells(value: &str, max_width: usize) -> String {
    let mut width = 0;
    let mut truncated = String::new();

    for ch in value.chars() {
        let ch_width = char_width(ch);
        if ch_width > 0 && width + ch_width > max_width {
            break;
        }
        width += ch_width;
        truncated.push(ch);
    }

    truncated
}

fn char_width(ch: char) -> usize {
    let mut value = String::new();
    value.push(ch);
    line_width(&Line::from(value))
}

pub(super) fn keys_match(hotkey: KeyEvent, key: KeyEvent) -> bool {
    if hotkey.modifiers != key.modifiers {
        return false;
    }
    match (hotkey.code, key.code) {
        (Key::Char(a), Key::Char(b)) => a.to_ascii_lowercase() == b.to_ascii_lowercase(),
        (a, b) => a == b,
    }
}

pub(super) fn matches_any(bindings: &[KeySpec], key: KeyEvent) -> bool {
    bindings.iter().any(|binding| binding.matches(key))
}

pub(super) fn hotkey_matches_sequence(hotkey: &str, sequence: &str) -> bool {
    crate::hotkey::normalize_hotkey(hotkey) == crate::hotkey::normalize_hotkey(sequence)
}
