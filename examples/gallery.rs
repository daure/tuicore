use std::time::Duration;

mod gallery_demo;

use gallery_demo::data::{DataViewMode, DemoRow, data_event_status, data_view_layout};
use gallery_demo::dialogs::{
    DialogExample, dialog_body_area, dialog_button, dialog_button_areas, dialog_demo_child_key,
    dialog_demo_child_route, dialog_demo_index, gallery_dialog,
};
use gallery_demo::dropdowns::{
    DropdownDemoItem, ThemeChoice, dropdown_area, dropdown_child_key, dropdown_child_route,
    dropdown_column_layout, dropdown_filled_fuzzy_single, dropdown_filled_multi_contains,
    dropdown_filled_no_search_immediate, dropdown_fuzzy_single, dropdown_grid_areas,
    dropdown_index, dropdown_multi_contains, dropdown_no_search_immediate, dropdown_preview_layout,
    theme_dropdown,
};
use gallery_demo::inputs::{
    button_layout, password_input_showcase_layout, text_input_showcase_layout,
    textarea_showcase_layout, toggle_layout, typography_showcase_layout,
};
use gallery_demo::layouts::{
    DemoBox, layout_demo_body, layout_flex_demo, layout_grid_demo, layout_overlay_demo,
    layout_split_demo, layout_stack_demo, render_layout_intro,
};
use gallery_demo::notifications::{
    notification_button_areas, notification_button_child_key, notification_button_child_route,
    notification_button_index, notification_buttons, notification_for_index,
    notification_trigger_layout,
};
use gallery_demo::panels::{
    PANEL_TITLE_CONTROL_COUNT, PanelTitleChoice, apply_panel_choice, panel_demo,
    panel_demo_child_key, panel_demo_child_route, panel_join_demo, panel_join_demo_child_key,
    panel_join_demo_child_route, panel_preview_layout, panel_separator_preview_layout,
    panel_tabs_join_demo, panel_tabs_join_demo_child_key, panel_tabs_join_demo_child_route,
    panel_title_child_key, panel_title_child_route, panel_title_column_layout,
    panel_title_control_areas, panel_title_dropdown, panel_title_dropdown_area, panel_title_index,
};
use gallery_demo::tabs::{
    labeled_area, modal_tabs_button_areas, modal_tabs_dialog, modal_tabs_open_child_key,
    modal_tabs_open_child_route, modal_tabs_open_index, modal_tabs_preview_layout,
    tab_demo_child_key, tab_demo_child_route, tab_demo_index, tabs_areas, tabs_demo,
};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use tuicore::{
    ActivationMode, Animated, AnimationSettings, Button, ChildKey, DataView, DataViewTypedEvent,
    DialogBackdrop, DialogCloseReason, DialogLayer, Dropdown, EventCtx, EventOutcome, EventRoute,
    Flex, FocusCtx, FocusId, FocusTarget, Grid, Header, Key, KeyEvent, KeyModifiers, LayoutCtx,
    LayoutResult, ModalCloseReason, Overlay, Panel, PanelHost, PanelTitlePosition,
    Paragraph as TuiParagraph, ParagraphOverflow, PasswordInput, SelectionMode, SelectionTrigger,
    Spinner, Split, Stack, Tabs, TabsVariant, TextInput, TextareaInput, Theme, ThemeName,
    TickResult, ToastRack, Toggle, TreeAdapter, TuiEvent, TuiNode,
};

#[derive(Debug, PartialEq)]
enum Msg {
    DialogOpened(DialogExample),
    DialogClosed(DialogCloseReason),
    ModalTabsOpened(TabsVariant),
    ModalTabsClosed(ModalCloseReason),
    NotificationTriggered(usize),
}

fn main() -> tuicore::Result<()> {
    tuicore::init();
    let dialog_layer = DialogLayer::new(Gallery::new(), gallery_dialog()).active(false);
    let root = DialogLayer::new(dialog_layer, modal_tabs_dialog()).active(false);
    tuicore::TreeApp::new(root)
        .on_message(|root, msg, ctx| match msg {
            Msg::DialogOpened(example) => {
                let dialog_layer = root.base_mut();
                dialog_layer.layer_mut().child_mut().set_example(example);
                dialog_layer
                    .layer_mut()
                    .dialog_mut()
                    .set_top_left(example.title());
                dialog_layer
                    .layer_mut()
                    .dialog_mut()
                    .set_bottom_left("Esc blurs");
                dialog_layer
                    .layer_mut()
                    .dialog_mut()
                    .set_bottom_right(format!("{}% viewport", example.percent()));
                if example == DialogExample::Full {
                    dialog_layer.layer_mut().dialog_mut().set_content([
                        "100% dialog: full-screen modal content.",
                        "This uses the Dialog chrome only, with text content inside.",
                        "Press x or Esc to close and restore focus.",
                    ]);
                } else {
                    dialog_layer.layer_mut().dialog_mut().clear_content();
                }
                dialog_layer.set_layer_percent(example.percent());
                dialog_layer.set_backdrop(DialogBackdrop::dim().amount(0.55));
                dialog_layer.set_active_with_dialog_focus(true, ctx);
            }
            Msg::DialogClosed(_reason) => {
                root.base_mut().set_active_with_context(false, ctx);
            }
            Msg::ModalTabsOpened(variant) => {
                root.layer_mut().set_variant(variant);
                root.layer_mut().prepare_modal_open(ctx.animation());
                root.set_layer_percent(72);
                root.set_backdrop(DialogBackdrop::dim().amount(0.55));
                root.set_active_with_context(true, ctx);
            }
            Msg::ModalTabsClosed(_reason) => {
                root.set_active_with_context(false, ctx);
            }
            Msg::NotificationTriggered(index) => {
                root.base_mut()
                    .base_mut()
                    .previews
                    .notification_triggers
                    .push(notification_for_index(index).ttl(Duration::from_secs(4)));
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
    theme_dropdown: Dropdown<ThemeChoice, ThemeName>,
    previews: PreviewState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct GalleryAreas {
    list_panel: Rect,
    list_body: Rect,
    preview_panel: Rect,
    preview_body: Rect,
    footer: Rect,
    theme_dropdown: Rect,
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
            ComponentKind::Panel,
            ComponentKind::Notifications,
            ComponentKind::Typography,
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
            theme_dropdown: theme_dropdown(),
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
        let [main, footer] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Fill(1), Constraint::Length(1)])
            .areas(area);
        let [list_panel, preview_panel] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .areas(main);
        let [theme_dropdown, _] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(36), Constraint::Fill(1)])
            .areas(footer);

        self.areas = GalleryAreas {
            list_panel,
            list_body: Panel::inner_area(list_panel),
            preview_panel,
            preview_body: Panel::inner_area(preview_panel),
            footer,
            theme_dropdown,
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
                    .layout(self.selected.preview(), self.areas.preview_body, area, ctx);
                ctx.register_focusable(FocusId::new("preview"), self.areas.preview_body, true);
            },
        );
        ctx.push_slot(gallery_theme_child_key(), theme_dropdown, |ctx| {
            self.theme_dropdown
                .layout_overlay::<Msg>(theme_dropdown, area, ctx);
        });
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, _area: Rect) {
        self.list_panel.render(frame, self.areas.list_panel);
        self.component_list.render(frame, self.areas.list_body);

        self.preview_panel.render(frame, self.areas.preview_panel);
        self.previews
            .render(self.selected.preview(), frame, self.areas.preview_body);

        self.theme_dropdown.render(frame, self.areas.theme_dropdown);
        self.previews
            .render_overlay(self.selected.preview(), frame, _area);
        self.theme_dropdown.render_popup_overlay(frame, _area);
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
            .merge(Animated::tick(&mut self.theme_dropdown, dt, settings))
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

        if let Some(route) = route
            .path
            .without_first_if(&gallery_theme_child_key())
            .map(EventRoute::new)
        {
            let before = self.theme_dropdown.selected_id();
            let child = self.theme_dropdown.dispatch_event(&route, event, ctx);
            if self.theme_dropdown.selected_id() != before {
                if let Some(name) = self.theme_dropdown.selected_id() {
                    tuicore::set_theme(Theme::named(name));
                    ctx.request_redraw();
                }
            }
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

        if let Some(child_target) = target.for_child(&gallery_theme_child_key()) {
            self.theme_dropdown
                .dispatch_focus(&child_target, focused, ctx);
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
    text_input_panel: PanelHost<TextInput<Msg>>,
    password_input: PasswordInput<Msg>,
    header_plain: Header,
    header_icon: Header,
    paragraph: TuiParagraph,
    textarea_input: TextareaInput<Msg>,
    textarea_panel: PanelHost<TextareaInput<Msg>>,
    button: Button<Msg>,
    button_presses: u32,
    toggle: Toggle<Msg>,
    checkbox_toggle: Toggle<Msg>,
    dialog_100: Button<Msg>,
    dialog_80: Button<Msg>,
    dialog_60: Button<Msg>,
    dialog_40: Button<Msg>,
    dialog_20: Button<Msg>,
    dialog_top: Button<Msg>,
    spinner: Spinner,
    notification_triggers: ToastRack,
    notification_buttons: [Button<Msg>; 4],
    panel_demo: Panel,
    panel_join_demo: PanelHost<Flex<Msg>>,
    panel_tabs_join_demo: PanelHost<Tabs<Msg>>,
    tabs_minimal: Tabs<Msg>,
    tabs_underline: Tabs<Msg>,
    tabs_boxed: Tabs<Msg>,
    tabs_modal_minimal_button: Button<Msg>,
    tabs_modal_underline_button: Button<Msg>,
    tabs_modal_boxed_button: Button<Msg>,
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
                .hotkey("i")
                .max_len(80),
            text_input_panel: Panel::new()
                .top_left("Description")
                .hotkey("p")
                .host(TextInput::new().placeholder("Nested input")),
            password_input: PasswordInput::new()
                .placeholder("Enter password...")
                .hotkey("pw")
                .value("hunter2")
                .max_len(80),
            header_plain: Header::new("Release Notes"),
            header_icon: Header::new("Settings").icon(""),
            paragraph: TuiParagraph::new(
                "Paragraphs render wrapped body copy for explanatory text, help panels, and quiet content blocks. This one is intentionally longer than its preview box so the ellipsis overflow behavior is visible without needing a separate gallery entry.",
            )
            .overflow(ParagraphOverflow::Ellipsis)
            .max_lines(1),
            textarea_input: TextareaInput::new()
                .placeholder("Write multiple lines...")
                .value("First line\nSecond line")
                .hotkey("t")
                .max_lines(8),
            textarea_panel: Panel::new()
                .top_left("Description")
                .hotkey("p")
                .host(
                    TextareaInput::new()
                        .placeholder("Nested textarea")
                        .value("Draft note")
                        .max_lines(4),
                ),
            button: Button::new("button").hotkey("b"),
            button_presses: 0,
            toggle: Toggle::new("Telemetry").hotkey("x"),
            checkbox_toggle: Toggle::new("Item").checkbox().checked(true).hotkey("i"),
            dialog_100: dialog_button(DialogExample::Full),
            dialog_80: dialog_button(DialogExample::Large),
            dialog_60: dialog_button(DialogExample::Medium),
            dialog_40: dialog_button(DialogExample::Small),
            dialog_20: dialog_button(DialogExample::Tiny),
            dialog_top: dialog_button(DialogExample::Top),
            spinner: Spinner::new(),
            notification_triggers: ToastRack::new(),
            notification_buttons: notification_buttons(),
            panel_demo: panel_demo(),
            panel_join_demo: panel_join_demo(),
            panel_tabs_join_demo: panel_tabs_join_demo(),
            tabs_minimal: tabs_demo(TabsVariant::Minimal).hotkey("m"),
            tabs_underline: tabs_demo(TabsVariant::Underline).hotkey("ma"),
            tabs_boxed: tabs_demo(TabsVariant::Boxed).hotkey("mam"),
            tabs_modal_minimal_button: Button::new("Open Style 1 tabs-as-dialog")
                .hotkey("td")
                .on_press(|| Msg::ModalTabsOpened(TabsVariant::Minimal)),
            tabs_modal_underline_button: Button::new("Open Style 2 tabs-as-dialog")
                .hotkey("tu")
                .on_press(|| Msg::ModalTabsOpened(TabsVariant::Underline)),
            tabs_modal_boxed_button: Button::new("Open Style 3 tabs-as-dialog")
                .hotkey("tb")
                .on_press(|| Msg::ModalTabsOpened(TabsVariant::Boxed)),
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

    fn layout(
        &mut self,
        preview: PreviewKind,
        area: Rect,
        overlay_bounds: Rect,
        ctx: &mut LayoutCtx,
    ) {
        match preview {
            PreviewKind::Tabs => self.layout_tabs(area, ctx),
            PreviewKind::Panel => self.layout_panel_preview(area, ctx),
            PreviewKind::PanelJoinedSeparators => self.layout_panel_join_preview(area, ctx),
            PreviewKind::PanelTabSeparators => self.layout_panel_tabs_join_preview(area, ctx),
            PreviewKind::Dialog => self.layout_dialog(area, ctx),
            PreviewKind::NotificationTriggers => self.layout_notification_triggers(area, ctx),
            PreviewKind::Button => self.layout_button(area, ctx),
            PreviewKind::Toggle => self.layout_toggle(area, ctx),
            PreviewKind::TextInput => {
                let [_, input, panel] = text_input_showcase_layout(area);
                ctx.push_slot(text_input_child_key(), input, |ctx| {
                    self.text_input.layout(input, ctx);
                });
                ctx.push_slot(text_input_panel_child_key(), panel, |ctx| {
                    self.text_input_panel.layout(panel, ctx);
                });
            }
            PreviewKind::PasswordInput => {
                let [_, input, _] = password_input_showcase_layout(area);
                ctx.push_slot(password_input_child_key(), input, |ctx| {
                    self.password_input.layout(input, ctx);
                });
            }
            PreviewKind::Typography => {
                let [_, plain, icon, _, paragraph] = typography_showcase_layout(area);
                ctx.push_slot(header_plain_child_key(), plain, |ctx| {
                    <Header as TuiNode<Msg>>::layout(&mut self.header_plain, plain, ctx);
                });
                ctx.push_slot(header_icon_child_key(), icon, |ctx| {
                    <Header as TuiNode<Msg>>::layout(&mut self.header_icon, icon, ctx);
                });
                ctx.push_slot(paragraph_child_key(), paragraph, |ctx| {
                    <TuiParagraph as TuiNode<Msg>>::layout(&mut self.paragraph, paragraph, ctx);
                });
            }
            PreviewKind::TextareaInput => {
                let [_, input, panel] = textarea_showcase_layout(area);
                ctx.push_slot(textarea_input_child_key(), input, |ctx| {
                    self.textarea_input.layout(input, ctx);
                });
                ctx.push_slot(textarea_panel_child_key(), panel, |ctx| {
                    self.textarea_panel.layout(panel, ctx);
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
            PreviewKind::Dropdown => self.layout_dropdowns(area, overlay_bounds, ctx),
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
            PreviewKind::PanelJoinedSeparators => self.render_panel_join_preview(frame, area),
            PreviewKind::PanelTabSeparators => self.render_panel_tabs_join_preview(frame, area),
            PreviewKind::Dialog => self.render_dialog(frame, area),
            PreviewKind::Spinner => self.render_spinner(frame, area),
            PreviewKind::NotificationTriggers => self.render_notification_triggers(frame, area),
            PreviewKind::TextInput => self.render_text_input(frame, area),
            PreviewKind::PasswordInput => self.render_password_input(frame, area),
            PreviewKind::Typography => self.render_typography(frame, area),
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

    fn render_overlay(&self, preview: PreviewKind, frame: &mut Frame, overlay_bounds: Rect) {
        if preview == PreviewKind::Dropdown {
            for index in 0..6 {
                self.dropdown(index)
                    .render_popup_overlay(frame, overlay_bounds);
            }
        }
        if preview == PreviewKind::NotificationTriggers {
            self.notification_triggers.render(frame, overlay_bounds);
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
            if let Some(route) = route
                .path
                .without_first_if(&text_input_child_key())
                .map(EventRoute::new)
            {
                return self.text_input.dispatch_event(&route, event, ctx);
            }
            let Some(route) = route
                .path
                .without_first_if(&text_input_panel_child_key())
                .map(EventRoute::new)
            else {
                return EventOutcome::Ignored;
            };
            return self.text_input_panel.dispatch_event(&route, event, ctx);
        }
        if preview == PreviewKind::PasswordInput {
            let Some(route) = route
                .path
                .without_first_if(&password_input_child_key())
                .map(EventRoute::new)
            else {
                return EventOutcome::Ignored;
            };
            return self.password_input.dispatch_event(&route, event, ctx);
        }
        if preview == PreviewKind::TextareaInput {
            if let Some(route) = route
                .path
                .without_first_if(&textarea_input_child_key())
                .map(EventRoute::new)
            {
                return self.textarea_input.dispatch_event(&route, event, ctx);
            }
            let Some(route) = route
                .path
                .without_first_if(&textarea_panel_child_key())
                .map(EventRoute::new)
            else {
                return EventOutcome::Ignored;
            };
            return self.textarea_panel.dispatch_event(&route, event, ctx);
        }
        if preview == PreviewKind::Tabs {
            if let Some((index, route)) = modal_tabs_open_child_route(route) {
                return self
                    .modal_tabs_button_mut(index)
                    .dispatch_event(&route, event, ctx);
            }
            let Some((index, route)) = tab_demo_child_route(route) else {
                return EventOutcome::Ignored;
            };
            return self.tab_demo_mut(index).dispatch_event(&route, event, ctx);
        }
        if preview == PreviewKind::Toggle {
            if let Some(route) = route
                .path
                .without_first_if(&toggle_switch_child_key())
                .map(EventRoute::new)
            {
                return self.toggle.dispatch_event(&route, event, ctx);
            }
            let Some(route) = route
                .path
                .without_first_if(&toggle_checkbox_child_key())
                .map(EventRoute::new)
            else {
                return EventOutcome::Ignored;
            };
            return self.checkbox_toggle.dispatch_event(&route, event, ctx);
        }
        if preview == PreviewKind::NotificationTriggers {
            return self.notification_trigger_dispatch_event(route, event, ctx);
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
        if preview == PreviewKind::PanelJoinedSeparators {
            let Some(route) = panel_join_demo_child_route(route) else {
                return EventOutcome::Ignored;
            };
            return self.panel_join_demo.dispatch_event(&route, event, ctx);
        }
        if preview == PreviewKind::PanelTabSeparators {
            let Some(route) = panel_tabs_join_demo_child_route(route) else {
                return EventOutcome::Ignored;
            };
            return self.panel_tabs_join_demo.dispatch_event(&route, event, ctx);
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
                if dispatch_focus_child(
                    &mut self.text_input,
                    target,
                    text_input_child_key(),
                    focused,
                    ctx,
                ) {
                    return;
                }
                dispatch_focus_child(
                    &mut self.text_input_panel,
                    target,
                    text_input_panel_child_key(),
                    focused,
                    ctx,
                );
            }
            PreviewKind::PasswordInput => {
                dispatch_focus_child(
                    &mut self.password_input,
                    target,
                    password_input_child_key(),
                    focused,
                    ctx,
                );
            }
            PreviewKind::TextareaInput => {
                if dispatch_focus_child(
                    &mut self.textarea_input,
                    target,
                    textarea_input_child_key(),
                    focused,
                    ctx,
                ) {
                    return;
                }
                dispatch_focus_child(
                    &mut self.textarea_panel,
                    target,
                    textarea_panel_child_key(),
                    focused,
                    ctx,
                );
            }
            PreviewKind::Tabs => {
                if let Some((index, child_target)) =
                    indexed_child_target(target, modal_tabs_open_index)
                {
                    self.modal_tabs_button_mut(index)
                        .dispatch_focus(&child_target, focused, ctx);
                } else {
                    dispatch_focus_indexed(
                        target,
                        tab_demo_index,
                        |state, index| state.tab_demo_mut(index),
                        self,
                        focused,
                        ctx,
                    );
                }
            }
            PreviewKind::Toggle => {
                if dispatch_focus_child(
                    &mut self.toggle,
                    target,
                    toggle_switch_child_key(),
                    focused,
                    ctx,
                ) {
                    return;
                }
                dispatch_focus_child(
                    &mut self.checkbox_toggle,
                    target,
                    toggle_checkbox_child_key(),
                    focused,
                    ctx,
                );
            }
            PreviewKind::NotificationTriggers => dispatch_focus_indexed(
                target,
                notification_button_index,
                |state, index| &mut state.notification_buttons[index],
                self,
                focused,
                ctx,
            ),
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
            PreviewKind::PanelJoinedSeparators => {
                dispatch_focus_child(
                    &mut self.panel_join_demo,
                    target,
                    panel_join_demo_child_key(),
                    focused,
                    ctx,
                );
            }
            PreviewKind::PanelTabSeparators => {
                dispatch_focus_child(
                    &mut self.panel_tabs_join_demo,
                    target,
                    panel_tabs_join_demo_child_key(),
                    focused,
                    ctx,
                );
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
            .merge(Animated::tick(
                &mut self.notification_triggers,
                dt,
                settings,
            ))
            .merge(Animated::tick(&mut self.button, dt, settings))
            .merge(Animated::tick(
                &mut self.notification_buttons[0],
                dt,
                settings,
            ))
            .merge(Animated::tick(
                &mut self.notification_buttons[1],
                dt,
                settings,
            ))
            .merge(Animated::tick(
                &mut self.notification_buttons[2],
                dt,
                settings,
            ))
            .merge(Animated::tick(
                &mut self.notification_buttons[3],
                dt,
                settings,
            ))
            .merge(Animated::tick(&mut self.toggle, dt, settings))
            .merge(Animated::tick(&mut self.checkbox_toggle, dt, settings))
            .merge(Animated::tick(&mut self.dialog_100, dt, settings))
            .merge(Animated::tick(&mut self.dialog_80, dt, settings))
            .merge(Animated::tick(&mut self.dialog_60, dt, settings))
            .merge(Animated::tick(&mut self.dialog_40, dt, settings))
            .merge(Animated::tick(&mut self.dialog_20, dt, settings))
            .merge(Animated::tick(&mut self.dialog_top, dt, settings))
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
            .merge(Animated::tick(
                &mut self.tabs_modal_minimal_button,
                dt,
                settings,
            ))
            .merge(Animated::tick(
                &mut self.tabs_modal_underline_button,
                dt,
                settings,
            ))
            .merge(Animated::tick(
                &mut self.tabs_modal_boxed_button,
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
            .merge(self.panel_join_demo.tick(dt, settings))
            .merge(self.panel_tabs_join_demo.tick(dt, settings))
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
            .merge(self.text_input_panel.tick(dt, settings))
            .merge(Animated::tick(&mut self.password_input, dt, settings))
            .merge(Animated::tick(&mut self.textarea_input, dt, settings))
            .merge(self.textarea_panel.tick(dt, settings))
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

    fn modal_tabs_button_mut(&mut self, index: usize) -> &mut Button<Msg> {
        match index {
            1 => &mut self.tabs_modal_underline_button,
            2 => &mut self.tabs_modal_boxed_button,
            _ => &mut self.tabs_modal_minimal_button,
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
            "Filled 4 • Inline label",
            &format!(
                "selected: {:?}\nquery: {:?}\nInline label keeps selected value bold.",
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
            "Filled 6 • No selection text",
            &format!(
                "selected: {:?}\nquery: {:?}\nShows --None-- before a value is chosen.",
                self.dropdown_filled_no_search_immediate.selected_id(),
                self.dropdown_filled_no_search_immediate.search_query()
            ),
        );

        for (index, area) in areas.iter().copied().enumerate() {
            self.dropdown(index).render(frame, dropdown_area(area));
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
                "Open app-level dialogs with backdrop dim tween. Press x or Esc to close.",
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
            5 => &self.dialog_top,
            _ => &self.dialog_100,
        }
    }

    fn dialog_button_mut(&mut self, index: usize) -> &mut Button<Msg> {
        match index {
            1 => &mut self.dialog_80,
            2 => &mut self.dialog_60,
            3 => &mut self.dialog_40,
            4 => &mut self.dialog_20,
            5 => &mut self.dialog_top,
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

    fn layout_dropdowns(&mut self, area: Rect, overlay_bounds: Rect, ctx: &mut LayoutCtx) {
        let [_, body] = dropdown_preview_layout(area);
        let grid_areas = dropdown_grid_areas(body);
        let areas = grid_areas.map(dropdown_area);

        ctx.push_slot(dropdown_child_key(0), areas[0], |ctx| {
            self.dropdown_fuzzy_single
                .layout_overlay::<Msg>(areas[0], overlay_bounds, ctx);
        });
        ctx.push_slot(dropdown_child_key(1), areas[1], |ctx| {
            self.dropdown_multi_contains
                .layout_overlay::<Msg>(areas[1], overlay_bounds, ctx);
        });
        ctx.push_slot(dropdown_child_key(2), areas[2], |ctx| {
            self.dropdown_no_search_immediate
                .layout_overlay::<Msg>(areas[2], overlay_bounds, ctx);
        });
        ctx.push_slot(dropdown_child_key(3), areas[3], |ctx| {
            self.dropdown_filled_fuzzy_single
                .layout_overlay::<Msg>(areas[3], overlay_bounds, ctx);
        });
        ctx.push_slot(dropdown_child_key(4), areas[4], |ctx| {
            self.dropdown_filled_multi_contains.layout_overlay::<Msg>(
                areas[4],
                overlay_bounds,
                ctx,
            );
        });
        ctx.push_slot(dropdown_child_key(5), areas[5], |ctx| {
            self.dropdown_filled_no_search_immediate
                .layout_overlay::<Msg>(areas[5], overlay_bounds, ctx);
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

    fn layout_panel_join_preview(&mut self, area: Rect, ctx: &mut LayoutCtx) {
        let [_, body] = panel_separator_preview_layout(area);
        ctx.push_slot(panel_join_demo_child_key(), body, |ctx| {
            self.panel_join_demo.layout(body, ctx);
        });
    }

    fn layout_panel_tabs_join_preview(&mut self, area: Rect, ctx: &mut LayoutCtx) {
        let [_, body] = panel_separator_preview_layout(area);
        ctx.push_slot(panel_tabs_join_demo_child_key(), body, |ctx| {
            self.panel_tabs_join_demo.layout(body, ctx);
        });
    }

    fn render_text_input(&self, frame: &mut Frame, area: Rect) {
        let [instructions, input, panel] = text_input_showcase_layout(area);
        frame.render_widget(
            Paragraph::new(
                "Type text. Enter submits. Tab inserts spaces. Esc/Ctrl+[ returns to list. Ctrl+Q quits from gallery root.\n\
                 Plain input has hotkey |i|. Nested panel has hotkey |p|.\n\
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
        self.text_input_panel.render(frame, panel);
    }

    fn render_password_input(&self, frame: &mut Frame, area: Rect) {
        let [instructions, input, preview] = password_input_showcase_layout(area);
        frame.render_widget(
            Paragraph::new(
                "Type a secret. Enter submits. Tab inserts spaces. Esc/Ctrl+[ returns to list. Ctrl+Q quits from gallery root.\n\
                 Text is masked; current editing shortcuts match TextInput.\n\
                 Shortcuts:\n\
                 • Ctrl+Left / Ctrl+Right / Alt+B / Alt+F : Jump word backward / forward\n\
                 • Ctrl+Backspace / Ctrl+W                : Delete word backward\n\
                 • Ctrl+Delete / Alt+D                    : Delete word forward\n\
                 • Ctrl+A / Ctrl+E                        : Move cursor to start / end of line\n\
                 • Ctrl+U / Ctrl+K                        : Delete to start / end of line\n\
                 • Ctrl+C                                 : Clear input",
            ),
            instructions,
        );
        self.password_input.render(frame, input);
        frame.render_widget(
            Paragraph::new(format!(
                "Current value: {}",
                self.password_input.current_value()
            )),
            preview,
        );
    }

    fn render_typography(&self, frame: &mut Frame, area: Rect) {
        let [instructions, plain, icon, paragraph_label, paragraph] =
            typography_showcase_layout(area);
        frame.render_widget(
            Paragraph::new(
                "Typography renders semantic text primitives.\n\
                 Headers support plain labels and Nerd Font icons.\n\
                 Paragraphs can wrap, clip, or ellipsize overflowing copy.",
            ),
            instructions,
        );
        self.header_plain.render(frame, plain);
        self.header_icon.render(frame, icon);
        frame.render_widget(Paragraph::new("Paragraph with ellipsis:"), paragraph_label);
        self.paragraph.render(frame, paragraph);
    }

    fn render_textarea_input(&self, frame: &mut Frame, area: Rect) {
        let [instructions, input, panel] = textarea_showcase_layout(area);
        frame.render_widget(
            Paragraph::new(
                "Type text. Enter inserts newline. Ctrl+Enter/Ctrl+D submits. Tab inserts spaces. Esc/Ctrl+[ returns to list. Ctrl+Q quits from gallery root.\n\
                 Plain textarea has hotkey |t|. Nested panel has hotkey |p|.\n\
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
        self.textarea_panel.render(frame, panel);
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
        let [_, switch_area, checkbox_area] = toggle_layout(area);
        ctx.push_slot(toggle_switch_child_key(), switch_area, |ctx| {
            self.toggle.layout(switch_area, ctx);
        });
        ctx.push_slot(toggle_checkbox_child_key(), checkbox_area, |ctx| {
            self.checkbox_toggle.layout(checkbox_area, ctx);
        });
    }

    fn render_toggle(&self, frame: &mut Frame, area: Rect) {
        let [instructions, switch_area, checkbox_area] = toggle_layout(area);
        frame.render_widget(
            Paragraph::new(
                "Enter/Space toggles. Press x for switch style or i for checkbox style.",
            ),
            instructions,
        );
        self.toggle.render(frame, switch_area);
        self.checkbox_toggle.render(frame, checkbox_area);
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

    fn layout_notification_triggers(&mut self, area: Rect, ctx: &mut LayoutCtx) {
        let [_, buttons, _] = notification_trigger_layout(area);
        let button_areas = notification_button_areas(buttons);
        for (index, button_area) in button_areas.into_iter().enumerate() {
            ctx.push_slot(notification_button_child_key(index), button_area, |ctx| {
                self.notification_buttons[index].layout(button_area, ctx);
            });
        }
    }

    fn render_notification_triggers(&self, frame: &mut Frame, area: Rect) {
        let [help, buttons, footer] = notification_trigger_layout(area);
        frame.render_widget(
            Paragraph::new(
                "Press a button to push a toast. Notifications render over the full app area, not inside this preview panel.",
            ),
            help,
        );
        for (index, button_area) in notification_button_areas(buttons).into_iter().enumerate() {
            self.notification_buttons[index].render(frame, button_area);
        }
        frame.render_widget(
            Paragraph::new("Hotkeys: ni info • ns success • nw warning • ne error"),
            footer,
        );
    }

    fn notification_trigger_dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<Msg>,
    ) -> EventOutcome {
        let Some((index, route)) = notification_button_child_route(route) else {
            return EventOutcome::Ignored;
        };
        self.notification_buttons[index].dispatch_event(&route, event, ctx)
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

    fn render_panel_join_preview(&self, frame: &mut Frame, area: Rect) {
        let [help, body] = panel_separator_preview_layout(area);
        frame.render_widget(
            Paragraph::new("Split/Flex/Grid separators share one Separator model. PanelHost patches edge contacts into join glyphs."),
            help,
        );
        self.panel_join_demo.render(frame, body);
    }

    fn render_panel_tabs_join_preview(&self, frame: &mut Frame, area: Rect) {
        let [help, body] = panel_separator_preview_layout(area);
        frame.render_widget(
            Paragraph::new("Tabs can host split/nested separators. Focus lands on Tabs/body components; separator glyphs are not focus targets."),
            help,
        );
        self.panel_tabs_join_demo.render(frame, body);
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
        let [buttons_area, demos_area] = modal_tabs_preview_layout(area);
        let button_areas = modal_tabs_button_areas(buttons_area);
        ctx.push_slot(modal_tabs_open_child_key(0), button_areas[0], |ctx| {
            self.tabs_modal_minimal_button.layout(button_areas[0], ctx);
        });
        ctx.push_slot(modal_tabs_open_child_key(1), button_areas[1], |ctx| {
            self.tabs_modal_underline_button
                .layout(button_areas[1], ctx);
        });
        ctx.push_slot(modal_tabs_open_child_key(2), button_areas[2], |ctx| {
            self.tabs_modal_boxed_button.layout(button_areas[2], ctx);
        });
        let [minimal, underline, boxed] = tabs_areas(demos_area);
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
        let [buttons_area, demos_area] = modal_tabs_preview_layout(area);
        let button_areas = modal_tabs_button_areas(buttons_area);
        self.tabs_modal_minimal_button
            .render(frame, button_areas[0]);
        self.tabs_modal_underline_button
            .render(frame, button_areas[1]);
        self.tabs_modal_boxed_button.render(frame, button_areas[2]);
        let [minimal, underline, boxed] = tabs_areas(demos_area);
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

fn gallery_list_child_key() -> ChildKey {
    ChildKey::new("component-list")
}

fn gallery_preview_child_key() -> ChildKey {
    ChildKey::new("preview")
}

fn gallery_theme_child_key() -> ChildKey {
    ChildKey::new("theme")
}

fn text_input_child_key() -> ChildKey {
    ChildKey::new("text-input")
}

fn text_input_panel_child_key() -> ChildKey {
    ChildKey::new("text-input-panel")
}

fn password_input_child_key() -> ChildKey {
    ChildKey::new("password-input")
}

fn header_plain_child_key() -> ChildKey {
    ChildKey::new("header-plain")
}

fn header_icon_child_key() -> ChildKey {
    ChildKey::new("header-icon")
}

fn paragraph_child_key() -> ChildKey {
    ChildKey::new("paragraph")
}

fn textarea_input_child_key() -> ChildKey {
    ChildKey::new("textarea-input")
}

fn textarea_panel_child_key() -> ChildKey {
    ChildKey::new("textarea-panel")
}

fn toggle_switch_child_key() -> ChildKey {
    ChildKey::new("toggle-switch")
}

fn toggle_checkbox_child_key() -> ChildKey {
    ChildKey::new("toggle-checkbox")
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

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
enum ComponentKind {
    Tabs,
    Panel,
    PanelJoinedSeparators,
    PanelTabSeparators,
    Dialog,
    Spinner,
    Notifications,
    NotificationTriggers,
    Typography,
    Layouts,
    LayoutFlex,
    LayoutSplit,
    LayoutStack,
    LayoutOverlay,
    LayoutGrid,
    Inputs,
    Button,
    TextInput,
    PasswordInput,
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
    const ALL: [Self; 31] = [
        Self::Tabs,
        Self::Panel,
        Self::PanelJoinedSeparators,
        Self::PanelTabSeparators,
        Self::Dialog,
        Self::Spinner,
        Self::Notifications,
        Self::NotificationTriggers,
        Self::Typography,
        Self::Layouts,
        Self::LayoutFlex,
        Self::LayoutSplit,
        Self::LayoutStack,
        Self::LayoutOverlay,
        Self::LayoutGrid,
        Self::Inputs,
        Self::Button,
        Self::TextInput,
        Self::PasswordInput,
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
            Self::PanelJoinedSeparators => "Joined Separators",
            Self::PanelTabSeparators => "Tabs + Separators",
            Self::Dialog => "Dialog",
            Self::Spinner => "Spinner",
            Self::Notifications => "Notifications",
            Self::NotificationTriggers => "Triggers",
            Self::Typography => "Typography",
            Self::Layouts => "Layouts",
            Self::LayoutFlex => "Flex",
            Self::LayoutSplit => "Split",
            Self::LayoutStack => "Stack",
            Self::LayoutOverlay => "Overlay",
            Self::LayoutGrid => "Grid",
            Self::Inputs => "Inputs",
            Self::Button => "Button",
            Self::TextInput => "Text",
            Self::PasswordInput => "Password",
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
            | Self::PasswordInput
            | Self::TextareaInput
            | Self::Toggle
            | Self::Dropdown => Some(Self::Inputs),
            Self::LayoutFlex
            | Self::LayoutSplit
            | Self::LayoutStack
            | Self::LayoutOverlay
            | Self::LayoutGrid => Some(Self::Layouts),
            Self::PanelJoinedSeparators | Self::PanelTabSeparators => Some(Self::Panel),
            Self::NotificationTriggers => Some(Self::Notifications),
            _ => None,
        }
    }

    fn preview(self) -> PreviewKind {
        match self {
            Self::Tabs => PreviewKind::Tabs,
            Self::Panel => PreviewKind::Panel,
            Self::PanelJoinedSeparators => PreviewKind::PanelJoinedSeparators,
            Self::PanelTabSeparators => PreviewKind::PanelTabSeparators,
            Self::Dialog => PreviewKind::Dialog,
            Self::Spinner => PreviewKind::Spinner,
            Self::Notifications => PreviewKind::NotificationTriggers,
            Self::NotificationTriggers => PreviewKind::NotificationTriggers,
            Self::Typography => PreviewKind::Typography,
            Self::Layouts | Self::LayoutFlex => PreviewKind::LayoutFlex,
            Self::LayoutSplit => PreviewKind::LayoutSplit,
            Self::LayoutStack => PreviewKind::LayoutStack,
            Self::LayoutOverlay => PreviewKind::LayoutOverlay,
            Self::LayoutGrid => PreviewKind::LayoutGrid,
            Self::Inputs | Self::Button => PreviewKind::Button,
            Self::TextInput => PreviewKind::TextInput,
            Self::PasswordInput => PreviewKind::PasswordInput,
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
    PanelJoinedSeparators,
    PanelTabSeparators,
    Dialog,
    Spinner,
    NotificationTriggers,
    Typography,
    LayoutFlex,
    LayoutSplit,
    LayoutStack,
    LayoutOverlay,
    LayoutGrid,
    TextInput,
    PasswordInput,
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
            Self::PanelJoinedSeparators => "Joined Separators",
            Self::PanelTabSeparators => "Tabs + Separators",
            Self::Dialog => "Dialog",
            Self::Spinner => "Spinner",
            Self::NotificationTriggers => "Notification Triggers",
            Self::Typography => "Typography",
            Self::LayoutFlex => "Flex Layout",
            Self::LayoutSplit => "Split Layout",
            Self::LayoutStack => "Stack Layout",
            Self::LayoutOverlay => "Overlay Layout",
            Self::LayoutGrid => "Grid Layout",
            Self::TextInput => "Text",
            Self::PasswordInput => "Password",
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
    fn bottom_right_panel_title_route_is_recognized() {
        let route = EventRoute::new(tuicore::TreePath::from_keys([panel_title_child_key(3)]));

        let (index, child_route) = panel_title_child_route(&route).expect("route should resolve");

        assert_eq!(index, 3);
        assert!(child_route.path.is_empty());
    }

    #[test]
    fn parent_preview_uses_first_child_demo() {
        assert_eq!(ComponentKind::Layouts.preview(), PreviewKind::LayoutFlex);
        assert_eq!(ComponentKind::Inputs.preview(), PreviewKind::Button);
        assert_eq!(ComponentKind::DataView.preview(), PreviewKind::DataList);
    }
}
