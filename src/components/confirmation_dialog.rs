use std::rc::Rc;
use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;

use super::{Dialog, DialogAction, DialogCloseReason};
use crate::KeySpec;
use crate::{
    Animated, AnimationSettings, EventCtx, EventOutcome, FocusCtx, FocusId, LayoutCtx,
    LayoutProposal, LayoutResult, LayoutSizeHint, TickResult, TuiEvent, TuiNode,
};

const DEFAULT_YES_TEXT: &str = "Ok";
const DEFAULT_NO_TEXT: &str = "Cancel";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmationDialogOutcome {
    Confirmed,
    Cancelled,
    Closed(DialogCloseReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfirmationDialogKeyBindings {
    pub yes: Option<KeySpec>,
    pub no: Option<KeySpec>,
}

impl Default for ConfirmationDialogKeyBindings {
    fn default() -> Self {
        Self {
            yes: Some(KeySpec::plain('o')),
            no: Some(KeySpec::plain('c')),
        }
    }
}

type OutcomeHandler<M> = Rc<dyn Fn(ConfirmationDialogOutcome) -> M>;

pub struct ConfirmationDialog<M = ()> {
    title: String,
    description: String,
    yes_text: String,
    no_text: String,
    keys: ConfirmationDialogKeyBindings,
    on_outcome: Option<OutcomeHandler<M>>,
    dialog: Dialog<M>,
}

impl<M> ConfirmationDialog<M>
where
    M: 'static,
{
    pub fn new(title: impl Into<String>, description: impl Into<String>) -> Self {
        let title = title.into();
        let description = description.into();
        let yes_text = DEFAULT_YES_TEXT.to_string();
        let no_text = DEFAULT_NO_TEXT.to_string();
        let keys = ConfirmationDialogKeyBindings::default();
        let dialog = confirmation_dialog(&title, &description, &yes_text, &no_text, keys, None);
        Self {
            title,
            description,
            yes_text,
            no_text,
            keys,
            on_outcome: None,
            dialog,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.set_title(title);
        self
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
        self.rebuild();
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.set_description(description);
        self
    }

    pub fn set_description(&mut self, description: impl Into<String>) {
        self.description = description.into();
        self.rebuild();
    }

    pub fn yes_text(mut self, text: impl Into<String>) -> Self {
        self.set_yes_text(text);
        self
    }

    pub fn set_yes_text(&mut self, text: impl Into<String>) {
        self.yes_text = text.into();
        self.rebuild();
    }

    pub fn no_text(mut self, text: impl Into<String>) -> Self {
        self.set_no_text(text);
        self
    }

    pub fn set_no_text(&mut self, text: impl Into<String>) {
        self.no_text = text.into();
        self.rebuild();
    }

    pub fn yes_hotkey(mut self, hotkey: KeySpec) -> Self {
        self.keys.yes = Some(hotkey);
        self.rebuild();
        self
    }

    pub fn no_hotkey(mut self, hotkey: KeySpec) -> Self {
        self.keys.no = Some(hotkey);
        self.rebuild();
        self
    }

    pub fn clear_yes_hotkey(mut self) -> Self {
        self.keys.yes = None;
        self.rebuild();
        self
    }

    pub fn clear_no_hotkey(mut self) -> Self {
        self.keys.no = None;
        self.rebuild();
        self
    }

    pub fn keybindings(mut self, keys: ConfirmationDialogKeyBindings) -> Self {
        self.keys = keys;
        self.rebuild();
        self
    }

    pub fn set_keybindings(&mut self, keys: ConfirmationDialogKeyBindings) {
        self.keys = keys;
        self.rebuild();
    }

    pub fn on_outcome(
        mut self,
        handler: impl Fn(ConfirmationDialogOutcome) -> M + 'static,
    ) -> Self {
        self.on_outcome = Some(Rc::new(handler));
        self.rebuild();
        self
    }

    pub fn dialog(&self) -> &Dialog<M> {
        &self.dialog
    }

    pub fn dialog_mut(&mut self) -> &mut Dialog<M> {
        &mut self.dialog
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        self.dialog.render(frame, area);
    }

    fn rebuild(&mut self) {
        self.dialog = confirmation_dialog(
            &self.title,
            &self.description,
            &self.yes_text,
            &self.no_text,
            self.keys,
            self.on_outcome.clone(),
        );
    }
}

impl<M> TuiNode<M> for ConfirmationDialog<M>
where
    M: 'static,
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        self.dialog.measure(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.dialog.layout(area, ctx)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _ctx: &mut crate::RenderCtx<'_>) {
        Self::render(self, frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        self.dialog.event(event, ctx)
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(&mut self.dialog, dt, settings)
    }

    fn focus(&mut self, target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        self.dialog.focus(target, focused, ctx);
    }
}

fn confirmation_dialog<M>(
    title: &str,
    description: &str,
    yes_text: &str,
    no_text: &str,
    keys: ConfirmationDialogKeyBindings,
    on_outcome: Option<OutcomeHandler<M>>,
) -> Dialog<M>
where
    M: 'static,
{
    let mut yes = DialogAction::new(yes_text);
    let mut no = DialogAction::new(no_text);
    if let Some(hotkey) = keys.yes {
        yes = yes.hotkey(hotkey);
    }
    if let Some(hotkey) = keys.no {
        no = no.hotkey(hotkey);
    }
    if let Some(handler) = on_outcome.clone() {
        yes = yes.on_trigger(move || handler(ConfirmationDialogOutcome::Confirmed));
    }
    if let Some(handler) = on_outcome.clone() {
        no = no.on_trigger(move || handler(ConfirmationDialogOutcome::Cancelled));
    }
    let dialog = Dialog::new()
        .top_left(title)
        .actions([yes, no])
        .content([description]);
    match on_outcome {
        Some(handler) => {
            dialog.on_close(move |reason| handler(ConfirmationDialogOutcome::Closed(reason)))
        }
        None => dialog,
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::{Key, animation_settings};

    #[test]
    fn renders_actions_as_bottom_right_text_with_hotkeys() {
        let mut dialog = ConfirmationDialog::<()>::new("Delete item", "This cannot be undone.")
            .yes_text("Delete")
            .no_text("Keep")
            .yes_hotkey(KeySpec::plain('d'))
            .no_hotkey(KeySpec::plain('k'));
        let area = Rect::new(0, 0, 50, 7);
        let mut layout = LayoutCtx::new();
        dialog.layout(area, &mut layout);
        let mut terminal = Terminal::new(TestBackend::new(50, 7)).expect("terminal should build");

        terminal
            .draw(|frame| dialog.render(frame, area))
            .expect("confirmation dialog should render");

        let buffer = terminal.backend().buffer();
        let bottom = (0..50)
            .map(|x| buffer.cell((x, 6)).unwrap().symbol())
            .collect::<String>();
        assert!(bottom.contains("Delete (d) · Keep (k)"), "{bottom}");
        assert!(bottom.ends_with('╯'), "{bottom}");
        assert_eq!(layout.focus_targets().len(), 1);
        assert_eq!(layout.focus_targets()[0].id.as_str(), "dialog");
    }

    #[test]
    fn action_hotkeys_emit_distinct_outcomes() {
        let mut dialog = ConfirmationDialog::new("Continue?", "Choose an action")
            .yes_hotkey(KeySpec::plain('d'))
            .no_hotkey(KeySpec::plain('k'))
            .on_outcome(|outcome| outcome);

        let mut confirm_ctx = EventCtx::new(animation_settings());
        dialog.event(&TuiEvent::Key(Key::Char('d').into()), &mut confirm_ctx);
        let mut cancel_ctx = EventCtx::new(animation_settings());
        dialog.event(&TuiEvent::Key(Key::Char('k').into()), &mut cancel_ctx);

        assert_eq!(
            confirm_ctx.messages(),
            &[ConfirmationDialogOutcome::Confirmed]
        );
        assert_eq!(
            cancel_ctx.messages(),
            &[ConfirmationDialogOutcome::Cancelled]
        );
    }

    #[test]
    fn default_actions_use_ok_and_cancel_hotkeys() {
        let dialog = ConfirmationDialog::<()>::new("Continue?", "Choose an action");
        let mut terminal = Terminal::new(TestBackend::new(40, 5)).expect("terminal should build");

        terminal
            .draw(|frame| dialog.render(frame, frame.area()))
            .expect("confirmation dialog should render");

        let buffer = terminal.backend().buffer();
        let bottom = (0..40)
            .map(|x| buffer.cell((x, 4)).unwrap().symbol())
            .collect::<String>();
        assert!(bottom.contains("Ok (o) · Cancel (c)"), "{bottom}");
    }

    #[test]
    fn close_key_emits_closed_outcome() {
        let mut dialog =
            ConfirmationDialog::new("Continue?", "Choose an action").on_outcome(|outcome| outcome);
        let mut ctx = EventCtx::new(animation_settings());

        dialog.event(&TuiEvent::Key(Key::Char('x').into()), &mut ctx);

        assert_eq!(
            ctx.messages(),
            &[ConfirmationDialogOutcome::Closed(
                DialogCloseReason::CloseKey
            )]
        );
    }
}
