use std::hash::Hash;
use std::time::Duration;

use ratatui::layout::Rect;

use super::{
    DataView, DataViewChoice, DataViewInteraction, DataViewOutcome, Dropdown, DropdownCommitMode,
    DropdownLabelPosition, DropdownOutcome, DropdownSearchMode, DropdownVariant, EMPTY_CHOICE_ID,
    ScrollOutcomeExt, column_key, dropdown_outcome,
};
use crate::AnimationSettings;

impl<T, Id> DataView<T, Id>
where
    Id: Clone + Eq + Hash,
{
    pub fn unique_filter_values(&self, column_id: &str) -> Vec<String> {
        self.filter_values(column_id)
    }

    pub fn filterable_columns(&self) -> Vec<&str> {
        self.columns
            .iter()
            .filter(|column| column.filter_key.is_some())
            .map(|column| column.id.as_str())
            .collect()
    }

    pub(crate) fn active_filter_value(&self, column_id: &str) -> Option<&str> {
        self.transform_state
            .filters
            .iter()
            .find(|filter| filter.column_id == column_id)
            .map(|filter| filter.value.as_str())
    }

    pub(crate) fn filter_active(&self, column_id: &str) -> bool {
        self.active_filter_value(column_id).is_some()
    }

    pub(crate) fn filter_column_id_for_key(&self, key: char) -> Option<String> {
        self.columns.iter().enumerate().find_map(|(index, column)| {
            (column.filter_key.is_some()
                && column_key(index).is_some_and(|candidate| candidate == key.to_ascii_lowercase()))
            .then(|| column.id.clone())
        })
    }

    pub(super) fn open_filter_values(&mut self, column_id: String) {
        let selected = self
            .active_filter_value(&column_id)
            .map(ToOwned::to_owned)
            .unwrap_or_default();
        let mut dropdown = self.choice_dropdown(
            self.filter_popup_title(&column_id),
            self.filter_choices(&column_id),
            selected,
        );
        dropdown.open();
        self.filter_dropdown = Some(Box::new(dropdown));
        self.interaction = DataViewInteraction::FilterValues { column_id };
        self.header_pick_elapsed = Duration::ZERO;
    }

    fn choice_dropdown(
        &self,
        label: impl Into<String>,
        choices: Vec<DataViewChoice>,
        selected: String,
    ) -> super::ChoiceDropdown {
        Dropdown::single(
            choices,
            |choice: &DataViewChoice| choice.id.clone(),
            |choice: &DataViewChoice| choice.label.clone(),
        )
        .selected([selected])
        .label(label)
        .label_position(DropdownLabelPosition::Inline)
        .variant(DropdownVariant::Filled)
        .centered(true)
        .show_field_when_open(false)
        .max_popup_height(12)
        .search_mode(DropdownSearchMode::Contains)
        .commit_mode(DropdownCommitMode::Explicit)
    }

    fn filter_popup_title(&self, column_id: &str) -> String {
        self.columns
            .iter()
            .find(|column| column.id == column_id)
            .map(|column| format!("Filter {}", column.header))
            .unwrap_or_else(|| String::from("Filter"))
    }

    fn filter_choices(&self, column_id: &str) -> Vec<DataViewChoice> {
        let mut choices = vec![DataViewChoice {
            id: String::from(EMPTY_CHOICE_ID),
            label: String::from("All"),
        }];
        choices.extend(
            self.filter_values(column_id)
                .into_iter()
                .map(|value| DataViewChoice {
                    id: value.clone(),
                    label: value,
                }),
        );
        choices
    }

    pub(super) fn apply_filter_dropdown_outcome(
        &mut self,
        column_id: &str,
        outcome: DropdownOutcome,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        if outcome.committed {
            let selected = self
                .filter_dropdown
                .as_ref()
                .and_then(|dropdown| dropdown.selected_id())
                .unwrap_or_default();
            let transform = if selected.is_empty() {
                self.clear_filter(column_id)
            } else {
                self.set_filter(column_id, selected)
            };
            self.close_choice_dropdowns();
            return self.transform_dropdown_outcome(transform, area, settings);
        }
        if outcome.closed || outcome.canceled {
            self.close_choice_dropdowns();
            return DataViewOutcome::CHANGED;
        }
        dropdown_outcome(outcome)
    }

    pub(super) fn transform_dropdown_outcome(
        &mut self,
        outcome: DataViewOutcome,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        if outcome.changed {
            let mut scrolled = self
                .ensure_highlight_visible(area, settings)
                .into_data_view_outcome(true, true);
            scrolled.activated = outcome.activated;
            scrolled
        } else {
            DataViewOutcome::HANDLED
        }
    }

    pub(super) fn close_choice_dropdowns(&mut self) {
        self.interaction = DataViewInteraction::Grid;
        self.filter_dropdown = None;
    }

    pub(crate) fn filter_values(&self, column_id: &str) -> Vec<String> {
        let Some(column) = self.columns.iter().find(|column| column.id == column_id) else {
            return Vec::new();
        };
        let Some(filter_key) = column.filter_key.as_deref() else {
            return Vec::new();
        };
        let mut values = self
            .base_row_refs()
            .into_iter()
            .map(filter_key)
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        values.sort();
        values.dedup();
        values
    }

    pub(super) fn filter_controls_enabled(&self) -> bool {
        self.filter_controls && self.headers && self.columns.len() > 1
    }
}
