use super::*;
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn menu_hints_share_the_right_edge_and_preserve_label_search() {
    let mut menu = Menu::new([
        MenuItem::new(1, "Preferences").hint("Ctrl+Shift+Enter"),
        MenuItem::new(2, "Notifications").hint("N"),
    ]);
    let bounds = Rect::new(0, 0, 60, 10);
    let anchor = Rect::new(0, 0, 1, 1);
    menu.open();
    let mut layout = LayoutCtx::new();
    layout.with_overlay_bounds(bounds, |ctx| {
        <Menu<_> as TuiNode<()>>::layout(&mut menu, anchor, ctx);
    });
    let popup = layout.overlays().last().unwrap().area;
    assert_eq!(popup.width, 28);
    let mut terminal = Terminal::new(TestBackend::new(bounds.width, bounds.height)).unwrap();
    terminal
        .draw(|frame| {
            let mut render = crate::RenderCtx::new();
            <Menu<_> as TuiNode<()>>::render(&menu, frame, anchor, &mut render);
            render.flush(frame);
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let preference_hint = buffer.cell((popup.right() - 1, popup.y + 1)).unwrap();
    assert_eq!(preference_hint.symbol(), "r");
    assert_eq!(preference_hint.fg, theme().muted_fg());
    let notification_hint = buffer.cell((popup.right() - 1, popup.y + 2)).unwrap();
    assert_eq!(notification_hint.symbol(), "N");
    assert_eq!(notification_hint.fg, theme().muted_fg());
    for character in "Notifications".chars() {
        menu.on_key(crate::Key::Char(character), bounds);
    }
    menu.on_key(crate::Key::Enter, bounds);
    assert_eq!(menu.take_activated(), vec![2]);
}
