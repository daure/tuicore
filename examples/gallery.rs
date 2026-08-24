use std::time::Duration;

mod gallery_demo;
mod list_control;

use gallery_demo::checklist::{ChecklistShowcase, release_checklist};
use gallery_demo::data::{DataViewMode, DemoRow, data_event_status, data_view_layout};
use gallery_demo::dialogs::{
    DialogExample, DockOverlayExample, GalleryDialogContent, GalleryDockOverlayContent,
    confirmation_button, dialog_body_area, dialog_button, dialog_button_areas,
    dialog_demo_child_key, dialog_demo_child_route, dialog_demo_index, dock_overlay_button,
    gallery_confirmation_dialog, gallery_dialog, gallery_dock_overlay,
};
use gallery_demo::diffs::{
    DiffDemo, inline_diff_demo, raw_patch_diff_demo, side_by_side_diff_demo, word_diff_demo,
};
use gallery_demo::dropdowns::{
    DropdownDemoItem, dropdown_area, dropdown_child_key, dropdown_child_route,
    dropdown_column_layout, dropdown_disabled, dropdown_filled_fuzzy_single,
    dropdown_filled_multi_contains, dropdown_filled_searchable_none_immediate,
    dropdown_fuzzy_single, dropdown_grid_areas, dropdown_index, dropdown_multi_contains,
    dropdown_no_search_immediate, dropdown_preview_layout,
};
use gallery_demo::forms::{FormControlId, ValidatedForm};
use gallery_demo::inputs::{
    button_layout, chip_layout, date_time_showcase_layout, password_input_showcase_layout,
    text_input_showcase_layout, textarea_showcase_layout, toggle_layout, typography_showcase,
};
use gallery_demo::layouts::{
    DemoBox, layout_demo_body, layout_flex_demo, layout_grid_demo, layout_layered_demo,
    layout_split_demo, layout_stack_demo, render_layout_intro,
};
use gallery_demo::notifications::{
    notification_button_areas, notification_button_child_key, notification_button_child_route,
    notification_button_index, notification_buttons, notification_for_index,
    notification_trigger_layout,
};
use gallery_demo::panels::{
    PanelTitleChoice, apply_panel_choice, panel_demo, panel_demo_child_key, panel_demo_child_route,
    panel_join_demo, panel_join_demo_child_key, panel_join_demo_child_route, panel_preview_layout,
    panel_separator_preview_layout, panel_tabs_join_demo, panel_tabs_join_demo_child_key,
    panel_tabs_join_demo_child_route, panel_title_child_key, panel_title_child_route,
    panel_title_column_layout, panel_title_control_areas, panel_title_dropdown,
    panel_title_dropdown_area, panel_title_index, panel_variant_child_key,
    panel_variant_child_route, panel_variant_index, panel_variants_layout,
};
use gallery_demo::relative_date::RelativeDateDemo;
#[cfg(test)]
use gallery_demo::relative_date::reference_picker_child_key;
use gallery_demo::speed_reader::{
    SpeedReaderExample, gallery_speed_reader, speed_reader_button_areas, speed_reader_buttons,
    speed_reader_child_key, speed_reader_index, speed_reader_route,
};
use gallery_demo::status_bar::demo_weather_report;
use gallery_demo::tabs::{
    ModalTabsExample, labeled_area, modal_tabs_button_areas, modal_tabs_dialog,
    modal_tabs_open_child_key, modal_tabs_open_child_route, modal_tabs_open_index,
    modal_tabs_preview_layout, tab_demo_child_key, tab_demo_child_route, tab_demo_index,
    tabs_areas, tabs_demo,
};
use list_control::{
    ListControlShowcase, compact_names, entity_table, reorder_mode, reorder_mode_tree,
};
#[cfg(test)]
use list_control::{compact_name_controls, entity_controls, reorder_control};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
#[cfg(test)]
use tuicore::{CalendarView, LayoutProposal};

use futures::StreamExt;
use ratatui::widgets::{Borders, Paragraph};
use rig::agent::{MultiTurnStreamItem, Text as RigText};
use rig::client::CompletionClient;
use rig::completion::ToolDefinition;
use rig::providers::chatgpt;
use rig::schemars::JsonSchema;
use rig::streaming::{StreamedAssistantContent, StreamingPrompt};
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use time::{Date, Month, PrimitiveDateTime, Time};
use tuicore::components::{AiDock, LlmEvent, StoreDebugView, ToolPolicy};
use tuicore::{
    ActivationMode, Animated, AnimationSettings, Button, Calendar, CalendarEntryRole, CalendarSpan,
    CalendarTypedEvent, ChildKey, Chip, ChipColorRole, ConfirmationDialog,
    ConfirmationDialogOutcome, DataView, DataViewTypedEvent, DatePicker, DatePickerDropdown,
    DateTimePicker, DateTimePickerDropdown, DateTimePickerLayout, DialogBackdrop,
    DialogCloseReason, DialogHost, DialogLayer, DialogLayerPlacement, DispatchOutcome, DockSpec,
    Dropdown, EventCtx, EventOutcome, EventRoute, Flex, FocusCtx, FocusId, FocusRequest,
    FocusTarget, Grid, HotkeyLabelMode, InputChrome, InspectField, InspectValue, Key, KeyEvent,
    KeyModifiers, Language, LayoutCtx, LayoutResult, LifecycleCtx, MenuButton, MenuItem,
    ModalCloseReason, Overlay, Panel, PanelHost, PanelTitlePosition, PasswordInput, RenderCtx,
    ScrollContainer, SeasonalEmptyState, SelectionGlyphs, SelectionMode, SelectionTrigger,
    SpeedReader, Spinner, Split, Stack, StatusBar, StatusBarMenuItem, StoreLogEntry, StoreLogPhase,
    Tabs, TabsVariant, TagInput, TextInput, TextareaInput, TickResult, TimePicker, TimePrecision,
    ToastRack, Toggle, TreeAdapter, TreePath, TuiEvent, TuiNode, WeatherProviderConfig,
};

#[derive(Debug, PartialEq)]
enum Msg {
    DialogOpened(DialogExample),
    DialogClosed(DialogCloseReason),
    DockOverlayOpened(DockOverlayExample),
    DockOverlayClosed(DialogCloseReason),
    ModalTabsOpened(ModalTabsExample),
    ModalTabsClosed(ModalCloseReason),
    PanelAction(&'static str),
    TabsAction(&'static str, usize),
    NotificationTriggered(usize),
    StoreViewOpened,
    StoreViewClosed(ModalCloseReason),
    ConfirmationOpened,
    ConfirmationFinished(ConfirmationDialogOutcome),
    SpeedReaderOpened(SpeedReaderExample),
    SpeedReaderClosed(DialogCloseReason),
    OpenAiDock,
    CloseAiDock,
    FormNameChanged(String),
    FormDescriptionChanged(String),
    FormPasswordChanged(String),
    FormStartSelected(Date),
    FormEndSelected(Date),
    RelativeReferenceSelected(Date),
    RelativeReferenceTimeSelected(Time),
    RelativeTargetSelected(Date),
    RelativeTargetTimeSelected(Time),
    FormSubmitAttempt,
    FormSubmitRequested(FormControlId),
    FormControlEditEnded(FormControlId),
}

type DialogDemoLayer = DialogLayer<Gallery, DialogHost<GalleryDialogContent, Msg>>;
type ModalTabsLayer = DialogLayer<DialogDemoLayer, Tabs<Msg>>;
type RootLayer = DialogLayer<ModalTabsLayer, DialogHost<GalleryDockOverlayContent, Msg>>;
type StoreViewLayer = DialogLayer<RootLayer, StoreDebugView<Msg>>;
type ConfirmationLayer = DialogLayer<StoreViewLayer, ConfirmationDialog<Msg>>;
type SpeedReaderLayer = DialogLayer<ConfirmationLayer, DialogHost<SpeedReader, Msg>>;

type AppRoot = DialogLayer<SpeedReaderLayer, tuicore::components::AiDock<Msg>>;

fn speed_reader_layer(root: &mut AppRoot) -> &mut SpeedReaderLayer {
    root.base_mut()
}

fn confirmation_layer(root: &mut AppRoot) -> &mut ConfirmationLayer {
    speed_reader_layer(root).base_mut()
}

fn store_view_layer(root: &mut AppRoot) -> &mut StoreViewLayer {
    confirmation_layer(root).base_mut()
}

fn get_root_layer(root: &mut AppRoot) -> &mut RootLayer {
    store_view_layer(root).base_mut()
}

fn dialog_demo_layer(root: &mut AppRoot) -> &mut DialogDemoLayer {
    get_root_layer(root).base_mut().base_mut()
}

fn modal_tabs_layer(root: &mut AppRoot) -> &mut ModalTabsLayer {
    get_root_layer(root).base_mut()
}

fn gallery(root: &mut AppRoot) -> &mut Gallery {
    get_root_layer(root).base_mut().base_mut().base_mut()
}

fn main() -> tuicore::Result<()> {
    tuicore::init();
    let dialog_layer = DialogLayer::new(Gallery::new(), gallery_dialog()).active(false);
    let tabs_layer = DialogLayer::new(dialog_layer, modal_tabs_dialog()).active(false);
    let root = DialogLayer::new(tabs_layer, gallery_dock_overlay()).active(false);
    let store_view = DialogLayer::new(root, empty_store_debug_dialog())
        .active(false)
        .layer_percent(76)
        .layer_cross_percent(88)
        .placement(DialogLayerPlacement::Center)
        .backdrop(DialogBackdrop::dim().amount(0.45));
    let confirmation = DialogLayer::new(store_view, gallery_confirmation_dialog())
        .active(false)
        .fit_content()
        .placement(DialogLayerPlacement::Center)
        .backdrop(DialogBackdrop::dim().amount(0.55));

    let speed_reader = DialogLayer::new(
        confirmation,
        gallery_speed_reader(SpeedReaderExample::Markdown),
    )
    .active(false)
    .fit_content()
    .fit_content_max(72, 12)
    .placement(DialogLayerPlacement::Center)
    .backdrop(DialogBackdrop::dim().amount(0.58));

    let final_root = DialogLayer::new(speed_reader, ai_dock_dialog()).active(false);

    tuicore::TreeApp::new(final_root)
        .on_message(|root, msg, ctx| {
            if gallery(root).previews.validated_form.apply_message(&msg) {
                ctx.request_layout();
                ctx.request_redraw();
                return;
            }
            if gallery(root).previews.relative_date.apply_message(&msg) {
                ctx.request_layout();
                ctx.request_redraw();
                return;
            }
            match msg {
                Msg::DialogOpened(example) => {
                    let dialog_layer = dialog_demo_layer(root);
                    dialog_layer.layer_mut().child_mut().set_example(example);
                    dialog_layer
                        .layer_mut()
                        .dialog_mut()
                        .set_top_left(example.title());
                    dialog_layer
                        .layer_mut()
                        .dialog_mut()
                        .set_bottom_left("Esc closes");
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
                    dialog_layer.set_active_with_context(true, ctx);
                }
                Msg::DialogClosed(_reason) => {
                    dialog_demo_layer(root).set_active_with_context(false, ctx);
                }
                Msg::DockOverlayOpened(example) => {
                    let r = get_root_layer(root);
                    r.layer_mut().child_mut().set_example(example);
                    r.layer_mut().dialog_mut().set_top_left(example.title());
                    let dock = match example {
                        DockOverlayExample::Top => DockSpec::top(30),
                        DockOverlayExample::Bottom => DockSpec::bottom(30),
                        DockOverlayExample::Left => DockSpec::left(32),
                        DockOverlayExample::Right => DockSpec::right(32),
                        DockOverlayExample::BottomSnackbar => {
                            DockSpec::bottom(16).cross_percent(80)
                        }
                        DockOverlayExample::BottomTabs => DockSpec::bottom(36),
                    };
                    r.set_docked(dock);
                    r.layer_mut()
                        .child_mut()
                        .set_tabs_edge_borders(dock.edge_borders());
                    r.set_backdrop(DialogBackdrop::dim().amount(0.55));
                    r.set_active_with_context(true, ctx);
                }
                Msg::DockOverlayClosed(_reason) => {
                    get_root_layer(root).set_active_with_context(false, ctx);
                }
                Msg::ModalTabsOpened(variant) => {
                    let tabs_layer = modal_tabs_layer(root);
                    tabs_layer.layer_mut().set_variant(variant.variant());
                    tabs_layer.layer_mut().clear_edge_borders();
                    tabs_layer.layer_mut().prepare_modal_open(ctx.animation());
                    let dock = match variant {
                        ModalTabsExample::CenterMinimal
                        | ModalTabsExample::CenterUnderline
                        | ModalTabsExample::CenterBoxed => None,
                        ModalTabsExample::Top => Some(DockSpec::top(30)),
                        ModalTabsExample::Bottom => Some(DockSpec::bottom(30)),
                        ModalTabsExample::Left => Some(DockSpec::left(32)),
                        ModalTabsExample::Right => Some(DockSpec::right(32)),
                        ModalTabsExample::BottomSnackbar => {
                            Some(DockSpec::bottom(16).cross_percent(80))
                        }
                    };
                    if let Some(dock) = dock {
                        tabs_layer.set_docked(dock);
                    } else {
                        tabs_layer.set_placement(DialogLayerPlacement::Center);
                        tabs_layer.set_layer_percent(72);
                        tabs_layer.set_layer_cross_percent(100);
                        tabs_layer.layer_mut().set_edge_borders(Borders::ALL);
                    }
                    tabs_layer.set_backdrop(DialogBackdrop::dim().amount(0.55));
                    tabs_layer.set_active_with_context(true, ctx);
                }
                Msg::ModalTabsClosed(_reason) => {
                    let tabs_layer = modal_tabs_layer(root);
                    tabs_layer.layer_mut().prepare_modal_close();
                    tabs_layer.set_active_with_context(false, ctx);
                }
                Msg::PanelAction(action) => {
                    gallery(root).previews.panel_action_status = format!("Panel action: {action}");
                    ctx.request_redraw();
                }
                Msg::TabsAction(action, selected) => {
                    gallery(root).previews.tabs_action_status =
                        format!("{action} on tab {}", selected + 1);
                    ctx.request_redraw();
                }
                Msg::NotificationTriggered(index) => {
                    gallery(root)
                        .previews
                        .notification_triggers
                        .push(notification_for_index(index).ttl(Duration::from_secs(4)));
                    ctx.request_redraw();
                }
                Msg::StoreViewOpened => {
                    let state = gallery(root).store_debug_state();
                    let events = gallery(root).store_debug_events();
                    let layer = store_view_layer(root);
                    layer.layer_mut().set_snapshot(state, events);
                    layer.set_active_with_context(true, ctx);
                }
                Msg::StoreViewClosed(_reason) => {
                    store_view_layer(root).set_active_with_context(false, ctx);
                }
                Msg::ConfirmationOpened => {
                    let layer = confirmation_layer(root);
                    layer.replace_layer(gallery_confirmation_dialog(), ctx);
                    layer.set_active_with_context(true, ctx);
                }
                Msg::ConfirmationFinished(outcome) => {
                    confirmation_layer(root).set_active_with_context(false, ctx);
                    gallery(root).previews.confirmation_status = match outcome {
                        ConfirmationDialogOutcome::Confirmed => {
                            "Confirmed: filter deleted".to_string()
                        }
                        ConfirmationDialogOutcome::Cancelled => {
                            "Cancelled: filter kept".to_string()
                        }
                        ConfirmationDialogOutcome::Closed(reason) => {
                            format!("Closed without a choice: {reason:?}")
                        }
                    };
                    ctx.request_redraw();
                }
                Msg::SpeedReaderOpened(example) => {
                    let layer = speed_reader_layer(root);
                    layer.replace_layer(gallery_speed_reader(example), ctx);
                    layer.set_active_with_context(true, ctx);
                }
                Msg::SpeedReaderClosed(_reason) => {
                    let layer = speed_reader_layer(root);
                    layer.layer_mut().child_mut().pause();
                    layer.set_active_with_context(false, ctx);
                }
                Msg::OpenAiDock => {
                    root.set_docked(DockSpec::bottom(80).cross_percent(80));
                    root.set_backdrop(DialogBackdrop::dim().amount(0.55));
                    root.set_active_with_context(true, ctx);
                }
                Msg::CloseAiDock => {
                    root.set_active_with_context(false, ctx);
                }
                Msg::FormNameChanged(_)
                | Msg::FormDescriptionChanged(_)
                | Msg::FormPasswordChanged(_)
                | Msg::FormStartSelected(_)
                | Msg::FormEndSelected(_)
                | Msg::FormSubmitAttempt
                | Msg::FormSubmitRequested(_)
                | Msg::FormControlEditEnded(_)
                | Msg::RelativeReferenceSelected(_)
                | Msg::RelativeReferenceTimeSelected(_)
                | Msg::RelativeTargetSelected(_)
                | Msg::RelativeTargetTimeSelected(_) => {
                    unreachable!("component messages handled above")
                }
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
    footer: StatusBar<Msg>,
    previews: Box<PreviewState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct GalleryAreas {
    list_panel: Rect,
    list_body: Rect,
    preview_panel: Rect,
    preview_body: Rect,
    footer: Rect,
}

impl Gallery {
    fn new() -> Self {
        let component_list = DataView::list(
            ComponentKind::ALL,
            |component| *component,
            |component| component.title().to_string(),
        )
        .action_bar(true)
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
            ComponentKind::StatusBar,
            ComponentKind::Layouts,
            ComponentKind::DataView,
            ComponentKind::ListControl,
            ComponentKind::DiffViewer,
        ])
        .focused(true);

        let footer = StatusBar::new()
            .menu_items([StatusBarMenuItem::Theme, StatusBarMenuItem::WeatherForecast])
            .weather_report(demo_weather_report())
            .weather_provider(WeatherProviderConfig::new().enabled(false))
            .on_ai_open(|| Msg::OpenAiDock)
            .on_store_view_open(|| Msg::StoreViewOpened);

        Self {
            component_list,
            selected: ComponentKind::Tabs,
            areas: GalleryAreas::default(),
            list_panel: Panel::new()
                .top_left("Components")
                .hotkey("c")
                .focused(true),
            preview_panel: Panel::new().top_left(ComponentKind::Tabs.preview().title()),
            footer,
            previews: Box::new(PreviewState::new()),
        }
    }

    fn select(&mut self, selected: ComponentKind) {
        self.selected = selected;
        self.preview_panel.set_top_left(selected.preview().title());
    }

    fn store_debug_state(&self) -> InspectValue {
        InspectValue::object([
            InspectField::new(
                "selected_component",
                InspectValue::string(self.selected.title()),
            ),
            InspectField::new(
                "preview",
                InspectValue::object([
                    InspectField::new(
                        "title",
                        InspectValue::string(self.selected.preview().title()),
                    ),
                    InspectField::new("focused", InspectValue::bool(true)),
                ]),
            ),
            InspectField::new(
                "sample_todos",
                InspectValue::list([
                    InspectValue::object([
                        InspectField::new("title", InspectValue::string("Wire Store view")),
                        InspectField::new("done", InspectValue::bool(true)),
                    ]),
                    InspectValue::object([
                        InspectField::new(
                            "title",
                            InspectValue::string("Swap in app StateInspect"),
                        ),
                        InspectField::new("done", InspectValue::bool(false)),
                    ]),
                ]),
            ),
        ])
    }

    fn store_debug_events(&self) -> Vec<StoreLogEntry> {
        vec![
            StoreLogEntry {
                sequence: 1,
                event_label: format!("SelectComponent({})", self.selected.title()),
                phase: StoreLogPhase::Received,
                outcome: None,
            },
            StoreLogEntry {
                sequence: 1,
                event_label: format!("SelectComponent({})", self.selected.title()),
                phase: StoreLogPhase::Handled,
                outcome: Some(DispatchOutcome::layout()),
            },
            StoreLogEntry {
                sequence: 2,
                event_label: "OpenStoreView".to_string(),
                phase: StoreLogPhase::Handled,
                outcome: Some(DispatchOutcome::redraw()),
            },
        ]
    }

    fn selected_from_list_events(&mut self) -> Option<ComponentKind> {
        self.component_list
            .take_events()
            .into_iter()
            .find_map(|event| match event {
                DataViewTypedEvent::HighlightChanged { row_id: Some(id) }
                | DataViewTypedEvent::Activated { row_id: id } => Some(id),
                DataViewTypedEvent::HighlightChanged { row_id: None }
                | DataViewTypedEvent::SelectionChanged { .. }
                | DataViewTypedEvent::TransformChanged { .. } => None,
            })
    }

    fn quit_key(event: &TuiEvent) -> bool {
        let TuiEvent::Key(KeyEvent { code, modifiers }) = event else {
            return false;
        };
        matches!(*code, Key::Char(value) if value.eq_ignore_ascii_case(&'q'))
            && modifiers.contains(KeyModifiers::CONTROL)
    }

    fn overview_key(event: &TuiEvent) -> bool {
        let TuiEvent::Key(key) = event else {
            return false;
        };
        tuicore::keybindings().focus().unfocus_matches(*key)
    }

    fn focus_component_overview(route: &EventRoute, ctx: &mut EventCtx<Msg>) -> EventOutcome {
        let current_path = ctx.current_path();
        let gallery_path = if current_path.is_empty() {
            TreePath::new()
        } else {
            current_path
                .strip_suffix(&route.path)
                .expect("absolute event path must end with the Gallery-relative route")
        };
        let overview_path = gallery_path.child(gallery_list_child_key());
        ctx.focus(FocusRequest::TargetAt {
            path: overview_path,
            id: FocusId::new("data-view"),
        });
        ctx.request_redraw();
        ctx.stop_propagation();
        EventOutcome::Handled
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

        self.areas = GalleryAreas {
            list_panel,
            list_body: Panel::inner_area(list_panel),
            preview_panel,
            preview_body: Panel::inner_area(preview_panel),
            footer,
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
                ctx.with_focus_events_before_global_hotkeys(|ctx| {
                    self.previews.layout(
                        self.selected.preview(),
                        self.areas.preview_body,
                        area,
                        ctx,
                    );
                    ctx.register_focusable(FocusId::new("preview"), self.areas.preview_body, true);
                });
            },
        );
        ctx.push_slot(gallery_footer_child_key(), footer, |ctx| {
            ctx.with_overlay_bounds(area, |ctx| self.footer.layout(footer, ctx));
        });

        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        self.list_panel.render(frame, self.areas.list_panel);
        self.component_list.render(frame, self.areas.list_body);

        self.preview_panel.render(frame, self.areas.preview_panel);
        self.previews
            .render(self.selected.preview(), frame, self.areas.preview_body, ctx);

        self.footer.render(frame, self.areas.footer, ctx);
        self.previews.notification_triggers.render(frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<Msg>) -> EventOutcome {
        if Self::quit_key(event) {
            ctx.request_quit();
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }

        let outcome = self.footer.event(event, ctx);
        if outcome.handled() {
            return outcome;
        }

        EventOutcome::Ignored
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        let tick_res = Animated::tick(&mut self.list_panel, dt, settings)
            .merge(Animated::tick(&mut self.preview_panel, dt, settings))
            .merge(Animated::tick(&mut self.component_list, dt, settings))
            .merge(self.footer.tick(dt, settings))
            .merge(self.previews.tick(dt, settings));

        tick_res
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<Msg>,
    ) -> EventOutcome {
        if route.path.first() == Some(&gallery_preview_child_key()) && Self::overview_key(event) {
            let preview = self.selected.preview();
            let preview_route = EventRoute::new(route.path.without_first());
            let child = self
                .previews
                .dispatch_event(preview, &preview_route, event, ctx);
            if matches!(ctx.focus_request(), Some(FocusRequest::Unfocus)) {
                return Self::focus_component_overview(route, ctx);
            }
            if child.handled() {
                return child;
            }
            return Self::focus_component_overview(route, ctx);
        }
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
            .without_first_if(&gallery_footer_child_key())
            .map(EventRoute::new)
        {
            let child = self.footer.dispatch_event(&route, event, ctx);
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

        if let Some(child_target) = target.for_child(&gallery_footer_child_key()) {
            self.footer.dispatch_focus(&child_target, focused, ctx);
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

    fn init(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        self.previews.init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        self.previews.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        self.previews.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        self.previews.destroy(ctx);
    }
}

struct PreviewState {
    text_input: TextInput<Msg>,
    text_input_panel: TextInput<Msg>,
    text_input_numbers: TextInput<Msg>,
    text_input_disabled: TextInput<Msg>,
    password_input: PasswordInput<Msg>,
    password_panel: PasswordInput<Msg>,
    typography: Flex<Msg>,
    textarea_input: TextareaInput<Msg>,
    textarea_panel: TextareaInput<Msg>,
    textarea_disabled: TextareaInput<Msg>,
    date_picker: DatePicker<Msg>,
    time_picker: TimePicker<Msg>,
    date_time_picker: DateTimePicker<Msg>,
    date_dropdown: DatePickerDropdown<Msg>,
    date_time_dropdown: DateTimePickerDropdown<Msg>,
    date_time_status: String,
    relative_date: RelativeDateDemo,
    calendar: Calendar<DemoCalendarEntry, &'static str, Msg>,
    calendar_borderless: Calendar<DemoCalendarEntry, &'static str, Msg>,
    calendar_status: String,
    status_bar: StatusBar<Msg>,
    button: Button<Msg>,
    disabled_button: Button<Msg>,
    button_presses: u32,
    chips: [Chip; 7],
    tag_input: TagInput,
    tag_input_panel: TagInput,
    toggle: Toggle<Msg>,
    checkbox_toggle: Toggle<Msg>,
    dialog_100: Button<Msg>,
    dialog_80: Button<Msg>,
    dialog_60: Button<Msg>,
    dialog_40: Button<Msg>,
    dialog_20: Button<Msg>,
    dialog_top: Button<Msg>,
    speed_reader_buttons: [Button<Msg>; 2],
    confirmation: Button<Msg>,
    confirmation_status: String,
    dock_top: Button<Msg>,
    dock_bottom: Button<Msg>,
    dock_left: Button<Msg>,
    dock_right: Button<Msg>,
    dock_snackbar: Button<Msg>,
    spinner: Spinner,
    seasonal_empty_state: SeasonalEmptyState,
    notification_triggers: ToastRack,
    notification_buttons: [Button<Msg>; 4],
    panel_demo: Panel<Msg>,
    panel_join_demo: PanelHost<Flex<Msg>, Msg>,
    panel_tabs_join_demo: PanelHost<Tabs<Msg>, Msg>,
    panel_one_row_demo: Panel<Msg>,
    panel_actions_demo: Panel<Msg>,
    panel_action_status: String,
    tabs_minimal: Tabs<Msg>,
    tabs_underline: Tabs<Msg>,
    tabs_boxed: Tabs<Msg>,
    tabs_one_row: Tabs<Msg>,
    tabs_action_status: String,
    tabs_modal_buttons: [Button<Msg>; 8],
    data_list: DataView<DemoRow, usize>,
    data_table: DataView<DemoRow, usize>,
    data_list_tree: DataView<DemoRow, usize>,
    data_table_tree: DataView<DemoRow, usize>,
    data_single_select: DataView<DemoRow, usize>,
    data_multi_select: DataView<DemoRow, usize>,
    data_checklist_tree: DataView<DemoRow, usize>,
    data_activate_on_navigate: DataView<DemoRow, usize>,
    data_jira_tickets: DataView<DemoRow, usize>,
    data_status: String,
    list_compact: ListControlShowcase<Msg>,
    list_entity_table: ListControlShowcase<Msg>,
    list_reorder: ListControlShowcase<Msg>,
    list_reorder_tree: ListControlShowcase<Msg>,
    checklist: ChecklistShowcase,
    panel_top_left: Dropdown<PanelTitleChoice, &'static str>,
    panel_top_right: Dropdown<PanelTitleChoice, &'static str>,
    panel_bottom_left: Dropdown<PanelTitleChoice, &'static str>,
    panel_bottom_right: Dropdown<PanelTitleChoice, &'static str>,
    dropdown_fuzzy_single: Dropdown<DropdownDemoItem, &'static str>,
    dropdown_multi_contains: Dropdown<DropdownDemoItem, &'static str>,
    dropdown_no_search_immediate: Dropdown<DropdownDemoItem, &'static str>,
    dropdown_filled_fuzzy_single: Dropdown<DropdownDemoItem, &'static str>,
    dropdown_filled_multi_contains: Dropdown<DropdownDemoItem, &'static str>,
    dropdown_filled_searchable_none_immediate: Dropdown<DropdownDemoItem, &'static str>,
    dropdown_disabled: Dropdown<DropdownDemoItem, &'static str>,
    menu_button: MenuButton<&'static str, Msg>,
    menu_status: String,
    layout_flex: Flex<Msg>,
    layout_split: Split<DemoBox, DemoBox>,
    layout_stack: Stack<Msg>,
    layout_layered: Overlay<DemoBox, DemoBox>,
    layout_grid: Grid<Msg>,
    scroll_mixed: ScrollContainer<Flex<Msg>, Msg>,
    scroll_data_views: ScrollContainer<Flex<Msg>, Msg>,
    validated_form: ValidatedForm,
    diff_side_by_side: DiffDemo<Msg>,
    diff_inline: DiffDemo<Msg>,
    syntax_highlighting: gallery_demo::syntax_highlighting::SyntaxHighlightingDemo,
    diff_word: DiffDemo<Msg>,
    diff_raw_patch: DiffDemo<Msg>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FormNavigation {
    Previous,
    Next,
}

#[derive(Clone, Copy)]
struct FormNavigationPosition {
    index: usize,
    count: usize,
    mode_active: bool,
}

#[derive(Clone)]
struct DemoCalendarEntry {
    id: &'static str,
    title: &'static str,
    span: CalendarSpan,
    role: Option<CalendarEntryRole>,
    detail: &'static str,
}

impl PreviewState {
    fn new() -> Self {
        Self {
            text_input: TextInput::new()
                .placeholder("Type one line...")
                .hotkey("ia")
                .editor_hotkey("ib")
                .max_len(80),
            text_input_panel: TextInput::new()
                .placeholder("Nested input")
                .hotkey("pa")
                .editor_hotkey("pb")
                .style(InputChrome::panel("Description").top_right("Panel style")),
            text_input_numbers: TextInput::new()
                .placeholder("Digits only")
                .value("2026")
                .numbers_only(true)
                .hotkey("na")
                .editor_hotkey("nb")
                .style(InputChrome::panel("Numbers only").top_right("0-9 · $EDITOR")),
            text_input_disabled: TextInput::new()
                .value("Disabled panel input")
                .disabled(true)
                .hotkey("d")
                .style(InputChrome::panel("Disabled")),
            password_input: PasswordInput::new()
                .placeholder("Enter password...")
                .hotkey("pw")
                .value("hunter2")
                .max_len(80),
            password_panel: PasswordInput::new()
                .placeholder("Nested secret")
                .hotkey("pp")
                .style(InputChrome::panel("Secret")),
            typography: typography_showcase(),
            textarea_input: TextareaInput::new()
                .placeholder("Write 2-4 rows...")
                .value("First line\nSecond line\nThird line\nFourth line\nFifth line\nSixth line scrolls")
                .hotkey("ta")
                .editor_hotkey("tb")
                .min_rows(2)
                .max_rows(4),
            textarea_panel: TextareaInput::new()
                .placeholder("Write Markdown...")
                .value("# Draft note\n\n- More detail\n- Third row\n- Fourth row\n- Fifth row scrolls")
                .language(Language::Markdown)
                .hotkey("pa")
                .editor_hotkey("pb")
                .min_rows(2)
                .max_rows(4)
                .style(InputChrome::panel("Markdown").top_right("$EDITOR · .md")),
            textarea_disabled: TextareaInput::new()
                .value("Disabled textarea\nNavigation still works")
                .hotkey("d")
                .min_rows(2)
                .max_rows(2)
                .disabled(true)
                .style(InputChrome::panel("Disabled")),
            date_picker: DatePicker::new()
                .today(demo_date())
                .value(Some(demo_date()))
                .hotkey("dp"),
            time_picker: TimePicker::new()
                .value(demo_time())
                .minute_step(15)
                .precision(TimePrecision::HourMinute)
                .hotkey("tp")
                .style(InputChrome::panel("Time").top_right("15m")),
            date_time_picker: DateTimePicker::new()
                .value(Some(demo_datetime()))
                .layout(DateTimePickerLayout::Stepped),
            date_dropdown: DatePickerDropdown::new()
                .today(demo_date())
                .value(Some(demo_date()))
                .hotkey("dd")
                .panel("Date"),
            date_time_dropdown: DateTimePickerDropdown::new()
                .today(demo_date())
                .value(Some(demo_datetime()))
                .hotkey("dt")
                .panel("Date & time"),
            date_time_status: String::from("Pickers seeded to 2026-06-22 09:30"),
            relative_date: RelativeDateDemo::new(),
            calendar: demo_calendar(),
            calendar_borderless: demo_calendar().bordered(false),
            calendar_status: String::from("No calendar event yet"),
            status_bar: StatusBar::new()
                .menu_items([
                    StatusBarMenuItem::Theme,
                    StatusBarMenuItem::WeatherForecast,
                ])
                .weather_report(demo_weather_report())
                .weather_provider(WeatherProviderConfig::new().enabled(false))
                .on_ai_open(|| Msg::OpenAiDock)
                .on_store_view_open(|| Msg::StoreViewOpened),
            button: Button::new("Enabled").hotkey("b"),
            disabled_button: Button::new("Disabled").hotkey("d").disabled(true),
            button_presses: 0,
            chips: [
                Chip::new("Chip value"),
                Chip::new("Prepend").prepend_icon(""),
                Chip::new("Append").append_icon(""),
                Chip::new("Both icons").prepend_icon("󰄬").append_icon(""),
                Chip::new("Success")
                    .prepend_icon("")
                    .color_role(ChipColorRole::Success),
                Chip::new("Warning")
                    .prepend_icon("")
                    .color_role(ChipColorRole::Warning),
                Chip::new("Error")
                    .prepend_icon("")
                    .color_role(ChipColorRole::Error),
            ],
            tag_input: demo_tag_input(),
            tag_input_panel: demo_tag_input_panel(),
            toggle: Toggle::new("Telemetry").hotkey("x"),
            checkbox_toggle: Toggle::new("Item").checkbox().checked(true).hotkey("i"),
            dialog_100: dialog_button(DialogExample::Full),
            dialog_80: dialog_button(DialogExample::Large),
            dialog_60: dialog_button(DialogExample::Medium),
            dialog_40: dialog_button(DialogExample::Small),
            dialog_20: dialog_button(DialogExample::Tiny),
            dialog_top: dialog_button(DialogExample::Top),
            speed_reader_buttons: speed_reader_buttons(),
            confirmation: confirmation_button(),
            confirmation_status: String::from("No confirmation result yet"),
            dock_top: dock_overlay_button(DockOverlayExample::Top),
            dock_bottom: dock_overlay_button(DockOverlayExample::Bottom),
            dock_left: dock_overlay_button(DockOverlayExample::Left),
            dock_right: dock_overlay_button(DockOverlayExample::Right),
            dock_snackbar: dock_overlay_button(DockOverlayExample::BottomSnackbar),
            spinner: Spinner::new(),
            seasonal_empty_state: SeasonalEmptyState::new("No records for this filter"),
            notification_triggers: ToastRack::new(),
            notification_buttons: notification_buttons(),
            panel_demo: panel_demo(),
            panel_join_demo: panel_join_demo(),
            panel_tabs_join_demo: panel_tabs_join_demo(),
            panel_one_row_demo: Panel::new()
                .one_row(true)
                .top_left("One-row panel")
                .content(["Only the top border is rendered; content uses full width."]),
            panel_actions_demo: Panel::new()
                .top_left("Panel action hotkeys")
                .hotkey("pf")
                .action_hotkey("r", || Msg::PanelAction("refresh"))
                .action_hotkey("x", || Msg::PanelAction("clear"))
                .content(["Focus |pf| · refresh |r| · clear |x|"]),
            panel_action_status: String::from("No panel action yet"),
            tabs_minimal: tabs_demo(TabsVariant::Minimal).hotkey("m"),
            tabs_underline: tabs_demo(TabsVariant::Underline).hotkey("ma"),
            tabs_boxed: tabs_demo(TabsVariant::Boxed)
                .hotkey("mam")
                .action_hotkey("tr", |selected| Msg::TabsAction("refresh", selected))
                .action_hotkey("tc", |selected| Msg::TabsAction("clear", selected)),
            tabs_one_row: tabs_demo(TabsVariant::OneRow).hotkey("mar"),
            tabs_action_status: String::from("No tabs action yet"),
            tabs_modal_buttons: [
                modal_tabs_button(ModalTabsExample::CenterMinimal),
                modal_tabs_button(ModalTabsExample::CenterUnderline),
                modal_tabs_button(ModalTabsExample::CenterBoxed),
                modal_tabs_button(ModalTabsExample::Top),
                modal_tabs_button(ModalTabsExample::Bottom),
                modal_tabs_button(ModalTabsExample::Left),
                modal_tabs_button(ModalTabsExample::Right),
                modal_tabs_button(ModalTabsExample::BottomSnackbar),
            ],
            data_list: DataViewMode::List.data_view(),
            data_table: DataViewMode::Table.data_view(),
            data_list_tree: DataViewMode::ListTree.data_view(),
            data_table_tree: DataViewMode::TableTree.data_view(),
            data_single_select: DataViewMode::SingleSelect.data_view(),
            data_multi_select: DataViewMode::MultiSelect.data_view(),
            data_checklist_tree: DataViewMode::ChecklistTree.data_view(),
            data_activate_on_navigate: DataViewMode::ActivateOnNavigate.data_view(),
            data_jira_tickets: DataViewMode::JiraTickets.data_view(),
            data_status: String::from("No event yet"),
            list_compact: compact_names(),
            list_entity_table: entity_table(),
            list_reorder: reorder_mode(),
            list_reorder_tree: reorder_mode_tree(),
            checklist: release_checklist(),
            panel_top_left: panel_title_dropdown(PanelTitlePosition::TopLeft),
            panel_top_right: panel_title_dropdown(PanelTitlePosition::TopRight),
            panel_bottom_left: panel_title_dropdown(PanelTitlePosition::BottomLeft),
            panel_bottom_right: panel_title_dropdown(PanelTitlePosition::BottomRight),
            dropdown_fuzzy_single: dropdown_fuzzy_single(),
            dropdown_multi_contains: dropdown_multi_contains(),
            dropdown_no_search_immediate: dropdown_no_search_immediate(),
            dropdown_filled_fuzzy_single: dropdown_filled_fuzzy_single(),
            dropdown_filled_multi_contains: dropdown_filled_multi_contains(),
            dropdown_filled_searchable_none_immediate: dropdown_filled_searchable_none_immediate(),
            dropdown_disabled: dropdown_disabled(),
            menu_button: demo_menu_button(),
            menu_status: String::from("No menu action yet"),
            layout_flex: layout_flex_demo(),
            layout_split: layout_split_demo(),
            layout_stack: layout_stack_demo(),
            layout_layered: layout_layered_demo(),
            layout_grid: layout_grid_demo(),
            scroll_mixed: scroll_mixed_demo(),
            scroll_data_views: scroll_data_views_demo(),
            validated_form: ValidatedForm::new(),
            diff_side_by_side: side_by_side_diff_demo(),
            diff_inline: inline_diff_demo(),
            syntax_highlighting: gallery_demo::syntax_highlighting::SyntaxHighlightingDemo::new(),
            diff_word: word_diff_demo(),
            diff_raw_patch: raw_patch_diff_demo(),
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
            PreviewKind::PanelVariants => self.layout_panel_variants_preview(area, ctx),
            PreviewKind::PanelJoinedSeparators => self.layout_panel_join_preview(area, ctx),
            PreviewKind::PanelTabSeparators => self.layout_panel_tabs_join_preview(area, ctx),
            PreviewKind::Dialog => self.layout_dialog(area, ctx),
            PreviewKind::SpeedReader => {
                for (index, button_area) in speed_reader_button_areas(area).into_iter().enumerate()
                {
                    ctx.push_slot(speed_reader_child_key(index), button_area, |ctx| {
                        self.speed_reader_buttons[index].layout(button_area, ctx);
                    });
                }
            }
            PreviewKind::NotificationTriggers => self.layout_notification_triggers(area, ctx),
            PreviewKind::Button => self.layout_button(area, ctx),
            PreviewKind::Toggle => self.layout_toggle(area, ctx),
            PreviewKind::TextInput => {
                let [_, input, panel, numbers, disabled] = text_input_showcase_layout(area);
                ctx.push_slot(text_input_child_key(), input, |ctx| {
                    self.text_input.layout(input, ctx);
                });
                ctx.push_slot(text_input_panel_child_key(), panel, |ctx| {
                    self.text_input_panel.layout(panel, ctx);
                });
                ctx.push_slot(text_input_numbers_child_key(), numbers, |ctx| {
                    self.text_input_numbers.layout(numbers, ctx);
                });
                ctx.push_slot(text_input_disabled_child_key(), disabled, |ctx| {
                    self.text_input_disabled.layout(disabled, ctx);
                });
            }
            PreviewKind::PasswordInput => {
                let [_, input, panel, _] = password_input_showcase_layout(area);
                ctx.push_slot(password_input_child_key(), input, |ctx| {
                    self.password_input.layout(input, ctx);
                });
                ctx.push_slot(password_panel_child_key(), panel, |ctx| {
                    self.password_panel.layout(panel, ctx);
                });
            }
            PreviewKind::Typography => {
                self.typography.layout(area, ctx);
            }
            PreviewKind::TextareaInput => {
                let [_, input, panel, disabled] = textarea_showcase_layout(area);
                ctx.push_slot(textarea_input_child_key(), input, |ctx| {
                    self.textarea_input.layout(input, ctx);
                });
                ctx.push_slot(textarea_panel_child_key(), panel, |ctx| {
                    self.textarea_panel.layout(panel, ctx);
                });
                ctx.push_slot(textarea_disabled_child_key(), disabled, |ctx| {
                    self.textarea_disabled.layout(disabled, ctx);
                });
            }
            PreviewKind::DateTimePicker => self.layout_date_time(area, ctx),
            PreviewKind::RelativeDate => {
                self.relative_date.layout(area, ctx);
            }
            PreviewKind::ValidatedForm => {
                self.validated_form.layout(area, ctx);
            }
            PreviewKind::Calendar => self.layout_calendar(area, ctx),
            PreviewKind::SeasonalEmptyState => {
                <SeasonalEmptyState as TuiNode<Msg>>::layout(
                    &mut self.seasonal_empty_state,
                    area,
                    ctx,
                );
            }
            PreviewKind::TagInput => self.layout_tag_input(area, ctx),
            PreviewKind::StatusBar => self.layout_status_bar(area, overlay_bounds, ctx),
            PreviewKind::DataList
            | PreviewKind::DataTable
            | PreviewKind::DataListTree
            | PreviewKind::DataTableTree
            | PreviewKind::DataSingleSelect
            | PreviewKind::DataMultiSelect
            | PreviewKind::DataChecklistTree
            | PreviewKind::DataActivateOnNavigate
            | PreviewKind::DataJiraTickets => {
                let [_, body] = data_view_layout(area);
                <DataView<DemoRow, usize> as TuiNode<Msg>>::layout(
                    self.active_data_view_mut(preview),
                    body,
                    ctx,
                );
            }
            preview if preview.is_list_control() => {
                ctx.with_overlay_bounds(overlay_bounds, |ctx| {
                    self.active_list_control_mut(preview).layout(area, ctx);
                });
            }
            PreviewKind::Checklist => {
                ctx.with_overlay_bounds(overlay_bounds, |ctx| {
                    self.checklist.layout(area, ctx);
                });
            }
            PreviewKind::Dropdown => self.layout_dropdowns(area, overlay_bounds, ctx),
            PreviewKind::Menu => self.layout_menu(area, overlay_bounds, ctx),
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
                self.layout_layered.layout(layout_demo_body(area), ctx);
            }
            PreviewKind::LayoutGrid => {
                self.layout_grid.layout(layout_demo_body(area), ctx);
            }
            PreviewKind::LayoutScrollMixed => {
                self.scroll_mixed.layout(area, ctx);
            }
            PreviewKind::LayoutScrollDataViews => {
                self.scroll_data_views.layout(area, ctx);
            }
            PreviewKind::SyntaxHighlighting => {
                self.syntax_highlighting.layout(area, ctx);
            }
            preview if preview.is_diff_viewer() => {
                self.active_diff_viewer_mut(preview).layout(area, ctx);
            }
            _ => {}
        }
    }

    fn handle_form_navigation(
        &self,
        preview: PreviewKind,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<Msg>,
    ) -> bool {
        let Some(direction) = form_navigation_key(event) else {
            return false;
        };
        let Some(position) = self.form_navigation_position(preview, route) else {
            return false;
        };
        if position.mode_active {
            return false;
        }

        match direction {
            FormNavigation::Previous if position.index > 0 => ctx.focus_previous(),
            FormNavigation::Next if position.index + 1 < position.count => ctx.focus_next(),
            FormNavigation::Previous | FormNavigation::Next => {}
        }
        ctx.stop_propagation();
        true
    }

    fn form_navigation_position(
        &self,
        preview: PreviewKind,
        route: &EventRoute,
    ) -> Option<FormNavigationPosition> {
        let key = route.path.first()?;
        let (index, count, mode_active) = match preview {
            PreviewKind::TextInput => {
                let keys = [
                    text_input_child_key(),
                    text_input_panel_child_key(),
                    text_input_numbers_child_key(),
                    text_input_disabled_child_key(),
                ];
                let index = child_position(key, &keys)?;
                let active = match index {
                    0 => self.text_input.insert_mode(),
                    1 => self.text_input_panel.insert_mode(),
                    2 => self.text_input_numbers.insert_mode(),
                    _ => self.text_input_disabled.insert_mode(),
                };
                (index, keys.len(), active)
            }
            PreviewKind::PasswordInput => {
                let keys = [password_input_child_key(), password_panel_child_key()];
                let index = child_position(key, &keys)?;
                let active = if index == 0 {
                    self.password_input.insert_mode()
                } else {
                    self.password_panel.insert_mode()
                };
                (index, keys.len(), active)
            }
            PreviewKind::TextareaInput => {
                let keys = [
                    textarea_input_child_key(),
                    textarea_panel_child_key(),
                    textarea_disabled_child_key(),
                ];
                let index = child_position(key, &keys)?;
                let active = match index {
                    0 => self.textarea_input.insert_mode(),
                    1 => self.textarea_panel.insert_mode(),
                    _ => self.textarea_disabled.insert_mode(),
                };
                (index, keys.len(), active)
            }
            PreviewKind::DateTimePicker => {
                let keys = [
                    date_picker_child_key(),
                    time_picker_child_key(),
                    date_time_picker_child_key(),
                    date_dropdown_child_key(),
                    date_time_dropdown_child_key(),
                ];
                let index = child_position(key, &keys)?;
                let active = match index {
                    3 => self.date_dropdown.is_open(),
                    4 => self.date_time_dropdown.is_open(),
                    _ => false,
                };
                (index, keys.len(), active)
            }
            PreviewKind::RelativeDate => {
                let index = self.relative_date.picker_position(key)?;
                (index, 4, false)
            }
            PreviewKind::TagInput => {
                let keys = [tag_input_child_key(), tag_input_panel_child_key()];
                let index = child_position(key, &keys)?;
                let active = if index == 0 {
                    self.tag_input.is_active()
                } else {
                    self.tag_input_panel.is_active()
                };
                (index, keys.len(), active)
            }
            PreviewKind::Toggle => {
                let keys = [toggle_switch_child_key(), toggle_checkbox_child_key()];
                let index = child_position(key, &keys)?;
                (index, keys.len(), false)
            }
            PreviewKind::Dropdown => {
                let index = dropdown_index(key)?;
                (index, 7, self.dropdown(index).is_open())
            }
            _ => return None,
        };
        Some(FormNavigationPosition {
            index,
            count,
            mode_active,
        })
    }

    fn render<'a>(
        &'a self,
        preview: PreviewKind,
        frame: &mut Frame,
        area: Rect,
        ctx: &mut RenderCtx<'a>,
    ) {
        match preview {
            PreviewKind::Tabs => self.render_tabs(frame, area, ctx),
            PreviewKind::Panel => self.render_panel_preview(frame, area, ctx),
            PreviewKind::PanelVariants => self.render_panel_variants_preview(frame, area, ctx),
            PreviewKind::PanelJoinedSeparators => self.render_panel_join_preview(frame, area, ctx),
            PreviewKind::PanelTabSeparators => {
                self.render_panel_tabs_join_preview(frame, area, ctx)
            }
            PreviewKind::Dialog => self.render_dialog(frame, area),
            PreviewKind::SpeedReader => self.render_speed_reader(frame, area),
            PreviewKind::Spinner => self.render_spinner(frame, area),
            PreviewKind::SeasonalEmptyState => <SeasonalEmptyState as TuiNode<Msg>>::render(
                &self.seasonal_empty_state,
                frame,
                area,
                ctx,
            ),
            PreviewKind::NotificationTriggers => self.render_notification_triggers(frame, area),
            PreviewKind::TextInput => self.render_text_input(frame, area),
            PreviewKind::PasswordInput => self.render_password_input(frame, area),
            PreviewKind::Typography => self.render_typography(frame, area, ctx),
            PreviewKind::Colors => self.render_colors(frame, area),
            PreviewKind::TextareaInput => self.render_textarea_input(frame, area),
            PreviewKind::DateTimePicker => self.render_date_time(frame, area, ctx),
            PreviewKind::RelativeDate => self.relative_date.render(frame, area, ctx),
            PreviewKind::ValidatedForm => self.validated_form.render(frame, area, ctx),
            PreviewKind::Calendar => self.render_calendar(frame, area, ctx),
            PreviewKind::StatusBar => self.render_status_bar(frame, area, ctx),
            PreviewKind::Button => self.render_button(frame, area),
            PreviewKind::Chip => self.render_chips(frame, area),
            PreviewKind::TagInput => self.render_tag_input(frame, area, ctx),
            PreviewKind::Toggle => self.render_toggle(frame, area),
            PreviewKind::DataList
            | PreviewKind::DataTable
            | PreviewKind::DataListTree
            | PreviewKind::DataTableTree
            | PreviewKind::DataSingleSelect
            | PreviewKind::DataMultiSelect
            | PreviewKind::DataChecklistTree
            | PreviewKind::DataActivateOnNavigate
            | PreviewKind::DataJiraTickets => self.render_data_view(preview, frame, area),
            preview @ (PreviewKind::ListCompact
            | PreviewKind::ListEntityTable
            | PreviewKind::ListReorder
            | PreviewKind::ListReorderTree) => {
                self.active_list_control(preview).render(frame, area, ctx);
            }
            PreviewKind::Checklist => self.checklist.render(frame, area, ctx),
            PreviewKind::Dropdown => self.render_dropdown_preview(frame, area, ctx),
            PreviewKind::Menu => self.render_menu(frame, area, ctx),
            PreviewKind::LayoutFlex => self.render_layout_flex(frame, area, ctx),
            PreviewKind::LayoutSplit => self.render_layout_split(frame, area, ctx),
            PreviewKind::LayoutStack => self.render_layout_stack(frame, area, ctx),
            PreviewKind::LayoutOverlay => self.render_layout_layered(frame, area, ctx),
            PreviewKind::LayoutGrid => self.render_layout_grid(frame, area, ctx),
            PreviewKind::LayoutScrollMixed => self.scroll_mixed.render(frame, area, ctx),
            PreviewKind::LayoutScrollDataViews => self.scroll_data_views.render(frame, area, ctx),
            PreviewKind::SyntaxHighlighting => {
                self.syntax_highlighting.render(frame, area, ctx);
            }
            preview @ (PreviewKind::DiffSideBySide
            | PreviewKind::DiffInline
            | PreviewKind::DiffWord
            | PreviewKind::DiffRawPatch) => {
                self.active_diff_viewer(preview).render(frame, area, ctx)
            }
        }
    }

    fn data_view_dispatch_event(
        &mut self,
        preview: PreviewKind,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<Msg>,
    ) -> EventOutcome {
        if !preview.is_data_view() {
            return EventOutcome::Ignored;
        }

        let outcome = self
            .active_data_view_mut(preview)
            .dispatch_event(route, event, ctx);
        self.record_data_events(preview);
        if outcome == EventOutcome::Handled {
            return outcome;
        }

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

        EventOutcome::Ignored
    }

    fn dispatch_event(
        &mut self,
        preview: PreviewKind,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<Msg>,
    ) -> EventOutcome {
        if self.handle_form_navigation(preview, route, event, ctx) {
            return EventOutcome::Handled;
        }
        if preview == PreviewKind::LayoutScrollMixed {
            return self.scroll_mixed.dispatch_event(route, event, ctx);
        }
        if preview == PreviewKind::LayoutScrollDataViews {
            return self.scroll_data_views.dispatch_event(route, event, ctx);
        }
        if preview == PreviewKind::TextInput {
            if let Some(route) = route
                .path
                .without_first_if(&text_input_child_key())
                .map(EventRoute::new)
            {
                return self.text_input.dispatch_event(&route, event, ctx);
            }
            if let Some(route) = route
                .path
                .without_first_if(&text_input_panel_child_key())
                .map(EventRoute::new)
            {
                return self.text_input_panel.dispatch_event(&route, event, ctx);
            }
            if let Some(route) = route
                .path
                .without_first_if(&text_input_numbers_child_key())
                .map(EventRoute::new)
            {
                return self.text_input_numbers.dispatch_event(&route, event, ctx);
            }
            let Some(route) = route
                .path
                .without_first_if(&text_input_disabled_child_key())
                .map(EventRoute::new)
            else {
                return EventOutcome::Ignored;
            };
            return self.text_input_disabled.dispatch_event(&route, event, ctx);
        }
        if preview == PreviewKind::PasswordInput {
            if let Some(route) = route
                .path
                .without_first_if(&password_input_child_key())
                .map(EventRoute::new)
            {
                return self.password_input.dispatch_event(&route, event, ctx);
            }
            let Some(route) = route
                .path
                .without_first_if(&password_panel_child_key())
                .map(EventRoute::new)
            else {
                return EventOutcome::Ignored;
            };
            return self.password_panel.dispatch_event(&route, event, ctx);
        }
        if preview == PreviewKind::TextareaInput {
            if let Some(route) = route
                .path
                .without_first_if(&textarea_input_child_key())
                .map(EventRoute::new)
            {
                return self.textarea_input.dispatch_event(&route, event, ctx);
            }
            if let Some(route) = route
                .path
                .without_first_if(&textarea_panel_child_key())
                .map(EventRoute::new)
            {
                return self.textarea_panel.dispatch_event(&route, event, ctx);
            }
            let Some(route) = route
                .path
                .without_first_if(&textarea_disabled_child_key())
                .map(EventRoute::new)
            else {
                return EventOutcome::Ignored;
            };
            return self.textarea_disabled.dispatch_event(&route, event, ctx);
        }
        if preview == PreviewKind::DateTimePicker {
            return self.date_time_dispatch_event(route, event, ctx);
        }
        if preview == PreviewKind::RelativeDate {
            return self.relative_date.dispatch_event(route, event, ctx);
        }
        if preview == PreviewKind::ValidatedForm {
            return self.validated_form.dispatch_event(route, event, ctx);
        }
        if preview == PreviewKind::Calendar {
            return self.calendar_dispatch_event(route, event, ctx);
        }
        if preview == PreviewKind::StatusBar {
            return self.status_bar.dispatch_event(route, event, ctx);
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
        if preview == PreviewKind::TagInput {
            if let Some(route) = route
                .path
                .without_first_if(&tag_input_child_key())
                .map(EventRoute::new)
            {
                return self.tag_input.dispatch_event(&route, event, ctx);
            }
            let Some(route) = route
                .path
                .without_first_if(&tag_input_panel_child_key())
                .map(EventRoute::new)
            else {
                return EventOutcome::Ignored;
            };
            return self.tag_input_panel.dispatch_event(&route, event, ctx);
        }
        if preview == PreviewKind::Menu {
            return self.menu_dispatch_event(route, event, ctx);
        }
        if preview.is_data_view() {
            return self.data_view_dispatch_event(preview, route, event, ctx);
        }
        if preview.is_list_control() {
            return self
                .active_list_control_mut(preview)
                .dispatch_event(route, event, ctx);
        }
        if preview == PreviewKind::Checklist {
            return self.checklist.dispatch_event(route, event, ctx);
        }
        if preview == PreviewKind::SyntaxHighlighting {
            return self.syntax_highlighting.dispatch_event(route, event, ctx);
        }
        if preview.is_diff_viewer() {
            return self
                .active_diff_viewer_mut(preview)
                .dispatch_event(route, event, ctx);
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
        if preview == PreviewKind::PanelVariants {
            let Some((index, route)) = panel_variant_child_route(route) else {
                return EventOutcome::Ignored;
            };
            return match index {
                0 => self.panel_one_row_demo.dispatch_event(&route, event, ctx),
                _ => self.panel_actions_demo.dispatch_event(&route, event, ctx),
            };
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
        if preview == PreviewKind::SpeedReader {
            let Some((index, route)) = speed_reader_route(route) else {
                return EventOutcome::Ignored;
            };
            return self.speed_reader_buttons[index].dispatch_event(&route, event, ctx);
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
                if dispatch_focus_child(
                    &mut self.text_input_panel,
                    target,
                    text_input_panel_child_key(),
                    focused,
                    ctx,
                ) {
                    return;
                }
                if dispatch_focus_child(
                    &mut self.text_input_numbers,
                    target,
                    text_input_numbers_child_key(),
                    focused,
                    ctx,
                ) {
                    return;
                }
                dispatch_focus_child(
                    &mut self.text_input_disabled,
                    target,
                    text_input_disabled_child_key(),
                    focused,
                    ctx,
                );
            }
            PreviewKind::PasswordInput => {
                if dispatch_focus_child(
                    &mut self.password_input,
                    target,
                    password_input_child_key(),
                    focused,
                    ctx,
                ) {
                    return;
                }
                dispatch_focus_child(
                    &mut self.password_panel,
                    target,
                    password_panel_child_key(),
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
                if dispatch_focus_child(
                    &mut self.textarea_panel,
                    target,
                    textarea_panel_child_key(),
                    focused,
                    ctx,
                ) {
                    return;
                }
                dispatch_focus_child(
                    &mut self.textarea_disabled,
                    target,
                    textarea_disabled_child_key(),
                    focused,
                    ctx,
                );
            }
            PreviewKind::DateTimePicker => {
                if dispatch_focus_child(
                    &mut self.date_picker,
                    target,
                    date_picker_child_key(),
                    focused,
                    ctx,
                ) {
                    return;
                }
                if dispatch_focus_child(
                    &mut self.time_picker,
                    target,
                    time_picker_child_key(),
                    focused,
                    ctx,
                ) {
                    return;
                }
                if dispatch_focus_child(
                    &mut self.date_time_picker,
                    target,
                    date_time_picker_child_key(),
                    focused,
                    ctx,
                ) {
                    return;
                }
                if dispatch_focus_child(
                    &mut self.date_dropdown,
                    target,
                    date_dropdown_child_key(),
                    focused,
                    ctx,
                ) {
                    return;
                }
                dispatch_focus_child(
                    &mut self.date_time_dropdown,
                    target,
                    date_time_dropdown_child_key(),
                    focused,
                    ctx,
                );
            }
            PreviewKind::RelativeDate => {
                self.relative_date.dispatch_focus(target, focused, ctx);
            }
            PreviewKind::ValidatedForm => {
                self.validated_form.dispatch_focus(target, focused, ctx);
            }
            PreviewKind::Calendar => {
                if dispatch_focus_child(
                    &mut self.calendar,
                    target,
                    calendar_child_key(),
                    focused,
                    ctx,
                ) {
                    return;
                }
                dispatch_focus_child(
                    &mut self.calendar_borderless,
                    target,
                    calendar_borderless_child_key(),
                    focused,
                    ctx,
                );
            }
            PreviewKind::StatusBar => self.status_bar.dispatch_focus(target, focused, ctx),
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
            PreviewKind::SpeedReader => {
                dispatch_focus_indexed(
                    target,
                    speed_reader_index,
                    |state, index| &mut state.speed_reader_buttons[index],
                    self,
                    focused,
                    ctx,
                );
            }
            PreviewKind::Button => self.button.dispatch_focus(target, focused, ctx),
            PreviewKind::TagInput => {
                if dispatch_focus_child(
                    &mut self.tag_input,
                    target,
                    tag_input_child_key(),
                    focused,
                    ctx,
                ) {
                    return;
                }
                dispatch_focus_child(
                    &mut self.tag_input_panel,
                    target,
                    tag_input_panel_child_key(),
                    focused,
                    ctx,
                );
            }
            PreviewKind::Menu => self.menu_dispatch_focus(target, focused, ctx),
            PreviewKind::LayoutScrollMixed => {
                self.scroll_mixed.dispatch_focus(target, focused, ctx)
            }
            PreviewKind::LayoutScrollDataViews => {
                self.scroll_data_views.dispatch_focus(target, focused, ctx)
            }
            preview if preview.is_data_view() => self
                .active_data_view_mut(preview)
                .dispatch_focus(target, focused, ctx),
            preview if preview.is_list_control() => self
                .active_list_control_mut(preview)
                .dispatch_focus(target, focused, ctx),
            PreviewKind::Checklist => self.checklist.dispatch_focus(target, focused, ctx),
            PreviewKind::SyntaxHighlighting => self
                .syntax_highlighting
                .dispatch_focus(target, focused, ctx),
            preview if preview.is_diff_viewer() => self
                .active_diff_viewer_mut(preview)
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
            PreviewKind::PanelVariants => {
                dispatch_focus_indexed(
                    target,
                    panel_variant_index,
                    |state, index| match index {
                        0 => &mut state.panel_one_row_demo,
                        _ => &mut state.panel_actions_demo,
                    },
                    self,
                    focused,
                    ctx,
                );
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
            .merge(<SeasonalEmptyState as TuiNode<Msg>>::tick(
                &mut self.seasonal_empty_state,
                dt,
                settings,
            ))
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
            .merge(
                <Calendar<DemoCalendarEntry, &'static str, Msg> as TuiNode<Msg>>::tick(
                    &mut self.calendar,
                    dt,
                    settings,
                ),
            )
            .merge(
                <Calendar<DemoCalendarEntry, &'static str, Msg> as TuiNode<Msg>>::tick(
                    &mut self.calendar_borderless,
                    dt,
                    settings,
                ),
            )
            .merge(<DatePicker<Msg> as TuiNode<Msg>>::tick(
                &mut self.date_picker,
                dt,
                settings,
            ))
            .merge(<DateTimePicker<Msg> as TuiNode<Msg>>::tick(
                &mut self.date_time_picker,
                dt,
                settings,
            ))
            .merge(<DatePickerDropdown<Msg> as TuiNode<Msg>>::tick(
                &mut self.date_dropdown,
                dt,
                settings,
            ))
            .merge(<DateTimePickerDropdown<Msg> as TuiNode<Msg>>::tick(
                &mut self.date_time_dropdown,
                dt,
                settings,
            ))
            .merge(<StatusBar<Msg> as TuiNode<Msg>>::tick(
                &mut self.status_bar,
                dt,
                settings,
            ))
            .merge(Animated::tick(&mut self.dialog_100, dt, settings))
            .merge(Animated::tick(&mut self.dialog_80, dt, settings))
            .merge(Animated::tick(&mut self.dialog_60, dt, settings))
            .merge(Animated::tick(&mut self.dialog_40, dt, settings))
            .merge(Animated::tick(&mut self.dialog_20, dt, settings))
            .merge(Animated::tick(&mut self.dialog_top, dt, settings))
            .merge(Animated::tick(
                &mut self.speed_reader_buttons[0],
                dt,
                settings,
            ))
            .merge(Animated::tick(
                &mut self.speed_reader_buttons[1],
                dt,
                settings,
            ))
            .merge(Animated::tick(&mut self.confirmation, dt, settings))
            .merge(Animated::tick(&mut self.dock_top, dt, settings))
            .merge(Animated::tick(&mut self.dock_bottom, dt, settings))
            .merge(Animated::tick(&mut self.dock_left, dt, settings))
            .merge(Animated::tick(&mut self.dock_right, dt, settings))
            .merge(Animated::tick(&mut self.dock_snackbar, dt, settings))
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
            .merge(<Tabs<Msg> as TuiNode<Msg>>::tick(
                &mut self.tabs_one_row,
                dt,
                settings,
            ))
            .merge(Animated::tick(
                &mut self.tabs_modal_buttons[0],
                dt,
                settings,
            ))
            .merge(Animated::tick(
                &mut self.tabs_modal_buttons[1],
                dt,
                settings,
            ))
            .merge(Animated::tick(
                &mut self.tabs_modal_buttons[2],
                dt,
                settings,
            ))
            .merge(Animated::tick(
                &mut self.tabs_modal_buttons[3],
                dt,
                settings,
            ))
            .merge(Animated::tick(
                &mut self.tabs_modal_buttons[4],
                dt,
                settings,
            ))
            .merge(Animated::tick(
                &mut self.tabs_modal_buttons[5],
                dt,
                settings,
            ))
            .merge(Animated::tick(
                &mut self.tabs_modal_buttons[6],
                dt,
                settings,
            ))
            .merge(Animated::tick(
                &mut self.tabs_modal_buttons[7],
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
            .merge(self.list_compact.tick(dt, settings))
            .merge(self.list_entity_table.tick(dt, settings))
            .merge(self.list_reorder.tick(dt, settings))
            .merge(self.list_reorder_tree.tick(dt, settings))
            .merge(self.checklist.tick(dt, settings))
            .merge(Animated::tick(&mut self.panel_demo, dt, settings))
            .merge(self.panel_join_demo.tick(dt, settings))
            .merge(self.panel_tabs_join_demo.tick(dt, settings))
            .merge(Animated::tick(&mut self.panel_one_row_demo, dt, settings))
            .merge(Animated::tick(&mut self.panel_actions_demo, dt, settings))
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
                &mut self.dropdown_filled_searchable_none_immediate,
                dt,
                settings,
            ))
            .merge(Animated::tick(&mut self.dropdown_disabled, dt, settings))
            .merge(self.menu_button.tick(dt, settings))
            .merge(Animated::tick(&mut self.text_input, dt, settings))
            .merge(Animated::tick(&mut self.text_input_panel, dt, settings))
            .merge(Animated::tick(&mut self.text_input_numbers, dt, settings))
            .merge(Animated::tick(&mut self.text_input_disabled, dt, settings))
            .merge(Animated::tick(&mut self.password_input, dt, settings))
            .merge(Animated::tick(&mut self.password_panel, dt, settings))
            .merge(Animated::tick(&mut self.textarea_input, dt, settings))
            .merge(Animated::tick(&mut self.textarea_panel, dt, settings))
            .merge(Animated::tick(&mut self.textarea_disabled, dt, settings))
            .merge(self.relative_date.tick(dt, settings))
            .merge(self.validated_form.tick(dt, settings))
            .merge(self.diff_side_by_side.tick(dt, settings))
            .merge(self.diff_inline.tick(dt, settings))
            .merge(TuiNode::<Msg>::tick(
                &mut self.syntax_highlighting,
                dt,
                settings,
            ))
            .merge(self.diff_word.tick(dt, settings))
            .merge(self.diff_raw_patch.tick(dt, settings))
            .merge(self.scroll_mixed.tick(dt, settings))
            .merge(self.scroll_data_views.tick(dt, settings))
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        self.relative_date.init(ctx);
        self.validated_form.init(ctx);
        self.list_compact.init(ctx);
        self.list_entity_table.init(ctx);
        self.list_reorder.init(ctx);
        self.list_reorder_tree.init(ctx);
        self.checklist.init(ctx);
        self.menu_button.init(ctx);
        self.diff_side_by_side.init(ctx);
        self.diff_inline.init(ctx);
        TuiNode::<Msg>::init(&mut self.syntax_highlighting, ctx);
        self.diff_word.init(ctx);
        self.diff_raw_patch.init(ctx);
        self.scroll_mixed.init(ctx);
        self.scroll_data_views.init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        self.relative_date.mount(ctx);
        self.validated_form.mount(ctx);
        self.list_compact.mount(ctx);
        self.list_entity_table.mount(ctx);
        self.list_reorder.mount(ctx);
        self.list_reorder_tree.mount(ctx);
        self.checklist.mount(ctx);
        self.menu_button.mount(ctx);
        self.diff_side_by_side.mount(ctx);
        self.diff_inline.mount(ctx);
        TuiNode::<Msg>::mount(&mut self.syntax_highlighting, ctx);
        self.diff_word.mount(ctx);
        self.diff_raw_patch.mount(ctx);
        self.scroll_mixed.mount(ctx);
        self.scroll_data_views.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        self.diff_raw_patch.unmount(ctx);
        self.diff_word.unmount(ctx);
        self.scroll_data_views.unmount(ctx);
        self.scroll_mixed.unmount(ctx);
        self.diff_inline.unmount(ctx);
        TuiNode::<Msg>::unmount(&mut self.syntax_highlighting, ctx);
        self.diff_side_by_side.unmount(ctx);
        self.menu_button.unmount(ctx);
        self.list_reorder_tree.unmount(ctx);
        self.list_reorder.unmount(ctx);
        self.checklist.unmount(ctx);
        self.list_entity_table.unmount(ctx);
        self.list_compact.unmount(ctx);
        self.validated_form.unmount(ctx);
        self.relative_date.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        self.diff_raw_patch.destroy(ctx);
        self.diff_word.destroy(ctx);
        self.scroll_data_views.destroy(ctx);
        self.scroll_mixed.destroy(ctx);
        self.diff_inline.destroy(ctx);
        TuiNode::<Msg>::destroy(&mut self.syntax_highlighting, ctx);
        self.diff_side_by_side.destroy(ctx);
        self.menu_button.destroy(ctx);
        self.list_reorder_tree.destroy(ctx);
        self.list_reorder.destroy(ctx);
        self.checklist.destroy(ctx);
        self.list_entity_table.destroy(ctx);
        self.list_compact.destroy(ctx);
        self.validated_form.destroy(ctx);
        self.relative_date.destroy(ctx);
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

    fn active_diff_viewer(&self, preview: PreviewKind) -> &DiffDemo<Msg> {
        match preview {
            PreviewKind::DiffSideBySide => &self.diff_side_by_side,
            PreviewKind::DiffInline => &self.diff_inline,
            PreviewKind::DiffWord => &self.diff_word,
            PreviewKind::DiffRawPatch => &self.diff_raw_patch,
            _ => unreachable!("active diff viewer requires a diff preview"),
        }
    }

    fn active_diff_viewer_mut(&mut self, preview: PreviewKind) -> &mut DiffDemo<Msg> {
        match preview {
            PreviewKind::DiffSideBySide => &mut self.diff_side_by_side,
            PreviewKind::DiffInline => &mut self.diff_inline,
            PreviewKind::DiffWord => &mut self.diff_word,
            PreviewKind::DiffRawPatch => &mut self.diff_raw_patch,
            _ => unreachable!("active diff viewer requires a diff preview"),
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
            3 => &mut self.tabs_one_row,
            _ => &mut self.tabs_minimal,
        }
    }

    fn modal_tabs_button_mut(&mut self, index: usize) -> &mut Button<Msg> {
        &mut self.tabs_modal_buttons[index.min(7)]
    }

    fn dropdown_mut(&mut self, index: usize) -> &mut Dropdown<DropdownDemoItem, &'static str> {
        match index {
            1 => &mut self.dropdown_multi_contains,
            2 => &mut self.dropdown_no_search_immediate,
            3 => &mut self.dropdown_filled_fuzzy_single,
            4 => &mut self.dropdown_filled_multi_contains,
            5 => &mut self.dropdown_filled_searchable_none_immediate,
            6 => &mut self.dropdown_disabled,
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
            PreviewKind::DataJiraTickets => &self.data_jira_tickets,
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
            PreviewKind::DataJiraTickets => &mut self.data_jira_tickets,
            _ => &mut self.data_list,
        }
    }

    fn active_list_control(&self, preview: PreviewKind) -> &ListControlShowcase<Msg> {
        match preview {
            PreviewKind::ListEntityTable => &self.list_entity_table,
            PreviewKind::ListReorder => &self.list_reorder,
            PreviewKind::ListReorderTree => &self.list_reorder_tree,
            _ => &self.list_compact,
        }
    }

    fn active_list_control_mut(&mut self, preview: PreviewKind) -> &mut ListControlShowcase<Msg> {
        match preview {
            PreviewKind::ListEntityTable => &mut self.list_entity_table,
            PreviewKind::ListReorder => &mut self.list_reorder,
            PreviewKind::ListReorderTree => &mut self.list_reorder_tree,
            _ => &mut self.list_compact,
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

    fn render_dropdown_preview<'a>(
        &'a self,
        frame: &mut Frame,
        area: Rect,
        ctx: &mut RenderCtx<'a>,
    ) {
        let [help, body] = dropdown_preview_layout(area);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw("1-6 focus demo • disabled 7 is read-only • Enter/Space opens • "),
                Span::raw("Ctrl+J/Ctrl+K navigate while typing search; Enter commit; Esc cancel; Space opens/toggles multi • "),
                Span::raw("Tab/BackTab or h/k/j/l moves across form demos • Ctrl+Q quits"),
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
                "selected: {:?}\nquery: {:?}\nType `no` to reveal and select --None--.",
                self.dropdown_filled_searchable_none_immediate.selected_id(),
                self.dropdown_filled_searchable_none_immediate
                    .search_query()
            ),
        );
        self.render_dropdown_column(
            frame,
            areas[6],
            6,
            "Disabled 7 • Read-only",
            "Muted dashed rounded chrome; focus, hotkey, search, and navigation work but selection stays locked.",
        );

        for (index, area) in areas.iter().copied().enumerate() {
            self.dropdown(index).render(frame, dropdown_area(area), ctx);
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
                "Open app-level dialogs or docked overlays with backdrop dim tween. Press x or Esc to close.",
            ),
            Rect::new(area.x, area.y, area.width, 2.min(area.height)),
        );
        let body = dialog_body_area(area);
        for (index, button_area) in dialog_button_areas(body).into_iter().enumerate() {
            self.dialog_button(index).render(frame, button_area);
        }
        frame.render_widget(
            Paragraph::new(self.confirmation_status.as_str()),
            Rect::new(area.x, area.y.saturating_add(1), area.width, 1),
        );
    }

    fn render_speed_reader(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(
            Paragraph::new(
                "RSVP reader with plain-text and Markdown modes. Both open paused in a centered, dimmed modal.",
            ),
            Rect::new(area.x, area.y, area.width, 2.min(area.height)),
        );
        for (index, button_area) in speed_reader_button_areas(area).into_iter().enumerate() {
            self.speed_reader_buttons[index].render(frame, button_area);
        }
    }

    fn dialog_button(&self, index: usize) -> &Button<Msg> {
        match index {
            1 => &self.dialog_80,
            2 => &self.dialog_60,
            3 => &self.dialog_40,
            4 => &self.dialog_20,
            5 => &self.dialog_top,
            6 => &self.dock_top,
            7 => &self.dock_bottom,
            8 => &self.dock_left,
            9 => &self.dock_right,
            10 => &self.dock_snackbar,
            11 => &self.confirmation,
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
            6 => &mut self.dock_top,
            7 => &mut self.dock_bottom,
            8 => &mut self.dock_left,
            9 => &mut self.dock_right,
            10 => &mut self.dock_snackbar,
            11 => &mut self.confirmation,
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
            ctx.with_overlay_bounds(overlay_bounds, |ctx| {
                <Dropdown<DropdownDemoItem, &'static str> as TuiNode<Msg>>::layout(
                    &mut self.dropdown_fuzzy_single,
                    areas[0],
                    ctx,
                );
            });
        });
        ctx.push_slot(dropdown_child_key(1), areas[1], |ctx| {
            ctx.with_overlay_bounds(overlay_bounds, |ctx| {
                <Dropdown<DropdownDemoItem, &'static str> as TuiNode<Msg>>::layout(
                    &mut self.dropdown_multi_contains,
                    areas[1],
                    ctx,
                );
            });
        });
        ctx.push_slot(dropdown_child_key(2), areas[2], |ctx| {
            ctx.with_overlay_bounds(overlay_bounds, |ctx| {
                <Dropdown<DropdownDemoItem, &'static str> as TuiNode<Msg>>::layout(
                    &mut self.dropdown_no_search_immediate,
                    areas[2],
                    ctx,
                );
            });
        });
        ctx.push_slot(dropdown_child_key(3), areas[3], |ctx| {
            ctx.with_overlay_bounds(overlay_bounds, |ctx| {
                <Dropdown<DropdownDemoItem, &'static str> as TuiNode<Msg>>::layout(
                    &mut self.dropdown_filled_fuzzy_single,
                    areas[3],
                    ctx,
                );
            });
        });
        ctx.push_slot(dropdown_child_key(4), areas[4], |ctx| {
            ctx.with_overlay_bounds(overlay_bounds, |ctx| {
                <Dropdown<DropdownDemoItem, &'static str> as TuiNode<Msg>>::layout(
                    &mut self.dropdown_filled_multi_contains,
                    areas[4],
                    ctx,
                );
            });
        });
        ctx.push_slot(dropdown_child_key(5), areas[5], |ctx| {
            ctx.with_overlay_bounds(overlay_bounds, |ctx| {
                <Dropdown<DropdownDemoItem, &'static str> as TuiNode<Msg>>::layout(
                    &mut self.dropdown_filled_searchable_none_immediate,
                    areas[5],
                    ctx,
                );
            });
        });
        ctx.push_slot(dropdown_child_key(6), areas[6], |ctx| {
            ctx.with_overlay_bounds(overlay_bounds, |ctx| {
                <Dropdown<DropdownDemoItem, &'static str> as TuiNode<Msg>>::layout(
                    &mut self.dropdown_disabled,
                    areas[6],
                    ctx,
                );
            });
        });
    }

    fn dropdown(&self, index: usize) -> &Dropdown<DropdownDemoItem, &'static str> {
        match index {
            1 => &self.dropdown_multi_contains,
            2 => &self.dropdown_no_search_immediate,
            3 => &self.dropdown_filled_fuzzy_single,
            4 => &self.dropdown_filled_multi_contains,
            5 => &self.dropdown_filled_searchable_none_immediate,
            6 => &self.dropdown_disabled,
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
            <Panel<Msg> as TuiNode<Msg>>::layout(&mut self.panel_demo, panel_area, ctx);
        });

        ctx.push_slot(panel_title_child_key(0), areas[0], |ctx| {
            ctx.with_overlay_bounds(area, |ctx| {
                <Dropdown<PanelTitleChoice, &'static str> as TuiNode<Msg>>::layout(
                    &mut self.panel_top_left,
                    areas[0],
                    ctx,
                );
            });
        });
        ctx.push_slot(panel_title_child_key(1), areas[1], |ctx| {
            ctx.with_overlay_bounds(area, |ctx| {
                <Dropdown<PanelTitleChoice, &'static str> as TuiNode<Msg>>::layout(
                    &mut self.panel_top_right,
                    areas[1],
                    ctx,
                );
            });
        });
        ctx.push_slot(panel_title_child_key(2), areas[2], |ctx| {
            ctx.with_overlay_bounds(area, |ctx| {
                <Dropdown<PanelTitleChoice, &'static str> as TuiNode<Msg>>::layout(
                    &mut self.panel_bottom_left,
                    areas[2],
                    ctx,
                );
            });
        });
        ctx.push_slot(panel_title_child_key(3), areas[3], |ctx| {
            ctx.with_overlay_bounds(area, |ctx| {
                <Dropdown<PanelTitleChoice, &'static str> as TuiNode<Msg>>::layout(
                    &mut self.panel_bottom_right,
                    areas[3],
                    ctx,
                );
            });
        });
    }

    fn layout_panel_variants_preview(&mut self, area: Rect, ctx: &mut LayoutCtx) {
        let [_, one_row, actions, _] = panel_variants_layout(area);
        ctx.push_slot(panel_variant_child_key(0), one_row, |ctx| {
            self.panel_one_row_demo.layout(one_row, ctx);
        });
        ctx.push_slot(panel_variant_child_key(1), actions, |ctx| {
            self.panel_actions_demo.layout(actions, ctx);
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
        let [instructions, input, panel, numbers, disabled] = text_input_showcase_layout(area);
        frame.render_widget(
            Paragraph::new(
                "Type text. Enter/Ctrl+Enter submits. Tab inserts spaces. Esc/Ctrl+[ exits input mode, then returns to the list.\n\
                 Hotkeys: |ia| plain action, |ib| plain $EDITOR, |pa| panel action, |pb| panel $EDITOR, |na| numbers action, |nb| numbers $EDITOR, |d| disabled. Invalid number editor saves are discarded with a warning.\n\
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
        self.text_input_numbers.render(frame, numbers);
        self.text_input_disabled.render(frame, disabled);
    }

    fn render_password_input(&self, frame: &mut Frame, area: Rect) {
        let [instructions, input, panel, preview] = password_input_showcase_layout(area);
        frame.render_widget(
            Paragraph::new(
                "Type a secret. Enter/Ctrl+Enter submits. Tab inserts spaces. Esc/Ctrl+[ exits input mode, then returns to the list.\n\
                 Text is masked; current editing shortcuts match TextInput. When idle, h/k moves back and j/l forward. Nested panel has hotkey |pp|.\n\
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
        self.password_panel.render(frame, panel);
        frame.render_widget(
            Paragraph::new(format!(
                "Current value: {}",
                self.password_input.current_value()
            )),
            preview,
        );
    }

    fn render_typography<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        self.typography.render(frame, area, ctx);
    }

    fn render_colors(&self, frame: &mut Frame, area: Rect) {
        let theme = tuicore::theme();
        let colors = [
            ("selected_fg", theme.selected_fg()),
            ("selected_bg", theme.selected_bg()),
            ("background_bg", theme.background_bg()),
            ("surface_bg", theme.surface_bg()),
            ("backdrop_bg", theme.backdrop_bg()),
            ("text_fg", theme.text_fg()),
            ("muted_fg", theme.muted_fg()),
            ("subtle_fg", theme.subtle_fg()),
            ("accent_fg", theme.accent_fg()),
            ("success_fg", theme.success_fg()),
            ("error_fg", theme.error_fg()),
            ("border_fg", theme.border_fg()),
            ("highlight_fg", theme.highlight_fg()),
            ("highlight_bg", theme.highlight_bg()),
            ("key_fg", theme.key_fg()),
            ("warning_fg", theme.warning_fg()),
            ("weather_sun_fg", theme.weather_sun_fg()),
            ("weather_cool_fg", theme.weather_cool_fg()),
            ("weather_warm_fg", theme.weather_warm_fg()),
            ("weather_hot_fg", theme.weather_hot_fg()),
            ("weather_rain_fg", theme.weather_rain_fg()),
        ];
        let column_width = (area.width / 3).max(1);
        for (index, (name, color)) in colors.into_iter().enumerate() {
            let column = index % 3;
            let row = index / 3;
            let x = area.x + column as u16 * column_width;
            let y = area.y + row as u16 * 3;
            if y >= area.y.saturating_add(area.height) {
                break;
            }
            let swatch = Rect::new(
                x,
                y,
                3.min(column_width),
                3.min(area.bottom().saturating_sub(y)),
            );
            for offset in 0..swatch.height {
                frame.render_widget(
                    Paragraph::new("   ").style(Style::default().bg(color)),
                    Rect::new(swatch.x, swatch.y + offset, swatch.width, 1),
                );
            }
            if column_width > 5 {
                frame.render_widget(
                    Paragraph::new(name).style(Style::default().fg(theme.text_fg())),
                    Rect::new(x + 4, y + 1, column_width.saturating_sub(4), 1),
                );
            }
        }
    }

    fn render_textarea_input(&self, frame: &mut Frame, area: Rect) {
        let [instructions, input, panel, disabled] = textarea_showcase_layout(area);
        frame.render_widget(
            Paragraph::new(
                "Type text. Enter inserts newline. Ctrl+Enter finishes editing. Ctrl+J also inserts newline. Tab inserts spaces. Esc/Ctrl+[ returns to list. Ctrl+Q quits from gallery root.\n\
                 Plain textarea uses |ta| action and |tb| $EDITOR. Markdown panel uses |pa| action and |pb| $EDITOR with an .md temp file. Disabled panel |d| blocks editing. When idle, h/k moves back and j/l forward.\n\
                 Shortcuts:\n\
                 • PgUp / PgDn / Ctrl+U / Ctrl+D          : Scroll overflowing text\n\
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
        self.textarea_disabled.render(frame, disabled);
    }

    fn layout_date_time(&mut self, area: Rect, ctx: &mut LayoutCtx) {
        let [_, date_area, _, combo_area, dropdown_area, _] = date_time_showcase_layout(area);
        let date_picker_area = Rect::new(date_area.x, date_area.y, date_area.width.min(24), 10);
        let time_picker_area = Rect::new(date_area.x, date_area.y + 10, date_area.width.min(14), 3);
        let inline_picker_area = Rect::new(
            combo_area.x,
            combo_area.y.saturating_add(1),
            combo_area.width.min(24),
            combo_area.height.saturating_sub(1).min(10),
        );
        let [date_dropdown_area, date_time_dropdown_area] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(26), Constraint::Length(33)])
            .areas(dropdown_area);
        ctx.push_slot(date_picker_child_key(), date_picker_area, |ctx| {
            self.date_picker.layout(date_picker_area, ctx);
        });
        ctx.push_slot(time_picker_child_key(), time_picker_area, |ctx| {
            self.time_picker.layout(time_picker_area, ctx);
        });
        ctx.push_slot(date_time_picker_child_key(), inline_picker_area, |ctx| {
            <DateTimePicker<Msg> as TuiNode<Msg>>::layout(
                &mut self.date_time_picker,
                inline_picker_area,
                ctx,
            );
        });
        ctx.push_slot(date_dropdown_child_key(), date_dropdown_area, |ctx| {
            self.date_dropdown.layout(date_dropdown_area, ctx);
        });
        ctx.push_slot(
            date_time_dropdown_child_key(),
            date_time_dropdown_area,
            |ctx| {
                self.date_time_dropdown.layout(date_time_dropdown_area, ctx);
            },
        );
    }

    fn render_date_time<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        let [
            instructions,
            date_area,
            status_area,
            combo_area,
            dropdown_area,
            help_area,
        ] = date_time_showcase_layout(area);
        frame.render_widget(
            Paragraph::new(
                "DatePicker, TimePicker, composed DateTimePicker, and dropdown DateTimePicker.\n\
                 Hotkeys: |dp| date, |tp| time, |dt| datetime dropdown. Date: arrows/hjkl move day/week, M month grid, Y year grid, T today, Ctrl+O $EDITOR, Enter select.\n\
                 Month/year selection: press Y, move with arrows/hjkl (PgUp/PgDn changes 24-year page), then Enter; choose a month with arrows/hjkl (PgUp/PgDn changes year), then Enter. Press M to open the month grid directly.\n\
                 Time: left/right or h/l selects field; up/down or k/j increments. Stepped inline picker uses Enter for date → time → commit; Escape from time returns to date. Tab/BackTab or Ctrl+h/k/j/l moves between form fields.",
            ),
            instructions,
        );
        let date_picker_area = Rect::new(date_area.x, date_area.y, date_area.width.min(24), 10);
        let time_picker_area = Rect::new(date_area.x, date_area.y + 10, date_area.width.min(14), 3);
        let inline_picker_area = Rect::new(
            combo_area.x,
            combo_area.y.saturating_add(1),
            combo_area.width.min(24),
            combo_area.height.saturating_sub(1).min(10),
        );
        let [date_dropdown_area, date_time_dropdown_area] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(26), Constraint::Length(33)])
            .areas(dropdown_area);
        self.date_picker.render(frame, date_picker_area);
        self.time_picker.render(frame, time_picker_area);
        frame.render_widget(Paragraph::new(self.date_time_status.clone()), status_area);
        frame.render_widget(
            Paragraph::new("Stepped inline DateTimePicker (date → time)"),
            Rect::new(combo_area.x, combo_area.y, combo_area.width.min(24), 1),
        );
        self.date_time_picker.render(frame, inline_picker_area);
        self.date_dropdown.render(frame, date_dropdown_area, ctx);
        self.date_time_dropdown
            .render(frame, date_time_dropdown_area, ctx);
        frame.render_widget(
            Paragraph::new(
                "Inline and dropdown datetime pickers show one step at a time. Enter advances or commits; Escape cancels time. Inline Ctrl+O edits the active step; closed dropdown Ctrl+O edits the full datetime.",
            ),
            help_area,
        );
    }

    fn date_time_dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<Msg>,
    ) -> EventOutcome {
        let outcome = if let Some(route) = route
            .path
            .without_first_if(&date_picker_child_key())
            .map(EventRoute::new)
        {
            self.date_picker.dispatch_event(&route, event, ctx)
        } else if let Some(route) = route
            .path
            .without_first_if(&time_picker_child_key())
            .map(EventRoute::new)
        {
            self.time_picker.dispatch_event(&route, event, ctx)
        } else if let Some(route) = route
            .path
            .without_first_if(&date_time_picker_child_key())
            .map(EventRoute::new)
        {
            self.date_time_picker.dispatch_event(&route, event, ctx)
        } else if let Some(route) = route
            .path
            .without_first_if(&date_dropdown_child_key())
            .map(EventRoute::new)
        {
            self.date_dropdown.dispatch_event(&route, event, ctx)
        } else if let Some(route) = route
            .path
            .without_first_if(&date_time_dropdown_child_key())
            .map(EventRoute::new)
        {
            self.date_time_dropdown.dispatch_event(&route, event, ctx)
        } else {
            return EventOutcome::Ignored;
        };
        self.date_time_status = format!(
            "date: {} • time: {} • datetime: {} • dropdown: {}",
            format_date_option(self.date_picker.current_value()),
            format_time(self.time_picker.current_value()),
            format_datetime_option(self.date_time_picker.current_value()),
            format_datetime_option(self.date_time_dropdown.current_value())
        );
        outcome
    }

    fn layout_calendar(&mut self, area: Rect, ctx: &mut LayoutCtx) {
        let [_, bordered_area, borderless_area, _] = calendar_preview_layout(area);
        ctx.push_slot(calendar_child_key(), bordered_area, |ctx| {
            <Calendar<DemoCalendarEntry, &'static str, Msg> as TuiNode<Msg>>::layout(
                &mut self.calendar,
                bordered_area,
                ctx,
            );
        });
        ctx.push_slot(calendar_borderless_child_key(), borderless_area, |ctx| {
            <Calendar<DemoCalendarEntry, &'static str, Msg> as TuiNode<Msg>>::layout(
                &mut self.calendar_borderless,
                borderless_area,
                ctx,
            );
        });
    }

    fn render_calendar<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        let [help, bordered_area, borderless_area, status] = calendar_preview_layout(area);
        frame.render_widget(
            Paragraph::new(
                "Bordered (top) and borderless (bottom): M/W/D switches views, T jumps today, arrows/hjkl navigate, Ctrl+U/D pages day entries, Enter drills down, Esc/Ctrl+[ goes back.",
            ),
            help,
        );
        <Calendar<DemoCalendarEntry, &'static str, Msg> as TuiNode<Msg>>::render(
            &self.calendar,
            frame,
            bordered_area,
            ctx,
        );
        <Calendar<DemoCalendarEntry, &'static str, Msg> as TuiNode<Msg>>::render(
            &self.calendar_borderless,
            frame,
            borderless_area,
            ctx,
        );
        frame.render_widget(Paragraph::new(self.calendar_status.clone()), status);
    }

    fn calendar_dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<Msg>,
    ) -> EventOutcome {
        let outcome = if let Some(route) = route
            .path
            .without_first_if(&calendar_child_key())
            .map(EventRoute::new)
        {
            self.calendar.dispatch_event(&route, event, ctx)
        } else if let Some(route) = route
            .path
            .without_first_if(&calendar_borderless_child_key())
            .map(EventRoute::new)
        {
            self.calendar_borderless.dispatch_event(&route, event, ctx)
        } else {
            return EventOutcome::Ignored;
        };
        self.record_calendar_events();
        outcome
    }

    fn record_calendar_events(&mut self) {
        let statuses = self
            .calendar
            .take_events()
            .into_iter()
            .chain(self.calendar_borderless.take_events())
            .map(calendar_event_status)
            .collect::<Vec<_>>();
        if !statuses.is_empty() {
            self.calendar_status = statuses.join(" • ");
        }
    }

    fn layout_status_bar(&mut self, area: Rect, overlay_bounds: Rect, ctx: &mut LayoutCtx) {
        let [_, bar, _] = status_bar_preview_layout(area);
        ctx.with_overlay_bounds(overlay_bounds, |ctx| {
            <StatusBar<Msg> as TuiNode<Msg>>::layout(&mut self.status_bar, bar, ctx);
        });
    }

    fn render_status_bar<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        let [help, bar, note] = status_bar_preview_layout(area);
        frame.render_widget(
            Paragraph::new("Reusable status bar: ; opens the menu, Theme opens a centered dropdown, ' opens AI dock. Weather and time sit on the right."),
            help,
        );
        self.status_bar.render(frame, bar, ctx);
        frame.render_widget(
            Paragraph::new("Menu contents are configured with StatusBar::menu_items([...])."),
            note,
        );
    }

    fn layout_button(&mut self, area: Rect, ctx: &mut LayoutCtx) {
        let [_, button_row, _] = button_layout(area);
        let [button_area, disabled_area] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(10), Constraint::Length(12)])
            .areas(button_row);
        self.button.layout(button_area, ctx);
        self.disabled_button.layout(disabled_area, ctx);
    }

    fn render_button(&self, frame: &mut Frame, area: Rect) {
        let [instructions, button_row, status] = button_layout(area);
        let [button_area, disabled_area] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(10), Constraint::Length(12)])
            .areas(button_row);
        frame.render_widget(
            Paragraph::new(format!(
                "Enabled and disabled styles. {} presses; b focuses and presses the enabled button.",
                tuicore::keybindings().button().press_label()
            )),
            instructions,
        );
        self.button.render(frame, button_area);
        self.disabled_button.render(frame, disabled_area);
        frame.render_widget(
            Paragraph::new(format!("Pressed {} times", self.button_presses)),
            status,
        );
    }

    fn render_chips(&self, frame: &mut Frame, area: Rect) {
        let [instructions, chips_area, _] = chip_layout(area);
        frame.render_widget(
            Paragraph::new(
                "Chip uses Nerd Font rounded powerline caps: Chip value. Icons keep one space between icon and label.",
            ),
            instructions,
        );

        let [first_row, second_row] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .areas(chips_area);
        let first_row_areas = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(14),
                Constraint::Length(13),
                Constraint::Length(13),
                Constraint::Length(18),
            ])
            .areas::<4>(first_row);
        let second_row_areas = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(14),
                Constraint::Length(14),
                Constraint::Length(12),
            ])
            .areas::<3>(second_row);
        for (chip, area) in self.chips[..4].iter().zip(first_row_areas) {
            chip.render(frame, area);
        }
        for (chip, area) in self.chips[4..].iter().zip(second_row_areas) {
            chip.render(frame, area);
        }
    }

    fn layout_tag_input(&mut self, area: Rect, ctx: &mut LayoutCtx) {
        let (_, input_area, panel_area, _) = self.tag_input_showcase_areas(area);
        ctx.push_slot(tag_input_child_key(), input_area, |ctx| {
            <TagInput as TuiNode<Msg>>::layout(&mut self.tag_input, input_area, ctx);
        });
        ctx.push_slot(tag_input_panel_child_key(), panel_area, |ctx| {
            <TagInput as TuiNode<Msg>>::layout(&mut self.tag_input_panel, panel_area, ctx);
        });
    }

    fn render_tag_input<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        let (help, input_area, panel_area, footer) = self.tag_input_showcase_areas(area);
        frame.render_widget(
            Paragraph::new(
                "TagInput: plain and panel-outline variants. Type filters existing tags in an overlay popup. Enter adds highlighted existing tag; Ctrl+Enter requests/creates exact text. Ctrl+j/k moves popup, Ctrl+h/l moves selected tags. When idle, Ctrl+h/k moves back and Ctrl+j/l forward between fields.",
            ),
            help,
        );
        self.tag_input.render(frame, input_area, ctx);
        self.tag_input_panel.render(frame, panel_area, ctx);
        frame.render_widget(
            Paragraph::new(Text::from(vec![
                Line::from("Content directly below TagInput — open either dropdown and it should cover this text."),
                Line::from("Filter to 'do' to see the popup shrink to the matching rows."),
                Line::from("Add most existing tags, then open again to see the empty state row."),
                Line::from("Hotkeys: |tag| jumps to plain input, |pt| jumps to panel outline."),
            ])),
            footer,
        );
    }

    fn tag_input_showcase_areas(&self, area: Rect) -> (Rect, Rect, Rect, Rect) {
        let help_height = 4;
        let input_height = self.tag_input.height_for_width(area.width);
        let panel_height = self.tag_input_panel.height_for_width(area.width);
        let help = Rect::new(area.x, area.y, area.width, help_height.min(area.height));
        let input_y = area.y.saturating_add(help.height);
        let input = Rect::new(
            area.x,
            input_y,
            area.width,
            input_height.min(area.bottom().saturating_sub(input_y)),
        );
        let panel_y = input.y.saturating_add(input.height).saturating_add(1);
        let panel = Rect::new(
            area.x,
            panel_y,
            area.width,
            panel_height.min(area.bottom().saturating_sub(panel_y)),
        );
        let footer_y = panel.y.saturating_add(panel.height).saturating_add(1);
        let footer = Rect::new(
            area.x,
            footer_y,
            area.width,
            area.bottom().saturating_sub(footer_y),
        );
        (help, input, panel, footer)
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

    fn layout_menu(&mut self, area: Rect, overlay_bounds: Rect, ctx: &mut LayoutCtx) {
        let [_, trigger_row, _] = menu_preview_layout(area);
        let trigger_area = Rect::new(trigger_row.x, trigger_row.y, trigger_row.width.min(15), 1);
        ctx.with_overlay_bounds(overlay_bounds, |ctx| {
            self.menu_button.layout(trigger_area, ctx);
        });
    }

    fn render_menu<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        let [help, trigger_row, status] = menu_preview_layout(area);
        let trigger_area = Rect::new(trigger_row.x, trigger_row.y, trigger_row.width.min(15), 1);
        frame.render_widget(
            Paragraph::new(
                "MenuButton owns its trigger + popup; Menu remains available for custom triggers. Open focuses search; Enter activates, Esc closes, Ctrl+j/k/d/u navigate.",
            ),
            help,
        );
        self.menu_button.render(frame, trigger_area, ctx);
        frame.render_widget(Paragraph::new(self.menu_status.clone()), status);
    }

    fn menu_dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<Msg>,
    ) -> EventOutcome {
        let outcome = self.menu_button.dispatch_event(route, event, ctx);
        for id in self.menu_button.take_activated() {
            self.menu_status = format!("Activated {id}");
        }
        outcome
    }

    fn menu_dispatch_focus(
        &mut self,
        target: &FocusTarget,
        focused: bool,
        ctx: &mut FocusCtx<Msg>,
    ) {
        self.menu_button.dispatch_focus(target, focused, ctx);
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
                "Enter/Space toggles. Press x for switch style or i for checkbox style. h/k moves back; j/l moves forward.",
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

    fn render_panel_preview<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
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
        self.render_panel_title_control(frame, areas[0], 0, "Top left", ctx);
        self.render_panel_title_control(frame, areas[1], 1, "Top right", ctx);
        self.render_panel_title_control(frame, areas[2], 2, "Bottom left", ctx);
        self.render_panel_title_control(frame, areas[3], 3, "Bottom right", ctx);
    }

    fn render_panel_variants_preview<'a>(
        &'a self,
        frame: &mut Frame,
        area: Rect,
        _ctx: &mut RenderCtx<'a>,
    ) {
        let [help, one_row, actions, status] = panel_variants_layout(area);
        frame.render_widget(
            Paragraph::new("One-row chrome and multiple bottom-right action hotkeys."),
            help,
        );
        self.panel_one_row_demo.render(frame, one_row);
        self.panel_actions_demo.render(frame, actions);
        frame.render_widget(Paragraph::new(self.panel_action_status.as_str()), status);
    }

    fn render_panel_join_preview<'a>(
        &'a self,
        frame: &mut Frame,
        area: Rect,
        ctx: &mut RenderCtx<'a>,
    ) {
        let [help, body] = panel_separator_preview_layout(area);
        frame.render_widget(
            Paragraph::new("Split/Flex/Grid separators share one Separator model. PanelHost patches edge contacts into join glyphs."),
            help,
        );
        self.panel_join_demo.render(frame, body, ctx);
    }

    fn render_panel_tabs_join_preview<'a>(
        &'a self,
        frame: &mut Frame,
        area: Rect,
        ctx: &mut RenderCtx<'a>,
    ) {
        let [help, body] = panel_separator_preview_layout(area);
        frame.render_widget(
            Paragraph::new("Tabs can host split/nested separators. Focus lands on Tabs/body components; separator glyphs are not focus targets."),
            help,
        );
        self.panel_tabs_join_demo.render(frame, body, ctx);
    }

    fn render_panel_title_control<'a>(
        &'a self,
        frame: &mut Frame,
        area: Rect,
        index: usize,
        title: &str,
        ctx: &mut RenderCtx<'a>,
    ) {
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
        self.panel_title_dropdown(index).render(frame, field, ctx);
    }

    fn layout_tabs(&mut self, area: Rect, ctx: &mut LayoutCtx) {
        let [buttons_area, demos_area] = modal_tabs_preview_layout(area);
        let button_areas = modal_tabs_button_areas(buttons_area);
        for (index, button_area) in button_areas.into_iter().enumerate() {
            ctx.push_slot(modal_tabs_open_child_key(index), button_area, |ctx| {
                self.tabs_modal_buttons[index].layout(button_area, ctx);
            });
        }
        let [minimal, underline, boxed, one_row] = tabs_areas(demos_area);
        let [_, minimal_tabs] = labeled_area(minimal);
        let [_, underline_tabs] = labeled_area(underline);
        let [_, boxed_tabs] = labeled_area(boxed);
        let [_, one_row_tabs] = labeled_area(one_row);
        ctx.push_slot(tab_demo_child_key(0), minimal_tabs, |ctx| {
            self.tabs_minimal.layout(minimal_tabs, ctx);
        });
        ctx.push_slot(tab_demo_child_key(1), underline_tabs, |ctx| {
            self.tabs_underline.layout(underline_tabs, ctx);
        });
        ctx.push_slot(tab_demo_child_key(2), boxed_tabs, |ctx| {
            self.tabs_boxed.layout(boxed_tabs, ctx);
        });
        ctx.push_slot(tab_demo_child_key(3), one_row_tabs, |ctx| {
            self.tabs_one_row.layout(one_row_tabs, ctx);
        });
    }

    fn render_tabs<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        let [buttons_area, demos_area] = modal_tabs_preview_layout(area);
        let button_areas = modal_tabs_button_areas(buttons_area);
        for (index, button_area) in button_areas.into_iter().enumerate() {
            self.tabs_modal_buttons[index].render(frame, button_area);
        }
        let [minimal, underline, boxed, one_row] = tabs_areas(demos_area);
        let [minimal_label, minimal_tabs] = labeled_area(minimal);
        let [underline_label, underline_tabs] = labeled_area(underline);
        let [boxed_label, boxed_tabs] = labeled_area(boxed);
        let [one_row_label, one_row_tabs] = labeled_area(one_row);

        frame.render_widget(Paragraph::new("Style 1: minimal (m)"), minimal_label);
        self.tabs_minimal.render(frame, minimal_tabs, ctx);
        frame.render_widget(Paragraph::new("Style 2: underline (l)"), underline_label);
        self.tabs_underline.render(frame, underline_tabs, ctx);
        frame.render_widget(
            Paragraph::new(format!(
                "Style 3: boxed actions (mam·tr·tc) — {}",
                self.tabs_action_status
            )),
            boxed_label,
        );
        self.tabs_boxed.render(frame, boxed_tabs, ctx);
        frame.render_widget(Paragraph::new("Style 4: one-row (mar)"), one_row_label);
        self.tabs_one_row.render(frame, one_row_tabs, ctx);
    }

    fn render_layout_flex<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        render_layout_intro(
            frame,
            area,
            "Flex: fixed, fit-content, fill, and CSS-like content shrink factors.",
        );
        self.layout_flex.render(frame, layout_demo_body(area), ctx);
    }

    fn render_layout_split<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        render_layout_intro(
            frame,
            area,
            "Split: two panes with ratio/content+fill style composition.",
        );
        self.layout_split.render(frame, layout_demo_body(area), ctx);
    }

    fn render_layout_stack<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        render_layout_intro(
            frame,
            area,
            "Stack: children share one area; later layers render on top with alignment/inset.",
        );
        self.layout_stack.render(frame, layout_demo_body(area), ctx);
    }

    fn render_layout_layered<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        render_layout_intro(
            frame,
            area,
            "Overlay: base gets normal flow; anchored layer floats without taking height.",
        );
        self.layout_layered
            .render(frame, layout_demo_body(area), ctx);
    }

    fn render_layout_grid<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        render_layout_intro(
            frame,
            area,
            "Grid: tracks mix fixed/fit/percent/fill with row gap 1, column gap 2, padding 1.",
        );
        self.layout_grid.render(frame, layout_demo_body(area), ctx);
    }
}

fn demo_menu_button() -> MenuButton<&'static str, Msg> {
    MenuButton::new(
        "Open menu",
        [
            MenuItem::new("new", "New file"),
            MenuItem::new("open", "Open recent"),
            MenuItem::new("rename", "Rename symbol"),
            MenuItem::new("format", "Format document"),
            MenuItem::new("command", "Run command"),
            MenuItem::new("settings", "Project settings"),
        ],
    )
    .visible_items(10)
    .hotkey("m")
    .hotkey_label_mode(HotkeyLabelMode::Inline)
}

fn menu_preview_layout(area: Rect) -> [Rect; 3] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .areas(area)
}

fn gallery_list_child_key() -> ChildKey {
    ChildKey::new("component-list")
}

fn gallery_preview_child_key() -> ChildKey {
    ChildKey::new("preview")
}

fn gallery_footer_child_key() -> ChildKey {
    ChildKey::new("footer")
}

fn text_input_child_key() -> ChildKey {
    ChildKey::new("text-input")
}

fn text_input_panel_child_key() -> ChildKey {
    ChildKey::new("text-input-panel")
}

fn text_input_numbers_child_key() -> ChildKey {
    ChildKey::new("text-input-numbers")
}

fn text_input_disabled_child_key() -> ChildKey {
    ChildKey::new("text-input-disabled")
}

fn password_input_child_key() -> ChildKey {
    ChildKey::new("password-input")
}

fn password_panel_child_key() -> ChildKey {
    ChildKey::new("password-panel")
}

fn textarea_input_child_key() -> ChildKey {
    ChildKey::new("textarea-input")
}

fn textarea_panel_child_key() -> ChildKey {
    ChildKey::new("textarea-panel")
}

fn textarea_disabled_child_key() -> ChildKey {
    ChildKey::new("textarea-disabled")
}

fn tag_input_child_key() -> ChildKey {
    ChildKey::new("tag-input")
}

fn tag_input_panel_child_key() -> ChildKey {
    ChildKey::new("tag-input-panel")
}

fn date_picker_child_key() -> ChildKey {
    ChildKey::new("date-picker")
}

fn time_picker_child_key() -> ChildKey {
    ChildKey::new("time-picker")
}

fn date_time_picker_child_key() -> ChildKey {
    ChildKey::new("date-time-picker")
}

fn date_dropdown_child_key() -> ChildKey {
    ChildKey::new("date-dropdown")
}

fn date_time_dropdown_child_key() -> ChildKey {
    ChildKey::new("date-time-dropdown")
}

fn calendar_child_key() -> ChildKey {
    ChildKey::new("calendar")
}

fn calendar_borderless_child_key() -> ChildKey {
    ChildKey::new("calendar-borderless")
}

fn status_bar_preview_layout(area: Rect) -> [Rect; 3] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .areas(area)
}

fn calendar_preview_layout(area: Rect) -> [Rect; 4] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Ratio(1, 2),
            Constraint::Ratio(1, 2),
            Constraint::Length(2),
        ])
        .areas(area)
}

fn demo_date() -> Date {
    Date::from_calendar_date(2026, Month::June, 22).expect("valid demo date")
}

fn demo_calendar() -> Calendar<DemoCalendarEntry, &'static str, Msg> {
    Calendar::new(
        demo_calendar_entries(),
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    )
    .cursor(demo_date())
    .role(|entry| entry.role)
    .render_entry(|entry| Line::from(entry.title))
    .render_detail(|entry| {
        Text::from(vec![
            Line::from(entry.title),
            Line::from(""),
            Line::from(entry.detail),
        ])
    })
}

fn demo_calendar_entries() -> Vec<DemoCalendarEntry> {
    vec![
        DemoCalendarEntry {
            id: "planning",
            title: "Sprint planning",
            span: CalendarSpan::timed(
                demo_date().with_time(Time::from_hms(9, 30, 0).expect("valid time")),
                demo_date().with_time(Time::from_hms(10, 30, 0).expect("valid time")),
            ),
            role: Some(CalendarEntryRole::Accent),
            detail: "Review backlog, capacity, and goal for sprint 42.",
        },
        DemoCalendarEntry {
            id: "lunch",
            title: "Design lunch",
            span: CalendarSpan::timed(
                demo_date().with_time(Time::from_hms(12, 0, 0).expect("valid time")),
                demo_date().with_time(Time::from_hms(13, 0, 0).expect("valid time")),
            ),
            role: Some(CalendarEntryRole::Success),
            detail: "Informal design review over lunch.",
        },
        DemoCalendarEntry {
            id: "release",
            title: "Release freeze",
            span: CalendarSpan::all_day(demo_date() + time::Duration::days(2)),
            role: Some(CalendarEntryRole::Warning),
            detail: "All-day release freeze marker.",
        },
        DemoCalendarEntry {
            id: "incident",
            title: "Incident retro",
            span: CalendarSpan::timed(
                (demo_date() + time::Duration::days(4))
                    .with_time(Time::from_hms(15, 0, 0).expect("valid time")),
                (demo_date() + time::Duration::days(4))
                    .with_time(Time::from_hms(16, 0, 0).expect("valid time")),
            ),
            role: Some(CalendarEntryRole::Error),
            detail: "Retrospective for failed deploy alert noise.",
        },
    ]
}

fn demo_tag_input() -> TagInput {
    TagInput::new(demo_labels())
        .placeholder("add tags")
        .hotkey("tag")
        .selected(["bug", "frontend", "urgent", "docs", "design", "qa"])
}

fn demo_tag_input_panel() -> TagInput {
    TagInput::new(demo_labels())
        .placeholder("panel tags")
        .hotkey("pt")
        .selected(["api", "blocked", "performance", "accessibility"])
        .style(InputChrome::panel("Tags").top_right("Panel style"))
}

fn demo_labels() -> [&'static str; 12] {
    [
        "bug",
        "frontend",
        "backend",
        "api",
        "urgent",
        "docs",
        "design",
        "qa",
        "blocked",
        "good first issue",
        "performance",
        "accessibility",
    ]
}

fn calendar_event_status(event: CalendarTypedEvent<&'static str>) -> String {
    match event {
        CalendarTypedEvent::ViewChanged { view } => format!("view {view:?}"),
        CalendarTypedEvent::RangeChanged { start, end } => format!("range {start}..{end}"),
        CalendarTypedEvent::CursorChanged { date } => format!("cursor {date}"),
        CalendarTypedEvent::DateActivated { date } => format!("activated {date}"),
        CalendarTypedEvent::EntryHighlighted { entry_id } => {
            format!("highlight {}", entry_id.unwrap_or("none"))
        }
        CalendarTypedEvent::EntryActivated { entry_id } => format!("activated {entry_id}"),
        CalendarTypedEvent::EntriesReordered { entry_ids } => {
            format!("reordered {entry_ids:?}")
        }
        CalendarTypedEvent::DrillDown { from, to } => format!("drill {from:?}->{to:?}"),
        CalendarTypedEvent::Back { from, to } => format!("back {from:?}->{to:?}"),
    }
}

fn demo_time() -> Time {
    Time::from_hms(9, 30, 0).expect("valid demo time")
}

fn demo_datetime() -> PrimitiveDateTime {
    demo_date().with_time(demo_time())
}

fn format_date_option(value: Option<Date>) -> String {
    value
        .map(|date| date.to_string())
        .unwrap_or_else(|| String::from("none"))
}

fn format_time(value: Time) -> String {
    format!("{:02}:{:02}", value.hour(), value.minute())
}

fn format_datetime_option(value: Option<PrimitiveDateTime>) -> String {
    value
        .map(|value| format!("{} {}", value.date(), format_time(value.time())))
        .unwrap_or_else(|| String::from("none"))
}

fn toggle_switch_child_key() -> ChildKey {
    ChildKey::new("toggle-switch")
}

fn toggle_checkbox_child_key() -> ChildKey {
    ChildKey::new("toggle-checkbox")
}

fn form_navigation_key(event: &TuiEvent) -> Option<FormNavigation> {
    let TuiEvent::Key(KeyEvent { code, modifiers }) = event else {
        return None;
    };
    if *modifiers != KeyModifiers::CONTROL {
        return None;
    }
    match code {
        Key::Char('h' | 'k') => Some(FormNavigation::Previous),
        Key::Char('j' | 'l') => Some(FormNavigation::Next),
        _ => None,
    }
}

fn child_position(key: &ChildKey, keys: &[ChildKey]) -> Option<usize> {
    keys.iter().position(|candidate| candidate == key)
}

fn modal_tabs_button(example: ModalTabsExample) -> Button<Msg> {
    Button::new(example.button_label())
        .hotkey(example.hotkey())
        .on_press(move || Msg::ModalTabsOpened(example))
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

fn scroll_mixed_demo() -> ScrollContainer<Flex<Msg>, Msg> {
    ScrollContainer::vertical(
        Flex::column()
            .gap(1)
            .child(
                "intro",
                Panel::new().top_left("One outer scrollbar").host(tuicore::Paragraph::new(
                    "Panels, inputs, buttons, and a DataView compose in one scroll viewport.\nFocus a control below the fold to reveal it.",
                )),
                tuicore::FlexItem::fit_content(),
            )
            .child(
                "input",
                TextInput::new().placeholder("Focus me"),
                tuicore::FlexItem::fit_content(),
            )
            .child(
                "button",
                Button::new("Action"),
                tuicore::FlexItem::fit_content(),
            )
            .child(
                "panel",
                Panel::new()
                    .top_left("Long panel")
                    .host(tuicore::Paragraph::new(
                        "A regular PanelHost participates in the outer page.\nIt owns chrome; ScrollContainer owns vertical scrolling.\nNo nested scrollbar appears here.",
                    )),
                tuicore::FlexItem::fit_content(),
            )
            .child(
                "table",
                DataViewMode::Table
                    .data_view()
                    .action_bar(false)
                    .parent_vertical_scroll(),
                tuicore::FlexItem::fit_content(),
            ),
    )
}

fn scroll_data_views_demo() -> ScrollContainer<Flex<Msg>, Msg> {
    ScrollContainer::vertical(
        Flex::column()
            .gap(1)
            .child(
                "sprint-5",
                delegated_tree_view("Sprint 5", 8, None),
                tuicore::FlexItem::fit_content(),
            )
            .child(
                "sprint-6",
                delegated_tree_view("Sprint 6", 8, None),
                tuicore::FlexItem::fit_content(),
            )
            .child(
                "sprint-7",
                delegated_tree_view("Sprint 7", 8, None),
                tuicore::FlexItem::fit_content(),
            )
            .child(
                "backlog",
                delegated_tree_view("Backlog", 120, Some("shift+b")),
                tuicore::FlexItem::fit_content(),
            ),
    )
}

fn delegated_tree_view(title: &str, rows: usize, hotkey: Option<&str>) -> DataView<DemoRow, usize> {
    let view = DataViewMode::ListTree
        .data_view()
        .expanded([])
        .selection_mode(SelectionMode::Multi)
        .selection_trigger(SelectionTrigger::OnActivate)
        .selection_glyphs(SelectionGlyphs::NERD_FONT)
        .parent_vertical_scroll();
    let view = if rows > 100 {
        DataViewMode::ListTree
            .data_view()
            .expanded([])
            .selection_mode(SelectionMode::Multi)
            .selection_trigger(SelectionTrigger::OnActivate)
            .selection_glyphs(SelectionGlyphs::NERD_FONT)
            .parent_vertical_scroll()
    } else {
        view
    };
    let view = view.hotkey(hotkey.unwrap_or(title));
    view
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
enum ComponentKind {
    Tabs,
    Panel,
    PanelVariants,
    PanelJoinedSeparators,
    PanelTabSeparators,
    Dialog,
    SpeedReader,
    Spinner,
    SeasonalEmptyState,
    Notifications,
    NotificationTriggers,
    Typography,
    Colors,
    Layouts,
    LayoutFlex,
    LayoutSplit,
    LayoutStack,
    LayoutOverlay,
    LayoutGrid,
    LayoutScrollMixed,
    LayoutScrollDataViews,
    Inputs,
    Button,
    Chip,
    TagInput,
    TextInput,
    PasswordInput,
    TextareaInput,
    DateTimePicker,
    RelativeDate,
    ValidatedForm,
    Calendar,
    Toggle,
    Dropdown,
    Menu,
    StatusBar,
    DataView,
    DataViewList,
    DataViewTable,
    DataViewListTree,
    DataViewTableTree,
    DataViewSingleSelect,
    DataViewMultiSelect,
    DataViewChecklistTree,
    DataViewActivateOnNavigate,
    DataViewJiraTickets,
    ListControl,
    ListCompact,
    ListEntityTable,
    ListReorder,
    ListReorderTree,
    Checklist,
    DiffViewer,
    DiffSideBySide,
    DiffInline,
    SyntaxHighlighting,
    DiffWord,
    DiffRawPatch,
}

impl ComponentKind {
    const ALL: [Self; 58] = [
        Self::Tabs,
        Self::Panel,
        Self::PanelVariants,
        Self::PanelJoinedSeparators,
        Self::PanelTabSeparators,
        Self::Dialog,
        Self::SpeedReader,
        Self::Spinner,
        Self::SeasonalEmptyState,
        Self::Notifications,
        Self::NotificationTriggers,
        Self::Typography,
        Self::Colors,
        Self::Layouts,
        Self::LayoutFlex,
        Self::LayoutSplit,
        Self::LayoutStack,
        Self::LayoutOverlay,
        Self::LayoutGrid,
        Self::LayoutScrollMixed,
        Self::LayoutScrollDataViews,
        Self::Inputs,
        Self::Button,
        Self::Chip,
        Self::TagInput,
        Self::TextInput,
        Self::PasswordInput,
        Self::TextareaInput,
        Self::DateTimePicker,
        Self::RelativeDate,
        Self::ValidatedForm,
        Self::Calendar,
        Self::Toggle,
        Self::Dropdown,
        Self::Menu,
        Self::StatusBar,
        Self::DataView,
        Self::DataViewList,
        Self::DataViewTable,
        Self::DataViewListTree,
        Self::DataViewTableTree,
        Self::DataViewSingleSelect,
        Self::DataViewMultiSelect,
        Self::DataViewChecklistTree,
        Self::DataViewActivateOnNavigate,
        Self::DataViewJiraTickets,
        Self::ListControl,
        Self::ListCompact,
        Self::ListEntityTable,
        Self::ListReorder,
        Self::ListReorderTree,
        Self::Checklist,
        Self::DiffViewer,
        Self::DiffSideBySide,
        Self::DiffInline,
        Self::SyntaxHighlighting,
        Self::DiffWord,
        Self::DiffRawPatch,
    ];

    fn title(self) -> &'static str {
        match self {
            Self::Tabs => "Tabs",
            Self::Panel => "Panels",
            Self::PanelVariants => "One-row + Actions",
            Self::PanelJoinedSeparators => "Joined Separators",
            Self::PanelTabSeparators => "Tabs + Separators",
            Self::Dialog => "Dialog",
            Self::SpeedReader => "Speed Reader",
            Self::Spinner => "Spinner",
            Self::SeasonalEmptyState => "Seasonal Empty State",
            Self::Notifications => "Notifications",
            Self::NotificationTriggers => "Triggers",
            Self::Typography => "Typography",
            Self::Colors => "Colors",
            Self::Layouts => "Layouts",
            Self::LayoutFlex => "Flex",
            Self::LayoutSplit => "Split",
            Self::LayoutStack => "Stack",
            Self::LayoutOverlay => "Overlay",
            Self::LayoutGrid => "Grid",
            Self::LayoutScrollMixed => "Scroll: Mixed content",
            Self::LayoutScrollDataViews => "Scroll: DataViews",
            Self::Inputs => "Inputs",
            Self::Button => "Button",
            Self::Chip => "Chip",
            Self::TagInput => "Tag Input",
            Self::TextInput => "Text",
            Self::PasswordInput => "Password",
            Self::TextareaInput => "Textarea",
            Self::DateTimePicker => "Date & Time",
            Self::RelativeDate => "Relative Date",
            Self::ValidatedForm => "Validated Form",
            Self::Calendar => "Calendar",
            Self::Toggle => "Toggle",
            Self::Dropdown => "Dropdown",
            Self::Menu => "Menu",
            Self::StatusBar => "Status Bar",
            Self::DataView => "DataView",
            Self::DataViewList => "List",
            Self::DataViewTable => "Table",
            Self::DataViewListTree => "List Tree",
            Self::DataViewTableTree => "Table Tree",
            Self::DataViewSingleSelect => "Single Select",
            Self::DataViewMultiSelect => "Multi Select",
            Self::DataViewChecklistTree => "Tree Checklist",
            Self::DataViewActivateOnNavigate => "Activate On Navigate",
            Self::DataViewJiraTickets => "Jira Tickets",
            Self::ListControl => "List Control",
            Self::ListCompact => "Compact names",
            Self::ListEntityTable => "Entity table",
            Self::ListReorder => "Reorder mode",
            Self::ListReorderTree => "Reorder mode tree",
            Self::Checklist => "Checklist",
            Self::DiffViewer => "Diff viewer",
            Self::DiffSideBySide => "Side-by-side / Split",
            Self::DiffInline => "Inline / Unified",
            Self::DiffWord => "Word / Intra-line",
            Self::DiffRawPatch => "Raw patch / Patch view",
            Self::SyntaxHighlighting => "Syntax Highlighting",
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
            | Self::DataViewActivateOnNavigate
            | Self::DataViewJiraTickets => Some(Self::DataView),
            Self::ListCompact
            | Self::ListEntityTable
            | Self::ListReorder
            | Self::ListReorderTree => Some(Self::ListControl),
            Self::Button
            | Self::Chip
            | Self::TagInput
            | Self::TextInput
            | Self::PasswordInput
            | Self::TextareaInput
            | Self::DateTimePicker
            | Self::RelativeDate
            | Self::ValidatedForm
            | Self::Calendar
            | Self::Toggle
            | Self::Dropdown
            | Self::Menu => Some(Self::Inputs),
            Self::LayoutFlex
            | Self::LayoutSplit
            | Self::LayoutStack
            | Self::LayoutOverlay
            | Self::LayoutGrid
            | Self::LayoutScrollMixed
            | Self::LayoutScrollDataViews => Some(Self::Layouts),
            Self::PanelVariants | Self::PanelJoinedSeparators | Self::PanelTabSeparators => {
                Some(Self::Panel)
            }
            Self::NotificationTriggers => Some(Self::Notifications),
            Self::DiffSideBySide | Self::DiffInline | Self::DiffWord | Self::DiffRawPatch => {
                Some(Self::DiffViewer)
            }
            _ => None,
        }
    }

    fn preview(self) -> PreviewKind {
        match self {
            Self::Tabs => PreviewKind::Tabs,
            Self::Panel => PreviewKind::Panel,
            Self::PanelVariants => PreviewKind::PanelVariants,
            Self::PanelJoinedSeparators => PreviewKind::PanelJoinedSeparators,
            Self::PanelTabSeparators => PreviewKind::PanelTabSeparators,
            Self::Dialog => PreviewKind::Dialog,
            Self::SpeedReader => PreviewKind::SpeedReader,
            Self::Spinner => PreviewKind::Spinner,
            Self::SeasonalEmptyState => PreviewKind::SeasonalEmptyState,
            Self::Notifications => PreviewKind::NotificationTriggers,
            Self::NotificationTriggers => PreviewKind::NotificationTriggers,
            Self::Typography => PreviewKind::Typography,
            Self::Colors => PreviewKind::Colors,
            Self::Layouts | Self::LayoutFlex => PreviewKind::LayoutFlex,
            Self::LayoutSplit => PreviewKind::LayoutSplit,
            Self::LayoutStack => PreviewKind::LayoutStack,
            Self::LayoutOverlay => PreviewKind::LayoutOverlay,
            Self::LayoutGrid => PreviewKind::LayoutGrid,
            Self::LayoutScrollMixed => PreviewKind::LayoutScrollMixed,
            Self::LayoutScrollDataViews => PreviewKind::LayoutScrollDataViews,
            Self::Inputs | Self::Button => PreviewKind::Button,
            Self::Chip => PreviewKind::Chip,
            Self::TagInput => PreviewKind::TagInput,
            Self::TextInput => PreviewKind::TextInput,
            Self::PasswordInput => PreviewKind::PasswordInput,
            Self::TextareaInput => PreviewKind::TextareaInput,
            Self::DateTimePicker => PreviewKind::DateTimePicker,
            Self::RelativeDate => PreviewKind::RelativeDate,
            Self::ValidatedForm => PreviewKind::ValidatedForm,
            Self::Calendar => PreviewKind::Calendar,
            Self::Toggle => PreviewKind::Toggle,
            Self::Dropdown => PreviewKind::Dropdown,
            Self::Menu => PreviewKind::Menu,
            Self::StatusBar => PreviewKind::StatusBar,
            Self::DataView | Self::DataViewList => PreviewKind::DataList,
            Self::DataViewTable => PreviewKind::DataTable,
            Self::DataViewListTree => PreviewKind::DataListTree,
            Self::DataViewTableTree => PreviewKind::DataTableTree,
            Self::DataViewSingleSelect => PreviewKind::DataSingleSelect,
            Self::DataViewMultiSelect => PreviewKind::DataMultiSelect,
            Self::DataViewChecklistTree => PreviewKind::DataChecklistTree,
            Self::DataViewActivateOnNavigate => PreviewKind::DataActivateOnNavigate,
            Self::DataViewJiraTickets => PreviewKind::DataJiraTickets,
            Self::ListControl | Self::ListCompact => PreviewKind::ListCompact,
            Self::ListEntityTable => PreviewKind::ListEntityTable,
            Self::ListReorder => PreviewKind::ListReorder,
            Self::ListReorderTree => PreviewKind::ListReorderTree,
            Self::Checklist => PreviewKind::Checklist,
            Self::DiffViewer | Self::DiffSideBySide => PreviewKind::DiffSideBySide,
            Self::DiffInline => PreviewKind::DiffInline,
            Self::SyntaxHighlighting => PreviewKind::SyntaxHighlighting,
            Self::DiffWord => PreviewKind::DiffWord,
            Self::DiffRawPatch => PreviewKind::DiffRawPatch,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewKind {
    Tabs,
    Panel,
    PanelVariants,
    PanelJoinedSeparators,
    PanelTabSeparators,
    Dialog,
    SpeedReader,
    Spinner,
    SeasonalEmptyState,
    NotificationTriggers,
    Typography,
    Colors,
    LayoutFlex,
    LayoutSplit,
    LayoutStack,
    LayoutOverlay,
    LayoutGrid,
    LayoutScrollMixed,
    LayoutScrollDataViews,
    TextInput,
    PasswordInput,
    TextareaInput,
    DateTimePicker,
    RelativeDate,
    ValidatedForm,
    Calendar,
    StatusBar,
    Button,
    Chip,
    TagInput,
    Toggle,
    Dropdown,
    Menu,
    DataList,
    DataTable,
    DataListTree,
    DataTableTree,
    DataSingleSelect,
    DataMultiSelect,
    DataChecklistTree,
    DataActivateOnNavigate,
    DataJiraTickets,
    ListCompact,
    ListEntityTable,
    ListReorder,
    ListReorderTree,
    Checklist,
    DiffSideBySide,
    DiffInline,
    SyntaxHighlighting,
    DiffWord,
    DiffRawPatch,
}

impl PreviewKind {
    fn title(self) -> &'static str {
        match self {
            Self::Tabs => "Tabs",
            Self::Panel => "Panels",
            Self::PanelVariants => "Panel One-row + Actions",
            Self::PanelJoinedSeparators => "Joined Separators",
            Self::PanelTabSeparators => "Tabs + Separators",
            Self::Dialog => "Dialog",
            Self::SpeedReader => "Speed Reader",
            Self::Spinner => "Spinner",
            Self::SeasonalEmptyState => "Seasonal Empty State",
            Self::NotificationTriggers => "Notification Triggers",
            Self::Typography => "Typography",
            Self::Colors => "Colors",
            Self::LayoutFlex => "Flex Layout",
            Self::LayoutSplit => "Split Layout",
            Self::LayoutStack => "Stack Layout",
            Self::LayoutOverlay => "Overlay Layout",
            Self::LayoutGrid => "Grid Layout",
            Self::LayoutScrollMixed => "Scroll Container: Mixed content",
            Self::LayoutScrollDataViews => "Scroll Container: DataViews",
            Self::TextInput => "Text",
            Self::PasswordInput => "Password",
            Self::TextareaInput => "Textarea",
            Self::DateTimePicker => "Date & Time",
            Self::RelativeDate => "Relative Date",
            Self::ValidatedForm => "Validated Form",
            Self::Calendar => "Calendar",
            Self::StatusBar => "Status Bar",
            Self::Button => "Button",
            Self::Chip => "Chip",
            Self::TagInput => "Tag Input",
            Self::Toggle => "Toggle",
            Self::Dropdown => "Dropdown",
            Self::Menu => "Menu",
            Self::DataList => "List",
            Self::DataTable => "Table",
            Self::DataListTree => "List Tree",
            Self::DataTableTree => "Table Tree",
            Self::DataSingleSelect => "Single Select",
            Self::DataMultiSelect => "Multi Select",
            Self::DataChecklistTree => "Tree Checklist",
            Self::DataActivateOnNavigate => "Activate On Navigate",
            Self::DataJiraTickets => "Jira Tickets",
            Self::ListCompact => "Compact names",
            Self::ListEntityTable => "Entity table",
            Self::ListReorder => "Reorder mode",
            Self::ListReorderTree => "Reorder mode tree",
            Self::Checklist => "Checklist",
            Self::DiffSideBySide => "Side-by-side / Split",
            Self::DiffInline => "Inline / Unified",
            Self::DiffWord => "Word / Intra-line",
            Self::DiffRawPatch => "Raw patch / Patch view",
            Self::SyntaxHighlighting => "Syntax Highlighting",
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
                | Self::DataJiraTickets
        )
    }

    fn is_list_control(self) -> bool {
        matches!(
            self,
            Self::ListCompact | Self::ListEntityTable | Self::ListReorder | Self::ListReorderTree
        )
    }

    fn is_diff_viewer(self) -> bool {
        matches!(
            self,
            Self::DiffSideBySide | Self::DiffInline | Self::DiffWord | Self::DiffRawPatch
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn gallery_root_state_stays_stack_safe() {
        let size = std::mem::size_of::<Gallery>();
        assert!(size < 64 * 1024, "Gallery is {size} bytes");
    }

    #[test]
    fn active_panel_gallery_action_hotkey_emits_message() {
        let mut state = PreviewState::new();
        let area = Rect::new(0, 0, 80, 24);
        let mut layout = LayoutCtx::new();
        state.layout(PreviewKind::PanelVariants, area, area, &mut layout);
        let target = layout
            .focus_targets()
            .iter()
            .find(|target| target.path.first() == Some(&panel_variant_child_key(1)))
            .expect("actions panel should register focus")
            .clone();
        state.dispatch_focus(
            PreviewKind::PanelVariants,
            &target,
            true,
            &mut FocusCtx::new(AnimationSettings::default()),
        );
        let route = EventRoute::new(target.path.clone());
        let mut ctx = EventCtx::default();

        let triggered = state.dispatch_event(
            PreviewKind::PanelVariants,
            &route,
            &TuiEvent::Key(KeyEvent::from(Key::Char('r'))),
            &mut ctx,
        );

        assert_eq!(triggered, EventOutcome::Handled);
        assert_eq!(ctx.messages(), &[Msg::PanelAction("refresh")]);
    }

    #[test]
    fn diff_viewer_has_four_children_with_working_toggles() {
        let children = [
            (ComponentKind::DiffSideBySide, PreviewKind::DiffSideBySide),
            (ComponentKind::DiffInline, PreviewKind::DiffInline),
            (ComponentKind::DiffWord, PreviewKind::DiffWord),
            (ComponentKind::DiffRawPatch, PreviewKind::DiffRawPatch),
        ];
        assert_eq!(
            ComponentKind::ALL
                .iter()
                .filter(|kind| kind.parent() == Some(ComponentKind::DiffViewer))
                .count(),
            4
        );

        for (component, preview) in children {
            assert_eq!(component.preview(), preview);
            let mut demo = match preview {
                PreviewKind::DiffSideBySide => side_by_side_diff_demo::<Msg>(),
                PreviewKind::DiffInline => inline_diff_demo::<Msg>(),
                PreviewKind::DiffWord => word_diff_demo::<Msg>(),
                PreviewKind::DiffRawPatch => raw_patch_diff_demo::<Msg>(),
                _ => unreachable!(),
            };
            assert_eq!(
                <tuicore::DiffViewer as TuiNode<Msg>>::measure(
                    demo.viewer(),
                    LayoutProposal::unbounded(),
                )
                .preferred
                .height,
                20
            );
            assert!(demo.viewer().content_size().height > 20);

            let mut layout = LayoutCtx::new();
            demo.layout(Rect::new(0, 0, 140, 24), &mut layout);
            assert_eq!(
                layout
                    .focus_targets()
                    .iter()
                    .filter(|target| target.id.as_str() == "toggle")
                    .count(),
                2
            );
            assert_eq!(
                layout
                    .focus_targets()
                    .iter()
                    .filter(|target| target.id.as_str() == "diff-viewer")
                    .count(),
                1
            );

            for (key, expected_headers, expected_wrap) in [
                (gallery_demo::diffs::headers_child_key(), false, true),
                (gallery_demo::diffs::wrap_child_key(), false, false),
            ] {
                let outcome = demo.dispatch_event(
                    &EventRoute::new(TreePath::from_keys([key])),
                    &TuiEvent::Key(KeyEvent::from(Key::Enter)),
                    &mut EventCtx::default(),
                );
                assert_eq!(outcome, EventOutcome::Handled);
                assert_eq!(demo.headers_enabled(), expected_headers);
                assert_eq!(demo.wrapping_enabled(), expected_wrap);
                assert_eq!(demo.viewer().headers_visible(), expected_headers);
                assert_eq!(demo.viewer().is_wrapping(), expected_wrap);
            }
        }
    }

    #[test]
    fn preview_date_picker_ticks_quick_jump_timeout() {
        let mut state = PreviewState::new();
        state.date_picker.on_key(Key::Char('1'));

        state.tick(Duration::from_millis(1_001), AnimationSettings::default());
        state.date_picker.on_key(Key::Char('8'));

        assert_eq!(state.date_picker.cursor().day(), 8);
    }

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
    fn reorder_gallery_control_enters_reorder_mode() {
        let mut control = reorder_control::<Msg>();
        let guidance = "Shift+J/K range · Ctrl+J/K navigate + Space select · Ctrl+M move block · ↑↓ move target · Enter commit · Esc clear/cancel";
        assert_eq!(
            control.panel_ref().title_text(PanelTitlePosition::TopLeft),
            Some(guidance)
        );
        let route = EventRoute::new(TreePath::from_keys([ChildKey::new("data")]));
        let key = KeyEvent {
            code: Key::Char('m'),
            modifiers: KeyModifiers::CONTROL,
        };

        control.dispatch_event(&route, &TuiEvent::Key(key), &mut EventCtx::default());

        assert!(control.is_reordering());
        assert_eq!(
            control.panel_ref().title_text(PanelTitlePosition::TopLeft),
            Some(guidance)
        );
    }

    #[test]
    fn tree_reorder_gallery_animates_the_move_highlight() {
        let mut state = PreviewState::new();
        let area = Rect::new(0, 0, 40, 14);
        state.layout(
            PreviewKind::ListReorderTree,
            area,
            area,
            &mut LayoutCtx::new(),
        );
        let route = EventRoute::new(TreePath::from_keys([
            ChildKey::new("first"),
            ChildKey::new("data"),
        ]));
        state.dispatch_event(
            PreviewKind::ListReorderTree,
            &route,
            &TuiEvent::Key(KeyEvent {
                code: Key::Char('m'),
                modifiers: KeyModifiers::CONTROL,
            }),
            &mut EventCtx::default(),
        );
        for _ in 0..5 {
            state.tick(Duration::from_millis(100), AnimationSettings::default());
        }

        let mut terminal = Terminal::new(TestBackend::new(40, 14)).expect("terminal should build");
        terminal
            .draw(|frame| {
                let mut render_ctx = RenderCtx::new();
                state.render(PreviewKind::ListReorderTree, frame, area, &mut render_ctx);
            })
            .expect("tree reorder gallery should render");

        let cell = (0..area.width)
            .find_map(|x| {
                let cell = terminal.backend().buffer().cell((x, 1))?;
                (cell.symbol() == "R").then_some(cell)
            })
            .expect("roadmap row should render");
        let theme = tuicore::theme();
        assert_eq!(cell.fg, theme.highlight_bg());
        assert_eq!(cell.bg, theme.highlight_fg());
    }

    #[test]
    fn mixed_list_control_example_supports_editing() {
        let mut controls = entity_controls::<Msg>();
        let mixed = &mut controls[1];

        mixed.dispatch_event(
            &EventRoute::new(TreePath::from_keys([ChildKey::new("data")])),
            &TuiEvent::Key(KeyEvent::from(Key::Char('e'))),
            &mut EventCtx::default(),
        );

        assert!(mixed.is_editing());
    }

    #[test]
    fn all_text_entity_example_opens_confirmation_with_default_remove_key() {
        let mut controls = entity_controls::<Msg>();
        assert!(controls[0].has_remove_confirmation());
        assert!(!controls[1].has_remove_confirmation());
        assert!(!controls[2].has_remove_confirmation());
        let initial_len = controls[0].items().len();

        controls[0].dispatch_event(
            &EventRoute::new(TreePath::from_keys([ChildKey::new("data")])),
            &TuiEvent::Key(KeyEvent {
                code: Key::Char('x'),
                modifiers: KeyModifiers::CONTROL,
            }),
            &mut EventCtx::default(),
        );

        assert!(controls[0].is_confirming_remove());
        assert_eq!(controls[0].items().len(), initial_len);
    }

    #[test]
    fn derived_people_creator_accepts_only_first_name_and_surname() {
        let mut controls = entity_controls::<Msg>();
        let derived = &mut controls[2];
        let mut ctx = EventCtx::default();
        derived.dispatch_event(
            &EventRoute::new(TreePath::from_keys([ChildKey::new("data")])),
            &TuiEvent::Key(KeyEvent::from(Key::Char('+'))),
            &mut ctx,
        );
        for (index, value) in ["Dorothy", "Vaughan"].into_iter().enumerate() {
            let slot = if index == 0 {
                "add-input".to_string()
            } else {
                format!("add-input-{index}")
            };
            let route = EventRoute::new(TreePath::from_keys([ChildKey::new(slot)]));
            derived.dispatch_event(&route, &TuiEvent::Paste(value.to_string()), &mut ctx);
            derived.dispatch_event(&route, &TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut ctx);
        }

        let added = derived.items().last().expect("person added");
        assert_eq!(added.name, "Dorothy");
        assert_eq!(added.owner, "Vaughan");
        assert_eq!(added.state, "Dorothy Vaughan");
        let added_id = added.id;
        derived.data_view_mut().highlight_id(&added_id);
        let cells = derived
            .data_view()
            .highlighted_json()
            .expect("added person highlighted");
        assert!(cells.contains("DV"));
        assert!(cells.contains("Dorothy Vaughan"));
    }

    #[test]
    fn derived_people_editor_recomputes_full_name() {
        let mut controls = entity_controls::<Msg>();
        let derived = &mut controls[2];
        let mut ctx = EventCtx::default();
        let data = EventRoute::new(TreePath::from_keys([ChildKey::new("data")]));
        derived.dispatch_event(
            &data,
            &TuiEvent::Key(KeyEvent::from(Key::Char('e'))),
            &mut ctx,
        );
        for (index, value) in ["Dorothy", "Vaughan"].into_iter().enumerate() {
            let slot = if index == 0 {
                "add-input".to_string()
            } else {
                format!("add-input-{index}")
            };
            let route = EventRoute::new(TreePath::from_keys([ChildKey::new(slot)]));
            derived.dispatch_event(&route, &TuiEvent::Paste(value.to_string()), &mut ctx);
            derived.dispatch_event(&route, &TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut ctx);
        }

        let edited = &derived.items()[0];
        assert_eq!(edited.name, "KatherineDorothy");
        assert_eq!(edited.owner, "JohnsonVaughan");
        assert_eq!(edited.state, "KatherineDorothy JohnsonVaughan");
        let cells = derived
            .data_view()
            .highlighted_json()
            .expect("edited person highlighted");
        assert!(cells.contains("KJ"));
        assert!(cells.contains("KatherineDorothy JohnsonVaughan"));
    }

    #[test]
    fn list_control_gallery_stacks_intrinsic_capped_examples() {
        let mut state = PreviewState::new();
        let area = Rect::new(0, 0, 80, 30);
        state.layout(PreviewKind::ListCompact, area, area, &mut LayoutCtx::new());
        let areas = [
            state
                .list_compact
                .child_rect(&ChildKey::new("first"))
                .unwrap(),
            state
                .list_compact
                .child_rect(&ChildKey::new("second"))
                .unwrap(),
        ];

        assert!(areas[0].height > 0);
        assert!(areas[1].height > 0);
        assert_eq!(areas[1].y, areas[0].bottom() + 1);
        assert!(areas[1].bottom() <= area.bottom());
        assert!(areas.iter().all(|child| child.height < area.height));
    }

    #[test]
    fn short_list_control_gallery_shares_height_between_examples() {
        let mut state = PreviewState::new();
        let area = Rect::new(4, 2, 40, 3);
        state.layout(PreviewKind::ListCompact, area, area, &mut LayoutCtx::new());
        let areas = [
            state
                .list_compact
                .child_rect(&ChildKey::new("first"))
                .unwrap(),
            state
                .list_compact
                .child_rect(&ChildKey::new("second"))
                .unwrap(),
        ];

        assert!(areas.iter().all(|child| !child.is_empty()));
        assert!(areas.iter().all(|child| child.x == area.x));
        assert!(areas.iter().all(|child| child.right() <= area.right()));
        assert!(areas.iter().all(|child| child.bottom() <= area.bottom()));
        assert_eq!(areas[1].y, areas[0].bottom() + 1);
    }

    #[test]
    fn short_entity_gallery_clips_fit_content_children_to_available_height() {
        let mut state = PreviewState::new();
        let area = Rect::new(4, 2, 40, 3);
        state.layout(
            PreviewKind::ListEntityTable,
            area,
            area,
            &mut LayoutCtx::new(),
        );

        let areas = ["first", "second", "third"].map(|key| {
            state
                .list_entity_table
                .child_rect(&ChildKey::new(key))
                .unwrap()
        });
        assert!(areas.iter().any(|child| !child.is_empty()));
        assert!(areas.iter().all(|child| child.bottom() <= area.bottom()));
    }

    #[test]
    fn hidden_list_control_examples_receive_empty_layout_areas() {
        let mut state = PreviewState::new();
        let area = Rect::new(0, 0, 40, 1);
        let mut layout = LayoutCtx::new();
        state.layout(PreviewKind::ListCompact, area, area, &mut layout);
        let areas = [
            state
                .list_compact
                .child_rect(&ChildKey::new("first"))
                .unwrap(),
            state
                .list_compact
                .child_rect(&ChildKey::new("second"))
                .unwrap(),
        ];

        assert!(areas.iter().all(|area| area.is_empty()));
        assert!(
            layout
                .focus_targets()
                .iter()
                .all(|target| target.area.is_empty())
        );
    }

    #[test]
    fn relative_date_time_messages_keep_both_outputs_in_sync() {
        let mut state = PreviewState::new();
        let reference_time = Time::from_hms(9, 0, 0).expect("valid reference time");
        let target_time = Time::from_hms(11, 0, 0).expect("valid target time");

        assert!(
            state
                .relative_date
                .apply_message(&Msg::RelativeReferenceSelected(demo_date()))
        );
        assert!(
            state
                .relative_date
                .apply_message(&Msg::RelativeTargetSelected(demo_date()))
        );
        assert!(
            state
                .relative_date
                .apply_message(&Msg::RelativeReferenceTimeSelected(reference_time))
        );
        assert!(
            state
                .relative_date
                .apply_message(&Msg::RelativeTargetTimeSelected(target_time))
        );

        assert_eq!(state.relative_date.reference().date(), demo_date());
        assert_eq!(state.relative_date.reference().time(), reference_time);
        assert_eq!(state.relative_date.target().date(), demo_date());
        assert_eq!(state.relative_date.target().time(), target_time);
        assert_eq!(state.relative_date.distance_text(), "In 2 hours");
        assert_eq!(state.relative_date.calendar_week_text(), "In 2 hours");
    }

    #[test]
    fn escape_shortcuts_from_preview_focus_return_to_component_overview() {
        let mut gallery = Gallery::new();
        gallery.select(ComponentKind::Toggle);
        let mut layout = LayoutCtx::new();
        gallery.layout(Rect::new(0, 0, 100, 30), &mut layout);
        let route = EventRoute::new(TreePath::from_keys([
            gallery_preview_child_key(),
            toggle_switch_child_key(),
        ]));
        for key in [
            KeyEvent::from(Key::Esc),
            KeyEvent {
                code: Key::Char('['),
                modifiers: KeyModifiers::CONTROL,
            },
        ] {
            let mut ctx = EventCtx::default();
            let outcome = gallery.dispatch_event(&route, &TuiEvent::Key(key), &mut ctx);

            assert_eq!(outcome, EventOutcome::Handled);
            assert_eq!(
                ctx.focus_request(),
                Some(&FocusRequest::TargetAt {
                    path: TreePath::from_keys([gallery_list_child_key()]),
                    id: FocusId::new("data-view"),
                })
            );
            let mut focus = tuicore::FocusManager::new();
            focus.apply_request(ctx.focus_request().unwrap(), layout.focus_targets());
            assert_eq!(focus.current().unwrap().id.as_str(), "data-view");
            assert_eq!(
                focus.current().unwrap().path,
                TreePath::from_keys([gallery_list_child_key()])
            );
        }
    }

    #[test]
    fn inline_date_time_controls_unfocus_to_exact_component_overview_target() {
        let cancel_keys = [
            KeyEvent::from(Key::Esc),
            KeyEvent {
                code: Key::Char('['),
                modifiers: KeyModifiers::CONTROL,
            },
        ];

        for child_key in [
            date_picker_child_key(),
            time_picker_child_key(),
            date_time_picker_child_key(),
        ] {
            for key in cancel_keys {
                let mut gallery = Gallery::new();
                gallery.select(ComponentKind::DateTimePicker);
                if child_key == date_time_picker_child_key() {
                    gallery
                        .previews
                        .date_time_picker
                        .event(&TuiEvent::Key(Key::Enter.into()), &mut EventCtx::default());
                }
                let mut layout = LayoutCtx::new();
                gallery.layout(Rect::new(0, 0, 100, 30), &mut layout);
                let route = EventRoute::new(TreePath::from_keys([
                    gallery_preview_child_key(),
                    child_key.clone(),
                ]));
                let mut ctx = EventCtx::default();

                let outcome = gallery.dispatch_event(&route, &TuiEvent::Key(key), &mut ctx);

                assert_eq!(outcome, EventOutcome::Handled);
                assert_eq!(
                    ctx.focus_request(),
                    Some(&FocusRequest::TargetAt {
                        path: TreePath::from_keys([gallery_list_child_key()]),
                        id: FocusId::new("data-view"),
                    })
                );
                let mut focus = tuicore::FocusManager::new();
                focus.apply_request(ctx.focus_request().unwrap(), layout.focus_targets());
                assert_eq!(focus.current().unwrap().id.as_str(), "data-view");
                assert_eq!(
                    focus.current().unwrap().path,
                    TreePath::from_keys([gallery_list_child_key()])
                );
            }
        }
    }

    #[test]
    fn date_time_dropdowns_close_before_next_cancel_targets_component_overview() {
        let cancel_keys = [
            KeyEvent::from(Key::Esc),
            KeyEvent {
                code: Key::Char('['),
                modifiers: KeyModifiers::CONTROL,
            },
        ];

        for child_key in [date_dropdown_child_key(), date_time_dropdown_child_key()] {
            for key in cancel_keys {
                let mut gallery = Gallery::new();
                gallery.select(ComponentKind::DateTimePicker);
                if child_key == date_dropdown_child_key() {
                    gallery.previews.date_dropdown.set_open(true);
                } else {
                    gallery.previews.date_time_dropdown.set_open(true);
                }
                let route = EventRoute::new(TreePath::from_keys([
                    gallery_preview_child_key(),
                    child_key.clone(),
                ]));
                let mut close_ctx = EventCtx::default();

                let close = gallery.dispatch_event(&route, &TuiEvent::Key(key), &mut close_ctx);

                assert_eq!(close, EventOutcome::Handled);
                assert_eq!(close_ctx.focus_request(), None);
                assert!(!gallery.previews.date_dropdown.is_open());
                assert!(!gallery.previews.date_time_dropdown.is_open());

                let mut leave_ctx = EventCtx::default();
                let leave = gallery.dispatch_event(&route, &TuiEvent::Key(key), &mut leave_ctx);

                assert_eq!(leave, EventOutcome::Handled);
                assert_eq!(
                    leave_ctx.focus_request(),
                    Some(&FocusRequest::TargetAt {
                        path: TreePath::from_keys([gallery_list_child_key()]),
                        id: FocusId::new("data-view"),
                    })
                );
            }
        }
    }

    #[test]
    fn every_gallery_preview_focus_receives_escape_before_global_hotkeys() {
        let mut gallery = Gallery::new();
        let mut layout = LayoutCtx::new();
        gallery.layout(Rect::new(0, 0, 100, 30), &mut layout);
        let preview_path = TreePath::from_keys([gallery_preview_child_key()]);
        let preview_targets = layout
            .focus_targets()
            .iter()
            .filter(|target| target.path.keys().starts_with(preview_path.keys()))
            .collect::<Vec<_>>();

        assert!(!preview_targets.is_empty());
        assert!(
            preview_targets
                .iter()
                .all(|target| target.focused_events_before_global_hotkeys)
        );
    }

    #[test]
    fn nested_gallery_escape_targets_absolute_component_overview_path() {
        std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(nested_gallery_escape_targets_absolute_component_overview_path_inner)
            .expect("test thread should start")
            .join()
            .expect("test thread should finish");
    }

    fn nested_gallery_escape_targets_absolute_component_overview_path_inner() {
        let dialog_layer = DialogLayer::new(Gallery::new(), gallery_dialog()).active(false);
        let tabs_layer = DialogLayer::new(dialog_layer, modal_tabs_dialog()).active(false);
        let root_layer = DialogLayer::new(tabs_layer, gallery_dock_overlay()).active(false);
        let store_view = DialogLayer::new(root_layer, empty_store_debug_dialog()).active(false);
        let confirmation =
            DialogLayer::new(store_view, gallery_confirmation_dialog()).active(false);
        let speed_reader = DialogLayer::new(
            confirmation,
            gallery_speed_reader(SpeedReaderExample::Markdown),
        )
        .active(false);
        let mut root = DialogLayer::new(speed_reader, ai_dock_dialog()).active(false);
        gallery(&mut root).select(ComponentKind::TextInput);
        gallery(&mut root).previews.text_input.set_focused(true);
        gallery(&mut root).previews.text_input.event(
            &TuiEvent::Key(KeyEvent::from(Key::Enter)),
            &mut EventCtx::default(),
        );
        let mut layout = LayoutCtx::new();
        root.layout(Rect::new(0, 0, 100, 30), &mut layout);
        let preview_target = layout
            .focus_targets()
            .iter()
            .find(|target| {
                target.id.as_str() == "input"
                    && target.path.keys().contains(&gallery_preview_child_key())
            })
            .expect("gallery preview should register focus")
            .clone();
        assert!(preview_target.focused_events_before_global_hotkeys);
        let overview_target = layout
            .focus_targets()
            .iter()
            .find(|target| {
                target.id.as_str() == "data-view"
                    && target.path.keys().contains(&gallery_list_child_key())
            })
            .expect("component overview should register focus")
            .clone();
        let route = EventRoute::new(preview_target.path);
        let first = tuicore::TreeDispatcher::new().dispatch_event(
            &mut root,
            &route,
            &TuiEvent::Key(KeyEvent::from(Key::Esc)),
            AnimationSettings::default(),
        );
        assert_eq!(first.outcome, EventOutcome::Handled);
        assert!(first.focus_request.is_none());
        assert!(!gallery(&mut root).previews.text_input.insert_mode());

        let collision_route = EventRoute::new(route.path.child(gallery_preview_child_key()));
        let second = tuicore::TreeDispatcher::new().dispatch_event(
            &mut root,
            &collision_route,
            &TuiEvent::Key(KeyEvent::from(Key::Esc)),
            AnimationSettings::default(),
        );

        assert_eq!(second.outcome, EventOutcome::Handled);
        assert_eq!(
            second.focus_request,
            Some(FocusRequest::TargetAt {
                path: overview_target.path,
                id: overview_target.id,
            })
        );
    }

    #[test]
    fn controlled_vim_form_navigation_moves_only_between_adjacent_form_components() {
        let mut state = PreviewState::new();
        let first = EventRoute::new(TreePath::from_keys([text_input_child_key()]));
        let middle = EventRoute::new(TreePath::from_keys([text_input_panel_child_key()]));

        let mut forward = EventCtx::default();
        let outcome = state.dispatch_event(
            PreviewKind::TextInput,
            &first,
            &TuiEvent::Key(KeyEvent {
                code: Key::Char('l'),
                modifiers: KeyModifiers::CONTROL,
            }),
            &mut forward,
        );
        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(forward.focus_request(), Some(&FocusRequest::Next));

        let mut backward = EventCtx::default();
        let outcome = state.dispatch_event(
            PreviewKind::TextInput,
            &middle,
            &TuiEvent::Key(KeyEvent {
                code: Key::Char('k'),
                modifiers: KeyModifiers::CONTROL,
            }),
            &mut backward,
        );
        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(backward.focus_request(), Some(&FocusRequest::Previous));

        let mut blocked = EventCtx::default();
        let outcome = state.dispatch_event(
            PreviewKind::TextInput,
            &first,
            &TuiEvent::Key(KeyEvent {
                code: Key::Char('h'),
                modifiers: KeyModifiers::CONTROL,
            }),
            &mut blocked,
        );
        assert_eq!(outcome, EventOutcome::Handled);
        assert!(blocked.focus_request().is_none());
    }

    #[test]
    fn vim_form_navigation_does_not_leave_active_input_mode() {
        let mut state = PreviewState::new();
        state.text_input.set_focused(true);
        state.text_input.event(
            &TuiEvent::Key(KeyEvent::from(Key::Enter)),
            &mut EventCtx::default(),
        );
        assert!(state.text_input.insert_mode());
        let route = EventRoute::new(TreePath::from_keys([text_input_child_key()]));
        let mut ctx = EventCtx::default();

        let outcome = state.dispatch_event(
            PreviewKind::TextInput,
            &route,
            &TuiEvent::Key(KeyEvent::from(Key::Char('l'))),
            &mut ctx,
        );

        assert_eq!(outcome, EventOutcome::Handled);
        assert!(ctx.focus_request().is_none());
        assert_eq!(state.text_input.current_value(), "l");
    }

    #[test]
    fn controlled_vim_navigation_covers_all_gallery_form_component_previews() {
        let cases = [
            (PreviewKind::TextInput, text_input_child_key()),
            (PreviewKind::PasswordInput, password_input_child_key()),
            (PreviewKind::TextareaInput, textarea_input_child_key()),
            (PreviewKind::DateTimePicker, date_picker_child_key()),
            (PreviewKind::RelativeDate, reference_picker_child_key()),
            (PreviewKind::Dropdown, dropdown_child_key(0)),
            (PreviewKind::TagInput, tag_input_child_key()),
            (PreviewKind::Toggle, toggle_switch_child_key()),
        ];

        for (preview, key) in cases {
            let mut state = PreviewState::new();
            let route = EventRoute::new(TreePath::from_keys([key]));
            let mut ctx = EventCtx::default();

            let outcome = state.dispatch_event(
                preview,
                &route,
                &TuiEvent::Key(KeyEvent {
                    code: Key::Char('j'),
                    modifiers: KeyModifiers::CONTROL,
                }),
                &mut ctx,
            );

            assert_eq!(outcome, EventOutcome::Handled, "preview: {preview:?}");
            assert_eq!(
                ctx.focus_request(),
                Some(&FocusRequest::Next),
                "preview: {preview:?}"
            );
        }
    }

    #[test]
    fn vim_form_navigation_stays_inside_open_dropdown() {
        let mut state = PreviewState::new();
        state.dropdown_mut(0).open();
        let route = EventRoute::new(TreePath::from_keys([dropdown_child_key(0)]));
        let mut ctx = EventCtx::default();

        state.dispatch_event(
            PreviewKind::Dropdown,
            &route,
            &TuiEvent::Key(KeyEvent {
                code: Key::Char('j'),
                modifiers: KeyModifiers::CONTROL,
            }),
            &mut ctx,
        );

        assert!(state.dropdown(0).is_open());
        assert!(ctx.focus_request().is_none());
    }

    #[test]
    fn preview_escape_exits_text_input_before_returning_to_overview() {
        let mut gallery = Gallery::new();
        gallery.select(ComponentKind::TextInput);
        gallery.previews.text_input.set_focused(true);
        gallery.previews.text_input.event(
            &TuiEvent::Key(KeyEvent::from(Key::Enter)),
            &mut EventCtx::default(),
        );
        let route = EventRoute::new(TreePath::from_keys([
            gallery_preview_child_key(),
            text_input_child_key(),
        ]));

        let mut first = EventCtx::default();
        let outcome =
            gallery.dispatch_event(&route, &TuiEvent::Key(KeyEvent::from(Key::Esc)), &mut first);
        assert_eq!(outcome, EventOutcome::Handled);
        assert!(!gallery.previews.text_input.insert_mode());
        assert!(first.focus_request().is_none());

        let mut second = EventCtx::default();
        gallery.dispatch_event(
            &route,
            &TuiEvent::Key(KeyEvent::from(Key::Esc)),
            &mut second,
        );
        assert!(matches!(
            second.focus_request(),
            Some(FocusRequest::TargetAt { id, .. }) if id.as_str() == "data-view"
        ));
    }

    #[test]
    fn preview_escape_closes_dropdown_before_returning_to_overview() {
        let mut gallery = Gallery::new();
        gallery.select(ComponentKind::Dropdown);
        gallery.previews.dropdown_mut(0).open();
        let route = EventRoute::new(TreePath::from_keys([
            gallery_preview_child_key(),
            dropdown_child_key(0),
        ]));

        let mut first = EventCtx::default();
        gallery.dispatch_event(&route, &TuiEvent::Key(KeyEvent::from(Key::Esc)), &mut first);
        assert!(!gallery.previews.dropdown(0).is_open());
        assert!(first.focus_request().is_none());

        let mut second = EventCtx::default();
        gallery.dispatch_event(
            &route,
            &TuiEvent::Key(KeyEvent::from(Key::Esc)),
            &mut second,
        );
        assert!(matches!(
            second.focus_request(),
            Some(FocusRequest::TargetAt { id, .. }) if id.as_str() == "data-view"
        ));
    }

    #[test]
    fn preview_escape_cancels_data_view_search_before_returning_to_overview() {
        let mut gallery = Gallery::new();
        gallery.select(ComponentKind::DataViewList);
        gallery
            .previews
            .active_data_view_mut(PreviewKind::DataList)
            .set_focused(true);
        let preview_route = EventRoute::new(TreePath::from_keys([gallery_preview_child_key()]));
        gallery.dispatch_event(
            &preview_route,
            &TuiEvent::Key(KeyEvent::from(Key::Char('/'))),
            &mut EventCtx::default(),
        );
        let search_route = EventRoute::new(TreePath::from_keys([
            gallery_preview_child_key(),
            ChildKey::new("search"),
        ]));

        let mut first = EventCtx::default();
        let outcome = gallery.dispatch_event(
            &search_route,
            &TuiEvent::Key(KeyEvent::from(Key::Esc)),
            &mut first,
        );
        assert_eq!(outcome, EventOutcome::Handled);
        assert!(matches!(
            first.focus_request(),
            Some(FocusRequest::TargetAt { path, id })
                if path.is_empty() && id.as_str() == "data-view"
        ));

        let mut second = EventCtx::default();
        gallery.dispatch_event(
            &search_route,
            &TuiEvent::Key(KeyEvent::from(Key::Esc)),
            &mut second,
        );
        assert_eq!(
            second.focus_request(),
            Some(&FocusRequest::TargetAt {
                path: TreePath::from_keys([gallery_list_child_key()]),
                id: FocusId::new("data-view"),
            })
        );
    }

    #[test]
    fn preview_escape_backs_out_of_calendar_before_returning_to_overview() {
        let mut gallery = Gallery::new();
        gallery.select(ComponentKind::Calendar);
        gallery.previews.calendar.set_focused(true);
        let route = EventRoute::new(TreePath::from_keys([
            gallery_preview_child_key(),
            calendar_child_key(),
        ]));
        gallery.dispatch_event(
            &route,
            &TuiEvent::Key(KeyEvent::from(Key::Enter)),
            &mut EventCtx::default(),
        );
        assert_eq!(gallery.previews.calendar.current_view(), CalendarView::Week);

        let mut first = EventCtx::default();
        let outcome =
            gallery.dispatch_event(&route, &TuiEvent::Key(KeyEvent::from(Key::Esc)), &mut first);
        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(
            gallery.previews.calendar.current_view(),
            CalendarView::Month
        );
        assert!(first.focus_request().is_none());

        let mut second = EventCtx::default();
        gallery.dispatch_event(
            &route,
            &TuiEvent::Key(KeyEvent::from(Key::Esc)),
            &mut second,
        );
        assert_eq!(
            second.focus_request(),
            Some(&FocusRequest::TargetAt {
                path: TreePath::from_keys([gallery_list_child_key()]),
                id: FocusId::new("data-view"),
            })
        );
    }

    #[test]
    fn notification_trigger_toasts_render_over_gallery() {
        let mut gallery = Gallery::new();
        gallery
            .previews
            .notification_triggers
            .push(notification_for_index(0).sticky());
        let mut settings = AnimationSettings::default();
        settings.enabled = false;
        gallery.tick(Duration::ZERO, settings);

        let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("terminal should build");
        terminal
            .draw(|frame| {
                let area = frame.area();
                let mut layout_ctx = LayoutCtx::new();
                gallery.layout(area, &mut layout_ctx);
                let mut render_ctx = RenderCtx::new();
                gallery.render(frame, area, &mut render_ctx);
            })
            .expect("gallery should render");

        assert!(buffer_contains(terminal.backend().buffer(), "One line"));
    }

    fn buffer_contains(buffer: &ratatui::buffer::Buffer, needle: &str) -> bool {
        (0..buffer.area.height).any(|y| {
            let row = (0..buffer.area.width)
                .map(|x| buffer.cell((x, y)).map(|cell| cell.symbol()).unwrap_or(" "))
                .collect::<String>();
            row.contains(needle)
        })
    }
}

// Rig Calculator Tool and dialog definition
#[derive(Deserialize, Serialize, JsonSchema)]
struct CalculatorArgs {
    op: String,
    x: f64,
    y: f64,
}

struct CalculatorTool {
    sender: mpsc::Sender<LlmEvent>,
    request_id: u64,
}

impl Tool for CalculatorTool {
    const NAME: &'static str = "calculator";
    type Error = std::convert::Infallible;
    type Args = CalculatorArgs;
    type Output = f64;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Perform basic mathematical calculations: add, sub, mul, div.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "required": ["op", "x", "y"],
                "properties": {
                    "op": {
                        "type": "string",
                        "enum": ["add", "sub", "mul", "div"],
                        "description": "Operation to perform"
                    },
                    "x": { "type": "number", "description": "First number" },
                    "y": { "type": "number", "description": "Second number" }
                }
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let args_str = serde_json::to_string_pretty(&args).unwrap_or_default();

        let _ = self.sender.send(LlmEvent::approval(
            self.request_id,
            Self::NAME,
            args_str,
            tx,
        ));

        let approved = rx.await.unwrap_or(false);
        if !approved {
            let _ = self.sender.send(LlmEvent::status(
                self.request_id,
                "Tool call 'calculator' was denied by the user.",
            ));
            return Ok(0.0);
        }

        let _ = self.sender.send(LlmEvent::status(
            self.request_id,
            "Tool call 'calculator' approved. Executing...",
        ));

        let result = match args.op.as_str() {
            "add" => args.x + args.y,
            "sub" => args.x - args.y,
            "mul" => args.x * args.y,
            "div" => {
                if args.y == 0.0 {
                    0.0
                } else {
                    args.x / args.y
                }
            }
            _ => 0.0,
        };

        let _ = self.sender.send(LlmEvent::status(
            self.request_id,
            format!("Tool call 'calculator' completed. Result: {}", result),
        ));

        Ok(result)
    }
}

fn ai_dock_dialog() -> AiDock<Msg> {
    let runner = |prompt: String,
                  history: Vec<rig::message::Message>,
                  sender: mpsc::Sender<LlmEvent>,
                  request_id: u64,
                  _provider: String,
                  model: String| {
        thread::spawn(move || {
            let runtime = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(err) => {
                    let _ = sender.send(LlmEvent::error(
                        request_id,
                        format!("Tokio runtime error: {}", err),
                    ));
                    return;
                }
            };

            runtime.block_on(async {
                let model = if model.is_empty() {
                    std::env::var("LLM_MODEL")
                        .unwrap_or_else(|_| "openai/gpt-5.5".to_string())
                } else {
                    if model.contains('/') {
                        model
                    } else {
                        format!("openai/{}", model)
                    }
                };

                let status_sender = sender.clone();
                let token_dir = chatgpt_token_dir();
                let client = match chatgpt::Client::builder()
                    .oauth()
                    .token_dir(token_dir)
                    .on_device_code(move |code| {
                        let _ = status_sender.send(LlmEvent::status(
                            request_id,
                            format!(
                                "OAuth: Open {} and enter code {}",
                                code.verification_uri, code.user_code
                            ),
                        ));
                    })
                    .build()
                {
                    Ok(c) => c,
                    Err(err) => {
                        let _ = sender.send(LlmEvent::error(request_id, format!("Failed to build client: {}", err)));
                        return;
                    }
                };

                let _ = sender.send(LlmEvent::status(request_id, "Authorizing..."));
                if let Err(err) = client.authorize().await {
                    let _ = sender.send(LlmEvent::error(request_id, format!("Auth failed: {}", err)));
                    return;
                }

                let model_name = model.strip_prefix("openai/").unwrap_or(&model).to_string();
                let agent = client
                    .agent(&model_name)
                    .preamble("You are a helpful arithmetic assistant. Use the calculator tool for math operations. Summarize the tool result to the user.")
                    .tool(CalculatorTool {
                        sender: sender.clone(),
                        request_id,
                    })
                    .build();

                let _ = sender.send(LlmEvent::status(request_id, format!("Calling {}...", model_name)));
                let mut stream = agent
                    .stream_prompt(prompt)
                    .with_history(history)
                    .multi_turn(4)
                    .await;

                let mut output = String::new();
                let mut updated_history = Vec::new();
                let mut usage = rig::completion::Usage::new();

                while let Some(chunk) = stream.next().await {
                    match chunk {
                        Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(
                            RigText { text, .. },
                        ))) => {
                            output.push_str(&text);
                            let _ = sender.send(LlmEvent::chunk(request_id, text));
                        }
                        Ok(MultiTurnStreamItem::FinalResponse(final_response)) => {
                            usage = final_response
                                .completion_calls()
                                .last()
                                .map(|call| call.usage)
                                .unwrap_or_else(|| final_response.usage());
                            usage.total_tokens = usage.input_tokens.saturating_add(usage.output_tokens);
                            if let Some(hist) = final_response.history() {
                                updated_history = hist.to_vec();
                            }
                        }
                        Err(err) => {
                            let _ = sender.send(LlmEvent::error(request_id, format!("Stream error: {}", err)));
                            return;
                        }
                        _ => {}
                    }
                }

                let _ = sender.send(LlmEvent::complete_with_usage(
                    request_id,
                    updated_history,
                    output,
                    usage,
                ));
            });
        });
    };

    let calculator_schema = r#"{
  "type": "object",
  "required": ["op", "x", "y"],
  "properties": {
    "op": {
      "type": "string",
      "enum": ["add", "sub", "mul", "div"],
      "description": "Operation to perform"
    },
    "x": { "type": "number", "description": "First number" },
    "y": { "type": "number", "description": "Second number" }
  }
}"#;

    AiDock::new(runner)
        .on_close(|| Msg::CloseAiDock)
        .tool(
            "calculator",
            "Perform simple mathematical calculations",
            calculator_schema,
        )
        .tool_policy("calculator", ToolPolicy::AskBeforeRun)
}

fn empty_store_debug_dialog() -> StoreDebugView<Msg> {
    store_debug_dialog(InspectValue::object([]), Vec::new())
}

fn store_debug_dialog(state: InspectValue, events: Vec<StoreLogEntry>) -> StoreDebugView<Msg> {
    StoreDebugView::dialog(state, events).on_close(Msg::StoreViewClosed)
}

fn chatgpt_token_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("TUICORE_CHATGPT_TOKEN_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(dir).join("tuicore").join("rig-chatgpt");
    }
    if let Ok(dir) = std::env::var("APPDATA") {
        return PathBuf::from(dir).join("tuicore").join("rig-chatgpt");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("tuicore")
            .join("rig-chatgpt");
    }
    std::env::temp_dir().join("tuicore").join("rig-chatgpt")
}
