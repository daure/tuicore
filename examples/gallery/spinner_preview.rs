use tuicore::{Animated, Panel, Spinner};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, NoUserEvent};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::ratatui::Frame;
use tuirealm::ratatui::layout::{Constraint, Direction, Layout, Rect};
use tuirealm::ratatui::text::{Line, Span};
use tuirealm::ratatui::widgets::Paragraph;
use tuirealm::state::State;

use crate::shared::{Msg, focus_list_key};

pub struct SpinnerPreview {
    spinner: Spinner,
    panel: Panel,
    event_area: Rect,
}

impl SpinnerPreview {
    pub fn new() -> Self {
        Self {
            spinner: Spinner::new(),
            panel: Panel::new().top_left("Spinner"),
            event_area: Rect::default(),
        }
    }
}

impl Component for SpinnerPreview {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        // Render the panel outline
        self.panel.render(frame, area);

        let inner = Panel::inner_area(area);
        if inner.is_empty() {
            return;
        }

        // Split inner area for text and spinner
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5), // Text content
                Constraint::Min(3),    // Spinner centered below
            ])
            .split(inner);

        // Render info text
        let text = vec![
            Line::from(Span::raw(
                "Spinners are animated components that advance frames on ticks.",
            )),
            Line::from(Span::raw(
                "They respect global animation settings. Disabling animations will freeze them.",
            )),
            Line::from(Span::raw("")),
            Line::from(Span::raw(
                "This is the standard Braille dots loading spinner, cycling clockwise.",
            )),
        ];
        frame.render_widget(Paragraph::new(text), chunks[0]);

        // Render the spinner centered in the remaining space
        self.spinner.view(frame, chunks[1]);
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        self.panel.query(attr)
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        match (attr, value) {
            (Attribute::Width, AttrValue::Size(width)) => self.event_area.width = width,
            (Attribute::Height, AttrValue::Size(height)) => self.event_area.height = height,
            (attr, value) => self.panel.attr(attr, value),
        }
    }

    fn state(&self) -> State {
        self.panel.state()
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        self.panel.perform(cmd)
    }
}

impl AppComponent<Msg, NoUserEvent> for SpinnerPreview {
    fn on(&mut self, event: &Event<NoUserEvent>) -> Option<Msg> {
        match event {
            Event::Keyboard(KeyEvent { code: Key::Esc, .. })
            | Event::Keyboard(KeyEvent {
                code: Key::Char('q'),
                ..
            }) => Some(Msg::Quit),
            Event::Keyboard(key) if focus_list_key(*key) => Some(Msg::FocusList),
            Event::Keyboard(key) => self
                .panel
                .on_key(*key, self.event_area, tuicore::animation_settings())
                .needs_redraw()
                .then_some(Msg::Redraw),
            Event::Tick => {
                let settings = tuicore::animation_settings();
                let dt = settings.frame_duration();

                let res1 = self.spinner.tick(dt, settings);
                let res2 = self.panel.tick(dt, settings);

                let changed = res1.changed || res2.changed;

                changed.then_some(Msg::Redraw)
            }
            _ => Some(Msg::Redraw),
        }
    }
}
