use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use super::model::{LevelFn, ParentIdFn};
use super::{DataView, SortDirection, TreeAdapter, VisibleRow};

impl<T, Id> DataView<T, Id>
where
    Id: Clone + Eq + Hash,
{
    pub(super) fn visible_rows(&self) -> Vec<VisibleRow<'_, T, Id>> {
        self.paginated_rows(self.all_visible_rows())
    }

    pub(super) fn all_visible_rows(&self) -> Vec<VisibleRow<'_, T, Id>> {
        match &self.tree {
            Some(TreeAdapter::ParentId(parent_id)) => {
                let sorted = self.sorted_rows();
                self.parent_tree_rows(&sorted, parent_id.as_ref())
            }
            Some(TreeAdapter::Level(level)) => {
                let rows = self.row_refs();
                self.level_tree_rows(&rows, level.as_ref())
            }
            None => self
                .sorted_rows()
                .into_iter()
                .map(|row| VisibleRow {
                    row,
                    id: (self.row_id)(row),
                    parent_id: None,
                    depth: 0,
                    has_children: false,
                    expanded: false,
                })
                .collect(),
        }
    }

    pub(super) fn expandable_ids(&self) -> impl Iterator<Item = Id> + '_ {
        let expandable = match &self.tree {
            Some(TreeAdapter::ParentId(parent_id)) => {
                let rows = self.sorted_rows();
                let ids = rows
                    .iter()
                    .map(|row| (self.row_id)(row))
                    .collect::<Vec<_>>();
                let known_ids = ids.iter().cloned().collect::<HashSet<_>>();
                rows.iter()
                    .filter_map(|row| parent_id(row))
                    .filter(move |parent| known_ids.contains(parent))
                    .collect::<HashSet<_>>()
            }
            Some(TreeAdapter::Level(level)) => {
                let rows = self.row_refs();
                let ids = rows
                    .iter()
                    .map(|row| (self.row_id)(row))
                    .collect::<Vec<_>>();
                let levels = rows.iter().map(|row| level(row)).collect::<Vec<_>>();
                ids.into_iter()
                    .enumerate()
                    .filter_map(|(index, id)| {
                        levels
                            .get(index + 1)
                            .is_some_and(|next| *next > levels[index])
                            .then_some(id)
                    })
                    .collect::<HashSet<_>>()
            }
            None => HashSet::new(),
        };
        expandable.into_iter()
    }

    pub(super) fn row_ids(&self) -> Vec<Id> {
        self.rows.iter().map(|row| (self.row_id)(row)).collect()
    }

    pub(super) fn descendant_ids(&self, id: &Id) -> Vec<Id> {
        self.descendant_ids_by_id().remove(id).unwrap_or_default()
    }

    pub(super) fn descendant_ids_by_id(&self) -> HashMap<Id, Vec<Id>> {
        match &self.tree {
            Some(TreeAdapter::ParentId(parent_id)) => {
                self.parent_descendant_ids_by_id(parent_id.as_ref())
            }
            Some(TreeAdapter::Level(level)) => self.level_descendant_ids_by_id(level.as_ref()),
            None => HashMap::new(),
        }
    }

    pub(super) fn max_page(&self) -> usize {
        let Some(pagination) = &self.pagination else {
            return 0;
        };
        let total = self.all_visible_rows().len();
        total.saturating_sub(1) / pagination.page_size
    }

    fn row_refs(&self) -> Vec<&T> {
        if let Some(indices) = &self.visible_row_indices {
            indices
                .iter()
                .filter_map(|index| self.rows.get(*index))
                .collect()
        } else {
            self.rows.iter().collect()
        }
    }

    fn parent_descendant_ids_by_id(&self, parent_id: &ParentIdFn<T, Id>) -> HashMap<Id, Vec<Id>> {
        let ids = self.row_ids();
        let known_ids = ids.iter().cloned().collect::<HashSet<_>>();
        let mut children_by_parent: HashMap<Id, Vec<Id>> = HashMap::new();

        for (row, row_id) in self.rows.iter().zip(ids.iter()) {
            if let Some(parent) = parent_id(row).filter(|parent| known_ids.contains(parent)) {
                children_by_parent
                    .entry(parent)
                    .or_default()
                    .push(row_id.clone());
            }
        }

        ids.into_iter()
            .map(|id| {
                let descendants = collect_descendants(&children_by_parent, &id);
                (id, descendants)
            })
            .collect()
    }

    fn level_descendant_ids_by_id(&self, level: &LevelFn<T>) -> HashMap<Id, Vec<Id>> {
        let ids = self.row_ids();
        let levels = self.rows.iter().map(level).collect::<Vec<_>>();

        ids.iter()
            .enumerate()
            .map(|(index, id)| {
                let parent_level = levels[index];
                let descendants = ids
                    .iter()
                    .enumerate()
                    .skip(index + 1)
                    .take_while(|(index, _)| levels[*index] > parent_level)
                    .map(|(_, id)| id.clone())
                    .collect();
                (id.clone(), descendants)
            })
            .collect()
    }

    fn active_sort(&self) -> Option<(&dyn Fn(&T) -> String, SortDirection)> {
        let sort = self.sort.as_ref()?;
        let column = self
            .columns
            .iter()
            .find(|column| column.id == sort.column_id)?;
        let sort_key = column.sort_key.as_deref()?;
        Some((sort_key, sort.direction))
    }

    fn sorted_rows(&self) -> Vec<&T> {
        let mut rows = self.row_refs();
        let Some((sort_key, direction)) = self.active_sort() else {
            return rows;
        };
        rows.sort_by_key(|row| sort_key(row));
        if direction == SortDirection::Descending {
            rows.reverse();
        }
        rows
    }

    fn parent_tree_rows<'a>(
        &self,
        rows: &[&'a T],
        parent_id: &ParentIdFn<T, Id>,
    ) -> Vec<VisibleRow<'a, T, Id>> {
        let ids = rows
            .iter()
            .map(|row| (self.row_id)(row))
            .collect::<Vec<_>>();
        let parents = rows.iter().map(|row| parent_id(row)).collect::<Vec<_>>();
        let known_ids = ids.iter().cloned().collect::<HashSet<_>>();
        let mut children_by_parent: HashMap<Option<Id>, Vec<usize>> = HashMap::new();
        for (index, parent) in parents.iter().enumerate() {
            let parent = parent.clone().filter(|parent| known_ids.contains(parent));
            children_by_parent.entry(parent).or_default().push(index);
        }

        let mut output = Vec::new();
        let mut visited = HashSet::new();
        self.push_parent_tree_rows(
            None,
            0,
            rows,
            &ids,
            &children_by_parent,
            &mut visited,
            &mut output,
        );
        output
    }

    fn push_parent_tree_rows<'a>(
        &self,
        parent_id: Option<Id>,
        depth: usize,
        rows: &[&'a T],
        ids: &[Id],
        children_by_parent: &HashMap<Option<Id>, Vec<usize>>,
        visited: &mut HashSet<Id>,
        output: &mut Vec<VisibleRow<'a, T, Id>>,
    ) {
        let Some(indices) = children_by_parent.get(&parent_id).cloned() else {
            return;
        };
        for index in indices {
            let id = ids[index].clone();
            if !visited.insert(id.clone()) {
                continue;
            }
            let child_key = Some(id.clone());
            let has_children = children_by_parent
                .get(&child_key)
                .is_some_and(|children| !children.is_empty());
            let expanded = self.expanded.contains(&id);
            output.push(VisibleRow {
                row: rows[index],
                id,
                parent_id: parent_id.clone(),
                depth,
                has_children,
                expanded,
            });
            if has_children && expanded {
                self.push_parent_tree_rows(
                    child_key,
                    depth + 1,
                    rows,
                    ids,
                    children_by_parent,
                    visited,
                    output,
                );
            }
        }
    }

    fn level_tree_rows<'a>(
        &self,
        rows: &[&'a T],
        level: &LevelFn<T>,
    ) -> Vec<VisibleRow<'a, T, Id>> {
        let ids = rows
            .iter()
            .map(|row| (self.row_id)(row))
            .collect::<Vec<_>>();
        let levels = rows.iter().map(|row| level(row)).collect::<Vec<_>>();
        let mut parent_ids = vec![None; rows.len()];
        let mut roots = Vec::new();
        let mut children_by_parent = vec![Vec::new(); rows.len()];
        let mut stack: Vec<Option<usize>> = Vec::new();

        for (index, depth) in levels.iter().copied().enumerate() {
            stack.truncate(depth);
            let parent = depth
                .checked_sub(1)
                .and_then(|parent_depth| stack.get(parent_depth))
                .copied()
                .flatten();
            if let Some(parent) = parent {
                parent_ids[index] = Some(ids[parent].clone());
                children_by_parent[parent].push(index);
            } else {
                roots.push(index);
            }
            if stack.len() < depth {
                stack.resize(depth, None);
            }
            stack.push(Some(index));
        }

        if let Some((sort_key, direction)) = self.active_sort() {
            let sort_siblings = |indices: &mut Vec<usize>| {
                indices.sort_by_key(|index| sort_key(rows[*index]));
                if direction == SortDirection::Descending {
                    indices.reverse();
                }
            };
            sort_siblings(&mut roots);
            for children in &mut children_by_parent {
                sort_siblings(children);
            }
        }

        let mut output = Vec::new();
        for index in roots {
            self.push_level_tree_rows(
                index,
                rows,
                &ids,
                &levels,
                &parent_ids,
                &children_by_parent,
                &mut output,
            );
        }
        output
    }

    fn push_level_tree_rows<'a>(
        &self,
        index: usize,
        rows: &[&'a T],
        ids: &[Id],
        levels: &[usize],
        parent_ids: &[Option<Id>],
        children_by_parent: &[Vec<usize>],
        output: &mut Vec<VisibleRow<'a, T, Id>>,
    ) {
        let id = ids[index].clone();
        let has_children = !children_by_parent[index].is_empty();
        let expanded = self.expanded.contains(&id);
        output.push(VisibleRow {
            row: rows[index],
            id,
            parent_id: parent_ids[index].clone(),
            depth: levels[index],
            has_children,
            expanded,
        });
        if has_children && expanded {
            for child in &children_by_parent[index] {
                self.push_level_tree_rows(
                    *child,
                    rows,
                    ids,
                    levels,
                    parent_ids,
                    children_by_parent,
                    output,
                );
            }
        }
    }

    fn paginated_rows<'a>(&self, rows: Vec<VisibleRow<'a, T, Id>>) -> Vec<VisibleRow<'a, T, Id>> {
        let Some(pagination) = &self.pagination else {
            return rows;
        };
        let start = pagination.page.saturating_mul(pagination.page_size);
        rows.into_iter()
            .skip(start)
            .take(pagination.page_size)
            .collect()
    }
}

fn collect_descendants<Id>(children_by_parent: &HashMap<Id, Vec<Id>>, id: &Id) -> Vec<Id>
where
    Id: Clone + Eq + Hash,
{
    let mut descendants = Vec::new();
    let mut stack = children_by_parent.get(id).cloned().unwrap_or_default();
    let mut visited = HashSet::new();
    while let Some(child) = stack.pop() {
        if !visited.insert(child.clone()) {
            continue;
        }
        descendants.push(child.clone());
        if let Some(children) = children_by_parent.get(&child) {
            stack.extend(children.iter().cloned());
        }
    }
    descendants
}
