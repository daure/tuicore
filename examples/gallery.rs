use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use tuicore::{
    ActivationMode, Animated, AnimationSettings, BorderKind, Button, CellContext, ChildKey, Column,
    DataView, DataViewTypedEvent, Dialog, DialogCloseReason, DialogHost, DialogLayer, Dropdown,
    DropdownCommitMode, DropdownSearchMode, DropdownVariant, EventCtx, EventOutcome, EventRoute,
    Flex, FlexItem, FocusCtx, FocusId, FocusTarget, Gap, Grid, GridItem, GridTrack, HintSource,
    Key, KeyEvent, KeyModifiers, LayoutCtx, LayoutProposal, LayoutResult, LayoutSize,
    LayoutSizeHint, Overlay, OverlayAnchor, OverlaySize, Panel, PanelTitlePosition,
    SelectionGlyphs, SelectionMode, SelectionPropagation, SelectionTrigger, Separator,
    SeparatorColorRole, Spinner, Split, Stack, StackAlign, StackItem, Tab, Tabs, TabsVariant,
    TextInput, TextareaInput, TickResult, Toggle, TreeAdapter, TreeGlyphs, TuiEvent, TuiNode,
};

#[derive(Debug, PartialEq)]
enum Msg {
    DialogOpened(DialogExample),
    DialogClosed(DialogCloseReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DialogExample {
    Full,
    Large,
    Medium,
    Small,
    Tiny,
}

impl DialogExample {
    fn percent(self) -> u16 {
        match self {
            Self::Full => 100,
            Self::Large => 80,
            Self::Medium => 60,
            Self::Small => 40,
            Self::Tiny => 20,
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Full => "100% text dialog",
            Self::Large => "80% tabs dialog",
            Self::Medium => "60% input dialog",
            Self::Small => "40% toggle dialog",
            Self::Tiny => "20% data dialog",
        }
    }

    fn button_label(self) -> &'static str {
        match self {
            Self::Full => "Open 100% • text",
            Self::Large => "Open 80% • tabs",
            Self::Medium => "Open 60% • text input",
            Self::Small => "Open 40% • toggle",
            Self::Tiny => "Open 20% • data list",
        }
    }

    fn hotkey(self) -> &'static str {
        match self {
            Self::Full => "1",
            Self::Large => "2",
            Self::Medium => "3",
            Self::Small => "4",
            Self::Tiny => "5",
        }
    }
}

fn main() -> tuicore::Result<()> {
    tuicore::init();
    let root = DialogLayer::new(Gallery::new(), gallery_dialog()).active(false);
    tuicore::TreeApp::new(root)
        .on_message(|root, msg, ctx| match msg {
            Msg::DialogOpened(example) => {
                root.layer_mut().child_mut().set_example(example);
                root.layer_mut().dialog_mut().set_top_left(example.title());
                root.layer_mut().dialog_mut().set_bottom_left("Esc blurs");
                root.layer_mut()
                    .dialog_mut()
                    .set_bottom_right(format!("{}% viewport", example.percent()));
                if example == DialogExample::Full {
                    root.layer_mut().dialog_mut().set_content([
                        "100% dialog: full-screen modal content.",
                        "This uses the Dialog chrome only, with text content inside.",
                        "Press x or Esc to close and restore focus.",
                    ]);
                } else {
                    root.layer_mut().dialog_mut().clear_content();
                }
                root.set_layer_percent(example.percent());
                root.set_active(true);
                ctx.request_layout();
                ctx.request_redraw();
            }
            Msg::DialogClosed(_reason) => {
                root.set_active(false);
                ctx.request_layout();
                ctx.request_redraw();
            }
        })
        .run()
}

struct Gallery {
    component_list: DataView<ComponentKind, ComponentKind>,
    selected: ComponentKind,
    areas: GalleryAreas,
    list_panel: Panel,
    preview_panel: Panel,
    previews: PreviewState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct GalleryAreas {
    list_panel: Rect,
    list_body: Rect,
    preview_panel: Rect,
    preview_body: Rect,
}

struct GalleryDialogContent {
    example: DialogExample,
    tabs: Tabs<Msg>,
    input: TextInput<Msg>,
    toggle: Toggle<Msg>,
    data: DataView<DemoRow, usize>,
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

    fn set_example(&mut self, example: DialogExample) {
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
        self.dropdown.render_popup_overlay(frame, area);
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

impl Gallery {
    fn new() -> Self {
        let component_list = DataView::list(
            ComponentKind::ALL,
            |component| *component,
            |component| component.title().to_string(),
        )
        .tree(TreeAdapter::parent_id(|component: &ComponentKind| {
            component.parent()
        }))
        .activation_mode(ActivationMode::OnNavigate)
        .selection_mode(SelectionMode::Single)
        .selection_trigger(SelectionTrigger::OnNavigate)
        .hotkey("c")
        .selected([ComponentKind::Tabs])
        .expanded([
            ComponentKind::Inputs,
            ComponentKind::Layouts,
            ComponentKind::DataView,
        ])
        .focused(true);

        Self {
            component_list,
            selected: ComponentKind::Tabs,
            areas: GalleryAreas::default(),
            list_panel: Panel::new()
                .top_left("Components")
                .hotkey("c")
                .focused(true),
            preview_panel: Panel::new().top_left(ComponentKind::Tabs.preview().title()),
            previews: PreviewState::new(),
        }
    }

    fn select(&mut self, selected: ComponentKind) {
        self.selected = selected;
        self.preview_panel.set_top_left(selected.preview().title());
    }

    fn selected_from_list_events(&mut self) -> Option<ComponentKind> {
        self.component_list
            .take_events()
            .into_iter()
            .find_map(|event| match event {
                DataViewTypedEvent::HighlightChanged { row_id: Some(id) }
                | DataViewTypedEvent::Activated { row_id: id } => Some(id),
                DataViewTypedEvent::HighlightChanged { row_id: None }
                | DataViewTypedEvent::SelectionChanged { .. } => None,
            })
    }

    fn quit_key(event: &TuiEvent) -> bool {
        let TuiEvent::Key(KeyEvent { code, modifiers }) = event else {
            return false;
        };
        matches!(*code, Key::Char(value) if value.eq_ignore_ascii_case(&'q'))
            && modifiers.contains(KeyModifiers::CONTROL)
    }
}

impl TuiNode<Msg> for Gallery {
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        let [list_panel, preview_panel] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .areas(area);

        self.areas = GalleryAreas {
            list_panel,
            list_body: Panel::inner_area(list_panel),
            preview_panel,
            preview_body: Panel::inner_area(preview_panel),
        };
        ctx.push_slot(gallery_list_child_key(), self.areas.list_body, |ctx| {
            <DataView<ComponentKind, ComponentKind> as TuiNode<Msg>>::layout(
                &mut self.component_list,
                self.areas.list_body,
                ctx,
            );
        });
        ctx.push_slot(
            gallery_preview_child_key(),
            self.areas.preview_body,
            |ctx| {
                self.previews
                    .layout(self.selected.preview(), self.areas.preview_body, ctx);
                ctx.register_focusable(FocusId::new("preview"), self.areas.preview_body, true);
            },
        );
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, _area: Rect) {
        self.list_panel.render(frame, self.areas.list_panel);
        self.component_list.render(frame, self.areas.list_body);

        self.preview_panel.render(frame, self.areas.preview_panel);
        self.previews
            .render(self.selected.preview(), frame, self.areas.preview_body);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<Msg>) -> EventOutcome {
        if Self::quit_key(event) {
            ctx.request_quit();
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        EventOutcome::Ignored
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(&mut self.list_panel, dt, settings)
            .merge(Animated::tick(&mut self.preview_panel, dt, settings))
            .merge(Animated::tick(&mut self.component_list, dt, settings))
            .merge(self.previews.tick(dt, settings))
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<Msg>,
    ) -> EventOutcome {
        if route.path.is_empty() {
            return self.event(event, ctx);
        }

        if let Some(route) = route
            .path
            .without_first_if(&gallery_list_child_key())
            .map(EventRoute::new)
        {
            let child = self.component_list.dispatch_event(&route, event, ctx);
            if let Some(selected) = self.selected_from_list_events() {
                self.select(selected);
                ctx.request_layout();
                ctx.request_redraw();
            }
            if let TuiEvent::Key(KeyEvent {
                code: Key::Enter, ..
            }) = event
            {
                ctx.focus_next();
                ctx.request_redraw();
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
            return child.bubble(ctx, |ctx| self.event(event, ctx));
        }

        if let Some(route) = route
            .path
            .without_first_if(&gallery_preview_child_key())
            .map(EventRoute::new)
        {
            let child = self
                .previews
                .dispatch_event(self.selected.preview(), &route, event, ctx);
            return child.bubble(ctx, |ctx| self.event(event, ctx));
        }

        EventOutcome::Ignored
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<Msg>) {
        if let Some(child_target) = target.for_child(&gallery_list_child_key()) {
            self.component_list
                .dispatch_focus(&child_target, focused, ctx);
            self.list_panel.set_focused(focused, ctx.animation());
            ctx.request_redraw();
            return;
        }

        let Some(child_target) = target.for_child(&gallery_preview_child_key()) else {
            return;
        };

        if !child_target.path.is_empty() || child_target.id.as_str() != "preview" {
            self.previews
                .dispatch_focus(self.selected.preview(), &child_target, focused, ctx);
        }
        self.preview_panel.set_focused(focused, ctx.animation());
        ctx.request_redraw();
    }
}

struct PreviewState {
    text_input: TextInput<Msg>,
    textarea_input: TextareaInput<Msg>,
    button: Button<Msg>,
    button_presses: u32,
    toggle: Toggle<Msg>,
    dialog_100: Button<Msg>,
    dialog_80: Button<Msg>,
    dialog_60: Button<Msg>,
    dialog_40: Button<Msg>,
    dialog_20: Button<Msg>,
    spinner: Spinner,
    panel_demo: Panel,
    tabs_minimal: Tabs<Msg>,
    tabs_underline: Tabs<Msg>,
    tabs_boxed: Tabs<Msg>,
    data_list: DataView<DemoRow, usize>,
    data_table: DataView<DemoRow, usize>,
    data_list_tree: DataView<DemoRow, usize>,
    data_table_tree: DataView<DemoRow, usize>,
    data_single_select: DataView<DemoRow, usize>,
    data_multi_select: DataView<DemoRow, usize>,
    data_checklist_tree: DataView<DemoRow, usize>,
    data_activate_on_navigate: DataView<DemoRow, usize>,
    data_status: String,
    panel_top_left: Dropdown<PanelTitleChoice, &'static str>,
    panel_top_right: Dropdown<PanelTitleChoice, &'static str>,
    panel_bottom_left: Dropdown<PanelTitleChoice, &'static str>,
    panel_bottom_right: Dropdown<PanelTitleChoice, &'static str>,
    dropdown_fuzzy_single: Dropdown<DropdownDemoItem, &'static str>,
    dropdown_multi_contains: Dropdown<DropdownDemoItem, &'static str>,
    dropdown_no_search_immediate: Dropdown<DropdownDemoItem, &'static str>,
    dropdown_filled_fuzzy_single: Dropdown<DropdownDemoItem, &'static str>,
    dropdown_filled_multi_contains: Dropdown<DropdownDemoItem, &'static str>,
    dropdown_filled_no_search_immediate: Dropdown<DropdownDemoItem, &'static str>,
    layout_flex: Flex<Msg>,
    layout_split: Split<DemoBox, DemoBox>,
    layout_stack: Stack<Msg>,
    layout_overlay: Overlay<DemoBox, DemoBox>,
    layout_grid: Grid<Msg>,
}

impl PreviewState {
    fn new() -> Self {
        Self {
            text_input: TextInput::new()
                .placeholder("Type one line...")
                .value("tuicore")
                .max_len(80),
            textarea_input: TextareaInput::new()
                .placeholder("Write multiple lines...")
                .value("First line\nSecond line")
                .max_lines(8),
            button: Button::new("button").hotkey("b"),
            button_presses: 0,
            toggle: Toggle::new("Telemetry").hotkey("x"),
            dialog_100: dialog_button(DialogExample::Full),
            dialog_80: dialog_button(DialogExample::Large),
            dialog_60: dialog_button(DialogExample::Medium),
            dialog_40: dialog_button(DialogExample::Small),
            dialog_20: dialog_button(DialogExample::Tiny),
            spinner: Spinner::new(),
            panel_demo: panel_demo(),
            tabs_minimal: Tabs::default().variant(TabsVariant::Minimal).hotkey("m"),
            tabs_underline: Tabs::default().variant(TabsVariant::Underline).hotkey("l"),
            tabs_boxed: Tabs::new(vec![
                Tab::text("Overview", "Simple tabs component for tuicore.").hotkey("o"),
                Tab::text("Usage", "Use Tab::new(title, node), then Tabs::new(tabs).").hotkey("u"),
                Tab::text("State", "The selected tab is a plain index.").hotkey("s"),
            ])
            .variant(TabsVariant::Boxed)
            .hotkey("b"),
            data_list: DataViewMode::List.data_view(),
            data_table: DataViewMode::Table.data_view(),
            data_list_tree: DataViewMode::ListTree.data_view(),
            data_table_tree: DataViewMode::TableTree.data_view(),
            data_single_select: DataViewMode::SingleSelect.data_view(),
            data_multi_select: DataViewMode::MultiSelect.data_view(),
            data_checklist_tree: DataViewMode::ChecklistTree.data_view(),
            data_activate_on_navigate: DataViewMode::ActivateOnNavigate.data_view(),
            data_status: String::from("No event yet"),
            panel_top_left: panel_title_dropdown(PanelTitlePosition::TopLeft),
            panel_top_right: panel_title_dropdown(PanelTitlePosition::TopRight),
            panel_bottom_left: panel_title_dropdown(PanelTitlePosition::BottomLeft),
            panel_bottom_right: panel_title_dropdown(PanelTitlePosition::BottomRight),
            dropdown_fuzzy_single: dropdown_fuzzy_single(),
            dropdown_multi_contains: dropdown_multi_contains(),
            dropdown_no_search_immediate: dropdown_no_search_immediate(),
            dropdown_filled_fuzzy_single: dropdown_filled_fuzzy_single(),
            dropdown_filled_multi_contains: dropdown_filled_multi_contains(),
            dropdown_filled_no_search_immediate: dropdown_filled_no_search_immediate(),
            layout_flex: layout_flex_demo(),
            layout_split: layout_split_demo(),
            layout_stack: layout_stack_demo(),
            layout_overlay: layout_overlay_demo(),
            layout_grid: layout_grid_demo(),
        }
    }

    fn layout(&mut self, preview: PreviewKind, area: Rect, ctx: &mut LayoutCtx) {
        match preview {
            PreviewKind::Tabs => self.layout_tabs(area, ctx),
            PreviewKind::Panel => self.layout_panel_preview(area, ctx),
            PreviewKind::Dialog => self.layout_dialog(area, ctx),
            PreviewKind::Button => self.layout_button(area, ctx),
            PreviewKind::Toggle => self.layout_toggle(area, ctx),
            PreviewKind::TextInput => {
                let [_, input] = input_layout(area);
                ctx.push_slot(text_input_child_key(), input, |ctx| {
                    self.text_input.layout(input, ctx);
                });
            }
            PreviewKind::TextareaInput => {
                let [_, input] = input_layout(area);
                ctx.push_slot(textarea_input_child_key(), input, |ctx| {
                    self.textarea_input.layout(input, ctx);
                });
            }
            PreviewKind::DataList
            | PreviewKind::DataTable
            | PreviewKind::DataListTree
            | PreviewKind::DataTableTree
            | PreviewKind::DataSingleSelect
            | PreviewKind::DataMultiSelect
            | PreviewKind::DataChecklistTree
            | PreviewKind::DataActivateOnNavigate => {
                let [_, body] = data_view_layout(area);
                <DataView<DemoRow, usize> as TuiNode<Msg>>::layout(
                    self.active_data_view_mut(preview),
                    body,
                    ctx,
                );
            }
            PreviewKind::Dropdown => self.layout_dropdowns(area, ctx),
            PreviewKind::LayoutFlex => {
                self.layout_flex.layout(layout_demo_body(area), ctx);
            }
            PreviewKind::LayoutSplit => {
                self.layout_split.layout(layout_demo_body(area), ctx);
            }
            PreviewKind::LayoutStack => {
                self.layout_stack.layout(layout_demo_body(area), ctx);
            }
            PreviewKind::LayoutOverlay => {
                self.layout_overlay.layout(layout_demo_body(area), ctx);
            }
            PreviewKind::LayoutGrid => {
                self.layout_grid.layout(layout_demo_body(area), ctx);
            }
            _ => {}
        }
    }

    fn render(&self, preview: PreviewKind, frame: &mut Frame, area: Rect) {
        match preview {
            PreviewKind::Tabs => self.render_tabs(frame, area),
            PreviewKind::Panel => self.render_panel_preview(frame, area),
            PreviewKind::Dialog => self.render_dialog(frame, area),
            PreviewKind::Spinner => self.render_spinner(frame, area),
            PreviewKind::TextInput => self.render_text_input(frame, area),
            PreviewKind::TextareaInput => self.render_textarea_input(frame, area),
            PreviewKind::Button => self.render_button(frame, area),
            PreviewKind::Toggle => self.render_toggle(frame, area),
            PreviewKind::DataList
            | PreviewKind::DataTable
            | PreviewKind::DataListTree
            | PreviewKind::DataTableTree
            | PreviewKind::DataSingleSelect
            | PreviewKind::DataMultiSelect
            | PreviewKind::DataChecklistTree
            | PreviewKind::DataActivateOnNavigate => self.render_data_view(preview, frame, area),
            PreviewKind::Dropdown => self.render_dropdown_preview(frame, area),
            PreviewKind::LayoutFlex => self.render_layout_flex(frame, area),
            PreviewKind::LayoutSplit => self.render_layout_split(frame, area),
            PreviewKind::LayoutStack => self.render_layout_stack(frame, area),
            PreviewKind::LayoutOverlay => self.render_layout_overlay(frame, area),
            PreviewKind::LayoutGrid => self.render_layout_grid(frame, area),
        }
    }

    fn data_view_dispatch_event(
        &mut self,
        preview: PreviewKind,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<Msg>,
    ) -> EventOutcome {
        if let TuiEvent::Key(key) = event {
            if matches!(preview, PreviewKind::DataTable | PreviewKind::DataTableTree)
                && matches!(key.code, Key::Char('s'))
                && key.modifiers == KeyModifiers::NONE
            {
                self.active_data_view_mut(preview).toggle_sort("task");
                self.record_data_events(preview);
                ctx.request_redraw();
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
        }

        if !preview.is_data_view() {
            return EventOutcome::Ignored;
        }

        let outcome = self
            .active_data_view_mut(preview)
            .dispatch_event(route, event, ctx);
        self.record_data_events(preview);
        outcome
    }

    fn dispatch_event(
        &mut self,
        preview: PreviewKind,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<Msg>,
    ) -> EventOutcome {
        if preview == PreviewKind::TextInput {
            let Some(route) = route
                .path
                .without_first_if(&text_input_child_key())
                .map(EventRoute::new)
            else {
                return EventOutcome::Ignored;
            };
            return self.text_input.dispatch_event(&route, event, ctx);
        }
        if preview == PreviewKind::TextareaInput {
            let Some(route) = route
                .path
                .without_first_if(&textarea_input_child_key())
                .map(EventRoute::new)
            else {
                return EventOutcome::Ignored;
            };
            return self.textarea_input.dispatch_event(&route, event, ctx);
        }
        if preview == PreviewKind::Tabs {
            let Some((index, route)) = tab_demo_child_route(route) else {
                return EventOutcome::Ignored;
            };
            return self.tab_demo_mut(index).dispatch_event(&route, event, ctx);
        }
        if preview == PreviewKind::Toggle {
            return self.toggle.dispatch_event(route, event, ctx);
        }
        if preview == PreviewKind::Button {
            return self.button_dispatch_event(route, event, ctx);
        }
        if preview.is_data_view() {
            return self.data_view_dispatch_event(preview, route, event, ctx);
        }
        if preview == PreviewKind::Panel {
            if let Some(route) = panel_demo_child_route(route) {
                return self.panel_demo.dispatch_event(&route, event, ctx);
            }
            let Some((index, route)) = panel_title_child_route(route) else {
                return EventOutcome::Ignored;
            };
            return self
                .panel_title_dropdown_mut(index)
                .dispatch_event(&route, event, ctx);
        }
        if preview == PreviewKind::Dialog {
            let Some((index, route)) = dialog_demo_child_route(&route) else {
                return EventOutcome::Ignored;
            };
            return self
                .dialog_button_mut(index)
                .dispatch_event(&route, event, ctx);
        }
        if preview != PreviewKind::Dropdown {
            return EventOutcome::Ignored;
        }

        let Some((index, route)) = dropdown_child_route(route) else {
            return EventOutcome::Ignored;
        };
        self.dropdown_mut(index).dispatch_event(&route, event, ctx)
    }

    fn dispatch_focus(
        &mut self,
        preview: PreviewKind,
        target: &FocusTarget,
        focused: bool,
        ctx: &mut FocusCtx<Msg>,
    ) {
        match preview {
            PreviewKind::TextInput => {
                dispatch_focus_child(
                    &mut self.text_input,
                    target,
                    text_input_child_key(),
                    focused,
                    ctx,
                );
            }
            PreviewKind::TextareaInput => {
                dispatch_focus_child(
                    &mut self.textarea_input,
                    target,
                    textarea_input_child_key(),
                    focused,
                    ctx,
                );
            }
            PreviewKind::Tabs => dispatch_focus_indexed(
                target,
                tab_demo_index,
                |state, index| state.tab_demo_mut(index),
                self,
                focused,
                ctx,
            ),
            PreviewKind::Toggle => self.toggle.dispatch_focus(target, focused, ctx),
            PreviewKind::Dialog => {
                dispatch_focus_indexed(
                    target,
                    dialog_demo_index,
                    |state, index| state.dialog_button_mut(index),
                    self,
                    focused,
                    ctx,
                );
            }
            PreviewKind::Button => self.button.dispatch_focus(target, focused, ctx),
            preview if preview.is_data_view() => self
                .active_data_view_mut(preview)
                .dispatch_focus(target, focused, ctx),
            PreviewKind::Panel => {
                if !dispatch_focus_child(
                    &mut self.panel_demo,
                    target,
                    panel_demo_child_key(),
                    focused,
                    ctx,
                ) {
                    dispatch_focus_indexed(
                        target,
                        panel_title_index,
                        |state, index| state.panel_title_dropdown_mut(index),
                        self,
                        focused,
                        ctx,
                    );
                }
            }
            PreviewKind::Dropdown => dispatch_focus_indexed(
                target,
                dropdown_index,
                |state, index| state.dropdown_mut(index),
                self,
                focused,
                ctx,
            ),
            _ => {}
        }
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(&mut self.spinner, dt, settings)
            .merge(Animated::tick(&mut self.button, dt, settings))
            .merge(Animated::tick(&mut self.toggle, dt, settings))
            .merge(Animated::tick(&mut self.dialog_100, dt, settings))
            .merge(Animated::tick(&mut self.dialog_80, dt, settings))
            .merge(Animated::tick(&mut self.dialog_60, dt, settings))
            .merge(Animated::tick(&mut self.dialog_40, dt, settings))
            .merge(Animated::tick(&mut self.dialog_20, dt, settings))
            .merge(<Tabs<Msg> as TuiNode<Msg>>::tick(
                &mut self.tabs_minimal,
                dt,
                settings,
            ))
            .merge(<Tabs<Msg> as TuiNode<Msg>>::tick(
                &mut self.tabs_underline,
                dt,
                settings,
            ))
            .merge(<Tabs<Msg> as TuiNode<Msg>>::tick(
                &mut self.tabs_boxed,
                dt,
                settings,
            ))
            .merge(Animated::tick(&mut self.data_list, dt, settings))
            .merge(Animated::tick(&mut self.data_table, dt, settings))
            .merge(Animated::tick(&mut self.data_list_tree, dt, settings))
            .merge(Animated::tick(&mut self.data_table_tree, dt, settings))
            .merge(Animated::tick(&mut self.data_single_select, dt, settings))
            .merge(Animated::tick(&mut self.data_multi_select, dt, settings))
            .merge(Animated::tick(&mut self.data_checklist_tree, dt, settings))
            .merge(Animated::tick(
                &mut self.data_activate_on_navigate,
                dt,
                settings,
            ))
            .merge(Animated::tick(&mut self.panel_demo, dt, settings))
            .merge(Animated::tick(&mut self.panel_top_left, dt, settings))
            .merge(Animated::tick(&mut self.panel_top_right, dt, settings))
            .merge(Animated::tick(&mut self.panel_bottom_left, dt, settings))
            .merge(Animated::tick(&mut self.panel_bottom_right, dt, settings))
            .merge(Animated::tick(
                &mut self.dropdown_fuzzy_single,
                dt,
                settings,
            ))
            .merge(Animated::tick(
                &mut self.dropdown_multi_contains,
                dt,
                settings,
            ))
            .merge(Animated::tick(
                &mut self.dropdown_no_search_immediate,
                dt,
                settings,
            ))
            .merge(Animated::tick(
                &mut self.dropdown_filled_fuzzy_single,
                dt,
                settings,
            ))
            .merge(Animated::tick(
                &mut self.dropdown_filled_multi_contains,
                dt,
                settings,
            ))
            .merge(Animated::tick(
                &mut self.dropdown_filled_no_search_immediate,
                dt,
                settings,
            ))
            .merge(Animated::tick(&mut self.text_input, dt, settings))
            .merge(Animated::tick(&mut self.textarea_input, dt, settings))
    }

    fn panel_title_dropdown_mut(
        &mut self,
        index: usize,
    ) -> &mut Dropdown<PanelTitleChoice, &'static str> {
        match index {
            1 => &mut self.panel_top_right,
            2 => &mut self.panel_bottom_left,
            3 => &mut self.panel_bottom_right,
            _ => &mut self.panel_top_left,
        }
    }

    fn panel_title_dropdown(&self, index: usize) -> &Dropdown<PanelTitleChoice, &'static str> {
        match index {
            1 => &self.panel_top_right,
            2 => &self.panel_bottom_left,
            3 => &self.panel_bottom_right,
            _ => &self.panel_top_left,
        }
    }

    fn tab_demo_mut(&mut self, index: usize) -> &mut Tabs<Msg> {
        match index {
            1 => &mut self.tabs_underline,
            2 => &mut self.tabs_boxed,
            _ => &mut self.tabs_minimal,
        }
    }

    fn dropdown_mut(&mut self, index: usize) -> &mut Dropdown<DropdownDemoItem, &'static str> {
        match index {
            1 => &mut self.dropdown_multi_contains,
            2 => &mut self.dropdown_no_search_immediate,
            3 => &mut self.dropdown_filled_fuzzy_single,
            4 => &mut self.dropdown_filled_multi_contains,
            5 => &mut self.dropdown_filled_no_search_immediate,
            _ => &mut self.dropdown_fuzzy_single,
        }
    }

    fn active_data_view(&self, preview: PreviewKind) -> &DataView<DemoRow, usize> {
        match preview {
            PreviewKind::DataList => &self.data_list,
            PreviewKind::DataTable => &self.data_table,
            PreviewKind::DataListTree => &self.data_list_tree,
            PreviewKind::DataTableTree => &self.data_table_tree,
            PreviewKind::DataSingleSelect => &self.data_single_select,
            PreviewKind::DataMultiSelect => &self.data_multi_select,
            PreviewKind::DataChecklistTree => &self.data_checklist_tree,
            PreviewKind::DataActivateOnNavigate => &self.data_activate_on_navigate,
            _ => &self.data_list,
        }
    }

    fn active_data_view_mut(&mut self, preview: PreviewKind) -> &mut DataView<DemoRow, usize> {
        match preview {
            PreviewKind::DataList => &mut self.data_list,
            PreviewKind::DataTable => &mut self.data_table,
            PreviewKind::DataListTree => &mut self.data_list_tree,
            PreviewKind::DataTableTree => &mut self.data_table_tree,
            PreviewKind::DataSingleSelect => &mut self.data_single_select,
            PreviewKind::DataMultiSelect => &mut self.data_multi_select,
            PreviewKind::DataChecklistTree => &mut self.data_checklist_tree,
            PreviewKind::DataActivateOnNavigate => &mut self.data_activate_on_navigate,
            _ => &mut self.data_list,
        }
    }

    fn record_data_events(&mut self, preview: PreviewKind) {
        let statuses = self
            .active_data_view_mut(preview)
            .take_events()
            .into_iter()
            .map(data_event_status)
            .collect::<Vec<_>>();
        if !statuses.is_empty() {
            self.data_status = statuses.join(" • ");
        }
    }

    fn render_data_view(&self, preview: PreviewKind, frame: &mut Frame, area: Rect) {
        let [help, body] = data_view_layout(area);
        let mode = DataViewMode::from_preview(preview);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw(mode.help()),
                Span::raw("\n"),
                Span::raw(self.data_status.clone()),
            ])),
            help,
        );
        self.active_data_view(preview).render(frame, body);
    }

    fn render_dropdown_preview(&self, frame: &mut Frame, area: Rect) {
        let [help, body] = dropdown_preview_layout(area);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw("1-6 focus demo • Enter/Space opens • "),
                Span::raw("Ctrl+J/Ctrl+K navigate while typing search; Enter commit; Esc cancel; Space opens/toggles multi • "),
                Span::raw("Tab/BackTab moves across demos then out • Ctrl+Q quits"),
            ])),
            help,
        );

        let areas = dropdown_grid_areas(body);
        self.render_dropdown_column(
            frame,
            areas[0],
            0,
            "Bordered 1 • Fuzzy single",
            &format!(
                "selected: {:?}\nquery: {:?}\nSearch field is focused when opened.",
                self.dropdown_fuzzy_single.selected_id(),
                self.dropdown_fuzzy_single.search_query()
            ),
        );
        self.render_dropdown_column(
            frame,
            areas[1],
            1,
            "Bordered 2 • Contains multi",
            &format!(
                "selected: {:?}\nquery: {:?}\nSpace toggles highlighted row; Enter commits.",
                self.dropdown_multi_contains.selected_ids(),
                self.dropdown_multi_contains.search_query()
            ),
        );
        self.render_dropdown_column(
            frame,
            areas[2],
            2,
            "Bordered 3 • Centered (No search immediate)",
            &format!(
                "selected: {:?}\nquery: {:?}\nCtrl+J/Ctrl+K changes committed value while open.",
                self.dropdown_no_search_immediate.selected_id(),
                self.dropdown_no_search_immediate.search_query()
            ),
        );
        self.render_dropdown_column(
            frame,
            areas[3],
            3,
            "Filled 4 • Fuzzy single",
            &format!(
                "selected: {:?}\nquery: {:?}\nSearch field is focused when opened.",
                self.dropdown_filled_fuzzy_single.selected_id(),
                self.dropdown_filled_fuzzy_single.search_query()
            ),
        );
        self.render_dropdown_column(
            frame,
            areas[4],
            4,
            "Filled 5 • Contains multi",
            &format!(
                "selected: {:?}\nquery: {:?}\nSpace toggles highlighted row; Enter commits.",
                self.dropdown_filled_multi_contains.selected_ids(),
                self.dropdown_filled_multi_contains.search_query()
            ),
        );
        self.render_dropdown_column(
            frame,
            areas[5],
            5,
            "Filled 6 • No search immediate",
            &format!(
                "selected: {:?}\nquery: {:?}\nCtrl+J/Ctrl+K changes committed value while open.",
                self.dropdown_filled_no_search_immediate.selected_id(),
                self.dropdown_filled_no_search_immediate.search_query()
            ),
        );

        for (index, area) in areas.iter().copied().enumerate() {
            self.dropdown(index).render(frame, dropdown_area(area));
        }
        for index in 0..6 {
            self.dropdown(index).render_popup_overlay(frame, body);
        }
    }

    fn layout_dialog(&mut self, area: Rect, ctx: &mut LayoutCtx) {
        for (index, button_area) in dialog_button_areas(dialog_body_area(area))
            .into_iter()
            .enumerate()
        {
            ctx.push_slot(dialog_demo_child_key(index), button_area, |ctx| {
                self.dialog_button_mut(index).layout(button_area, ctx);
            });
        }
    }

    fn render_dialog(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(
            Paragraph::new(
                "Open app-level dialogs at different sizes. They cover the whole gallery, block sidenav hotkeys, and leave the backdrop visible unless 100%. Press x or Esc to close.",
            ),
            Rect::new(area.x, area.y, area.width, 2.min(area.height)),
        );
        let body = dialog_body_area(area);
        for (index, button_area) in dialog_button_areas(body).into_iter().enumerate() {
            self.dialog_button(index).render(frame, button_area);
        }
    }

    fn dialog_button(&self, index: usize) -> &Button<Msg> {
        match index {
            1 => &self.dialog_80,
            2 => &self.dialog_60,
            3 => &self.dialog_40,
            4 => &self.dialog_20,
            _ => &self.dialog_100,
        }
    }

    fn dialog_button_mut(&mut self, index: usize) -> &mut Button<Msg> {
        match index {
            1 => &mut self.dialog_80,
            2 => &mut self.dialog_60,
            3 => &mut self.dialog_40,
            4 => &mut self.dialog_20,
            _ => &mut self.dialog_100,
        }
    }

    fn render_dropdown_column(
        &self,
        frame: &mut Frame,
        area: Rect,
        index: usize,
        title: &str,
        details: &str,
    ) {
        let [label, _, details_area] = dropdown_column_layout(area);
        let active = self.dropdown(index).is_focused();
        let marker = if active { "▶ " } else { "  " };
        let style = if active {
            Style::default()
                .fg(tuicore::theme().accent_fg())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(tuicore::theme().text_fg())
        };
        frame.render_widget(
            Paragraph::new(format!("{marker}{}", title)).style(style),
            label,
        );
        frame.render_widget(Paragraph::new(details.to_string()), details_area);
    }

    fn layout_dropdowns(&mut self, area: Rect, ctx: &mut LayoutCtx) {
        let [_, body] = dropdown_preview_layout(area);
        let grid_areas = dropdown_grid_areas(body);
        let areas = grid_areas.map(dropdown_area);

        ctx.push_slot(dropdown_child_key(0), areas[0], |ctx| {
            self.dropdown_fuzzy_single
                .layout_overlay::<Msg>(areas[0], body, ctx);
        });
        ctx.push_slot(dropdown_child_key(1), areas[1], |ctx| {
            self.dropdown_multi_contains
                .layout_overlay::<Msg>(areas[1], body, ctx);
        });
        ctx.push_slot(dropdown_child_key(2), areas[2], |ctx| {
            self.dropdown_no_search_immediate
                .layout_overlay::<Msg>(areas[2], body, ctx);
        });
        ctx.push_slot(dropdown_child_key(3), areas[3], |ctx| {
            self.dropdown_filled_fuzzy_single
                .layout_overlay::<Msg>(areas[3], body, ctx);
        });
        ctx.push_slot(dropdown_child_key(4), areas[4], |ctx| {
            self.dropdown_filled_multi_contains
                .layout_overlay::<Msg>(areas[4], body, ctx);
        });
        ctx.push_slot(dropdown_child_key(5), areas[5], |ctx| {
            self.dropdown_filled_no_search_immediate
                .layout_overlay::<Msg>(areas[5], body, ctx);
        });
    }

    fn dropdown(&self, index: usize) -> &Dropdown<DropdownDemoItem, &'static str> {
        match index {
            1 => &self.dropdown_multi_contains,
            2 => &self.dropdown_no_search_immediate,
            3 => &self.dropdown_filled_fuzzy_single,
            4 => &self.dropdown_filled_multi_contains,
            5 => &self.dropdown_filled_no_search_immediate,
            _ => &self.dropdown_fuzzy_single,
        }
    }

    fn sync_panel_demo_from_dropdowns(&mut self) {
        self.panel_demo.clear_title(PanelTitlePosition::TopLeft);
        self.panel_demo.clear_title(PanelTitlePosition::TopRight);
        self.panel_demo.clear_title(PanelTitlePosition::BottomLeft);
        self.panel_demo.clear_title(PanelTitlePosition::BottomRight);
        apply_panel_choice(
            &mut self.panel_demo,
            PanelTitlePosition::TopLeft,
            self.panel_top_left.selected_id(),
        );
        apply_panel_choice(
            &mut self.panel_demo,
            PanelTitlePosition::TopRight,
            self.panel_top_right.selected_id(),
        );
        apply_panel_choice(
            &mut self.panel_demo,
            PanelTitlePosition::BottomLeft,
            self.panel_bottom_left.selected_id(),
        );
        apply_panel_choice(
            &mut self.panel_demo,
            PanelTitlePosition::BottomRight,
            self.panel_bottom_right.selected_id(),
        );
    }

    fn layout_panel_preview(&mut self, area: Rect, ctx: &mut LayoutCtx) {
        let [_, controls, panel_area] = panel_preview_layout(area);
        let areas = panel_title_control_areas(controls).map(panel_title_dropdown_area);

        self.sync_panel_demo_from_dropdowns();
        ctx.push_slot(panel_demo_child_key(), panel_area, |ctx| {
            <Panel as TuiNode<Msg>>::layout(&mut self.panel_demo, panel_area, ctx);
        });

        ctx.push_slot(panel_title_child_key(0), areas[0], |ctx| {
            self.panel_top_left
                .layout_overlay::<Msg>(areas[0], area, ctx);
        });
        ctx.push_slot(panel_title_child_key(1), areas[1], |ctx| {
            self.panel_top_right
                .layout_overlay::<Msg>(areas[1], area, ctx);
        });
        ctx.push_slot(panel_title_child_key(2), areas[2], |ctx| {
            self.panel_bottom_left
                .layout_overlay::<Msg>(areas[2], area, ctx);
        });
        ctx.push_slot(panel_title_child_key(3), areas[3], |ctx| {
            self.panel_bottom_right
                .layout_overlay::<Msg>(areas[3], area, ctx);
        });
    }

    fn render_text_input(&self, frame: &mut Frame, area: Rect) {
        let [instructions, input] = input_layout(area);
        frame.render_widget(
            Paragraph::new(
                "Type text. Enter submits. Tab returns to list. Ctrl+Q quits from gallery root.\n\
                 Shortcuts:\n\
                 • Ctrl+Left / Ctrl+Right / Alt+B / Alt+F : Jump word backward / forward\n\
                 • Ctrl+Backspace / Ctrl+W                : Delete word backward\n\
                 • Ctrl+Delete / Alt+D                    : Delete word forward\n\
                 • Ctrl+A / Ctrl+E                        : Move cursor to start / end of line\n\
                 • Ctrl+U / Ctrl+K                        : Delete to start / end of line\n\
                 • Ctrl+C                                 : Clear input\n\
                 • Ctrl+O                                 : Edit in external editor ($EDITOR)",
            ),
            instructions,
        );
        self.text_input.render(frame, input);
    }

    fn render_textarea_input(&self, frame: &mut Frame, area: Rect) {
        let [instructions, input] = textarea_layout(area);
        frame.render_widget(
            Paragraph::new(
                "Type text. Enter inserts newline. Ctrl+Enter/Ctrl+D submits. Tab returns to list. Ctrl+Q quits from gallery root.\n\
                 Shortcuts:\n\
                 • Ctrl+Left / Ctrl+Right / Alt+B / Alt+F : Jump word backward / forward\n\
                 • Ctrl+P / Ctrl+N                        : Move cursor up / down a line\n\
                 • Ctrl+Backspace / Ctrl+W                : Delete word backward\n\
                 • Ctrl+Delete / Alt+D                    : Delete word forward\n\
                 • Ctrl+A / Ctrl+E                        : Move cursor to start / end of line\n\
                 • Ctrl+U / Ctrl+K                        : Delete to start / end of line\n\
                 • Ctrl+C                                 : Clear input\n\
                 • Ctrl+O                                 : Edit in external editor ($EDITOR)",
            ),
            instructions,
        );
        self.textarea_input.render(frame, input);
    }

    fn layout_button(&mut self, area: Rect, ctx: &mut LayoutCtx) {
        let [_, button_area, _] = button_layout(area);
        self.button.layout(button_area, ctx);
    }

    fn render_button(&self, frame: &mut Frame, area: Rect) {
        let [instructions, button_area, status] = button_layout(area);
        frame.render_widget(
            Paragraph::new(format!(
                "{} presses. Press b from anywhere in this preview to focus and press.",
                tuicore::keybindings().button().press_label()
            )),
            instructions,
        );
        self.button.render(frame, button_area);
        frame.render_widget(
            Paragraph::new(format!("Pressed {} times", self.button_presses)),
            status,
        );
    }

    fn button_dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<Msg>,
    ) -> EventOutcome {
        let before = ctx.messages().len();
        let outcome = self.button.dispatch_event(route, event, ctx);
        if outcome.handled() && ctx.messages().len() == before {
            self.button_presses += 1;
        }
        outcome
    }

    fn layout_toggle(&mut self, area: Rect, ctx: &mut LayoutCtx) {
        let [_, toggle_area] = toggle_layout(area);
        self.toggle.layout(toggle_area, ctx);
    }

    fn render_toggle(&self, frame: &mut Frame, area: Rect) {
        let [instructions, toggle_area] = toggle_layout(area);
        frame.render_widget(
            Paragraph::new(
                "Enter/Space toggles. Press x from anywhere in this preview to focus and toggle.",
            ),
            instructions,
        );
        self.toggle.render(frame, toggle_area);
    }

    fn render_spinner(&self, frame: &mut Frame, area: Rect) {
        let [help, spinner] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Fill(1)])
            .areas(area);
        frame.render_widget(
            Paragraph::new("Spinner uses tuicore animation tick. Focus stays in gallery shell."),
            help,
        );
        self.spinner.render(frame, spinner);
    }

    fn render_panel_preview(&self, frame: &mut Frame, area: Rect) {
        let [help, controls, panel_area] = panel_preview_layout(area);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw("Enter/Space opens controls • "),
                Span::raw("Tab/BackTab moves through preview"),
            ])),
            help,
        );

        self.panel_demo.render(frame, panel_area);

        let areas = panel_title_control_areas(controls);
        self.render_panel_title_control(frame, areas[0], 0, "Top left");
        self.render_panel_title_control(frame, areas[1], 1, "Top right");
        self.render_panel_title_control(frame, areas[2], 2, "Bottom left");
        self.render_panel_title_control(frame, areas[3], 3, "Bottom right");

        for index in 0..PANEL_TITLE_CONTROL_COUNT {
            self.panel_title_dropdown(index)
                .render_popup_overlay(frame, area);
        }
    }

    fn render_panel_title_control(&self, frame: &mut Frame, area: Rect, index: usize, title: &str) {
        let [label, field] = panel_title_column_layout(area);
        let active = self.panel_title_dropdown(index).is_focused();
        let marker = if active { "▶ " } else { "  " };
        let style = if active {
            Style::default()
                .fg(tuicore::theme().accent_fg())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(tuicore::theme().text_fg())
        };
        frame.render_widget(
            Paragraph::new(format!("{marker}{}", title)).style(style),
            label,
        );
        self.panel_title_dropdown(index).render(frame, field);
    }

    fn layout_tabs(&mut self, area: Rect, ctx: &mut LayoutCtx) {
        let [minimal, underline, boxed] = tabs_areas(area);
        let [_, minimal_tabs] = labeled_area(minimal);
        let [_, underline_tabs] = labeled_area(underline);
        let [_, boxed_tabs] = labeled_area(boxed);
        ctx.push_slot(tab_demo_child_key(0), minimal_tabs, |ctx| {
            self.tabs_minimal.layout(minimal_tabs, ctx);
        });
        ctx.push_slot(tab_demo_child_key(1), underline_tabs, |ctx| {
            self.tabs_underline.layout(underline_tabs, ctx);
        });
        ctx.push_slot(tab_demo_child_key(2), boxed_tabs, |ctx| {
            self.tabs_boxed.layout(boxed_tabs, ctx);
        });
    }

    fn render_tabs(&self, frame: &mut Frame, area: Rect) {
        let [minimal, underline, boxed] = tabs_areas(area);
        let [minimal_label, minimal_tabs] = labeled_area(minimal);
        let [underline_label, underline_tabs] = labeled_area(underline);
        let [boxed_label, boxed_tabs] = labeled_area(boxed);

        frame.render_widget(Paragraph::new("Style 1: minimal (m)"), minimal_label);
        self.tabs_minimal.render(frame, minimal_tabs);
        frame.render_widget(Paragraph::new("Style 2: underline (l)"), underline_label);
        self.tabs_underline.render(frame, underline_tabs);
        frame.render_widget(Paragraph::new("Style 3: boxed (b)"), boxed_label);
        self.tabs_boxed.render(frame, boxed_tabs);
    }

    fn render_layout_flex(&self, frame: &mut Frame, area: Rect) {
        render_layout_intro(
            frame,
            area,
            "Flex: fixed + fit-content + fill with gap 2 and horizontal/vertical padding 2/1.",
        );
        self.layout_flex.render(frame, layout_demo_body(area));
    }

    fn render_layout_split(&self, frame: &mut Frame, area: Rect) {
        render_layout_intro(
            frame,
            area,
            "Split: two panes with ratio/content+fill style composition.",
        );
        self.layout_split.render(frame, layout_demo_body(area));
    }

    fn render_layout_stack(&self, frame: &mut Frame, area: Rect) {
        render_layout_intro(
            frame,
            area,
            "Stack: children share one area; later layers render on top with alignment/inset.",
        );
        self.layout_stack.render(frame, layout_demo_body(area));
    }

    fn render_layout_overlay(&self, frame: &mut Frame, area: Rect) {
        render_layout_intro(
            frame,
            area,
            "Overlay: base gets normal flow; anchored layer floats without taking height.",
        );
        self.layout_overlay.render(frame, layout_demo_body(area));
    }

    fn render_layout_grid(&self, frame: &mut Frame, area: Rect) {
        render_layout_intro(
            frame,
            area,
            "Grid: tracks mix fixed/fit/percent/fill with row gap 1, column gap 2, padding 1.",
        );
        self.layout_grid.render(frame, layout_demo_body(area));
    }
}

#[derive(Clone)]
struct DemoBox {
    title: &'static str,
    body: &'static str,
    size: LayoutSize,
}

impl DemoBox {
    fn new(title: &'static str, body: &'static str, width: u16, height: u16) -> Self {
        Self {
            title,
            body,
            size: LayoutSize::new(width, height),
        }
    }
}

impl TuiNode<Msg> for DemoBox {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        LayoutSizeHint {
            source: HintSource::Measured,
            min: LayoutSize::new(1, 1),
            preferred: self.size,
            expand: Default::default(),
        }
        .normalized(proposal)
    }

    fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        let title_style = Style::default()
            .fg(tuicore::theme().muted_fg())
            .add_modifier(Modifier::BOLD);
        let lines = vec![
            Line::from(Span::styled(self.title, title_style)),
            Line::from(self.body),
            Line::from(format!("rect: {}×{}", area.width, area.height)),
        ];
        frame.render_widget(Paragraph::new(lines), area);
    }
}

fn layout_flex_demo() -> Flex<Msg> {
    Flex::row()
        .padding(tuicore::Padding::horizontal_vertical(2, 1))
        .gap(2)
        .separator(Separator::new().role(SeparatorColorRole::Subtle))
        .child(
            "fixed",
            DemoBox::new("Fixed", "12 cols", 12, 3),
            FlexItem::fixed(12),
        )
        .child(
            "fit",
            DemoBox::new("FitContent", "measured child", 18, 3),
            FlexItem::fit_content(),
        )
        .child(
            "fill",
            DemoBox::new("Fill", "takes the rest", 12, 3),
            FlexItem::fill(1),
        )
}

fn layout_split_demo() -> Split<DemoBox, DemoBox> {
    Split::horizontal(
        DemoBox::new("Navigation", "ratio side pane", 20, 8),
        DemoBox::new("Workspace", "main region receives remainder", 40, 8),
    )
    .ratio(1, 2)
    .gap(1)
    .separator(Separator::new().role(SeparatorColorRole::Muted))
}

fn layout_stack_demo() -> Stack<Msg> {
    Stack::new()
        .child(
            "base",
            DemoBox::new("Base layer", "fills all available space", 30, 8),
            StackItem::new(),
        )
        .child(
            "center",
            DemoBox::new("Centered empty state", "fit-content layer", 26, 4),
            StackItem::new()
                .fit_content()
                .align(StackAlign::Center, StackAlign::Center),
        )
        .child(
            "badge",
            DemoBox::new("Badge", "top right", 18, 3),
            StackItem::new()
                .fixed(18, 3)
                .align(StackAlign::End, StackAlign::Start)
                .inset(tuicore::Padding::all(1)),
        )
}

fn layout_overlay_demo() -> Overlay<DemoBox, DemoBox> {
    Overlay::new(
        DemoBox::new(
            "Base content",
            "normal flow size comes from this child",
            32,
            8,
        ),
        DemoBox::new("Popover", "anchored overlay", 24, 5),
    )
    .anchor(OverlayAnchor::BottomRight)
    .layer_size(OverlaySize::FitContent)
}

fn layout_grid_demo() -> Grid<Msg> {
    Grid::new()
        .columns([
            GridTrack::fixed(14),
            GridTrack::fit_content(),
            GridTrack::fill(1),
        ])
        .rows([
            GridTrack::fixed(4),
            GridTrack::percent(35),
            GridTrack::fill(1),
        ])
        .gaps(Gap::new(1, 2))
        .separator(Separator::new().role(SeparatorColorRole::Muted))
        .padding(tuicore::Padding::all(1))
        .child(
            "filters",
            DemoBox::new("Filters", "fixed track", 10, 3),
            GridItem::new(0, 0),
        )
        .child(
            "summary",
            DemoBox::new("Summary", "fit-content track", 18, 3),
            GridItem::new(0, 1),
        )
        .child(
            "chart",
            DemoBox::new("Chart", "fills remaining width", 28, 8),
            GridItem::new(0, 2).span(2, 1),
        )
        .child(
            "table",
            DemoBox::new("Table", "spans first two columns", 30, 8),
            GridItem::new(1, 0).span(2, 2),
        )
}

fn render_layout_intro(frame: &mut Frame, area: Rect, text: &'static str) {
    frame.render_widget(Paragraph::new(text), layout_demo_header(area));
}

fn layout_demo_header(area: Rect) -> Rect {
    layout_demo_areas(area)[0]
}

fn layout_demo_body(area: Rect) -> Rect {
    layout_demo_areas(area)[1]
}

fn gallery_list_child_key() -> ChildKey {
    ChildKey::new("component-list")
}

fn gallery_preview_child_key() -> ChildKey {
    ChildKey::new("preview")
}

fn text_input_child_key() -> ChildKey {
    ChildKey::new("text-input")
}

fn textarea_input_child_key() -> ChildKey {
    ChildKey::new("textarea-input")
}

fn dialog_demo_child_key(index: usize) -> ChildKey {
    ChildKey::new(format!("dialog-demo-{index}"))
}

fn dialog_tab_child_key(key: &'static str) -> ChildKey {
    ChildKey::new(key)
}

fn dialog_demo_index(key: &ChildKey) -> Option<usize> {
    key.as_str().strip_prefix("dialog-demo-")?.parse().ok()
}

fn dialog_demo_child_route(route: &EventRoute) -> Option<(usize, EventRoute)> {
    let first = route.path.first()?;
    let index = dialog_demo_index(first)?;
    Some((index, EventRoute::new(route.path.without_first())))
}

fn dispatch_focus_child<N>(
    node: &mut N,
    target: &FocusTarget,
    key: ChildKey,
    focused: bool,
    ctx: &mut FocusCtx<Msg>,
) -> bool
where
    N: TuiNode<Msg>,
{
    let Some(target) = target.for_child(&key) else {
        return false;
    };
    node.dispatch_focus(&target, focused, ctx);
    true
}

fn dispatch_focus_indexed<N>(
    target: &FocusTarget,
    index_for: fn(&ChildKey) -> Option<usize>,
    node_for: impl for<'a> FnOnce(&'a mut PreviewState, usize) -> &'a mut N,
    state: &mut PreviewState,
    focused: bool,
    ctx: &mut FocusCtx<Msg>,
) where
    N: TuiNode<Msg>,
{
    let Some((index, target)) = indexed_child_target(target, index_for) else {
        return;
    };
    node_for(state, index).dispatch_focus(&target, focused, ctx);
}

fn indexed_child_target(
    target: &FocusTarget,
    index_for: fn(&ChildKey) -> Option<usize>,
) -> Option<(usize, FocusTarget)> {
    let first = target.path.first()?;
    let index = index_for(first)?;
    Some((index, target.for_child(first)?))
}

fn layout_demo_areas(area: Rect) -> [Rect; 2] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Fill(1)])
        .areas(area)
}

fn dialog_body_area(area: Rect) -> Rect {
    Rect::new(
        area.x,
        area.y.saturating_add(2),
        area.width,
        area.height.saturating_sub(2),
    )
}

fn dialog_button_areas(area: Rect) -> [Rect; 5] {
    let [_, body, _] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(9.min(area.height)),
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
        ])
        .spacing(1)
        .areas(body)
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
enum ComponentKind {
    Tabs,
    Panel,
    Dialog,
    Spinner,
    Layouts,
    LayoutFlex,
    LayoutSplit,
    LayoutStack,
    LayoutOverlay,
    LayoutGrid,
    Inputs,
    Button,
    TextInput,
    TextareaInput,
    Toggle,
    Dropdown,
    DataView,
    DataViewList,
    DataViewTable,
    DataViewListTree,
    DataViewTableTree,
    DataViewSingleSelect,
    DataViewMultiSelect,
    DataViewChecklistTree,
    DataViewActivateOnNavigate,
}

impl ComponentKind {
    const ALL: [Self; 25] = [
        Self::Tabs,
        Self::Panel,
        Self::Dialog,
        Self::Spinner,
        Self::Layouts,
        Self::LayoutFlex,
        Self::LayoutSplit,
        Self::LayoutStack,
        Self::LayoutOverlay,
        Self::LayoutGrid,
        Self::Inputs,
        Self::Button,
        Self::TextInput,
        Self::TextareaInput,
        Self::Toggle,
        Self::Dropdown,
        Self::DataView,
        Self::DataViewList,
        Self::DataViewTable,
        Self::DataViewListTree,
        Self::DataViewTableTree,
        Self::DataViewSingleSelect,
        Self::DataViewMultiSelect,
        Self::DataViewChecklistTree,
        Self::DataViewActivateOnNavigate,
    ];

    fn title(self) -> &'static str {
        match self {
            Self::Tabs => "Tabs",
            Self::Panel => "Panels",
            Self::Dialog => "Dialog",
            Self::Spinner => "Spinner",
            Self::Layouts => "Layouts",
            Self::LayoutFlex => "Flex",
            Self::LayoutSplit => "Split",
            Self::LayoutStack => "Stack",
            Self::LayoutOverlay => "Overlay",
            Self::LayoutGrid => "Grid",
            Self::Inputs => "Inputs",
            Self::Button => "Button",
            Self::TextInput => "Text",
            Self::TextareaInput => "Textarea",
            Self::Toggle => "Toggle",
            Self::Dropdown => "Dropdown",
            Self::DataView => "DataView",
            Self::DataViewList => "List",
            Self::DataViewTable => "Table",
            Self::DataViewListTree => "List Tree",
            Self::DataViewTableTree => "Table Tree",
            Self::DataViewSingleSelect => "Single Select",
            Self::DataViewMultiSelect => "Multi Select",
            Self::DataViewChecklistTree => "Tree Checklist",
            Self::DataViewActivateOnNavigate => "Activate On Navigate",
        }
    }

    fn parent(self) -> Option<Self> {
        match self {
            Self::DataViewList
            | Self::DataViewTable
            | Self::DataViewListTree
            | Self::DataViewTableTree
            | Self::DataViewSingleSelect
            | Self::DataViewMultiSelect
            | Self::DataViewChecklistTree
            | Self::DataViewActivateOnNavigate => Some(Self::DataView),
            Self::Button
            | Self::TextInput
            | Self::TextareaInput
            | Self::Toggle
            | Self::Dropdown => Some(Self::Inputs),
            Self::LayoutFlex
            | Self::LayoutSplit
            | Self::LayoutStack
            | Self::LayoutOverlay
            | Self::LayoutGrid => Some(Self::Layouts),
            _ => None,
        }
    }

    fn preview(self) -> PreviewKind {
        match self {
            Self::Tabs => PreviewKind::Tabs,
            Self::Panel => PreviewKind::Panel,
            Self::Dialog => PreviewKind::Dialog,
            Self::Spinner => PreviewKind::Spinner,
            Self::Layouts | Self::LayoutFlex => PreviewKind::LayoutFlex,
            Self::LayoutSplit => PreviewKind::LayoutSplit,
            Self::LayoutStack => PreviewKind::LayoutStack,
            Self::LayoutOverlay => PreviewKind::LayoutOverlay,
            Self::LayoutGrid => PreviewKind::LayoutGrid,
            Self::Inputs | Self::Button => PreviewKind::Button,
            Self::TextInput => PreviewKind::TextInput,
            Self::TextareaInput => PreviewKind::TextareaInput,
            Self::Toggle => PreviewKind::Toggle,
            Self::Dropdown => PreviewKind::Dropdown,
            Self::DataView | Self::DataViewList => PreviewKind::DataList,
            Self::DataViewTable => PreviewKind::DataTable,
            Self::DataViewListTree => PreviewKind::DataListTree,
            Self::DataViewTableTree => PreviewKind::DataTableTree,
            Self::DataViewSingleSelect => PreviewKind::DataSingleSelect,
            Self::DataViewMultiSelect => PreviewKind::DataMultiSelect,
            Self::DataViewChecklistTree => PreviewKind::DataChecklistTree,
            Self::DataViewActivateOnNavigate => PreviewKind::DataActivateOnNavigate,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewKind {
    Tabs,
    Panel,
    Dialog,
    Spinner,
    LayoutFlex,
    LayoutSplit,
    LayoutStack,
    LayoutOverlay,
    LayoutGrid,
    TextInput,
    TextareaInput,
    Button,
    Toggle,
    Dropdown,
    DataList,
    DataTable,
    DataListTree,
    DataTableTree,
    DataSingleSelect,
    DataMultiSelect,
    DataChecklistTree,
    DataActivateOnNavigate,
}

impl PreviewKind {
    fn title(self) -> &'static str {
        match self {
            Self::Tabs => "Tabs",
            Self::Panel => "Panels",
            Self::Dialog => "Dialog",
            Self::Spinner => "Spinner",
            Self::LayoutFlex => "Flex Layout",
            Self::LayoutSplit => "Split Layout",
            Self::LayoutStack => "Stack Layout",
            Self::LayoutOverlay => "Overlay Layout",
            Self::LayoutGrid => "Grid Layout",
            Self::TextInput => "Text",
            Self::TextareaInput => "Textarea",
            Self::Button => "Button",
            Self::Toggle => "Toggle",
            Self::Dropdown => "Dropdown",
            Self::DataList => "List",
            Self::DataTable => "Table",
            Self::DataListTree => "List Tree",
            Self::DataTableTree => "Table Tree",
            Self::DataSingleSelect => "Single Select",
            Self::DataMultiSelect => "Multi Select",
            Self::DataChecklistTree => "Tree Checklist",
            Self::DataActivateOnNavigate => "Activate On Navigate",
        }
    }

    fn is_data_view(self) -> bool {
        matches!(
            self,
            Self::DataList
                | Self::DataTable
                | Self::DataListTree
                | Self::DataTableTree
                | Self::DataSingleSelect
                | Self::DataMultiSelect
                | Self::DataChecklistTree
                | Self::DataActivateOnNavigate
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DataViewMode {
    List,
    Table,
    ListTree,
    TableTree,
    SingleSelect,
    MultiSelect,
    ChecklistTree,
    ActivateOnNavigate,
}

impl DataViewMode {
    fn from_preview(preview: PreviewKind) -> Self {
        match preview {
            PreviewKind::DataTable => Self::Table,
            PreviewKind::DataListTree => Self::ListTree,
            PreviewKind::DataTableTree => Self::TableTree,
            PreviewKind::DataSingleSelect => Self::SingleSelect,
            PreviewKind::DataMultiSelect => Self::MultiSelect,
            PreviewKind::DataChecklistTree => Self::ChecklistTree,
            PreviewKind::DataActivateOnNavigate => Self::ActivateOnNavigate,
            _ => Self::List,
        }
    }

    fn help(self) -> String {
        let bindings = tuicore::keybindings();
        let data_keys = bindings.data_view();
        let scroll_keys = format!(
            "{}/{}",
            bindings.line_up_label(),
            bindings.line_down_label()
        );
        let all_tree_keys = format!(
            "{}/{}",
            data_keys.collapse_all_label(),
            data_keys.expand_all_label()
        );
        match self {
            Self::List => format!(
                "100 rows • one column • no header • {scroll_keys} scroll • {} activates row",
                data_keys.activate_label()
            ),
            Self::Table => {
                format!(
                    "100 rows • headers + rich cells • {scroll_keys} scroll • s sorts task column"
                )
            }
            Self::ListTree => format!(
                "100 rows • {} node • {all_tree_keys} collapse/expand all • using tree glyphs /",
                data_keys.toggle_expansion_label()
            ),
            Self::TableTree => format!(
                "100 rows • rich cells • {} node • {all_tree_keys} all • s sorts • using tree glyphs /",
                data_keys.toggle_expansion_label()
            ),
            Self::SingleSelect => format!(
                "{} toggles row • {} selects + activates • single selected ID",
                data_keys.toggle_selection_label(),
                data_keys.activate_label()
            ),
            Self::MultiSelect => format!(
                "{} or {} toggles rows • selected IDs stay in source order",
                data_keys.activate_label(),
                data_keys.toggle_selection_label()
            ),
            Self::ChecklistTree => format!(
                "{} or {} cascades descendants • Nerd Font mixed icon",
                data_keys.activate_label(),
                data_keys.toggle_selection_label()
            ),
            Self::ActivateOnNavigate => {
                format!(
                    "{scroll_keys} changes active + selected row immediately • dropdown-style preview"
                )
            }
        }
    }

    fn data_view(self) -> DataView<DemoRow, usize> {
        let rows = demo_rows();
        let expanded = rows
            .iter()
            .filter(|row| row.parent.is_none() || (1..4).contains(&(row.id % 10)))
            .map(|row| row.id)
            .collect::<Vec<_>>();

        match self {
            Self::List => DataView::list(rows, |row| row.id, |row| row.name.clone()),
            Self::Table => DataView::new(rows, |row| row.id)
                .headers(true)
                .columns(demo_columns()),
            Self::ListTree => DataView::list(rows, |row| row.id, |row| row.name.clone())
                .tree(TreeAdapter::parent_id(|row: &DemoRow| row.parent))
                .tree_glyphs(TreeGlyphs::NERD_FONT)
                .expanded(expanded),
            Self::TableTree => DataView::new(rows, |row| row.id)
                .headers(true)
                .columns(demo_columns())
                .tree(TreeAdapter::parent_id(|row: &DemoRow| row.parent))
                .tree_glyphs(TreeGlyphs::NERD_FONT)
                .expanded(expanded),
            Self::SingleSelect => DataView::list(rows, |row| row.id, |row| row.name.clone())
                .selection_mode(SelectionMode::Single)
                .selection_trigger(SelectionTrigger::OnActivate),
            Self::MultiSelect => DataView::new(rows, |row| row.id)
                .headers(true)
                .columns(demo_columns())
                .selection_mode(SelectionMode::Multi)
                .selection_trigger(SelectionTrigger::OnActivate),
            Self::ChecklistTree => DataView::list(rows, |row| row.id, |row| row.name.clone())
                .tree(TreeAdapter::parent_id(|row: &DemoRow| row.parent))
                .tree_glyphs(TreeGlyphs::NERD_FONT)
                .selection_mode(SelectionMode::Multi)
                .selection_trigger(SelectionTrigger::OnActivate)
                .selection_propagation(SelectionPropagation::CascadeDescendants)
                .selection_glyphs(SelectionGlyphs::NERD_FONT)
                .expanded(expanded),
            Self::ActivateOnNavigate => DataView::list(rows, |row| row.id, |row| row.name.clone())
                .activation_mode(ActivationMode::OnNavigate)
                .selection_mode(SelectionMode::Single)
                .selection_trigger(SelectionTrigger::OnNavigate),
        }
    }
}

#[derive(Debug, Clone)]
struct DemoRow {
    id: usize,
    parent: Option<usize>,
    name: String,
    owner: &'static str,
    status: Status,
    progress: u8,
}

#[derive(Clone)]
struct DropdownDemoItem {
    id: &'static str,
    label: &'static str,
}

fn dropdown_items() -> Vec<DropdownDemoItem> {
    vec![
        DropdownDemoItem {
            id: "alpha",
            label: "Alpha backlog",
        },
        DropdownDemoItem {
            id: "beta",
            label: "Beta build",
        },
        DropdownDemoItem {
            id: "gamma",
            label: "Gamma release",
        },
        DropdownDemoItem {
            id: "delta",
            label: "Delta docs",
        },
        DropdownDemoItem {
            id: "omega",
            label: "Omega ops",
        },
    ]
}

fn dropdown_fuzzy_single() -> Dropdown<DropdownDemoItem, &'static str> {
    Dropdown::single(dropdown_items(), |row| row.id, |row| row.label.to_string())
        .placeholder("Pick release lane...")
        .selected_one("gamma")
        .label("Lane")
        .hotkey("1")
}

fn dropdown_multi_contains() -> Dropdown<DropdownDemoItem, &'static str> {
    Dropdown::multi(dropdown_items(), |row| row.id, |row| row.label.to_string())
        .placeholder("Pick workstreams...")
        .search_mode(DropdownSearchMode::Contains)
        .selected(["alpha", "delta"])
        .label("Work")
        .hotkey("2")
}

fn dropdown_no_search_immediate() -> Dropdown<DropdownDemoItem, &'static str> {
    Dropdown::single(dropdown_items(), |row| row.id, |row| row.label.to_string())
        .placeholder("Immediate lane...")
        .search_mode(DropdownSearchMode::None)
        .commit_mode(DropdownCommitMode::Immediate)
        .centered(true)
        .selected_one("beta")
        .label("Immediate")
        .hotkey("3")
}

fn dropdown_filled_fuzzy_single() -> Dropdown<DropdownDemoItem, &'static str> {
    dropdown_fuzzy_single()
        .variant(DropdownVariant::Filled)
        .label("Lane")
        .hotkey("4")
        .alt_style(true)
}

fn dropdown_filled_multi_contains() -> Dropdown<DropdownDemoItem, &'static str> {
    dropdown_multi_contains()
        .variant(DropdownVariant::Filled)
        .label("Work")
        .hotkey("5")
        .alt_style(true)
}

fn dropdown_filled_no_search_immediate() -> Dropdown<DropdownDemoItem, &'static str> {
    dropdown_no_search_immediate()
        .variant(DropdownVariant::Filled)
        .label("Immediate")
        .hotkey("6")
        .alt_style(true)
}

#[derive(Clone)]
struct PanelTitleChoice {
    id: &'static str,
    label: &'static str,
    enabled: bool,
}

fn panel_title_choices(position: PanelTitlePosition) -> Vec<PanelTitleChoice> {
    let enabled_label = match position {
        PanelTitlePosition::BottomRight => "show hotkey",
        _ => "show label",
    };
    vec![
        PanelTitleChoice {
            id: "none",
            label: "none",
            enabled: false,
        },
        PanelTitleChoice {
            id: "show",
            label: enabled_label,
            enabled: true,
        },
    ]
}

fn panel_demo() -> Panel {
    Panel::new().border(BorderKind::Plain).content([
        "Use dropdowns below to toggle each panel label or hotkey.",
        "Top labels use the standard - label - style.",
        "Bottom labels and hotkeys use the -| label |- inset style.",
    ])
}

fn dialog_button(example: DialogExample) -> Button<Msg> {
    Button::new(example.button_label())
        .hotkey(example.hotkey())
        .on_press(move || Msg::DialogOpened(example))
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

fn gallery_dialog() -> DialogHost<GalleryDialogContent, Msg> {
    let mut dialog = Dialog::new()
        .top_left(DialogExample::Large.title())
        .bottom_left("Esc blurs")
        .bottom_right("80% viewport")
        .on_close(Msg::DialogClosed);
    dialog.clear_title(tuicore::DialogTitlePosition::TopRight);
    dialog.host(GalleryDialogContent::new())
}

fn panel_title_dropdown(position: PanelTitlePosition) -> Dropdown<PanelTitleChoice, &'static str> {
    Dropdown::single(
        panel_title_choices(position),
        |row| row.id,
        |row| row.label.to_string(),
    )
    .placeholder(panel_title_placeholder(position))
    .selected_one("show")
    .label(panel_title_control_label(position))
    .hotkey(panel_title_control_hotkey(position))
}

fn panel_title_placeholder(position: PanelTitlePosition) -> &'static str {
    match position {
        PanelTitlePosition::TopLeft => "Top left title...",
        PanelTitlePosition::TopRight => "Top right title...",
        PanelTitlePosition::BottomLeft => "Bottom left title...",
        PanelTitlePosition::BottomRight => "Panel hotkey...",
    }
}

fn panel_title_control_label(position: PanelTitlePosition) -> &'static str {
    match position {
        PanelTitlePosition::TopLeft => "Top left",
        PanelTitlePosition::TopRight => "Top right",
        PanelTitlePosition::BottomLeft => "Bottom left",
        PanelTitlePosition::BottomRight => "Hotkey",
    }
}

fn panel_title_control_hotkey(position: PanelTitlePosition) -> &'static str {
    match position {
        PanelTitlePosition::TopLeft => "q",
        PanelTitlePosition::TopRight => "w",
        PanelTitlePosition::BottomLeft => "e",
        PanelTitlePosition::BottomRight => "r",
    }
}

fn apply_panel_choice(
    panel: &mut Panel,
    position: PanelTitlePosition,
    selected: Option<&'static str>,
) {
    let Some(choice) = panel_title_choices(position)
        .into_iter()
        .find(|choice| Some(choice.id) == selected)
    else {
        return;
    };
    if !choice.enabled {
        panel.clear_title(position);
        return;
    }

    match position {
        PanelTitlePosition::TopLeft => panel.set_top_left("top left"),
        PanelTitlePosition::TopRight => panel.set_top_right("top right"),
        PanelTitlePosition::BottomLeft => panel.set_bottom_left("bottom left"),
        PanelTitlePosition::BottomRight => panel.set_hotkey("p"),
    }
}

#[derive(Debug, Clone, Copy)]
enum Status {
    Ready,
    Active,
    Blocked,
}

fn data_event_status(event: DataViewTypedEvent<usize>) -> String {
    match event {
        DataViewTypedEvent::HighlightChanged { row_id } => format!("highlight → {row_id:?}"),
        DataViewTypedEvent::Activated { row_id } => format!("activated #{row_id}"),
        DataViewTypedEvent::SelectionChanged { selected, .. } => format!("selected {selected:?}"),
    }
}

fn demo_columns() -> Vec<Column<DemoRow, usize>> {
    vec![
        Column::text(
            "task",
            "Task",
            Constraint::Percentage(45),
            |row: &DemoRow| row.name.clone(),
        )
        .sortable(|row| row.name.clone()),
        Column::text(
            "owner",
            "Owner",
            Constraint::Percentage(20),
            |row: &DemoRow| row.owner.to_string(),
        )
        .sortable(|row| row.owner.to_string()),
        Column::rich(
            "status",
            "Status",
            Constraint::Percentage(20),
            |row: &DemoRow, _: &CellContext<usize>| {
                let theme = tuicore::theme();
                let (label, color) = match row.status {
                    Status::Ready => ("READY", theme.success_fg()),
                    Status::Active => ("ACTIVE", theme.accent_fg()),
                    Status::Blocked => ("BLOCKED", theme.error_fg()),
                };
                Line::from(Span::styled(
                    format!(" {label} "),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ))
            },
        ),
        Column::rich(
            "progress",
            "Progress",
            Constraint::Percentage(15),
            |row: &DemoRow, _: &CellContext<usize>| {
                let theme = tuicore::theme();
                let bars = (row.progress / 20) as usize;
                Line::from(vec![
                    Span::styled("█".repeat(bars), Style::default().fg(theme.accent_fg())),
                    Span::styled(
                        "░".repeat(5_usize.saturating_sub(bars)),
                        Style::default().fg(theme.subtle_fg()),
                    ),
                ])
            },
        ),
    ]
}

fn demo_rows() -> Vec<DemoRow> {
    let owners = ["Ada", "Lin", "Ken", "Mia", "Noor"];
    let mut rows = Vec::with_capacity(100);
    for group in 0..10 {
        let parent_id = group * 10;
        rows.push(DemoRow {
            id: parent_id,
            parent: None,
            name: format!("Module {:02}", group + 1),
            owner: "Core",
            status: status_for(group),
            progress: progress_for(group),
        });
        for section in 1..4 {
            let id = parent_id + section;
            rows.push(DemoRow {
                id,
                parent: Some(parent_id),
                name: format!("Module {:02} / section {:02}", group + 1, section),
                owner: owners[id % owners.len()],
                status: status_for(id),
                progress: progress_for(id),
            });
        }
        for task in 4..10 {
            let id = parent_id + task;
            let section_id = parent_id + 1 + ((task - 4) / 2);
            rows.push(DemoRow {
                id,
                parent: Some(section_id),
                name: format!("Module {:02} / task {:02}", group + 1, task - 3),
                owner: owners[id % owners.len()],
                status: status_for(id),
                progress: progress_for(id),
            });
        }
    }
    rows
}

fn status_for(index: usize) -> Status {
    match index % 5 {
        0 => Status::Ready,
        1 | 2 => Status::Active,
        _ => Status::Blocked,
    }
}

fn progress_for(index: usize) -> u8 {
    ((index * 17) % 101) as u8
}

fn data_view_layout(area: Rect) -> [Rect; 2] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Fill(1)])
        .areas(area)
}

fn panel_preview_layout(area: Rect) -> [Rect; 3] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(4),
            Constraint::Fill(1),
        ])
        .areas(area)
}

fn panel_title_control_areas(area: Rect) -> [Rect; 4] {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .areas(area)
}

fn panel_title_column_layout(area: Rect) -> [Rect; 2] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Fill(1)])
        .areas(area)
}

fn panel_title_dropdown_area(area: Rect) -> Rect {
    panel_title_column_layout(area)[1]
}

fn panel_title_child_key(index: usize) -> ChildKey {
    ChildKey::new(format!("panel-title-{index}"))
}

const PANEL_TITLE_CONTROL_COUNT: usize = 4;

fn panel_demo_child_key() -> ChildKey {
    ChildKey::new("panel-demo")
}

fn panel_title_index(key: &ChildKey) -> Option<usize> {
    key.as_str()
        .strip_prefix("panel-title-")?
        .parse()
        .ok()
        .filter(|index| *index < 4)
}

fn panel_title_child_route(route: &EventRoute) -> Option<(usize, EventRoute)> {
    let first = route.path.first()?;
    let index = panel_title_index(first)?;
    Some((index, EventRoute::new(route.path.without_first())))
}

fn panel_demo_child_route(route: &EventRoute) -> Option<EventRoute> {
    route
        .path
        .without_first_if(&panel_demo_child_key())
        .map(EventRoute::new)
}

fn dropdown_preview_layout(area: Rect) -> [Rect; 2] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Fill(1)])
        .areas(area)
}

fn dropdown_columns(area: Rect) -> [Rect; 3] {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .areas(area)
}

fn dropdown_grid_areas(area: Rect) -> [Rect; 6] {
    let rows: [Rect; 2] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .areas(area);
    let bordered = dropdown_columns(rows[0]);
    let filled = dropdown_columns(rows[1]);
    [
        bordered[0],
        bordered[1],
        bordered[2],
        filled[0],
        filled[1],
        filled[2],
    ]
}

fn dropdown_column_layout(area: Rect) -> [Rect; 3] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(10),
            Constraint::Fill(1),
        ])
        .areas(area)
}

fn dropdown_area(area: Rect) -> Rect {
    dropdown_column_layout(area)[1]
}

fn tab_demo_child_key(index: usize) -> ChildKey {
    ChildKey::new(format!("tab-demo-{index}"))
}

fn tab_demo_index(key: &ChildKey) -> Option<usize> {
    key.as_str()
        .strip_prefix("tab-demo-")?
        .parse()
        .ok()
        .filter(|index| *index < 3)
}

fn tab_demo_child_route(route: &EventRoute) -> Option<(usize, EventRoute)> {
    let first = route.path.first()?;
    let index = tab_demo_index(first)?;
    Some((index, EventRoute::new(route.path.without_first())))
}

fn dropdown_child_key(index: usize) -> ChildKey {
    ChildKey::new(format!("dropdown-{index}"))
}

fn dropdown_index(key: &ChildKey) -> Option<usize> {
    key.as_str()
        .strip_prefix("dropdown-")?
        .parse()
        .ok()
        .filter(|index| *index < 6)
}

fn dropdown_child_route(route: &EventRoute) -> Option<(usize, EventRoute)> {
    let first = route.path.first()?;
    let index = dropdown_index(first)?;
    Some((index, EventRoute::new(route.path.without_first())))
}

fn input_layout(area: Rect) -> [Rect; 2] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(11), Constraint::Length(1)])
        .areas(area)
}

fn toggle_layout(area: Rect) -> [Rect; 2] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Length(1)])
        .areas(area)
}

fn button_layout(area: Rect) -> [Rect; 3] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(area)
}

fn textarea_layout(area: Rect) -> [Rect; 2] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(12), Constraint::Fill(1)])
        .areas(area)
}

fn tabs_areas(area: Rect) -> [Rect; 3] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .areas(area)
}

fn labeled_area(area: Rect) -> [Rect; 2] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Fill(1)])
        .areas(area)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_preview_layout_keeps_panel_focus_state() {
        let mut state = PreviewState::new();
        state
            .panel_demo
            .set_focused(true, AnimationSettings::default());
        let mut ctx = LayoutCtx::new();

        state.layout_panel_preview(Rect::new(0, 0, 80, 20), &mut ctx);

        assert!(state.panel_demo.is_focused());
    }

    #[test]
    fn parent_preview_uses_first_child_demo() {
        assert_eq!(ComponentKind::Layouts.preview(), PreviewKind::LayoutFlex);
        assert_eq!(ComponentKind::Inputs.preview(), PreviewKind::Button);
        assert_eq!(ComponentKind::DataView.preview(), PreviewKind::DataList);
    }
}
