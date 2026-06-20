use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::{BorderKind, Theme, border_chars, preset, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeparatorColorRole {
    Border,
    Muted,
    Subtle,
    Accent,
}

impl Default for SeparatorColorRole {
    fn default() -> Self {
        Self::Border
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Separator {
    kind: Option<BorderKind>,
    role: SeparatorColorRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridSeparatorAxes {
    columns: bool,
    rows: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridSeparators {
    separator: Separator,
    axes: GridSeparatorAxes,
}

impl Separator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn kind(mut self, kind: BorderKind) -> Self {
        self.kind = Some(kind);
        self
    }

    pub fn role(mut self, role: SeparatorColorRole) -> Self {
        self.role = role;
        self
    }

    pub(crate) fn resolved_kind(self) -> BorderKind {
        self.kind.unwrap_or_else(|| preset().border())
    }

    pub(crate) fn style(self) -> Style {
        Style::default().fg(self.role.color(&theme()))
    }
}

impl SeparatorColorRole {
    fn color(self, theme: &Theme) -> ratatui::style::Color {
        match self {
            Self::Border => theme.border_fg(),
            Self::Muted => theme.muted_fg(),
            Self::Subtle => theme.subtle_fg(),
            Self::Accent => theme.accent_fg(),
        }
    }
}

impl GridSeparatorAxes {
    pub fn columns() -> Self {
        Self {
            columns: true,
            rows: false,
        }
    }

    pub fn rows() -> Self {
        Self {
            columns: false,
            rows: true,
        }
    }

    pub fn both() -> Self {
        Self {
            columns: true,
            rows: true,
        }
    }

    pub fn has_columns(self) -> bool {
        self.columns
    }

    pub fn has_rows(self) -> bool {
        self.rows
    }
}

impl GridSeparators {
    pub fn columns(separator: Separator) -> Self {
        Self {
            separator,
            axes: GridSeparatorAxes::columns(),
        }
    }

    pub fn rows(separator: Separator) -> Self {
        Self {
            separator,
            axes: GridSeparatorAxes::rows(),
        }
    }

    pub fn both(separator: Separator) -> Self {
        Self {
            separator,
            axes: GridSeparatorAxes::both(),
        }
    }

    pub fn separator(self) -> Separator {
        self.separator
    }

    pub fn axes(self) -> GridSeparatorAxes {
        self.axes
    }
}

pub(crate) fn separator_cell(enabled: bool, gaps: usize) -> u16 {
    (enabled && gaps > 0) as u16
}

pub(crate) fn separator_slots(enabled: bool, count: usize) -> u16 {
    if enabled {
        count.saturating_sub(1).min(usize::from(u16::MAX)) as u16
    } else {
        0
    }
}

pub(crate) fn draw_vertical(frame: &mut Frame, rect: Rect, separator: Separator) {
    draw_line(
        frame,
        rect,
        border_chars(separator.resolved_kind()).vertical,
        separator.style(),
    );
}

pub(crate) fn draw_horizontal(frame: &mut Frame, rect: Rect, separator: Separator) {
    draw_line(
        frame,
        rect,
        border_chars(separator.resolved_kind()).horizontal,
        separator.style(),
    );
}

#[allow(dead_code)]
pub(crate) fn draw_cross(frame: &mut Frame, x: u16, y: u16, separator: Separator) {
    frame
        .buffer_mut()
        .set_string(x, y, cross(separator.resolved_kind()), separator.style());
}

pub(crate) fn patch_border_joins(
    frame: &mut Frame,
    outer: Rect,
    inner: Rect,
    kind: BorderKind,
    style: Style,
) {
    if outer.width < 2 || outer.height < 2 || inner.width == 0 || inner.height == 0 {
        return;
    }
    let chars = border_chars(kind);
    let bottom_y = outer.bottom().saturating_sub(1);
    let right_x = outer.right().saturating_sub(1);

    for x in inner.x..inner.right() {
        if cell_symbol_is(frame, x, inner.y, chars.vertical) {
            frame
                .buffer_mut()
                .set_string(x, outer.y, chars.top_join, style);
        }
        let y = inner.bottom().saturating_sub(1);
        if cell_symbol_is(frame, x, y, chars.vertical) {
            frame
                .buffer_mut()
                .set_string(x, bottom_y, chars.bottom_join, style);
        }
    }

    for y in inner.y..inner.bottom() {
        if cell_symbol_is(frame, inner.x, y, chars.horizontal) {
            frame
                .buffer_mut()
                .set_string(outer.x, y, chars.left_join, style);
        }
        let x = inner.right().saturating_sub(1);
        if cell_symbol_is(frame, x, y, chars.horizontal) {
            frame
                .buffer_mut()
                .set_string(right_x, y, chars.right_join, style);
        }
    }
}

fn cell_symbol_is(frame: &mut Frame, x: u16, y: u16, expected: &str) -> bool {
    frame.buffer_mut()[(x, y)].symbol() == expected
}

fn draw_line(frame: &mut Frame, rect: Rect, symbol: &str, style: Style) {
    for y in rect.y..rect.y.saturating_add(rect.height) {
        for x in rect.x..rect.x.saturating_add(rect.width) {
            frame.buffer_mut().set_string(x, y, symbol, style);
        }
    }
}

pub(crate) fn cross(kind: BorderKind) -> &'static str {
    match kind {
        BorderKind::Plain | BorderKind::Rounded => "┼",
        BorderKind::Double => "╬",
        BorderKind::Thick => "╋",
    }
}
