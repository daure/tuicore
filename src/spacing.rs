#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Padding {
    pub left: u16,
    pub right: u16,
    pub top: u16,
    pub bottom: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Gap {
    pub row: u16,
    pub column: u16,
}

impl Padding {
    pub fn all(value: u16) -> Self {
        Self {
            left: value,
            right: value,
            top: value,
            bottom: value,
        }
    }

    pub fn horizontal_vertical(horizontal: u16, vertical: u16) -> Self {
        Self {
            left: horizontal,
            right: horizontal,
            top: vertical,
            bottom: vertical,
        }
    }
}

impl Gap {
    pub fn new(row: u16, column: u16) -> Self {
        Self { row, column }
    }

    pub fn all(value: u16) -> Self {
        Self {
            row: value,
            column: value,
        }
    }
}
