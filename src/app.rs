use std::hash::Hash;

use tuirealm::application::{Application, ApplicationResult, PollStrategy};
use tuirealm::component::AppComponent;
use tuirealm::event::NoUserEvent;
use tuirealm::listener::EventListenerCfg;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::ratatui::Frame;
use tuirealm::ratatui::layout::Rect;
use tuirealm::state::State;
use tuirealm::subscription::Sub;

use crate::{animation_settings, animation_tick_subscription};

pub struct TuicoreApp<ComponentId, Msg, UserEvent>
where
    ComponentId: Eq + PartialEq + Clone + Hash,
    Msg: PartialEq,
    UserEvent: Eq + PartialEq + Clone + Send + 'static,
{
    inner: Application<ComponentId, Msg, UserEvent>,
}

impl<ComponentId, Msg> TuicoreApp<ComponentId, Msg, NoUserEvent>
where
    ComponentId: Eq + PartialEq + Clone + Hash,
    Msg: PartialEq + 'static,
{
    pub fn new() -> Self {
        let animation = animation_settings();
        let frame_duration = animation.frame_duration();
        let mut listener = EventListenerCfg::default().crossterm_input_listener(frame_duration, 3);
        if animation.enabled {
            listener = listener.tick_interval(frame_duration);
        }

        Self::with_listener(listener)
    }
}

impl<ComponentId, Msg, UserEvent> TuicoreApp<ComponentId, Msg, UserEvent>
where
    ComponentId: Eq + PartialEq + Clone + Hash,
    Msg: PartialEq + 'static,
    UserEvent: Eq + PartialEq + Clone + Send + 'static,
{
    pub fn with_listener(listener: EventListenerCfg<UserEvent>) -> Self {
        Self {
            inner: Application::init(listener),
        }
    }

    pub fn mount(
        &mut self,
        id: ComponentId,
        component: impl AppComponent<Msg, UserEvent> + 'static,
    ) -> ApplicationResult<()> {
        self.inner.mount(
            id.clone(),
            Box::new(component),
            vec![animation_tick_subscription(id)],
        )
    }

    pub fn mount_with_subscriptions(
        &mut self,
        id: ComponentId,
        component: impl AppComponent<Msg, UserEvent> + 'static,
        mut subscriptions: Vec<Sub<ComponentId, UserEvent>>,
    ) -> ApplicationResult<()> {
        // Tuirealm deduplicates matching subscriptions per component/event.
        // Put tuicore animation tick first so auto-wired animation ticks win collisions.
        subscriptions.insert(0, animation_tick_subscription(id.clone()));
        self.inner.mount(id, Box::new(component), subscriptions)
    }

    pub fn tick(&mut self, strategy: PollStrategy) -> ApplicationResult<Vec<Msg>> {
        self.inner.tick(strategy)
    }

    pub fn view(&mut self, id: &ComponentId, frame: &mut Frame, area: Rect) {
        self.inner.view(id, frame, area);
    }

    pub fn active(&mut self, id: &ComponentId) -> ApplicationResult<()> {
        self.inner.active(id)
    }

    pub fn attr(
        &mut self,
        id: &ComponentId,
        attr: Attribute,
        value: AttrValue,
    ) -> ApplicationResult<()> {
        self.inner.attr(id, attr, value)
    }

    pub fn query<'a>(
        &'a self,
        id: &ComponentId,
        query: Attribute,
    ) -> ApplicationResult<Option<QueryResult<'a>>> {
        self.inner.query(id, query)
    }

    pub fn state(&self, id: &ComponentId) -> ApplicationResult<State> {
        self.inner.state(id)
    }

    pub fn focus(&self) -> Option<&ComponentId> {
        self.inner.focus()
    }

    pub fn inner(&self) -> &Application<ComponentId, Msg, UserEvent> {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut Application<ComponentId, Msg, UserEvent> {
        &mut self.inner
    }
}

impl<ComponentId, Msg> Default for TuicoreApp<ComponentId, Msg, NoUserEvent>
where
    ComponentId: Eq + PartialEq + Clone + Hash,
    Msg: PartialEq + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}
