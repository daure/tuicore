#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusChain<T> {
    current: T,
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
