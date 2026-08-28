use std::cell::RefCell;
use std::hash::Hash;
use std::rc::Rc;

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::{Frame, Terminal};

use super::*;
use crate::event::KeyModifiers;
use crate::{
    ChildKey, Dialog, DialogLayer, EventCtx, EventRoute, ExternalEditorResponse, Flex, FlexItem,
    FocusCtx, FocusId, FocusRequest, KeyBindings, KeySpec, LayoutCtx, LayoutProposal, MouseButton,
    MouseEvent, MouseEventKind, Propagation, RenderCtx, Tab, Tabs, TuiEvent, TuiNode, border_chars,
    preset,
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

struct DialogControlsTabBody {
    dropdown: Dropdown<&'static str, &'static str>,
    dropdown_area: Rect,
}

struct EmptyNode;

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
fn error_tone_overrides_focus_for_own_border_and_title() {
    let mut dropdown = single_dropdown().label("Environment").error(true);
    dropdown.focus_region = Some(DropdownFocusRegion::Field);
    let mut terminal = Terminal::new(TestBackend::new(24, 3)).expect("terminal should build");

    terminal
        .draw(|frame| render_dropdown(&dropdown, frame, frame.area()))
        .expect("dropdown should render");

    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.cell((0, 0)).unwrap().fg, theme().error_fg());
    assert_eq!(buffer.cell((3, 0)).unwrap().symbol(), "E");
    assert_eq!(buffer.cell((3, 0)).unwrap().fg, theme().error_fg());
}

#[test]
fn focused_disabled_dropdown_keeps_muted_dashed_chrome_with_accent_focus_cue() {
    let mut dropdown = single_dropdown()
        .label("Environment")
        .error(true)
        .selected_one("Beta")
        .disabled(true);
    dropdown.focus_region = Some(DropdownFocusRegion::Field);
    let mut terminal = Terminal::new(TestBackend::new(24, 3)).expect("terminal should build");

    terminal
        .draw(|frame| render_dropdown(&dropdown, frame, frame.area()))
        .expect("dropdown should render");

    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), "╭");
    assert_eq!(buffer.cell((23, 0)).unwrap().symbol(), "╮");
    assert_eq!(buffer.cell((0, 2)).unwrap().symbol(), "╰");
    assert_eq!(buffer.cell((23, 2)).unwrap().symbol(), "╯");
    assert_eq!(buffer.cell((1, 0)).unwrap().symbol(), "-");
    assert_eq!(buffer.cell((0, 1)).unwrap().symbol(), "╎");
    assert_eq!(buffer.cell((1, 0)).unwrap().fg, theme().accent_fg());
    assert!(
        buffer
            .cell((1, 0))
            .unwrap()
            .modifier
            .contains(Modifier::BOLD)
    );
    assert_eq!(buffer.cell((1, 1)).unwrap().fg, theme().muted_fg());
}

#[test]
fn unfocused_disabled_dropdown_renders_hotkey_with_border_color() {
    let mut dropdown = single_dropdown().hotkey("it").disabled(true);
    dropdown.pending_hotkey_prefix = Some("i".into());
    let mut terminal = Terminal::new(TestBackend::new(24, 3)).expect("terminal should build");

    terminal
        .draw(|frame| render_dropdown(&dropdown, frame, frame.area()))
        .expect("dropdown should render");

    let buffer = terminal.backend().buffer();
    for (position, symbol) in [(20, "┤"), (21, "i"), (22, "t"), (23, "╎")] {
        let cell = buffer.cell((position, 2)).unwrap();
        assert_eq!(cell.symbol(), symbol);
        assert_eq!(cell.fg, theme().border_fg());
    }
}

#[test]
fn open_disabled_dropdown_uses_accent_hotkey_and_dashed_popup_border() {
    let mut dropdown = single_dropdown()
        .label("Priority")
        .hotkey("it")
        .disabled(true);
    dropdown.open();
    let mut terminal = Terminal::new(TestBackend::new(24, 8)).expect("terminal should build");

    terminal
        .draw(|frame| {
            dropdown.render_field(frame, Rect::new(0, 0, 24, 3));
            dropdown.render_popup(frame, Rect::new(0, 3, 24, 5), DropdownPopupDirection::Down);
        })
        .expect("dropdown should render");

    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.cell((0, 0)).unwrap().fg, theme().accent_fg());
    assert_eq!(buffer.cell((1, 0)).unwrap().fg, theme().accent_fg());
    assert_eq!(buffer.cell((3, 0)).unwrap().fg, theme().accent_fg());
    assert_eq!(buffer.cell((23, 0)).unwrap().fg, theme().accent_fg());
    assert_eq!(buffer.cell((21, 2)).unwrap().fg, theme().accent_fg());
    assert_eq!(buffer.cell((1, 3)).unwrap().symbol(), "-");
    assert_eq!(buffer.cell((0, 4)).unwrap().symbol(), "╎");
    assert_eq!(buffer.cell((1, 3)).unwrap().fg, theme().accent_fg());
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
fn disabled_backdrop_leaves_host_undimmed_and_renders_popup() {
    let mut dropdown = single_dropdown()
        .variant(DropdownVariant::Filled)
        .backdrop_amount(0.0)
        .disabled(true);
    dropdown.open();
    layout_dropdown(&mut dropdown, Rect::new(0, 0, 12, 1), AREA);
    let mut terminal = Terminal::new(TestBackend::new(24, 10)).expect("terminal should build");
    let host_style = Style::default()
        .fg(Color::Rgb(200, 200, 200))
        .bg(Color::Rgb(10, 20, 30));

    terminal
        .draw(|frame| {
            frame.buffer_mut().set_string(0, 9, "X", host_style);
            render_dropdown(&dropdown, frame, Rect::new(0, 0, 12, 1));
        })
        .expect("dropdown should render");

    let buffer = terminal.backend().buffer();
    let host_cell = buffer.cell((0, 9)).unwrap();
    assert_eq!(host_cell.fg, Color::Rgb(200, 200, 200));
    assert_eq!(host_cell.bg, Color::Rgb(10, 20, 30));
    assert!(!host_cell.modifier.contains(Modifier::DIM));

    let popup_row = (0..12)
        .map(|x| buffer.cell((x, 2)).unwrap().symbol())
        .collect::<String>();
    assert!(popup_row.contains("Alpha"), "{popup_row}");
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
fn immediate_open_keeps_full_backdrop_and_requests_focus() {
    let mut dropdown = single_dropdown();
    layout_dropdown(&mut dropdown, Rect::new(0, 0, 12, 1), AREA);
    let mut ctx = EventCtx::<()>::new(AnimationSettings::default());

    let outcome = dropdown.open_immediate_with_context(&mut ctx);

    assert!(outcome.opened);
    assert_eq!(dropdown.backdrop_tween.value(), DROPDOWN_BACKDROP_AMOUNT);
    assert!(!dropdown.backdrop_tween.is_active());
    assert!(ctx.layout_requested());
    assert!(ctx.redraw_requested());
    assert!(ctx.focus_request().is_some());
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
fn enter_with_or_without_control_commits_single_draft() {
    for key in [
        KeyEvent::from(Key::Enter),
        KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::CONTROL,
        },
    ] {
        let mut dropdown = single_dropdown();
        dropdown.open();
        dropdown.on_key(ctrl('j'), AREA);
        dropdown.on_key(key, AREA);

        assert_eq!(dropdown.selected_id(), Some("Beta"));
        assert!(!dropdown.is_open());
    }
}

#[test]
fn focused_empty_dropdown_does_not_render_field_cursor() {
    let mut dropdown = single_dropdown().placeholder("Pick...");
    dropdown.focus_region = Some(DropdownFocusRegion::Field);
    let mut terminal = Terminal::new(TestBackend::new(24, 3)).expect("terminal should build");

    terminal
        .draw(|frame| render_dropdown(&dropdown, frame, frame.area()))
        .expect("dropdown should render");

    let buffer = terminal.backend().buffer();
    let first = buffer.cell((1, 1)).unwrap();
    let second = buffer.cell((2, 1)).unwrap();
    assert_eq!(first.fg, second.fg);
    assert_eq!(first.bg, second.bg);
    assert_eq!(first.modifier, second.modifier);
}

#[test]
fn closed_ctrl_j_and_ctrl_k_do_not_open_or_navigate() {
    for key in [ctrl('j'), ctrl('k')] {
        let mut dropdown = single_dropdown();

        let outcome = dropdown.on_key(key, AREA);

        assert_eq!(outcome, DropdownOutcome::IDLE);
        assert!(!dropdown.is_open());
        assert_eq!(dropdown.data_view.highlighted_id(), Some("Alpha"));
    }
}

#[test]
fn disabled_dropdown_registers_focus_and_hotkey_then_navigates_read_only() {
    let mut dropdown = single_dropdown()
        .selected_one("Alpha")
        .hotkey("d")
        .disabled(true);
    let layout = layout_dropdown(&mut dropdown, AREA, AREA);
    let target = layout.focus_targets()[0].clone();
    let mut focus = FocusCtx::<()>::default();

    assert!(dropdown.is_disabled());
    assert_eq!(target.area, Rect::new(0, 0, 24, 3));
    assert!(target.enabled);
    assert!(target.tab_stop);
    assert_eq!(target.hotkey_sequences, ["d"]);
    dropdown.dispatch_focus(&target, true, &mut focus);
    assert!(dropdown.is_focused());

    let mut open_event = EventCtx::<()>::default();
    let opened = dropdown.dispatch_event(
        &EventRoute::new(target.path.clone()),
        &TuiEvent::Hotkey(HotkeyEvent::Commit("d".into())),
        &mut open_event,
    );

    assert_eq!(opened, EventOutcome::Handled);
    assert!(dropdown.is_open());
    assert_eq!(
        open_event.focus_request(),
        Some(&FocusRequest::TargetAt {
            path: target.path.clone(),
            id: FocusId::new(SEARCH_FOCUS),
        })
    );

    let mut event = EventCtx::<()>::default();
    let outcome = dropdown.dispatch_event(
        &EventRoute::new(target.path.clone()),
        &TuiEvent::Key(ctrl('j')),
        &mut event,
    );

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(event.propagation(), Propagation::Stopped);
    assert_eq!(dropdown.data_view.highlighted_id(), Some("Beta"));
    assert_eq!(dropdown.selected_id(), Some("Alpha"));
    assert_eq!(dropdown.draft, vec!["Alpha"]);
}

#[test]
fn enabled_dropdown_routes_ctrl_j_and_ctrl_k() {
    let mut dropdown = single_dropdown();
    dropdown.open();

    for (key, expected) in [(ctrl('j'), "Beta"), (ctrl('k'), "Alpha")] {
        let mut event = EventCtx::<()>::default();
        let outcome = dropdown.dispatch_event(
            &EventRoute::new(TreePath::default()),
            &TuiEvent::Key(key),
            &mut event,
        );

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(event.propagation(), Propagation::Stopped);
        assert_eq!(dropdown.data_view.highlighted_id(), Some(expected));
    }
}

#[test]
fn idempotent_disabled_setter_preserves_enabled_hotkey_sequence() {
    let mut dropdown = single_dropdown().hotkey("db");

    assert!(dropdown.on_key(char_key('d'), AREA).handled);
    dropdown.set_disabled(false);

    assert!(dropdown.on_key(char_key('b'), AREA).opened);
}

#[test]
fn disabling_open_dropdown_preserves_view_state_and_rebases_draft() {
    let mut dropdown = single_dropdown().selected_one("Alpha");
    dropdown.open();
    dropdown.on_key(ctrl('j'), AREA);
    dropdown.set_search_query("b");
    dropdown.focus_region = Some(DropdownFocusRegion::Search);
    dropdown.sync_child_focus();

    dropdown.set_disabled(true);

    assert!(dropdown.is_open());
    assert_eq!(dropdown.search_query(), "b");
    assert_eq!(dropdown.data_view.highlighted_id(), Some("Beta"));
    assert_eq!(dropdown.focus_region, Some(DropdownFocusRegion::Search));
    assert_eq!(dropdown.draft, vec!["Alpha"]);
    assert_eq!(dropdown.opened_committed, vec!["Alpha"]);
    assert_eq!(dropdown.data_view.selected_ids(), vec!["Alpha"]);
}

#[test]
fn disabled_dropdown_keeps_explicit_and_immediate_search_navigation_read_only() {
    for mode in [DropdownCommitMode::Explicit, DropdownCommitMode::Immediate] {
        let callbacks = Rc::new(RefCell::new(Vec::new()));
        let captured = Rc::clone(&callbacks);
        let mut dropdown = single_dropdown()
            .commit_mode(mode)
            .selected_one("Alpha")
            .on_select(move |ids| captured.borrow_mut().push(ids));
        dropdown.open();
        dropdown.set_disabled(true);

        let search = dropdown.on_key(char_key('g'), AREA);
        let navigation = dropdown.on_key(ctrl('j'), AREA);
        assert_eq!(dropdown.search_query(), "g");
        let commit = dropdown.on_key(Key::Enter, AREA);

        assert!(!search.committed);
        assert!(!navigation.committed);
        assert!(commit.closed);
        assert!(!commit.committed);
        assert_eq!(dropdown.selected_id(), Some("Alpha"));
        assert_eq!(dropdown.draft, vec!["Alpha"]);
        assert!(callbacks.borrow().is_empty());
    }
}

#[test]
fn disabled_multi_toggle_and_commit_keep_selection_markers_locked() {
    let callbacks = Rc::new(RefCell::new(Vec::new()));
    let captured = Rc::clone(&callbacks);
    let mut dropdown = multi_dropdown()
        .selected(["Alpha"])
        .on_select(move |ids| captured.borrow_mut().push(ids));
    dropdown.open();
    dropdown.set_disabled(true);
    dropdown.on_key(ctrl('j'), AREA);

    let toggle = dropdown.on_key(Key::Enter, AREA);
    let commit = dropdown.on_key(ctrl_enter(), AREA);

    assert!(toggle.handled);
    assert!(!toggle.changed);
    assert!(commit.closed);
    assert!(!commit.committed);
    assert_eq!(dropdown.selected_ids(), vec!["Alpha"]);
    assert_eq!(dropdown.draft, vec!["Alpha"]);
    assert_eq!(dropdown.data_view.selected_ids(), vec!["Alpha"]);
    assert!(callbacks.borrow().is_empty());
}

#[test]
fn disabled_direct_commit_closes_without_mutating_or_notifying() {
    let callbacks = Rc::new(RefCell::new(Vec::new()));
    let captured = Rc::clone(&callbacks);
    let mut dropdown = single_dropdown()
        .selected_one("Alpha")
        .on_select(move |ids| captured.borrow_mut().push(ids));
    dropdown.open();
    dropdown.on_key(ctrl('j'), AREA);
    dropdown.set_disabled(true);

    let outcome = dropdown.commit();

    assert!(outcome.closed);
    assert!(!outcome.committed);
    assert_eq!(dropdown.selected_id(), Some("Alpha"));
    assert!(callbacks.borrow().is_empty());
}

#[test]
fn disabled_dropdown_accepts_paste_and_external_editor_search_without_committing() {
    let callbacks = Rc::new(RefCell::new(Vec::new()));
    let captured = Rc::clone(&callbacks);
    let mut dropdown = single_dropdown()
        .selected_one("Alpha")
        .on_select(move |ids| captured.borrow_mut().push(ids));
    dropdown.open();
    dropdown.set_disabled(true);
    dropdown.focus_region = Some(DropdownFocusRegion::Search);
    dropdown.sync_child_focus();
    let mut ctx = EventCtx::<()>::default();

    let paste = dropdown.event(&TuiEvent::Paste("g".into()), &mut ctx);
    let editor = dropdown.event(
        &TuiEvent::ExternalEditor(crate::ExternalEditorResponse {
            value: "b".into(),
            line: 1,
            col: 1,
        }),
        &mut ctx,
    );

    assert_eq!(paste, EventOutcome::Handled);
    assert_eq!(editor, EventOutcome::Handled);
    assert_eq!(dropdown.search_query(), "b");
    assert_eq!(dropdown.filtered, vec!["Beta"]);
    assert_eq!(dropdown.selected_id(), Some("Alpha"));
    assert_eq!(dropdown.draft, vec!["Alpha"]);
    assert!(callbacks.borrow().is_empty());
}

#[test]
fn disabled_dropdown_accepts_paste_with_field_focus_after_external_editor() {
    let mut dropdown = single_dropdown()
        .auto_focus_search(false)
        .selected_one("Alpha")
        .disabled(true);
    dropdown.open();
    dropdown.focus_region = Some(DropdownFocusRegion::Field);
    dropdown.sync_child_focus();
    let mut ctx = EventCtx::<()>::default();

    assert_eq!(
        dropdown.event(
            &TuiEvent::ExternalEditor(ExternalEditorResponse {
                value: "b".into(),
                line: 1,
                col: 1,
            }),
            &mut ctx,
        ),
        EventOutcome::Handled
    );
    assert_eq!(
        dropdown.event(&TuiEvent::Paste("e".into()), &mut ctx),
        EventOutcome::Handled
    );
    assert_eq!(dropdown.search_query(), "eb");
    assert_eq!(dropdown.selected_id(), Some("Alpha"));
}

#[test]
fn disabled_dropdown_allows_programmatic_selection_and_row_updates() {
    let mut dropdown = single_dropdown().selected_one("Alpha").disabled(true);

    dropdown.set_selected_one("Beta");
    assert_eq!(dropdown.selected_id(), Some("Beta"));
    dropdown.clear_selection();
    assert_eq!(dropdown.selected_id(), None);
    dropdown.set_rows(["Beta", "Gamma"]);
    dropdown.set_selected_one("Gamma");

    assert_eq!(dropdown.selected_id(), Some("Gamma"));
}

#[test]
fn open_ctrl_j_and_ctrl_k_navigate_items() {
    let mut dropdown = single_dropdown();
    dropdown.open();

    assert!(dropdown.on_key(ctrl('j'), AREA).handled);
    assert_eq!(dropdown.data_view.highlighted_id(), Some("Beta"));
    assert!(dropdown.on_key(ctrl('k'), AREA).handled);
    assert_eq!(dropdown.data_view.highlighted_id(), Some("Alpha"));
    assert!(dropdown.is_open());
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
fn mouse_open_requests_search_focus() {
    let mut dropdown = single_dropdown();
    layout_dropdown(&mut dropdown, AREA, AREA);
    let mut ctx = EventCtx::<()>::default();

    let outcome = dropdown.event(&mouse_down(AREA.x, AREA.y), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(
        ctx.focus_request(),
        Some(&FocusRequest::TargetAt {
            path: TreePath::new(),
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
fn search_can_require_a_minimum_query_and_limit_matches() {
    let mut dropdown = Dropdown::single(
        ["Alpha", "Alpine", "Alpaca", "Beta"],
        |value| value.to_string(),
        |value| value.to_string(),
    )
    .min_search_chars(3)
    .max_filtered_items(2);

    dropdown.open();
    dropdown.on_key(char_key('a'), AREA);
    dropdown.on_key(char_key('l'), AREA);
    assert!(dropdown.filtered.is_empty());

    dropdown.on_key(char_key('p'), AREA);
    assert_eq!(dropdown.filtered.len(), 2);
}

#[test]
fn dropdown_can_show_a_default_subset_then_search_all_options() {
    let mut dropdown = Dropdown::single(
        ["Active one", "Active two", "Resolved match"],
        |value| value.to_string(),
        |value| value.to_string(),
    )
    .visible_without_search(["Active one".to_string(), "Active two".to_string()])
    .min_search_chars(1)
    .max_filtered_items(10);

    dropdown.open();
    assert_eq!(
        dropdown.filtered,
        ["Active one".to_string(), "Active two".to_string()]
    );

    dropdown.on_key(char_key('R'), AREA);
    assert_eq!(dropdown.filtered, ["Resolved match".to_string()]);
}

#[test]
fn replacing_rows_preserves_open_search_and_filters_new_options() {
    let mut dropdown = Dropdown::single(
        ["Old result"],
        |value| value.to_string(),
        |value| value.to_string(),
    );
    dropdown.open();
    dropdown.on_key(char_key('a'), AREA);
    dropdown.on_key(char_key('l'), AREA);

    dropdown.set_rows(["Alpha", "Alpine", "Beta"]);

    assert!(dropdown.is_open());
    assert_eq!(dropdown.search_query(), "al");
    assert_eq!(dropdown.filtered, ["Alpha", "Alpine"]);
}

#[test]
fn external_search_does_not_filter_or_highlight_stale_rows() {
    let mut dropdown = single_dropdown().search_mode(DropdownSearchMode::External);
    dropdown.open();

    dropdown.on_key(char_key('a'), AREA);
    dropdown.on_key(char_key('l'), AREA);

    assert_eq!(dropdown.filtered, ROWS);
    let line = highlighted_label_line(
        "Alpha".into(),
        dropdown.search_query(),
        DropdownSearchMode::External,
    );
    assert_eq!(line.spans.len(), 1);
    assert_eq!(line.spans[0].content, "Alpha");
    assert_eq!(line.spans[0].style, Style::default());
}

#[test]
fn search_mode_can_switch_from_external_to_fuzzy_at_runtime() {
    let mut dropdown = single_dropdown().search_mode(DropdownSearchMode::External);
    dropdown.open();
    dropdown.on_key(char_key('m'), AREA);
    dropdown.on_key(char_key('m'), AREA);
    assert_eq!(dropdown.filtered, ROWS);

    dropdown.set_search_mode(DropdownSearchMode::Fuzzy);

    assert_eq!(dropdown.filtered, ["Gamma"]);
}

#[test]
fn external_search_can_render_custom_loading_spinner() {
    let dropdown = single_dropdown()
        .search_mode(DropdownSearchMode::External)
        .external_loading(true)
        .external_loading_message("Searching Jira");
    let mut terminal = Terminal::new(TestBackend::new(24, 6)).unwrap();

    terminal
        .draw(|frame| {
            dropdown.render_popup(frame, frame.area(), DropdownPopupDirection::Down);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let rendered = (0..6)
        .flat_map(|y| (0..24).map(move |x| buffer.cell((x, y)).unwrap().symbol()))
        .collect::<String>();
    assert!(rendered.contains(dropdown.external_spinner.glyph()));
    assert!(rendered.contains("Searching Jira"));
    assert!(!rendered.contains("No results"));
}

#[test]
fn open_popup_highlights_matching_search_characters() {
    let mut dropdown = single_dropdown();
    dropdown.open();
    dropdown.on_key(char_key('a'), AREA);
    dropdown.on_key(char_key('l'), AREA);
    let mut terminal = Terminal::new(TestBackend::new(16, 6)).expect("terminal should build");

    terminal
        .draw(|frame| dropdown.render_popup(frame, frame.area(), DropdownPopupDirection::Down))
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
    let outcome = dropdown.on_key(Key::Enter, AREA);

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
fn immediate_cancel_keys_keep_committed_navigation_value() {
    for cancel in [KeyEvent::from(Key::Esc), ctrl('[')] {
        let mut dropdown = single_dropdown()
            .commit_mode(DropdownCommitMode::Immediate)
            .selected_one("Alpha");
        dropdown.open();
        dropdown.on_key(char_key('g'), AREA);
        dropdown.on_key(cancel, AREA);

        assert!(!dropdown.is_open());
        assert_eq!(dropdown.selected_id(), Some("Gamma"));
    }
}

#[test]
fn immediate_cancel_keeps_callback_and_selection_consistent() {
    let selected = Rc::new(RefCell::new(Vec::new()));
    let captured = Rc::clone(&selected);
    let mut dropdown = single_dropdown()
        .commit_mode(DropdownCommitMode::Immediate)
        .selected_one("Alpha")
        .on_select(move |ids| *captured.borrow_mut() = ids);

    dropdown.open();
    dropdown.on_key(ctrl('j'), AREA);
    dropdown.cancel();

    assert_eq!(dropdown.selected_id(), Some("Beta"));
    assert_eq!(&*selected.borrow(), &["Beta"]);
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
    dropdown.on_key(Key::Enter, AREA);
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
fn space_is_added_to_multi_dropdown_search_query() {
    let mut dropdown = multi_dropdown();

    dropdown.open();
    dropdown.on_key(char_key('a'), AREA);
    let outcome = dropdown.on_key(Key::Char(' '), AREA);

    assert!(outcome.changed);
    assert_eq!(dropdown.search_query(), "a ");
    assert!(dropdown.draft.is_empty());
}

#[test]
fn enter_toggles_and_ctrl_enter_commits_multi_selection() {
    let mut dropdown = multi_dropdown();

    dropdown.open();
    let toggle = dropdown.on_key(Key::Enter, AREA);

    assert!(toggle.changed);
    assert!(dropdown.is_open());
    assert_eq!(dropdown.draft, vec!["Alpha"]);
    assert!(dropdown.selected_ids().is_empty());

    let commit = dropdown.on_key(
        KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::CONTROL,
        },
        AREA,
    );

    assert!(commit.committed);
    assert!(!dropdown.is_open());
    assert_eq!(dropdown.selected_ids(), vec!["Alpha"]);
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
    assert_eq!(buffer.cell((0, 0)).unwrap().fg, theme().text_fg());
    assert_eq!(buffer.cell((0, 0)).unwrap().bg, theme().surface_bg());
    assert!(
        !buffer
            .cell((1, 0))
            .unwrap()
            .modifier
            .contains(Modifier::BOLD)
    );
    assert_eq!(buffer.cell((1, 0)).unwrap().symbol(), "B");
    assert_eq!(buffer.cell((10, 0)).unwrap().symbol(), "");
}

#[test]
fn filled_variant_renders_hotkey_and_reserves_its_width() {
    let dropdown = single_dropdown()
        .variant(DropdownVariant::Filled)
        .placeholder("Labels")
        .hotkey("ab");
    let measured =
        <Dropdown<_, _> as TuiNode<()>>::measure(&dropdown, LayoutProposal::at_most(40, 1));
    let mut terminal = Terminal::new(TestBackend::new(20, 1)).expect("terminal should build");

    terminal
        .draw(|frame| render_dropdown(&dropdown, frame, frame.area()))
        .expect("dropdown should render");

    let row = (0..20)
        .map(|x| terminal.backend().buffer().cell((x, 0)).unwrap().symbol())
        .collect::<String>();
    assert!(row.contains("Labels |ab|"));
    assert!(measured.preferred.width >= 14);
}

#[test]
fn focused_filled_trigger_uses_focus_style() {
    let mut dropdown = single_dropdown()
        .variant(DropdownVariant::Filled)
        .selected_one("Beta");
    let mut layout = LayoutCtx::new();
    <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut layout);
    let target = layout.focus_targets()[0].clone();
    dropdown.dispatch_focus(
        &target,
        true,
        &mut FocusCtx::<()>::new(AnimationSettings::default()),
    );
    let mut terminal = Terminal::new(TestBackend::new(12, 3)).expect("terminal should build");

    terminal
        .draw(|frame| render_dropdown(&dropdown, frame, frame.area()))
        .expect("dropdown should render");

    let cell = terminal.backend().buffer().cell((1, 0)).unwrap();
    assert_eq!(cell.fg, theme().highlight_fg());
    assert_eq!(cell.bg, theme().highlight_bg());
    assert!(cell.modifier.contains(Modifier::BOLD));
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
    assert_eq!(buffer.cell((0, 0)).unwrap().bg, theme().surface_bg());
}

#[test]
fn pressed_filled_trigger_pulses_success_then_returns_to_surface() {
    let mut dropdown = single_dropdown().variant(DropdownVariant::Filled);
    let mut event = EventCtx::<()>::default();

    assert_eq!(
        dropdown.event(&TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut event),
        EventOutcome::Handled
    );

    let render_background = |dropdown: &Dropdown<&'static str, &'static str>| {
        let mut terminal = Terminal::new(TestBackend::new(12, 1)).expect("terminal should build");
        terminal
            .draw(|frame| render_dropdown(dropdown, frame, frame.area()))
            .expect("dropdown should render");
        terminal.backend().buffer().cell((0, 0)).unwrap().bg
    };
    assert_eq!(render_background(&dropdown), theme().success_fg());

    for _ in 0..2 {
        Animated::tick(
            &mut dropdown,
            Duration::from_millis(100),
            AnimationSettings::default(),
        );
    }
    assert_eq!(render_background(&dropdown), theme().surface_bg());
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
    assert!(buffer.cell((7, 0)).unwrap().modifier.is_empty());
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
fn placeholder_renders_in_field_while_no_selection_text_renders_in_popup() {
    let mut dropdown = single_dropdown()
        .variant(DropdownVariant::Filled)
        .placeholder("Items")
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
    assert!(field.contains("Items"));
    assert!(!field.contains("--None--"));
    assert!(option.contains("--None--"));
}

#[test]
fn filled_alt_hotkey_placeholder_uses_muted_color() {
    let dropdown = single_dropdown()
        .variant(DropdownVariant::Filled)
        .placeholder("Items")
        .no_selection_text("--None--")
        .label("Immediate")
        .hotkey("6")
        .alt_style(true);
    let mut terminal = Terminal::new(TestBackend::new(16, 2)).expect("terminal should build");

    terminal
        .draw(|frame| render_dropdown(&dropdown, frame, frame.area()))
        .expect("dropdown should render");

    let placeholder = terminal.backend().buffer().cell((0, 1)).unwrap();
    assert_eq!(placeholder.symbol(), "I");
    assert_eq!(placeholder.fg, theme().muted_fg());
    assert_eq!(placeholder.bg, theme().surface_bg());
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
fn searchable_none_clears_selection_and_restores_placeholder_after_close() {
    let mut dropdown = single_dropdown()
        .variant(DropdownVariant::Filled)
        .placeholder("Items")
        .search_mode(DropdownSearchMode::Contains)
        .commit_mode(DropdownCommitMode::Immediate)
        .no_selection_text("--None--")
        .label("Immediate")
        .hotkey("6")
        .alt_style(true)
        .selected_one("Alpha");

    dropdown.open();
    dropdown.on_key(char_key('n'), AREA);
    dropdown.on_key(char_key('o'), AREA);

    assert!(dropdown.show_no_selection_row());
    assert!(dropdown.no_selection_highlighted);
    assert_eq!(dropdown.selected_id(), None);

    layout_dropdown(
        &mut dropdown,
        Rect::new(0, 0, 16, 2),
        Rect::new(0, 0, 16, 8),
    );
    let mut open_terminal = Terminal::new(TestBackend::new(16, 8)).expect("terminal should build");
    open_terminal
        .draw(|frame| render_dropdown(&dropdown, frame, Rect::new(0, 0, 16, 2)))
        .expect("dropdown should render");
    let option = (0..16)
        .map(|x| {
            open_terminal
                .backend()
                .buffer()
                .cell((x, 3))
                .unwrap()
                .symbol()
        })
        .collect::<String>();
    assert!(option.contains("--None--"));

    dropdown.on_key(Key::Enter, AREA);

    assert_eq!(dropdown.selected_id(), None);
    assert!(!dropdown.is_open());

    let mut terminal = Terminal::new(TestBackend::new(16, 2)).expect("terminal should build");
    terminal
        .draw(|frame| render_dropdown(&dropdown, frame, frame.area()))
        .expect("dropdown should render");
    let field = (0..16)
        .map(|x| terminal.backend().buffer().cell((x, 1)).unwrap().symbol())
        .collect::<String>();
    assert!(field.contains("Items"));
    assert!(!field.contains("--None--"));
}

#[test]
fn highlighted_matching_no_selection_row_uses_highlight_colors_for_all_text() {
    let mut dropdown = single_dropdown()
        .search_mode(DropdownSearchMode::Contains)
        .no_selection_text("None option");
    dropdown.open();
    dropdown.on_key(char_key('o'), AREA);
    dropdown.on_key(char_key('n'), AREA);
    let mut terminal = Terminal::new(TestBackend::new(16, 6)).expect("terminal should build");

    terminal
        .draw(|frame| dropdown.render_popup(frame, frame.area(), DropdownPopupDirection::Down))
        .expect("dropdown should render");

    let buffer = terminal.backend().buffer();
    for x in 1..=11 {
        let cell = buffer.cell((x, 2)).unwrap();
        assert_eq!(cell.fg, theme().highlight_fg());
        assert_eq!(cell.bg, theme().highlight_bg());
    }
    assert!((1..=11).any(|x| {
        buffer
            .cell((x, 2))
            .unwrap()
            .modifier
            .contains(Modifier::UNDERLINED)
    }));
    assert!(
        !buffer
            .cell((1, 2))
            .unwrap()
            .modifier
            .contains(Modifier::UNDERLINED)
    );
}

#[test]
fn narrow_popup_truncates_no_selection_text_at_unicode_cell_boundary() {
    let mut dropdown = single_dropdown()
        .search_mode(DropdownSearchMode::Contains)
        .no_selection_text("ab界cdZ");
    dropdown.open();
    dropdown.on_key(char_key('界'), AREA);
    let mut terminal = Terminal::new(TestBackend::new(8, 5)).expect("terminal should build");

    terminal
        .draw(|frame| dropdown.render_popup(frame, frame.area(), DropdownPopupDirection::Down))
        .expect("dropdown should render");

    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.cell((1, 2)).unwrap().symbol(), "a");
    assert_eq!(buffer.cell((2, 2)).unwrap().symbol(), "b");
    assert_eq!(buffer.cell((3, 2)).unwrap().symbol(), "界");
    assert_eq!(buffer.cell((5, 2)).unwrap().symbol(), "c");
    assert_eq!(buffer.cell((6, 2)).unwrap().symbol(), "d");
    assert_eq!(buffer.cell((7, 2)).unwrap().symbol(), "│");
}

#[test]
fn nonmatching_search_hides_no_selection_text() {
    let mut dropdown = single_dropdown()
        .search_mode(DropdownSearchMode::Contains)
        .no_selection_text("--None--");

    dropdown.open();
    dropdown.on_key(char_key('z'), AREA);

    assert!(!dropdown.show_no_selection_row());
    assert!(!dropdown.no_selection_highlighted);
}

#[test]
fn explicit_nonmatching_search_preserves_selected_value_on_commit() {
    let mut dropdown = single_dropdown()
        .search_mode(DropdownSearchMode::Contains)
        .no_selection_text("--None--")
        .selected_one("Beta");

    dropdown.open();
    dropdown.on_key(char_key('z'), AREA);

    assert!(dropdown.filtered.is_empty());
    assert_eq!(dropdown.draft, vec!["Beta"]);
    assert_eq!(dropdown.selected_id(), Some("Beta"));

    dropdown.on_key(Key::Enter, AREA);

    assert_eq!(dropdown.selected_id(), Some("Beta"));
}

#[test]
fn immediate_nonmatching_search_preserves_selected_value() {
    let mut dropdown = single_dropdown()
        .search_mode(DropdownSearchMode::Fuzzy)
        .commit_mode(DropdownCommitMode::Immediate)
        .no_selection_text("--None--")
        .selected_one("Beta");

    dropdown.open();
    let outcome = dropdown.on_key(char_key('z'), AREA);

    assert!(!outcome.committed);
    assert!(dropdown.filtered.is_empty());
    assert_eq!(dropdown.draft, vec!["Beta"]);
    assert_eq!(dropdown.selected_id(), Some("Beta"));

    dropdown.on_key(Key::Enter, AREA);

    assert_eq!(dropdown.selected_id(), Some("Beta"));
}

#[test]
fn no_selection_search_respects_contains_and_fuzzy_matching() {
    let mut contains = single_dropdown()
        .search_mode(DropdownSearchMode::Contains)
        .no_selection_text("No Selection");
    let mut fuzzy = single_dropdown()
        .search_mode(DropdownSearchMode::Fuzzy)
        .no_selection_text("No Selection");

    contains.open();
    fuzzy.open();
    for key in [char_key('n'), char_key('s')] {
        contains.on_key(key, AREA);
        fuzzy.on_key(key, AREA);
    }

    assert!(!contains.show_no_selection_row());
    assert!(fuzzy.show_no_selection_row());
}

#[test]
fn matching_no_selection_and_regular_rows_both_contribute_to_popup_height() {
    let mut dropdown = single_dropdown()
        .search_mode(DropdownSearchMode::Contains)
        .no_selection_text("Alpha none")
        .selected_one("Beta");

    dropdown.open();
    for key in "alpha".chars().map(char_key) {
        dropdown.on_key(key, AREA);
    }

    let [_, popup_area] = dropdown.areas(Rect::new(0, 0, 24, 20));
    let [_, list_area] = dropdown.popup_inner_areas(popup_area);

    assert_eq!(dropdown.filtered, vec!["Alpha"]);
    assert!(dropdown.show_no_selection_row());
    assert!(!dropdown.no_selection_highlighted);
    assert_eq!(dropdown.visible_popup_rows(), 2);
    assert_eq!(popup_area.height, 5);
    assert_eq!(list_area.height, 2);
}

#[test]
fn clearing_nonmatching_query_restores_preserved_selection_highlight() {
    let mut dropdown = single_dropdown()
        .commit_mode(DropdownCommitMode::Immediate)
        .no_selection_text("--None--")
        .selected_one("Beta");

    dropdown.open();
    dropdown.on_key(char_key('z'), AREA);
    dropdown.on_key(Key::Backspace, AREA);

    assert_eq!(dropdown.search_query(), "");
    assert_eq!(dropdown.filtered, ROWS.to_vec());
    assert!(dropdown.show_no_selection_row());
    assert!(!dropdown.no_selection_highlighted);
    assert_eq!(dropdown.data_view.highlighted_id(), Some("Beta"));
    assert_eq!(dropdown.selected_id(), Some("Beta"));
}

#[test]
fn multi_navigation_across_no_selection_row_preserves_draft() {
    let mut dropdown = multi_dropdown()
        .search_mode(DropdownSearchMode::None)
        .no_selection_text("--None--")
        .selected(["Beta"]);

    dropdown.open();
    dropdown.on_key(ctrl('k'), AREA);
    assert_eq!(dropdown.data_view.highlighted_id(), Some("Alpha"));

    dropdown.on_key(ctrl('k'), AREA);
    assert!(dropdown.no_selection_highlighted);
    assert_eq!(dropdown.draft, vec!["Beta"]);

    dropdown.on_key(ctrl('j'), AREA);
    assert!(!dropdown.no_selection_highlighted);
    assert_eq!(dropdown.draft, vec!["Beta"]);
}

#[test]
fn multi_toggle_on_no_selection_row_clears_draft_before_commit() {
    let mut dropdown = multi_dropdown()
        .search_mode(DropdownSearchMode::None)
        .no_selection_text("--None--")
        .selected(["Alpha", "Beta"]);

    dropdown.open();
    dropdown.on_key(ctrl('k'), AREA);
    assert!(dropdown.no_selection_highlighted);

    let toggle = dropdown.on_key(Key::Enter, AREA);

    assert!(toggle.changed);
    assert!(dropdown.is_open());
    assert!(dropdown.draft.is_empty());

    let commit = dropdown.on_key(ctrl_enter(), AREA);

    assert!(commit.committed);
    assert!(!dropdown.is_open());
    assert!(dropdown.selected_ids().is_empty());
}

#[test]
fn multi_ctrl_enter_on_no_selection_row_clears_commits_and_closes() {
    let mut dropdown = multi_dropdown()
        .search_mode(DropdownSearchMode::None)
        .no_selection_text("--None--")
        .selected(["Alpha", "Beta"]);

    dropdown.open();
    dropdown.on_key(ctrl('k'), AREA);
    assert!(dropdown.no_selection_highlighted);
    assert_eq!(dropdown.draft, vec!["Alpha", "Beta"]);

    let outcome = dropdown.on_key(ctrl_enter(), AREA);

    assert!(outcome.committed);
    assert!(outcome.closed);
    assert!(!dropdown.is_open());
    assert!(dropdown.selected_ids().is_empty());
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
fn focused_bordered_dropdown_uses_bold_accent_border() {
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
    let border = buffer.cell((0, 2)).unwrap();
    assert_eq!(border.fg, theme().accent_fg());
    assert!(border.modifier.contains(Modifier::BOLD));
}

#[test]
fn unfocused_bordered_dropdown_lacks_focus_cue() {
    let dropdown = single_dropdown();
    let mut terminal = Terminal::new(TestBackend::new(12, 3)).expect("terminal should build");

    terminal
        .draw(|frame| render_dropdown(&dropdown, frame, frame.area()))
        .expect("dropdown should render");

    let border = terminal.backend().buffer().cell((0, 2)).unwrap();
    assert_eq!(border.fg, theme().border_fg());
    assert!(!border.modifier.contains(Modifier::BOLD));
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
fn open_non_search_dropdown_suppresses_global_control_traversal() {
    let mut dropdown = single_dropdown().search_mode(DropdownSearchMode::None);
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
fn tab_keys_cancel_and_request_directional_focus() {
    for (key, request) in [
        (Key::Tab, FocusRequest::Next),
        (Key::BackTab, FocusRequest::Previous),
    ] {
        let mut dropdown = single_dropdown();
        dropdown.open();
        let mut ctx = EventCtx::<()>::default();

        let outcome = dropdown.event(&TuiEvent::Key(KeyEvent::from(key)), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert!(!dropdown.is_open());
        assert_eq!(ctx.focus_request(), Some(&request));
        assert!(ctx.layout_requested());
        assert!(ctx.redraw_requested());
        assert_eq!(ctx.propagation(), Propagation::Stopped);
    }
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
fn dynamic_popup_opens_down_when_desired_height_fits() {
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
fn dynamic_popup_opens_down_when_desired_height_exactly_fits() {
    let mut dropdown = single_dropdown();
    dropdown.open();
    let field_area = Rect::new(0, 4, 24, 3);
    let bounds = Rect::new(0, 0, 24, 12);
    layout_dropdown(&mut dropdown, field_area, bounds);

    let popup_area = dropdown.popup_overlay_area(bounds);

    assert_eq!(popup_area, Rect::new(0, 6, 24, 6));
}

#[test]
fn dynamic_popup_opens_up_when_desired_height_would_clip_below() {
    let mut dropdown = single_dropdown().label("Size");
    dropdown.open();
    let field_area = Rect::new(0, 8, 24, 3);
    let bounds = Rect::new(0, 0, 24, 12);
    layout_dropdown(&mut dropdown, field_area, bounds);

    let popup_area = dropdown.popup_overlay_area(bounds);

    assert_eq!(popup_area, Rect::new(0, 3, 24, 6));

    let mut terminal = Terminal::new(TestBackend::new(bounds.width, bounds.height)).unwrap();
    terminal
        .draw(|frame| render_dropdown(&dropdown, frame, field_area))
        .unwrap();

    let buffer = terminal.backend().buffer();
    let chars = border_chars(preset().border());
    assert_eq!(
        buffer.cell((0, field_area.y)).unwrap().symbol(),
        chars.left_join
    );
    assert_eq!(
        buffer.cell((23, field_area.y)).unwrap().symbol(),
        chars.right_join
    );
}

#[test]
fn explicit_down_popup_stays_down_and_clips() {
    let mut dropdown = single_dropdown().popup_direction(DropdownPopupDirection::Down);
    dropdown.open();
    let field_area = Rect::new(0, 8, 24, 3);
    let bounds = Rect::new(0, 0, 24, 12);
    layout_dropdown(&mut dropdown, field_area, bounds);

    let popup_area = dropdown.popup_overlay_area(bounds);

    assert_eq!(popup_area, Rect::new(0, 10, 24, 2));
}

#[test]
fn explicit_up_popup_stays_up_and_clips_when_below_has_room() {
    let mut dropdown = single_dropdown().popup_direction(DropdownPopupDirection::Up);
    dropdown.open();
    let field_area = Rect::new(0, 1, 24, 3);
    let bounds = Rect::new(0, 0, 24, 12);
    layout_dropdown(&mut dropdown, field_area, bounds);

    let popup_area = dropdown.popup_overlay_area(bounds);

    assert_eq!(popup_area, Rect::new(0, 0, 24, 2));
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
fn upward_bordered_popup_connects_to_trigger_and_preserves_header() {
    let mut dropdown = single_dropdown()
        .label("Size")
        .popup_direction(DropdownPopupDirection::Up);
    dropdown.open();
    let field_area = Rect::new(0, 8, 24, 3);
    let bounds = Rect::new(0, 0, 24, 12);
    layout_dropdown(&mut dropdown, field_area, bounds);

    let popup_area = dropdown.popup_overlay_area(bounds);

    assert_eq!(popup_area.y + popup_area.height, field_area.y + 1);

    let mut terminal = Terminal::new(TestBackend::new(bounds.width, bounds.height)).unwrap();
    terminal
        .draw(|frame| render_dropdown(&dropdown, frame, field_area))
        .unwrap();

    let buffer = terminal.backend().buffer();
    let chars = border_chars(preset().border());
    assert_eq!(
        buffer.cell((0, popup_area.y)).unwrap().symbol(),
        chars.top_left
    );
    assert_eq!(
        buffer.cell((23, popup_area.y)).unwrap().symbol(),
        chars.top_right
    );
    assert_eq!(
        buffer.cell((0, field_area.y)).unwrap().symbol(),
        chars.left_join
    );
    assert_eq!(
        buffer.cell((23, field_area.y)).unwrap().symbol(),
        chars.right_join
    );
    let shared_header = (0..24)
        .map(|x| buffer.cell((x, field_area.y)).unwrap().symbol())
        .collect::<String>();
    assert!(shared_header.contains("Size"));
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
        Some(theme().surface_bg())
    );
    assert_eq!(buffer.cell((0, 3)).unwrap().symbol(), "B");
    assert_eq!(buffer.cell((0, 3)).unwrap().bg, theme().surface_bg());
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
fn left_click_on_closed_field_opens_dropdown() {
    let mut dropdown = single_dropdown();
    layout_dropdown(
        &mut dropdown,
        Rect::new(4, 2, 12, 3),
        Rect::new(0, 0, 24, 12),
    );
    let mut event = EventCtx::<()>::default();

    let outcome = dropdown.event(&mouse_down(4, 2), &mut event);

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(dropdown.is_open());
    assert!(event.layout_requested());
    assert_eq!(event.propagation(), Propagation::Stopped);
}

#[test]
fn left_click_on_popup_row_selects_and_commits_single_dropdown() {
    let mut dropdown = single_dropdown().search_mode(DropdownSearchMode::None);
    let bounds = Rect::new(0, 0, 24, 12);
    dropdown.open();
    layout_dropdown(&mut dropdown, Rect::new(4, 2, 12, 3), bounds);
    let popup_area = dropdown.popup_overlay_area(bounds);
    let row_area = dropdown.popup_rows_area(dropdown.popup_inner_areas(popup_area)[1]);
    let mut event = EventCtx::<()>::default();

    let outcome = dropdown.event(&mouse_down(row_area.x, row_area.y + 1), &mut event);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(dropdown.selected_id(), Some("Beta"));
    assert!(!dropdown.is_open());
    assert!(event.layout_requested());
    assert_eq!(event.propagation(), Propagation::Stopped);
}

#[test]
fn search_event_requests_layout_when_popup_height_and_direction_change() {
    let mut dropdown = single_dropdown();
    dropdown.open();
    let field_area = Rect::new(0, 4, 24, 3);
    let bounds = Rect::new(0, 0, 24, 10);
    layout_dropdown(&mut dropdown, field_area, bounds);
    assert_eq!(dropdown.popup_overlay_area(bounds), Rect::new(0, 0, 24, 5));
    let mut event = EventCtx::<()>::default();

    let outcome = dropdown.event(&TuiEvent::Key(char_key('z')), &mut event);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(dropdown.popup_overlay_area(bounds), Rect::new(0, 6, 24, 4));
    assert!(event.layout_requested());
    assert!(event.redraw_requested());
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

fn ctrl_enter() -> KeyEvent {
    KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::CONTROL,
    }
}

fn mouse_down(column: u16, row: u16) -> TuiEvent {
    TuiEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

fn open_list_area<T, Id>(dropdown: &mut Dropdown<T, Id>, area: Rect) -> Rect
where
    T: 'static,
    Id: Clone + Eq + Hash + 'static,
{
    let popup_area = dropdown.popup_overlay_area(area);
    dropdown.popup_inner_areas(popup_area)[1]
}
