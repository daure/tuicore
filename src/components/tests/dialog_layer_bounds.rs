use super::*;
use crate::{Button, Dialog, Flex, FlexItem, Paragraph, RenderCtx};
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn extended_dock_covers_footer_without_resizing_its_base() {
    crate::init();
    let layer = DialogLayer::new(Button::<()>::new("Base"), Dialog::new())
        .docked(DockSpec::right(40))
        .extend_to_overlay_bottom(true);
    let mut root = Flex::column()
        .child("page", layer, FlexItem::fill(1))
        .child(
            "footer",
            Paragraph::new("status".repeat(20)),
            FlexItem::fixed(1),
        );
    for area in [Rect::new(0, 0, 100, 40), Rect::new(0, 0, 80, 32)] {
        let mut layout = LayoutCtx::new();
        layout.with_overlay_bounds(area, |ctx| root.layout(area, ctx));
        let base = layout
            .hit_regions()
            .iter()
            .find(|target| {
                target.path == TreePath::from_keys([ChildKey::new("page"), ChildKey::first()])
            })
            .unwrap();
        let dialog = layout
            .focus_targets()
            .iter()
            .find(|target| target.id.as_str() == "dialog")
            .unwrap();
        assert_eq!(base.area.bottom(), area.bottom() - 1);
        assert_eq!(dialog.area.bottom(), area.bottom());
        let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
        terminal
            .draw(|frame| {
                let mut render = RenderCtx::new();
                root.render(frame, area, &mut render);
                render.flush(frame);
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(0, area.bottom() - 1)].symbol(), "s");
        assert_eq!(buffer[(area.right() - 1, area.bottom() - 1)].symbol(), " ");
    }
}
