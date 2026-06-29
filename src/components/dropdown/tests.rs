use std::cell::RefCell;
use std::hash::Hash;
use std::rc::Rc;

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::{Frame, Terminal};

use super::*;
use crate::event::KeyModifiers;
use crate::{
    ChildKey, Dialog, DialogLayer, EventCtx, EventRoute, Flex, FlexItem, FocusCtx, FocusId,
    FocusRequest, KeyBindings, KeySpec, LayoutCtx, LayoutProposal, NonFocusable, Propagation,
    RenderCtx, Tab, Tabs, TuiEvent, TuiNode,
};

fn single_dropdown() -> Dropdown<&'static str, &'static str> {
    Dropdown::single(ROWS, |row| *row, |row| row.to_string())
}

fn multi_dropdown() -> Dropdown<&'static str, &'static str> {
    Dropdown::multi(ROWS, |row| *row, |row| row.to_string())
}

fn numeric_dropdown(count: u8) -> Dropdown<u8, u8> {
    Dropdown::single(0..count, |row| *row, |row| row.to_string())
}

fn render_dropdown<T, Id>(dropdown: &Dropdown<T, Id>, frame: &mut Frame<'_>, area: Rect)
where
    T: 'static,
    Id: Clone + Eq + Hash + 'static,
{
    let mut ctx = RenderCtx::new();
    dropdown.render(frame, area, &mut ctx);
    ctx.flush(frame);
}

fn layout_dropdown<T, Id>(dropdown: &mut Dropdown<T, Id>, area: Rect, bounds: Rect) -> LayoutCtx
where
    T: 'static,
    Id: Clone + Eq + Hash + 'static,
{
    let mut ctx = LayoutCtx::new();
    ctx.with_overlay_bounds(bounds, |ctx| {
        <Dropdown<_, _> as TuiNode<()>>::layout(dropdown, area, ctx);
    });
    ctx
}

struct NonForwardingComposite {
    dropdown: Dropdown<&'static str, &'static str>,
}

struct DialogControlsTabBody {
    dropdown: Dropdown<&'static str, &'static str>,
    dropdown_area: Rect,
}

struct EmptyNode;

impl TuiNode<()> for NonForwardingComposite {
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        <Dropdown<_, _> as TuiNode<()>>::layout(&mut self.dropdown, area, ctx)
    }

    fn render<'a>(&'a self, frame: &mut ratatui::Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        <Dropdown<_, _> as TuiNode<()>>::render(&self.dropdown, frame, area, ctx);
    }
}

impl DialogControlsTabBody {
    fn open() -> Self {
        let mut dropdown = single_dropdown();
        dropdown.open();
        Self {
            dropdown,
            dropdown_area: Rect::default(),
        }
    }
}

impl TuiNode<()> for EmptyNode {
    fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
        LayoutResult::new(area)
    }

    fn render(&self, _frame: &mut ratatui::Frame, _area: Rect, _ctx: &mut RenderCtx<'_>) {}
}

impl TuiNode<()> for DialogControlsTabBody {
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.dropdown_area = Rect::new(area.x, area.y, area.width.min(12), 3.min(area.height));
        ctx.push_slot(ChildKey::from("dropdown"), self.dropdown_area, |ctx| {
            <Dropdown<_, _> as TuiNode<()>>::layout(&mut self.dropdown, self.dropdown_area, ctx);
        });
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut ratatui::Frame, _area: Rect, ctx: &mut RenderCtx<'a>) {
        self.dropdown.render(frame, self.dropdown_area, ctx);
    }
}

const ROWS: [&str; 3] = ["Alpha", "Beta", "Gamma"];
const AREA: Rect = Rect::new(0, 0, 24, 10);

struct KeyBindingsGuard {
    previous: KeyBindings,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl KeyBindingsGuard {
    fn replace(next: KeyBindings) -> Self {
        let lock = crate::ENV_LOCK.lock().expect("test env lock should lock");
        let previous = keybindings();
        crate::set_keybindings(next);
        Self {
            previous,
            _lock: lock,
        }
    }
}

impl Drop for KeyBindingsGuard {
    fn drop(&mut self) {
        crate::set_keybindings(self.previous.clone());
    }
}

#[test]
fn custom_action_keys_open_and_commit_dropdown() {
    let keys = DropdownActionKeys {
        open: vec![KeySpec::plain('o')],
        commit: vec![KeySpec::plain('c')],
        toggle: vec![KeySpec::plain('t')],
    };
    let mut dropdown = single_dropdown().action_keys(keys);

    assert!(!dropdown.on_key(KeyEvent::from(Key::Enter), AREA).handled);
    assert!(dropdown.on_key(KeyEvent::from(Key::Char('o')), AREA).opened);
    assert!(
        dropdown
            .on_key(KeyEvent::from(Key::Char('c')), AREA)
            .committed
    );
}

#[test]
fn open_popup_dims_backdrop_but_not_trigger() {
    let mut dropdown = single_dropdown()
        .selected_one("Beta")
        .variant(DropdownVariant::Filled);
    dropdown.open();
    layout_dropdown(&mut dropdown, Rect::new(0, 0, 12, 1), AREA);
    let mut terminal = Terminal::new(TestBackend::new(24, 10)).expect("terminal should build");

    terminal
        .draw(|frame| {
            frame.buffer_mut().set_string(
                0,
                9,
                "X",
                Style::default()
                    .fg(Color::Rgb(200, 200, 200))
                    .bg(Color::Rgb(10, 20, 30)),
            );
            render_dropdown(&dropdown, frame, Rect::new(0, 0, 12, 1));
        })
        .expect("dropdown should render");

    let buffer = terminal.backend().buffer();
    let backdrop_cell = buffer.cell((0, 9)).unwrap();
    assert_ne!(backdrop_cell.fg, Color::Rgb(200, 200, 200));
    assert!(backdrop_cell.modifier.contains(Modifier::DIM));

    let trigger_text = (0..12)
        .map(|x| buffer.cell((x, 0)).unwrap().symbol())
        .collect::<String>();
    assert!(trigger_text.contains("Beta"), "{trigger_text}");
    assert!(
        !buffer
            .cell((1, 0))
            .unwrap()
            .modifier
            .contains(Modifier::DIM)
    );
}

#[test]
fn normal_render_plus_overlay_dims_backdrop_once() {
    let mut baseline = single_dropdown()
        .selected_one("Beta")
        .variant(DropdownVariant::Filled);
    baseline.open();
    layout_dropdown(&mut baseline, Rect::new(0, 0, 12, 1), AREA);
    let mut baseline_terminal =
        Terminal::new(TestBackend::new(24, 10)).expect("terminal should build");
    baseline_terminal
        .draw(|frame| {
            frame.buffer_mut().set_string(
                0,
                9,
                "X",
                Style::default()
                    .fg(Color::Rgb(200, 200, 200))
                    .bg(Color::Rgb(10, 20, 30)),
            );
            render_dropdown(&baseline, frame, Rect::new(0, 0, 12, 1));
        })
        .expect("dropdown should render");

    let mut dropdown = single_dropdown()
        .selected_one("Beta")
        .variant(DropdownVariant::Filled);
    dropdown.open();
    layout_dropdown(&mut dropdown, Rect::new(0, 0, 12, 1), AREA);
    let mut terminal = Terminal::new(TestBackend::new(24, 10)).expect("terminal should build");
    terminal
        .draw(|frame| {
            frame.buffer_mut().set_string(
                0,
                9,
                "X",
                Style::default()
                    .fg(Color::Rgb(200, 200, 200))
                    .bg(Color::Rgb(10, 20, 30)),
            );
            render_dropdown(&dropdown, frame, Rect::new(0, 0, 12, 1));
        })
        .expect("dropdown should render");

    let expected = baseline_terminal.backend().buffer().cell((0, 9)).unwrap();
    let actual = terminal.backend().buffer().cell((0, 9)).unwrap();
    assert_eq!(actual.fg, expected.fg);
    assert_eq!(actual.bg, expected.bg);
    assert_eq!(actual.modifier, expected.modifier);
}

#[test]
fn opening_from_event_tweens_backdrop_dim() {
    let mut dropdown = single_dropdown();
    layout_dropdown(&mut dropdown, Rect::new(0, 0, 12, 1), AREA);
    let mut ctx = EventCtx::<()>::new(AnimationSettings::default());

    let outcome = dropdown.event(&TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut ctx);

    assert!(outcome.handled());
    assert!(dropdown.is_open());
    assert_eq!(dropdown.backdrop_tween.value(), 0.0);
    assert!(dropdown.backdrop_tween.is_active());

    Animated::tick(
        &mut dropdown,
        Duration::from_millis(125),
        AnimationSettings::default(),
    );

    assert!(dropdown.backdrop_tween.value() > 0.0);
    assert!(dropdown.backdrop_tween.value() < DROPDOWN_BACKDROP_AMOUNT);
}

#[test]
fn open_clones_committed_selection_to_draft() {
    let mut dropdown = single_dropdown().selected_one("Beta");

    dropdown.open();
    dropdown.on_key(ctrl('j'), AREA);
    dropdown.cancel();

    assert_eq!(dropdown.selected_id(), Some("Beta"));
}

#[test]
fn cancel_when_closed_preserves_committed_selection() {
    let mut dropdown = single_dropdown().selected_one("Beta");

    let outcome = dropdown.cancel();

    assert_eq!(dropdown.selected_id(), Some("Beta"));
    assert_eq!(outcome, DropdownOutcome::HANDLED);
}

#[test]
fn enter_commits_single_draft() {
    let mut dropdown = single_dropdown();

    dropdown.open();
    dropdown.on_key(ctrl('j'), AREA);
    dropdown.on_key(Key::Enter, AREA);

    assert_eq!(dropdown.selected_id(), Some("Beta"));
    assert!(!dropdown.is_open());
}

#[test]
fn ctrl_d_and_ctrl_u_page_navigate_defaults() {
    let mut dropdown = single_dropdown();

    dropdown.open();
    dropdown.on_key(ctrl('d'), AREA);
    assert_eq!(dropdown.data_view.highlighted_id(), Some("Gamma"));

    dropdown.on_key(ctrl('u'), AREA);
    assert_eq!(dropdown.data_view.highlighted_id(), Some("Alpha"));
}

#[test]
fn closed_ctrl_j_and_ctrl_k_open_with_navigating() {
    let mut dropdown = single_dropdown();
    let outcome = dropdown.on_key(ctrl('j'), AREA);
    assert!(outcome.opened);
    assert!(dropdown.is_open());
    assert_eq!(dropdown.data_view.highlighted_id(), Some("Beta"));

    let mut dropdown = single_dropdown();
    let outcome = dropdown.on_key(ctrl('k'), AREA);
    assert!(outcome.opened);
    assert!(dropdown.is_open());
    assert_eq!(dropdown.data_view.highlighted_id(), Some("Alpha"));
}

#[test]
fn closed_plain_j_and_k_do_not_open() {
    for key in [char_key('j'), char_key('k')] {
        let mut dropdown = single_dropdown();

        let outcome = dropdown.on_key(key, AREA);

        assert!(!outcome.opened);
        assert!(!dropdown.is_open());
    }
}

#[test]
fn ctrl_d_and_ctrl_u_page_navigation_moves_by_visible_page_step() {
    let mut dropdown = numeric_dropdown(20);

    dropdown.open();
    dropdown.on_key(char_key('1'), AREA);
    dropdown.on_key(ctrl('d'), AREA);

    assert_eq!(dropdown.search_query(), "1");
    assert!(dropdown.data_view.highlighted_id().unwrap() > 1);

    dropdown.on_key(ctrl('u'), AREA);
    assert_eq!(dropdown.search_query(), "1");
    assert_eq!(dropdown.data_view.highlighted_id(), Some(1));
}

#[test]
fn escape_rolls_back_single_draft() {
    let mut dropdown = single_dropdown().selected_one("Alpha");

    dropdown.open();
    dropdown.on_key(ctrl('j'), AREA);
    dropdown.on_key(Key::Esc, AREA);

    assert_eq!(dropdown.selected_id(), Some("Alpha"));
    assert!(!dropdown.is_open());
}

#[test]
fn configured_unfocus_key_cancels_open_dropdown() {
    let _guard = KeyBindingsGuard::replace(
        KeyBindings::new().with_focus_unfocus([KeySpec::key(Key::Esc), KeySpec::plain('q')]),
    );
    let mut dropdown = single_dropdown().selected_one("Alpha");

    dropdown.open();
    dropdown.on_key(ctrl('j'), AREA);
    dropdown.on_key(char_key('q'), AREA);

    assert_eq!(dropdown.selected_id(), Some("Alpha"));
    assert!(!dropdown.is_open());
}

#[test]
fn typing_search_filters_rows_before_commit() {
    let mut dropdown = single_dropdown();

    dropdown.open();
    dropdown.on_key(char_key('g'), AREA);
    dropdown.on_key(char_key('a'), AREA);
    dropdown.on_key(Key::Enter, AREA);

    assert_eq!(dropdown.selected_id(), Some("Gamma"));
}

#[test]
fn enter_commit_clears_search_query() {
    let mut dropdown = single_dropdown();

    dropdown.open();
    dropdown.on_key(char_key('g'), AREA);
    dropdown.on_key(Key::Enter, AREA);

    assert_eq!(dropdown.selected_id(), Some("Gamma"));
    assert_eq!(dropdown.search_query(), "");
}

#[test]
fn escape_cancel_clears_search_query_and_filter() {
    let mut dropdown = single_dropdown();

    dropdown.open();
    dropdown.on_key(char_key('g'), AREA);
    dropdown.on_key(Key::Esc, AREA);

    assert_eq!(dropdown.search_query(), "");
    assert_eq!(dropdown.filtered, ROWS.to_vec());
}

#[test]
fn tab_while_open_cancels_and_requests_next_focus() {
    let mut dropdown = single_dropdown();
    dropdown.open();
    dropdown.on_key(char_key('g'), AREA);
    let mut ctx = EventCtx::<()>::default();

    let outcome = dropdown.event(&TuiEvent::Key(Key::Tab.into()), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(!dropdown.is_open());
    assert_eq!(dropdown.search_query(), "");
    assert_eq!(ctx.focus_request(), Some(&FocusRequest::Next));
    assert_eq!(ctx.propagation(), Propagation::Stopped);
}

#[test]
fn hotkey_open_requests_search_focus_at_dropdown_path() {
    let mut flex: Flex<()> = Flex::row()
        .child("first", single_dropdown().hotkey("f"), FlexItem::fixed(12))
        .child("second", single_dropdown().hotkey("s"), FlexItem::fixed(12));
    let mut layout = LayoutCtx::new();
    flex.layout(AREA, &mut layout);
    let target = layout
        .focus_targets()
        .iter()
        .find(|target| target.hotkey_sequences == ["s".to_string()])
        .expect("second dropdown target should exist")
        .clone();
    let mut ctx = EventCtx::<()>::default();

    let outcome = flex.dispatch_event(
        &EventRoute::new(target.path.clone()),
        &TuiEvent::Hotkey(HotkeyEvent::Commit("s".into())),
        &mut ctx,
    );

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(
        ctx.focus_request(),
        Some(&FocusRequest::TargetAt {
            path: target.path,
            id: FocusId::new(SEARCH_FOCUS),
        })
    );
}

#[test]
fn dropdown_navigation_preserves_search_query() {
    let mut dropdown = single_dropdown();

    dropdown.open();
    dropdown.on_key(char_key('a'), AREA);
    dropdown.on_key(ctrl('j'), AREA);

    assert_eq!(dropdown.search_query(), "a");
}

#[test]
fn contains_search_requires_contiguous_match() {
    let mut dropdown = single_dropdown().search_mode(DropdownSearchMode::Contains);

    dropdown.open();
    dropdown.on_key(char_key('m'), AREA);
    dropdown.on_key(char_key('m'), AREA);
    dropdown.on_key(Key::Enter, AREA);

    assert_eq!(dropdown.selected_id(), Some("Gamma"));
}

#[test]
fn open_popup_highlights_matching_search_characters() {
    let mut dropdown = single_dropdown();
    dropdown.open();
    dropdown.on_key(char_key('a'), AREA);
    dropdown.on_key(char_key('l'), AREA);
    let mut terminal = Terminal::new(TestBackend::new(16, 6)).expect("terminal should build");

    terminal
        .draw(|frame| dropdown.render_popup(frame, frame.area()))
        .expect("dropdown should render");

    let buffer = terminal.backend().buffer();
    assert!(
        buffer
            .cell((1, 2))
            .unwrap()
            .modifier
            .contains(Modifier::UNDERLINED)
    );
    assert!(
        buffer
            .cell((2, 2))
            .unwrap()
            .modifier
            .contains(Modifier::UNDERLINED)
    );
    assert!(
        !buffer
            .cell((3, 2))
            .unwrap()
            .modifier
            .contains(Modifier::UNDERLINED)
    );
}

#[test]
fn disabled_search_ignores_typing() {
    let mut dropdown = single_dropdown().search_mode(DropdownSearchMode::None);

    dropdown.open();
    dropdown.on_key(char_key('g'), AREA);
    dropdown.on_key(Key::Enter, AREA);

    assert_eq!(dropdown.search_query(), "");
    assert_eq!(dropdown.selected_id(), Some("Alpha"));
}

#[test]
fn immediate_commit_updates_selection_while_open() {
    let mut dropdown = single_dropdown()
        .commit_mode(DropdownCommitMode::Immediate)
        .selected_one("Alpha");

    dropdown.open();
    let outcome = dropdown.on_key(ctrl('j'), AREA);

    assert!(outcome.committed);
    assert!(dropdown.is_open());
    assert_eq!(dropdown.selected_id(), Some("Beta"));
}

#[test]
fn immediate_commit_calls_on_select_when_highlight_changes() {
    let selected = Rc::new(RefCell::new(Vec::new()));
    let captured = Rc::clone(&selected);
    let mut dropdown = single_dropdown()
        .commit_mode(DropdownCommitMode::Immediate)
        .selected_one("Alpha")
        .on_select(move |ids| *captured.borrow_mut() = ids);

    dropdown.open();
    dropdown.on_key(ctrl('j'), AREA);

    assert_eq!(&*selected.borrow(), &["Beta"]);
}

#[test]
fn multi_close_on_select_calls_on_select() {
    let selected = Rc::new(RefCell::new(Vec::new()));
    let captured = Rc::clone(&selected);
    let mut dropdown = multi_dropdown()
        .close_on_select(true)
        .on_select(move |ids| *captured.borrow_mut() = ids);

    dropdown.open();
    dropdown.on_key(ctrl('j'), AREA);
    let outcome = dropdown.on_key(Key::Char(' '), AREA);

    assert!(outcome.committed);
    assert_eq!(&*selected.borrow(), &["Beta"]);
}

#[test]
fn immediate_commit_updates_selection_while_filtering() {
    let mut dropdown = single_dropdown()
        .commit_mode(DropdownCommitMode::Immediate)
        .selected_one("Alpha");

    dropdown.open();
    let outcome = dropdown.on_key(char_key('g'), AREA);

    assert!(outcome.committed);
    assert!(dropdown.is_open());
    assert_eq!(dropdown.selected_id(), Some("Gamma"));
}

#[test]
fn immediate_enter_closes_without_changing_current_selection() {
    let mut dropdown = single_dropdown()
        .commit_mode(DropdownCommitMode::Immediate)
        .selected_one("Alpha");

    dropdown.open();
    dropdown.on_key(char_key('g'), AREA);
    let outcome = dropdown.on_key(Key::Enter, AREA);

    assert!(outcome.closed);
    assert!(!dropdown.is_open());
    assert_eq!(dropdown.selected_id(), Some("Gamma"));
}

#[test]
fn immediate_escape_restores_value_from_before_open() {
    let mut dropdown = single_dropdown()
        .commit_mode(DropdownCommitMode::Immediate)
        .selected_one("Alpha");

    dropdown.open();
    dropdown.on_key(char_key('g'), AREA);
    dropdown.on_key(Key::Esc, AREA);

    assert!(!dropdown.is_open());
    assert_eq!(dropdown.selected_id(), Some("Alpha"));
}

#[test]
fn immediate_ctrl_left_bracket_restores_value_from_before_open() {
    let mut dropdown = single_dropdown()
        .commit_mode(DropdownCommitMode::Immediate)
        .selected_one("Alpha");

    dropdown.open();
    dropdown.on_key(char_key('g'), AREA);
    dropdown.on_key(ctrl('['), AREA);

    assert!(!dropdown.is_open());
    assert_eq!(dropdown.selected_id(), Some("Alpha"));
}

#[test]
fn explicit_single_keeps_trigger_value_until_commit() {
    let mut dropdown = single_dropdown().selected_one("Alpha");

    dropdown.open();
    dropdown.on_key(ctrl('j'), AREA);

    assert_eq!(dropdown.selected_summary(), "Alpha");
    assert_eq!(dropdown.selected_id(), Some("Alpha"));
}

#[test]
fn open_highlights_committed_selection() {
    let mut dropdown = single_dropdown()
        .search_mode(DropdownSearchMode::None)
        .selected_one("Beta");

    dropdown.open();

    assert_eq!(dropdown.data_view.highlighted_id(), Some("Beta"));
}

#[test]
fn searchable_dropdown_keeps_field_focus_until_runtime_focuses_search() {
    let mut dropdown = single_dropdown();
    let mut layout = LayoutCtx::new();
    <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut layout);
    let field = layout.focus_targets()[0].clone();
    let mut focus = FocusCtx::<()>::new(AnimationSettings::default());

    dropdown.dispatch_focus(&field, true, &mut focus);
    dropdown.open();

    assert_eq!(dropdown.focus_region, Some(DropdownFocusRegion::Field));
    assert!(dropdown.data_view.focused_for_test());

    let mut open_layout = LayoutCtx::new();
    <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut open_layout);
    let search = open_layout.focus_targets()[0].clone();
    dropdown.dispatch_focus(&field, false, &mut focus);
    dropdown.dispatch_focus(&search, true, &mut focus);

    assert_eq!(dropdown.focus_region, Some(DropdownFocusRegion::Search));
    assert!(dropdown.data_view.focused_for_test());
}

#[test]
fn open_preserves_unfocused_state() {
    let mut dropdown = single_dropdown();

    dropdown.open();

    assert!(dropdown.is_open());
    assert!(!dropdown.is_focused());
    assert!(!dropdown.data_view.focused_for_test());
}

#[test]
fn multi_toggle_then_escape_rolls_back() {
    let mut dropdown = multi_dropdown().selected(["Alpha"]);

    dropdown.open();
    dropdown.on_key(ctrl('j'), AREA);
    dropdown.on_key(Key::Char(' '), AREA);
    dropdown.on_key(Key::Esc, AREA);

    assert_eq!(dropdown.selected_ids(), vec!["Alpha"]);
}

#[test]
fn ctrl_space_toggles_highlighted_multi_row() {
    let mut dropdown = multi_dropdown();

    dropdown.open();
    dropdown.on_key(ctrl('j'), AREA);
    dropdown.on_key(ctrl(' '), AREA);

    assert_eq!(dropdown.draft, vec!["Beta"]);
}

#[test]
fn ctrl_space_commits_highlighted_single_row() {
    let mut dropdown = single_dropdown();

    dropdown.open();
    dropdown.on_key(ctrl('j'), AREA);
    let outcome = dropdown.on_key(ctrl(' '), AREA);

    assert!(outcome.committed);
    assert_eq!(dropdown.selected_id(), Some("Beta"));
    assert!(!dropdown.is_open());
}

#[test]
fn closed_layout_registers_field_focus() {
    let mut dropdown = single_dropdown();
    let mut ctx = LayoutCtx::new();

    <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut ctx);

    let targets = ctx.focus_targets();
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].id.as_str(), FIELD_FOCUS);
    assert!(targets[0].path.is_empty());
}

#[test]
fn filled_variant_registers_compact_field_focus() {
    let mut dropdown = single_dropdown().variant(DropdownVariant::Filled);
    let mut ctx = LayoutCtx::new();

    <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut ctx);

    let targets = ctx.focus_targets();
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].id.as_str(), FIELD_FOCUS);
    assert_eq!(targets[0].area.height, 1);
}

#[test]
fn bordered_dropdown_measure_reports_field_height() {
    let dropdown = single_dropdown();

    let hint = <Dropdown<_, _> as TuiNode<()>>::measure(&dropdown, LayoutProposal::unbounded());

    assert_eq!(hint.preferred.height, 3);
    assert!(!hint.expand.height);
}

#[test]
fn filled_dropdown_measure_reports_compact_field_height() {
    let dropdown = single_dropdown().variant(DropdownVariant::Filled);

    let hint = <Dropdown<_, _> as TuiNode<()>>::measure(&dropdown, LayoutProposal::unbounded());

    assert_eq!(hint.preferred.height, 1);
    assert!(!hint.expand.height);
}

#[test]
fn flex_fit_content_uses_dropdown_variant_height() {
    let mut bordered: Flex<()> =
        Flex::column().child("dropdown", single_dropdown(), FlexItem::fit_content());
    let mut filled: Flex<()> = Flex::column().child(
        "dropdown",
        single_dropdown().variant(DropdownVariant::Filled),
        FlexItem::fit_content(),
    );
    let mut ctx = LayoutCtx::new();

    bordered.layout(Rect::new(0, 0, 24, 10), &mut ctx);
    filled.layout(Rect::new(0, 0, 24, 10), &mut ctx);

    assert_eq!(
        bordered
            .child_rect(&ChildKey::from("dropdown"))
            .unwrap()
            .height,
        3
    );
    assert_eq!(
        filled
            .child_rect(&ChildKey::from("dropdown"))
            .unwrap()
            .height,
        1
    );
}

#[test]
fn flex_horizontal_fit_content_allocates_width_based_on_text() {
    let mut flex: Flex<()> = Flex::row().child(
        "dropdown",
        single_dropdown().selected_one("Beta"),
        FlexItem::fit_content(),
    );
    let mut ctx = LayoutCtx::new();

    flex.layout(Rect::new(0, 0, 40, 3), &mut ctx);

    // "Beta" is 4 cells, plus 2 border cells, arrow spacing, and right padding = 9 width.
    assert_eq!(
        flex.child_rect(&ChildKey::from("dropdown")).unwrap().width,
        9
    );
}

#[test]
fn flex_fit_content_uses_display_width_for_dropdown_text() {
    let mut flex: Flex<()> = Flex::row().child(
        "dropdown",
        Dropdown::single(["界"], |row| *row, |row| row.to_string()).selected_one("界"),
        FlexItem::fit_content(),
    );
    let mut ctx = LayoutCtx::new();

    flex.layout(Rect::new(0, 0, 40, 3), &mut ctx);

    assert_eq!(
        flex.child_rect(&ChildKey::from("dropdown")).unwrap().width,
        7
    );
}

#[test]
fn bordered_variant_renders_trigger_with_nerd_font_chevron() {
    let dropdown = single_dropdown().selected_one("Beta");
    let mut terminal = Terminal::new(TestBackend::new(12, 3)).expect("terminal should build");

    terminal
        .draw(|frame| render_dropdown(&dropdown, frame, frame.area()))
        .expect("dropdown should render");

    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.cell((1, 1)).unwrap().symbol(), "B");
    assert_eq!(buffer.cell((9, 1)).unwrap().symbol(), "");
    assert_eq!(buffer.cell((10, 1)).unwrap().symbol(), " ");
}

#[test]
fn open_bordered_variant_renders_up_chevron() {
    let mut dropdown = single_dropdown().selected_one("Beta");
    dropdown.open();
    let mut terminal = Terminal::new(TestBackend::new(12, 3)).expect("terminal should build");

    terminal
        .draw(|frame| render_dropdown(&dropdown, frame, frame.area()))
        .expect("dropdown should render");

    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.cell((9, 1)).unwrap().symbol(), "");
}

#[test]
fn open_layout_returns_trigger_field_area_only() {
    let mut dropdown = single_dropdown();
    dropdown.open();

    let result = <Dropdown<_, _> as TuiNode<()>>::layout(
        &mut dropdown,
        Rect::new(0, 0, 24, 3),
        &mut LayoutCtx::new(),
    );

    assert_eq!(result.area, Rect::new(0, 0, 24, 3));
}

#[test]
fn filled_variant_renders_filled_trigger_with_nerd_font_chevron() {
    let dropdown = single_dropdown()
        .variant(DropdownVariant::Filled)
        .selected_one("Beta");
    let mut terminal = Terminal::new(TestBackend::new(12, 3)).expect("terminal should build");

    terminal
        .draw(|frame| render_dropdown(&dropdown, frame, frame.area()))
        .expect("dropdown should render");

    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.cell((0, 0)).unwrap().bg, theme().highlight_bg());
    assert_eq!(buffer.cell((1, 0)).unwrap().symbol(), "B");
    assert_eq!(buffer.cell((10, 0)).unwrap().symbol(), "");
}

#[test]
fn open_filled_variant_renders_up_chevron() {
    let mut dropdown = single_dropdown()
        .variant(DropdownVariant::Filled)
        .selected_one("Beta");
    dropdown.open();
    let mut terminal = Terminal::new(TestBackend::new(12, 3)).expect("terminal should build");

    terminal
        .draw(|frame| render_dropdown(&dropdown, frame, frame.area()))
        .expect("dropdown should render");

    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.cell((10, 0)).unwrap().symbol(), "");
}

#[test]
fn filled_inline_label_renders_label_value_and_hotkey_on_one_line() {
    let dropdown = single_dropdown()
        .variant(DropdownVariant::Filled)
        .label("Lane")
        .hotkey("4")
        .alt_style(true)
        .label_position(DropdownLabelPosition::Inline)
        .selected_one("Gamma");
    let mut terminal = Terminal::new(TestBackend::new(24, 1)).expect("terminal should build");

    terminal
        .draw(|frame| render_dropdown(&dropdown, frame, frame.area()))
        .expect("dropdown should render");

    let buffer = terminal.backend().buffer();
    let row = (0..24)
        .map(|x| buffer.cell((x, 0)).unwrap().symbol())
        .collect::<String>();
    assert!(row.starts_with("Lane: Gamma |4|"));
    assert!(row.contains("Lane: Gamma |4|"));
    assert!(
        buffer
            .cell((7, 0))
            .unwrap()
            .modifier
            .contains(Modifier::BOLD)
    );
}

#[test]
fn filled_alt_top_label_trigger_has_no_leading_padding() {
    let dropdown = single_dropdown()
        .variant(DropdownVariant::Filled)
        .label("Work")
        .hotkey("5")
        .alt_style(true)
        .selected_one("Gamma");
    let mut terminal = Terminal::new(TestBackend::new(24, 2)).expect("terminal should build");

    terminal
        .draw(|frame| render_dropdown(&dropdown, frame, frame.area()))
        .expect("dropdown should render");

    let buffer = terminal.backend().buffer();
    let row = (0..24)
        .map(|x| buffer.cell((x, 1)).unwrap().symbol())
        .collect::<String>();
    assert!(row.starts_with("Gamma"));
}

#[test]
fn no_selection_text_renders_empty_value_and_popup_option() {
    let mut dropdown = single_dropdown()
        .variant(DropdownVariant::Filled)
        .no_selection_text("--None--");
    dropdown.open();
    layout_dropdown(
        &mut dropdown,
        Rect::new(0, 0, 16, 1),
        Rect::new(0, 0, 16, 8),
    );
    let mut terminal = Terminal::new(TestBackend::new(16, 8)).expect("terminal should build");

    terminal
        .draw(|frame| {
            render_dropdown(&dropdown, frame, Rect::new(0, 0, 16, 1));
        })
        .expect("dropdown should render");

    let buffer = terminal.backend().buffer();
    let field = (0..16)
        .map(|x| buffer.cell((x, 0)).unwrap().symbol())
        .collect::<String>();
    let option = (0..16)
        .map(|x| buffer.cell((x, 2)).unwrap().symbol())
        .collect::<String>();
    assert!(field.contains("--None--"));
    assert!(option.contains("--None--"));
}

#[test]
fn no_selection_text_can_be_selected_to_clear_value() {
    let mut dropdown = single_dropdown()
        .variant(DropdownVariant::Filled)
        .no_selection_text("--None--")
        .selected_one("Alpha");

    dropdown.open();
    dropdown.on_key(ctrl('k'), AREA);
    dropdown.on_key(Key::Enter, AREA);

    assert_eq!(dropdown.selected_id(), None);
}

#[test]
fn immediate_no_selection_text_clears_value_when_highlighted() {
    let mut dropdown = single_dropdown()
        .variant(DropdownVariant::Filled)
        .search_mode(DropdownSearchMode::None)
        .commit_mode(DropdownCommitMode::Immediate)
        .no_selection_text("--None--")
        .selected_one("Alpha");

    dropdown.open();
    let outcome = dropdown.on_key(ctrl('k'), AREA);

    assert_eq!(dropdown.selected_id(), None);
    assert!(outcome.committed);
}

#[test]
fn no_selection_highlight_uses_same_style_as_focused_rows() {
    let mut dropdown = single_dropdown()
        .variant(DropdownVariant::Filled)
        .search_mode(DropdownSearchMode::None)
        .no_selection_text("--None--")
        .selected_one("Alpha");

    dropdown.open();
    dropdown.on_key(ctrl('k'), AREA);
    layout_dropdown(
        &mut dropdown,
        Rect::new(0, 0, 16, 1),
        Rect::new(0, 0, 16, 8),
    );
    let mut terminal = Terminal::new(TestBackend::new(16, 8)).expect("terminal should build");

    terminal
        .draw(|frame| render_dropdown(&dropdown, frame, Rect::new(0, 0, 16, 1)))
        .expect("dropdown should render");

    let cell = terminal.backend().buffer().cell((0, 1)).unwrap();
    assert_eq!(cell.fg, theme().highlight_fg());
    assert_eq!(cell.bg, theme().highlight_bg());
    assert!(cell.modifier.contains(Modifier::BOLD));

    let blank_cell = terminal.backend().buffer().cell((15, 1)).unwrap();
    assert_eq!(blank_cell.bg, theme().highlight_bg());
}

#[test]
fn focused_bordered_popup_uses_accent_border() {
    let mut dropdown = single_dropdown();
    let mut initial_layout = LayoutCtx::new();
    <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut initial_layout);
    let field = initial_layout.focus_targets()[0].clone();
    let mut focus = FocusCtx::<()>::new(AnimationSettings::default());
    dropdown.dispatch_focus(&field, true, &mut focus);
    dropdown.open();
    layout_dropdown(
        &mut dropdown,
        Rect::new(0, 0, 12, 3),
        Rect::new(0, 0, 12, 8),
    );
    let mut terminal = Terminal::new(TestBackend::new(12, 8)).expect("terminal should build");

    terminal
        .draw(|frame| render_dropdown(&dropdown, frame, Rect::new(0, 0, 12, 3)))
        .expect("dropdown should render");

    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.cell((0, 2)).unwrap().fg, theme().accent_fg());
}

#[test]
fn open_dropdown_keeps_trigger_chrome_accented_under_popup() {
    let mut dropdown = single_dropdown();
    dropdown.open();
    let layout = layout_dropdown(
        &mut dropdown,
        Rect::new(0, 0, 12, 3),
        Rect::new(0, 0, 12, 8),
    );
    let search = layout
        .focus_targets()
        .iter()
        .find(|target| target.id.as_str() == SEARCH_FOCUS)
        .expect("search focus target should exist")
        .clone();
    let mut focus = FocusCtx::<()>::new(AnimationSettings::default());
    dropdown.dispatch_focus(&search, true, &mut focus);
    let mut terminal = Terminal::new(TestBackend::new(12, 8)).expect("terminal should build");

    terminal
        .draw(|frame| render_dropdown(&dropdown, frame, Rect::new(0, 0, 12, 3)))
        .expect("dropdown should render");

    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.cell((0, 0)).unwrap().fg, theme().accent_fg());
    assert_eq!(buffer.cell((0, 2)).unwrap().fg, theme().accent_fg());
}

#[test]
fn open_render_draws_trigger_without_inline_popup() {
    let mut dropdown = single_dropdown();
    dropdown.open();
    let mut terminal = Terminal::new(TestBackend::new(12, 8)).expect("terminal should build");

    terminal
        .draw(|frame| render_dropdown(&dropdown, frame, frame.area()))
        .expect("dropdown should render");

    let buffer = terminal.backend().buffer();
    assert_ne!(buffer.cell((0, 2)).unwrap().symbol(), " ");
    assert_eq!(buffer.cell((0, 3)).unwrap().symbol(), " ");
}

#[test]
fn open_node_render_flushes_popup_portal() {
    let mut dropdown = single_dropdown();
    dropdown.open();
    let mut layout = LayoutCtx::new();
    layout.with_overlay_bounds(Rect::new(0, 0, 12, 8), |ctx| {
        <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, Rect::new(0, 0, 12, 1), ctx);
    });
    let mut terminal = Terminal::new(TestBackend::new(12, 8)).expect("terminal should build");

    terminal
        .draw(|frame| {
            let mut render = RenderCtx::new();
            <Dropdown<_, _> as TuiNode<()>>::render(
                &dropdown,
                frame,
                Rect::new(0, 0, 12, 1),
                &mut render,
            );
            render.flush(frame);
        })
        .expect("dropdown should render");

    let row = (0..12)
        .map(|x| terminal.backend().buffer().cell((x, 2)).unwrap().symbol())
        .collect::<String>();
    assert!(row.contains("Alpha"), "{row}");
}

#[test]
fn inherent_render_flushes_popup_portal() {
    let mut dropdown = single_dropdown();
    dropdown.open();
    let mut layout = LayoutCtx::new();
    layout.with_overlay_bounds(Rect::new(0, 0, 12, 8), |ctx| {
        <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, Rect::new(0, 0, 12, 1), ctx);
    });
    let mut terminal = Terminal::new(TestBackend::new(12, 8)).expect("terminal should build");

    terminal
        .draw(|frame| {
            let mut render = RenderCtx::new();
            dropdown.render(frame, Rect::new(0, 0, 12, 1), &mut render);
            assert!(!render.is_empty());
            render.flush(frame);
        })
        .expect("dropdown should render");

    let row = (0..12)
        .map(|x| terminal.backend().buffer().cell((x, 2)).unwrap().symbol())
        .collect::<String>();
    assert!(row.contains("Alpha"), "{row}");
}

#[test]
fn dropdown_inside_tabs_dialog_flushes_popup_from_normal_render() {
    let tabs = Tabs::new(vec![Tab::new("Controls", DialogControlsTabBody::open())]);
    let mut dialog = Dialog::new().host(tabs);
    let area = Rect::new(0, 0, 30, 12);
    let mut layout = LayoutCtx::new();
    layout.with_overlay_bounds(area, |ctx| {
        <_ as TuiNode<()>>::layout(&mut dialog, area, ctx);
    });
    let mut terminal = Terminal::new(TestBackend::new(30, 12)).expect("terminal should build");

    terminal
        .draw(|frame| {
            let mut render = RenderCtx::new();
            <_ as TuiNode<()>>::render(&dialog, frame, area, &mut render);
            render.flush(frame);
        })
        .expect("dialog controls should render");

    let buffer = terminal.backend().buffer();
    let rendered = (0..12)
        .flat_map(|y| (0..30).map(move |x| buffer.cell((x, y)).unwrap().symbol()))
        .collect::<String>();
    assert!(rendered.contains("Alpha"), "{rendered}");
}

#[test]
fn dropdown_inside_dialog_layer_dialog_tabs_flushes_popup_from_normal_render() {
    let tabs = Tabs::new(vec![Tab::new("Controls", DialogControlsTabBody::open())]);
    let host = Dialog::new().host(tabs);
    let mut layer = DialogLayer::new(EmptyNode, host).active(true);
    let area = Rect::new(0, 0, 30, 12);
    let mut layout = LayoutCtx::new();
    layout.with_overlay_bounds(area, |ctx| {
        <_ as TuiNode<()>>::layout(&mut layer, area, ctx);
    });
    let mut terminal = Terminal::new(TestBackend::new(30, 12)).expect("terminal should build");

    terminal
        .draw(|frame| {
            let mut render = RenderCtx::new();
            <_ as TuiNode<()>>::render(&layer, frame, area, &mut render);
            render.flush(frame);
        })
        .expect("dialog layer controls should render");

    let buffer = terminal.backend().buffer();
    let rendered = (0..12)
        .flat_map(|y| (0..30).map(move |x| buffer.cell((x, y)).unwrap().symbol()))
        .collect::<String>();
    assert!(rendered.contains("Alpha"), "{rendered}");
}

#[test]
fn open_node_layout_uses_inherited_overlay_bounds() {
    let mut dropdown = single_dropdown();
    dropdown.open();
    let mut layout = LayoutCtx::new();
    let bounds = Rect::new(0, 0, 24, 20);

    layout.with_overlay_bounds(bounds, |ctx| {
        <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, Rect::new(0, 0, 24, 3), ctx);
    });

    assert_eq!(layout.overlays().len(), 1);
    assert_eq!(layout.overlays()[0].bounds, bounds);
    assert_eq!(layout.overlays()[0].area, Rect::new(0, 2, 24, 6));
}

#[test]
fn dropdown_inside_non_forwarding_composite_flushes_popup_portal() {
    let mut composite = NonForwardingComposite {
        dropdown: single_dropdown(),
    };
    composite.dropdown.open();
    let mut layout = LayoutCtx::new();
    layout.with_overlay_bounds(Rect::new(0, 0, 12, 8), |ctx| {
        composite.layout(Rect::new(0, 0, 12, 1), ctx);
    });
    let mut terminal = Terminal::new(TestBackend::new(12, 8)).expect("terminal should build");

    terminal
        .draw(|frame| {
            let mut render = RenderCtx::new();
            composite.render(frame, Rect::new(0, 0, 12, 1), &mut render);
            render.flush(frame);
        })
        .expect("dropdown should render");

    let row = (0..12)
        .map(|x| terminal.backend().buffer().cell((x, 2)).unwrap().symbol())
        .collect::<String>();
    assert!(row.contains("Alpha"), "{row}");
}

#[test]
fn non_focusable_dropdown_flushes_popup_portal() {
    let mut inner = single_dropdown();
    inner.open();
    let mut dropdown = NonFocusable::new(inner);
    let mut layout = LayoutCtx::new();
    layout.with_overlay_bounds(Rect::new(0, 0, 12, 8), |ctx| {
        <NonFocusable<Dropdown<_, _>> as TuiNode<()>>::layout(
            &mut dropdown,
            Rect::new(0, 0, 12, 1),
            ctx,
        );
    });
    let mut terminal = Terminal::new(TestBackend::new(12, 8)).expect("terminal should build");

    terminal
        .draw(|frame| {
            let mut render = RenderCtx::new();
            <NonFocusable<Dropdown<_, _>> as TuiNode<()>>::render(
                &dropdown,
                frame,
                Rect::new(0, 0, 12, 1),
                &mut render,
            );
            render.flush(frame);
        })
        .expect("dropdown should render");

    let row = (0..12)
        .map(|x| terminal.backend().buffer().cell((x, 2)).unwrap().symbol())
        .collect::<String>();
    assert!(row.contains("Alpha"), "{row}");
}

#[test]
fn open_layout_registers_single_external_search_focus() {
    let mut dropdown = single_dropdown();
    dropdown.open();
    let mut ctx = LayoutCtx::new();

    <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut ctx);

    let targets = ctx.focus_targets();
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].id.as_str(), "input");
    assert!(targets[0].path.is_empty());
}

#[test]
fn open_search_dropdown_suppresses_global_hotkeys_on_field_focus() {
    let mut dropdown = single_dropdown().auto_focus_search(false);
    dropdown.open();
    let mut ctx = LayoutCtx::new();

    <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut ctx);

    let target = ctx
        .focus_targets()
        .iter()
        .find(|target| target.id.as_str() == FIELD_FOCUS)
        .expect("field focus target");
    assert!(target.suppress_global_hotkeys);
}

#[test]
fn open_layout_focus_targets_use_overlay_popup_areas() {
    let mut dropdown = single_dropdown();
    dropdown.open();

    let ctx = layout_dropdown(
        &mut dropdown,
        Rect::new(0, 0, 24, 3),
        Rect::new(0, 0, 24, 20),
    );

    let targets = ctx.focus_targets();
    assert_eq!(targets[0].area, Rect::new(1, 3, 22, 1));
}

#[test]
fn tab_from_open_dropdown_cancels_and_requests_next_focus() {
    let mut dropdown = single_dropdown();
    dropdown.open();
    let mut ctx = EventCtx::<()>::default();

    let outcome = dropdown.event(&TuiEvent::Key(KeyEvent::from(Key::Tab)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(!dropdown.is_open());
    assert_eq!(ctx.focus_request(), Some(&crate::FocusRequest::Next));
    assert!(ctx.layout_requested());
    assert!(ctx.redraw_requested());
    assert_eq!(ctx.propagation(), Propagation::Stopped);
}

#[test]
fn backtab_from_open_dropdown_cancels_and_requests_previous_focus() {
    let mut dropdown = single_dropdown();
    dropdown.open();
    let mut ctx = EventCtx::<()>::default();

    let outcome = dropdown.event(&TuiEvent::Key(KeyEvent::from(Key::BackTab)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(!dropdown.is_open());
    assert_eq!(ctx.focus_request(), Some(&crate::FocusRequest::Previous));
    assert!(ctx.layout_requested());
    assert!(ctx.redraw_requested());
    assert_eq!(ctx.propagation(), Propagation::Stopped);
}

#[test]
fn open_dropdown_closes_when_focused_target_blurs() {
    let mut dropdown = single_dropdown();
    dropdown.open();
    let mut layout = LayoutCtx::new();
    <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut layout);
    let target = layout.focus_targets()[0].clone();
    let mut focus = FocusCtx::<()>::new(AnimationSettings::default());

    dropdown.dispatch_focus(&target, true, &mut focus);
    dropdown.dispatch_focus(&target, false, &mut focus);

    assert!(!dropdown.is_open());
    assert!(!dropdown.is_focused());
    assert!(focus.redraw_requested());
}

#[test]
fn opening_search_dropdown_does_not_close_during_runtime_field_to_search_transition() {
    let mut dropdown = single_dropdown();
    let mut layout = LayoutCtx::new();
    <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut layout);
    let field = layout.focus_targets()[0].clone();
    let mut focus = FocusCtx::<()>::new(AnimationSettings::default());

    dropdown.dispatch_focus(&field, true, &mut focus);
    dropdown.open();
    let mut open_layout = LayoutCtx::new();
    <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut open_layout);
    let search = open_layout.focus_targets()[0].clone();
    dropdown.dispatch_focus(&field, false, &mut focus);
    dropdown.dispatch_focus(&search, true, &mut focus);

    assert!(dropdown.is_open());
    assert_eq!(dropdown.focus_region, Some(DropdownFocusRegion::Search));
}

#[test]
fn open_search_dropdown_can_keep_focus_on_field_when_auto_focus_disabled() {
    let mut dropdown = single_dropdown().auto_focus_search(false);
    dropdown.open();
    let mut layout = LayoutCtx::new();

    <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut layout);

    assert_eq!(layout.focus_targets()[0].id.as_str(), "field");
}

#[test]
fn open_layout_sizes_popup_to_visible_items() {
    let mut dropdown = single_dropdown();
    dropdown.open();

    let area = open_list_area(&mut dropdown, Rect::new(0, 0, 24, 20));

    assert_eq!(area.height, 3);
}

#[test]
fn open_layout_centers_selected_row_in_popup_view_when_possible() {
    let mut dropdown = numeric_dropdown(30)
        .selected_one(20)
        .search_mode(DropdownSearchMode::None)
        .max_popup_height(5);
    dropdown.open();
    layout_dropdown(
        &mut dropdown,
        Rect::new(0, 0, 12, 1),
        Rect::new(0, 0, 12, 8),
    );
    let mut terminal = Terminal::new(TestBackend::new(12, 8)).expect("terminal should build");

    terminal
        .draw(|frame| {
            render_dropdown(&dropdown, frame, Rect::new(0, 0, 12, 1));
        })
        .expect("dropdown should render");

    let buffer = terminal.backend().buffer();
    let rendered = (0..8)
        .flat_map(|y| (0..12).map(move |x| buffer.cell((x, y)).unwrap().symbol()))
        .collect::<String>();
    assert!(rendered.contains("19"));
    assert!(rendered.contains("20"));
    assert!(rendered.contains("21"));
}

#[test]
fn bordered_and_filled_popups_size_to_same_content_with_variant_chrome() {
    let mut bordered = single_dropdown();
    bordered.open();
    let mut filled = single_dropdown().variant(DropdownVariant::Filled);
    filled.open();

    let [_, bordered_popup] = bordered.areas(Rect::new(0, 0, 24, 20));
    let [_, filled_popup] = filled.areas(Rect::new(0, 0, 24, 20));
    let [_, bordered_list] = bordered.popup_inner_areas(bordered_popup);
    let [_, filled_list] = filled.popup_inner_areas(filled_popup);

    assert_eq!(bordered_list.height, 3);
    assert_eq!(filled_list.height, 3);
    assert_eq!(bordered_popup.height, 6);
    assert_eq!(filled_popup.height, 4);
}

#[test]
fn bordered_popup_area_overlaps_field_bottom_row() {
    let mut dropdown = single_dropdown();
    dropdown.open();

    let [field_area, popup_area] = dropdown.areas(AREA);

    assert_eq!(popup_area.y, field_area.y + field_area.height - 1);
}

#[test]
fn overlay_popup_extends_beyond_trigger_field_when_bounds_allow() {
    let mut dropdown = single_dropdown();
    dropdown.open();
    layout_dropdown(
        &mut dropdown,
        Rect::new(0, 0, 24, 3),
        Rect::new(0, 0, 24, 20),
    );

    let popup_area = dropdown.popup_overlay_area(Rect::new(0, 0, 24, 20));

    assert_eq!(popup_area, Rect::new(0, 2, 24, 6));
    assert!(popup_area.y + popup_area.height > 3);
}

#[test]
fn centered_popup_overlay_centers_popup_within_bounds() {
    let mut dropdown = single_dropdown().centered(true);
    dropdown.open();
    layout_dropdown(
        &mut dropdown,
        Rect::new(2, 2, 24, 3),
        Rect::new(0, 0, 100, 40),
    );

    let popup_area = dropdown.popup_overlay_area(Rect::new(0, 0, 100, 40));

    assert_eq!(popup_area, Rect::new(30, 17, 40, 6));
}

#[test]
fn upward_popup_opens_above_trigger() {
    let mut dropdown = single_dropdown().popup_direction(DropdownPopupDirection::Up);
    dropdown.open();
    layout_dropdown(
        &mut dropdown,
        Rect::new(0, 10, 24, 1),
        Rect::new(0, 0, 24, 12),
    );

    let popup_area = dropdown.popup_overlay_area(Rect::new(0, 0, 24, 12));

    assert_eq!(popup_area.y + popup_area.height, 10);
}

#[test]
fn filled_popup_layout_has_no_border_offset() {
    let mut dropdown = single_dropdown().variant(DropdownVariant::Filled);
    dropdown.open();

    let [_, popup_area] = dropdown.areas(AREA);
    let [search_area, list_area] = dropdown.popup_inner_areas(popup_area);

    assert_eq!(search_area.y, popup_area.y);
    assert_eq!(search_area.x, popup_area.x);
    assert_eq!(list_area.y, popup_area.y + 1);
    assert_eq!(list_area.x, popup_area.x);
}

#[test]
fn open_layout_sizes_popup_to_no_results_row() {
    let mut dropdown = single_dropdown();
    dropdown.open();
    dropdown.on_key(char_key('z'), Rect::new(0, 0, 24, 20));

    let area = open_list_area(&mut dropdown, Rect::new(0, 0, 24, 20));

    assert_eq!(area.height, 1);
}

#[test]
fn open_layout_caps_popup_at_default_max() {
    let mut dropdown = numeric_dropdown(40);
    dropdown.open();

    let area = open_list_area(&mut dropdown, Rect::new(0, 0, 24, 60));

    assert_eq!(area.height, 27);
}

#[test]
fn max_popup_height_overrides_preset_max() {
    let mut dropdown = numeric_dropdown(40).max_popup_height(5);
    dropdown.open();

    let area = open_list_area(&mut dropdown, Rect::new(0, 0, 24, 60));

    assert_eq!(area.height, 2);
}

#[test]
fn filled_popup_caps_height_without_border_chrome() {
    let mut dropdown = numeric_dropdown(40).variant(DropdownVariant::Filled);
    dropdown.open();

    let [_, popup_area] = dropdown.areas(Rect::new(0, 0, 24, 60));
    let [_, list_area] = dropdown.popup_inner_areas(popup_area);

    assert_eq!(popup_area.height, 30);
    assert_eq!(list_area.height, 29);
}

#[test]
fn filled_popup_applies_background_to_content_rows() {
    let mut dropdown = single_dropdown().variant(DropdownVariant::Filled);
    dropdown.open();
    layout_dropdown(
        &mut dropdown,
        Rect::new(0, 0, 12, 1),
        Rect::new(0, 0, 12, 6),
    );
    let mut terminal = Terminal::new(TestBackend::new(12, 6)).expect("terminal should build");

    terminal
        .draw(|frame| {
            render_dropdown(&dropdown, frame, Rect::new(0, 0, 12, 1));
        })
        .expect("dropdown should render");

    let buffer = terminal.backend().buffer();
    assert_eq!(
        dropdown.popup_content_style().unwrap().bg,
        Some(theme().border_fg())
    );
    assert_eq!(buffer.cell((0, 3)).unwrap().symbol(), "B");
    assert_eq!(buffer.cell((0, 3)).unwrap().bg, theme().border_fg());
}

#[test]
fn node_event_opens_and_requests_layout() {
    let mut dropdown = single_dropdown();
    let mut layout = LayoutCtx::new();
    <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut layout);
    let target = layout.focus_targets()[0].clone();
    let mut focus = FocusCtx::<()>::default();
    dropdown.dispatch_focus(&target, true, &mut focus);
    let mut event = EventCtx::<()>::default();

    let outcome = dropdown.event(&TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut event);

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(dropdown.is_open());
    assert!(event.layout_requested());
    assert_eq!(event.propagation(), Propagation::Stopped);
}

#[test]
fn hotkey_opens_dropdown() {
    let mut dropdown = single_dropdown().hotkey("d");
    let mut layout = LayoutCtx::new();
    <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut layout);
    let target = layout.focus_targets()[0].clone();
    let mut focus = FocusCtx::<()>::default();
    dropdown.dispatch_focus(&target, true, &mut focus);
    let mut event = EventCtx::<()>::default();

    let outcome = dropdown.event(&TuiEvent::Key(KeyEvent::from(Key::Char('d'))), &mut event);

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(dropdown.is_open());
}

#[test]
fn uppercase_hotkey_commit_opens_dropdown() {
    let mut dropdown = single_dropdown().hotkey("D");
    let mut event = EventCtx::<()>::default();

    let outcome = dropdown.event(
        &TuiEvent::Hotkey(HotkeyEvent::Commit("d".to_string())),
        &mut event,
    );

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(dropdown.is_open());
    assert!(event.layout_requested());
}

#[test]
fn hotkey_commit_focuses_search_when_auto_focus_is_enabled() {
    let mut dropdown = single_dropdown().hotkey("db");
    let mut event = EventCtx::<()>::default();

    let outcome = dropdown.event(
        &TuiEvent::Hotkey(HotkeyEvent::Commit("db".to_string())),
        &mut event,
    );

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(dropdown.is_open());
    assert_eq!(
        event.focus_request(),
        Some(&FocusRequest::TargetAt {
            path: TreePath::default(),
            id: FocusId::new(SEARCH_FOCUS),
        })
    );
}

#[test]
fn multiletter_hotkey_opens_after_direct_sequence() {
    let mut dropdown = single_dropdown().hotkey("db");

    let pending = dropdown.on_key(KeyEvent::from(Key::Char('d')), AREA);
    let matched = dropdown.on_key(KeyEvent::from(Key::Char('b')), AREA);

    assert!(pending.handled);
    assert!(!pending.opened);
    assert!(matched.handled);
    assert!(matched.opened);
    assert!(dropdown.is_open());
}

#[test]
fn focused_multiletter_hotkey_opens_from_key_events() {
    let mut dropdown = single_dropdown().hotkey("db");
    let mut layout = LayoutCtx::new();
    <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut layout);
    let target = layout.focus_targets()[0].clone();
    let mut focus = FocusCtx::<()>::default();
    dropdown.dispatch_focus(&target, true, &mut focus);
    let mut event = EventCtx::<()>::default();

    let pending = dropdown.event(&TuiEvent::Key(KeyEvent::from(Key::Char('d'))), &mut event);
    let matched = dropdown.event(&TuiEvent::Key(KeyEvent::from(Key::Char('b'))), &mut event);

    assert_eq!(pending, EventOutcome::Handled);
    assert_eq!(matched, EventOutcome::Handled);
    assert!(dropdown.is_open());
}

#[test]
fn dropdown_with_label_and_hotkey_renders_in_borders() {
    let dropdown = single_dropdown().label("Database").hotkey("d");
    let mut terminal = Terminal::new(TestBackend::new(24, 3)).expect("terminal should build");

    terminal
        .draw(|frame| render_dropdown(&dropdown, frame, frame.area()))
        .expect("dropdown should render");

    let buffer = terminal.backend().buffer();
    let top = (0..24)
        .map(|x| buffer.cell((x, 0)).unwrap().symbol())
        .collect::<String>();
    assert!(top.contains("Database"));

    let bottom = (0..24)
        .map(|x| buffer.cell((x, 2)).unwrap().symbol())
        .collect::<String>();
    assert!(bottom.contains("┤d│"));
}

#[test]
fn dropdown_with_alternative_style_layout_and_render() {
    let mut dropdown = single_dropdown()
        .label("Search")
        .hotkey("s")
        .alt_style(true);

    let area = Rect::new(0, 0, 24, 4);
    let mut ctx = LayoutCtx::new();
    <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, area, &mut ctx);

    let hint = <Dropdown<_, _> as TuiNode<()>>::measure(&dropdown, LayoutProposal::unbounded());
    assert_eq!(hint.preferred.height, 4);

    let mut terminal = Terminal::new(TestBackend::new(24, 4)).expect("terminal should build");
    terminal
        .draw(|frame| render_dropdown(&dropdown, frame, frame.area()))
        .expect("dropdown should render");

    let buffer = terminal.backend().buffer();
    let row0 = (0..24)
        .map(|x| buffer.cell((x, 0)).unwrap().symbol())
        .collect::<String>();
    assert!(row0.contains("Search |s|"));

    let row1 = (0..24)
        .map(|x| buffer.cell((x, 1)).unwrap().symbol())
        .collect::<String>();
    assert!(row1.contains("╭"));
    assert!(row1.contains("╮"));
}

fn char_key(value: char) -> KeyEvent {
    KeyEvent {
        code: Key::Char(value),
        modifiers: KeyModifiers::NONE,
    }
}

fn ctrl(value: char) -> KeyEvent {
    KeyEvent {
        code: Key::Char(value),
        modifiers: KeyModifiers::CONTROL,
    }
}

fn open_list_area<T, Id>(dropdown: &mut Dropdown<T, Id>, area: Rect) -> Rect
where
    T: 'static,
    Id: Clone + Eq + Hash + 'static,
{
    let popup_area = dropdown.popup_overlay_area(area);
    dropdown.popup_inner_areas(popup_area)[1]
}
