use std::sync::Arc;

use ratatui::{TerminalOptions, Viewport};

use super::*;
use crate::runtime::renderer::{
    DirectKittyIntent, DirectKittyPlacementId, DirectKittySourceRect, GraphicsFrame, GraphicsLevel,
};
use crate::{
    DialogLayer, Flex, FlexItem, Grid, GridItem, GridTrack, LayoutCtx, Paragraph,
    ScrollbarVisibility, Tab, Tabs, TabsVariant,
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

struct DirectKittyContent;

impl TuiNode<()> for DirectKittyContent {
    fn measure(&self, _: LayoutProposal) -> LayoutSizeHint {
        LayoutSizeHint::content(8, 8)
    }

    fn layout(&mut self, area: Rect, _: &mut LayoutCtx) -> LayoutResult {
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        frame.render_widget(
            ratatui::widgets::Paragraph::new("zero\none\ntwo\nthree\nfour\nfive\nsix\nseven"),
            area,
        );
        ctx.register_direct_kitty(DirectKittyIntent {
            id: DirectKittyPlacementId {
                image_id: 7,
                placement_id: 1,
            },
            area: Rect::new(0, 0, 8, 8),
            source_rect: DirectKittySourceRect {
                x: 0,
                y: 0,
                width: 80,
                height: 160,
            },
            generation: 0,
            payload: Arc::from("payload"),
            level: GraphicsLevel::base(),
            z_index: 0,
        });
    }
}

fn render_direct_kitty_scroll(
    node: &ScrollContainer<DirectKittyContent>,
    area: Rect,
) -> (String, GraphicsFrame) {
    let mut graphics = None;
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal
        .draw(|frame| {
            let mut ctx = RenderCtx::new();
            node.render(frame, area, &mut ctx);
            graphics = Some(ctx.take_graphics_frame());
        })
        .unwrap();
    (
        terminal.backend().buffer()[(0, 0)].symbol().to_owned(),
        graphics.unwrap(),
    )
}

#[test]
fn scrolling_pauses_direct_kitty_until_movement_is_idle_for_the_debounce() {
    let no_scrollbars = ScrollbarConfig {
        vertical: ScrollbarVisibility::Never,
        horizontal: ScrollbarVisibility::Never,
        ..ScrollbarConfig::default()
    };
    let mut node = ScrollContainer::vertical(DirectKittyContent)
        .scrollbars(no_scrollbars)
        .pause_direct_kitty_while_scrolling(Duration::from_millis(80));
    let area = Rect::new(0, 0, 8, 4);
    node.layout(area, &mut LayoutCtx::new());

    let (initial_text, initial_graphics) = render_direct_kitty_scroll(&node, area);
    assert_eq!(initial_text, "z");
    assert_eq!(initial_graphics.residents.len(), 1);
    assert_eq!(initial_graphics.intents.len(), 1);

    let route = EventRoute::new(TreePath::new().child(ChildKey::body()));
    let wheel_down = TuiEvent::Mouse(crate::MouseEvent {
        kind: crate::MouseEventKind::ScrollDown,
        column: 0,
        row: 0,
        modifiers: crate::KeyModifiers::NONE,
    });
    let mut event = EventCtx::default();
    assert_eq!(
        node.dispatch_event(&route, &wheel_down, &mut event),
        EventOutcome::Handled
    );
    assert!(event.tick_requested());
    let (scrolled_text, suppressed_graphics) = render_direct_kitty_scroll(&node, area);
    assert_eq!(scrolled_text, "o");
    assert!(suppressed_graphics.residents.is_empty());
    assert!(suppressed_graphics.intents.is_empty());

    let before_repeat = node.tick(Duration::from_millis(60), AnimationSettings::default());
    assert!(before_repeat.active);
    let mut repeated_event = EventCtx::default();
    node.dispatch_event(&route, &wheel_down, &mut repeated_event);
    let before_expiry = node.tick(Duration::from_millis(79), AnimationSettings::default());
    assert!(before_expiry.active);
    assert!(!before_expiry.changed);
    let (repeated_text, repeated_graphics) = render_direct_kitty_scroll(&node, area);
    assert_eq!(repeated_text, "t");
    assert!(repeated_graphics.residents.is_empty());
    assert!(repeated_graphics.intents.is_empty());

    let expiry = node.tick(Duration::from_millis(1), AnimationSettings::default());
    assert!(expiry.changed);
    assert!(!expiry.active);
    let (final_text, final_graphics) = render_direct_kitty_scroll(&node, area);
    assert_eq!(final_text, "t");
    assert_eq!(final_graphics.residents.len(), 1);
    assert_eq!(final_graphics.intents.len(), 1);
    assert_eq!(final_graphics.intents[0].area, area);
    assert_eq!(
        final_graphics.intents[0].source_rect,
        DirectKittySourceRect {
            x: 0,
            y: 40,
            width: 80,
            height: 80,
        }
    );
}

#[test]
fn animated_scroll_starts_the_debounce_after_movement_stops() {
    let mut node = ScrollContainer::vertical(DirectKittyContent)
        .pause_direct_kitty_while_scrolling(Duration::from_millis(20));
    node.layout(Rect::new(0, 0, 8, 4), &mut LayoutCtx::new());
    let settings = AnimationSettings {
        default_duration: Duration::from_millis(50),
        ..AnimationSettings::default()
    };

    assert!(node.scroll_to(ScrollOffset::new(0, 2), settings));
    assert!(node.tick(Duration::from_millis(25), settings).active);
    let movement_finished = node.tick(Duration::from_millis(25), settings);
    assert!(movement_finished.active);
    let before_expiry = node.tick(Duration::from_millis(19), settings);
    assert!(!before_expiry.changed);
    assert!(before_expiry.active);
    let expiry = node.tick(Duration::from_millis(1), settings);
    assert!(expiry.changed);
    assert!(!expiry.active);
}

struct NestedDirectKittyContent {
    child: ScrollContainer<DirectKittyContent>,
}

impl TuiNode<()> for NestedDirectKittyContent {
    fn measure(&self, _: LayoutProposal) -> LayoutSizeHint {
        LayoutSizeHint::content(8, 8)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.child.layout(Rect::new(1, 1, 5, 4), ctx);
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        self.child.render(frame, area, ctx);
    }
}

#[test]
fn direct_kitty_intents_use_nested_scroll_transforms_and_clips() {
    let no_scrollbars = ScrollbarConfig {
        vertical: ScrollbarVisibility::Never,
        horizontal: ScrollbarVisibility::Never,
        ..ScrollbarConfig::default()
    };
    let mut node = ScrollContainer::both(NestedDirectKittyContent {
        child: ScrollContainer::both(DirectKittyContent).scrollbars(no_scrollbars),
    })
    .scrollbars(no_scrollbars);
    let area = Rect::new(10, 5, 4, 3);
    node.layout(area, &mut LayoutCtx::new());
    node.scroll_to(
        ScrollOffset::new(2, 2),
        AnimationSettings {
            enabled: false,
            ..AnimationSettings::default()
        },
    );
    let mut graphics = None;
    let mut terminal = Terminal::new(TestBackend::new(20, 10)).unwrap();

    terminal
        .draw(|frame| {
            let mut ctx = RenderCtx::new();
            node.render(frame, area, &mut ctx);
            graphics = Some(ctx.take_graphics_frame());
        })
        .unwrap();

    let graphics = graphics.unwrap();
    assert_eq!(graphics.intents.len(), 1);
    assert_eq!(graphics.intents[0].area, Rect::new(10, 5, 4, 3));
    assert_eq!(
        graphics.intents[0].source_rect,
        DirectKittySourceRect {
            x: 10,
            y: 20,
            width: 40,
            height: 60,
        }
    );
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
