use tuicore::{Animated, FocusChain, Panel, Tabs, TabsVariant};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, NoUserEvent};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::ratatui::Frame;
use tuirealm::ratatui::layout::{Constraint, Direction, Layout, Rect};
use tuirealm::ratatui::widgets::Paragraph;
use tuirealm::state::State;

use crate::shared::Msg;

pub struct TabsPreview {
    panel: Panel,
    minimal: Tabs<Msg, NoUserEvent>,
    underline: Tabs<Msg, NoUserEvent>,
    boxed: Tabs<Msg, NoUserEvent>,
    focus: FocusChain<TabsFocus>,
    app_focused: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TabsFocus {
    Outer,
    Minimal,
    Underline,
    Boxed,
}

const TABS_FOCUS_ORDER: [TabsFocus; 4] = [
    TabsFocus::Outer,
    TabsFocus::Minimal,
    TabsFocus::Underline,
    TabsFocus::Boxed,
];

impl TabsPreview {
    pub fn new() -> Self {
        Self {
            panel: Panel::new(),
            minimal: Tabs::default().variant(TabsVariant::Minimal),
            underline: Tabs::default().variant(TabsVariant::Underline),
            boxed: Tabs::default().variant(TabsVariant::Boxed),
            focus: FocusChain::new(TabsFocus::Outer),
            app_focused: false,
        }
    }

    fn example_areas(area: Rect) -> [Rect; 3] {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(33),
                Constraint::Percentage(33),
                Constraint::Percentage(34),
            ])
            .areas(area)
    }

    fn focus_next(&mut self) -> Option<Msg> {
        self.focus
            .next(&TABS_FOCUS_ORDER)
            .map(|_| {
                self.sync_focus();
                Msg::Redraw
            })
            .or(Some(Msg::FocusList))
    }

    fn focus_previous(&mut self) -> Option<Msg> {
        self.focus
            .previous(&TABS_FOCUS_ORDER)
            .map(|_| {
                self.sync_focus();
                Msg::Redraw
            })
            .or(Some(Msg::FocusList))
    }

    fn sync_focus(&mut self) {
        let focus = self.focus.current();
        self.panel
            .attr(Attribute::Focus, AttrValue::Flag(focus == TabsFocus::Outer));
        self.minimal.attr(
            Attribute::Focus,
            AttrValue::Flag(focus == TabsFocus::Minimal),
        );
        self.underline.attr(
            Attribute::Focus,
            AttrValue::Flag(focus == TabsFocus::Underline),
        );
        self.boxed
            .attr(Attribute::Focus, AttrValue::Flag(focus == TabsFocus::Boxed));
    }

    fn clear_focus(&mut self) {
        self.panel.attr(Attribute::Focus, AttrValue::Flag(false));
        self.minimal.attr(Attribute::Focus, AttrValue::Flag(false));
        self.underline
            .attr(Attribute::Focus, AttrValue::Flag(false));
        self.boxed.attr(Attribute::Focus, AttrValue::Flag(false));
    }

    fn focused_tabs_on(&mut self, event: &Event<NoUserEvent>) -> Option<Msg> {
        match self.focus.current() {
            TabsFocus::Outer => None,
            TabsFocus::Minimal => self.minimal.on(event),
            TabsFocus::Underline => self.underline.on(event),
            TabsFocus::Boxed => self.boxed.on(event),
        }
    }
}

impl Component for TabsPreview {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        self.panel.view(frame, area);
        let [minimal, underline, boxed] = Self::example_areas(Panel::inner_area(area));
        let [minimal_label, minimal_tabs] = labeled_area(minimal);
        let [underline_label, underline_tabs] = labeled_area(underline);
        let [boxed_label, boxed_tabs] = labeled_area(boxed);

        frame.render_widget(Paragraph::new("Style 1: minimal"), minimal_label);
        self.minimal.view(frame, minimal_tabs);
        frame.render_widget(Paragraph::new("Style 2: underline"), underline_label);
        self.underline.view(frame, underline_tabs);
        frame.render_widget(Paragraph::new("Style 3: boxed"), boxed_label);
        self.boxed.view(frame, boxed_tabs);
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        match attr {
            Attribute::Focus => Some(QueryResult::Owned(AttrValue::Flag(self.app_focused))),
            _ => self.panel.query(attr),
        }
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        match (attr, value) {
            (Attribute::Focus, AttrValue::Flag(true)) => {
                self.app_focused = true;
                self.focus.reset(TabsFocus::Outer);
                self.sync_focus();
            }
            (Attribute::Focus, AttrValue::Flag(false)) => {
                self.app_focused = false;
                self.clear_focus();
            }
            (attr, value) => {
                self.panel.attr(attr, value.clone());
                self.minimal.attr(attr, value.clone());
                self.underline.attr(attr, value.clone());
                self.boxed.attr(attr, value);
            }
        }
    }

    fn state(&self) -> State {
        self.panel.state()
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        CmdResult::Invalid(cmd)
    }
}

fn labeled_area(area: Rect) -> [Rect; 2] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Fill(1)])
        .areas(area)
}

impl AppComponent<Msg, NoUserEvent> for TabsPreview {
    fn on(&mut self, event: &Event<NoUserEvent>) -> Option<Msg> {
        match event {
            Event::Keyboard(KeyEvent { code: Key::Esc, .. })
            | Event::Keyboard(KeyEvent {
                code: Key::Char('q'),
                ..
            }) => Some(Msg::Quit),
            Event::Keyboard(KeyEvent { code: Key::Tab, .. }) => self.focus_next(),
            Event::Keyboard(KeyEvent {
                code: Key::BackTab, ..
            }) => self.focus_previous(),
            Event::Tick => {
                let settings = tuicore::animation_settings();
                let dt = settings.frame_duration();
                let tick = self
                    .panel
                    .tick(dt, settings)
                    .merge(self.minimal.tick(dt, settings))
                    .merge(self.underline.tick(dt, settings))
                    .merge(self.boxed.tick(dt, settings));
                (tick.changed || tick.active).then_some(Msg::Redraw)
            }
            _ => self.focused_tabs_on(event).or(Some(Msg::Redraw)),
        }
    }
}
