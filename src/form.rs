//! Typed, synchronous form state and validation primitives.

/// Validation state shared by every form model.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FormStatus {
    #[default]
    Valid,
    Invalid,
}

/// Controls when validation errors become visible after deliberate editing.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ErrorDisplay {
    #[default]
    OnInputExit,
    OnInput,
}

/// Recursive interface implemented by controls and typed form containers.
pub trait FormModel<E> {
    type Value;

    fn value(&self) -> Self::Value;
    fn status(&self) -> FormStatus;
    fn validate(&mut self) -> FormStatus;
    /// Refreshes error payloads presented by this model and its descendants.
    fn refresh_presented_errors(&mut self);
    fn reset(&mut self);
}

type Validator<T, E> = Box<dyn Fn(&T) -> Option<E>>;

/// One typed value with ordered synchronous validators.
pub struct FormControl<T, E> {
    value: T,
    initial: T,
    validators: Vec<Validator<T, E>>,
    errors: Vec<E>,
    presented_errors: Vec<E>,
    error_display: ErrorDisplay,
    dirty: bool,
    touched: bool,
    editing: bool,
    input_during_edit: bool,
}

impl<T: Clone, E: Clone> FormControl<T, E> {
    pub fn new(value: T) -> Self {
        Self {
            initial: value.clone(),
            value,
            validators: Vec::new(),
            errors: Vec::new(),
            presented_errors: Vec::new(),
            error_display: ErrorDisplay::default(),
            dirty: false,
            touched: false,
            editing: false,
            input_during_edit: false,
        }
    }

    pub fn error_display(mut self, error_display: ErrorDisplay) -> Self {
        self.error_display = error_display;
        self
    }

    pub fn validator(mut self, validator: impl Fn(&T) -> Option<E> + 'static) -> Self {
        self.validators.push(Box::new(validator));
        self.validate();
        self.refresh_presented_errors();
        self
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn errors(&self) -> &[E] {
        &self.errors
    }

    /// Last validation errors captured at this control's configured display trigger.
    pub fn presented_errors(&self) -> &[E] {
        &self.presented_errors
    }

    pub fn errors_visible(&self, form_submitted: bool) -> bool {
        self.should_show_errors(form_submitted) && !self.presented_errors.is_empty()
    }

    pub fn visible_errors(&self, form_submitted: bool) -> &[E] {
        if self.errors_visible(form_submitted) {
            &self.presented_errors
        } else {
            &[]
        }
    }

    pub fn dirty(&self) -> bool {
        self.dirty
    }

    pub fn pristine(&self) -> bool {
        !self.dirty
    }

    pub fn touched(&self) -> bool {
        self.touched
    }

    pub fn editing(&self) -> bool {
        self.editing
    }

    pub fn status(&self) -> FormStatus {
        if self.errors.is_empty() {
            FormStatus::Valid
        } else {
            FormStatus::Invalid
        }
    }

    pub fn valid(&self) -> bool {
        self.status() == FormStatus::Valid
    }

    pub fn invalid(&self) -> bool {
        self.status() == FormStatus::Invalid
    }

    pub fn set_value(&mut self, value: T) {
        self.value = value;
        self.validate();
        if !self.editing || self.error_display == ErrorDisplay::OnInput {
            self.refresh_presented_errors();
        }
    }

    pub fn begin_edit(&mut self) {
        if self.editing {
            return;
        }
        self.editing = true;
        self.input_during_edit = false;
    }

    pub fn input(&mut self, value: T) {
        self.begin_edit();
        self.value = value;
        self.input_during_edit = true;
        if self.error_display == ErrorDisplay::OnInput {
            self.dirty = true;
        }
        self.validate();
        if self.error_display == ErrorDisplay::OnInput {
            self.refresh_presented_errors();
        }
    }

    pub fn end_edit(&mut self) {
        if !self.editing {
            return;
        }
        match self.error_display {
            ErrorDisplay::OnInputExit => {
                self.dirty = true;
                self.touched = true;
                self.refresh_presented_errors();
            }
            ErrorDisplay::OnInput if self.input_during_edit => {
                self.touched = true;
            }
            ErrorDisplay::OnInput => {}
        }
        self.editing = false;
        self.input_during_edit = false;
    }

    pub fn should_show_errors(&self, form_submitted: bool) -> bool {
        if form_submitted {
            return true;
        }
        match self.error_display {
            ErrorDisplay::OnInputExit => self.touched,
            ErrorDisplay::OnInput => self.dirty,
        }
    }

    pub fn validate(&mut self) -> FormStatus {
        self.errors = self
            .validators
            .iter()
            .filter_map(|validator| validator(&self.value))
            .collect();
        self.status()
    }

    pub fn refresh_presented_errors(&mut self) {
        self.presented_errors.clone_from(&self.errors);
    }

    pub fn reset(&mut self) {
        self.value = self.initial.clone();
        self.dirty = false;
        self.touched = false;
        self.editing = false;
        self.input_during_edit = false;
        self.validate();
        self.refresh_presented_errors();
    }
}

impl<T: Clone, E: Clone> FormModel<E> for FormControl<T, E> {
    type Value = T;

    fn value(&self) -> Self::Value {
        self.value.clone()
    }

    fn status(&self) -> FormStatus {
        FormControl::status(self)
    }

    fn validate(&mut self) -> FormStatus {
        FormControl::validate(self)
    }

    fn refresh_presented_errors(&mut self) {
        FormControl::refresh_presented_errors(self);
    }

    fn reset(&mut self) {
        FormControl::reset(self);
    }
}

/// Typed controls with group-owned cross-field validation.
pub struct FormGroup<C, E>
where
    C: FormModel<E>,
{
    controls: C,
    validators: Vec<Validator<C::Value, E>>,
    errors: Vec<E>,
    presented_errors: Vec<E>,
    presented_errors_visible: bool,
    submitted: bool,
}

impl<C, E: Clone> FormGroup<C, E>
where
    C: FormModel<E>,
{
    pub fn new(mut controls: C) -> Self {
        controls.validate();
        Self {
            controls,
            validators: Vec::new(),
            errors: Vec::new(),
            presented_errors: Vec::new(),
            presented_errors_visible: false,
            submitted: false,
        }
    }

    pub fn validator(mut self, validator: impl Fn(&C::Value) -> Option<E> + 'static) -> Self {
        self.validators.push(Box::new(validator));
        self.validate();
        self
    }

    pub fn controls(&self) -> &C {
        &self.controls
    }

    pub fn update_controls<R>(&mut self, update: impl FnOnce(&mut C) -> R) -> R {
        let result = update(&mut self.controls);
        self.validate();
        result
    }

    pub fn errors(&self) -> &[E] {
        &self.errors
    }

    /// Last group-owned errors captured by an explicit presentation trigger.
    pub fn presented_errors(&self) -> &[E] {
        &self.presented_errors
    }

    pub fn errors_visible(&self) -> bool {
        self.presented_errors_visible && !self.presented_errors.is_empty()
    }

    pub fn visible_errors(&self) -> &[E] {
        if self.errors_visible() {
            &self.presented_errors
        } else {
            &[]
        }
    }

    pub fn submitted(&self) -> bool {
        self.submitted
    }

    pub fn status(&self) -> FormStatus {
        if self.controls.status() == FormStatus::Invalid || !self.errors.is_empty() {
            FormStatus::Invalid
        } else {
            FormStatus::Valid
        }
    }

    pub fn valid(&self) -> bool {
        self.status() == FormStatus::Valid
    }

    pub fn invalid(&self) -> bool {
        self.status() == FormStatus::Invalid
    }

    pub fn validate(&mut self) -> FormStatus {
        self.controls.validate();
        let value = self.controls.value();
        self.errors = self
            .validators
            .iter()
            .filter_map(|validator| validator(&value))
            .collect();
        self.status()
    }

    pub fn refresh_presented_errors(&mut self) {
        self.presented_errors.clone_from(&self.errors);
        self.presented_errors_visible = true;
    }

    pub fn submit_attempt(&mut self) -> (C::Value, FormStatus) {
        self.submitted = true;
        let status = self.validate();
        self.controls.refresh_presented_errors();
        self.refresh_presented_errors();
        (self.controls.value(), status)
    }

    pub fn reset(&mut self) {
        self.submitted = false;
        self.controls.reset();
        self.validate();
        self.presented_errors.clone_from(&self.errors);
        self.presented_errors_visible = false;
    }
}

impl<C, E: Clone> FormModel<E> for FormGroup<C, E>
where
    C: FormModel<E>,
{
    type Value = C::Value;

    fn value(&self) -> Self::Value {
        self.controls.value()
    }

    fn status(&self) -> FormStatus {
        FormGroup::status(self)
    }

    fn validate(&mut self) -> FormStatus {
        FormGroup::validate(self)
    }

    fn refresh_presented_errors(&mut self) {
        self.controls.refresh_presented_errors();
        FormGroup::refresh_presented_errors(self);
    }

    fn reset(&mut self) {
        FormGroup::reset(self);
    }
}

/// Homogeneous collection of recursive form models.
pub struct FormArray<C, E> {
    controls: Vec<C>,
    marker: std::marker::PhantomData<fn() -> E>,
}

impl<C, E> FormArray<C, E>
where
    C: FormModel<E>,
{
    pub fn new(mut controls: Vec<C>) -> Self {
        for control in &mut controls {
            control.validate();
        }
        Self {
            controls,
            marker: std::marker::PhantomData,
        }
    }

    pub fn controls(&self) -> &[C] {
        &self.controls
    }

    pub fn controls_mut(&mut self) -> &mut [C] {
        &mut self.controls
    }

    pub fn get(&self, index: usize) -> Option<&C> {
        self.controls.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut C> {
        self.controls.get_mut(index)
    }

    pub fn len(&self) -> usize {
        self.controls.len()
    }

    pub fn is_empty(&self) -> bool {
        self.controls.is_empty()
    }

    pub fn push(&mut self, mut control: C) {
        control.validate();
        self.controls.push(control);
    }

    pub fn remove(&mut self, index: usize) -> C {
        self.controls.remove(index)
    }

    pub fn status(&self) -> FormStatus {
        if self
            .controls
            .iter()
            .any(|control| control.status() == FormStatus::Invalid)
        {
            FormStatus::Invalid
        } else {
            FormStatus::Valid
        }
    }

    pub fn valid(&self) -> bool {
        self.status() == FormStatus::Valid
    }

    pub fn invalid(&self) -> bool {
        self.status() == FormStatus::Invalid
    }

    pub fn reset(&mut self) {
        for control in &mut self.controls {
            control.reset();
        }
    }
}

impl<C, E> FormModel<E> for FormArray<C, E>
where
    C: FormModel<E>,
{
    type Value = Vec<C::Value>;

    fn value(&self) -> Self::Value {
        self.controls.iter().map(FormModel::value).collect()
    }

    fn status(&self) -> FormStatus {
        FormArray::status(self)
    }

    fn validate(&mut self) -> FormStatus {
        for control in &mut self.controls {
            control.validate();
        }
        self.status()
    }

    fn refresh_presented_errors(&mut self) {
        for control in &mut self.controls {
            control.refresh_presented_errors();
        }
    }

    fn reset(&mut self) {
        FormArray::reset(self);
    }
}

/// Zero-sized factory for form models.
#[derive(Clone, Copy, Debug, Default)]
pub struct FormBuilder;

impl FormBuilder {
    pub fn control<T: Clone, E: Clone>(value: T) -> FormControl<T, E> {
        FormControl::new(value)
    }

    pub fn group<C, E: Clone>(controls: C) -> FormGroup<C, E>
    where
        C: FormModel<E>,
    {
        FormGroup::new(controls)
    }

    pub fn array<C, E>(controls: Vec<C>) -> FormArray<C, E>
    where
        C: FormModel<E>,
    {
        FormArray::new(controls)
    }
}
