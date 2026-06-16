use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use tuicore::{
    ActivationMode, Animated, AnimationSettings, BorderKind, CellContext, Column, DataView,
    DataViewTypedEvent, EventCtx, EventOutcome, Key, KeyEvent, KeyModifiers, LayoutCtx,
    LayoutResult, Panel, PanelVariant, SelectionGlyphs, SelectionMode, SelectionPropagation,
    SelectionTrigger, Spinner, Tabs, TabsVariant, TextInput, TextareaInput, TickResult,
    TreeAdapter, TreeGlyphs, TuiEvent, TuiNode,
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
        .expanded([ComponentKind::Inputs, ComponentKind::DataView])
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

    fn focus_next(&mut self, settings: AnimationSettings) {
        if self.focus == GalleryFocus::Preview {
            let advanced = match self.selected.preview() {
                PreviewKind::Tabs => self.previews.focus_next_tab_demo(),
                PreviewKind::Panel => self.previews.focus_next_panel(),
                _ => false,
            };
            if advanced {
                self.previews
                    .set_focused(self.selected.preview(), true, settings);
                return;
            }
        }
        self.focus = if self.focus == GalleryFocus::List {
            self.previews.reset_tab_demo_focus();
            GalleryFocus::Preview
        } else {
            GalleryFocus::List
        };
        self.sync_focus(settings);
    }

    fn focus_previous(&mut self, settings: AnimationSettings) {
        if self.focus == GalleryFocus::Preview {
            let moved = match self.selected.preview() {
                PreviewKind::Tabs => self.previews.focus_previous_tab_demo(),
                PreviewKind::Panel => self.previews.focus_previous_panel(),
                _ => false,
            };
            if moved {
                self.previews
                    .set_focused(self.selected.preview(), true, settings);
                return;
            }
        }
        self.focus = if self.focus == GalleryFocus::List {
            match self.selected.preview() {
                PreviewKind::Tabs => self.previews.focus_last_tab_demo(),
                PreviewKind::Panel => self.previews.focus_last_panel(),
                _ => {}
            }
            GalleryFocus::Preview
        } else {
            GalleryFocus::List
        };
        self.sync_focus(settings);
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

    fn quit_key(event: &TuiEvent) -> bool {
        let TuiEvent::Key(KeyEvent { code, modifiers }) = event else {
            return false;
        };
        *code == Key::Esc
            || matches!(*code, Key::Char('q'))
            || (matches!(*code, Key::Char(value) if value.eq_ignore_ascii_case(&'c'))
                && modifiers.contains(KeyModifiers::CONTROL))
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
            .layout(self.selected.preview(), self.areas.preview_body);
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

        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };

        let bindings = tuicore::keybindings();
        if bindings.focus().previous_matches(*key) {
            self.focus_previous(ctx.animation());
            ctx.request_redraw();
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        if bindings.focus().next_matches(*key) {
            self.focus_next(ctx.animation());
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
}

struct PreviewState {
    text_input: TextInput<Msg>,
    textarea_input: TextareaInput<Msg>,
    spinner: Spinner,
    panels: Vec<Panel>,
    focused_panel: usize,
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
            panels: demo_panels(),
            focused_panel: 0,
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
        }
    }

    fn set_focused(&mut self, preview: PreviewKind, focused: bool, settings: AnimationSettings) {
        self.text_input.set_focused(focused);
        self.textarea_input.set_focused(focused);
        for (index, panel) in self.panels.iter_mut().enumerate() {
            panel.set_focused(
                focused && preview == PreviewKind::Panel && self.focused_panel == index,
                settings,
            );
        }
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
        self.active_data_view_mut(preview).set_focused(focused);
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

    fn focus_next_panel(&mut self) -> bool {
        if self.focused_panel + 1 >= self.panels.len() {
            return false;
        }
        self.focused_panel += 1;
        true
    }

    fn focus_previous_panel(&mut self) -> bool {
        if self.focused_panel == 0 {
            return false;
        }
        self.focused_panel -= 1;
        true
    }

    fn focus_last_panel(&mut self) {
        self.focused_panel = self.panels.len().saturating_sub(1);
    }

    fn layout(&mut self, preview: PreviewKind, area: Rect) {
        match preview {
            PreviewKind::Tabs => self.layout_tabs(area),
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
            PreviewKind::TextInput => self.text_input.on_key(key).needs_redraw(),
            PreviewKind::TextareaInput => self.textarea_input.on_key(key).needs_redraw(),
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
                outcome.handled || outcome.needs_redraw()
            }
            PreviewKind::Panel | PreviewKind::Spinner => false,
        }
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(&mut self.spinner, dt, settings)
            .merge(
                self.panels
                    .iter_mut()
                    .fold(TickResult::IDLE, |tick, panel| {
                        tick.merge(Animated::tick(panel, dt, settings))
                    }),
            )
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

    fn render_text_input(&self, frame: &mut Frame, area: Rect) {
        let [instructions, input] = input_layout(area);
        frame.render_widget(
            Paragraph::new(
                "Type text. Ctrl+C quits from gallery root. Enter submits. Esc quits. Tab returns to list.",
            ),
            instructions,
        );
        self.text_input.render(frame, input);
    }

    fn render_textarea_input(&self, frame: &mut Frame, area: Rect) {
        let [instructions, input] = textarea_layout(area);
        frame.render_widget(
            Paragraph::new(
                "Type text. Enter inserts newline. Ctrl+Enter/Ctrl+D submits. Esc quits. Tab returns to list.",
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
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(34),
                Constraint::Percentage(33),
                Constraint::Percentage(33),
            ])
            .split(area);
        let areas = [
            two_columns(rows[0])[0],
            two_columns(rows[0])[1],
            two_columns(rows[1])[0],
            two_columns(rows[1])[1],
            two_columns(rows[2])[0],
            two_columns(rows[2])[1],
        ];
        for (panel, area) in self.panels.iter().zip(areas) {
            panel.render(frame, area);
        }
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

    fn tabs_on_key(&mut self, key: KeyEvent, ctx: &mut EventCtx<Msg>) -> bool {
        let event = TuiEvent::Key(key);
        match self.focused_tab_demo {
            0 => self.tabs_minimal.event(&event, ctx).handled(),
            1 => self.tabs_underline.event(&event, ctx).handled(),
            _ => self.tabs_boxed.event(&event, ctx).handled(),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
enum ComponentKind {
    Tabs,
    Panel,
    Spinner,
    Inputs,
    TextInput,
    TextareaInput,
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
    const ALL: [Self; 15] = [
        Self::Tabs,
        Self::Panel,
        Self::Spinner,
        Self::Inputs,
        Self::TextInput,
        Self::TextareaInput,
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
            Self::Inputs => "Inputs",
            Self::TextInput => "Text",
            Self::TextareaInput => "Textarea",
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
            Self::TextInput | Self::TextareaInput => Some(Self::Inputs),
            _ => None,
        }
    }

    fn preview(self) -> PreviewKind {
        match self {
            Self::Tabs => PreviewKind::Tabs,
            Self::Panel => PreviewKind::Panel,
            Self::Spinner => PreviewKind::Spinner,
            Self::Inputs | Self::TextInput => PreviewKind::TextInput,
            Self::TextareaInput => PreviewKind::TextareaInput,
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
    TextInput,
    TextareaInput,
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
            Self::TextInput => "Text",
            Self::TextareaInput => "Textarea",
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

fn demo_panels() -> Vec<Panel> {
    vec![
        Panel::new().content(["No title", "Panels can still render titleless."]),
        Panel::new()
            .top_left("Left")
            .content(["Top-left title slot"]),
        Panel::new()
            .top_right("Right")
            .content(["Top-right title slot"]),
        Panel::new()
            .top_left("Left")
            .top_right("Right")
            .content(["Both title slots"]),
        Panel::new()
            .top_left("Style 1")
            .border(BorderKind::Rounded)
            .content(["Standard overlaid title"]),
        Panel::new()
            .top_left("Processes")
            .border(BorderKind::Plain)
            .variant(PanelVariant::InsetTitle)
            .content(["✖ No processes running"]),
    ]
}

fn two_columns(area: Rect) -> [Rect; 2] {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .areas(area)
}

fn data_view_layout(area: Rect) -> [Rect; 2] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Fill(1)])
        .areas(area)
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
