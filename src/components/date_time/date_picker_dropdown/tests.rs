use super::*;

#[test]
fn date_picker_dropdown_normalizes_committed_hotkey() {
    let mut dropdown = DatePickerDropdown::<()>::new().hotkey(" D ");
    let mut ctx = EventCtx::new(crate::animation_settings());

    let outcome = dropdown.handle_hotkey(&HotkeyEvent::Commit("d".into()), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(dropdown.is_open());
}

#[test]
fn date_picker_dropdown_measure_stays_field_height_when_open() {
    let mut dropdown = DatePickerDropdown::<()>::new();
    let proposal = LayoutProposal::unbounded();

    assert_eq!(dropdown.measure(proposal).preferred.height, 1);

    dropdown.set_open(true);

    assert_eq!(dropdown.measure(proposal).preferred.height, 1);
}

#[test]
fn date_picker_dropdown_places_popup_inside_overlay_bounds() {
    let mut dropdown = DatePickerDropdown::<()>::new();
    let mut ctx = LayoutCtx::new();
    let bounds = Rect::new(0, 0, 80, 24);

    dropdown.layout(Rect::new(5, 2, 30, 1), &mut ctx);

    assert_eq!(dropdown.popup_area(bounds), Rect::new(5, 3, 24, 10));

    dropdown.layout(Rect::new(5, 20, 30, 1), &mut ctx);

    assert_eq!(dropdown.popup_area(bounds), Rect::new(5, 10, 24, 10));
}
