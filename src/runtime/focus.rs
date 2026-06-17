use crate::{FocusRepair, FocusRequest, FocusTarget, TreePath};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusTransition {
    pub previous: Option<FocusTarget>,
    pub current: Option<FocusTarget>,
}

#[derive(Debug, Clone, Default)]
pub struct FocusManager {
    current: Option<FocusTarget>,
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
        }

        self.set_current(nearest_enabled_target(self.current.as_ref(), targets))
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
        let enabled = enabled_targets(targets);
        if enabled.is_empty() {
            return None;
        }

        let index = self
            .current
            .as_ref()
            .and_then(|current| {
                enabled
                    .iter()
                    .position(|target| same_focus(target, current))
            })
            .map(|index| (index + 1) % enabled.len())
            .unwrap_or(0);

        Some(enabled[index].clone())
    }

    fn previous_target(&self, targets: &[FocusTarget]) -> Option<FocusTarget> {
        let enabled = enabled_targets(targets);
        if enabled.is_empty() {
            return None;
        }

        let index = self
            .current
            .as_ref()
            .and_then(|current| {
                enabled
                    .iter()
                    .position(|target| same_focus(target, current))
            })
            .map(|index| {
                if index == 0 {
                    enabled.len() - 1
                } else {
                    index - 1
                }
            })
            .unwrap_or(0);

        Some(enabled[index].clone())
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
}

fn enabled_targets(targets: &[FocusTarget]) -> Vec<&FocusTarget> {
    targets.iter().filter(|target| target.enabled).collect()
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

fn nearest_enabled_target(
    current: Option<&FocusTarget>,
    targets: &[FocusTarget],
) -> Option<FocusTarget> {
    let Some(current) = current else {
        return targets.iter().find(|target| target.enabled).cloned();
    };
    targets
        .iter()
        .filter(|target| target.enabled)
        .min_by_key(|target| focus_distance(current, target))
        .cloned()
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
            },
            FocusTarget {
                id: FocusId::new("input"),
                path: TreePath::from_keys([ChildKey::new("two")]),
                area: Rect::default(),
                enabled: true,
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
            },
            FocusTarget {
                id: FocusId::new("input"),
                path: TreePath::from_keys([ChildKey::new("shared")]),
                area: Rect::default(),
                enabled: true,
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
            },
            FocusTarget {
                id: FocusId::new("input"),
                path: TreePath::from_keys([ChildKey::new("two")]),
                area: Rect::default(),
                enabled: true,
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
}
