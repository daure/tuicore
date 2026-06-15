use tuicore::{
    AnimationSettings, Column, DataView, DataViewTypedEvent, KeyBindings, KeySpec, keybindings,
    set_keybindings,
};
use tuirealm::event::{Key, KeyEvent, KeyModifiers};
use tuirealm::ratatui::layout::{Constraint, Rect};

#[derive(Clone)]
struct Row {
    id: usize,
    name: &'static str,
}

#[test]
fn configured_data_view_activation_key_emits_activation() {
    let _guard = KeybindingsGuard::replace(
        KeyBindings::new().with_data_view_activate([KeySpec::plain('a')]),
    );
    let mut view = DataView::new([Row { id: 1, name: "one" }], |row| row.id).column(Column::text(
        "name",
        "Name",
        Constraint::Percentage(100),
        |row: &Row| row.name.to_string(),
    ));
    let mut settings = AnimationSettings::default();
    settings.enabled = false;

    let outcome = view.on_key_with_settings(
        KeyEvent {
            code: Key::Char('a'),
            modifiers: KeyModifiers::NONE,
        },
        Rect::new(0, 0, 20, 1),
        settings,
    );

    assert!(outcome.activated);
    assert_eq!(
        view.take_last_activated().map(|event| event.row_id),
        Some(1)
    );
    assert_eq!(
        view.take_events(),
        vec![DataViewTypedEvent::Activated { row_id: 1 }]
    );
}

struct KeybindingsGuard(KeyBindings);

impl KeybindingsGuard {
    fn replace(next: KeyBindings) -> Self {
        let previous = keybindings();
        set_keybindings(next);
        Self(previous)
    }
}

impl Drop for KeybindingsGuard {
    fn drop(&mut self) {
        set_keybindings(self.0.clone());
    }
}
