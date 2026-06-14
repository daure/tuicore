use std::error::Error;
use std::time::Duration;

use tuicore::{
    Animated, AnimationSpec, Easing, Panel, ScrollAxes, ScrollBehavior, ScrollSize, ScrollState,
    Tabs, TabsVariant, line_width, theme,
};
use tuirealm::application::{Application, PollStrategy};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, NoUserEvent};
use tuirealm::listener::EventListenerCfg;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::ratatui::Frame;
use tuirealm::ratatui::layout::{Constraint, Direction, Layout, Rect};
use tuirealm::ratatui::style::{Modifier, Style};
use tuirealm::ratatui::text::{Line, Span};
use tuirealm::ratatui::widgets::{List, ListItem, ListState, Paragraph};
use tuirealm::state::State;
use tuirealm::terminal::{CrosstermTerminalAdapter, TerminalAdapter};

fn main() -> Result<(), Box<dyn Error>> {
    tuicore::init();
    let mut model = Model::new()?;

    while !model.quit {
        let frame_duration = tuicore::animation_settings().frame_duration();
        model.sync_preview_area()?;
        for msg in model.app.tick(PollStrategy::Once(frame_duration))? {
            model.update(msg)?;
        }

        if model.redraw {
            model.view()?;
            model.redraw = false;
        }
    }

    Ok(())
}

#[derive(Debug, Eq, PartialEq, Clone, Hash)]
enum Id {
    ComponentList,
    Panel,
    ScrollAnimated,
    TabsMinimal,
    TabsUnderline,
    TabsBoxed,
}

#[derive(Debug, PartialEq)]
enum Msg {
    Quit,
    FocusList,
    FocusPreview,
    Selected(ComponentKind),
    Redraw,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum ComponentKind {
    TabsMinimal,
    TabsUnderline,
    TabsBoxed,
    Panel,
    ScrollAnimated,
}

impl ComponentKind {
    const ALL: [Self; 5] = [
        Self::TabsMinimal,
        Self::TabsUnderline,
        Self::TabsBoxed,
        Self::Panel,
        Self::ScrollAnimated,
    ];

    fn title(self) -> &'static str {
        match self {
            Self::TabsMinimal => "Tabs: minimal",
            Self::TabsUnderline => "Tabs: underline",
            Self::TabsBoxed => "Tabs: boxed",
            Self::Panel => "Panel",
            Self::ScrollAnimated => "Scroll: animated",
        }
    }

    fn preview_id(self) -> Id {
        match self {
            Self::TabsMinimal => Id::TabsMinimal,
            Self::TabsUnderline => Id::TabsUnderline,
            Self::TabsBoxed => Id::TabsBoxed,
            Self::Panel => Id::Panel,
            Self::ScrollAnimated => Id::ScrollAnimated,
        }
    }
}

struct Model {
    app: Application<Id, Msg, NoUserEvent>,
    terminal: CrosstermTerminalAdapter,
    selected: ComponentKind,
    quit: bool,
    redraw: bool,
}

impl Model {
    fn new() -> Result<Self, Box<dyn Error>> {
        let animation = tuicore::animation_settings();
        let frame_duration = animation.frame_duration();
        let mut listener = EventListenerCfg::default().crossterm_input_listener(frame_duration, 3);
        if animation.enabled {
            listener = listener.tick_interval(animation.frame_duration());
        }

        let mut app = Application::init(listener);
        app.mount(
            Id::ComponentList,
            Box::new(ComponentList::new(ComponentKind::ALL.to_vec())),
            Vec::new(),
        )?;
        app.mount(
            Id::TabsMinimal,
            Box::new(TabsPreview::new(TabsVariant::Minimal)),
            Vec::new(),
        )?;
        app.mount(
            Id::TabsUnderline,
            Box::new(TabsPreview::new(TabsVariant::Underline)),
            Vec::new(),
        )?;
        app.mount(
            Id::TabsBoxed,
            Box::new(TabsPreview::new(TabsVariant::Boxed)),
            Vec::new(),
        )?;
        app.mount(Id::Panel, Box::new(PanelPreview::new()), Vec::new())?;
        app.mount(
            Id::ScrollAnimated,
            Box::new(AnimatedScrollPreview::new()),
            Vec::new(),
        )?;
        app.active(&Id::ComponentList)?;

        let mut terminal = CrosstermTerminalAdapter::new()?;
        terminal.enable_raw_mode()?;
        terminal.enter_alternate_screen()?;

        Ok(Self {
            app,
            terminal,
            selected: ComponentKind::TabsMinimal,
            quit: false,
            redraw: true,
        })
    }

    fn view(&mut self) -> Result<(), Box<dyn Error>> {
        self.terminal.draw(|frame| {
            let [left, right] = Self::layout(frame.area());

            self.app.view(&Id::ComponentList, frame, left);
            self.app.view(&self.selected.preview_id(), frame, right);
        })?;

        Ok(())
    }

    fn sync_preview_area(&mut self) -> Result<(), Box<dyn Error>> {
        let area = self.terminal.raw().size()?.into();
        let preview = Self::layout(area)[1];
        for id in [Id::Panel, Id::ScrollAnimated] {
            self.app
                .attr(&id, Attribute::Width, AttrValue::Size(preview.width))?;
            self.app
                .attr(&id, Attribute::Height, AttrValue::Size(preview.height))?;
        }
        Ok(())
    }

    fn layout(area: Rect) -> [Rect; 2] {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .areas(area)
    }

    fn update(&mut self, msg: Msg) -> Result<(), Box<dyn Error>> {
        self.redraw = true;
        match msg {
            Msg::Quit => self.quit = true,
            Msg::FocusList => self.app.active(&Id::ComponentList)?,
            Msg::FocusPreview => self.app.active(&self.selected.preview_id())?,
            Msg::Selected(component) => self.selected = component,
            Msg::Redraw => {}
        }

        Ok(())
    }
}

impl Drop for Model {
    fn drop(&mut self) {
        let _ = self.terminal.leave_alternate_screen();
        let _ = self.terminal.disable_raw_mode();
    }
}

struct ComponentList {
    components: Vec<ComponentKind>,
    selected: usize,
    focused: bool,
}

impl ComponentList {
    fn new(components: Vec<ComponentKind>) -> Self {
        Self {
            components,
            selected: 0,
            focused: false,
        }
    }
}

impl Component for ComponentList {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let theme = theme();
        let panel = Panel::new()
            .top_left("Components")
            .top_right("↑/↓ navigate · q quit")
            .focused(self.focused);
        panel.render(frame, area);

        let items = self
            .components
            .iter()
            .map(|component| ListItem::new(Line::from(Span::raw(component.title()))));
        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .fg(theme.selected_fg())
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("› ");
        let mut state = ListState::default().with_selected(Some(self.selected));

        frame.render_stateful_widget(list, Panel::inner_area(area), &mut state);
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        match attr {
            Attribute::Focus => Some(QueryResult::Owned(AttrValue::Flag(self.focused))),
            _ => None,
        }
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        if attr == Attribute::Focus
            && let AttrValue::Flag(focused) = value
        {
            self.focused = focused;
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
            Event::Keyboard(KeyEvent { code: Key::Tab, .. }) => Some(Msg::FocusPreview),
            Event::Keyboard(KeyEvent { code: Key::Up, .. }) => {
                self.selected = self.selected.saturating_sub(1);
                Some(Msg::Selected(self.components[self.selected]))
            }
            Event::Keyboard(KeyEvent {
                code: Key::Down, ..
            }) => {
                self.selected = (self.selected + 1).min(self.components.len().saturating_sub(1));
                Some(Msg::Selected(self.components[self.selected]))
            }
            _ => Some(Msg::Redraw),
        }
    }
}

struct TabsPreview {
    tabs: Tabs<Msg, NoUserEvent>,
}

impl TabsPreview {
    fn new(variant: TabsVariant) -> Self {
        Self {
            tabs: Tabs::default().variant(variant),
        }
    }
}

impl Component for TabsPreview {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        self.tabs.view(frame, area);
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        self.tabs.query(attr)
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        self.tabs.attr(attr, value);
    }

    fn state(&self) -> State {
        self.tabs.state()
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        self.tabs.perform(cmd)
    }
}

impl AppComponent<Msg, NoUserEvent> for TabsPreview {
    fn on(&mut self, event: &Event<NoUserEvent>) -> Option<Msg> {
        match event {
            Event::Keyboard(KeyEvent { code: Key::Esc, .. })
            | Event::Keyboard(KeyEvent {
                code: Key::Char('q'),
                ..
            }) => Some(Msg::Quit),
            Event::Keyboard(KeyEvent {
                code: Key::Tab | Key::BackTab,
                ..
            }) => Some(Msg::FocusList),
            _ => self.tabs.on(event).or(Some(Msg::Redraw)),
        }
    }
}

struct PanelPreview {
    panel: Panel,
    event_area: Rect,
}

impl PanelPreview {
    fn new() -> Self {
        Self {
            panel: Panel::new().top_left("local").top_right("Panel").content([
                "Panel chrome uses the global border preset.",
                "Top-left and top-right title slots are optional.",
                "Scrollable Panel owns ScrollState and renders ratatui scrollbars.",
                "Use ↑/↓, configured page keys, Home/End for vertical scroll.",
                "Use ←/→ for horizontal scroll when preview has focus.",
                "",
                "01  Short row.",
                "02  This row is intentionally long so horizontal scroll has visible work to do in narrow terminals.",
                "03  Unicode width comes from ratatui Line::width(): café 🚀 漢字.",
                "04  Animation starts from event handling; render only reads current offset.",
                "05  Global animation disabled snaps scrolling immediately.",
                "06  ScrollState can be reused outside Panel for lists, tables, grids, and custom widgets.",
                "07  Content keeps going.",
                "08  Content keeps going.",
                "09  Content keeps going.",
                "10  Content keeps going.",
                "11  Content keeps going.",
                "12  Content keeps going.",
                "13  Content keeps going.",
                "14  Content keeps going.",
                "15  Content keeps going.",
                "16  Content keeps going.",
                "17  Content keeps going.",
                "18  Content keeps going.",
                "19  Content keeps going.",
                "20  End of scroll demo.",
            ]).scrollable(ScrollAxes::Both),
            event_area: Rect::default(),
        }
    }
}

impl Component for PanelPreview {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        self.panel.view(frame, area);
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

impl AppComponent<Msg, NoUserEvent> for PanelPreview {
    fn on(&mut self, event: &Event<NoUserEvent>) -> Option<Msg> {
        match event {
            Event::Keyboard(KeyEvent { code: Key::Esc, .. })
            | Event::Keyboard(KeyEvent {
                code: Key::Char('q'),
                ..
            }) => Some(Msg::Quit),
            Event::Keyboard(KeyEvent {
                code: Key::Tab | Key::BackTab,
                ..
            }) => Some(Msg::FocusList),
            Event::Keyboard(key) => self
                .panel
                .on_key(*key, self.event_area, tuicore::animation_settings())
                .needs_redraw()
                .then_some(Msg::Redraw),
            Event::Tick => {
                let settings = tuicore::animation_settings();
                self.panel
                    .tick(settings.frame_duration(), settings)
                    .changed
                    .then_some(Msg::Redraw)
            }
            _ => Some(Msg::Redraw),
        }
    }
}

struct AnimatedScrollPreview {
    scroll: ScrollState,
    rows: Vec<String>,
    selected: usize,
    focused: bool,
    event_area: Rect,
}

impl AnimatedScrollPreview {
    fn new() -> Self {
        Self {
            scroll: ScrollState::from_preset(ScrollAxes::Vertical, tuicore::preset().scroll())
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
                    format!("{:03}  {}", index + 1, component.title())
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
            .top_left("ScrollState")
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
                Paragraph::new(Line::from(Span::styled(format!("{prefix}{row}"), style))),
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
            Event::Keyboard(KeyEvent {
                code: Key::Tab | Key::BackTab,
                ..
            }) => Some(Msg::FocusList),
            Event::Keyboard(key) => {
                let bindings = tuicore::keybindings();
                let viewport = self.event_viewport();
                let page = viewport.height.max(1);
                let next_selected = if key.code == Key::Up {
                    self.selected.saturating_sub(1)
                } else if key.code == Key::Down {
                    (self.selected + 1).min(self.rows.len().saturating_sub(1))
                } else if bindings.page_up_matches(*key) {
                    self.selected.saturating_sub(page)
                } else if bindings.page_down_matches(*key) {
                    (self.selected + page).min(self.rows.len().saturating_sub(1))
                } else if key.code == Key::Home {
                    0
                } else if key.code == Key::End {
                    self.rows.len().saturating_sub(1)
                } else {
                    return Some(Msg::Redraw);
                };

                self.selected = next_selected;
                let target = self.scroll_target_for_selection(viewport);
                let _ = self.scroll.scroll_to(
                    tuicore::ScrollOffset::new(0, target),
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
