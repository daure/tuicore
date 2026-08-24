use std::collections::{HashMap, HashSet};
use std::hash::Hash;

#[cfg(test)]
use super::DisplayRow;
use super::{
    DataView, ReorderUnavailableReason, SelectionOverlay, SelectionOverlayPosition, TreeAdapter,
};

#[derive(Debug, Clone)]
pub(crate) struct TreeEditSnapshot<Id> {
    pub ids: Vec<Id>,
    parents: HashMap<Id, Option<Id>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TreeMoveResult<Id> {
    pub parent_id: Option<Id>,
    pub sibling_index: usize,
}

impl<T, Id> DataView<T, Id>
where
    Id: Clone + Eq + Hash,
{
    pub(crate) fn tree_is_mutable(&self) -> bool {
        matches!(self.tree, Some(TreeAdapter::MutableParentId { .. }))
    }

    pub(crate) fn set_selection_overlay(
        &mut self,
        selected: Vec<Id>,
        position: Option<SelectionOverlayPosition<Id>>,
        placeholder_depth: usize,
        placeholder_focused: bool,
    ) {
        self.selection_overlay = Some(SelectionOverlay {
            selected,
            position,
            placeholder_depth,
            placeholder_focused,
        });
    }

    pub(crate) fn clear_selection_overlay(&mut self) {
        self.selection_overlay = None;
    }

    pub(crate) fn tree_siblings(&self, id: &Id) -> Option<(Option<Id>, Vec<Id>)> {
        let parent_id = self.tree_parent_id(id)?;
        Some((parent_id.clone(), self.tree_children(parent_id.as_ref())))
    }

    pub(crate) fn tree_children_for_parent(&self, parent_id: Option<&Id>) -> Vec<Id> {
        self.tree_children(parent_id)
    }

    pub(crate) fn move_tree_sibling_block(
        &mut self,
        ids: &[Id],
        source_parent_id: Option<Id>,
        target_parent_id: Option<Id>,
        sibling_index: usize,
    ) -> Option<TreeMoveResult<Id>> {
        let Some(TreeAdapter::MutableParentId {
            parent_id: get_parent_id,
            set_parent_id,
        }) = self.tree.as_ref()
        else {
            return None;
        };
        let siblings = self.tree_children(source_parent_id.as_ref());
        if ids.len() < 2 || !ids.iter().all(|id| siblings.contains(id)) {
            return None;
        }
        let subtree_ids = ids
            .iter()
            .flat_map(|id| std::iter::once(id.clone()).chain(self.descendant_ids(id)))
            .collect::<HashSet<_>>();
        if target_parent_id
            .as_ref()
            .is_some_and(|parent_id| subtree_ids.contains(parent_id))
        {
            return None;
        }
        for row in &mut self.rows {
            let id = (self.row_id)(row);
            if ids.contains(&id) {
                set_parent_id(row, target_parent_id.clone());
                assert!(
                    (self.row_id)(row) == id,
                    "TreeAdapter parent setter must preserve the row ID"
                );
                assert!(
                    get_parent_id(row) == target_parent_id,
                    "TreeAdapter parent setter must apply the requested parent ID"
                );
            }
        }
        let mut moving = Vec::new();
        let mut remaining = Vec::new();
        for row in std::mem::take(&mut self.rows) {
            if subtree_ids.contains(&(self.row_id)(&row)) {
                moving.push(row);
            } else {
                remaining.push(row);
            }
        }
        let remaining_siblings = self
            .tree_children_in(
                &remaining,
                target_parent_id.as_ref(),
                get_parent_id.as_ref(),
            )
            .iter()
            .filter(|id| !subtree_ids.contains(*id))
            .cloned()
            .collect::<Vec<_>>();
        let target = sibling_index.min(remaining_siblings.len());
        let insertion = if let Some(next) = remaining_siblings.get(target) {
            remaining
                .iter()
                .position(|row| &(self.row_id)(row) == next)
                .unwrap_or(remaining.len())
        } else if let Some(last) = remaining_siblings.last() {
            let last_subtree = std::iter::once(last.clone())
                .chain(self.descendant_ids_in(&remaining, last, get_parent_id.as_ref()))
                .collect::<HashSet<_>>();
            remaining
                .iter()
                .rposition(|row| last_subtree.contains(&(self.row_id)(row)))
                .map_or(remaining.len(), |index| index + 1)
        } else if let Some(parent) = target_parent_id.as_ref() {
            remaining
                .iter()
                .position(|row| &(self.row_id)(row) == parent)
                .map_or(remaining.len(), |index| index + 1)
        } else {
            remaining.len()
        };
        remaining.splice(insertion..insertion, moving);
        self.rows = remaining;
        self.clamp_visible_state();
        self.reposition_highlight_silently(&ids[0]);
        Some(TreeMoveResult {
            parent_id: target_parent_id,
            sibling_index: target,
        })
    }

    pub(crate) fn tree_edit_snapshot(&self) -> Option<TreeEditSnapshot<Id>> {
        let parent_id = match self.tree.as_ref()? {
            TreeAdapter::ParentId(parent_id) | TreeAdapter::MutableParentId { parent_id, .. } => {
                parent_id
            }
            TreeAdapter::Level(_) => return None,
        };
        let ids = self.row_ids();
        let parents = self
            .rows
            .iter()
            .map(|row| ((self.row_id)(row), parent_id(row)))
            .collect();
        Some(TreeEditSnapshot { ids, parents })
    }

    pub(crate) fn tree_edit_snapshot_matches(&self, snapshot: &TreeEditSnapshot<Id>) -> bool {
        let Some(current) = self.tree_edit_snapshot() else {
            return false;
        };
        current.ids == snapshot.ids
            && current.parents.len() == snapshot.parents.len()
            && current
                .parents
                .iter()
                .all(|(id, parent)| snapshot.parents.get(id) == Some(parent))
    }

    pub(crate) fn tree_edit_unavailable_reason(&self) -> Option<ReorderUnavailableReason> {
        if self.visible_row_indices.is_some() {
            return Some(ReorderUnavailableReason::VisibleSubset);
        }
        if !self.transform_state.search.trim().is_empty()
            || !self.transform_state.filters.is_empty()
        {
            return Some(ReorderUnavailableReason::TransformActive);
        }
        if self.pagination.is_some() {
            return Some(ReorderUnavailableReason::Paginated);
        }
        let ids = self.row_ids();
        (ids.iter().collect::<HashSet<_>>().len() != ids.len())
            .then_some(ReorderUnavailableReason::DuplicateRowIds)
    }

    pub(crate) fn set_new_row_parent(&self, row: &mut T, parent_id: Option<Id>) -> bool {
        let Some(TreeAdapter::MutableParentId {
            parent_id: get_parent_id,
            set_parent_id,
        }) = self.tree.as_ref()
        else {
            return false;
        };
        let row_id = (self.row_id)(row);
        set_parent_id(row, parent_id.clone());
        assert!(
            (self.row_id)(row) == row_id,
            "TreeAdapter parent setter must preserve the row ID"
        );
        assert!(
            get_parent_id(row) == parent_id,
            "TreeAdapter parent setter must apply the requested parent ID"
        );
        true
    }

    pub(crate) fn expand_tree_row(&mut self, id: Id) {
        self.expanded.insert(id);
    }

    pub(crate) fn tree_expansion_snapshot(&self) -> HashSet<Id> {
        self.expanded.clone()
    }

    pub(crate) fn restore_tree_expansion(&mut self, expanded: HashSet<Id>) {
        self.expanded = expanded;
    }

    pub(crate) fn move_tree_sibling(
        &mut self,
        id: &Id,
        delta: isize,
    ) -> Option<TreeMoveResult<Id>> {
        let parent_id = self.tree_parent_id(id)?;
        let siblings = self.tree_children(parent_id.as_ref());
        let current = siblings.iter().position(|candidate| candidate == id)?;
        let target = current
            .saturating_add_signed(delta)
            .min(siblings.len().saturating_sub(1));
        (target != current)
            .then(|| self.reparent_tree_row(id, parent_id, target))
            .flatten()
    }

    pub(crate) fn indent_tree_row(&mut self, id: &Id) -> Option<TreeMoveResult<Id>> {
        let parent_id = self.tree_parent_id(id)?;
        let siblings = self.tree_children(parent_id.as_ref());
        let current = siblings.iter().position(|candidate| candidate == id)?;
        let new_parent = siblings.get(current.checked_sub(1)?).cloned()?;
        let target = self.tree_children(Some(&new_parent)).len();
        let result = self.reparent_tree_row(id, Some(new_parent.clone()), target);
        if result.is_some() {
            self.expanded.insert(new_parent);
        }
        result
    }

    pub(crate) fn outdent_tree_row(&mut self, id: &Id) -> Option<TreeMoveResult<Id>> {
        let parent_id = self.tree_parent_id(id)??;
        let grandparent_id = self.tree_parent_id(&parent_id)?;
        let parent_index = self
            .tree_children(grandparent_id.as_ref())
            .iter()
            .position(|candidate| candidate == &parent_id)?;
        self.reparent_tree_row(id, grandparent_id, parent_index + 1)
    }

    pub(crate) fn restore_tree_edit_after_conflict(
        &mut self,
        original: &TreeEditSnapshot<Id>,
        staged: &TreeEditSnapshot<Id>,
    ) {
        let Some(TreeAdapter::MutableParentId {
            parent_id: get_parent_id,
            set_parent_id,
        }) = self.tree.as_ref()
        else {
            return;
        };
        for row in &mut self.rows {
            let id = (self.row_id)(row);
            let Some(staged_parent) = staged.parents.get(&id) else {
                continue;
            };
            if get_parent_id(row) == staged_parent.clone()
                && let Some(original_parent) = original.parents.get(&id)
            {
                set_parent_id(row, original_parent.clone());
                assert!(
                    (self.row_id)(row) == id,
                    "TreeAdapter parent setter must preserve the row ID"
                );
                assert!(
                    get_parent_id(row) == original_parent.clone(),
                    "TreeAdapter parent setter must apply the requested parent ID"
                );
            }
        }
        if self.row_ids() == staged.ids {
            self.reorder_source_rows(&original.ids);
        }
        self.clamp_visible_state();
    }

    pub(crate) fn tree_move_result(&self, id: &Id) -> Option<TreeMoveResult<Id>> {
        let parent_id = self.tree_parent_id(id)?;
        let sibling_index = self
            .tree_children(parent_id.as_ref())
            .iter()
            .position(|candidate| candidate == id)?;
        Some(TreeMoveResult {
            parent_id,
            sibling_index,
        })
    }

    pub(crate) fn tree_parent_id(&self, id: &Id) -> Option<Option<Id>> {
        let parent_id = match self.tree.as_ref()? {
            TreeAdapter::ParentId(parent_id) | TreeAdapter::MutableParentId { parent_id, .. } => {
                parent_id
            }
            TreeAdapter::Level(_) => return None,
        };
        self.rows
            .iter()
            .find(|row| &(self.row_id)(row) == id)
            .map(|row| parent_id(row).filter(|parent| self.contains_row_id(parent)))
    }

    pub(crate) fn tree_depth(&self, id: &Id) -> Option<usize> {
        let mut depth = 0;
        let Some(mut parent) = self.tree_parent_id(id)? else {
            return Some(depth);
        };
        loop {
            depth += 1;
            match self.tree_parent_id(&parent)? {
                Some(next) => parent = next,
                None => return Some(depth),
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn selection_placeholder_depth_for_test(&self) -> Option<usize> {
        self.display_rows().into_iter().find_map(|row| match row {
            DisplayRow::SelectionPlaceholder { depth, .. } => Some(depth),
            DisplayRow::Data(_) => None,
        })
    }

    fn tree_children(&self, parent: Option<&Id>) -> Vec<Id> {
        let parent_id = match self.tree.as_ref() {
            Some(TreeAdapter::ParentId(parent_id))
            | Some(TreeAdapter::MutableParentId { parent_id, .. }) => parent_id,
            _ => return Vec::new(),
        };
        self.rows
            .iter()
            .filter(|row| {
                parent_id(row)
                    .filter(|candidate| self.contains_row_id(candidate))
                    .as_ref()
                    == parent
            })
            .map(|row| (self.row_id)(row))
            .collect()
    }

    fn tree_children_in(
        &self,
        rows: &[T],
        parent: Option<&Id>,
        parent_id: &dyn Fn(&T) -> Option<Id>,
    ) -> Vec<Id> {
        let known_ids = rows
            .iter()
            .map(|row| (self.row_id)(row))
            .collect::<HashSet<_>>();
        rows.iter()
            .filter(|row| {
                parent_id(row)
                    .filter(|candidate| known_ids.contains(candidate))
                    .as_ref()
                    == parent
            })
            .map(|row| (self.row_id)(row))
            .collect()
    }

    fn reparent_tree_row(
        &mut self,
        id: &Id,
        parent_id: Option<Id>,
        sibling_index: usize,
    ) -> Option<TreeMoveResult<Id>> {
        if parent_id
            .as_ref()
            .is_some_and(|parent_id| parent_id == id || self.descendant_ids(id).contains(parent_id))
        {
            return None;
        }
        let subtree_ids = std::iter::once(id.clone())
            .chain(self.descendant_ids(id))
            .collect::<Vec<_>>();
        let subtree_set = subtree_ids.iter().cloned().collect::<HashSet<_>>();
        let mut moving = Vec::new();
        let mut remaining = Vec::new();
        for row in std::mem::take(&mut self.rows) {
            if subtree_set.contains(&(self.row_id)(&row)) {
                moving.push(row);
            } else {
                remaining.push(row);
            }
        }
        let Some(TreeAdapter::MutableParentId {
            set_parent_id,
            parent_id: get_parent_id,
        }) = self.tree.as_ref()
        else {
            self.rows = remaining;
            self.rows.extend(moving);
            return None;
        };
        let root = moving.iter_mut().find(|row| &(self.row_id)(row) == id)?;
        let root_id = (self.row_id)(root);
        set_parent_id(root, parent_id.clone());
        assert!(
            (self.row_id)(root) == root_id,
            "TreeAdapter parent setter must preserve the row ID"
        );
        assert!(
            get_parent_id(root) == parent_id,
            "TreeAdapter parent setter must apply the requested parent ID"
        );

        let known_ids = remaining
            .iter()
            .map(|row| (self.row_id)(row))
            .chain(moving.iter().map(|row| (self.row_id)(row)))
            .collect::<HashSet<_>>();
        let siblings = remaining
            .iter()
            .filter(|row| {
                get_parent_id(row).filter(|parent| known_ids.contains(parent)) == parent_id
            })
            .map(|row| (self.row_id)(row))
            .collect::<Vec<_>>();
        let target = sibling_index.min(siblings.len());
        let insertion = if let Some(next_sibling) = siblings.get(target) {
            remaining
                .iter()
                .position(|row| &(self.row_id)(row) == next_sibling)
                .unwrap_or(remaining.len())
        } else if let Some(last_sibling) = siblings.last() {
            let last_subtree = std::iter::once(last_sibling.clone())
                .chain(self.descendant_ids_in(&remaining, last_sibling, get_parent_id.as_ref()))
                .collect::<HashSet<_>>();
            remaining
                .iter()
                .rposition(|row| last_subtree.contains(&(self.row_id)(row)))
                .map_or(remaining.len(), |index| index + 1)
        } else if let Some(parent_id) = parent_id.as_ref() {
            remaining
                .iter()
                .position(|row| &(self.row_id)(row) == parent_id)
                .map_or(remaining.len(), |index| index + 1)
        } else {
            remaining.len()
        };
        remaining.splice(insertion..insertion, moving);
        self.rows = remaining;
        self.clamp_visible_state();
        self.reposition_highlight_silently(id);
        Some(TreeMoveResult {
            parent_id,
            sibling_index: target,
        })
    }

    fn descendant_ids_in(
        &self,
        rows: &[T],
        id: &Id,
        parent_id: &dyn Fn(&T) -> Option<Id>,
    ) -> Vec<Id> {
        let mut descendants = Vec::new();
        let mut frontier = vec![id.clone()];
        while let Some(parent) = frontier.pop() {
            for row in rows {
                let child_id = (self.row_id)(row);
                if parent_id(row).as_ref() == Some(&parent) && !descendants.contains(&child_id) {
                    frontier.push(child_id.clone());
                    descendants.push(child_id);
                }
            }
        }
        descendants
    }

    fn reorder_source_rows(&mut self, ids: &[Id]) {
        let mut rows = std::mem::take(&mut self.rows);
        let mut reordered = Vec::with_capacity(rows.len());
        for id in ids {
            if let Some(index) = rows.iter().position(|row| &(self.row_id)(row) == id) {
                reordered.push(rows.remove(index));
            }
        }
        reordered.append(&mut rows);
        self.rows = reordered;
    }
}
