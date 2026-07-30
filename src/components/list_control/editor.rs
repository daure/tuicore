use std::hash::Hash;

use super::{DATA_FOCUS, DATA_SLOT, ListControl, ListControlEvent};
use crate::{AnimationSettings, EventCtx, EventRoute};

impl<T, Id, M: 'static> ListControl<T, Id, M>
where
    T: 'static,
    Id: Clone + Eq + Hash,
{
    pub(super) fn editor_active(&self) -> bool {
        self.adding || self.editing.is_some()
    }

    pub(super) fn begin_add(&mut self, parent_id: Option<Id>) {
        self.adding = true;
        self.adding_parent = Some(parent_id);
        self.editing = None;
        self.active_field = 0;
        for (index, input) in self.inputs.iter_mut().enumerate() {
            input.reset();
            input.set_focused(index == 0);
        }
        self.inputs[0].open_dropdown();
        self.data_view.set_focused(false);
    }

    pub(super) fn begin_add_child(&mut self) -> bool {
        if !self.data_view.tree_is_mutable() {
            return false;
        }
        let Some(parent_id) = self.data_view.highlighted_id() else {
            return false;
        };
        self.begin_add(Some(parent_id));
        true
    }

    pub(super) fn begin_add_sibling(&mut self) {
        let parent_id = self
            .data_view
            .highlighted_id()
            .and_then(|id| self.data_view.tree_parent_id(&id))
            .unwrap_or(None);
        self.begin_add(parent_id);
    }

    pub(super) fn begin_edit(&mut self) -> bool {
        let Some(editable) = &self.editable else {
            return false;
        };
        let Some(row_id) = self.data_view.highlighted_id() else {
            return false;
        };
        let row = self
            .data_view
            .rows()
            .iter()
            .find(|row| self.data_view.row_id(row) == row_id)
            .expect("highlighted row exists");
        let values = (editable.getter)(row);
        assert_eq!(
            values.len(),
            self.inputs.len(),
            "ListControl editable getter must return one value per field"
        );
        self.adding = false;
        self.adding_parent = None;
        self.editing = Some(row_id);
        self.active_field = 0;
        for (index, (input, value)) in self.inputs.iter_mut().zip(values).enumerate() {
            input.reset();
            input.set_value(value);
            input.set_focused(index == 0);
        }
        self.inputs[0].open_dropdown();
        self.data_view.set_focused(false);
        true
    }

    pub(super) fn cancel_editor(&mut self, focus_data: bool) {
        if self.adding {
            self.events.push(ListControlEvent::AddCancelled);
        } else if let Some(row_id) = self.editing.take() {
            self.events.push(ListControlEvent::EditCancelled { row_id });
        }
        self.finish_editor(focus_data);
    }

    fn finish_editor(&mut self, focus_data: bool) {
        self.adding = false;
        self.adding_parent = None;
        self.editing = None;
        self.active_field = 0;
        for input in &mut self.inputs {
            input.reset();
        }
        self.data_view.set_focused(focus_data);
    }

    fn values(&self) -> Vec<String> {
        self.inputs.iter().map(|input| input.value()).collect()
    }

    fn submit(&mut self, settings: AnimationSettings) -> bool {
        let values = self.values();
        if values
            .iter()
            .zip(&self.required_fields)
            .any(|(value, required)| *required && value.is_empty())
        {
            return false;
        }
        if self.adding {
            let mut row = (self.creator)(values, self.data_view.rows());
            let parent_id = self.adding_parent.clone().unwrap_or(None);
            self.data_view
                .set_new_row_parent(&mut row, parent_id.clone());
            let row_id = self.data_view.row_id(&row);
            if let Some(parent_id) = parent_id.clone() {
                self.data_view.expand_tree_row(parent_id);
            }
            self.data_view.push_row(row);
            if self.data_view.highlighted_id().as_ref() == Some(&row_id) {
                self.data_view.reveal_highlighted_with_settings(settings);
            }
            if let Some(parent_id) = parent_id {
                self.events
                    .push(ListControlEvent::AddedChild { row_id, parent_id });
            } else {
                self.events.push(ListControlEvent::Added { row_id });
            }
        } else {
            let row_id = self.editing.clone().expect("editing row exists");
            let editable = self.editable.as_ref().expect("editable mapping exists");
            if self
                .data_view
                .update_row(&row_id, |row| (editable.mutator)(row, values))
                .is_none()
            {
                self.events.push(ListControlEvent::EditCancelled { row_id });
                self.finish_editor(true);
                return true;
            }
            self.events.push(ListControlEvent::Edited { row_id });
        }
        self.finish_editor(true);
        true
    }

    pub(super) fn advance_field(&mut self, route: &EventRoute, ctx: &mut EventCtx<M>) -> bool {
        if self.required_fields[self.active_field]
            && self.inputs[self.active_field].value().is_empty()
        {
            return false;
        }
        if self.active_field + 1 == self.inputs.len() {
            return self.submit(ctx.animation());
        }
        self.inputs[self.active_field].set_focused(false);
        self.active_field += 1;
        self.inputs[self.active_field].set_focused(true);
        self.inputs[self.active_field].open_dropdown();
        let slot = Self::input_slot(self.active_field);
        Self::focus_child(
            ctx,
            route,
            slot.as_str(),
            self.inputs[self.active_field].focus_id(),
        );
        ctx.request_layout();
        true
    }

    pub(super) fn restore_data_focus(&mut self, route: &EventRoute, ctx: &mut EventCtx<M>) {
        Self::focus_child(ctx, route, DATA_SLOT, DATA_FOCUS);
        ctx.request_layout();
    }
}
