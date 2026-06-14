use tuicore::{Animated, BorderKind, FocusChain, Panel, PanelVariant};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, NoUserEvent};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::ratatui::Frame;
use tuirealm::ratatui::layout::{Constraint, Direction, Layout, Rect};
use tuirealm::state::State;

use crate::shared::Msg;

pub struct PanelPreview {
    panel: Panel,
    no_title: Panel,
    left_title: Panel,
    right_title: Panel,
    both_titles: Panel,
    standard: Panel,
    inset: Panel,
    focus: FocusChain<PanelFocus>,
    app_focused: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PanelFocus {
    Outer,
    NoTitle,
    LeftTitle,
    RightTitle,
    BothTitles,
    Standard,
    Inset,
}

const PANEL_FOCUS_ORDER: [PanelFocus; 7] = [
    PanelFocus::Outer,
    PanelFocus::NoTitle,
    PanelFocus::LeftTitle,
    PanelFocus::RightTitle,
    PanelFocus::BothTitles,
    PanelFocus::Standard,
    PanelFocus::Inset,
];

impl PanelPreview {
    pub fn new() -> Self {
        Self {
            panel: Panel::new(),
            no_title: Panel::new().content(["No title", "Outer gallery panel is titleless too."]),
            left_title: Panel::new()
                .top_left("Left")
                .content(["Top-left title slot"]),
            right_title: Panel::new()
                .top_right("Right")
                .content(["Top-right title slot"]),
            both_titles: Panel::new()
                .top_left("Left")
                .top_right("Right")
                .content(["Both title slots"]),
            standard: Panel::new()
                .top_left("Style 1")
                .border(BorderKind::Rounded)
                .content(["Standard overlaid title"]),
            inset: Panel::new()
                .top_left("Processes")
                .border(BorderKind::Plain)
                .variant(PanelVariant::InsetTitle)
                .content(["✖ No processes running"]),
            focus: FocusChain::new(PanelFocus::Outer),
            app_focused: false,
        }
    }

    fn render_examples(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(34),
                Constraint::Percentage(33),
                Constraint::Percentage(33),
            ])
            .split(area);
        let top = two_columns(rows[0]);
        let middle = two_columns(rows[1]);
        let bottom = two_columns(rows[2]);

        self.no_title.render(frame, top[0]);
        self.left_title.render(frame, top[1]);
        self.right_title.render(frame, middle[0]);
        self.both_titles.render(frame, middle[1]);
        self.standard.render(frame, bottom[0]);
        self.inset.render(frame, bottom[1]);
    }

    fn focus_next(&mut self) -> Option<Msg> {
        self.focus
            .next(&PANEL_FOCUS_ORDER)
            .map(|_| {
                self.sync_focus();
                Msg::Redraw
            })
            .or(Some(Msg::FocusList))
    }

    fn focus_previous(&mut self) -> Option<Msg> {
        self.focus
            .previous(&PANEL_FOCUS_ORDER)
            .map(|_| {
                self.sync_focus();
                Msg::Redraw
            })
            .or(Some(Msg::FocusList))
    }

    fn sync_focus(&mut self) {
        let focus = self.focus.current();
        self.panel.attr(
            Attribute::Focus,
            AttrValue::Flag(focus == PanelFocus::Outer),
        );
        self.no_title.attr(
            Attribute::Focus,
            AttrValue::Flag(focus == PanelFocus::NoTitle),
        );
        self.left_title.attr(
            Attribute::Focus,
            AttrValue::Flag(focus == PanelFocus::LeftTitle),
        );
        self.right_title.attr(
            Attribute::Focus,
            AttrValue::Flag(focus == PanelFocus::RightTitle),
        );
        self.both_titles.attr(
            Attribute::Focus,
            AttrValue::Flag(focus == PanelFocus::BothTitles),
        );
        self.standard.attr(
            Attribute::Focus,
            AttrValue::Flag(focus == PanelFocus::Standard),
        );
        self.inset.attr(
            Attribute::Focus,
            AttrValue::Flag(focus == PanelFocus::Inset),
        );
    }

    fn clear_focus(&mut self) {
        self.panel.attr(Attribute::Focus, AttrValue::Flag(false));
        self.no_title.attr(Attribute::Focus, AttrValue::Flag(false));
        self.left_title
            .attr(Attribute::Focus, AttrValue::Flag(false));
        self.right_title
            .attr(Attribute::Focus, AttrValue::Flag(false));
        self.both_titles
            .attr(Attribute::Focus, AttrValue::Flag(false));
        self.standard.attr(Attribute::Focus, AttrValue::Flag(false));
        self.inset.attr(Attribute::Focus, AttrValue::Flag(false));
    }
}

impl Component for PanelPreview {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        self.panel.view(frame, area);
        self.render_examples(frame, Panel::inner_area(area));
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
                self.focus.reset(PanelFocus::Outer);
                self.sync_focus();
            }
            (Attribute::Focus, AttrValue::Flag(false)) => {
                self.app_focused = false;
                self.clear_focus();
            }
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

impl AppComponent<Msg, NoUserEvent> for PanelPreview {
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
                self.panel
                    .tick(dt, settings)
                    .merge(self.no_title.tick(dt, settings))
                    .merge(self.left_title.tick(dt, settings))
                    .merge(self.right_title.tick(dt, settings))
                    .merge(self.both_titles.tick(dt, settings))
                    .merge(self.standard.tick(dt, settings))
                    .merge(self.inset.tick(dt, settings))
                    .changed
                    .then_some(Msg::Redraw)
            }
            _ => Some(Msg::Redraw),
        }
    }
}

fn two_columns(area: Rect) -> [Rect; 2] {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .areas(area)
}
