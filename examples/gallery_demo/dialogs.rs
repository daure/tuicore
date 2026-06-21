use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::Paragraph;
use tuicore::{
    Animated, AnimationSettings, Button, ChildKey, DataView, Dialog, DialogHost, Dropdown,
    EventCtx, EventOutcome, EventRoute, FocusCtx, FocusTarget, LayoutCtx, LayoutResult, Tab, Tabs,
    TextInput, TickResult, Toggle, TuiEvent, TuiNode,
};

use super::data::{DataViewMode, DemoRow};
use super::dropdowns::{DropdownDemoItem, dropdown_fuzzy_single};
use crate::{Msg, dispatch_focus_child};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DialogExample {
    Full,
    Large,
    Medium,
    Small,
    Tiny,
    Top,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockOverlayExample {
    Top,
    Bottom,
    Left,
    Right,
    BottomSnackbar,
    BottomTabs,
}

impl DockOverlayExample {
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Top => "Top dock overlay",
            Self::Bottom => "Bottom dock overlay",
            Self::Left => "Left dock overlay",
            Self::Right => "Right dock overlay",
            Self::BottomSnackbar => "Bottom snackbar overlay",
            Self::BottomTabs => "Tabbed bottom bar",
        }
    }

    fn button_label(self) -> &'static str {
        match self {
            Self::Top => "Open top dock",
            Self::Bottom => "Open bottom dock",
            Self::Left => "Open left dock",
            Self::Right => "Open right dock",
            Self::BottomSnackbar => "Open 80% snackbar",
            Self::BottomTabs => "Open docked tab bar",
        }
    }

    fn hotkey(self) -> &'static str {
        match self {
            Self::Top => "ot",
            Self::Bottom => "ob",
            Self::Left => "ol",
            Self::Right => "or",
            Self::BottomSnackbar => "os",
            Self::BottomTabs => "od",
        }
    }
}

impl DialogExample {
    pub(crate) fn percent(self) -> u16 {
        match self {
            Self::Full => 100,
            Self::Large => 80,
            Self::Medium => 60,
            Self::Small => 40,
            Self::Tiny => 20,
            Self::Top => 50,
        }
    }

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Full => "100% text dialog",
            Self::Large => "80% tabs dialog",
            Self::Medium => "60% input dialog",
            Self::Small => "40% toggle dialog",
            Self::Tiny => "20% data dialog",
            Self::Top => "50% input dialog",
        }
    }

    fn button_label(self) -> &'static str {
        match self {
            Self::Full => "Open 100% • text",
            Self::Large => "Open 80% • tabs",
            Self::Medium => "Open 60% • input",
            Self::Small => "Open 40% • toggle",
            Self::Tiny => "Open 20% • data",
            Self::Top => "Open 50% • input",
        }
    }

    fn hotkey(self) -> &'static str {
        match self {
            Self::Full => "1",
            Self::Large => "2",
            Self::Medium => "3",
            Self::Small => "4",
            Self::Tiny => "5",
            Self::Top => "6",
        }
    }
}

pub(crate) struct GalleryDialogContent {
    example: DialogExample,
    tabs: Tabs<Msg>,
    input: TextInput<Msg>,
    toggle: Toggle<Msg>,
    data: DataView<DemoRow, usize>,
}

pub(crate) struct GalleryDockOverlayContent {
    example: DockOverlayExample,
    tabs: Tabs<Msg>,
}

struct DialogControlsTab {
    toggle: Toggle<Msg>,
    dropdown: Dropdown<DropdownDemoItem, &'static str>,
    input: TextInput<Msg>,
    areas: [Rect; 3],
}

struct DialogTreeTab {
    data: DataView<DemoRow, usize>,
    text_area: Rect,
    data_area: Rect,
}

impl GalleryDialogContent {
    fn new() -> Self {
        Self {
            example: DialogExample::Large,
            tabs: dialog_tabs(),
            input: TextInput::new().placeholder("Type inside the modal..."),
            toggle: Toggle::new("Enable modal option").hotkey("t"),
            data: DataViewMode::List.data_view().hotkey("d"),
        }
    }

    pub(crate) fn set_example(&mut self, example: DialogExample) {
        self.example = example;
    }
}

impl GalleryDockOverlayContent {
    fn new() -> Self {
        Self {
            example: DockOverlayExample::BottomTabs,
            tabs: Tabs::new(vec![
                Tab::text(
                    "Status",
                    "Bottom tab bar: status content. Press x or Esc to close the overlay.",
                )
                .hotkey("1"),
                Tab::text(
                    "Logs",
                    "Recent activity lives here while app content underneath is dimmed.",
                )
                .hotkey("2"),
                Tab::text("Actions", "Quick actions can sit in a docked tab overlay.").hotkey("3"),
            ]),
        }
    }

    pub(crate) fn set_example(&mut self, example: DockOverlayExample) {
        self.example = example;
    }
}

impl TuiNode<Msg> for GalleryDialogContent {
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        match self.example {
            DialogExample::Full => {}
            DialogExample::Large => {
                self.tabs.layout(area, ctx);
            }
            DialogExample::Medium => {
                self.input.layout(area, ctx);
            }
            DialogExample::Small => {
                self.toggle.layout(area, ctx);
            }
            DialogExample::Tiny => {
                <DataView<DemoRow, usize> as TuiNode<Msg>>::layout(&mut self.data, area, ctx);
            }
            DialogExample::Top => {
                self.input.layout(area, ctx);
            }
        }
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        match self.example {
            DialogExample::Full => {}
            DialogExample::Large => self.tabs.render(frame, area),
            DialogExample::Medium => self.input.render(frame, area),
            DialogExample::Small => self.toggle.render(frame, area),
            DialogExample::Tiny => self.data.render(frame, area),
            DialogExample::Top => self.input.render(frame, area),
        }
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<Msg>,
    ) -> EventOutcome {
        match self.example {
            DialogExample::Full => EventOutcome::Ignored,
            DialogExample::Large => self.tabs.dispatch_event(route, event, ctx),
            DialogExample::Medium => self.input.dispatch_event(route, event, ctx),
            DialogExample::Small => self.toggle.dispatch_event(route, event, ctx),
            DialogExample::Tiny => self.data.dispatch_event(route, event, ctx),
            DialogExample::Top => self.input.dispatch_event(route, event, ctx),
        }
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        <Tabs<Msg> as TuiNode<Msg>>::tick(&mut self.tabs, dt, settings)
            .merge(Animated::tick(&mut self.input, dt, settings))
            .merge(Animated::tick(&mut self.toggle, dt, settings))
            .merge(Animated::tick(&mut self.data, dt, settings))
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<Msg>) {
        match self.example {
            DialogExample::Full => {}
            DialogExample::Large => self.tabs.dispatch_focus(target, focused, ctx),
            DialogExample::Medium => self.input.dispatch_focus(target, focused, ctx),
            DialogExample::Small => self.toggle.dispatch_focus(target, focused, ctx),
            DialogExample::Tiny => self.data.dispatch_focus(target, focused, ctx),
            DialogExample::Top => self.input.dispatch_focus(target, focused, ctx),
        }
    }
}

impl TuiNode<Msg> for GalleryDockOverlayContent {
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        if self.example == DockOverlayExample::BottomTabs {
            self.tabs.layout(area, ctx);
        }
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        if self.example == DockOverlayExample::BottomTabs {
            self.tabs.render(frame, area);
        } else {
            frame.render_widget(
                Paragraph::new(format!(
                    "{}\n\nDocked overlay content. Underlying gallery is faded and blocked. Press x or Esc to close.",
                    self.example.title()
                )),
                area,
            );
        }
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<Msg>,
    ) -> EventOutcome {
        if self.example == DockOverlayExample::BottomTabs {
            self.tabs.dispatch_event(route, event, ctx)
        } else {
            EventOutcome::Ignored
        }
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        <Tabs<Msg> as TuiNode<Msg>>::tick(&mut self.tabs, dt, settings)
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<Msg>) {
        if self.example == DockOverlayExample::BottomTabs {
            self.tabs.dispatch_focus(target, focused, ctx);
        }
    }
}

impl DialogControlsTab {
    fn new() -> Self {
        Self {
            toggle: Toggle::new("Enable safety checks").hotkey("t"),
            dropdown: dropdown_fuzzy_single().hotkey("d"),
            input: TextInput::new().placeholder("Dialog text input..."),
            areas: [Rect::default(); 3],
        }
    }
}

impl TuiNode<Msg> for DialogControlsTab {
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        let [_, toggle, dropdown, input, _] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Fill(1),
            ])
            .areas(area);
        self.areas = [toggle, dropdown, input];
        ctx.push_slot(dialog_tab_child_key("toggle"), toggle, |ctx| {
            self.toggle.layout(toggle, ctx);
        });
        ctx.push_slot(dialog_tab_child_key("dropdown"), dropdown, |ctx| {
            self.dropdown.layout_overlay::<Msg>(dropdown, area, ctx);
        });
        ctx.push_slot(dialog_tab_child_key("input"), input, |ctx| {
            self.input.layout(input, ctx);
        });
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(
            Paragraph::new("First tab: toggle, dropdown, and text input."),
            Rect::new(area.x, area.y, area.width, 1),
        );
        self.toggle.render(frame, self.areas[0]);
        self.dropdown.render(frame, self.areas[1]);
        self.input.render(frame, self.areas[2]);
        if self.dropdown.is_open() {
            self.dropdown.render_popup_overlay(frame, frame.area());
        }
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<Msg>,
    ) -> EventOutcome {
        if let Some(route) = route
            .path
            .without_first_if(&dialog_tab_child_key("toggle"))
            .map(EventRoute::new)
        {
            return self.toggle.dispatch_event(&route, event, ctx);
        }
        if let Some(route) = route
            .path
            .without_first_if(&dialog_tab_child_key("dropdown"))
            .map(EventRoute::new)
        {
            return self.dropdown.dispatch_event(&route, event, ctx);
        }
        if let Some(route) = route
            .path
            .without_first_if(&dialog_tab_child_key("input"))
            .map(EventRoute::new)
        {
            return self.input.dispatch_event(&route, event, ctx);
        }
        EventOutcome::Ignored
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(&mut self.toggle, dt, settings)
            .merge(Animated::tick(&mut self.dropdown, dt, settings))
            .merge(Animated::tick(&mut self.input, dt, settings))
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<Msg>) {
        if dispatch_focus_child(
            &mut self.toggle,
            target,
            dialog_tab_child_key("toggle"),
            focused,
            ctx,
        ) {
            return;
        }
        if dispatch_focus_child(
            &mut self.dropdown,
            target,
            dialog_tab_child_key("dropdown"),
            focused,
            ctx,
        ) {
            return;
        }
        dispatch_focus_child(
            &mut self.input,
            target,
            dialog_tab_child_key("input"),
            focused,
            ctx,
        );
    }
}

impl DialogTreeTab {
    fn new() -> Self {
        Self {
            data: DataViewMode::ChecklistTree.data_view().hotkey("m"),
            text_area: Rect::default(),
            data_area: Rect::default(),
        }
    }
}

impl TuiNode<Msg> for DialogTreeTab {
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        let [text_area, data_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Fill(1)])
            .areas(area);
        self.text_area = text_area;
        self.data_area = data_area;
        ctx.push_slot(dialog_tab_child_key("tree"), data_area, |ctx| {
            <DataView<DemoRow, usize> as TuiNode<Msg>>::layout(&mut self.data, data_area, ctx);
        });
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, _area: Rect) {
        frame.render_widget(
            Paragraph::new(
                "Second tab: paragraph on top and a multi-select tree below. Space toggles rows; Enter activates.",
            ),
            self.text_area,
        );
        self.data.render(frame, self.data_area);
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<Msg>,
    ) -> EventOutcome {
        let Some(route) = route
            .path
            .without_first_if(&dialog_tab_child_key("tree"))
            .map(EventRoute::new)
        else {
            return EventOutcome::Ignored;
        };
        self.data.dispatch_event(&route, event, ctx)
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(&mut self.data, dt, settings)
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<Msg>) {
        dispatch_focus_child(
            &mut self.data,
            target,
            dialog_tab_child_key("tree"),
            focused,
            ctx,
        );
    }
}

pub(crate) fn dialog_button(example: DialogExample) -> Button<Msg> {
    Button::new(example.button_label())
        .hotkey(example.hotkey())
        .on_press(move || Msg::DialogOpened(example))
}

pub(crate) fn dock_overlay_button(example: DockOverlayExample) -> Button<Msg> {
    Button::new(example.button_label())
        .hotkey(example.hotkey())
        .on_press(move || Msg::DockOverlayOpened(example))
}

fn dialog_tabs() -> Tabs<Msg> {
    Tabs::new(vec![
        Tab::new("Controls", DialogControlsTab::new()).hotkey("1"),
        Tab::new("Tree", DialogTreeTab::new()).hotkey("2"),
        Tab::new(
            "Nested",
            Tabs::new(vec![
                Tab::text("Alpha", "Nested tab content: alpha text.").hotkey("a"),
                Tab::text("Beta", "Nested tab content: beta text.").hotkey("b"),
                Tab::text("Gamma", "Nested tab content: gamma text.").hotkey("g"),
            ]),
        )
        .hotkey("3"),
    ])
}

pub(crate) fn gallery_dialog() -> DialogHost<GalleryDialogContent, Msg> {
    let mut dialog = Dialog::new()
        .top_left(DialogExample::Large.title())
        .bottom_left("Esc closes")
        .bottom_right("80% viewport")
        .on_close(Msg::DialogClosed);
    dialog.clear_title(tuicore::DialogTitlePosition::TopRight);
    dialog.host(GalleryDialogContent::new())
}

pub(crate) fn gallery_dock_overlay() -> DialogHost<GalleryDockOverlayContent, Msg> {
    Dialog::new()
        .top_left(DockOverlayExample::BottomTabs.title())
        .bottom_left("Esc closes")
        .bottom_right("docked")
        .on_close(Msg::DockOverlayClosed)
        .host(GalleryDockOverlayContent::new())
}

fn dialog_tab_child_key(key: &'static str) -> ChildKey {
    ChildKey::new(key)
}

pub(crate) fn dialog_demo_child_key(index: usize) -> ChildKey {
    ChildKey::new(format!("dialog-demo-{index}"))
}

pub(crate) fn dialog_demo_index(key: &ChildKey) -> Option<usize> {
    key.as_str().strip_prefix("dialog-demo-")?.parse().ok()
}

pub(crate) fn dialog_demo_child_route(route: &EventRoute) -> Option<(usize, EventRoute)> {
    let first = route.path.first()?;
    let index = dialog_demo_index(first)?;
    Some((index, EventRoute::new(route.path.without_first())))
}

pub(crate) fn dialog_body_area(area: Rect) -> Rect {
    Rect::new(
        area.x,
        area.y.saturating_add(2),
        area.width,
        area.height.saturating_sub(2),
    )
}

pub(crate) fn dialog_button_areas(area: Rect) -> [Rect; 12] {
    let [_, body, _] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(23.min(area.height)),
            Constraint::Fill(1),
        ])
        .areas(area);
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .spacing(1)
        .areas(body)
}
