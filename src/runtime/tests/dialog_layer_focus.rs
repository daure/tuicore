use super::*;
use crate::{
    ChildKey, Dialog, DialogHost, DialogLayer, Flex, FocusId, Key, KeyEvent, RenderCtx, TextInput,
};
use ratatui::{Terminal, backend::TestBackend};

fn name_dialog(value: String) -> DialogHost<TextInput<String>, String> {
    Dialog::new().host(
        TextInput::new()
            .panel("Name")
            .value(value)
            .on_edit_end(|value| value),
    )
}

#[test]
fn submitting_a_replaced_dialog_input_keeps_focused_navigation_chrome() {
    crate::init();
    let target = FocusRequest::TargetAt {
        path: TreePath::from_keys([ChildKey::second(), ChildKey::body()]),
        id: FocusId::new("input"),
    };
    let root = DialogLayer::new(Flex::column(), name_dialog(String::new()));
    let mut app = TreeApp::new(root)
        .animation_settings(AnimationSettings {
            enabled: false,
            ..AnimationSettings::default()
        })
        .initial_focus(target.clone())
        .on_message(move |root, value, ctx| {
            root.replace_layer(name_dialog(value), ctx);
            ctx.focus(target.clone());
        })
        .run_test_events(
            [Key::Enter, Key::Char('q'), Key::Enter, Key::Null]
                .map(|key| TuiEvent::Key(KeyEvent::from(key))),
            Rect::new(0, 0, 32, 7),
        );

    assert_eq!(app.root.layer().child().current_value(), "q");
    assert!(!app.root.layer().child().insert_mode());
    let area = Rect::new(0, 0, 32, 7);
    let mut layout = crate::LayoutCtx::new();
    app.root.layout(area, &mut layout);
    let input = layout
        .focus_targets()
        .iter()
        .find(|target| target.id.as_str() == "input")
        .unwrap();
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal
        .draw(|frame| {
            let mut render = RenderCtx::new();
            app.root.render(frame, area, &mut render);
            render.flush(frame);
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert_eq!(
        buffer[(input.area.x - 1, input.area.y - 1)].fg,
        crate::theme().accent_fg()
    );
    assert_eq!(
        buffer[(input.area.x, input.area.y)].bg,
        crate::theme().selected_bg()
    );
}
