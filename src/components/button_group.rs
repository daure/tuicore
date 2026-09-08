use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::{
    Animated, AnimationSettings, EventCtx, EventOutcome, FocusCtx, FocusId, FocusRequest,
    HintSource, HitRegion, HotkeyEvent, HotkeyLabelMode, HotkeyMatch, HotkeySequenceMatcher,
    LayoutCtx, LayoutProposal, LayoutResult, LayoutSize, LayoutSizeHint, MouseButton,
    MouseEventKind, TickResult, TuiEvent, TuiNode, hotkey_label_spans, hotkey_underline_style,
    keybindings, lerp_color, line_width, theme,
};

const BUTTON_GROUP_FOCUS: &str = "button-group";
const LEFT_CAP: &str = "";
const RIGHT_CAP: &str = "";

type SelectHandler<Id, M> = Box<dyn Fn(&Id) -> M>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonGroupItem<Id> {
    id: Id,
    label: String,
    prepend_icon: Option<String>,
    hotkey: Option<String>,
}

impl<Id> ButtonGroupItem<Id> {
    pub fn new(id: Id, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            prepend_icon: None,
            hotkey: None,
        }
    }

    pub fn prepend_icon(mut self, icon: impl Into<String>) -> Self {
        self.prepend_icon = Some(icon.into());
        self
    }

    pub fn hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.hotkey = Some(hotkey.into());
        self
    }

    pub fn id(&self) -> &Id {
        &self.id
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonGroupOutcome {
    pub handled: bool,
    pub changed: bool,
    pub selected: Option<usize>,
}

impl ButtonGroupOutcome {
    const fn ignored(selected: Option<usize>) -> Self {
        Self {
            handled: false,
            changed: false,
            selected,
        }
    }

    const fn handled(selected: Option<usize>, changed: bool) -> Self {
        Self {
            handled: true,
            changed,
            selected,
        }
    }
}

pub struct ButtonGroup<Id, M = ()> {
    items: Vec<ButtonGroupItem<Id>>,
    selected: usize,
    focused: bool,
    tab_stop: bool,
    hotkey_label_mode: HotkeyLabelMode,
    hotkey_matcher: HotkeySequenceMatcher,
    pending_hotkey_prefix: Option<String>,
    on_select: Option<SelectHandler<Id, M>>,
    area: Rect,
}

impl<Id, M> ButtonGroup<Id, M> {
    pub fn new(items: impl IntoIterator<Item = ButtonGroupItem<Id>>) -> Self {
        let items = items.into_iter().collect::<Vec<_>>();
        let hotkey_matcher = item_hotkey_matcher(&items);
        Self {
            items,
            selected: 0,
            focused: false,
            tab_stop: true,
            hotkey_label_mode: HotkeyLabelMode::PreferMnemonic,
            hotkey_matcher,
            pending_hotkey_prefix: None,
            on_select: None,
            area: Rect::default(),
        }
    }

    pub fn selected(mut self, selected: usize) -> Self {
        self.set_selected(selected);
        self
    }

    pub fn set_selected(&mut self, selected: usize) -> bool {
        if selected >= self.items.len() || selected == self.selected {
            return false;
        }
        self.selected = selected;
        true
    }

    pub fn selected_index(&self) -> Option<usize> {
        (!self.items.is_empty()).then_some(self.selected)
    }

    pub fn selected_id(&self) -> Option<&Id> {
        self.items.get(self.selected).map(ButtonGroupItem::id)
    }

    pub fn items(&self) -> &[ButtonGroupItem<Id>] {
        &self.items
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.set_focused(focused);
        self
    }

    pub fn set_focused(&mut self, focused: bool) {
        if self.focused && !focused {
            self.hotkey_matcher = item_hotkey_matcher(&self.items);
            self.pending_hotkey_prefix = None;
        }
        self.focused = focused;
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn tab_stop(mut self, tab_stop: bool) -> Self {
        self.tab_stop = tab_stop;
        self
    }

    pub fn hotkey_label_mode(mut self, mode: HotkeyLabelMode) -> Self {
        self.hotkey_label_mode = mode;
        self
    }

    pub fn on_select(mut self, handler: impl Fn(&Id) -> M + 'static) -> Self {
        self.on_select = Some(Box::new(handler));
        self
    }

    pub fn select(&mut self, selected: usize) -> ButtonGroupOutcome {
        if selected >= self.items.len() {
            return ButtonGroupOutcome::ignored(self.selected_index());
        }
        let changed = self.set_selected(selected);
        ButtonGroupOutcome::handled(self.selected_index(), changed)
    }

    pub fn on_key(&mut self, key: impl Into<crate::KeyEvent>) -> ButtonGroupOutcome {
        let key = key.into();
        match self.hotkey_matcher.on_key(key) {
            HotkeyMatch::Matched(hotkey_index) => {
                let Some(item_index) = self.hotkey_item_index(hotkey_index) else {
                    return ButtonGroupOutcome::ignored(self.selected_index());
                };
                return self.select(item_index);
            }
            HotkeyMatch::Pending | HotkeyMatch::Canceled => {
                return ButtonGroupOutcome::handled(self.selected_index(), false);
            }
            HotkeyMatch::Ignored => {}
        }
        if !self.focused || self.items.is_empty() {
            return ButtonGroupOutcome::ignored(self.selected_index());
        }
        if keybindings().line_left_matches(key) {
            return self.select(self.selected.saturating_sub(1));
        }
        if keybindings().line_right_matches(key) {
            return self.select((self.selected + 1).min(self.items.len() - 1));
        }
        ButtonGroupOutcome::ignored(self.selected_index())
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }
        frame.render_widget(Paragraph::new(self.line()), area);
    }

    pub fn line(&self) -> Line<'static> {
        if self.items.is_empty() {
            return Line::default();
        }
        let active_prefix = if self.hotkey_matcher.prefix().is_empty() {
            self.pending_hotkey_prefix.as_deref()
        } else {
            Some(self.hotkey_matcher.prefix())
        };
        let first_background = self.item_colors(0).0;
        let mut spans = vec![Span::styled(
            LEFT_CAP,
            Style::default().fg(first_background),
        )];
        for (index, item) in self.items.iter().enumerate() {
            spans.extend(self.item_spans(index, item, active_prefix));
        }
        let last_background = self.item_colors(self.items.len() - 1).0;
        spans.push(Span::styled(
            RIGHT_CAP,
            Style::default().fg(last_background),
        ));
        Line::from(spans)
    }

    fn item_spans(
        &self,
        index: usize,
        item: &ButtonGroupItem<Id>,
        active_prefix: Option<&str>,
    ) -> Vec<Span<'static>> {
        let (background, foreground) = self.item_colors(index);
        let selected_modifier = if index == self.selected {
            Modifier::BOLD
        } else {
            Modifier::empty()
        };
        let base_style = Style::default()
            .fg(foreground)
            .bg(background)
            .add_modifier(selected_modifier);
        let mut spans = Vec::new();
        if index > 0 {
            spans.push(Span::styled(" ", base_style));
        }
        if let Some(icon) = &item.prepend_icon {
            spans.push(Span::styled(format!("{icon} "), base_style));
        }
        let hotkey_style = hotkey_underline_style(base_style);
        let mut label_spans = hotkey_label_spans(
            &item.label,
            item.hotkey.as_deref(),
            self.hotkey_label_mode,
            active_prefix,
            base_style,
            hotkey_style,
        );
        if let (Some(start), Some(end)) = (
            label_spans.iter().position(|span| span.content == "|"),
            label_spans.iter().rposition(|span| span.content == "|"),
        ) {
            for span in &mut label_spans[start + 1..end] {
                span.style = hotkey_style;
            }
        }
        spans.extend(label_spans);
        if index + 1 < self.items.len() {
            spans.push(Span::styled(" ", base_style));
        }
        spans
    }

    fn item_colors(&self, index: usize) -> (ratatui::style::Color, ratatui::style::Color) {
        let theme = theme();
        if index == self.selected && self.focused {
            (
                theme.accent_fg(),
                theme.contrast_foreground(theme.accent_fg()),
            )
        } else if index == self.selected {
            (theme.highlight_bg(), theme.highlight_fg())
        } else if index.is_multiple_of(2) {
            (
                lerp_color(theme.surface_bg(), theme.background_bg(), 0.05),
                theme.text_fg(),
            )
        } else {
            (
                lerp_color(theme.surface_bg(), theme.text_fg(), 0.05),
                theme.text_fg(),
            )
        }
    }

    fn item_width(&self, index: usize) -> usize {
        let item = &self.items[index];
        line_width(&Line::from(self.item_spans(index, item, None)))
    }

    fn item_at(&self, column: u16, row: u16) -> Option<usize> {
        if row < self.area.y || row >= self.area.bottom() || column < self.area.x {
            return None;
        }
        let offset = usize::from(column - self.area.x);
        let mut start = 1;
        for index in 0..self.items.len() {
            let end = start + self.item_width(index);
            if (start..end).contains(&offset) {
                return Some(index);
            }
            start = end;
        }
        None
    }

    fn hotkey_item_index(&self, hotkey_index: usize) -> Option<usize> {
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.hotkey.is_some())
            .nth(hotkey_index)
            .map(|(index, _)| index)
    }

    fn hotkey_commit_index(&self, sequence: &str) -> Option<usize> {
        let sequence = crate::hotkey::normalize_hotkey(sequence);
        self.items.iter().position(|item| {
            item.hotkey
                .as_deref()
                .is_some_and(|hotkey| crate::hotkey::normalize_hotkey(hotkey) == sequence)
        })
    }

    fn emit_selection(&self, outcome: ButtonGroupOutcome, ctx: &mut EventCtx<M>) {
        if outcome.changed
            && let (Some(on_select), Some(id)) = (&self.on_select, self.selected_id())
        {
            ctx.emit(on_select(id));
        }
    }
}

impl<Id, M> TuiNode<M> for ButtonGroup<Id, M>
where
    M: 'static,
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let width = line_width(&self.line()).min(u16::MAX as usize) as u16;
        LayoutSizeHint {
            source: HintSource::Measured,
            min: LayoutSize::new(width, 1),
            preferred: LayoutSize::new(width, 1),
            expand: crate::AxisExpand {
                width: false,
                height: false,
            },
        }
        .normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.area = area;
        ctx.register_hit_region(HitRegion::new(ctx.current_path(), area));
        let hotkeys = self
            .items
            .iter()
            .filter_map(|item| item.hotkey.clone())
            .collect::<Vec<_>>();
        if hotkeys.is_empty() {
            ctx.register_focusable(FocusId::new(BUTTON_GROUP_FOCUS), area, true);
        } else {
            ctx.register_focusable_with_hotkey_sequences(
                FocusId::new(BUTTON_GROUP_FOCUS),
                area,
                true,
                hotkeys,
            );
        }
        ctx.set_focus_tab_stop(FocusId::new(BUTTON_GROUP_FOCUS), self.tab_stop);
        ctx.set_focus_control(FocusId::new(BUTTON_GROUP_FOCUS), true);
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _ctx: &mut crate::RenderCtx<'_>) {
        Self::render(self, frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        let outcome = match event {
            TuiEvent::Hotkey(HotkeyEvent::Pending(prefix)) => {
                self.pending_hotkey_prefix = Some(prefix.clone());
                ctx.request_redraw();
                return EventOutcome::Ignored;
            }
            TuiEvent::Hotkey(HotkeyEvent::Canceled) => {
                if self.pending_hotkey_prefix.take().is_some() {
                    ctx.request_redraw();
                }
                return EventOutcome::Ignored;
            }
            TuiEvent::Hotkey(HotkeyEvent::Commit(sequence)) => {
                self.pending_hotkey_prefix = None;
                let Some(index) = self.hotkey_commit_index(sequence) else {
                    return EventOutcome::Ignored;
                };
                self.select(index)
            }
            TuiEvent::Mouse(mouse)
                if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) =>
            {
                let Some(index) = self.item_at(mouse.column, mouse.row) else {
                    return EventOutcome::Ignored;
                };
                ctx.focus(FocusRequest::TargetAt {
                    path: ctx.current_path(),
                    id: FocusId::new(BUTTON_GROUP_FOCUS),
                });
                self.select(index)
            }
            TuiEvent::Key(key) => self.on_key(*key),
            _ => return EventOutcome::Ignored,
        };
        if !outcome.handled {
            return EventOutcome::Ignored;
        }
        self.emit_selection(outcome, ctx);
        ctx.request_redraw();
        ctx.stop_propagation();
        EventOutcome::Handled
    }

    fn focus(&mut self, _target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        self.set_focused(focused);
        ctx.request_redraw();
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(self, dt, settings)
    }
}

impl<Id, M> Animated for ButtonGroup<Id, M> {
    fn tick(&mut self, dt: Duration, _settings: AnimationSettings) -> TickResult {
        if self.hotkey_matcher.tick(dt) {
            TickResult::CHANGED
        } else {
            TickResult::IDLE
        }
    }
}

fn item_hotkey_matcher<Id>(items: &[ButtonGroupItem<Id>]) -> HotkeySequenceMatcher {
    HotkeySequenceMatcher::new(items.iter().filter_map(|item| item.hotkey.clone()))
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::{Key, KeyEvent, KeyModifiers, MouseEvent, Propagation, TreePath};

    fn group<M>() -> ButtonGroup<&'static str, M> {
        ButtonGroup::new([
            ButtonGroupItem::new("success", "Success")
                .prepend_icon("")
                .hotkey("s"),
            ButtonGroupItem::new("warning", "Warning")
                .prepend_icon("")
                .hotkey("w"),
            ButtonGroupItem::new("prepend", "Prepend")
                .prepend_icon("")
                .hotkey("p"),
        ])
    }

    #[test]
    fn renders_one_contiguous_rounded_group() {
        let group = group::<()>();
        let mut terminal = Terminal::new(TestBackend::new(40, 1)).expect("terminal should build");

        terminal
            .draw(|frame| group.render(frame, frame.area()))
            .expect("button group should render");

        let row = (0..40)
            .map(|x| terminal.backend().buffer().cell((x, 0)).unwrap().symbol())
            .collect::<String>();
        assert!(row.starts_with(" Success   Warning   Prepend"));
        let right_cap = terminal.backend().buffer().cell((32, 0)).unwrap();
        assert_eq!(
            right_cap.fg,
            lerp_color(theme().surface_bg(), theme().background_bg(), 0.05)
        );
        let selected = terminal.backend().buffer().cell((3, 0)).unwrap();
        let light = terminal.backend().buffer().cell((14, 0)).unwrap();
        let dark = terminal.backend().buffer().cell((25, 0)).unwrap();
        assert_eq!(selected.bg, theme().highlight_bg());
        assert!(selected.modifier.contains(Modifier::BOLD));
        assert_eq!(
            light.bg,
            lerp_color(theme().surface_bg(), theme().text_fg(), 0.05)
        );
        assert_eq!(
            dark.bg,
            lerp_color(theme().surface_bg(), theme().background_bg(), 0.05)
        );
        assert_ne!(light.bg, dark.bg);
    }

    #[test]
    fn focused_navigation_changes_the_single_selection() {
        let mut group = group::<()>().focused(true);

        let right = group.on_key(Key::Right);
        let vim_right = group.on_key(Key::Char('l'));
        let left = group.on_key(Key::Char('h'));

        assert_eq!(right.selected, Some(1));
        assert!(right.changed);
        assert_eq!(vim_right.selected, Some(2));
        assert_eq!(left.selected, Some(1));
        assert_eq!(group.selected_id(), Some(&"warning"));
    }

    #[test]
    fn navigation_is_ignored_while_blurred() {
        let mut group = group::<()>();

        let outcome = group.on_key(Key::Right);

        assert_eq!(outcome, ButtonGroupOutcome::ignored(Some(0)));
        assert_eq!(group.selected_index(), Some(0));
    }

    #[test]
    fn item_hotkey_selects_and_emits_the_item_id() {
        let mut group = group().on_select(|id| *id);
        let mut layout = LayoutCtx::new();
        group.layout(Rect::new(0, 0, 40, 1), &mut layout);
        let mut ctx = EventCtx::default();

        let outcome = group.event(
            &TuiEvent::Hotkey(HotkeyEvent::Commit("w".to_string())),
            &mut ctx,
        );

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(group.selected_id(), Some(&"warning"));
        assert_eq!(ctx.drain_messages().collect::<Vec<_>>(), vec!["warning"]);
        assert_eq!(ctx.propagation(), Propagation::Stopped);
        assert_eq!(layout.focus_targets()[0].hotkey_sequences, ["s", "w", "p"]);
    }

    #[test]
    fn clicking_an_item_selects_it_and_focuses_the_group() {
        let mut group = group().on_select(|id| *id);
        group.layout(Rect::new(4, 2, 40, 1), &mut LayoutCtx::new());
        let mut ctx = EventCtx::new_at_path(AnimationSettings::default(), TreePath::new());

        let outcome = group.event(
            &TuiEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 16,
                row: 2,
                modifiers: KeyModifiers::NONE,
            }),
            &mut ctx,
        );

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(group.selected_id(), Some(&"warning"));
        assert_eq!(ctx.drain_messages().collect::<Vec<_>>(), vec!["warning"]);
        assert_eq!(
            ctx.focus_request(),
            Some(&FocusRequest::TargetAt {
                path: TreePath::new(),
                id: FocusId::new(BUTTON_GROUP_FOCUS),
            })
        );
    }

    #[test]
    fn selected_item_is_bold_and_uses_focus_aware_semantic_color() {
        let blurred_group = group::<()>().selected(1);
        let line = blurred_group.line();
        let bold_spans = line
            .spans
            .iter()
            .filter(|span| span.style.add_modifier.contains(Modifier::BOLD))
            .collect::<Vec<_>>();

        assert!(!bold_spans.is_empty());
        assert!(bold_spans.iter().all(|span| {
            span.style.bg == Some(theme().highlight_bg())
                && !span.content.contains("Success")
                && !span.content.contains("Prepend")
        }));

        let focused_line = group::<()>().selected(1).focused(true).line();
        assert!(
            focused_line
                .spans
                .iter()
                .filter(|span| span.style.add_modifier.contains(Modifier::BOLD))
                .all(|span| span.style.bg == Some(theme().accent_fg()))
        );
    }

    #[test]
    fn hotkey_letters_are_underlined_in_labels() {
        let line = group::<()>().line();
        let underlined = line
            .spans
            .iter()
            .filter(|span| span.style.add_modifier.contains(Modifier::UNDERLINED))
            .map(|span| span.content.as_ref())
            .collect::<Vec<_>>();

        assert_eq!(underlined, ["S", "W", "P"]);
    }

    #[test]
    fn missing_hotkey_letter_is_appended_with_padding() {
        let group = ButtonGroup::<_, ()>::new([ButtonGroupItem::new("help", "Help").hotkey("x")]);
        let line = group.line();
        let text = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        let hotkey = line
            .spans
            .iter()
            .find(|span| span.content == "x")
            .expect("fallback hotkey should render");

        assert_eq!(text, "Help |x|");
        assert!(hotkey.style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn direct_multiletter_hotkey_waits_for_completion() {
        let mut group = ButtonGroup::<_, ()>::new([
            ButtonGroupItem::new("one", "One").hotkey("go"),
            ButtonGroupItem::new("two", "Two").hotkey("gt"),
        ]);

        let pending = group.on_key(KeyEvent::from(Key::Char('g')));
        let selected = group.on_key(KeyEvent::from(Key::Char('t')));

        assert!(pending.handled);
        assert!(!pending.changed);
        assert_eq!(selected.selected, Some(1));
        assert!(selected.changed);
    }
}
