use tuicore::{
    ErrorDisplay, FormArray, FormBuilder, FormControl, FormGroup, FormModel, FormStatus,
};

#[derive(Clone, Debug, PartialEq)]
struct Credentials {
    password: String,
    confirmation: String,
}

struct CredentialControls {
    password: FormControl<String, &'static str>,
    confirmation: FormControl<String, &'static str>,
}

impl FormModel<&'static str> for CredentialControls {
    type Value = Credentials;

    fn value(&self) -> Self::Value {
        Credentials {
            password: self.password.value().clone(),
            confirmation: self.confirmation.value().clone(),
        }
    }

    fn status(&self) -> FormStatus {
        if self.password.invalid() || self.confirmation.invalid() {
            FormStatus::Invalid
        } else {
            FormStatus::Valid
        }
    }

    fn validate(&mut self) -> FormStatus {
        self.password.validate();
        self.confirmation.validate();
        self.status()
    }

    fn refresh_presented_errors(&mut self) {
        self.password.refresh_presented_errors();
        self.confirmation.refresh_presented_errors();
    }

    fn reset(&mut self) {
        self.password.reset();
        self.confirmation.reset();
    }
}

fn credentials() -> CredentialControls {
    CredentialControls {
        password: FormBuilder::control(String::new())
            .validator(|value| (value.len() < 4).then_some("password too short")),
        confirmation: FormBuilder::control(String::new()),
    }
}

#[test]
fn validators_run_during_control_initialization() {
    let control = FormControl::new(2)
        .validator(|value| (*value < 5).then_some("too small"))
        .validator(|value| (*value % 2 == 0).then_some("must be odd"));

    assert_eq!(control.status(), FormStatus::Invalid);
    assert_eq!(control.errors(), &["too small", "must be odd"]);
}

#[test]
fn activation_enters_editing_but_remains_pristine_and_hides_errors() {
    let mut control =
        FormControl::new(String::new()).validator(|value| value.is_empty().then_some("required"));

    control.begin_edit();

    assert!(control.editing());
    assert!(control.pristine());
    assert!(!control.touched());
    assert!(!control.should_show_errors(false));
}

#[test]
fn default_input_without_begin_edit_stays_pristine_until_input_exit() {
    let mut control =
        FormControl::new(String::new()).validator(|value| (value.len() < 3).then_some("too short"));

    control.input("x".into());
    assert!(control.editing());
    assert!(control.pristine());
    assert!(!control.touched());
    assert!(!control.should_show_errors(false));

    control.end_edit();
    assert!(control.dirty());
    assert!(control.touched());
    assert!(!control.editing());
    assert!(control.should_show_errors(false));
}

#[test]
fn default_input_exit_after_activation_marks_unchanged_control() {
    let mut control =
        FormControl::new(String::new()).validator(|value| value.is_empty().then_some("required"));

    control.begin_edit();
    control.end_edit();

    assert!(control.dirty());
    assert!(control.touched());
    assert!(control.should_show_errors(false));
}

#[test]
fn input_exit_without_activation_changes_nothing() {
    let mut control =
        FormControl::new(String::new()).validator(|value| value.is_empty().then_some("required"));

    control.end_edit();

    assert!(control.pristine());
    assert!(!control.touched());
    assert!(!control.should_show_errors(false));
}

#[test]
fn on_input_reveals_first_invalid_input() {
    let mut control = FormControl::new(String::new())
        .error_display(ErrorDisplay::OnInput)
        .validator(|value| (value.len() < 3).then_some("too short"));

    control.begin_edit();
    assert!(!control.should_show_errors(false));
    control.input("x".into());

    assert!(control.dirty());
    assert!(!control.touched());
    assert!(control.should_show_errors(false));
}

#[test]
fn repeated_begin_edit_preserves_visible_on_input_error() {
    let mut control = FormControl::new(String::new())
        .error_display(ErrorDisplay::OnInput)
        .validator(|value| (value.len() < 3).then_some("too short"));

    control.input("x".into());
    assert!(control.should_show_errors(false));

    control.begin_edit();

    assert!(control.editing());
    assert!(control.should_show_errors(false));
}

#[test]
fn on_input_error_remains_visible_during_second_edit_session() {
    let mut control = FormControl::new(String::new())
        .error_display(ErrorDisplay::OnInput)
        .validator(|value| (value.len() < 3).then_some("too short"));

    control.input("x".into());
    control.end_edit();
    assert!(control.should_show_errors(false));

    control.begin_edit();
    assert!(control.should_show_errors(false));

    control.input("y".into());
    assert!(control.should_show_errors(false));
}

#[test]
fn on_input_exit_error_remains_visible_during_second_edit_session() {
    let mut control =
        FormControl::new(String::new()).validator(|value| (value.len() < 3).then_some("too short"));

    control.begin_edit();
    control.end_edit();
    assert!(control.should_show_errors(false));

    control.begin_edit();

    assert!(control.editing());
    assert!(control.should_show_errors(false));
}

#[test]
fn on_input_exit_keeps_presented_error_during_valid_and_invalid_edits() {
    let mut control = FormControl::new(String::new())
        .validator(|value| value.is_empty().then_some("required"))
        .validator(|value| (!value.is_empty() && value.len() < 3).then_some("too short"));
    control.begin_edit();
    control.end_edit();
    assert_eq!(control.visible_errors(false), &["required"]);

    control.input("x".into());
    assert_eq!(control.errors(), &["too short"]);
    assert_eq!(control.presented_errors(), &["required"]);
    control.end_edit();
    assert_eq!(control.visible_errors(false), &["too short"]);

    control.input("valid".into());
    assert!(control.errors().is_empty());
    assert_eq!(control.presented_errors(), &["too short"]);
    control.end_edit();
    assert!(control.visible_errors(false).is_empty());
}

#[test]
fn on_input_refreshes_presented_errors_immediately() {
    let mut control = FormControl::new(String::new())
        .error_display(ErrorDisplay::OnInput)
        .validator(|value| (value.len() < 3).then_some("too short"));

    control.input("x".into());
    assert_eq!(control.visible_errors(false), &["too short"]);
    control.input("valid".into());
    assert!(control.presented_errors().is_empty());
    assert!(!control.errors_visible(false));
}

#[test]
fn validation_recomputes_errors_in_validator_order() {
    let mut control = FormControl::new(0)
        .validator(|value| (*value <= 0).then_some("not positive"))
        .validator(|value| (*value % 2 == 0).then_some("even"));

    assert_eq!(control.errors(), &["not positive", "even"]);
    control.set_value(3);
    assert!(control.errors().is_empty());
    control.set_value(2);
    assert_eq!(control.errors(), &["even"]);
}

#[test]
fn programmatic_set_and_reset_refresh_presented_payloads_without_revealing_reset_errors() {
    let mut control =
        FormControl::new(String::new()).validator(|value| value.is_empty().then_some("required"));
    control.set_value("valid".into());
    assert!(control.presented_errors().is_empty());

    control.reset();
    assert_eq!(control.presented_errors(), &["required"]);
    assert!(!control.errors_visible(false));
}

#[test]
fn set_value_during_on_input_exit_edit_keeps_presented_error_until_exit() {
    let mut control = FormControl::new(String::new())
        .validator(|value| value.is_empty().then_some("required"))
        .validator(|value| (!value.is_empty() && value.len() < 3).then_some("too short"));
    control.begin_edit();
    control.end_edit();
    assert_eq!(control.visible_errors(false), &["required"]);

    control.begin_edit();
    control.set_value("x".into());

    assert_eq!(control.errors(), &["too short"]);
    assert_eq!(control.presented_errors(), &["required"]);
    control.end_edit();
    assert_eq!(control.visible_errors(false), &["too short"]);
}

#[test]
fn group_aggregates_child_status_and_owns_cross_field_errors() {
    let mut group = FormGroup::new(credentials())
        .validator(|value| (value.password != value.confirmation).then_some("passwords differ"));

    assert_eq!(group.status(), FormStatus::Invalid);
    assert!(group.errors().is_empty());
    assert_eq!(group.controls().password.errors(), &["password too short"]);

    group.update_controls(|controls| controls.password.set_value("valid".into()));
    assert_eq!(group.errors(), &["passwords differ"]);
    assert!(group.controls().password.errors().is_empty());
    assert!(group.controls().confirmation.errors().is_empty());

    group.update_controls(|controls| controls.confirmation.set_value("valid".into()));
    assert!(group.errors().is_empty());

    group.update_controls(|controls| controls.confirmation.set_value("different".into()));
    assert_eq!(group.errors(), &["passwords differ"]);
}

#[test]
fn group_errors_remain_presented_until_explicit_trigger() {
    let mut group = FormGroup::new(credentials())
        .validator(|value| (value.password != value.confirmation).then_some("passwords differ"));
    group.update_controls(|controls| controls.password.set_value("valid".into()));
    group.refresh_presented_errors();
    assert_eq!(group.visible_errors(), &["passwords differ"]);

    group.update_controls(|controls| controls.confirmation.set_value("valid".into()));
    assert!(group.errors().is_empty());
    assert_eq!(group.presented_errors(), &["passwords differ"]);
    group.refresh_presented_errors();
    assert!(group.visible_errors().is_empty());
}

#[test]
fn nested_groups_aggregate_status() {
    let inner = FormGroup::new(credentials());
    let outer = FormGroup::<_, &'static str>::new(inner);

    assert!(outer.invalid());
}

#[test]
fn array_status_tracks_push_and_remove() {
    let valid = FormControl::new(2).validator(|value| (*value < 0).then_some("negative"));
    let invalid = FormControl::new(-1).validator(|value| (*value < 0).then_some("negative"));
    let mut array = FormArray::new(vec![valid]);

    assert!(array.valid());
    array.push(invalid);
    assert!(array.invalid());
    let removed = array.remove(1);
    assert!(removed.invalid());
    assert!(array.valid());
}

#[test]
fn submit_reveals_all_without_touching_or_dirtying_controls() {
    let mut group = FormBuilder::group(credentials());
    group.update_controls(|controls| controls.password.begin_edit());

    let (value, status) = group.submit_attempt();

    assert_eq!(
        value,
        Credentials {
            password: String::new(),
            confirmation: String::new(),
        }
    );
    assert_eq!(status, FormStatus::Invalid);
    assert!(group.submitted());
    assert!(
        group
            .controls()
            .password
            .should_show_errors(group.submitted())
    );
    assert!(group.controls().password.pristine());
    assert!(!group.controls().password.touched());
    assert!(group.controls().password.editing());
    assert!(group.controls().confirmation.pristine());
    assert!(!group.controls().confirmation.touched());
    assert_eq!(
        group.controls().password.presented_errors(),
        &["password too short"]
    );
}

#[test]
fn reset_restores_nested_descendant_values_and_clears_edit_state_and_submission() {
    let mut control = FormControl::<_, ()>::new("initial");
    control.begin_edit();
    control.input("changed");
    control.end_edit();
    control.reset();
    assert_eq!(control.value(), &"initial");
    assert!(control.pristine());
    assert!(!control.touched());
    assert!(!control.editing());

    let inner = FormGroup::<_, ()>::new(FormArray::new(vec![control]));
    let mut group = FormGroup::<_, ()>::new(FormArray::new(vec![inner]));
    group.update_controls(|array| {
        array.get_mut(0).unwrap().update_controls(|inner_array| {
            inner_array.get_mut(0).unwrap().begin_edit();
            inner_array.get_mut(0).unwrap().input("again");
            inner_array.get_mut(0).unwrap().end_edit();
        });
        array.get_mut(0).unwrap().submit_attempt();
    });
    group.submit_attempt();
    group.reset();

    assert!(!group.submitted());
    let inner = group.controls().get(0).unwrap();
    let control = inner.controls().get(0).unwrap();
    assert!(!inner.submitted());
    assert!(control.pristine());
    assert!(!control.touched());
    assert!(!control.editing());
    assert_eq!(control.value(), &"initial");
}
