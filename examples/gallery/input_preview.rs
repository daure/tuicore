use tuicore::{Animated, Panel, TextInput, TextareaInput};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, NoUserEvent};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::ratatui::Frame;
use tuirealm::ratatui::layout::{Constraint, Direction, Layout, Rect};
use tuirealm::ratatui::widgets::Paragraph;
use tuirealm::state::State;

use crate::shared::{Msg, focus_list_key};

pub struct TextInputPreview {
    panel: Panel,
    input: TextInput,
    focused: bool,
}

impl TextInputPreview {
    pub fn new() -> Self {
        Self {
            panel: Panel::new().top_left("Input: text"),
            input: TextInput::new()
                .placeholder("Type one line...")
                .value("tuicore")
                .max_len(80),
            focused: false,
        }
    }
}

impl Component for TextInputPreview {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        self.panel.render(frame, area);
        let inner = Panel::inner_area(area);
        let [instructions, input] = input_layout(inner);
        frame.render_widget(
            Paragraph::new(
                "Type text. Ctrl+C clears. Enter submits. Esc cancels. Tab returns to list; q quits there.",
            ),
            instructions,
        );
        self.input.view(frame, input);
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        match attr {
            Attribute::Focus => Some(QueryResult::Owned(AttrValue::Flag(self.focused))),
            _ => self.panel.query(attr),
        }
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        match (attr, value) {
            (Attribute::Focus, AttrValue::Flag(focused)) => {
                self.focused = focused;
                self.panel.attr(Attribute::Focus, AttrValue::Flag(focused));
                self.input.attr(Attribute::Focus, AttrValue::Flag(focused));
            }
            (attr, value) => self.panel.attr(attr, value),
        }
    }

    fn state(&self) -> State {
        self.input.state()
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        self.input.perform(cmd)
    }
}

impl AppComponent<Msg, NoUserEvent> for TextInputPreview {
    fn on(&mut self, event: &Event<NoUserEvent>) -> Option<Msg> {
        match event {
            Event::Tick => tick_panel(&mut self.panel),
            Event::Keyboard(key) if focus_list_key(*key) => Some(Msg::FocusList),
            Event::Keyboard(key) => self
                .input
                .on_key(*key)
                .needs_redraw()
                .then_some(Msg::Redraw),
            _ => Some(Msg::Redraw),
        }
    }
}

pub struct TextareaInputPreview {
    panel: Panel,
    input: TextareaInput,
    focused: bool,
}

impl TextareaInputPreview {
    pub fn new() -> Self {
        Self {
            panel: Panel::new().top_left("Input: textarea"),
            input: TextareaInput::new()
                .placeholder("Write multiple lines...")
                .value("First line\nSecond line")
                .max_lines(8),
            focused: false,
        }
    }
}

impl Component for TextareaInputPreview {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        self.panel.render(frame, area);
        let inner = Panel::inner_area(area);
        let [instructions, input] = textarea_layout(inner);
        frame.render_widget(
            Paragraph::new(
                "Type text. Ctrl+C clears. Enter inserts newline. Ctrl+Enter or Ctrl+D submits. Esc cancels. Tab returns to list; q quits there.",
            ),
            instructions,
        );
        self.input.view(frame, input);
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        match attr {
            Attribute::Focus => Some(QueryResult::Owned(AttrValue::Flag(self.focused))),
            _ => self.panel.query(attr),
        }
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        match (attr, value) {
            (Attribute::Focus, AttrValue::Flag(focused)) => {
                self.focused = focused;
                self.panel.attr(Attribute::Focus, AttrValue::Flag(focused));
                self.input.attr(Attribute::Focus, AttrValue::Flag(focused));
            }
            (attr, value) => self.panel.attr(attr, value),
        }
    }

    fn state(&self) -> State {
        self.input.state()
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        self.input.perform(cmd)
    }
}

impl AppComponent<Msg, NoUserEvent> for TextareaInputPreview {
    fn on(&mut self, event: &Event<NoUserEvent>) -> Option<Msg> {
        match event {
            Event::Tick => tick_panel(&mut self.panel),
            Event::Keyboard(key) if focus_list_key(*key) => Some(Msg::FocusList),
            Event::Keyboard(key) => self
                .input
                .on_key(*key)
                .needs_redraw()
                .then_some(Msg::Redraw),
            _ => Some(Msg::Redraw),
        }
    }
}

fn input_layout(area: Rect) -> [Rect; 2] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Length(1)])
        .areas(area)
}

fn textarea_layout(area: Rect) -> [Rect; 2] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Fill(1)])
        .areas(area)
}

fn tick_panel(panel: &mut Panel) -> Option<Msg> {
    let settings = tuicore::animation_settings();
    panel
        .tick(settings.frame_duration(), settings)
        .changed
        .then_some(Msg::Redraw)
}
