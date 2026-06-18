use crate::{FocusRepair, FocusRequest, FocusTarget, TreePath};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusTransition {
    pub previous: Option<FocusTarget>,
    pub current: Option<FocusTarget>,
}

#[derive(Debug, Clone, Default)]
pub struct FocusManager {
    current: Option<FocusTarget>,
    last_focused: Option<FocusTarget>,
}

impl FocusManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current(&self) -> Option<&FocusTarget> {
        self.current.as_ref()
    }

    pub fn current_path(&self) -> TreePath {
        self.current
            .as_ref()
            .map(|target| target.path.clone())
            .unwrap_or_default()
    }

    pub fn validate(&mut self, targets: &[FocusTarget]) -> Option<FocusTransition> {
        if let Some(current) = &self.current {
            if let Some(updated) = targets
                .iter()
                .find(|target| target.enabled && same_focus(target, current))
                .cloned()
            {
                self.current = Some(updated);
                return None;
            }
        } else if self
            .last_focused
            .as_ref()
            .is_some_and(|last| targets.iter().any(|target| same_focus(target, last)))
        {
            return None;
        }

        self.set_current(validate_replacement_target(self.current.as_ref(), targets))
    }

    pub fn repair(
        &mut self,
        repair: &FocusRepair,
        targets: &[FocusTarget],
    ) -> Option<FocusTransition> {
        if let Some(current) = &self.current {
            if let Some(updated) = targets
                .iter()
                .find(|target| target.enabled && same_focus(target, current))
                .cloned()
            {
                self.current = Some(updated);
                return None;
            }
        } else if self
            .last_focused
            .as_ref()
            .is_some_and(|last| targets.iter().any(|target| same_focus(target, last)))
        {
            return None;
        }

        self.set_current(repair_target(repair, self.current.as_ref(), targets))
    }

    pub fn apply_request(
        &mut self,
        request: &FocusRequest,
        targets: &[FocusTarget],
    ) -> Option<FocusTransition> {
        match request {
            FocusRequest::Next => self.set_current(self.next_target(targets)),
            FocusRequest::Previous => self.set_current(self.previous_target(targets)),
            FocusRequest::Unfocus => self.set_current(self.parent_target(targets)),
            FocusRequest::Last => self.set_current_if_found(self.last_enabled_target(targets)),
            FocusRequest::Target(id) => {
                self.set_current_if_found(unique_enabled_target(targets, |target| &target.id == id))
            }
            FocusRequest::Path(path) => self
                .set_current_if_found(unique_enabled_target(targets, |target| {
                    &target.path == path
                })),
            FocusRequest::TargetAt { path, id } => {
                self.set_current_if_found(unique_enabled_target(targets, |target| {
                    &target.path == path && &target.id == id
                }))
            }
        }
    }

    pub fn next(&mut self, targets: &[FocusTarget]) -> Option<FocusTransition> {
        self.apply_request(&FocusRequest::Next, targets)
    }

    pub fn previous(&mut self, targets: &[FocusTarget]) -> Option<FocusTransition> {
        self.apply_request(&FocusRequest::Previous, targets)
    }

    fn next_target(&self, targets: &[FocusTarget]) -> Option<FocusTarget> {
        let traversal = traversal_targets(targets);
        if traversal.is_empty() {
            return None;
        }

        if let Some(current) = &self.current {
            if let Some(first_child) = first_leaf_descendant(current, targets) {
                return Some(first_child);
            }
            let index = traversal
                .iter()
                .position(|target| same_focus(target, current))
                .map(|index| (index + 1) % traversal.len())
                .or_else(|| {
                    nearest_enabled_target(Some(current), targets).and_then(|nearest| {
                        traversal
                            .iter()
                            .position(|target| same_focus(target, &nearest))
                            .map(|index| (index + 1) % traversal.len())
                    })
                })
                .unwrap_or(0);
            Some(traversal[index].clone())
        } else if let Some(last) = &self.last_focused {
            let target = traversal
                .iter()
                .find(|target| same_focus(target, last))
                .map(|&t| t.clone())
                .or_else(|| nearest_traversal_target(Some(last), targets));
            target
        } else {
            Some(traversal[0].clone())
        }
    }

    fn previous_target(&self, targets: &[FocusTarget]) -> Option<FocusTarget> {
        let traversal = traversal_targets(targets);
        if traversal.is_empty() {
            return None;
        }

        if let Some(current) = &self.current {
            if let Some(last_child) = last_leaf_descendant(current, targets) {
                return Some(last_child);
            }
            let index = traversal
                .iter()
                .position(|target| same_focus(target, current))
                .map(|index| {
                    if index == 0 {
                        traversal.len() - 1
                    } else {
                        index - 1
                    }
                })
                .or_else(|| {
                    nearest_enabled_target(Some(current), targets).and_then(|nearest| {
                        traversal
                            .iter()
                            .position(|target| same_focus(target, &nearest))
                            .map(|index| {
                                if index == 0 {
                                    traversal.len() - 1
                                } else {
                                    index - 1
                                }
                            })
                    })
                })
                .unwrap_or(0);
            Some(traversal[index].clone())
        } else if let Some(last) = &self.last_focused {
            let target = traversal
                .iter()
                .find(|target| same_focus(target, last))
                .map(|&t| t.clone())
                .or_else(|| nearest_traversal_target(Some(last), targets));
            target
        } else {
            Some(traversal[0].clone())
        }
    }

    fn set_current(&mut self, next: Option<FocusTarget>) -> Option<FocusTransition> {
        if self
            .current
            .as_ref()
            .zip(next.as_ref())
            .is_some_and(|(current, next)| same_focus(current, next))
        {
            self.current = next;
            return None;
        }

        let previous = std::mem::replace(&mut self.current, next);
        if previous.is_none() && self.current.is_none() {
            return None;
        }

        if let Some(ref prev) = previous {
            self.last_focused = Some(prev.clone());
        }

        Some(FocusTransition {
            previous,
            current: self.current.clone(),
        })
    }

    fn set_current_if_found(&mut self, next: Option<FocusTarget>) -> Option<FocusTransition> {
        match next {
            Some(next) => self.set_current(Some(next)),
            None => None,
        }
    }

    fn last_enabled_target(&self, targets: &[FocusTarget]) -> Option<FocusTarget> {
        let last = self.last_focused.as_ref()?;
        targets
            .iter()
            .find(|target| target.enabled && same_focus(target, last))
            .cloned()
    }

    fn parent_target(&self, targets: &[FocusTarget]) -> Option<FocusTarget> {
        let current = self.current.as_ref()?;
        targets
            .iter()
            .filter(|target| {
                target.enabled
                    && current.path.keys().starts_with(target.path.keys())
                    && target.path != current.path
            })
            .max_by_key(|target| target.path.keys().len())
            .cloned()
    }
}

fn enabled_targets(targets: &[FocusTarget]) -> Vec<&FocusTarget> {
    targets.iter().filter(|target| target.enabled).collect()
}

fn traversal_targets(targets: &[FocusTarget]) -> Vec<&FocusTarget> {
    let leaves = targets
        .iter()
        .filter(|target| target.enabled && !has_enabled_descendant(target, targets))
        .collect::<Vec<_>>();
    if leaves.is_empty() {
        enabled_targets(targets)
    } else {
        leaves
    }
}

fn repair_target(
    repair: &FocusRepair,
    current: Option<&FocusTarget>,
    targets: &[FocusTarget],
) -> Option<FocusTarget> {
    match *repair {
        FocusRepair::RemovedChild { index: _ } => nearest_enabled_target(current, targets),
    }
}

fn validate_replacement_target(
    current: Option<&FocusTarget>,
    targets: &[FocusTarget],
) -> Option<FocusTarget> {
    let Some(current) = current else {
        return targets.iter().find(|target| target.enabled).cloned();
    };

    if let Some(replacement) = same_path_target(current, targets) {
        return Some(replacement);
    }

    if let Some(ancestor) = nearest_enabled_ancestor(current, targets) {
        if let Some(descendant) = first_leaf_descendant(&ancestor, targets) {
            return Some(descendant);
        }
        return Some(ancestor);
    }

    nearest_enabled_target(Some(current), targets)
}

fn nearest_enabled_target(
    current: Option<&FocusTarget>,
    targets: &[FocusTarget],
) -> Option<FocusTarget> {
    let Some(current) = current else {
        return targets.iter().find(|target| target.enabled).cloned();
    };

    if let Some(same_path) = same_path_target(current, targets) {
        return Some(same_path);
    }

    if !current.path.is_empty() {
        let descendants = targets
            .iter()
            .filter(|target| {
                target.enabled
                    && target.path.keys().starts_with(current.path.keys())
                    && target.path != current.path
            })
            .cloned()
            .collect::<Vec<_>>();

        if !descendants.is_empty() {
            return descendants
                .into_iter()
                .min_by_key(|target| focus_distance(current, target));
        }
    }

    let ancestors = targets
        .iter()
        .filter(|target| {
            target.enabled
                && current.path.keys().starts_with(target.path.keys())
                && target.path != current.path
        })
        .cloned()
        .collect::<Vec<_>>();

    if !ancestors.is_empty() {
        return ancestors
            .into_iter()
            .max_by_key(|target| target.path.keys().len());
    }

    let leaves = targets
        .iter()
        .filter(|target| target.enabled && !has_enabled_descendant(target, targets))
        .collect::<Vec<_>>();
    if leaves_share_root(&leaves) {
        return leaves.first().copied().cloned();
    }

    leaves
        .into_iter()
        .min_by_key(|target| focus_distance(current, target))
        .cloned()
        .or_else(|| {
            targets
                .iter()
                .filter(|target| target.enabled)
                .min_by_key(|target| focus_distance(current, target))
                .cloned()
        })
}

fn same_path_target(current: &FocusTarget, targets: &[FocusTarget]) -> Option<FocusTarget> {
    targets
        .iter()
        .find(|target| target.enabled && target.path == current.path)
        .cloned()
}

fn nearest_enabled_ancestor(current: &FocusTarget, targets: &[FocusTarget]) -> Option<FocusTarget> {
    targets
        .iter()
        .filter(|target| {
            target.enabled
                && current.path.keys().starts_with(target.path.keys())
                && target.path != current.path
        })
        .max_by_key(|target| target.path.keys().len())
        .cloned()
}

fn first_leaf_descendant(parent: &FocusTarget, targets: &[FocusTarget]) -> Option<FocusTarget> {
    targets
        .iter()
        .find(|target| {
            target.enabled
                && target.path != parent.path
                && target.path.keys().starts_with(parent.path.keys())
                && !has_enabled_descendant(target, targets)
        })
        .cloned()
}

fn last_leaf_descendant(parent: &FocusTarget, targets: &[FocusTarget]) -> Option<FocusTarget> {
    targets
        .iter()
        .rev()
        .find(|target| {
            target.enabled
                && target.path != parent.path
                && target.path.keys().starts_with(parent.path.keys())
                && !has_enabled_descendant(target, targets)
        })
        .cloned()
}

fn nearest_traversal_target(
    current: Option<&FocusTarget>,
    targets: &[FocusTarget],
) -> Option<FocusTarget> {
    let current = current?;
    traversal_targets(targets)
        .into_iter()
        .min_by_key(|target| focus_distance(current, target))
        .cloned()
}

fn has_enabled_descendant(target: &FocusTarget, targets: &[FocusTarget]) -> bool {
    targets.iter().any(|other| {
        other.enabled
            && other.path != target.path
            && other.path.keys().starts_with(target.path.keys())
    })
}

fn leaves_share_root(leaves: &[&FocusTarget]) -> bool {
    let Some(first) = leaves.first().and_then(|target| target.path.first()) else {
        return false;
    };
    leaves
        .iter()
        .all(|target| target.path.first() == Some(first))
}

fn focus_distance(current: &FocusTarget, target: &FocusTarget) -> u32 {
    let current_x = u32::from(current.area.x).saturating_mul(2) + u32::from(current.area.width);
    let current_y = u32::from(current.area.y).saturating_mul(2) + u32::from(current.area.height);
    let target_x = u32::from(target.area.x).saturating_mul(2) + u32::from(target.area.width);
    let target_y = u32::from(target.area.y).saturating_mul(2) + u32::from(target.area.height);

    current_x.abs_diff(target_x) + current_y.abs_diff(target_y)
}

fn unique_enabled_target(
    targets: &[FocusTarget],
    matches: impl Fn(&FocusTarget) -> bool,
) -> Option<FocusTarget> {
    let mut found = targets
        .iter()
        .filter(|target| target.enabled && matches(target));
    let target = found.next()?;
    if found.next().is_some() {
        None
    } else {
        Some(target.clone())
    }
}

fn same_focus(left: &FocusTarget, right: &FocusTarget) -> bool {
    left.id == right.id && left.path == right.path
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;

    use super::*;
    use crate::{ChildKey, FocusId};

    fn target(id: &str) -> FocusTarget {
        FocusTarget {
            id: FocusId::new(id),
            path: TreePath::from_keys([ChildKey::new(id)]),
            area: Rect::default(),
            enabled: true,
            hotkey: None,
            hotkeys: Vec::new(),
        }
    }

    fn target_at_path(id: &str, path: TreePath) -> FocusTarget {
        FocusTarget {
            id: FocusId::new(id),
            path,
            area: Rect::default(),
            enabled: true,
            hotkey: None,
            hotkeys: Vec::new(),
        }
    }

    fn target_at(id: &str, area: Rect) -> FocusTarget {
        FocusTarget { area, ..target(id) }
    }

    fn target_with_path(id: &str, path: TreePath, area: Rect) -> FocusTarget {
        FocusTarget {
            id: FocusId::new(id),
            path,
            area,
            enabled: true,
            hotkey: None,
            hotkeys: Vec::new(),
        }
    }

    #[test]
    fn focus_next_wraps_enabled_targets() {
        let targets = [target("one"), target("two")];
        let mut manager = FocusManager::new();

        manager.validate(&targets);
        manager.next(&targets);

        assert_eq!(manager.current().unwrap().id.as_str(), "two");
    }

    #[test]
    fn validate_repairs_missing_focus_to_first_enabled_target() {
        let old_targets = [target("old")];
        let new_targets = [target("new")];
        let mut manager = FocusManager::new();

        manager.validate(&old_targets);
        manager.validate(&new_targets);

        assert_eq!(manager.current().unwrap().id.as_str(), "new");
    }

    #[test]
    fn validate_repairs_missing_focus_to_nearest_enabled_target() {
        let old_targets = [
            target_at("one", Rect::new(0, 0, 5, 1)),
            target_at("two", Rect::new(10, 0, 5, 1)),
        ];
        let new_targets = [
            target_at("one", Rect::new(0, 0, 5, 1)),
            target_at("three", Rect::new(10, 0, 5, 1)),
        ];
        let mut manager = FocusManager::new();

        manager.validate(&old_targets);
        manager.next(&old_targets);
        manager.validate(&new_targets);

        assert_eq!(manager.current().unwrap().id.as_str(), "three");
    }

    #[test]
    fn last_request_restores_previous_enabled_target() {
        let targets = [target("one"), target("two")];
        let mut manager = FocusManager::new();

        manager.validate(&targets);
        manager.next(&targets);
        manager.apply_request(&FocusRequest::Last, &targets);

        assert_eq!(manager.current().unwrap().id.as_str(), "one");
    }

    #[test]
    fn unfocus_request_moves_to_nearest_parent_target() {
        let targets = [
            target_at_path("parent", TreePath::new()),
            target_at_path("child", TreePath::from_keys([ChildKey::new("body")])),
        ];
        let mut manager = FocusManager::new();

        manager.apply_request(
            &FocusRequest::TargetAt {
                path: TreePath::from_keys([ChildKey::new("body")]),
                id: FocusId::new("child"),
            },
            &targets,
        );
        manager.apply_request(&FocusRequest::Unfocus, &targets);

        assert_eq!(manager.current().unwrap().id.as_str(), "parent");
    }

    #[test]
    fn repeated_unfocus_climbs_nested_focusable_parents() {
        let dialog = TreePath::from_keys([ChildKey::new("dialog")]);
        let tabs = TreePath::from_keys([ChildKey::new("dialog"), ChildKey::new("body")]);
        let input = TreePath::from_keys([
            ChildKey::new("dialog"),
            ChildKey::new("body"),
            ChildKey::new("input"),
        ]);
        let targets = [
            target_at_path("input", input.clone()),
            target_at_path("tabs", tabs.clone()),
            target_at_path("dialog", dialog.clone()),
        ];
        let mut manager = FocusManager::new();

        manager.apply_request(
            &FocusRequest::TargetAt {
                path: input,
                id: FocusId::new("input"),
            },
            &targets,
        );
        manager.apply_request(&FocusRequest::Unfocus, &targets);
        assert_eq!(manager.current().unwrap().id.as_str(), "tabs");

        manager.apply_request(&FocusRequest::Unfocus, &targets);
        assert_eq!(manager.current().unwrap().id.as_str(), "dialog");

        manager.apply_request(&FocusRequest::Unfocus, &targets);
        assert!(manager.current().is_none());
    }

    #[test]
    fn next_and_previous_skip_container_targets() {
        let targets = [
            target_at_path(
                "first",
                TreePath::from_keys([ChildKey::new("panel"), ChildKey::new("first")]),
            ),
            target_at_path("panel", TreePath::from_keys([ChildKey::new("panel")])),
            target_at_path("second", TreePath::from_keys([ChildKey::new("second")])),
        ];
        let mut manager = FocusManager::new();

        manager.validate(&targets);
        assert_eq!(manager.current().unwrap().id.as_str(), "first");

        manager.next(&targets);
        assert_eq!(manager.current().unwrap().id.as_str(), "second");

        manager.previous(&targets);
        assert_eq!(manager.current().unwrap().id.as_str(), "first");
    }

    #[test]
    fn next_from_container_focuses_first_child() {
        let targets = [
            target_at_path(
                "first",
                TreePath::from_keys([ChildKey::new("dialog"), ChildKey::new("first")]),
            ),
            target_at_path(
                "second",
                TreePath::from_keys([ChildKey::new("dialog"), ChildKey::new("second")]),
            ),
            target_at_path("dialog", TreePath::from_keys([ChildKey::new("dialog")])),
        ];
        let mut manager = FocusManager::new();

        manager.apply_request(
            &FocusRequest::TargetAt {
                path: TreePath::from_keys([ChildKey::new("dialog")]),
                id: FocusId::new("dialog"),
            },
            &targets,
        );
        manager.next(&targets);

        assert_eq!(manager.current().unwrap().id.as_str(), "first");
    }

    #[test]
    fn missing_focus_repairs_to_leaf_target_before_container_shell() {
        let old_targets = [FocusTarget {
            id: FocusId::new("launcher"),
            path: TreePath::from_keys([ChildKey::new("base")]),
            area: Rect::new(80, 30, 10, 1),
            enabled: true,
            hotkey: None,
            hotkeys: Vec::new(),
        }];
        let new_targets = [
            FocusTarget {
                id: FocusId::new("toggle"),
                path: TreePath::from_keys([
                    ChildKey::new("dialog"),
                    ChildKey::new("body"),
                    ChildKey::new("tabs"),
                    ChildKey::new("toggle"),
                ]),
                area: Rect::new(0, 0, 1, 1),
                enabled: true,
                hotkey: None,
                hotkeys: Vec::new(),
            },
            FocusTarget {
                id: FocusId::new("input"),
                path: TreePath::from_keys([
                    ChildKey::new("dialog"),
                    ChildKey::new("body"),
                    ChildKey::new("tabs"),
                    ChildKey::new("input"),
                ]),
                area: Rect::new(80, 30, 10, 1),
                enabled: true,
                hotkey: None,
                hotkeys: Vec::new(),
            },
            FocusTarget {
                id: FocusId::new("tabs"),
                path: TreePath::from_keys([ChildKey::new("dialog"), ChildKey::new("body")]),
                area: Rect::new(0, 0, 100, 40),
                enabled: true,
                hotkey: None,
                hotkeys: Vec::new(),
            },
            FocusTarget {
                id: FocusId::new("dialog"),
                path: TreePath::from_keys([ChildKey::new("dialog")]),
                area: Rect::new(0, 0, 100, 40),
                enabled: true,
                hotkey: None,
                hotkeys: Vec::new(),
            },
        ];
        let mut manager = FocusManager::new();

        manager.validate(&old_targets);
        manager.validate(&new_targets);

        assert_eq!(manager.current().unwrap().id.as_str(), "toggle");
    }

    #[test]
    fn validation_after_selected_child_changes_focuses_first_leaf_under_parent() {
        let old_targets = [
            target_at_path("input", TreePath::from_keys([ChildKey::new("tab-0")])),
            target_at_path("tabs", TreePath::new()),
        ];
        let new_targets = [
            target_at_path("toggle", TreePath::from_keys([ChildKey::new("tab-1")])),
            target_at_path("input", TreePath::from_keys([ChildKey::new("tab-1-input")])),
            target_at_path("tabs", TreePath::new()),
        ];
        let mut manager = FocusManager::new();

        manager.validate(&old_targets);
        manager.validate(&new_targets);

        assert_eq!(manager.current().unwrap().id.as_str(), "toggle");
    }

    #[test]
    fn next_from_missing_current_anchors_on_nearest_surviving_target() {
        let old_targets = [FocusTarget {
            id: FocusId::new("search"),
            path: TreePath::from_keys([ChildKey::new("dropdown")]),
            area: Rect::new(10, 0, 1, 1),
            enabled: true,
            hotkey: None,
            hotkeys: Vec::new(),
        }];
        let new_targets = [
            FocusTarget {
                id: FocusId::new("before"),
                path: TreePath::from_keys([ChildKey::new("before")]),
                area: Rect::new(0, 0, 1, 1),
                enabled: true,
                hotkey: None,
                hotkeys: Vec::new(),
            },
            FocusTarget {
                id: FocusId::new("field"),
                path: TreePath::from_keys([ChildKey::new("dropdown")]),
                area: Rect::new(10, 0, 1, 1),
                enabled: true,
                hotkey: None,
                hotkeys: Vec::new(),
            },
            FocusTarget {
                id: FocusId::new("after"),
                path: TreePath::from_keys([ChildKey::new("after")]),
                area: Rect::new(20, 0, 1, 1),
                enabled: true,
                hotkey: None,
                hotkeys: Vec::new(),
            },
        ];
        let mut manager = FocusManager::new();

        manager.validate(&old_targets);
        manager.next(&new_targets);

        assert_eq!(manager.current().unwrap().id.as_str(), "after");
    }

    #[test]
    fn next_from_missing_current_anchors_on_same_path_replacement() {
        let old_targets = [FocusTarget {
            id: FocusId::new("input"),
            path: TreePath::from_keys([ChildKey::new("dropdown")]),
            area: Rect::new(10, 0, 1, 1),
            enabled: true,
            hotkey: None,
            hotkeys: Vec::new(),
        }];
        let new_targets = [
            FocusTarget {
                id: FocusId::new("before"),
                path: TreePath::from_keys([ChildKey::new("before")]),
                area: Rect::new(0, 0, 1, 1),
                enabled: true,
                hotkey: None,
                hotkeys: Vec::new(),
            },
            FocusTarget {
                id: FocusId::new("field"),
                path: TreePath::from_keys([ChildKey::new("dropdown")]),
                area: Rect::new(10, 0, 1, 1),
                enabled: true,
                hotkey: None,
                hotkeys: Vec::new(),
            },
            FocusTarget {
                id: FocusId::new("after"),
                path: TreePath::from_keys([ChildKey::new("after")]),
                area: Rect::new(20, 0, 1, 1),
                enabled: true,
                hotkey: None,
                hotkeys: Vec::new(),
            },
        ];
        let mut manager = FocusManager::new();

        manager.validate(&old_targets);
        manager.next(&new_targets);

        assert_eq!(manager.current().unwrap().id.as_str(), "after");
    }

    #[test]
    fn repair_removed_child_focuses_nearest_enabled_target() {
        let old_targets = [
            target_at("one", Rect::new(0, 0, 5, 1)),
            target_at("two", Rect::new(10, 0, 5, 1)),
            target_at("three", Rect::new(11, 0, 5, 1)),
        ];
        let new_targets = [
            target_at("one", Rect::new(0, 0, 5, 1)),
            target_at("three", Rect::new(11, 0, 5, 1)),
        ];
        let mut manager = FocusManager::new();

        manager.validate(&old_targets);
        manager.next(&old_targets);
        manager.repair(&FocusRepair::RemovedChild { index: 1 }, &new_targets);

        assert_eq!(manager.current().unwrap().id.as_str(), "three");
    }

    #[test]
    fn repair_removed_nested_child_does_not_treat_local_index_as_global_index() {
        let removed_path = TreePath::from_keys([ChildKey::new("parent"), ChildKey::new("removed")]);
        let near_path = TreePath::from_keys([ChildKey::new("parent"), ChildKey::new("near")]);
        let old_targets = [target_with_path(
            "input",
            removed_path.clone(),
            Rect::new(10, 0, 1, 1),
        )];
        let new_targets = [
            target_at("global-zero", Rect::new(100, 0, 1, 1)),
            target_with_path("near", near_path, Rect::new(11, 0, 1, 1)),
        ];
        let mut manager = FocusManager::new();

        manager.apply_request(
            &FocusRequest::TargetAt {
                path: removed_path,
                id: FocusId::new("input"),
            },
            &old_targets,
        );
        manager.repair(&FocusRepair::RemovedChild { index: 0 }, &new_targets);

        assert_eq!(manager.current().unwrap().id.as_str(), "near");
    }

    #[test]
    fn target_request_ignores_ambiguous_local_ids() {
        let targets = [
            FocusTarget {
                id: FocusId::new("input"),
                path: TreePath::from_keys([ChildKey::new("one")]),
                area: Rect::default(),
                enabled: true,
                hotkey: None,
                hotkeys: Vec::new(),
            },
            FocusTarget {
                id: FocusId::new("input"),
                path: TreePath::from_keys([ChildKey::new("two")]),
                area: Rect::default(),
                enabled: true,
                hotkey: None,
                hotkeys: Vec::new(),
            },
        ];
        let mut manager = FocusManager::new();

        manager.apply_request(&FocusRequest::Target(FocusId::new("input")), &targets);

        assert!(manager.current().is_none());
    }

    #[test]
    fn explicit_request_misses_preserve_existing_focus() {
        let targets = [target("current")];
        let mut manager = FocusManager::new();

        manager.validate(&targets);
        for request in [
            FocusRequest::Target(FocusId::new("missing")),
            FocusRequest::Path(TreePath::from_keys([ChildKey::new("missing")])),
            FocusRequest::TargetAt {
                path: TreePath::from_keys([ChildKey::new("missing")]),
                id: FocusId::new("missing"),
            },
        ] {
            let transition = manager.apply_request(&request, &targets);

            assert!(transition.is_none());
            assert_eq!(manager.current().unwrap().id.as_str(), "current");
        }
    }

    #[test]
    fn ambiguous_explicit_requests_preserve_existing_focus() {
        let targets = [
            target("current"),
            FocusTarget {
                id: FocusId::new("input"),
                path: TreePath::from_keys([ChildKey::new("shared")]),
                area: Rect::default(),
                enabled: true,
                hotkey: None,
                hotkeys: Vec::new(),
            },
            FocusTarget {
                id: FocusId::new("input"),
                path: TreePath::from_keys([ChildKey::new("shared")]),
                area: Rect::default(),
                enabled: true,
                hotkey: None,
                hotkeys: Vec::new(),
            },
        ];
        let mut manager = FocusManager::new();

        manager.validate(&targets);
        for request in [
            FocusRequest::Target(FocusId::new("input")),
            FocusRequest::Path(TreePath::from_keys([ChildKey::new("shared")])),
            FocusRequest::TargetAt {
                path: TreePath::from_keys([ChildKey::new("shared")]),
                id: FocusId::new("input"),
            },
        ] {
            let transition = manager.apply_request(&request, &targets);

            assert!(transition.is_none());
            assert_eq!(manager.current().unwrap().id.as_str(), "current");
        }
    }

    #[test]
    fn target_at_request_selects_exact_focus_identity() {
        let targets = [
            FocusTarget {
                id: FocusId::new("input"),
                path: TreePath::from_keys([ChildKey::new("one")]),
                area: Rect::default(),
                enabled: true,
                hotkey: None,
                hotkeys: Vec::new(),
            },
            FocusTarget {
                id: FocusId::new("input"),
                path: TreePath::from_keys([ChildKey::new("two")]),
                area: Rect::default(),
                enabled: true,
                hotkey: None,
                hotkeys: Vec::new(),
            },
        ];
        let mut manager = FocusManager::new();

        manager.apply_request(
            &FocusRequest::TargetAt {
                path: TreePath::from_keys([ChildKey::new("two")]),
                id: FocusId::new("input"),
            },
            &targets,
        );

        assert_eq!(
            manager.current().unwrap().path,
            TreePath::from_keys([ChildKey::new("two")])
        );
    }

    #[test]
    fn unfocus_and_restore_focus_via_next_previous() {
        let targets = [target("one"), target("two"), target("three")];
        let mut manager = FocusManager::new();

        // 1. Initial validation focuses first target.
        manager.validate(&targets);
        assert_eq!(manager.current().unwrap().id.as_str(), "one");

        // 2. Move focus to "two".
        manager.apply_request(&FocusRequest::Next, &targets);
        assert_eq!(manager.current().unwrap().id.as_str(), "two");

        // 3. Request unfocus.
        let transition = manager.apply_request(&FocusRequest::Unfocus, &targets);
        assert!(transition.is_some());
        assert!(manager.current().is_none());
        assert_eq!(manager.last_focused.as_ref().unwrap().id.as_str(), "two");

        // 4. Validate should keep us unfocused.
        manager.validate(&targets);
        assert!(manager.current().is_none());

        // 5. Pressing next/previous should restore focus back to the last focused element ("two").
        let transition_next = manager.apply_request(&FocusRequest::Next, &targets);
        assert!(transition_next.is_some());
        assert_eq!(manager.current().unwrap().id.as_str(), "two");

        // 6. Unfocus again and test with Previous request.
        manager.apply_request(&FocusRequest::Unfocus, &targets);
        assert!(manager.current().is_none());

        let transition_prev = manager.apply_request(&FocusRequest::Previous, &targets);
        assert!(transition_prev.is_some());
        assert_eq!(manager.current().unwrap().id.as_str(), "two");
    }
}
