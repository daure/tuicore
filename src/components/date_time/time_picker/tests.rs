use super::*;

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
