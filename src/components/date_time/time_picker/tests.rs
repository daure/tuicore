use super::*;
use crate::Key;

#[test]
fn time_picker_arrow_keys_move_minutes_by_one() {
    let mut picker = TimePicker::<()>::new().minute_step(15);
    picker.active_field = TimeField::Minute;
    picker.on_key(Key::Down);
    assert_eq!(picker.draft_value().minute(), 59);
    picker.on_key(Key::Up);
    assert_eq!(picker.draft_value().minute(), 0);
}

#[test]
fn time_picker_uses_plain_hjkl_instead_of_control_hjkl() {
    let mut picker = TimePicker::<()>::new();

    let control = |code| KeyEvent {
        code,
        modifiers: crate::KeyModifiers::CONTROL,
    };
    assert_eq!(
        picker.on_key(control(Key::Char('l'))),
        PickerOutcome::IGNORED
    );
    assert_eq!(picker.active_field(), TimeField::Hour);

    assert!(picker.on_key(Key::Char('l')).changed);
    assert_eq!(picker.active_field(), TimeField::Minute);
    assert!(picker.on_key(Key::Char('h')).changed);
    assert_eq!(picker.active_field(), TimeField::Hour);

    let hour = picker.draft_value().hour();
    assert!(picker.on_key(Key::Char('k')).changed);
    assert_eq!(picker.draft_value().hour(), (hour + 1) % 24);
    assert!(picker.on_key(Key::Char('j')).changed);
    assert_eq!(picker.draft_value().hour(), hour);
}

#[test]
fn time_picker_accepts_typed_hour_and_clips_to_bounds() {
    let mut picker = TimePicker::<()>::new();

    picker.on_key(Key::Char('1'));
    picker.on_key(Key::Char('4'));
    assert_eq!(picker.draft_value().hour(), 14);
    assert_eq!(picker.active_field(), TimeField::Minute);

    picker.active_field = TimeField::Hour;
    picker.on_key(Key::Char('2'));
    picker.on_key(Key::Char('5'));
    assert_eq!(picker.draft_value().hour(), 23);
    assert_eq!(picker.active_field(), TimeField::Minute);
}

#[test]
fn time_picker_accepts_typed_minutes_and_navigation_keys() {
    let mut picker = TimePicker::<()>::new().minute_step(15);
    picker.active_field = TimeField::Minute;

    picker.on_key(Key::Char('4'));
    picker.on_key(Key::Char('5'));
    assert_eq!(picker.draft_value().minute(), 45);

    picker.on_key(Key::Home);
    assert_eq!(picker.draft_value().minute(), 0);
    picker.on_key(Key::End);
    assert_eq!(picker.draft_value().minute(), 59);
    picker.on_key(Key::Char('g'));
    picker.on_key(Key::Char('g'));
    assert_eq!(picker.draft_value().minute(), 0);
    picker.on_key(KeyEvent {
        code: Key::Char('g'),
        modifiers: crate::KeyModifiers::SHIFT,
    });
    assert_eq!(picker.draft_value().minute(), 59);
    picker.on_key(Key::PageDown);
    assert_eq!(picker.draft_value().minute(), 44);
    picker.on_key(Key::PageUp);
    assert_eq!(picker.draft_value().minute(), 59);
}

#[test]
fn time_picker_registers_and_handles_hotkey() {
    let mut picker = TimePicker::<()>::new().hotkey("t");
    let mut layout = LayoutCtx::new();
    picker.layout(Rect::new(0, 0, 12, 1), &mut layout);
    assert_eq!(layout.focus_targets()[0].hotkey_sequences, vec!["t"]);

    let mut ctx = EventCtx::<()>::new(crate::animation_settings());
    let pending = picker.event(
        &TuiEvent::Hotkey(HotkeyEvent::Pending("t".into())),
        &mut ctx,
    );
    assert_eq!(pending, EventOutcome::Ignored);
    assert_eq!(picker.pending_hotkey_prefix.as_deref(), Some("t"));

    let commit = picker.event(&TuiEvent::Hotkey(HotkeyEvent::Commit("t".into())), &mut ctx);
    assert_eq!(commit, EventOutcome::Handled);
    assert_eq!(picker.pending_hotkey_prefix, None);
}

#[test]
fn time_picker_uses_configured_today_key_for_now() {
    let now = super::super::today_time();
    let other = if now.hour() == 0 {
        time::Time::from_hms(1, now.minute(), now.second()).unwrap()
    } else {
        time::Time::from_hms(0, now.minute(), now.second()).unwrap()
    };
    let mut picker = TimePicker::<()>::new().value(other);

    let ignored = picker.on_key(Key::Char('n'));
    assert_eq!(ignored, PickerOutcome::IGNORED);

    let selected = picker.on_key(Key::Char('t'));
    assert!(selected.selected);
    assert_ne!(picker.current_value(), other);
}

#[test]
fn time_picker_measures_visible_hotkey_text() {
    let picker = TimePicker::<()>::new().hotkey("ctrl+t");
    let expected = crate::line_width(&picker.time_line()).min(u16::MAX as usize) as u16;

    assert_eq!(picker.measure_size(), (expected, 1));
    assert!(expected > 8);
}

#[test]
fn time_picker_applies_external_editor_time_with_whitespace() {
    let mut picker = TimePicker::<()>::new();
    let mut ctx = EventCtx::new(crate::animation_settings());

    let outcome = picker.event(
        &TuiEvent::ExternalEditor(crate::ExternalEditorResponse {
            value: String::from("  14:35\n  "),
            line: 1,
            col: 1,
        }),
        &mut ctx,
    );

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(
        picker.current_value(),
        time::Time::from_hms(14, 35, 0).unwrap()
    );
}
