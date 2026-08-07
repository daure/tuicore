use super::*;
use crate::{Key, KeyEvent, KeyModifiers, ScrollOffset, ScrollbarVisibility};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

fn rendered(viewer: &DiffViewer, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| viewer.render(frame, frame.area()))
        .unwrap();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| terminal.backend().buffer().cell((x, y)).unwrap().symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn layout(viewer: &mut DiffViewer, width: u16, height: u16) {
    <DiffViewer as TuiNode<()>>::layout(
        viewer,
        Rect::new(0, 0, width, height),
        &mut LayoutCtx::new(),
    );
}

#[test]
fn styles_render_distinct_marked_layouts() {
    let old = "one\ntwo\n";
    let new = "one\nthree\n";
    assert!(rendered(&DiffViewer::new(old, new), 60, 8).contains("│"));
    assert!(rendered(&DiffViewer::new(old, new).style(DiffStyle::Inline), 60, 8).contains("- two"));
    let raw = rendered(&DiffViewer::new(old, new).style(DiffStyle::RawPatch), 60, 8);
    assert!(raw.contains("--- old"));
    assert!(raw.contains("+++ new"));
    assert!(raw.contains("@@ -1,2 +1,2 @@"));
}

#[test]
fn word_style_highlights_unicode_change() {
    let viewer = DiffViewer::new("hello 🌍", "hello 🌎").style(DiffStyle::Word);
    let lines = viewer.styled_lines();
    assert!(lines.iter().flat_map(|line| &line.spans).any(|span| {
        span.content.contains('🌎') && span.style.bg == Some(theme().diff_added_emphasis_bg())
    }));
}

#[test]
fn measurement_obeys_row_bounds() {
    let short = DiffViewer::new("", "").min_rows(4).max_rows(7);
    let tall = DiffViewer::new("", (0..20).map(|n| format!("{n}\n")).collect::<String>())
        .min_rows(2)
        .max_rows(5);
    assert_eq!(
        <DiffViewer as TuiNode<()>>::measure(&short, LayoutProposal::unbounded())
            .preferred
            .height,
        4
    );
    assert_eq!(
        <DiffViewer as TuiNode<()>>::measure(&tall, LayoutProposal::unbounded())
            .preferred
            .height,
        5
    );
}

#[test]
fn row_bound_builders_are_order_independent_above_defaults() {
    let min_then_max = DiffViewer::new("", "").min_rows(30).max_rows(40);
    let max_then_min = DiffViewer::new("", "").max_rows(40).min_rows(30);

    assert_eq!(min_then_max.min_rows, 30);
    assert_eq!(min_then_max.max_rows, 40);
    assert_eq!(
        (min_then_max.min_rows, min_then_max.max_rows),
        (max_then_min.min_rows, max_then_min.max_rows)
    );
}

#[test]
fn rebuild_clamps_scroll_after_content_shrinks() {
    let old = (0..30).map(|n| format!("line {n}\n")).collect::<String>();
    let new = (0..30)
        .map(|n| match n {
            0 | 29 => format!("changed {n}\n"),
            _ => format!("line {n}\n"),
        })
        .collect::<String>();
    let area = Rect::new(0, 0, 12, 3);
    let settings = AnimationSettings {
        enabled: false,
        ..AnimationSettings::default()
    };
    let mut viewer = DiffViewer::new(old, new)
        .style(DiffStyle::Inline)
        .context_lines(30);
    let geometry = viewer.scroll_geometry(area);
    viewer.scroll.scroll_to(
        ScrollOffset::new(99, 99),
        geometry.viewport,
        geometry.content,
        settings,
    );
    viewer.area = area;
    let previous = viewer.scroll.offset();

    viewer.set_context_lines(0);

    let geometry = viewer.scroll_geometry(area);
    assert!(viewer.scroll.offset().y < previous.y);
    assert!(
        viewer.scroll.offset().y
            <= geometry
                .content
                .height
                .saturating_sub(geometry.viewport.height)
    );
    assert!(
        viewer.scroll.offset().x
            <= geometry
                .content
                .width
                .saturating_sub(geometry.viewport.width)
    );
}

#[test]
fn layout_clamps_scroll_when_viewport_grows() {
    let mut viewer = DiffViewer::new("", (0..10).map(|n| format!("{n}\n")).collect::<String>())
        .style(DiffStyle::Inline)
        .scrollbars(ScrollbarConfig {
            vertical: ScrollbarVisibility::Never,
            horizontal: ScrollbarVisibility::Never,
            ..ScrollbarConfig::default()
        });
    let small = Rect::new(0, 0, 20, 2);
    let settings = AnimationSettings {
        enabled: false,
        ..AnimationSettings::default()
    };
    let geometry = viewer.scroll_geometry(small);
    viewer.scroll.scroll_to(
        ScrollOffset::new(0, 99),
        geometry.viewport,
        geometry.content,
        settings,
    );
    assert!(viewer.scroll.offset().y > 0);

    <DiffViewer as TuiNode<()>>::layout(
        &mut viewer,
        Rect::new(0, 0, 20, 20),
        &mut LayoutCtx::new(),
    );

    assert_eq!(viewer.scroll.offset(), ScrollOffset::new(0, 0));
}

#[test]
fn side_by_side_header_separator_matches_unicode_body_separator() {
    let viewer = DiffViewer::new("α🌍\n", "β界\n").labels("旧🌍", "新界");
    let lines = viewer.plain_lines();
    let separator_columns = lines
        .iter()
        .filter_map(|line| line.split_once('│').map(|(left, _)| display_width(left)))
        .collect::<Vec<_>>();

    assert_eq!(separator_columns.len(), 3);
    assert!(
        separator_columns
            .iter()
            .all(|column| *column == separator_columns[0])
    );
    assert!(
        lines
            .iter()
            .any(|line| line.contains("@@") && line.contains('│'))
    );
}

#[test]
fn narrow_side_by_side_hunks_do_not_create_divider_only_spacer_rows() {
    let mut viewer = DiffViewer::new("one\ntwo\nthree\n", "one\nchanged\nthree\n")
        .show_headers(true)
        .wrap(true);
    layout(&mut viewer, 25, 20);

    assert!(
        viewer
            .display_plain_lines()
            .iter()
            .any(|line| line.contains("@@") && line.contains('│'))
    );
    assert!(
        !viewer
            .display_plain_lines()
            .iter()
            .any(|line| line.trim() == "│")
    );
}

#[test]
fn raw_patch_emits_valid_ranges_for_insert_delete_and_multiple_hunks() {
    let insertion = DiffViewer::new("", "追加\n").style(DiffStyle::RawPatch);
    assert_eq!(
        insertion.plain_lines(),
        ["--- old", "+++ new", "@@ -0,0 +1,1 @@", "+追加"]
    );

    let deletion = DiffViewer::new("gone\n", "").style(DiffStyle::RawPatch);
    assert_eq!(
        deletion.plain_lines(),
        ["--- old", "+++ new", "@@ -1,1 +0,0 @@", "-gone"]
    );

    let multiple = DiffViewer::new("a\nb\nc\nd\ne\n", "A\nb\nc\nd\nE\n")
        .style(DiffStyle::RawPatch)
        .context_lines(0);
    assert_eq!(
        multiple.plain_lines(),
        [
            "--- old",
            "+++ new",
            "@@ -1,1 +1,1 @@",
            "-a",
            "+A",
            "@@ -5,1 +5,1 @@",
            "-e",
            "+E",
        ]
    );
}

#[test]
fn raw_patch_places_newline_marker_after_affected_emitted_line() {
    let viewer = DiffViewer::new("", "last").style(DiffStyle::RawPatch);

    assert_eq!(
        viewer.plain_lines(),
        [
            "--- old",
            "+++ new",
            "@@ -0,0 +1,1 @@",
            "+last",
            "\\ No newline at end of file",
        ]
    );
}

#[test]
fn raw_patch_is_empty_for_identical_lf_text_with_trailing_newline() {
    let viewer = DiffViewer::new("same\ntext\n", "same\ntext\n").style(DiffStyle::RawPatch);

    assert_eq!(viewer.plain_lines(), Vec::<String>::new());
}

#[test]
fn raw_patch_is_empty_for_identical_text_without_trailing_newline() {
    let viewer = DiffViewer::new("same\ntext", "same\ntext").style(DiffStyle::RawPatch);

    assert_eq!(viewer.plain_lines(), Vec::<String>::new());
}

#[test]
fn raw_patch_is_empty_for_crlf_and_lf_normalized_equality() {
    let viewer = DiffViewer::new("same\r\ntext\r\n", "same\ntext\n").style(DiffStyle::RawPatch);

    assert_eq!(viewer.plain_lines(), Vec::<String>::new());
}

#[test]
fn non_raw_styles_show_identical_content() {
    for style in [DiffStyle::SideBySide, DiffStyle::Inline, DiffStyle::Word] {
        let viewer = DiffViewer::new("same\n", "same\n").style(style);

        assert!(
            viewer
                .plain_lines()
                .iter()
                .any(|line| line.contains("same"))
        );
    }
}

#[test]
fn tiny_areas_and_both_axis_scrolling_do_not_panic() {
    let mut viewer = DiffViewer::new("long old line\na\nb\n", "longer new line\nx\ny\n")
        .style(DiffStyle::Inline)
        .wrap(false)
        .scrollbars(ScrollbarConfig {
            vertical: ScrollbarVisibility::Never,
            horizontal: ScrollbarVisibility::Never,
            ..ScrollbarConfig::default()
        });
    let settings = AnimationSettings {
        enabled: false,
        ..AnimationSettings::default()
    };
    assert!(
        viewer
            .on_key_with_settings(Key::Down, Rect::new(0, 0, 4, 1), settings)
            .changed
    );
    assert!(
        viewer
            .on_key_with_settings(Key::Right, Rect::new(0, 0, 4, 1), settings)
            .changed
    );
    let _ = rendered(&viewer, 1, 1);

    viewer.set_wrap(true);
    layout(&mut viewer, 0, 0);
    layout(&mut viewer, 1, 1);
    let _ = rendered(&viewer, 1, 1);
}

#[test]
fn crlf_and_trailing_newline_are_handled() {
    let same = DiffViewer::new("a\r\nb\r\n", "a\nb\n").style(DiffStyle::RawPatch);
    assert!(!same.plain_lines().iter().any(|line| line.starts_with("@@")));
    let missing = DiffViewer::new("a\n", "a").style(DiffStyle::RawPatch);
    assert!(
        missing
            .plain_lines()
            .iter()
            .any(|line| line == "\\ No newline at end of file")
    );
}

#[test]
fn headers_toggle_removes_only_metadata_in_every_style() {
    let expected = [
        (DiffStyle::SideBySide, vec!["1 - one │ 1 + two"]),
        (DiffStyle::Inline, vec!["1   - one", "  1 + two"]),
        (DiffStyle::Word, vec!["1   - one", "  1 + two"]),
        (DiffStyle::RawPatch, vec!["-one", "+two"]),
    ];

    for (style, rows) in expected {
        let mut viewer = DiffViewer::new("one\n", "two\n").style(style);
        assert!(viewer.headers_visible());
        assert!(
            viewer
                .plain_lines()
                .iter()
                .any(|row| row.starts_with("@@") || row.contains("old"))
        );

        viewer.set_show_headers(false);

        assert!(!viewer.headers_visible());
        assert_eq!(viewer.plain_lines(), rows);
    }
}

#[test]
fn hiding_headers_keeps_no_newline_marker() {
    let viewer = DiffViewer::new("one\n", "two")
        .style(DiffStyle::RawPatch)
        .show_headers(false);

    assert_eq!(
        viewer.plain_lines(),
        ["-one", "+two", "\\ No newline at end of file"]
    );
}

#[test]
fn wrapping_disables_horizontal_scrolling_and_scrollbar() {
    let mut viewer = DiffViewer::new("short\n", "a very long changed line\n")
        .style(DiffStyle::Inline)
        .wrap(true);
    layout(&mut viewer, 10, 4);
    let geometry = viewer.scroll_geometry(Rect::new(0, 0, 10, 4));

    let outcome = viewer.on_key_with_settings(
        Key::Right,
        Rect::new(0, 0, 10, 4),
        AnimationSettings {
            enabled: false,
            ..AnimationSettings::default()
        },
    );

    assert!(viewer.is_wrapping());
    assert!(!outcome.handled);
    assert_eq!(viewer.scroll.offset().x, 0);
    assert!(geometry.layout.horizontal_bar.is_none());
    assert!(viewer.content_size().width <= geometry.viewport.width);
}

#[test]
fn unicode_and_leading_whitespace_wrap_into_vertically_scrollable_rows() {
    let mut viewer = DiffViewer::new("", "  αβγδεζηθ\n")
        .style(DiffStyle::RawPatch)
        .show_headers(false)
        .wrap(true);
    layout(&mut viewer, 5, 2);

    assert_eq!(viewer.display_plain_lines()[0], "+  α");
    assert!(viewer.display_plain_lines()[1].starts_with(" βγδ"));
    assert!(viewer.content_size().height > 2);
    assert!(
        viewer
            .on_key_with_settings(
                Key::Down,
                Rect::new(0, 0, 5, 2),
                AnimationSettings {
                    enabled: false,
                    ..AnimationSettings::default()
                },
            )
            .changed
    );
}

#[test]
fn resizing_recomputes_wrapped_height_and_clamps_vertical_offset() {
    let mut viewer = DiffViewer::new("", "abcdefghijklmnopqrstuvwxyz\n")
        .style(DiffStyle::RawPatch)
        .show_headers(false)
        .wrap(true);
    let settings = AnimationSettings {
        enabled: false,
        ..AnimationSettings::default()
    };
    layout(&mut viewer, 5, 2);
    let narrow_height = viewer.content_size().height;
    let geometry = viewer.scroll_geometry(Rect::new(0, 0, 5, 2));
    viewer.scroll.scroll_to(
        ScrollOffset::new(0, usize::MAX),
        geometry.viewport,
        geometry.content,
        settings,
    );
    assert!(viewer.scroll.offset().y > 0);

    layout(&mut viewer, 40, 5);

    assert!(viewer.content_size().height < narrow_height);
    assert_eq!(viewer.scroll.offset().y, 0);
}

#[test]
fn side_by_side_wrapping_aligns_every_continuation_divider() {
    let mut viewer = DiffViewer::new(
        "left side has substantially more text\n",
        "right side is even longer than the left side text\n",
    )
    .show_headers(false)
    .wrap(true);
    layout(&mut viewer, 25, 10);
    let columns = viewer
        .display_plain_lines()
        .iter()
        .filter_map(|row| row.split_once('│').map(|(left, _)| display_width(left)))
        .collect::<Vec<_>>();

    assert!(columns.len() > 1);
    assert!(columns.iter().all(|column| *column == columns[0]));
    assert!(viewer.display_plain_lines()[1].starts_with("    "));
}

#[test]
fn word_emphasis_survives_wrapping() {
    let mut viewer = DiffViewer::new("prefix oldword suffix", "prefix newword suffix")
        .style(DiffStyle::Word)
        .show_headers(false)
        .wrap(true);
    layout(&mut viewer, 10, 10);

    let emphasized = viewer
        .styled_lines()
        .iter()
        .flat_map(|line| &line.spans)
        .filter(|span| span.style.bg == Some(theme().diff_added_emphasis_bg()))
        .map(|span| span.content.as_ref())
        .collect::<String>();
    assert!(emphasized.contains("newword"));
}

#[test]
fn raw_patch_logical_rows_do_not_change_when_wrapping_toggles() {
    let mut viewer =
        DiffViewer::new("before\n", "after with a long line\n").style(DiffStyle::RawPatch);
    let logical = viewer.plain_lines();

    viewer.set_wrap(true);
    layout(&mut viewer, 8, 4);
    assert_ne!(viewer.display_plain_lines(), logical);
    assert_eq!(viewer.plain_lines(), logical);

    viewer.set_wrap(false);
    assert_eq!(viewer.plain_lines(), logical);
}

#[test]
fn navigation_selects_logical_rows_and_exposes_new_file_location() {
    let mut viewer = DiffViewer::new("same\nbefore\nlast\n", "same\nafter\nlast\n")
        .style(DiffStyle::Inline)
        .wrap(false);
    let settings = AnimationSettings {
        enabled: false,
        ..AnimationSettings::default()
    };

    assert_eq!(
        viewer.selected_location(),
        Some(DiffLocation {
            old_line: Some(1),
            new_line: Some(1),
        })
    );
    assert!(
        viewer
            .on_key_with_settings(Key::Down, Rect::new(0, 0, 40, 3), settings)
            .changed
    );
    assert_eq!(
        viewer.selected_location(),
        Some(DiffLocation {
            old_line: Some(2),
            new_line: Some(2),
        })
    );
}

#[test]
fn focused_selection_uses_data_view_highlight_colors_for_paired_rows() {
    let mut viewer = DiffViewer::new("same\nbefore\n", "same\nafter\n")
        .style(DiffStyle::Inline)
        .wrap(false)
        .focused(true);
    let settings = AnimationSettings {
        enabled: false,
        ..AnimationSettings::default()
    };
    viewer.on_key_with_settings(Key::Down, Rect::new(0, 0, 40, 3), settings);

    let highlighted = viewer
        .styled_lines()
        .into_iter()
        .filter(|line| line.to_string().contains("before") || line.to_string().contains("after"))
        .flat_map(|line| line.spans)
        .collect::<Vec<_>>();
    assert!(!highlighted.is_empty());
    assert!(highlighted.iter().all(|span| {
        span.style.fg == Some(theme().highlight_fg())
            && span.style.bg == Some(theme().highlight_bg())
    }));
}

#[test]
fn page_home_end_and_gg_move_selection_and_center_page_jumps() {
    let old = (0..30)
        .map(|line| format!("before {line}\n"))
        .collect::<String>();
    let new = (0..30)
        .map(|line| format!("after {line}\n"))
        .collect::<String>();
    let mut viewer = DiffViewer::new(old, new)
        .style(DiffStyle::Inline)
        .wrap(false);
    let area = Rect::new(0, 0, 40, 5);
    let settings = AnimationSettings {
        enabled: false,
        ..AnimationSettings::default()
    };
    layout(&mut viewer, area.width, area.height);

    assert!(
        viewer
            .on_key_with_settings(Key::PageDown, area, settings)
            .changed
    );
    let selected = viewer.selected_location().expect("selection should exist");
    assert!(selected.new_line.expect("new line") > 1);
    let selected_row = viewer
        .display_parts
        .iter()
        .position(|line| line.location == Some(selected))
        .expect("selected display row should exist");
    let geometry = viewer.scroll_geometry(area);
    let expected = selected_row
        .saturating_sub(geometry.viewport.height / 2)
        .min(
            geometry
                .content
                .height
                .saturating_sub(geometry.viewport.height),
        );
    assert_eq!(viewer.scroll.offset().y, expected);

    assert!(
        viewer
            .on_key_with_settings(
                KeyEvent {
                    code: Key::Char('d'),
                    modifiers: KeyModifiers::CONTROL,
                },
                area,
                settings,
            )
            .changed
    );
    let after_ctrl_d = viewer.selected_location().unwrap().new_line.unwrap();
    assert!(
        viewer
            .on_key_with_settings(
                KeyEvent {
                    code: Key::Char('u'),
                    modifiers: KeyModifiers::CONTROL,
                },
                area,
                settings,
            )
            .changed
    );
    assert!(viewer.selected_location().unwrap().new_line.unwrap() < after_ctrl_d);

    assert!(
        viewer
            .on_key_with_settings(Key::End, area, settings)
            .changed
    );
    assert_eq!(viewer.selected_location().unwrap().new_line, Some(30));
    assert!(
        viewer
            .on_key_with_settings(Key::Home, area, settings)
            .changed
    );
    assert_eq!(viewer.selected_location().unwrap().new_line, Some(1));

    assert!(
        viewer
            .on_key_with_settings(Key::Char('G'), area, settings)
            .changed
    );
    assert_eq!(viewer.selected_location().unwrap().new_line, Some(30));
    assert!(
        !viewer
            .on_key_with_settings(Key::Char('g'), area, settings)
            .changed
    );
    assert!(
        viewer
            .on_key_with_settings(Key::Char('g'), area, settings)
            .changed
    );
    assert_eq!(viewer.selected_location().unwrap().new_line, Some(1));
}

#[test]
fn line_navigation_centers_selection_immediately() {
    let old = (0..20)
        .map(|line| format!("before {line}\n"))
        .collect::<String>();
    let new = (0..20)
        .map(|line| format!("after {line}\n"))
        .collect::<String>();
    let mut viewer = DiffViewer::new(old, new)
        .style(DiffStyle::Inline)
        .wrap(false);
    let area = Rect::new(0, 0, 40, 5);
    layout(&mut viewer, area.width, area.height);

    for _ in 0..5 {
        assert!(
            viewer
                .on_key_with_settings(Key::Down, area, AnimationSettings::default())
                .changed
        );
        let selected = viewer.selected_location().unwrap();
        let row = viewer
            .display_parts
            .iter()
            .position(|line| line.location == Some(selected))
            .unwrap();
        let geometry = viewer.scroll_geometry(area);
        let expected = row.saturating_sub(geometry.viewport.height / 2).min(
            geometry
                .content
                .height
                .saturating_sub(geometry.viewport.height),
        );
        assert_eq!(viewer.scroll.offset().y, expected);
        assert!(!viewer.scroll.is_active());
    }
}

#[test]
fn page_navigation_animates_toward_the_centered_selection() {
    let old = (0..30)
        .map(|line| format!("before {line}\n"))
        .collect::<String>();
    let new = (0..30)
        .map(|line| format!("after {line}\n"))
        .collect::<String>();
    let mut viewer = DiffViewer::new(old, new)
        .style(DiffStyle::Inline)
        .wrap(false);
    let area = Rect::new(0, 0, 40, 5);
    layout(&mut viewer, area.width, area.height);

    let outcome = viewer.on_key_with_settings(Key::PageDown, area, AnimationSettings::default());

    assert!(outcome.changed);
    assert!(outcome.active);
    assert!(viewer.scroll.offset().y < viewer.scroll.target_offset().y);
}

#[test]
fn resize_recenters_the_selected_row_immediately() {
    let old = (0..20)
        .map(|line| format!("before {line}\n"))
        .collect::<String>();
    let new = (0..20)
        .map(|line| format!("after {line}\n"))
        .collect::<String>();
    let mut viewer = DiffViewer::new(old, new)
        .style(DiffStyle::Inline)
        .wrap(false);
    let small = Rect::new(0, 0, 40, 5);
    layout(&mut viewer, small.width, small.height);
    for _ in 0..8 {
        viewer.on_key_with_settings(
            Key::Down,
            small,
            AnimationSettings {
                enabled: false,
                ..AnimationSettings::default()
            },
        );
    }

    let large = Rect::new(0, 0, 40, 9);
    layout(&mut viewer, large.width, large.height);

    let selected = viewer.selected_location().unwrap();
    let row = viewer
        .display_parts
        .iter()
        .position(|line| line.location == Some(selected))
        .unwrap();
    let geometry = viewer.scroll_geometry(large);
    let expected = row.saturating_sub(geometry.viewport.height / 2).min(
        geometry
            .content
            .height
            .saturating_sub(geometry.viewport.height),
    );
    assert_eq!(viewer.scroll.offset().y, expected);
    assert!(!viewer.scroll.is_active());
}

#[test]
fn focused_selection_paints_the_entire_viewport_row() {
    let mut viewer = DiffViewer::new("same\nbefore\n", "same\nafter\n")
        .style(DiffStyle::Inline)
        .show_headers(false)
        .wrap(false)
        .focused(true);
    layout(&mut viewer, 40, 4);
    let mut terminal = Terminal::new(TestBackend::new(40, 4)).unwrap();
    terminal
        .draw(|frame| viewer.render(frame, frame.area()))
        .unwrap();

    let buffer = terminal.backend().buffer();
    assert_eq!(
        buffer.cell((39, 0)).unwrap().bg,
        theme().highlight_bg(),
        "highlight should fill beyond line content"
    );
}
