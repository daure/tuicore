use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use super::{
    ActivationMode, CheckState, DataView, DataViewEvent, DataViewOutcome, DataViewTypedEvent,
    SelectionGlyphs, SelectionMode, SelectionPropagation, SelectionTrigger,
};

impl<T, Id> DataView<T, Id>
where
    Id: Clone + Eq + Hash,
{
    pub fn selection_mode(mut self, mode: SelectionMode) -> Self {
        self.selection_mode = mode;
        self.normalize_selection();
        self
    }

    pub fn selection_trigger(mut self, trigger: SelectionTrigger) -> Self {
        self.selection_trigger = trigger;
        self
    }

    pub fn selection_propagation(mut self, propagation: SelectionPropagation) -> Self {
        self.selection_propagation = propagation;
        self.normalize_selection();
        self
    }

    pub fn selection_glyphs(mut self, glyphs: SelectionGlyphs) -> Self {
        self.selection_glyphs = glyphs;
        self
    }

    pub fn selected(mut self, ids: impl IntoIterator<Item = Id>) -> Self {
        if self.selection_mode == SelectionMode::None {
            self.selected.clear();
            return self;
        }
        self.selected = self.known_ids(ids.into_iter().collect());
        self.normalize_selection();
        self
    }

    /// Returns directly selected row IDs in source order.
    ///
    /// With cascade selection, non-leaf rows can render as checked because all
    /// descendants are selected even when the parent ID is not stored here.
    /// Selecting or toggling a parent stores the parent plus its descendants.
    pub fn selected_ids(&self) -> Vec<Id> {
        if self.selection_mode == SelectionMode::None {
            return Vec::new();
        }
        self.ordered_selected_ids(&self.selected)
    }

    pub fn selected_id(&self) -> Option<Id> {
        self.selected_ids().into_iter().next()
    }

    /// Returns whether the row ID is directly selected.
    ///
    /// Use [`Self::check_state`] for cascade-derived parent check state.
    pub fn is_selected(&self, id: &Id) -> bool {
        self.selection_mode != SelectionMode::None
            && self.selected.contains(id)
            && self.contains_row_id(id)
    }

    pub fn select_id(&mut self, id: Id) -> bool {
        self.select_id_internal(id)
    }

    pub fn toggle_selected(&mut self, id: Id) -> bool {
        self.toggle_selected_internal(id)
    }

    pub fn clear_selection(&mut self) -> bool {
        self.replace_selection(HashSet::new())
    }

    /// Returns the visual check state for a row.
    ///
    /// With cascade selection, parent state is derived from descendants and can
    /// be checked or indeterminate independently from direct parent selection.
    pub fn check_state(&self, id: &Id) -> CheckState {
        if self.selection_mode == SelectionMode::None {
            return CheckState::Unchecked;
        }
        let descendants = self.selection_descendants_by_id();
        self.check_state_with_descendants(id, &descendants)
    }

    pub(super) fn selection_descendants_by_id(&self) -> HashMap<Id, Vec<Id>> {
        if self.cascades_selection() {
            self.descendant_ids_by_id()
        } else {
            HashMap::new()
        }
    }

    pub(super) fn check_state_with_descendants(
        &self,
        id: &Id,
        descendants_by_id: &HashMap<Id, Vec<Id>>,
    ) -> CheckState {
        if self.selection_mode == SelectionMode::None {
            return CheckState::Unchecked;
        }
        let ids = self.check_state_ids_from_descendants(id, descendants_by_id);
        let checked = ids.iter().filter(|id| self.selected.contains(*id)).count();
        match checked {
            0 => CheckState::Unchecked,
            count if count == ids.len() => CheckState::Checked,
            _ => CheckState::Indeterminate,
        }
    }

    pub(super) fn selection_glyph_with_descendants(
        &self,
        id: &Id,
        descendants_by_id: &HashMap<Id, Vec<Id>>,
    ) -> &'static str {
        match self.check_state_with_descendants(id, descendants_by_id) {
            CheckState::Unchecked => self.selection_glyphs.unchecked,
            CheckState::Checked => self.selection_glyphs.checked,
            CheckState::Indeterminate => self.selection_glyphs.indeterminate,
        }
    }

    pub fn take_last_activated(&mut self) -> Option<DataViewEvent<Id>> {
        self.last_activated
            .take()
            .map(|row_id| DataViewEvent { row_id })
    }

    pub fn take_events(&mut self) -> Vec<DataViewTypedEvent<Id>> {
        self.events.drain(..).collect()
    }

    pub fn drain_events(&mut self) -> Vec<DataViewTypedEvent<Id>> {
        self.take_events()
    }

    pub(super) fn activate_highlighted(&mut self) -> DataViewOutcome {
        let Some(row_id) = self.highlighted_id() else {
            return DataViewOutcome::IDLE;
        };
        let selection_changed = if self.selection_trigger == SelectionTrigger::OnActivate {
            self.apply_activate_selection(row_id.clone())
        } else {
            false
        };
        if self.activation_mode == ActivationMode::Manual {
            return DataViewOutcome {
                handled: true,
                changed: selection_changed,
                active: false,
                activated: false,
            };
        }
        let activated = self.activation_mode == ActivationMode::OnActivateKey;
        if activated {
            self.emit_activation(row_id);
        }
        DataViewOutcome {
            handled: true,
            changed: selection_changed,
            active: false,
            activated,
        }
    }

    pub(super) fn toggle_highlighted_selection(&mut self) -> DataViewOutcome {
        if self.selection_mode == SelectionMode::None {
            return DataViewOutcome::IDLE;
        }
        let Some(row_id) = self.highlighted_id() else {
            return DataViewOutcome::IDLE;
        };
        let changed = self.toggle_selected_internal(row_id);
        DataViewOutcome {
            handled: true,
            changed,
            active: false,
            activated: false,
        }
    }

    pub(super) fn emit_activation(&mut self, row_id: Id) {
        self.last_activated = Some(row_id.clone());
        self.events.push(DataViewTypedEvent::Activated { row_id });
    }

    pub(super) fn select_id_internal(&mut self, id: Id) -> bool {
        if !self.contains_row_id(&id) {
            return false;
        }
        match self.selection_mode {
            SelectionMode::None => false,
            SelectionMode::Single => self.replace_selection([id].into_iter().collect()),
            SelectionMode::Multi => {
                let mut next = self.selected.clone();
                for id in self.selection_group(id) {
                    next.insert(id);
                }
                self.replace_selection(next)
            }
        }
    }

    fn apply_activate_selection(&mut self, id: Id) -> bool {
        match self.selection_mode {
            SelectionMode::None => false,
            SelectionMode::Single => self.select_id_internal(id),
            SelectionMode::Multi => self.toggle_selected_internal(id),
        }
    }

    pub(super) fn toggle_selected_internal(&mut self, id: Id) -> bool {
        if !self.contains_row_id(&id) {
            return false;
        }
        match self.selection_mode {
            SelectionMode::None => false,
            SelectionMode::Single => {
                if self.selected.len() == 1 && self.selected.contains(&id) {
                    self.replace_selection(HashSet::new())
                } else {
                    self.replace_selection([id].into_iter().collect())
                }
            }
            SelectionMode::Multi => {
                let group = self.selection_group(id.clone());
                let checked = self.check_state(&id) == CheckState::Checked;
                let mut next = self.selected.clone();
                for id in group {
                    if checked {
                        next.remove(&id);
                    } else {
                        next.insert(id);
                    }
                }
                self.replace_selection(next)
            }
        }
    }

    fn selection_group(&self, id: Id) -> Vec<Id> {
        if !self.cascades_selection() {
            return vec![id];
        }
        let mut group = Vec::with_capacity(1);
        group.push(id.clone());
        group.extend(self.descendant_ids(&id));
        group
    }

    pub(super) fn replace_selection(&mut self, next: HashSet<Id>) -> bool {
        if self.selection_mode == SelectionMode::None {
            let changed = !self.selected.is_empty();
            self.selected.clear();
            return changed;
        }

        let current = self.known_ids(self.selected.clone());
        let next = self.known_ids(next);
        if current == next {
            self.selected = current;
            return false;
        }

        let selected = self.ordered_selected_ids(&next);
        let added = self.ordered_diff(&next, &current);
        let removed = self.ordered_diff(&current, &next);
        self.selected = next;
        self.events.push(DataViewTypedEvent::SelectionChanged {
            selected,
            added,
            removed,
        });
        true
    }

    fn ordered_selected_ids(&self, selected: &HashSet<Id>) -> Vec<Id> {
        self.row_ids()
            .into_iter()
            .filter(|id| selected.contains(id))
            .collect()
    }

    fn ordered_diff(&self, included: &HashSet<Id>, excluded: &HashSet<Id>) -> Vec<Id> {
        self.row_ids()
            .into_iter()
            .filter(|id| included.contains(id) && !excluded.contains(id))
            .collect()
    }

    pub(super) fn normalize_selection(&mut self) {
        match self.selection_mode {
            SelectionMode::None => self.selected.clear(),
            SelectionMode::Single => {
                if let Some(first) = self.selected_ids().into_iter().next() {
                    self.selected = [first].into_iter().collect();
                } else {
                    self.selected.clear();
                }
            }
            SelectionMode::Multi => {
                self.selected = self.normalized_multi_selection(self.selected.clone());
            }
        }
    }

    fn normalized_multi_selection(&self, selected: HashSet<Id>) -> HashSet<Id> {
        let selected = self.known_ids(selected);
        if !self.cascades_selection() {
            return selected;
        }

        selected
            .into_iter()
            .flat_map(|id| self.selection_group(id))
            .collect::<HashSet<_>>()
    }

    fn known_ids(&self, ids: HashSet<Id>) -> HashSet<Id> {
        let known = self.row_ids().into_iter().collect::<HashSet<_>>();
        ids.into_iter()
            .filter(|id| known.contains(id))
            .collect::<HashSet<_>>()
    }

    pub(super) fn contains_row_id(&self, id: &Id) -> bool {
        self.rows.iter().any(|row| (self.row_id)(row) == *id)
    }

    fn check_state_ids_from_descendants(
        &self,
        id: &Id,
        descendants_by_id: &HashMap<Id, Vec<Id>>,
    ) -> Vec<Id> {
        if !self.cascades_selection() {
            return vec![id.clone()];
        }

        let descendants = descendants_by_id.get(id).cloned().unwrap_or_default();
        if descendants.is_empty() {
            vec![id.clone()]
        } else {
            descendants
                .into_iter()
                .filter(|descendant| {
                    descendants_by_id
                        .get(descendant)
                        .is_none_or(|children| children.is_empty())
                })
                .collect()
        }
    }

    fn cascades_selection(&self) -> bool {
        self.selection_mode == SelectionMode::Multi
            && self.selection_propagation == SelectionPropagation::CascadeDescendants
    }
}
