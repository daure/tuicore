use tuicore::{Animated, DataView, Panel, TreeAdapter, TreeGlyphs};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, NoUserEvent};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::ratatui::Frame;
use tuirealm::ratatui::layout::Rect;
use tuirealm::state::State;

use crate::shared::{ComponentKind, Msg, focus_nav_message};

pub struct ComponentList {
    list: DataView<ComponentKind, ComponentKind>,
    focused: bool,
    list_area: Rect,
    panel: Panel,
}

impl ComponentList {
    pub fn new(components: Vec<ComponentKind>) -> Self {
        let list = DataView::list(
            components,
            |component| *component,
            |component| component.title().to_string(),
        )
        .tree(TreeAdapter::parent_id(|component: &ComponentKind| {
            component.parent()
        }))
        .tree_glyphs(TreeGlyphs::NERD_FONT)
        .expanded([ComponentKind::DataView]);
        Self {
            list,
            focused: false,
            list_area: Rect::default(),
            panel: Panel::new().top_left("Components"),
        }
    }
}

impl Component for ComponentList {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        self.panel.render(frame, area);
        self.list.render(frame, Panel::inner_area(area));
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        match attr {
            Attribute::Focus => Some(QueryResult::Owned(AttrValue::Flag(self.focused))),
            _ => None,
        }
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        match (attr, value) {
            (Attribute::Focus, AttrValue::Flag(focused)) => {
                self.focused = focused;
                self.panel.attr(Attribute::Focus, AttrValue::Flag(focused));
                self.list.attr(Attribute::Focus, AttrValue::Flag(focused));
            }
            (Attribute::Width, AttrValue::Size(width)) => self.list_area.width = width,
            (Attribute::Height, AttrValue::Size(height)) => self.list_area.height = height,
            _ => {}
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        CmdResult::Invalid(cmd)
    }
}

impl AppComponent<Msg, NoUserEvent> for ComponentList {
    fn on(&mut self, event: &Event<NoUserEvent>) -> Option<Msg> {
        match event {
            Event::Keyboard(KeyEvent { code: Key::Esc, .. })
            | Event::Keyboard(KeyEvent {
                code: Key::Char('q'),
                ..
            }) => Some(Msg::Quit),
            Event::Tick => {
                let settings = tuicore::animation_settings();
                let tick = self
                    .panel
                    .tick(settings.frame_duration(), settings)
                    .merge(self.list.tick(settings.frame_duration(), settings));
                tick.changed.then_some(Msg::Redraw)
            }
            Event::Keyboard(key) => {
                if let Some(msg) = focus_nav_message(*key) {
                    return Some(msg);
                }

                let outcome = self.list.on_key(*key, self.list_area);
                if outcome.needs_redraw() {
                    self.list
                        .highlighted_id()
                        .map(Msg::Selected)
                        .or(Some(Msg::Redraw))
                } else {
                    None
                }
            }
            _ => Some(Msg::Redraw),
        }
    }
}
