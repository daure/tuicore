use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use tuicore::{
    ActivationMode, Animated, AnimationSettings, BorderKind, CellContext, ChildKey, Column,
    DataView, DataViewTypedEvent, Dropdown, DropdownCommitMode, DropdownSearchMode,
    DropdownVariant, EventCtx, EventOutcome, EventRoute, Flex, FlexItem, FocusCtx, FocusId,
    FocusRequest, FocusTarget, Gap, Grid, GridItem, GridTrack, HintSource, Key, KeyEvent,
    KeyModifiers, LayoutCtx, LayoutProposal, LayoutResult, LayoutSize, LayoutSizeHint, Overlay,
    OverlayAnchor, OverlaySize, Panel, PanelTitlePosition, PanelTitleStyle, SelectionGlyphs,
    SelectionMode, SelectionPropagation, SelectionTrigger, Separator, SeparatorColorRole, Spinner,
    Split, Stack, StackAlign, StackItem, Tabs, TabsVariant, TextInput, TextareaInput, TickResult,
    TreeAdapter, TreeGlyphs, TreePath, TuiEvent, TuiNode,
};

#[derive(Debug, PartialEq)]
enum Msg {}

fn main() -> tuicore::Result<()> {
    tuicore::init();
    tuicore::TreeApp::new(Gallery::new()).run()
}

struct Gallery {
    component_list: DataView<ComponentKind, ComponentKind>,
    selected: ComponentKind,
    focus: GalleryFocus,
    areas: GalleryAreas,
    list_panel: Panel,
    preview_panel: Panel,
    previews: PreviewState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GalleryFocus {
    List,
    Preview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct GalleryAreas {
    list_panel: Rect,
    list_body: Rect,
    preview_panel: Rect,
    preview_body: Rect,
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
            focus: GalleryFocus::List,
            areas: GalleryAreas::default(),
            list_panel: Panel::new().top_left("Components").focused(true),
            preview_panel: Panel::new().top_left(ComponentKind::Tabs.preview().title()),
            previews: PreviewState::new(),
        }
    }

    fn select(&mut self, selected: ComponentKind) {
        self.selected = selected;
        self.preview_panel.set_top_left(selected.preview().title());
    }

    fn sync_focus(&mut self, settings: AnimationSettings) {
        let list_focused = self.focus == GalleryFocus::List;
        let preview_focused = self.focus == GalleryFocus::Preview;
        self.list_panel.set_focused(list_focused, settings);
        self.preview_panel.set_focused(preview_focused, settings);
        self.component_list.set_focused(list_focused);
        self.previews
            .set_focused(self.selected.preview(), preview_focused, settings);
    }

    fn focus_next(&mut self, ctx: &mut EventCtx<Msg>) {
        if self.focus == GalleryFocus::Preview {
            let advanced = match self.selected.preview() {
                PreviewKind::Tabs => self.previews.focus_next_tab_demo(),
                PreviewKind::Panel => return self.focus_next_panel_title(ctx),
                _ => false,
            };
            if advanced {
                self.previews
                    .set_focused(self.selected.preview(), true, ctx.animation());
                return;
            }
        }
        self.focus = if self.focus == GalleryFocus::List {
            self.previews.reset_tab_demo_focus();
            GalleryFocus::Preview
        } else {
            GalleryFocus::List
        };
        self.sync_focus(ctx.animation());
        if self.focus == GalleryFocus::Preview {
            match self.selected.preview() {
                PreviewKind::Panel => {
                    self.previews.focus_panel_title_node(0, ctx);
                }
                PreviewKind::Dropdown => {
                    self.previews.focus_dropdown_node(0, ctx);
                }
                _ => {
                    ctx.focus(FocusRequest::Target(FocusId::new("preview")));
                }
            }
        } else {
            ctx.focus(FocusRequest::Target(FocusId::new("data-view")));
        }
    }

    fn focus_previous(&mut self, ctx: &mut EventCtx<Msg>) {
        if self.focus == GalleryFocus::Preview {
            let moved = match self.selected.preview() {
                PreviewKind::Tabs => self.previews.focus_previous_tab_demo(),
                PreviewKind::Panel => return self.focus_previous_panel_title(ctx),
                _ => false,
            };
            if moved {
                self.previews
                    .set_focused(self.selected.preview(), true, ctx.animation());
                return;
            }
        }
        self.focus = if self.focus == GalleryFocus::List {
            match self.selected.preview() {
                PreviewKind::Tabs => self.previews.focus_last_tab_demo(),
                PreviewKind::Panel => self.previews.focused_panel = PANEL_DEMO_FOCUS_INDEX,
                _ => {}
            }
            GalleryFocus::Preview
        } else {
            GalleryFocus::List
        };
        self.sync_focus(ctx.animation());
        if self.focus == GalleryFocus::Preview {
            match self.selected.preview() {
                PreviewKind::Panel => {
                    self.previews
                        .focus_panel_title_node(self.previews.focused_panel, ctx);
                }
                PreviewKind::Dropdown => {
                    self.previews
                        .focus_dropdown_node(self.previews.focused_dropdown, ctx);
                }
                _ => {
                    ctx.focus(FocusRequest::Target(FocusId::new("preview")));
                }
            }
        } else {
            ctx.focus(FocusRequest::Target(FocusId::new("data-view")));
        }
    }

    fn handle_list_key(&mut self, key: KeyEvent, ctx: &mut EventCtx<Msg>) -> EventOutcome {
        let outcome =
            self.component_list
                .on_key_with_settings(key, self.areas.list_body, ctx.animation());
        if let Some(selected) = self.selected_from_list_events() {
            self.select(selected);
        }
        if matches!(key.code, Key::Enter) {
            self.focus = GalleryFocus::Preview;
            self.sync_focus(ctx.animation());
            match self.selected.preview() {
                PreviewKind::Dropdown => {
                    self.previews.focus_dropdown_node(0, ctx);
                }
                PreviewKind::Panel => {
                    self.previews.focus_panel_title_node(0, ctx);
                }
                _ => {
                    ctx.focus(FocusRequest::Target(FocusId::new("preview")));
                }
            }
        }
        if outcome.handled || outcome.needs_redraw() || matches!(key.code, Key::Enter) {
            ctx.request_redraw();
            ctx.stop_propagation();
            EventOutcome::Handled
        } else {
            EventOutcome::Ignored
        }
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

    fn handle_preview_key(&mut self, key: KeyEvent, ctx: &mut EventCtx<Msg>) -> EventOutcome {
        if self
            .previews
            .on_key(self.selected.preview(), key, self.areas.preview_body, ctx)
        {
            ctx.request_redraw();
            ctx.stop_propagation();
            EventOutcome::Handled
        } else {
            EventOutcome::Ignored
        }
    }

    fn preview_handles_escape(&self) -> bool {
        if self.focus != GalleryFocus::Preview {
            return false;
        }
        match self.selected.preview() {
            PreviewKind::Panel => self.previews.panel_title_dropdown_is_open(),
            PreviewKind::Dropdown => self.previews.dropdown_is_open(),
            _ => false,
        }
    }

    fn focus_next_panel_title(&mut self, ctx: &mut EventCtx<Msg>) {
        if self.previews.focused_panel < 3 {
            self.previews
                .focus_panel_title_node(self.previews.focused_panel + 1, ctx);
        } else if self.previews.focused_panel == 3 {
            self.previews.focus_panel_demo(ctx);
        } else {
            self.previews.close_panel_title_dropdowns();
            ctx.focus(FocusRequest::TargetAt {
                path: TreePath::new(),
                id: FocusId::new("data-view"),
            });
        }
    }

    fn focus_previous_panel_title(&mut self, ctx: &mut EventCtx<Msg>) {
        if self.previews.focused_panel == PANEL_DEMO_FOCUS_INDEX {
            self.previews.focus_panel_title_node(3, ctx);
        } else if self.previews.focused_panel > 0 {
            self.previews
                .focus_panel_title_node(self.previews.focused_panel - 1, ctx);
        } else {
            self.previews.close_panel_title_dropdowns();
            ctx.focus(FocusRequest::TargetAt {
                path: TreePath::new(),
                id: FocusId::new("data-view"),
            });
        }
    }

    fn focus_next_dropdown(&mut self, ctx: &mut EventCtx<Msg>) {
        if self.focus == GalleryFocus::List {
            self.previews.focus_dropdown_node(0, ctx);
        } else if self.previews.focused_dropdown < 5 {
            self.previews
                .focus_dropdown_node(self.previews.focused_dropdown + 1, ctx);
        } else {
            self.previews.close_dropdowns();
            ctx.focus(FocusRequest::TargetAt {
                path: TreePath::new(),
                id: FocusId::new("data-view"),
            });
        }
    }

    fn focus_previous_dropdown(&mut self, ctx: &mut EventCtx<Msg>) {
        if self.focus == GalleryFocus::List {
            self.previews.focus_dropdown_node(5, ctx);
        } else if self.previews.focused_dropdown > 0 {
            self.previews
                .focus_dropdown_node(self.previews.focused_dropdown - 1, ctx);
        } else {
            self.previews.close_dropdowns();
            ctx.focus(FocusRequest::TargetAt {
                path: TreePath::new(),
                id: FocusId::new("data-view"),
            });
        }
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
        <DataView<ComponentKind, ComponentKind> as TuiNode<Msg>>::layout(
            &mut self.component_list,
            self.areas.list_body,
            ctx,
        );
        self.previews
            .layout(self.selected.preview(), self.areas.preview_body, ctx);

        let has_sub_focusables = matches!(
            self.selected.preview(),
            PreviewKind::Panel | PreviewKind::Dropdown
        );
        if !has_sub_focusables {
            ctx.register_focusable(FocusId::new("preview"), self.areas.preview_body, true);
        }
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
        if let TuiEvent::Key(key) = event {
            if key.code == Key::Esc && self.preview_handles_escape() {
                if !self.list_panel.is_focused() && !self.preview_panel.is_focused() {
                    return EventOutcome::Ignored;
                }
                return self.handle_preview_key(*key, ctx);
            }
        }

        if Self::quit_key(event) {
            ctx.request_quit();
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }

        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };

        if !self.list_panel.is_focused() && !self.preview_panel.is_focused() {
            return EventOutcome::Ignored;
        }

        let bindings = tuicore::keybindings();
        if bindings.focus().previous_matches(*key) {
            if self.selected.preview() == PreviewKind::Dropdown {
                self.focus_previous_dropdown(ctx);
                ctx.request_redraw();
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
            self.focus_previous(ctx);
            ctx.request_redraw();
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        if bindings.focus().next_matches(*key) {
            if self.selected.preview() == PreviewKind::Dropdown {
                self.focus_next_dropdown(ctx);
                ctx.request_redraw();
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
            self.focus_next(ctx);
            ctx.request_redraw();
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }

        match self.focus {
            GalleryFocus::List => self.handle_list_key(*key, ctx),
            GalleryFocus::Preview => self.handle_preview_key(*key, ctx),
        }
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

        let child = self
            .previews
            .dispatch_event(self.selected.preview(), route, event, ctx);
        child.bubble(ctx, |ctx| self.event(event, ctx))
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<Msg>) {
        if target.id.as_str() == "preview" {
            if focused {
                self.focus = GalleryFocus::Preview;
                self.list_panel.set_focused(false, ctx.animation());
                self.component_list.set_focused(false);
                self.preview_panel.set_focused(true, ctx.animation());
                self.previews
                    .set_focused(self.selected.preview(), true, ctx.animation());
            } else {
                self.preview_panel.set_focused(false, ctx.animation());
                self.previews
                    .set_focused(self.selected.preview(), false, ctx.animation());
            }
            ctx.request_redraw();
            return;
        }

        if target.path.is_empty() {
            self.component_list.dispatch_focus(target, focused, ctx);
            self.list_panel.set_focused(focused, ctx.animation());
            if focused {
                self.focus = GalleryFocus::List;
                self.preview_panel.set_focused(false, ctx.animation());
                self.previews
                    .set_focused(self.selected.preview(), false, ctx.animation());
            } else {
                self.preview_panel.set_focused(false, ctx.animation());
                self.previews
                    .set_focused(self.selected.preview(), false, ctx.animation());
            }
            ctx.request_redraw();
            return;
        }

        if self
            .previews
            .dispatch_focus(self.selected.preview(), target, focused, ctx)
        {
            if focused {
                self.focus = GalleryFocus::Preview;
                self.list_panel.set_focused(false, ctx.animation());
                self.component_list.set_focused(false);
                self.preview_panel.set_focused(true, ctx.animation());
                self.previews
                    .set_focused(self.selected.preview(), true, ctx.animation());
            } else {
                self.preview_panel.set_focused(false, ctx.animation());
            }
            ctx.request_redraw();
        }
    }
}

struct PreviewState {
    text_input: TextInput<Msg>,
    textarea_input: TextareaInput<Msg>,
    spinner: Spinner,
    focused_panel: usize,
    panel_demo: Panel,
    tabs_minimal: Tabs<Msg>,
    tabs_underline: Tabs<Msg>,
    tabs_boxed: Tabs<Msg>,
    focused_tab_demo: usize,
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
    focused_dropdown: usize,
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
            spinner: Spinner::new(),
            focused_panel: 0,
            panel_demo: Panel::new(),
            tabs_minimal: Tabs::default().variant(TabsVariant::Minimal),
            tabs_underline: Tabs::default().variant(TabsVariant::Underline),
            tabs_boxed: Tabs::default().variant(TabsVariant::Boxed),
            focused_tab_demo: 0,
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
            focused_dropdown: 0,
            layout_flex: layout_flex_demo(),
            layout_split: layout_split_demo(),
            layout_stack: layout_stack_demo(),
            layout_overlay: layout_overlay_demo(),
            layout_grid: layout_grid_demo(),
        }
    }

    fn set_focused(&mut self, preview: PreviewKind, focused: bool, settings: AnimationSettings) {
        self.text_input.set_focused(focused);
        self.textarea_input.set_focused(focused);
        self.tabs_minimal.set_focused(
            focused && preview == PreviewKind::Tabs && self.focused_tab_demo == 0,
            settings,
        );
        self.tabs_underline.set_focused(
            focused && preview == PreviewKind::Tabs && self.focused_tab_demo == 1,
            settings,
        );
        self.tabs_boxed.set_focused(
            focused && preview == PreviewKind::Tabs && self.focused_tab_demo == 2,
            settings,
        );
        self.active_data_view_mut(PreviewKind::DataList)
            .set_focused(false);
        self.active_data_view_mut(PreviewKind::DataTable)
            .set_focused(false);
        self.active_data_view_mut(PreviewKind::DataListTree)
            .set_focused(false);
        self.active_data_view_mut(PreviewKind::DataTableTree)
            .set_focused(false);
        self.active_data_view_mut(PreviewKind::DataSingleSelect)
            .set_focused(false);
        self.active_data_view_mut(PreviewKind::DataMultiSelect)
            .set_focused(false);
        self.active_data_view_mut(PreviewKind::DataChecklistTree)
            .set_focused(false);
        self.active_data_view_mut(PreviewKind::DataActivateOnNavigate)
            .set_focused(false);
        if preview == PreviewKind::Panel && !focused {
            self.close_panel_title_dropdowns();
            self.focused_panel = 0;
        }
        if preview == PreviewKind::Dropdown && !focused {
            self.close_dropdowns();
        }
        if preview.is_data_view() {
            self.active_data_view_mut(preview).set_focused(focused);
        }
    }

    fn reset_tab_demo_focus(&mut self) {
        self.focused_tab_demo = 0;
    }

    fn focus_next_tab_demo(&mut self) -> bool {
        if self.focused_tab_demo >= 2 {
            return false;
        }
        self.focused_tab_demo += 1;
        true
    }

    fn focus_previous_tab_demo(&mut self) -> bool {
        if self.focused_tab_demo == 0 {
            return false;
        }
        self.focused_tab_demo -= 1;
        true
    }

    fn focus_last_tab_demo(&mut self) {
        self.focused_tab_demo = 2;
    }

    fn layout(&mut self, preview: PreviewKind, area: Rect, ctx: &mut LayoutCtx) {
        match preview {
            PreviewKind::Tabs => self.layout_tabs(area),
            PreviewKind::Panel => self.layout_panel_preview(area, ctx),
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
                    &mut LayoutCtx::new(),
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
            PreviewKind::Spinner => self.render_spinner(frame, area),
            PreviewKind::TextInput => self.render_text_input(frame, area),
            PreviewKind::TextareaInput => self.render_textarea_input(frame, area),
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

    fn on_key(
        &mut self,
        preview: PreviewKind,
        key: KeyEvent,
        area: Rect,
        ctx: &mut EventCtx<Msg>,
    ) -> bool {
        match preview {
            PreviewKind::TextInput => {
                let outcome = self.text_input.on_key(key);
                if outcome.clear {
                    ctx.request_clear();
                }
                if outcome.needs_redraw() {
                    ctx.request_redraw();
                }
                outcome.handled
            }
            PreviewKind::TextareaInput => {
                let outcome = self.textarea_input.on_key(key);
                if outcome.clear {
                    ctx.request_clear();
                }
                if outcome.needs_redraw() {
                    ctx.request_redraw();
                }
                outcome.handled
            }
            PreviewKind::Tabs => self.tabs_on_key(key, ctx),
            PreviewKind::DataTable | PreviewKind::DataTableTree
                if matches!(key.code, Key::Char('s')) && key.modifiers == KeyModifiers::NONE =>
            {
                self.active_data_view_mut(preview).toggle_sort("task");
                self.record_data_events(preview);
                true
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
                let outcome = self.active_data_view_mut(preview).on_key_with_settings(
                    key,
                    body,
                    ctx.animation(),
                );
                self.record_data_events(preview);
                if outcome.needs_redraw() {
                    ctx.request_redraw();
                }
                outcome.handled
            }
            PreviewKind::Panel => self.panel_on_key(key, area, ctx),
            PreviewKind::Dropdown => self.dropdown_on_key(key, area, ctx),
            PreviewKind::Spinner
            | PreviewKind::LayoutFlex
            | PreviewKind::LayoutSplit
            | PreviewKind::LayoutStack
            | PreviewKind::LayoutOverlay
            | PreviewKind::LayoutGrid => false,
        }
    }

    fn dispatch_event(
        &mut self,
        preview: PreviewKind,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<Msg>,
    ) -> EventOutcome {
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
    ) -> bool {
        if preview == PreviewKind::Panel {
            if let Some(target) = panel_demo_child_target(target) {
                if focused {
                    self.focused_panel = PANEL_DEMO_FOCUS_INDEX;
                    self.close_panel_title_dropdowns();
                }
                self.panel_demo.dispatch_focus(&target, focused, ctx);
                return true;
            }
            let Some((index, target)) = panel_title_child_target(target) else {
                return false;
            };
            if focused {
                self.focused_panel = index;
                self.close_inactive_panel_title_dropdowns();
            }
            self.panel_title_dropdown_mut(index)
                .dispatch_focus(&target, focused, ctx);
            return true;
        }
        if preview != PreviewKind::Dropdown {
            return false;
        }

        let Some((index, target)) = dropdown_child_target(target) else {
            return false;
        };
        if focused {
            self.focused_dropdown = index;
            self.close_inactive_dropdowns();
        }
        self.dropdown_mut(index)
            .dispatch_focus(&target, focused, ctx);
        true
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(&mut self.spinner, dt, settings)
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

    fn panel_title_dropdown_is_open(&self) -> bool {
        self.panel_top_left.is_open()
            || self.panel_top_right.is_open()
            || self.panel_bottom_left.is_open()
            || self.panel_bottom_right.is_open()
    }

    fn close_panel_title_dropdowns(&mut self) {
        self.panel_top_left.cancel();
        self.panel_top_right.cancel();
        self.panel_bottom_left.cancel();
        self.panel_bottom_right.cancel();
    }

    fn close_inactive_panel_title_dropdowns(&mut self) {
        if self.focused_panel != 0 {
            self.panel_top_left.cancel();
        }
        if self.focused_panel != 1 {
            self.panel_top_right.cancel();
        }
        if self.focused_panel != 2 {
            self.panel_bottom_left.cancel();
        }
        if self.focused_panel != 3 {
            self.panel_bottom_right.cancel();
        }
    }

    fn active_panel_title_dropdown_mut(&mut self) -> &mut Dropdown<PanelTitleChoice, &'static str> {
        match self.focused_panel {
            1 => &mut self.panel_top_right,
            2 => &mut self.panel_bottom_left,
            3 => &mut self.panel_bottom_right,
            _ => &mut self.panel_top_left,
        }
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

    fn dropdown_is_open(&self) -> bool {
        self.dropdown_fuzzy_single.is_open()
            || self.dropdown_multi_contains.is_open()
            || self.dropdown_no_search_immediate.is_open()
            || self.dropdown_filled_fuzzy_single.is_open()
            || self.dropdown_filled_multi_contains.is_open()
            || self.dropdown_filled_no_search_immediate.is_open()
    }

    fn close_dropdowns(&mut self) {
        self.dropdown_fuzzy_single.cancel();
        self.dropdown_multi_contains.cancel();
        self.dropdown_no_search_immediate.cancel();
        self.dropdown_filled_fuzzy_single.cancel();
        self.dropdown_filled_multi_contains.cancel();
        self.dropdown_filled_no_search_immediate.cancel();
    }

    fn close_inactive_dropdowns(&mut self) {
        if self.focused_dropdown != 0 {
            self.dropdown_fuzzy_single.cancel();
        }
        if self.focused_dropdown != 1 {
            self.dropdown_multi_contains.cancel();
        }
        if self.focused_dropdown != 2 {
            self.dropdown_no_search_immediate.cancel();
        }
        if self.focused_dropdown != 3 {
            self.dropdown_filled_fuzzy_single.cancel();
        }
        if self.focused_dropdown != 4 {
            self.dropdown_filled_multi_contains.cancel();
        }
        if self.focused_dropdown != 5 {
            self.dropdown_filled_no_search_immediate.cancel();
        }
    }

    fn active_dropdown_mut(&mut self) -> &mut Dropdown<DropdownDemoItem, &'static str> {
        match self.focused_dropdown {
            1 => &mut self.dropdown_multi_contains,
            2 => &mut self.dropdown_no_search_immediate,
            3 => &mut self.dropdown_filled_fuzzy_single,
            4 => &mut self.dropdown_filled_multi_contains,
            5 => &mut self.dropdown_filled_no_search_immediate,
            _ => &mut self.dropdown_fuzzy_single,
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

        self.render_inactive_dropdowns(frame, areas);
        self.dropdown(self.focused_dropdown)
            .render(frame, dropdown_area(areas[self.focused_dropdown]));
        self.dropdown(self.focused_dropdown)
            .render_popup_overlay(frame, body);
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

    fn render_inactive_dropdowns(&self, frame: &mut Frame, areas: [Rect; 6]) {
        for (index, area) in areas.iter().copied().enumerate() {
            if self.focused_dropdown != index {
                self.dropdown(index).render(frame, dropdown_area(area));
            }
        }
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

    fn dropdown_on_key(&mut self, key: KeyEvent, area: Rect, ctx: &mut EventCtx<Msg>) -> bool {
        if key.modifiers == KeyModifiers::NONE {
            match key.code {
                Key::Char('1') => return self.focus_dropdown_node(0, ctx),
                Key::Char('2') => return self.focus_dropdown_node(1, ctx),
                Key::Char('3') => return self.focus_dropdown_node(2, ctx),
                Key::Char('4') => return self.focus_dropdown_node(3, ctx),
                Key::Char('5') => return self.focus_dropdown_node(4, ctx),
                Key::Char('6') => return self.focus_dropdown_node(5, ctx),
                _ => {}
            }
        }

        let [_, body] = dropdown_preview_layout(area);
        let outcome = self.active_dropdown_mut().on_key(key, body);
        if outcome.opened || outcome.closed {
            ctx.request_layout();
        }
        if outcome.opened {
            self.close_inactive_dropdowns();
        }
        outcome.handled || outcome.changed
    }

    fn focus_dropdown_node(&mut self, index: usize, ctx: &mut EventCtx<Msg>) -> bool {
        self.focused_dropdown = index;
        self.close_dropdowns();
        ctx.focus(FocusRequest::TargetAt {
            path: TreePath::from_keys([dropdown_child_key(index)]),
            id: FocusId::new("field"),
        });
        true
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

    fn panel_from_dropdowns(&self) -> Panel {
        let mut panel = Panel::new().border(BorderKind::Plain).content([
            "Use dropdowns below to independently configure each border title.",
            "Style 1 draws a normal title over the border.",
            "Style 2 draws the -| Title |- inset style.",
        ]);
        apply_title_choice(
            &mut panel,
            PanelTitlePosition::TopLeft,
            self.panel_top_left.selected_id(),
        );
        apply_title_choice(
            &mut panel,
            PanelTitlePosition::TopRight,
            self.panel_top_right.selected_id(),
        );
        apply_title_choice(
            &mut panel,
            PanelTitlePosition::BottomLeft,
            self.panel_bottom_left.selected_id(),
        );
        apply_title_choice(
            &mut panel,
            PanelTitlePosition::BottomRight,
            self.panel_bottom_right.selected_id(),
        );
        panel
    }

    fn layout_panel_preview(&mut self, area: Rect, ctx: &mut LayoutCtx) {
        let [_, controls, panel_area] = panel_preview_layout(area);
        let areas = panel_title_control_areas(controls).map(panel_title_dropdown_area);

        self.panel_demo = self
            .panel_from_dropdowns()
            .focused(self.focused_panel == PANEL_DEMO_FOCUS_INDEX);
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

    fn panel_on_key(&mut self, key: KeyEvent, area: Rect, ctx: &mut EventCtx<Msg>) -> bool {
        if self.focused_panel == PANEL_DEMO_FOCUS_INDEX {
            return self.panel_demo.event(&TuiEvent::Key(key), ctx).handled();
        }

        let outcome = self.active_panel_title_dropdown_mut().on_key(key, area);
        if outcome.opened || outcome.closed {
            ctx.request_layout();
        }
        if outcome.opened {
            self.close_inactive_panel_title_dropdowns();
        }
        outcome.handled || outcome.changed
    }

    fn focus_panel_title_node(&mut self, index: usize, ctx: &mut EventCtx<Msg>) -> bool {
        self.focused_panel = index;
        self.close_panel_title_dropdowns();
        ctx.focus(FocusRequest::TargetAt {
            path: TreePath::from_keys([panel_title_child_key(index)]),
            id: FocusId::new("field"),
        });
        true
    }

    fn focus_panel_demo(&mut self, ctx: &mut EventCtx<Msg>) -> bool {
        self.focused_panel = PANEL_DEMO_FOCUS_INDEX;
        self.close_panel_title_dropdowns();
        ctx.focus(FocusRequest::TargetAt {
            path: TreePath::from_keys([panel_demo_child_key()]),
            id: FocusId::new("panel"),
        });
        true
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

        if self.focused_panel < PANEL_TITLE_CONTROL_COUNT {
            self.panel_title_dropdown(self.focused_panel)
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

    fn layout_tabs(&mut self, area: Rect) {
        let [minimal, underline, boxed] = tabs_areas(area);
        let [_, minimal_tabs] = labeled_area(minimal);
        let [_, underline_tabs] = labeled_area(underline);
        let [_, boxed_tabs] = labeled_area(boxed);
        self.tabs_minimal
            .layout(minimal_tabs, &mut LayoutCtx::new());
        self.tabs_underline
            .layout(underline_tabs, &mut LayoutCtx::new());
        self.tabs_boxed.layout(boxed_tabs, &mut LayoutCtx::new());
    }

    fn render_tabs(&self, frame: &mut Frame, area: Rect) {
        let [minimal, underline, boxed] = tabs_areas(area);
        let [minimal_label, minimal_tabs] = labeled_area(minimal);
        let [underline_label, underline_tabs] = labeled_area(underline);
        let [boxed_label, boxed_tabs] = labeled_area(boxed);

        frame.render_widget(Paragraph::new("Style 1: minimal"), minimal_label);
        self.tabs_minimal.render(frame, minimal_tabs);
        frame.render_widget(Paragraph::new("Style 2: underline"), underline_label);
        self.tabs_underline.render(frame, underline_tabs);
        frame.render_widget(Paragraph::new("Style 3: boxed"), boxed_label);
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

    fn tabs_on_key(&mut self, key: KeyEvent, ctx: &mut EventCtx<Msg>) -> bool {
        let event = TuiEvent::Key(key);
        match self.focused_tab_demo {
            0 => self.tabs_minimal.event(&event, ctx).handled(),
            1 => self.tabs_underline.event(&event, ctx).handled(),
            _ => self.tabs_boxed.event(&event, ctx).handled(),
        }
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

fn layout_demo_areas(area: Rect) -> [Rect; 2] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Fill(1)])
        .areas(area)
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
enum ComponentKind {
    Tabs,
    Panel,
    Spinner,
    Layouts,
    LayoutFlex,
    LayoutSplit,
    LayoutStack,
    LayoutOverlay,
    LayoutGrid,
    Inputs,
    TextInput,
    TextareaInput,
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
    const ALL: [Self; 22] = [
        Self::Tabs,
        Self::Panel,
        Self::Spinner,
        Self::Layouts,
        Self::LayoutFlex,
        Self::LayoutSplit,
        Self::LayoutStack,
        Self::LayoutOverlay,
        Self::LayoutGrid,
        Self::Inputs,
        Self::TextInput,
        Self::TextareaInput,
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
            Self::Spinner => "Spinner",
            Self::Layouts => "Layouts",
            Self::LayoutFlex => "Flex",
            Self::LayoutSplit => "Split",
            Self::LayoutStack => "Stack",
            Self::LayoutOverlay => "Overlay",
            Self::LayoutGrid => "Grid",
            Self::Inputs => "Inputs",
            Self::TextInput => "Text",
            Self::TextareaInput => "Textarea",
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
            Self::TextInput | Self::TextareaInput | Self::Dropdown => Some(Self::Inputs),
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
            Self::Spinner => PreviewKind::Spinner,
            Self::Layouts | Self::LayoutFlex => PreviewKind::LayoutFlex,
            Self::LayoutSplit => PreviewKind::LayoutSplit,
            Self::LayoutStack => PreviewKind::LayoutStack,
            Self::LayoutOverlay => PreviewKind::LayoutOverlay,
            Self::LayoutGrid => PreviewKind::LayoutGrid,
            Self::Inputs | Self::TextInput => PreviewKind::TextInput,
            Self::TextareaInput => PreviewKind::TextareaInput,
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
    Spinner,
    LayoutFlex,
    LayoutSplit,
    LayoutStack,
    LayoutOverlay,
    LayoutGrid,
    TextInput,
    TextareaInput,
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
            Self::Spinner => "Spinner",
            Self::LayoutFlex => "Flex Layout",
            Self::LayoutSplit => "Split Layout",
            Self::LayoutStack => "Stack Layout",
            Self::LayoutOverlay => "Overlay Layout",
            Self::LayoutGrid => "Grid Layout",
            Self::TextInput => "Text",
            Self::TextareaInput => "Textarea",
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
}

fn dropdown_multi_contains() -> Dropdown<DropdownDemoItem, &'static str> {
    Dropdown::multi(dropdown_items(), |row| row.id, |row| row.label.to_string())
        .placeholder("Pick workstreams...")
        .search_mode(DropdownSearchMode::Contains)
        .selected(["alpha", "delta"])
}

fn dropdown_no_search_immediate() -> Dropdown<DropdownDemoItem, &'static str> {
    Dropdown::single(dropdown_items(), |row| row.id, |row| row.label.to_string())
        .placeholder("Immediate lane...")
        .search_mode(DropdownSearchMode::None)
        .commit_mode(DropdownCommitMode::Immediate)
        .centered(true)
        .selected_one("beta")
}

fn dropdown_filled_fuzzy_single() -> Dropdown<DropdownDemoItem, &'static str> {
    dropdown_fuzzy_single().variant(DropdownVariant::Filled)
}

fn dropdown_filled_multi_contains() -> Dropdown<DropdownDemoItem, &'static str> {
    dropdown_multi_contains().variant(DropdownVariant::Filled)
}

fn dropdown_filled_no_search_immediate() -> Dropdown<DropdownDemoItem, &'static str> {
    dropdown_no_search_immediate().variant(DropdownVariant::Filled)
}

#[derive(Clone)]
struct PanelTitleChoice {
    id: &'static str,
    label: &'static str,
    style: Option<PanelTitleStyle>,
}

fn panel_title_choices() -> Vec<PanelTitleChoice> {
    vec![
        PanelTitleChoice {
            id: "none",
            label: "no text",
            style: None,
        },
        PanelTitleChoice {
            id: "style-1",
            label: "top left style 1",
            style: Some(PanelTitleStyle::Standard),
        },
        PanelTitleChoice {
            id: "style-2",
            label: "top left style 2",
            style: Some(PanelTitleStyle::Inset),
        },
    ]
}

fn panel_title_dropdown(position: PanelTitlePosition) -> Dropdown<PanelTitleChoice, &'static str> {
    Dropdown::single(
        panel_title_choices(),
        |row| row.id,
        |row| row.label.to_string(),
    )
    .placeholder(panel_title_placeholder(position))
    .selected_one("style-1")
}

fn panel_title_placeholder(position: PanelTitlePosition) -> &'static str {
    match position {
        PanelTitlePosition::TopLeft => "Top left title...",
        PanelTitlePosition::TopRight => "Top right title...",
        PanelTitlePosition::BottomLeft => "Bottom left title...",
        PanelTitlePosition::BottomRight => "Bottom right title...",
    }
}

fn apply_title_choice(
    panel: &mut Panel,
    position: PanelTitlePosition,
    selected: Option<&'static str>,
) {
    let Some(choice) = panel_title_choices()
        .into_iter()
        .find(|choice| Some(choice.id) == selected)
    else {
        return;
    };
    let Some(style) = choice.style else {
        panel.clear_title(position);
        return;
    };
    panel.set_title(position, choice.label, style);
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
const PANEL_DEMO_FOCUS_INDEX: usize = PANEL_TITLE_CONTROL_COUNT;

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

fn panel_title_child_target(target: &FocusTarget) -> Option<(usize, FocusTarget)> {
    let first = target.path.first()?;
    let index = panel_title_index(first)?;
    let child_target = FocusTarget {
        id: target.id.clone(),
        path: target.path.without_first(),
        area: target.area,
        enabled: target.enabled,
    };
    Some((index, child_target))
}

fn panel_demo_child_target(target: &FocusTarget) -> Option<FocusTarget> {
    Some(FocusTarget {
        id: target.id.clone(),
        path: target.path.without_first_if(&panel_demo_child_key())?,
        area: target.area,
        enabled: target.enabled,
    })
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

fn dropdown_child_target(target: &FocusTarget) -> Option<(usize, FocusTarget)> {
    let first = target.path.first()?;
    let index = dropdown_index(first)?;
    let child_target = FocusTarget {
        id: target.id.clone(),
        path: target.path.without_first(),
        area: target.area,
        enabled: target.enabled,
    };
    Some((index, child_target))
}

fn input_layout(area: Rect) -> [Rect; 2] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(11), Constraint::Length(1)])
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
