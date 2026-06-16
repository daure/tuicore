use std::{error::Error, fmt};

use crate::{FocusKeyBindings, KeyEvent, keybindings};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusChain<T> {
    current: T,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusRouter<T> {
    order: Vec<T>,
    current: usize,
    wrap: FocusWrap,
    keys: FocusKeyBindings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusWrap {
    Wrap,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDirection {
    Next,
    Previous,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusOutcome<T> {
    Ignored,
    Moved { from: T, to: T },
    Boundary { at: T, direction: FocusDirection },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusRouterError {
    EmptyOrder,
    DuplicateTarget,
    UnknownTarget,
}

impl<T> FocusChain<T>
where
    T: Copy + Eq,
{
    pub fn new(current: T) -> Self {
        Self { current }
    }

    pub fn current(&self) -> T {
        self.current
    }

    pub fn reset(&mut self, current: T) {
        self.current = current;
    }

    pub fn next(&mut self, order: &[T]) -> Option<T> {
        let index = order.iter().position(|item| *item == self.current)?;
        let next = order.get(index + 1).copied()?;
        self.current = next;
        Some(next)
    }

    pub fn previous(&mut self, order: &[T]) -> Option<T> {
        let index = order.iter().position(|item| *item == self.current)?;
        let previous = index
            .checked_sub(1)
            .and_then(|index| order.get(index))
            .copied()?;
        self.current = previous;
        Some(previous)
    }
}

impl<T> FocusRouter<T>
where
    T: Clone + Eq,
{
    pub fn try_new(order: impl IntoIterator<Item = T>) -> Result<Self, FocusRouterError> {
        let order = order.into_iter().collect::<Vec<_>>();
        if order.is_empty() {
            return Err(FocusRouterError::EmptyOrder);
        }
        if has_duplicates(&order) {
            return Err(FocusRouterError::DuplicateTarget);
        }
        Ok(Self {
            order,
            current: 0,
            wrap: FocusWrap::Stop,
            keys: keybindings().focus().clone(),
        })
    }

    pub fn with_initial(mut self, target: &T) -> Result<Self, FocusRouterError> {
        self.current = self.index_of(target)?;
        Ok(self)
    }

    pub fn with_wrap(mut self, wrap: FocusWrap) -> Self {
        self.wrap = wrap;
        self
    }

    pub fn with_keys(mut self, keys: FocusKeyBindings) -> Self {
        self.keys = keys;
        self
    }

    pub fn current(&self) -> &T {
        &self.order[self.current]
    }

    pub fn is_current(&self, target: &T) -> bool {
        self.current() == target
    }

    pub fn focus(&mut self, target: &T) -> Result<FocusOutcome<T>, FocusRouterError> {
        let next = self.index_of(target)?;
        Ok(self.move_to(next))
    }

    pub fn focus_next(&mut self) -> FocusOutcome<T> {
        match self.current + 1 {
            next if next < self.order.len() => self.move_to(next),
            _ if self.wrap == FocusWrap::Wrap => self.move_to(0),
            _ => FocusOutcome::Boundary {
                at: self.current().clone(),
                direction: FocusDirection::Next,
            },
        }
    }

    pub fn focus_previous(&mut self) -> FocusOutcome<T> {
        if let Some(previous) = self.current.checked_sub(1) {
            self.move_to(previous)
        } else if self.wrap == FocusWrap::Wrap {
            self.move_to(self.order.len() - 1)
        } else {
            FocusOutcome::Boundary {
                at: self.current().clone(),
                direction: FocusDirection::Previous,
            }
        }
    }

    pub fn on_key(&mut self, key: impl Into<KeyEvent>) -> FocusOutcome<T> {
        let key = key.into();
        if self.keys.next_matches(key) {
            self.focus_next()
        } else if self.keys.previous_matches(key) {
            self.focus_previous()
        } else {
            FocusOutcome::Ignored
        }
    }

    fn index_of(&self, target: &T) -> Result<usize, FocusRouterError> {
        self.order
            .iter()
            .position(|item| item == target)
            .ok_or(FocusRouterError::UnknownTarget)
    }

    fn move_to(&mut self, next: usize) -> FocusOutcome<T> {
        let from = self.current().clone();
        self.current = next;
        let to = self.current().clone();
        if from == to {
            FocusOutcome::Ignored
        } else {
            FocusOutcome::Moved { from, to }
        }
    }
}

impl<T> FocusOutcome<T> {
    pub fn moved(self) -> Option<(T, T)> {
        match self {
            Self::Moved { from, to } => Some((from, to)),
            _ => None,
        }
    }
}

impl<T> Default for FocusRouter<T>
where
    T: Clone + Eq + Default,
{
    fn default() -> Self {
        Self {
            order: vec![T::default()],
            current: 0,
            wrap: FocusWrap::Stop,
            keys: FocusKeyBindings::default(),
        }
    }
}

impl fmt::Display for FocusRouterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyOrder => f.write_str("focus order cannot be empty"),
            Self::DuplicateTarget => f.write_str("focus order contains a duplicate target"),
            Self::UnknownTarget => f.write_str("focus target is not in focus order"),
        }
    }
}

impl Error for FocusRouterError {}

fn has_duplicates<T: Eq>(items: &[T]) -> bool {
    items
        .iter()
        .enumerate()
        .any(|(index, item)| items.iter().skip(index + 1).any(|other| other == item))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Key, KeyModifiers, KeySpec};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Target {
        First,
        Second,
        Third,
    }

    const ORDER: [Target; 3] = [Target::First, Target::Second, Target::Third];

    #[test]
    fn next_moves_through_order_until_boundary() {
        let mut focus = FocusChain::new(Target::First);

        assert_eq!(focus.next(&ORDER), Some(Target::Second));
        assert_eq!(focus.current(), Target::Second);
        assert_eq!(focus.next(&ORDER), Some(Target::Third));
        assert_eq!(focus.next(&ORDER), None);
        assert_eq!(focus.current(), Target::Third);
    }

    #[test]
    fn previous_moves_through_order_until_boundary() {
        let mut focus = FocusChain::new(Target::Third);

        assert_eq!(focus.previous(&ORDER), Some(Target::Second));
        assert_eq!(focus.current(), Target::Second);
        assert_eq!(focus.previous(&ORDER), Some(Target::First));
        assert_eq!(focus.previous(&ORDER), None);
        assert_eq!(focus.current(), Target::First);
    }

    #[test]
    fn missing_current_is_boundary() {
        let mut focus = FocusChain::new(Target::Third);

        assert_eq!(focus.next(&ORDER[..2]), None);
        assert_eq!(focus.previous(&ORDER[..2]), None);
        assert_eq!(focus.current(), Target::Third);
    }

    #[test]
    fn router_moves_and_wraps() {
        let mut router = FocusRouter::try_new([Target::First, Target::Second])
            .unwrap()
            .with_wrap(FocusWrap::Wrap);

        assert_eq!(router.current(), &Target::First);
        assert_eq!(
            router.focus_next(),
            FocusOutcome::Moved {
                from: Target::First,
                to: Target::Second
            }
        );
        assert_eq!(
            router.focus_next(),
            FocusOutcome::Moved {
                from: Target::Second,
                to: Target::First
            }
        );
    }

    #[test]
    fn router_reports_boundary_when_stopped() {
        let mut router = FocusRouter::try_new([Target::First]).unwrap();

        assert_eq!(
            router.focus_previous(),
            FocusOutcome::Boundary {
                at: Target::First,
                direction: FocusDirection::Previous
            }
        );
    }

    #[test]
    fn router_rejects_empty_and_duplicates() {
        assert_eq!(
            FocusRouter::<Target>::try_new([]),
            Err(FocusRouterError::EmptyOrder)
        );
        assert_eq!(
            FocusRouter::try_new([Target::First, Target::First]),
            Err(FocusRouterError::DuplicateTarget)
        );
    }

    #[test]
    fn router_accepts_custom_focus_bindings() {
        let mut router = FocusRouter::try_new([Target::First, Target::Second])
            .unwrap()
            .with_keys(FocusKeyBindings::new().with_next([KeySpec::plain('l')]));

        assert_eq!(
            router.on_key(KeyEvent {
                code: Key::Char('l'),
                modifiers: KeyModifiers::NONE,
            }),
            FocusOutcome::Moved {
                from: Target::First,
                to: Target::Second,
            }
        );
    }
}
