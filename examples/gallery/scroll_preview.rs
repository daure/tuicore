use std::time::Duration;

use tuicore::{
    Animated, AnimationSpec, Easing, Panel, ScrollAxes, ScrollBehavior, ScrollSize, ScrollState,
    line_width, theme,
};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, NoUserEvent};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::ratatui::Frame;
use tuirealm::ratatui::layout::Rect;
use tuirealm::ratatui::style::{Modifier, Style};
use tuirealm::ratatui::text::{Line, Span};
use tuirealm::ratatui::widgets::Paragraph;
use tuirealm::state::State;

use crate::shared::{ComponentKind, Msg, focus_list_key};

pub struct AnimatedScrollPreview {
    scroll: ScrollState,
    rows: Vec<String>,
    selected: usize,
    focused: bool,
    event_area: Rect,
}

impl AnimatedScrollPreview {
    pub fn new() -> Self {
        Self {
            scroll: ScrollState::from_preset(ScrollAxes::Both, tuicore::preset().scroll())
                .behavior(ScrollBehavior {
                    animation: AnimationSpec {
                        duration: Some(Duration::from_millis(150)),
                        easing: Some(Easing::EaseOutCubic),
                        enabled: None,
                    },
                    ..ScrollBehavior::default()
                }),
            rows: (0..100)
                .map(|index| {
                    let component = ComponentKind::ALL[index % ComponentKind::ALL.len()];
                    format!(
                        "{:03}  {}  — long horizontal content lane {:03} :: arrows ←/→ move sideways :: smooth scrollbars stay in sync :: café 🚀 漢字",
                        index + 1,
                        component.title(),
                        index + 1,
                    )
                })
                .collect(),
            selected: 0,
            focused: false,
            event_area: Rect::default(),
        }
    }

    fn content_size(&self) -> ScrollSize {
        ScrollSize::new(
            self.rows
                .iter()
                .map(|row| line_width(&Line::from(row.as_str())))
                .max()
                .unwrap_or(0),
            self.rows.len(),
        )
    }

    fn body_area(area: Rect) -> Rect {
        Panel::inner_area(area)
    }

    fn event_viewport(&self) -> ScrollSize {
        let body = Self::body_area(self.event_area);
        ScrollSize::from_area(self.scroll.layout(body, self.content_size()).viewport)
    }

    fn scroll_target_for_selection(&self, viewport: ScrollSize) -> usize {
        let height = viewport.height.max(1);
        let max_scroll = self.rows.len().saturating_sub(height);
        self.selected.saturating_sub(height / 2).min(max_scroll)
    }
}

impl Component for AnimatedScrollPreview {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let panel = Panel::new()
            .top_left("Scroll: animated")
            .top_right("animated demo")
            .focused(self.focused);
        panel.render(frame, area);

        let body = Self::body_area(area);
        let geometry = self.scroll.geometry(body, self.content_size());
        let offset = self.scroll.offset();
        let theme = theme();
        for (visible, (row_index, row)) in self
            .rows
            .iter()
            .enumerate()
            .skip(offset.y)
            .take(geometry.viewport.height)
            .enumerate()
        {
            let selected = row_index == self.selected;
            let y = geometry.layout.viewport.y.saturating_add(visible as u16);
            let x = geometry.layout.viewport.x;
            let prefix = if selected { "› " } else { "  " };
            let style = if selected {
                Style::default()
                    .fg(theme.selected_fg())
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(format!("{prefix}{row}"), style)))
                    .scroll((0, offset.x.min(u16::MAX as usize) as u16)),
                Rect::new(x, y, geometry.layout.viewport.width, 1),
            );
        }
        self.scroll
            .render_scrollbars(frame, geometry.layout, geometry.content, self.focused);
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        match attr {
            Attribute::Focus => Some(QueryResult::Owned(AttrValue::Flag(self.focused))),
            _ => None,
        }
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        match (attr, value) {
            (Attribute::Focus, AttrValue::Flag(focused)) => self.focused = focused,
            (Attribute::Width, AttrValue::Size(width)) => self.event_area.width = width,
            (Attribute::Height, AttrValue::Size(height)) => self.event_area.height = height,
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

impl AppComponent<Msg, NoUserEvent> for AnimatedScrollPreview {
    fn on(&mut self, event: &Event<NoUserEvent>) -> Option<Msg> {
        match event {
            Event::Keyboard(KeyEvent { code: Key::Esc, .. })
            | Event::Keyboard(KeyEvent {
                code: Key::Char('q'),
                ..
            }) => Some(Msg::Quit),
            Event::Keyboard(key) if focus_list_key(*key) => Some(Msg::FocusList),
            Event::Keyboard(key) => {
                let bindings = tuicore::keybindings();
                let viewport = self.event_viewport();
                let page = viewport.height.max(1);
                if bindings.line_left_matches(*key) || bindings.line_right_matches(*key) {
                    self.scroll.on_key(
                        *key,
                        viewport,
                        self.content_size(),
                        tuicore::animation_settings(),
                    );
                    return Some(Msg::Redraw);
                }

                let next_selected = if bindings.line_up_matches(*key) {
                    self.selected.saturating_sub(1)
                } else if bindings.line_down_matches(*key) {
                    (self.selected + 1).min(self.rows.len().saturating_sub(1))
                } else if bindings.page_up_matches(*key) {
                    self.selected.saturating_sub(page)
                } else if bindings.page_down_matches(*key) {
                    (self.selected + page).min(self.rows.len().saturating_sub(1))
                } else if bindings.home_matches(*key) {
                    0
                } else if bindings.end_matches(*key) {
                    self.rows.len().saturating_sub(1)
                } else {
                    return Some(Msg::Redraw);
                };

                self.selected = next_selected;
                let target = self.scroll_target_for_selection(viewport);
                let x = self.scroll.target_offset().x;
                let _ = self.scroll.scroll_to(
                    tuicore::ScrollOffset::new(x, target),
                    viewport,
                    self.content_size(),
                    tuicore::animation_settings(),
                );
                Some(Msg::Redraw)
            }
            Event::Tick => {
                let settings = tuicore::animation_settings();
                let tick = self.scroll.tick(settings.frame_duration(), settings);
                (tick.changed || tick.active).then_some(Msg::Redraw)
            }
            _ => Some(Msg::Redraw),
        }
    }
}
