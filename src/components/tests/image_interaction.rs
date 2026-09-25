use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::{
    AnimationSettings, EventRoute, Flex, FlexItem, KeyModifiers, MouseEvent, ScrollContainer,
};

fn image(counter: Arc<AtomicUsize>) -> Image {
    Image::from_base64(super::tests::TEST_PNG)
        .unwrap()
        .protocol(ImageProtocol::Kitty)
        .size(8, 8)
        .on_double_click(move || {
            counter.fetch_add(1, Ordering::Relaxed);
        })
}

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> TuiEvent {
    TuiEvent::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

#[test]
fn double_click_activates_only_inside_image_pixels_and_wheel_input_bubbles() {
    let opened = Arc::new(AtomicUsize::new(0));
    let mut image = image(Arc::clone(&opened));
    let mut layout = LayoutCtx::new();
    <Image as TuiNode<()>>::layout(&mut image, Rect::new(3, 4, 8, 8), &mut layout);
    assert_eq!(layout.hit_regions()[0].area, Rect::new(3, 4, 8, 2));

    let click = mouse(MouseEventKind::Down(MouseButton::Left), 4, 4);
    image.event(&click, &mut EventCtx::<()>::default());
    assert_eq!(opened.load(Ordering::Relaxed), 0);
    image.event(
        &mouse(MouseEventKind::Up(MouseButton::Left), 4, 4),
        &mut EventCtx::<()>::default(),
    );
    image.event(&click, &mut EventCtx::<()>::default());
    assert_eq!(opened.load(Ordering::Relaxed), 1);
    image.event(&click, &mut EventCtx::<()>::default());
    assert_eq!(
        opened.load(Ordering::Relaxed),
        1,
        "third click starts a new pair"
    );

    let mut wheel = EventCtx::<()>::default();
    assert_eq!(
        image.event(&mouse(MouseEventKind::ScrollDown, 4, 4), &mut wheel),
        EventOutcome::Ignored
    );
    assert_eq!(wheel.propagation(), crate::Propagation::Continue);
    image.event(&click, &mut EventCtx::<()>::default());
    assert_eq!(opened.load(Ordering::Relaxed), 1, "scroll cancels the pair");

    image.last_click = Some((Instant::now() - Duration::from_secs(1), 4, 4));
    image.event(&click, &mut EventCtx::<()>::default());
    assert_eq!(opened.load(Ordering::Relaxed), 1, "slow clicks stay single");
    for _ in 0..2 {
        assert_eq!(
            image.event(
                &mouse(MouseEventKind::Down(MouseButton::Left), 4, 7),
                &mut EventCtx::<()>::default(),
            ),
            EventOutcome::Ignored
        );
    }
    assert_eq!(
        opened.load(Ordering::Relaxed),
        1,
        "padding is not clickable"
    );
}

#[test]
fn scrolled_image_hit_region_routes_rebased_clicks_to_the_visible_image() {
    let opened = Arc::new(AtomicUsize::new(0));
    let content = Flex::column()
        .child("image", image(Arc::clone(&opened)), FlexItem::fixed(8))
        .child("tail", crate::Paragraph::new("tail"), FlexItem::fixed(8));
    let mut scroll = ScrollContainer::<_, ()>::vertical(content);
    let area = Rect::new(10, 5, 8, 4);
    scroll.layout(area, &mut LayoutCtx::new());
    scroll.scroll_to(
        crate::ScrollOffset::new(0, 1),
        AnimationSettings {
            enabled: false,
            ..AnimationSettings::default()
        },
    );
    let mut layout = LayoutCtx::new();
    scroll.layout(area, &mut layout);
    let image_path = crate::TreePath::new()
        .child(crate::ChildKey::body())
        .child("image".into());
    let hit = layout
        .hit_regions()
        .iter()
        .rev()
        .find(|hit| hit.path == image_path)
        .unwrap();
    assert_eq!(hit.area.y, 5);
    assert_eq!(hit.area.height, 1);
    let route = EventRoute::new(hit.path.clone());
    let click = mouse(
        MouseEventKind::Down(MouseButton::Left),
        hit.area.x,
        hit.area.y,
    );
    for _ in 0..2 {
        assert_eq!(
            scroll.dispatch_event(&route, &click, &mut EventCtx::default()),
            EventOutcome::Handled
        );
    }
    assert_eq!(opened.load(Ordering::Relaxed), 1);
}
