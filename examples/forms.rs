use std::time::Duration;

use ratatui::{Frame, layout::Rect};
use tuicore::{
    AnimationSettings, Button, EventCtx, EventOutcome, EventRoute, FocusCtx, FocusId, FocusTarget,
    FormBuilder, FormControl, FormField, FormGroup, FormModel, FormStatus, LayoutCtx,
    LayoutProposal, LayoutResult, LayoutSizeHint, LifecycleCtx, Panel, PanelHost, Paragraph,
    RenderCtx, Split, TextInput, TickResult, TuiEvent, TuiNode,
};

#[derive(Clone, Copy)]
enum FormError {
    NameRequired,
    EmailInvalid,
}

impl FormError {
    fn message(self) -> &'static str {
        match self {
            Self::NameRequired => "Name is required",
            Self::EmailInvalid => "Enter a valid email address",
        }
    }
}

struct Controls {
    name: FormControl<String, FormError>,
    email: FormControl<String, FormError>,
}

impl FormModel<FormError> for Controls {
    type Value = (String, String);

    fn value(&self) -> Self::Value {
        (self.name.value().clone(), self.email.value().clone())
    }

    fn status(&self) -> FormStatus {
        if self.name.invalid() || self.email.invalid() {
            FormStatus::Invalid
        } else {
            FormStatus::Valid
        }
    }

    fn validate(&mut self) -> FormStatus {
        self.name.validate();
        self.email.validate();
        self.status()
    }

    fn refresh_presented_errors(&mut self) {
        self.name.refresh_presented_errors();
        self.email.refresh_presented_errors();
    }

    fn reset(&mut self) {
        self.name.reset();
        self.email.reset();
    }
}

#[derive(Clone)]
enum Msg {
    NameChanged(String),
    EmailChanged(String),
    EditStarted(Field),
    EditEnded(Field),
    Submit,
}

#[derive(Clone, Copy)]
enum Field {
    Name,
    Email,
}

type InputField = FormField<TextInput<Msg>, Msg>;
type FormView = PanelHost<Split<Split<InputField, InputField>, Split<Button<Msg>, Paragraph>>>;

struct FormExample {
    model: FormGroup<Controls, FormError>,
    view: FormView,
}

impl FormExample {
    fn new() -> Self {
        let model = FormBuilder::group(Controls {
            name: FormBuilder::control(String::new())
                .validator(|value| value.trim().is_empty().then_some(FormError::NameRequired)),
            email: FormBuilder::control(String::new()).validator(|value| {
                (!value.contains('@') || !value.contains('.')).then_some(FormError::EmailInvalid)
            }),
        });
        let name = FormField::new(
            "Name",
            TextInput::new()
                .placeholder("Ada Lovelace")
                .on_change(Msg::NameChanged)
                .on_submit(|_| Msg::EditStarted(Field::Name))
                .on_edit_end(|_| Msg::EditEnded(Field::Name)),
        );
        let email = FormField::new(
            "Email",
            TextInput::new()
                .placeholder("ada@example.com")
                .on_change(Msg::EmailChanged)
                .on_submit(|_| Msg::EditStarted(Field::Email))
                .on_edit_end(|_| Msg::EditEnded(Field::Email)),
        );
        let fields = Split::vertical(name, email).ratio(1, 1).gap(1);
        let actions = Split::vertical(
            Button::new("Submit").on_press(|| Msg::Submit),
            Paragraph::new("Enter edits fields • Esc ends editing • Tab moves focus"),
        )
        .ratio(1, 2)
        .gap(1);
        let view = Panel::new()
            .top_left("Reactive form")
            .host(Split::vertical(fields, actions).ratio(3, 1).gap(1));

        Self { model, view }
    }

    fn handle_message(&mut self, message: Msg, ctx: &mut EventCtx<Msg>) {
        match message {
            Msg::NameChanged(value) => self.model.update_controls(|c| c.name.input(value)),
            Msg::EmailChanged(value) => self.model.update_controls(|c| c.email.input(value)),
            Msg::EditStarted(field) => self.model.update_controls(|c| match field {
                Field::Name => c.name.begin_edit(),
                Field::Email => c.email.begin_edit(),
            }),
            Msg::EditEnded(field) => self.model.update_controls(|c| match field {
                Field::Name => c.name.end_edit(),
                Field::Email => c.email.end_edit(),
            }),
            Msg::Submit => {
                let ((name, email), status) = self.model.submit_attempt();
                let text = match status {
                    FormStatus::Valid => format!("Submitted: {name} <{email}>"),
                    FormStatus::Invalid => "Fix validation errors before submitting".to_string(),
                };
                self.status_mut().set_text(text);
            }
        }
        self.sync_chrome();
        ctx.request_layout();
        ctx.request_redraw();
    }

    fn sync_chrome(&mut self) {
        let submitted = self.model.submitted();
        let controls = self.model.controls();
        let name = controls
            .name
            .visible_errors(submitted)
            .first()
            .copied()
            .map(|error| error.message().to_string());
        let email = controls
            .email
            .visible_errors(submitted)
            .first()
            .copied()
            .map(|error| error.message().to_string());
        self.fields_mut().first_mut().set_error(name);
        self.fields_mut().second_mut().set_error(email);
    }

    fn fields_mut(&mut self) -> &mut Split<InputField, InputField> {
        self.view.child_mut().first_mut()
    }

    fn status_mut(&mut self) -> &mut Paragraph {
        self.view.child_mut().second_mut().second_mut()
    }
}

impl TuiNode<Msg> for FormExample {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        self.view.measure(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.view.layout(area, ctx)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        self.view.render(frame, area, ctx);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<Msg>) -> EventOutcome {
        self.view.event(event, ctx)
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<Msg>,
    ) -> EventOutcome {
        self.view.dispatch_event(route, event, ctx)
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.view.tick(dt, settings)
    }

    fn focus(&mut self, target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<Msg>) {
        self.view.focus(target, focused, ctx);
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<Msg>) {
        self.view.dispatch_focus(target, focused, ctx);
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        self.view.init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        self.view.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        self.view.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        self.view.destroy(ctx);
    }
}

fn main() -> tuicore::Result<()> {
    tuicore::init();
    tuicore::TreeApp::new(FormExample::new())
        .on_message(|app, message, ctx| app.handle_message(message, ctx))
        .run()
}
