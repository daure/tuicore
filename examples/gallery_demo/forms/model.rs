use time::Date;
use tuicore::{ErrorDisplay, FormBuilder, FormControl, FormGroup, FormModel, FormStatus};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FormError {
    NameRequired,
    NameTooShort,
    DescriptionRequired,
    PasswordRequired,
    PasswordTooShort,
    StartRequired,
    EndRequired,
    EndNotAfterStart,
    EnvironmentRequired,
    TagsTooFew,
    TagsTooMany,
}

impl FormError {
    pub(super) fn message(self) -> &'static str {
        match self {
            Self::NameRequired => "Name is required",
            Self::NameTooShort => "Name must be at least 3 characters",
            Self::DescriptionRequired => "Description is required",
            Self::PasswordRequired => "Password is required",
            Self::PasswordTooShort => "Password must be at least 8 characters",
            Self::StartRequired => "Start date is required",
            Self::EndRequired => "End date is required",
            Self::EndNotAfterStart => "End date must be after start date",
            Self::EnvironmentRequired => "Environment is required",
            Self::TagsTooFew => "Select at least 2 tags",
            Self::TagsTooMany => "Select no more than 4 tags",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ValidatedFormValue {
    name: String,
    description: String,
    password: String,
    start: Option<Date>,
    end: Option<Date>,
    environment: Option<&'static str>,
    tags: Vec<String>,
}

pub(super) struct ValidatedFormControls {
    pub(super) name: FormControl<String, FormError>,
    pub(super) description: FormControl<String, FormError>,
    pub(super) password: FormControl<String, FormError>,
    pub(super) start: FormControl<Option<Date>, FormError>,
    pub(super) end: FormControl<Option<Date>, FormError>,
    pub(super) environment: FormControl<Option<&'static str>, FormError>,
    pub(super) tags: FormControl<Vec<String>, FormError>,
}

impl FormModel<FormError> for ValidatedFormControls {
    type Value = ValidatedFormValue;

    fn value(&self) -> Self::Value {
        ValidatedFormValue {
            name: self.name.value().clone(),
            description: self.description.value().clone(),
            password: self.password.value().clone(),
            start: *self.start.value(),
            end: *self.end.value(),
            environment: *self.environment.value(),
            tags: self.tags.value().clone(),
        }
    }

    fn status(&self) -> FormStatus {
        if self.name.invalid()
            || self.description.invalid()
            || self.password.invalid()
            || self.start.invalid()
            || self.end.invalid()
            || self.environment.invalid()
            || self.tags.invalid()
        {
            FormStatus::Invalid
        } else {
            FormStatus::Valid
        }
    }

    fn validate(&mut self) -> FormStatus {
        self.name.validate();
        self.description.validate();
        self.password.validate();
        self.start.validate();
        self.end.validate();
        self.environment.validate();
        self.tags.validate();
        self.status()
    }

    fn refresh_presented_errors(&mut self) {
        self.name.refresh_presented_errors();
        self.description.refresh_presented_errors();
        self.password.refresh_presented_errors();
        self.start.refresh_presented_errors();
        self.end.refresh_presented_errors();
        self.environment.refresh_presented_errors();
        self.tags.refresh_presented_errors();
    }

    fn reset(&mut self) {
        self.name.reset();
        self.description.reset();
        self.password.reset();
        self.start.reset();
        self.end.reset();
        self.environment.reset();
        self.tags.reset();
    }
}

pub(super) fn build_model() -> FormGroup<ValidatedFormControls, FormError> {
    let controls = ValidatedFormControls {
        name: FormBuilder::control(String::new())
            .validator(|value| value.trim().is_empty().then_some(FormError::NameRequired))
            .validator(|value| {
                (!value.trim().is_empty() && value.trim().chars().count() < 3)
                    .then_some(FormError::NameTooShort)
            }),
        description: FormBuilder::control(String::new()).validator(|value| {
            value
                .trim()
                .is_empty()
                .then_some(FormError::DescriptionRequired)
        }),
        password: FormBuilder::control(String::new())
            .error_display(ErrorDisplay::OnInput)
            .validator(|value| value.is_empty().then_some(FormError::PasswordRequired))
            .validator(|value| {
                (!value.is_empty() && value.chars().count() < 8)
                    .then_some(FormError::PasswordTooShort)
            }),
        start: FormBuilder::control(None)
            .validator(|value| value.is_none().then_some(FormError::StartRequired)),
        end: FormBuilder::control(None)
            .validator(|value| value.is_none().then_some(FormError::EndRequired)),
        environment: FormBuilder::control(None)
            .validator(|value| value.is_none().then_some(FormError::EnvironmentRequired)),
        tags: FormBuilder::control(Vec::new())
            .validator(|value| (value.len() < 2).then_some(FormError::TagsTooFew))
            .validator(|value| (value.len() > 4).then_some(FormError::TagsTooMany)),
    };
    FormBuilder::group(controls).validator(|value| match (value.start, value.end) {
        (Some(start), Some(end)) if end <= start => Some(FormError::EndNotAfterStart),
        _ => None,
    })
}
