use super::{DROPDOWN_FOCUS, INPUT_FOCUS};
use crate::components::{Dropdown, TextInput};

pub(super) enum ListControlInput<M> {
    Text(TextInput<M>),
    Dropdown(Option<Dropdown<(String, String), String>>),
}

impl<M> ListControlInput<M> {
    pub(super) fn value(&self) -> String {
        match self {
            Self::Text(input) => input.current_value().trim().to_string(),
            Self::Dropdown(input) => input
                .as_ref()
                .expect("dropdown input is present")
                .selected_id()
                .unwrap_or_default(),
        }
    }

    pub(super) fn focus_id(&self) -> &'static str {
        match self {
            Self::Text(_) => INPUT_FOCUS,
            Self::Dropdown(input)
                if input.as_ref().expect("dropdown input is present").is_open() =>
            {
                INPUT_FOCUS
            }
            Self::Dropdown(_) => DROPDOWN_FOCUS,
        }
    }

    pub(super) fn reset(&mut self) {
        match self {
            Self::Text(input) => {
                input.set_value("");
                input.set_focused(false);
            }
            Self::Dropdown(input) => {
                let input = input.as_mut().expect("dropdown input is present");
                if input.is_open() {
                    input.cancel();
                }
                input.clear_selection();
            }
        }
    }

    pub(super) fn set_value(&mut self, value: String) {
        match self {
            Self::Text(input) => {
                input.set_value(value);
                input.move_cursor_to_end();
            }
            Self::Dropdown(input) => {
                let dropdown = input.take().expect("dropdown input is present");
                *input = Some(if value.is_empty() {
                    dropdown.selected([])
                } else {
                    dropdown.selected_one(value)
                });
            }
        }
    }

    pub(super) fn set_focused(&mut self, focused: bool) {
        if let Self::Text(input) = self {
            input.set_focused(focused);
            input.set_insert_mode(focused);
        }
    }

    pub(super) fn dropdown_is_open(&self) -> bool {
        matches!(self, Self::Dropdown(input) if input.as_ref().expect("dropdown input is present").is_open())
    }

    pub(super) fn open_dropdown(&mut self) -> bool {
        let Self::Dropdown(input) = self else {
            return false;
        };
        input.as_mut().expect("dropdown input is present").open();
        true
    }

    pub(super) fn is_focused(&self) -> bool {
        match self {
            Self::Text(_) => false,
            Self::Dropdown(input) => input
                .as_ref()
                .expect("dropdown input is present")
                .is_focused(),
        }
    }
}
