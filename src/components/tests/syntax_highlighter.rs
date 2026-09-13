use super::*;
use crate::{
    Key, ScrollAxes, ScrollbarConfig, ScrollbarGutter, ScrollbarStyle, ScrollbarVisibility,
};
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn line_navigation_scrolls_to_selected_line_without_tweening() {
    let code = (0..20)
        .map(|line| format!("line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut highlighter = SyntaxHighlighter::new(code, Language::Rust);
    highlighter.content_size = ScrollSize::new(7, 20);
    let area = Rect::new(0, 0, 7, 4);
    for _ in 0..8 {
        highlighter.on_key_with_settings(Key::Char('j'), area, AnimationSettings::default());
    }
    assert_eq!(
        highlighter.scroll.offset(),
        highlighter.scroll.target_offset()
    );
    assert!(!highlighter.scroll.is_active());
    highlighter.on_key_with_settings(Key::Char('k'), area, AnimationSettings::default());
    assert_eq!(
        highlighter.scroll.offset(),
        highlighter.scroll.target_offset()
    );
    assert!(!highlighter.scroll.is_active());
}

fn render(highlighter: &SyntaxHighlighter, area: Rect) -> String {
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal
        .draw(|frame| {
            <SyntaxHighlighter as TuiNode<()>>::render(
                highlighter,
                frame,
                area,
                &mut RenderCtx::new(),
            );
        })
        .unwrap();
    (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| terminal.backend().buffer()[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn wrapped_description_supports_visual_navigation_and_resize() {
    crate::init();
    let code = "Details before the final marker END";
    let mut highlighter = SyntaxHighlighter::new(code, Language::Markdown).wrap(true);
    let area = Rect::new(0, 0, 14, 2);
    <SyntaxHighlighter as TuiNode<()>>::layout(&mut highlighter, area, &mut LayoutCtx::new());
    assert!(highlighter.content_size.height > 2);
    assert!(!render(&highlighter, area).contains("END"));
    highlighter.on_key_with_settings(
        Key::End,
        area,
        AnimationSettings {
            enabled: false,
            ..Default::default()
        },
    );
    assert_eq!(
        highlighter.selected_line,
        Some(highlighter.content_size.height - 1)
    );
    assert!(render(&highlighter, area).contains("END"));
    assert_eq!(highlighter.scroll.offset().x, 0);

    let wide = Rect::new(0, 0, 60, 4);
    <SyntaxHighlighter as TuiNode<()>>::layout(&mut highlighter, wide, &mut LayoutCtx::new());
    assert_eq!(highlighter.content_size.height, 1);
    assert_eq!(highlighter.selected_line, Some(0));
    assert_eq!(highlighter.scroll.offset().y, 0);
    assert!(render(&highlighter, wide).contains(code));

    highlighter.set_code("Short text");
    <SyntaxHighlighter as TuiNode<()>>::layout(&mut highlighter, area, &mut LayoutCtx::new());
    assert_eq!(highlighter.content_size.height, 1);
    assert!(render(&highlighter, area).contains("Short text"));
}

#[test]
fn wrapping_is_opt_in_and_preserves_indentation_and_wide_text() {
    crate::init();
    let area = Rect::new(0, 0, 16, 6);
    let mut highlighter = SyntaxHighlighter::new(
        "  - 界界 words across several lines END",
        Language::Markdown,
    );
    <SyntaxHighlighter as TuiNode<()>>::layout(&mut highlighter, area, &mut LayoutCtx::new());
    assert_eq!(highlighter.content_size.height, 1);
    assert!(!render(&highlighter, area).contains("END"));
    highlighter.set_wrap(true);
    <SyntaxHighlighter as TuiNode<()>>::layout(&mut highlighter, area, &mut LayoutCtx::new());
    let text = render(&highlighter, area);
    assert!(text.starts_with("  - "));
    assert!(text.contains("END"));
    assert!(highlighter.content_size.height > 1);
}

#[test]
fn copy_region_excludes_the_vertical_scrollbar_gutter() {
    let mut highlighter = SyntaxHighlighter::new(
        (0..8)
            .map(|line| format!("line {line}"))
            .collect::<Vec<_>>()
            .join("\n"),
        Language::Markdown,
    );
    highlighter.scroll = ScrollState::new(ScrollAxes::Vertical).scrollbars(ScrollbarConfig {
        vertical: ScrollbarVisibility::Always,
        horizontal: ScrollbarVisibility::Never,
        gutter: ScrollbarGutter::Reserve,
        style: ScrollbarStyle::ThinTrack,
    });
    let area = Rect::new(0, 0, 8, 3);
    let mut layout = LayoutCtx::new();

    <SyntaxHighlighter as TuiNode<()>>::layout(&mut highlighter, area, &mut layout);

    assert_eq!(layout.copy_regions()[0].area(), Rect::new(0, 0, 7, 3));
}
