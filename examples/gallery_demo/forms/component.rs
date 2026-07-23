use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::Paragraph;
use time::Date;
use tuicore::{
    AnimationSettings, AxisProposal, ChildKey, DatePickerDropdown, Dropdown, EventCtx,
    EventOutcome, EventRoute, FocusCtx, FocusTarget, FormControl, FormField, FormGroup, FormStatus,
    Key, KeyEvent, KeyModifiers, LayoutCtx, LayoutProposal, LayoutResult, LifecycleCtx,
    PasswordInput, RenderCtx, TagInput, TagInputEvent, TextInput, TextareaInput, TickResult,
    TuiEvent, TuiNode,
};

use super::super::super::Msg;
use super::model::{FormError, ValidatedFormControls, build_model};

type TextField = FormField<TextInput<Msg>, Msg>;
type PasswordField = FormField<PasswordInput<Msg>, Msg>;
type TextareaField = FormField<TextareaInput<Msg>, Msg>;
type DateField = FormField<DatePickerDropdown<Msg>, Msg>;
type DropdownField = FormField<Dropdown<EnvironmentOption, &'static str>, Msg>;
type TagField = FormField<TagInput, Msg>;

#[derive(Clone)]
pub(super) struct EnvironmentOption {
    id: &'static str,
    value: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FormControlId {
    Name,
    Description,
    Password,
    Start,
    End,
    Environment,
    Tags,
}

impl FormControlId {
    pub(super) const ALL: [Self; 7] = [
        Self::Name,
        Self::Description,
        Self::Password,
        Self::Start,
        Self::End,
        Self::Environment,
        Self::Tags,
    ];

    pub(super) fn key(self) -> ChildKey {
        ChildKey::new(match self {
            Self::Name => "validated-form-name",
            Self::Description => "validated-form-description",
            Self::Password => "validated-form-password",
            Self::Start => "validated-form-start",
            Self::End => "validated-form-end",
            Self::Environment => "validated-form-environment",
            Self::Tags => "validated-form-tags",
        })
    }

    fn from_key(key: &ChildKey) -> Option<Self> {
        Self::ALL.into_iter().find(|field| field.key() == *key)
    }
}

#[derive(Clone, Copy, Default)]
struct FormAreas {
    instructions: Rect,
    name: Rect,
    description: Rect,
    password: Rect,
    start: Rect,
    end: Rect,
    environment: Rect,
    tags: Rect,
    status: Rect,
}

impl FormAreas {
    fn field(self, control: FormControlId) -> Rect {
        match control {
            FormControlId::Name => self.name,
            FormControlId::Description => self.description,
            FormControlId::Password => self.password,
            FormControlId::Start => self.start,
            FormControlId::End => self.end,
            FormControlId::Environment => self.environment,
            FormControlId::Tags => self.tags,
        }
    }
}

pub(crate) struct ValidatedForm {
    pub(super) model: FormGroup<ValidatedFormControls, FormError>,
    pub(super) name: TextField,
    pub(super) description: TextareaField,
    pub(super) password: PasswordField,
    pub(super) start: DateField,
    pub(super) end: DateField,
    pub(super) environment: DropdownField,
    pub(super) tags: TagField,
    pub(super) status: String,
    areas: FormAreas,
}

impl ValidatedForm {
    pub(crate) fn new() -> Self {
        Self {
            model: build_model(),
            name: FormField::new(
                "Name",
                TextInput::new()
                    .placeholder("Release title")
                    .on_change(Msg::FormNameChanged)
                    .on_edit_end(|_| Msg::FormControlEditEnded(FormControlId::Name))
                    .on_submit(|_| Msg::FormSubmitRequested(FormControlId::Name)),
            ),
            description: FormField::new(
                "Description",
                TextareaInput::new()
                    .placeholder("What will ship?")
                    .min_rows(2)
                    .max_rows(3)
                    .on_change(Msg::FormDescriptionChanged)
                    .on_edit_end(|_| Msg::FormControlEditEnded(FormControlId::Description))
                    .on_submit(|_| Msg::FormSubmitRequested(FormControlId::Description)),
            ),
            password: FormField::new(
                "Password",
                PasswordInput::new()
                    .placeholder("At least 8 characters")
                    .on_change(Msg::FormPasswordChanged)
                    .on_edit_end(|_| Msg::FormControlEditEnded(FormControlId::Password))
                    .on_submit(|_| Msg::FormSubmitRequested(FormControlId::Password)),
            ),
            start: FormField::new(
                "Start date",
                DatePickerDropdown::new()
                    .today(demo_date())
                    .placeholder("Choose start")
                    .on_submit(|| Msg::FormSubmitRequested(FormControlId::Start))
                    .on_select(Msg::FormStartSelected),
            ),
            end: FormField::new(
                "End date",
                DatePickerDropdown::new()
                    .today(demo_date())
                    .placeholder("Choose end")
                    .on_submit(|| Msg::FormSubmitRequested(FormControlId::End))
                    .on_select(Msg::FormEndSelected),
            ),
            environment: FormField::new(
                "",
                Dropdown::single(
                    [
                        EnvironmentOption {
                            id: "dev",
                            value: "Development",
                        },
                        EnvironmentOption {
                            id: "stage",
                            value: "Staging",
                        },
                        EnvironmentOption {
                            id: "prod",
                            value: "Production",
                        },
                    ],
                    |option| option.id,
                    |option| option.value.to_string(),
                )
                .label("Environment")
                .placeholder("Choose environment"),
            )
            .embedded(),
            tags: FormField::new(
                "Tags (2–4)",
                TagInput::new(["frontend", "backend", "api", "docs", "urgent", "qa"])
                    .placeholder("Add 2–4 tags"),
            ),
            status: "Not submitted • Ctrl+Enter submits outside textarea".to_string(),
            areas: FormAreas::default(),
        }
    }

    pub(crate) fn apply_message(&mut self, message: &Msg) -> bool {
        match message {
            Msg::FormNameChanged(value) => self.change_name(value.clone()),
            Msg::FormDescriptionChanged(value) => self.change_description(value.clone()),
            Msg::FormPasswordChanged(value) => self.change_password(value.clone()),
            Msg::FormStartSelected(value) => self.select_start(*value),
            Msg::FormEndSelected(value) => self.select_end(*value),
            Msg::FormSubmitAttempt => self.submit_attempt(),
            Msg::FormSubmitRequested(control) => self.begin_edit(*control),
            Msg::FormControlEditEnded(control) => self.end_edit(*control),
            _ => return false,
        }
        true
    }

    pub(crate) fn change_name(&mut self, value: String) {
        self.model
            .update_controls(|controls| controls.name.input(value));
        self.sync_errors();
    }

    pub(crate) fn change_description(&mut self, value: String) {
        self.model
            .update_controls(|controls| controls.description.input(value));
        self.sync_errors();
    }

    pub(crate) fn change_password(&mut self, value: String) {
        self.model
            .update_controls(|controls| controls.password.input(value));
        self.sync_errors();
    }

    pub(crate) fn select_start(&mut self, value: Date) {
        self.model
            .update_controls(|controls| controls.start.input(Some(value)));
        self.sync_errors();
    }

    pub(crate) fn select_end(&mut self, value: Date) {
        self.model
            .update_controls(|controls| controls.end.input(Some(value)));
        self.sync_errors();
    }

    pub(crate) fn begin_edit(&mut self, control: FormControlId) {
        self.model.update_controls(|controls| match control {
            FormControlId::Name => controls.name.begin_edit(),
            FormControlId::Description => controls.description.begin_edit(),
            FormControlId::Password => controls.password.begin_edit(),
            FormControlId::Start => controls.start.begin_edit(),
            FormControlId::End => controls.end.begin_edit(),
            FormControlId::Environment => controls.environment.begin_edit(),
            FormControlId::Tags => controls.tags.begin_edit(),
        });
    }

    pub(crate) fn end_edit(&mut self, control: FormControlId) {
        self.model.update_controls(|controls| match control {
            FormControlId::Name => controls.name.end_edit(),
            FormControlId::Description => controls.description.end_edit(),
            FormControlId::Password => controls.password.end_edit(),
            FormControlId::Start => controls.start.end_edit(),
            FormControlId::End => controls.end.end_edit(),
            FormControlId::Environment => controls.environment.end_edit(),
            FormControlId::Tags => controls.tags.end_edit(),
        });
        self.model.refresh_presented_errors();
        self.sync_errors();
    }

    pub(crate) fn submit_attempt(&mut self) {
        let (_, status) = self.model.submit_attempt();
        self.status = match status {
            FormStatus::Valid => "Submitted • valid",
            FormStatus::Invalid => "Submitted • invalid",
        }
        .to_string();
        self.sync_errors();
    }

    fn update_environment(&mut self, value: Option<&'static str>) {
        self.model
            .update_controls(|controls| controls.environment.input(value));
        self.sync_errors();
    }

    pub(super) fn sync_tags_from_component(&mut self) {
        let tags = self
            .tags
            .child()
            .selected_tags()
            .iter()
            .map(|tag| tag.label().to_string())
            .collect();
        self.model
            .update_controls(|controls| controls.tags.input(tags));
        self.sync_errors();
    }

    fn sync_errors(&mut self) {
        let submitted = self.model.submitted();
        let controls = self.model.controls();
        let name = visible_control_error(&controls.name, submitted);
        let description = visible_control_error(&controls.description, submitted);
        let password = visible_control_error(&controls.password, submitted);
        let start = visible_control_error(&controls.start, submitted);
        let end = visible_control_error(&controls.end, submitted).or_else(|| {
            controls
                .end
                .should_show_errors(submitted)
                .then(|| self.model.presented_errors().first().copied())
                .flatten()
                .map(|error| error.message().to_string())
        });
        let environment = visible_control_error(&controls.environment, submitted);
        let tags = visible_control_error(&controls.tags, submitted);
        self.name.set_error(name);
        self.description.set_error(description);
        self.password.set_error(password);
        self.start.set_error(start);
        self.end.set_error(end);
        self.environment.set_error(environment);
        let environment_has_error = self.environment.error().is_some();
        self.environment
            .child_mut()
            .set_error(environment_has_error);
        self.tags.set_error(tags);
    }

    fn field_mut(&mut self, control: FormControlId) -> &mut dyn TuiNode<Msg> {
        match control {
            FormControlId::Name => &mut self.name,
            FormControlId::Description => &mut self.description,
            FormControlId::Password => &mut self.password,
            FormControlId::Start => &mut self.start,
            FormControlId::End => &mut self.end,
            FormControlId::Environment => &mut self.environment,
            FormControlId::Tags => &mut self.tags,
        }
    }

    fn field(&self, control: FormControlId) -> &dyn TuiNode<Msg> {
        match control {
            FormControlId::Name => &self.name,
            FormControlId::Description => &self.description,
            FormControlId::Password => &self.password,
            FormControlId::Start => &self.start,
            FormControlId::End => &self.end,
            FormControlId::Environment => &self.environment,
            FormControlId::Tags => &self.tags,
        }
    }

    fn popup_is_open(&self, control: FormControlId) -> bool {
        match control {
            FormControlId::Start => self.start.child().is_open(),
            FormControlId::End => self.end.child().is_open(),
            FormControlId::Environment => self.environment.child().is_open(),
            _ => false,
        }
    }

    pub(crate) fn control_is_active(&self, control: FormControlId) -> bool {
        match control {
            FormControlId::Name => self.name.child().insert_mode(),
            FormControlId::Description => self.description.child().insert_mode(),
            FormControlId::Password => self.password.child().insert_mode(),
            FormControlId::Start => self.start.child().is_open(),
            FormControlId::End => self.end.child().is_open(),
            FormControlId::Environment => self.environment.child().is_open(),
            FormControlId::Tags => self.tags.child().is_active(),
        }
    }

    pub(crate) fn route_has_active_control(&self, route: &EventRoute) -> bool {
        route
            .path
            .first()
            .and_then(FormControlId::from_key)
            .is_some_and(|control| self.control_is_active(control))
    }

    fn handle_form_navigation(
        &self,
        control: FormControlId,
        event: &TuiEvent,
        ctx: &mut EventCtx<Msg>,
    ) -> bool {
        let TuiEvent::Key(KeyEvent { code, modifiers }) = event else {
            return false;
        };
        if *modifiers != KeyModifiers::CONTROL || self.control_is_active(control) {
            return false;
        }
        let direction = match code {
            Key::Char('h' | 'k') => -1,
            Key::Char('j' | 'l') => 1,
            _ => return false,
        };
        let index = FormControlId::ALL
            .iter()
            .position(|candidate| *candidate == control)
            .expect("form control belongs to ALL");
        if direction < 0 && index > 0 {
            ctx.focus_previous();
        } else if direction > 0 && index + 1 < FormControlId::ALL.len() {
            ctx.focus_next();
        }
        ctx.stop_propagation();
        true
    }
}

fn visible_control_error<T: Clone>(
    control: &FormControl<T, FormError>,
    submitted: bool,
) -> Option<String> {
    control
        .visible_errors(submitted)
        .first()
        .copied()
        .map(|error| error.message().to_string())
}

pub(super) fn demo_date() -> Date {
    Date::from_calendar_date(2026, time::Month::July, 16).expect("demo date should be valid")
}

impl TuiNode<Msg> for ValidatedForm {
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.areas = solve_form_areas(self, area);
        for control in FormControlId::ALL {
            let field_area = self.areas.field(control);
            ctx.push_slot(control.key(), field_area, |ctx| {
                self.field_mut(control).layout(field_area, ctx);
            });
        }
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut Frame, _area: Rect, ctx: &mut RenderCtx<'a>) {
        frame.render_widget(
            Paragraph::new(
                "Ctrl+Enter submits • in textarea, Ctrl+Enter exits input without submitting",
            ),
            self.areas.instructions,
        );
        for control in FormControlId::ALL {
            self.field(control)
                .render(frame, self.areas.field(control), ctx);
        }
        let color = if self.model.submitted() && self.model.valid() {
            tuicore::theme().success_fg()
        } else if self.model.submitted() {
            tuicore::theme().error_fg()
        } else {
            tuicore::theme().muted_fg()
        };
        frame.render_widget(
            Paragraph::new(self.status.as_str()).style(Style::default().fg(color)),
            self.areas.status,
        );
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<Msg>,
    ) -> EventOutcome {
        let Some(control) = route.path.first().and_then(FormControlId::from_key) else {
            return EventOutcome::Ignored;
        };
        if self.handle_form_navigation(control, event, ctx) {
            return EventOutcome::Handled;
        }
        let child_route = EventRoute::new(route.path.without_first());
        let submit_shortcut = matches!(
            event,
            TuiEvent::Key(KeyEvent {
                code: Key::Enter,
                modifiers: KeyModifiers::CONTROL,
            })
        );
        if submit_shortcut && control != FormControlId::Description {
            ctx.emit(Msg::FormSubmitAttempt);
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        match control {
            FormControlId::Start | FormControlId::End => {
                let was_open = self.popup_is_open(control);
                let outcome = self
                    .field_mut(control)
                    .dispatch_event(&child_route, event, ctx);
                if was_open && !self.popup_is_open(control) {
                    ctx.emit(Msg::FormControlEditEnded(control));
                }
                outcome
            }
            FormControlId::Environment => self.dispatch_environment(&child_route, event, ctx),
            FormControlId::Tags => self.dispatch_tags(&child_route, event, ctx),
            _ => self
                .field_mut(control)
                .dispatch_event(&child_route, event, ctx),
        }
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<Msg>) {
        let Some((first, control)) = target
            .path
            .first()
            .and_then(|key| FormControlId::from_key(key).map(|control| (key, control)))
        else {
            return;
        };
        let was_active = match control {
            FormControlId::Start => self.start.child().is_open(),
            FormControlId::End => self.end.child().is_open(),
            FormControlId::Environment => self.environment.child().is_open(),
            FormControlId::Tags => self.tags.child().is_active(),
            _ => false,
        };
        if let Some(child_target) = target.for_child(first) {
            self.field_mut(control)
                .dispatch_focus(&child_target, focused, ctx);
        }
        if !focused && was_active && !self.popup_is_open(control) {
            ctx.emit(Msg::FormControlEditEnded(control));
        }
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        FormControlId::ALL
            .into_iter()
            .fold(TickResult::IDLE, |result, control| {
                result.merge(self.field_mut(control).tick(dt, settings))
            })
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        for control in FormControlId::ALL {
            self.field_mut(control).init(ctx);
        }
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        for control in FormControlId::ALL {
            self.field_mut(control).mount(ctx);
        }
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        for control in FormControlId::ALL {
            self.field_mut(control).unmount(ctx);
        }
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        for control in FormControlId::ALL {
            self.field_mut(control).destroy(ctx);
        }
    }
}

impl ValidatedForm {
    fn dispatch_environment(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<Msg>,
    ) -> EventOutcome {
        let was_open = self.environment.child().is_open();
        let selected = self.environment.child().selected_id();
        let outcome = self.environment.dispatch_event(route, event, ctx);
        let is_open = self.environment.child().is_open();
        let next_selected = self.environment.child().selected_id();
        if !was_open
            && is_open
            && matches!(event, TuiEvent::Key(key) if key.modifiers.is_empty() && key.code == Key::Enter)
        {
            ctx.emit(Msg::FormSubmitRequested(FormControlId::Environment));
            ctx.request_layout();
            ctx.request_redraw();
        }
        if selected != next_selected {
            self.update_environment(next_selected);
            ctx.request_layout();
            ctx.request_redraw();
        }
        if was_open && !is_open {
            ctx.emit(Msg::FormControlEditEnded(FormControlId::Environment));
        }
        outcome
    }

    fn dispatch_tags(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<Msg>,
    ) -> EventOutcome {
        let was_active = self.tags.child().is_active();
        let outcome = self.tags.dispatch_event(route, event, ctx);
        let is_active = self.tags.child().is_active();
        let events = self.tags.child_mut().take_events();
        if events.contains(&TagInputEvent::SubmitRequested) {
            ctx.emit(Msg::FormSubmitRequested(FormControlId::Tags));
        }
        let value_changed = events.iter().any(|event| {
            !matches!(
                event,
                TagInputEvent::QueryChanged { .. } | TagInputEvent::SubmitRequested
            )
        });
        if value_changed {
            self.sync_tags_from_component();
            ctx.request_layout();
            ctx.request_redraw();
        }
        if was_active && !is_active {
            ctx.emit(Msg::FormControlEditEnded(FormControlId::Tags));
        }
        outcome
    }
}

// Flex/Grid own homogeneous child collections. This solver keeps typed field access while
// isolating dynamic FormField measurement and the only raw Layout usage.
fn solve_form_areas(form: &ValidatedForm, area: Rect) -> FormAreas {
    let proposal = |width| LayoutProposal {
        width: AxisProposal::Exact(width),
        height: AxisProposal::Unbounded,
    };
    let half_width = area.width / 2;
    let name_height = form.name.measure(proposal(area.width)).preferred.height;
    let description_height = form
        .description
        .measure(proposal(area.width))
        .preferred
        .height;
    let password_height = form.password.measure(proposal(area.width)).preferred.height;
    let dates_height = form
        .start
        .measure(proposal(half_width))
        .preferred
        .height
        .max(form.end.measure(proposal(half_width)).preferred.height);
    let environment_height = form
        .environment
        .measure(proposal(area.width))
        .preferred
        .height;
    let tags_height = form.tags.measure(proposal(area.width)).preferred.height;
    let rows: [Rect; 9] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(name_height),
            Constraint::Length(description_height),
            Constraint::Length(password_height),
            Constraint::Length(dates_height),
            Constraint::Length(environment_height),
            Constraint::Length(tags_height),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .areas(area);
    let dates: [Rect; 2] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .areas(rows[4]);
    FormAreas {
        instructions: rows[0],
        name: rows[1],
        description: rows[2],
        password: rows[3],
        start: dates[0],
        end: dates[1],
        environment: rows[5],
        tags: rows[6],
        status: rows[7],
    }
}
