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

fn send(highlighter: &mut SyntaxHighlighter, event: TuiEvent) -> EventCtx<()> {
    let mut ctx = EventCtx::new(AnimationSettings {
        enabled: false,
        ..Default::default()
    });
    assert_eq!(highlighter.event(&event, &mut ctx), EventOutcome::Handled);
    assert_eq!(ctx.propagation(), crate::Propagation::Stopped);
    ctx
}

fn press(highlighter: &mut SyntaxHighlighter, key: impl Into<KeyEvent>) -> EventCtx<()> {
    send(highlighter, TuiEvent::Key(key.into()))
}

fn search_view(code: &str, area: Rect, wrap: bool) -> SyntaxHighlighter {
    let mut view = SyntaxHighlighter::new(code, Language::Rust).wrap(wrap);
    view.focused = true;
    <SyntaxHighlighter as TuiNode<()>>::layout(&mut view, area, &mut LayoutCtx::new());
    view
}

fn query(view: &mut SyntaxHighlighter, query: &str) {
    press(view, Key::Char('/'));
    for c in query.chars() {
        press(view, Key::Char(c));
    }
    press(view, Key::Enter);
}

#[test]
fn search_navigates_case_insensitive_matches_and_resumes_line_navigation_after_escape() {
    let area = Rect::new(0, 0, 24, 4);
    let mut view = search_view("needle\none\ntwo\nNEEDLE\nthree\nneedle\nend", area, false);
    query(&mut view, "needle");
    assert!(render(&view, area).lines().last().unwrap().contains("1/3"));
    for (line, counter) in [(3, "2/3"), (5, "3/3"), (0, "1/3")] {
        press(&mut view, Key::Char('n'));
        assert_eq!(view.selected_line, Some(line));
        assert!(
            render(&view, area)
                .lines()
                .last()
                .unwrap()
                .contains(counter)
        );
    }
    press(
        &mut view,
        KeyEvent {
            code: Key::Char('n'),
            modifiers: crate::KeyModifiers::SHIFT,
        },
    );
    assert_eq!(view.selected_line, Some(5));
    assert!(press(&mut view, Key::Esc).layout_requested());
    assert!(!view.search.is_active());
    press(&mut view, Key::Char('k'));
    assert_eq!(view.selected_line, Some(4));
}

#[test]
fn search_entry_captures_typing_and_paste_and_reserves_a_non_copyable_status_row() {
    let area = Rect::new(0, 0, 24, 4);
    let mut view = search_view("needle\nother", area, false);
    let before = <SyntaxHighlighter as TuiNode<()>>::measure(&view, LayoutProposal::unbounded());
    assert!(press(&mut view, Key::Char('/')).layout_requested());
    let mut layout = LayoutCtx::new();
    <SyntaxHighlighter as TuiNode<()>>::layout(&mut view, area, &mut layout);
    assert!(layout.focus_targets()[0].suppress_global_hotkeys);
    assert!(layout.focus_targets()[0].focused_events_before_global_hotkeys);
    assert_eq!(layout.copy_regions()[0].area().bottom(), area.bottom() - 1);
    send(&mut view, TuiEvent::Paste("nee\ndle\t".into()));
    assert_eq!(view.search.query(), "needle");
    let after = <SyntaxHighlighter as TuiNode<()>>::measure(&view, LayoutProposal::unbounded());
    assert_eq!(after.preferred.height, before.preferred.height + 1);
    press(&mut view, Key::Backspace);
    assert_eq!(view.search.query(), "needl");
    let selected = view.selected_line;
    press(&mut view, Key::Down);
    assert_eq!(view.selected_line, selected);
    assert!(press(&mut view, Key::Enter).layout_requested());
    let mut layout = LayoutCtx::new();
    <SyntaxHighlighter as TuiNode<()>>::layout(&mut view, area, &mut layout);
    assert!(!layout.focus_targets()[0].suppress_global_hotkeys);
    press(
        &mut view,
        KeyEvent {
            code: Key::Char('['),
            modifiers: crate::KeyModifiers::CONTROL,
        },
    );
    assert!(!view.search.is_active());
}

#[test]
fn search_is_focus_gated_and_handles_no_matches_and_replaced_content() {
    let area = Rect::new(0, 0, 24, 4);
    let mut view = search_view("needle", area, false);
    view.focused = false;
    assert_eq!(
        view.event(
            &TuiEvent::Key(Key::Char('/').into()),
            &mut EventCtx::<()>::default()
        ),
        EventOutcome::Ignored
    );
    view.focused = true;
    query(&mut view, "missing");
    press(&mut view, Key::Char('n'));
    press(&mut view, Key::Char('N'));
    assert!(render(&view, area).lines().last().unwrap().contains("0/0"));
    query(&mut view, "needle");
    assert!(render(&view, area).lines().last().unwrap().contains("1/1"));
    view.set_code("");
    assert!(!view.search.is_active());
    query(&mut view, "x");
    assert!(render(&view, area).lines().last().unwrap().contains("0/0"));
    for area in [Rect::default(), Rect::new(0, 0, 1, 1)] {
        <SyntaxHighlighter as TuiNode<()>>::layout(&mut view, area, &mut LayoutCtx::new());
        press(&mut view, Key::Char('n'));
    }
}

#[test]
fn search_reveals_horizontal_matches_using_terminal_cell_widths() {
    let area = Rect::new(0, 0, 14, 4);
    let mut view = search_view("// 界界界界界界界界 needle\nnext", area, false);
    query(&mut view, "needle");
    assert!(view.scroll.offset().x > 0);
    assert!(
        render(&view, area)
            .lines()
            .next()
            .unwrap()
            .contains("needle"),
        "offset={:?}, geometry={:?}, rendered={:?}",
        view.scroll.offset(),
        view.scroll_geometry(area),
        render(&view, area)
    );
    query(&mut view, "//");
    assert_eq!(view.scroll.offset().x, 0);
}

#[test]
fn search_reveals_the_wrapped_match_start_and_reanchors_after_resize() {
    let area = Rect::new(0, 0, 14, 4);
    let code = "// intro\n// words before a needleword and extra trailing words\n// end";
    let mut view = search_view(code, area, true);
    query(&mut view, "needle");
    let before = view.selected_line.unwrap();
    assert!(
        render(&view, area)
            .lines()
            .take(3)
            .any(|line| line.contains("needle"))
    );
    assert_eq!(view.scroll.offset().x, 0);
    let wide = Rect::new(0, 0, 60, 4);
    <SyntaxHighlighter as TuiNode<()>>::layout(&mut view, wide, &mut LayoutCtx::new());
    assert_eq!(view.selected_line, Some(1));
    assert!(before > 1);
    assert!(
        render(&view, wide)
            .lines()
            .take(3)
            .any(|line| line.contains("needle"))
    );
    press(&mut view, Key::Esc);
    press(&mut view, Key::Char('j'));
    assert_eq!(view.selected_line, Some(2));
}

#[test]
fn clearing_search_keeps_the_source_match_selected_when_the_gutter_disappears() {
    let area = Rect::new(0, 0, 14, 4);
    let mut view = search_view("a needleword X\none\ntwo\nthree", area, true);
    query(&mut view, "X");
    assert_eq!(view.selected_line, Some(1));
    press(&mut view, Key::Esc);
    assert_eq!(view.selected_line, Some(0));
}

#[test]
fn search_highlights_overlapping_unicode_matches_without_losing_syntax_styles() {
    use ratatui::style::Modifier;

    let area = Rect::new(0, 0, 40, 4);
    let mut view = search_view("let café = \"éÉé\";", area, false);
    let original = view.highlighted_text();
    query(&mut view, "éé");
    press(&mut view, Key::Char('n'));
    let mut styled = view.highlighted_text();
    view.highlight_search_matches(&mut styled);
    let chars = styled.lines[0]
        .spans
        .iter()
        .flat_map(|span| span.content.chars().map(move |c| (c, span.style)))
        .collect::<Vec<_>>();
    assert_eq!(
        chars.iter().map(|(c, _)| c).collect::<String>(),
        "let café = \"éÉé\";"
    );
    assert_eq!(chars[0].1, original.lines[0].spans[0].style);
    for (index, (_, style)) in chars.iter().enumerate().take(15).skip(12) {
        assert_eq!(style.fg, Some(theme().accent_fg()));
        assert_eq!(
            style
                .add_modifier
                .contains(Modifier::UNDERLINED | Modifier::BOLD),
            index >= 13
        );
    }
    let before = view.search_layout_state();
    let scroll = view.scroll.offset();
    render(&view, area);
    render(&view, area);
    assert_eq!(view.search_layout_state(), before);
    assert_eq!(view.scroll.offset(), scroll);
}
