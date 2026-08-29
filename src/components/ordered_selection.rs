pub(crate) struct OrderedSelection<Id> {
    pub(crate) selected: Vec<Id>,
    pub(crate) anchor: Id,
    pub(crate) range_mode: bool,
}

impl<Id: Clone + Eq> OrderedSelection<Id> {
    pub(crate) fn extend_range(&mut self, ordered: &[Id], current: &Id, destination: &Id) {
        let Some(current_index) = ordered.iter().position(|id| id == current) else {
            return;
        };
        let Some(destination_index) = ordered.iter().position(|id| id == destination) else {
            return;
        };
        let anchor_index = ordered
            .iter()
            .position(|id| id == &self.anchor)
            .unwrap_or(current_index);
        let (start, end) = if anchor_index <= destination_index {
            (anchor_index, destination_index)
        } else {
            (destination_index, anchor_index)
        };
        self.selected = ordered[start..=end].to_vec();
        self.range_mode = true;
    }

    pub(crate) fn move_with_control(&mut self, current: Id) {
        if self.selected.is_empty() {
            self.selected.push(current.clone());
        }
        self.anchor = current;
        self.range_mode = false;
    }

    pub(crate) fn toggle(&mut self, ordered: &[Id], current: Id) -> Vec<Id> {
        if self.selected.iter().any(|id| !ordered.contains(id)) {
            self.selected.clear();
        }
        self.anchor = current.clone();
        self.range_mode = false;
        if let Some(index) = self.selected.iter().position(|id| id == &current) {
            self.selected.remove(index);
        } else {
            self.selected.push(current);
        }
        self.selected = ordered
            .iter()
            .filter(|id| self.selected.contains(id))
            .cloned()
            .collect();
        self.selected.clone()
    }

    pub(crate) fn reconcile(&mut self, ordered: &[Id]) -> bool {
        self.selected = ordered
            .iter()
            .filter(|id| self.selected.contains(id))
            .cloned()
            .collect();
        if self.selected.is_empty() {
            return false;
        }
        if !ordered.contains(&self.anchor) {
            self.anchor = self.selected[0].clone();
        }
        true
    }
}
