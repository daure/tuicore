use super::*;

#[test]
fn gutter_overlay_preserves_geometry_content_and_resolved_selection_style() {
    let mut view = DataView::new([0, 1, 2], |row| *row)
        .column(Column::multiline(
            "value",
            "",
            Constraint::Fill(1),
            |row, _| match row {
                0 => Text::from("Header"),
                1 => Text::from(" First\n Second"),
                _ => Text::from("Neighbour"),
            },
        ))
        .row_height_by(|row| if *row == 1 { 2 } else { 1 });
    view.highlight_id(&1);
    view.focused = true;
    let area = Rect::new(3, 2, 16, 4);
    let geometry = view.scroll_geometry(area);
    let mut terminal = Terminal::new(TestBackend::new(24, 8)).unwrap();
    terminal.draw(|frame| view.render(frame, area)).unwrap();
    let baseline = terminal.backend().buffer().clone();

    view.set_left_gutter_overlay_by(|row, style| {
        if *row == 1 {
            assert_eq!(style.bg, Some(theme().selected_bg()));
        }
        Some(Span::styled("┃", Style::default().fg(theme().success_fg())))
    });
    assert_eq!(view.scroll_geometry(area).content, geometry.content);
    terminal.draw(|frame| view.render(frame, area)).unwrap();
    let actual = terminal.backend().buffer();
    for y in 0..8 {
        for x in 0..24 {
            let cell = actual.cell((x, y)).unwrap();
            let original = baseline.cell((x, y)).unwrap();
            if x == area.x && (3..5).contains(&y) {
                assert_eq!(cell.symbol(), "┃");
                assert_eq!(cell.fg, theme().success_fg());
                assert_eq!(cell.bg, original.bg);
            } else {
                assert_eq!(cell, original);
            }
        }
    }

    view.set_left_gutter_overlay_by(|_, _| None);
    terminal.draw(|frame| view.render(frame, area)).unwrap();
    assert_eq!(terminal.backend().buffer(), &baseline);
}

#[test]
fn gutter_overlay_tracks_clipped_multiline_rows() {
    let mut view = DataView::new([1, 2], |row| *row)
        .column(Column::multiline(
            "value",
            "",
            Constraint::Fill(1),
            |row, _| Text::from(format!(" {row} first\n {row} second")),
        ))
        .row_height(2)
        .left_gutter_overlay_by(|row, _| (*row == 1).then(|| Span::raw("┃")));
    let area = Rect::new(0, 0, 12, 2);
    let geometry = view.scroll_geometry(area);
    view.scroll.scroll_to(
        ScrollOffset::new(0, 1),
        geometry.viewport,
        geometry.content,
        AnimationSettings {
            enabled: false,
            ..Default::default()
        },
    );
    let mut terminal = Terminal::new(TestBackend::new(12, 2)).unwrap();
    terminal.draw(|frame| view.render(frame, area)).unwrap();
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), "┃");
    assert_eq!(buffer.cell((3, 0)).unwrap().symbol(), "s");
    assert_eq!(buffer.cell((0, 1)).unwrap().symbol(), " ");
    assert_eq!(buffer.cell((1, 1)).unwrap().symbol(), "2");
}
