use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use tuicore::{
    AnimationSettings, DiffStyle, DiffViewer, EventCtx, Key, KeyEvent, LayoutCtx, TextareaInput,
    TuiEvent, TuiNode,
};

#[test]
fn diff_hits_follow_source_sides_through_wrapping_and_scrolling() {
    let old = "Before\nImage: original-long-filename.png\nAfter\nOld ending";
    let new = "Before\nImage: updated-long-filename.png\nAfter\nNew ending";
    let settings = AnimationSettings {
        enabled: false,
        ..AnimationSettings::default()
    };
    for style in [DiffStyle::Word, DiffStyle::SideBySide] {
        let mut view = DiffViewer::new(old, new)
            .style(style)
            .wrap(true)
            .show_headers(false)
            .focused(true);
        let area = Rect::new(2, 1, 32, 5);
        <DiffViewer as TuiNode<()>>::layout(&mut view, area, &mut LayoutCtx::new());
        for end in [false, true] {
            if end {
                view.on_key_with_settings(Key::End, area, settings);
            }
            let mut terminal = Terminal::new(TestBackend::new(36, 8)).unwrap();
            terminal.draw(|frame| view.render(frame, area)).unwrap();
            let mut hits = 0;
            for y in area.y..area.bottom() {
                for x in area.x..area.right() {
                    let Some((location, column)) = view.text_position_at(x, y) else {
                        continue;
                    };
                    let source = if location.old_line.is_some() {
                        old
                    } else {
                        new
                    };
                    let line = location.old_line.or(location.new_line).unwrap();
                    let expected = source
                        .lines()
                        .nth(line - 1)
                        .unwrap()
                        .chars()
                        .nth(column)
                        .unwrap();
                    assert_eq!(
                        terminal.backend().buffer()[(x, y)].symbol(),
                        expected.to_string(),
                        "{style:?} at {x},{y}"
                    );
                    hits += 1;
                }
            }
            assert!(hits > 0);
            assert_eq!(view.text_position_at(area.x, area.y), None);
        }
    }
}

#[test]
fn textarea_hits_follow_wrapped_unicode_text_after_scrolling() {
    let text = "Before\nImage: 画像-long-filename.png\nAfter\nLast";
    let mut view = TextareaInput::<()>::new()
        .value(text)
        .disabled(true)
        .wrap(true)
        .focused(true);
    let area = Rect::new(2, 1, 15, 3);
    view.layout(area, &mut LayoutCtx::new());
    for end in [false, true] {
        if end {
            view.event(
                &TuiEvent::Key(KeyEvent::from(Key::End)),
                &mut EventCtx::new(AnimationSettings {
                    enabled: false,
                    ..AnimationSettings::default()
                }),
            );
        }
        let mut terminal = Terminal::new(TestBackend::new(20, 6)).unwrap();
        terminal.draw(|frame| view.render(frame, area)).unwrap();
        let mut hits = 0;
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                let symbol = terminal.backend().buffer()[(x, y)].symbol();
                if symbol.trim().is_empty() {
                    continue;
                }
                if let Some(index) = view.text_index_at(x, y) {
                    assert_eq!(symbol, text.chars().nth(index).unwrap().to_string());
                    hits += 1;
                }
            }
        }
        assert!(hits > 0);
        assert_eq!(view.text_index_at(0, 0), None);
    }
}
