use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::marker::PhantomData;
use std::rc::Rc;

pub trait StoreLike {
    type Event;
    type State;

    fn state(&self) -> &Self::State;
    fn dispatch(&mut self, event: Self::Event) -> DispatchOutcome;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispatchOutcome {
    /// Reducer changed app state.
    pub changed: bool,
    /// Caller should request redraw.
    pub redraw: bool,
    /// Caller should request layout before redraw.
    pub layout: bool,
}

impl DispatchOutcome {
    /// Creates a custom outcome. Convenience constructors keep the usual invariant that state
    /// changes request redraws, but custom integration code may need explicit combinations.
    pub const fn new(changed: bool, redraw: bool, layout: bool) -> Self {
        Self {
            changed,
            redraw,
            layout,
        }
    }

    pub const fn unchanged() -> Self {
        Self::new(false, false, false)
    }

    pub const fn changed() -> Self {
        Self::new(true, true, false)
    }

    pub const fn redraw() -> Self {
        Self::new(false, true, false)
    }

    pub const fn layout() -> Self {
        Self::new(true, true, true)
    }
}

pub trait StoreObserver<E> {
    fn before_dispatch(&mut self, event: &E, sequence: u64);
    fn after_dispatch(&mut self, sequence: u64, outcome: DispatchOutcome);
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopStoreObserver;

impl<E> StoreObserver<E> for NoopStoreObserver {
    fn before_dispatch(&mut self, _event: &E, _sequence: u64) {}

    fn after_dispatch(&mut self, _sequence: u64, _outcome: DispatchOutcome) {}
}

pub struct Store<S, E, R, O = NoopStoreObserver> {
    state: S,
    reducer: R,
    observer: O,
    sequence: u64,
    _event: PhantomData<fn(E)>,
}

impl<S, E, R> Store<S, E, R>
where
    R: FnMut(&mut S, E) -> DispatchOutcome,
{
    pub fn new(state: S, reducer: R) -> Self {
        Self {
            state,
            reducer,
            observer: NoopStoreObserver,
            sequence: 0,
            _event: PhantomData,
        }
    }
}

impl<S, E, R, O> Store<S, E, R, O>
where
    R: FnMut(&mut S, E) -> DispatchOutcome,
    O: StoreObserver<E>,
{
    pub fn state(&self) -> &S {
        &self.state
    }

    pub fn dispatch(&mut self, event: E) -> DispatchOutcome {
        let sequence = self.next_sequence();
        self.observer.before_dispatch(&event, sequence);
        let outcome = (self.reducer)(&mut self.state, event);
        self.observer.after_dispatch(sequence, outcome);
        outcome
    }

    pub fn with_observer<NextObserver>(self, observer: NextObserver) -> Store<S, E, R, NextObserver>
    where
        NextObserver: StoreObserver<E>,
    {
        Store {
            state: self.state,
            reducer: self.reducer,
            observer,
            sequence: self.sequence,
            _event: PhantomData,
        }
    }

    fn next_sequence(&mut self) -> u64 {
        self.sequence = self
            .sequence
            .checked_add(1)
            .expect("store dispatch sequence overflowed");
        self.sequence
    }
}

impl<S, E, R, O> StoreLike for Store<S, E, R, O>
where
    R: FnMut(&mut S, E) -> DispatchOutcome,
    O: StoreObserver<E>,
{
    type Event = E;
    type State = S;

    fn state(&self) -> &Self::State {
        self.state()
    }

    fn dispatch(&mut self, event: Self::Event) -> DispatchOutcome {
        self.dispatch(event)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreLogPhase {
    Received,
    Handled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreLogEntry {
    pub sequence: u64,
    pub event_label: String,
    pub phase: StoreLogPhase,
    pub outcome: Option<DispatchOutcome>,
}

#[derive(Debug, Clone)]
pub struct EventLog {
    entries: Rc<RefCell<VecDeque<StoreLogEntry>>>,
    capacity: usize,
}

impl EventLog {
    pub fn bounded(capacity: usize) -> Self {
        Self {
            entries: Rc::new(RefCell::new(VecDeque::with_capacity(capacity))),
            capacity,
        }
    }

    pub fn observer<E>(&self) -> EventLogObserver<E, fn(&E) -> String> {
        self.observer_with_label(type_label::<E>)
    }

    pub fn observer_with_label<E, F>(&self, label: F) -> EventLogObserver<E, F>
    where
        F: FnMut(&E) -> String,
    {
        EventLogObserver {
            log: self.clone(),
            label,
            pending_labels: HashMap::new(),
            _event: PhantomData,
        }
    }

    pub fn entries(&self) -> Vec<StoreLogEntry> {
        self.entries.borrow().iter().cloned().collect()
    }

    fn push(&self, entry: StoreLogEntry) {
        if self.capacity == 0 {
            return;
        }

        let mut entries = self.entries.borrow_mut();
        while entries.len() >= self.capacity {
            entries.pop_front();
        }
        entries.push_back(entry);
    }
}

pub struct EventLogObserver<E, F> {
    log: EventLog,
    label: F,
    pending_labels: HashMap<u64, String>,
    _event: PhantomData<fn(&E)>,
}

impl<E, F> StoreObserver<E> for EventLogObserver<E, F>
where
    F: FnMut(&E) -> String,
{
    fn before_dispatch(&mut self, event: &E, sequence: u64) {
        let event_label = (self.label)(event);
        self.pending_labels.insert(sequence, event_label.clone());
        self.log.push(StoreLogEntry {
            sequence,
            event_label,
            phase: StoreLogPhase::Received,
            outcome: None,
        });
    }

    fn after_dispatch(&mut self, sequence: u64, outcome: DispatchOutcome) {
        let event_label = self
            .pending_labels
            .remove(&sequence)
            .unwrap_or_else(type_name_label::<E>);
        self.log.push(StoreLogEntry {
            sequence,
            event_label,
            phase: StoreLogPhase::Handled,
            outcome: Some(outcome),
        });
    }
}

fn type_label<E>(_: &E) -> String {
    type_name_label::<E>()
}

fn type_name_label<E>() -> String {
    std::any::type_name::<E>().to_string()
}

#[derive(Debug, Clone, PartialEq)]
pub enum InspectValue {
    Object(Vec<InspectField>),
    List(Vec<InspectValue>),
    String(String),
    Number(String),
    Bool(bool),
    Null,
}

impl InspectValue {
    pub fn object(fields: impl IntoIterator<Item = InspectField>) -> Self {
        Self::Object(fields.into_iter().collect())
    }

    pub fn list(values: impl IntoIterator<Item = InspectValue>) -> Self {
        Self::List(values.into_iter().collect())
    }

    pub fn string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }

    pub fn number(value: impl ToString) -> Self {
        Self::Number(value.to_string())
    }

    pub const fn bool(value: bool) -> Self {
        Self::Bool(value)
    }

    pub const fn null() -> Self {
        Self::Null
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InspectField {
    pub name: String,
    pub value: InspectValue,
}

impl InspectField {
    pub fn new(name: impl Into<String>, value: InspectValue) -> Self {
        Self {
            name: name.into(),
            value,
        }
    }
}

pub trait StateInspect {
    fn inspect(&self) -> InspectValue;
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::cell::RefCell;
    use std::rc::Rc;

    enum CounterEvent {
        Add(i32),
    }

    #[test]
    fn dispatch_mutates_state_and_returns_outcome() {
        let mut store = Store::new(0, |state, event| {
            let CounterEvent::Add(amount) = event;
            *state += amount;
            DispatchOutcome::changed()
        });

        let outcome = store.dispatch(CounterEvent::Add(3));

        assert_eq!(outcome, DispatchOutcome::changed());
        assert_eq!(*store.state(), 3);
    }

    struct SecretEvent {
        amount: i32,
    }

    struct RecordingObserver {
        calls: Rc<RefCell<Vec<(u64, &'static str, Option<DispatchOutcome>)>>>,
    }

    impl StoreObserver<SecretEvent> for RecordingObserver {
        fn before_dispatch(&mut self, event: &SecretEvent, sequence: u64) {
            self.calls.borrow_mut().push((
                sequence,
                "before",
                Some(DispatchOutcome::new(event.amount > 0, false, false)),
            ));
        }

        fn after_dispatch(&mut self, sequence: u64, outcome: DispatchOutcome) {
            self.calls
                .borrow_mut()
                .push((sequence, "after", Some(outcome)));
        }
    }

    #[test]
    fn observer_sees_same_sequence_without_event_clone_or_debug() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let observer = RecordingObserver {
            calls: Rc::clone(&calls),
        };
        let mut store = Store::new(0, |state, event: SecretEvent| {
            *state += event.amount;
            DispatchOutcome::layout()
        })
        .with_observer(observer);

        let outcome = store.dispatch(SecretEvent { amount: 2 });

        assert_eq!(outcome, DispatchOutcome::layout());
        assert_eq!(
            calls.borrow().as_slice(),
            &[
                (1, "before", Some(DispatchOutcome::new(true, false, false))),
                (1, "after", Some(DispatchOutcome::layout())),
            ]
        );
    }

    #[test]
    fn event_log_wraps_to_capacity_and_allows_zero_capacity() {
        let log = EventLog::bounded(3);
        let mut store = Store::new(0, |state, event: CounterEvent| {
            let CounterEvent::Add(amount) = event;
            *state += amount;
            DispatchOutcome::changed()
        })
        .with_observer(log.observer::<CounterEvent>());

        store.dispatch(CounterEvent::Add(1));
        store.dispatch(CounterEvent::Add(2));

        let entries = log.entries();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].sequence, 1);
        assert_eq!(entries[0].phase, StoreLogPhase::Handled);
        assert_eq!(entries[1].sequence, 2);
        assert_eq!(entries[1].phase, StoreLogPhase::Received);
        assert_eq!(entries[2].sequence, 2);
        assert_eq!(entries[2].phase, StoreLogPhase::Handled);

        let zero = EventLog::bounded(0);
        let mut store = Store::new((), |_, _: SecretEvent| DispatchOutcome::redraw())
            .with_observer(zero.observer::<SecretEvent>());
        store.dispatch(SecretEvent { amount: 1 });
        assert!(zero.entries().is_empty());
    }

    #[test]
    fn event_log_uses_privacy_safe_default_label() {
        let log = EventLog::bounded(2);
        let mut store = Store::new((), |_, _: SecretEvent| DispatchOutcome::unchanged())
            .with_observer(log.observer::<SecretEvent>());

        store.dispatch(SecretEvent { amount: 7 });

        let expected_label = std::any::type_name::<SecretEvent>();
        let entries = log.entries();
        assert_eq!(entries.len(), 2);
        assert!(
            entries
                .iter()
                .all(|entry| entry.event_label == expected_label)
        );
        assert!(entries.iter().all(|entry| !entry.event_label.contains('7')));
    }

    #[test]
    fn event_log_custom_label_formatter_is_opt_in() {
        let log = EventLog::bounded(2);
        let mut store = Store::new((), |_, _: SecretEvent| DispatchOutcome::redraw())
            .with_observer(log.observer_with_label::<SecretEvent, _>(|event| {
                format!("secret-event:{}", event.amount.signum())
            }));

        store.dispatch(SecretEvent { amount: -8 });

        let entries = log.entries();
        assert_eq!(entries.len(), 2);
        assert!(
            entries
                .iter()
                .all(|entry| entry.event_label == "secret-event:-1")
        );
    }

    #[test]
    fn inspect_value_constructors_build_state_tree() {
        let value = InspectValue::object([
            InspectField::new("count", InspectValue::number(3)),
            InspectField::new(
                "items",
                InspectValue::list([InspectValue::string("alpha"), InspectValue::null()]),
            ),
            InspectField::new("active", InspectValue::bool(true)),
        ]);

        assert_eq!(
            value,
            InspectValue::Object(vec![
                InspectField {
                    name: "count".to_string(),
                    value: InspectValue::Number("3".to_string()),
                },
                InspectField {
                    name: "items".to_string(),
                    value: InspectValue::List(vec![
                        InspectValue::String("alpha".to_string()),
                        InspectValue::Null,
                    ]),
                },
                InspectField {
                    name: "active".to_string(),
                    value: InspectValue::Bool(true),
                },
            ])
        );
    }
}
