use ratatui::{TerminalOptions, Viewport};

use super::*;
use crate::{
    DialogLayer, Flex, FlexItem, Grid, GridItem, GridTrack, LayoutCtx, Paragraph, Tab, Tabs,
    TabsVariant,
};

fn grid() -> Grid {
    Grid::new()
        .columns(vec![GridTrack::fixed(10)])
        .rows(vec![GridTrack::fixed(8)])
        .child(
            "text",
            Paragraph::new("0123456789\nabcdefghij\nklmnopqrst\nuvwxyzABCD\nEFGHIJKLMN"),
            GridItem::new(0, 0),
        )
}

#[test]
fn tabbed_grid_under_dialog_renders_through_terminal_shrink_and_relayout() {
    let body = Flex::column().child(
        "scroll",
        ScrollContainer::vertical(grid()),
        FlexItem::fill(1),
    );
    let tabs = Tabs::new(vec![Tab::new("Notes", body)])
        .variant(TabsVariant::Underline)
        .bordered(false);
    let base = Flex::column().child("tabs", tabs, FlexItem::fill(1));
    let mut root = DialogLayer::new(base, Paragraph::new("Dialog"));
    let mut terminal = Terminal::new(TestBackend::new(62, 10)).unwrap();

    root.layout(Rect::new(0, 0, 62, 10), &mut LayoutCtx::new());
    terminal.backend_mut().resize(62, 1);
    terminal
        .draw(|frame| {
            let mut ctx = RenderCtx::new();
            root.render(frame, frame.area(), &mut ctx);
            ctx.flush(frame);
        })
        .unwrap();
    assert_eq!(terminal.backend().buffer()[(0, 0)].symbol(), "D");

    for height in [0, 1, 2, 3, 10] {
        terminal.backend_mut().resize(62, height);
        let area = Rect::new(0, 0, 62, height);
        root.layout(area, &mut LayoutCtx::new());
        terminal
            .draw(|frame| {
                let mut ctx = RenderCtx::new();
                root.render(frame, area, &mut ctx);
                ctx.flush(frame);
            })
            .unwrap();
    }
    root.set_active(false);
    let area = Rect::new(0, 0, 62, 10);
    root.layout(area, &mut LayoutCtx::new());
    terminal
        .draw(|frame| root.render(frame, area, &mut RenderCtx::new()))
        .unwrap();
    assert_eq!(terminal.backend().buffer()[(0, 2)].symbol(), "0");
}

#[test]
fn clipped_scrolled_grid_preserves_source_coordinates() {
    let mut node = ScrollContainer::both(grid());
    let area = Rect::new(1, 1, 6, 4);
    node.layout(area, &mut LayoutCtx::new());
    node.scroll_to(
        ScrollOffset::new(2, 1),
        AnimationSettings {
            enabled: false,
            ..AnimationSettings::default()
        },
    );
    assert_eq!(node.offset(), ScrollOffset::new(2, 1));

    let mut terminal = Terminal::with_options(
        TestBackend::new(10, 8),
        TerminalOptions {
            viewport: Viewport::Fixed(Rect::new(3, 2, 3, 2)),
        },
    )
    .unwrap();
    let completed = terminal
        .draw(|frame| node.render(frame, area, &mut RenderCtx::new()))
        .unwrap();

    assert_eq!(completed.buffer[(3, 2)].symbol(), "o");
    assert_eq!(completed.buffer[(5, 2)].symbol(), "q");
    assert_eq!(completed.buffer[(3, 3)].symbol(), "y");
}

struct CursorContent;

impl TuiNode<()> for CursorContent {
    fn measure(&self, _: LayoutProposal) -> LayoutSizeHint {
        LayoutSizeHint::content(8, 4)
    }

    fn layout(&mut self, area: Rect, _: &mut LayoutCtx) -> LayoutResult {
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut Frame, _: Rect, _: &mut RenderCtx<'a>) {
        frame.set_cursor_position((3, 1));
    }
}

#[test]
fn child_cursor_is_visible_only_inside_the_destination_frame() {
    let mut node = ScrollContainer::vertical(CursorContent);
    let area = Rect::new(1, 1, 8, 4);
    node.layout(area, &mut LayoutCtx::new());
    let mut terminal = Terminal::new(TestBackend::new(10, 6)).unwrap();

    for (width, height, visible) in [(10, 6, true), (10, 2, false), (4, 6, false)] {
        terminal.backend_mut().resize(width, height);
        terminal
            .draw(|frame| node.render(frame, area, &mut RenderCtx::new()))
            .unwrap();
        assert_eq!(terminal.backend().cursor_visible(), visible);
        if visible {
            assert_eq!(terminal.backend().cursor_position(), (4, 2).into());
        }
    }
}
