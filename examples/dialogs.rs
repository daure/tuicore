use ratatui::layout::Constraint;
use tuicore::{
    Button, ConfirmationDialog, ConfirmationDialogOutcome, DialogBackdrop, DialogLayer, EventCtx,
    FocusRequest, KeySpec, Paragraph, Split, TreeApp,
};

#[derive(Debug)]
enum Msg {
    OpenConfirmation,
    ConfirmationFinished(ConfirmationDialogOutcome),
}

type Base = Split<Button<Msg>, Paragraph>;
type Root = DialogLayer<Base, ConfirmationDialog<Msg>>;

fn handle_message(root: &mut Root, msg: Msg, ctx: &mut EventCtx<Msg>) {
    match msg {
        Msg::OpenConfirmation => root.set_active_with_context(true, ctx),
        Msg::ConfirmationFinished(outcome) => {
            let _ = root.layer_mut().take_outcomes();
            root.set_active_with_context(false, ctx);
            let status = match outcome {
                ConfirmationDialogOutcome::Confirmed => "Confirmed: draft deleted",
                ConfirmationDialogOutcome::Cancelled => "Cancelled: draft kept",
                ConfirmationDialogOutcome::Closed(_) => "Closed without choosing",
            };
            root.base_mut().second_mut().set_text(format!(
                "{status}\n\nFocus returned to the button that opened the dialog."
            ));
            ctx.request_redraw();
        }
    }
}

fn main() -> tuicore::Result<()> {
    tuicore::init();

    let base = Split::vertical(
        Button::new("Delete draft")
            .hotkey("d")
            .on_press(|| Msg::OpenConfirmation),
        Paragraph::new("Press Enter or d to open the confirmation.\n\nCtrl+Q exits."),
    )
    .constraints(Constraint::Length(1), Constraint::Fill(1))
    .gap(1);

    let confirmation = ConfirmationDialog::new(
        "Delete draft?",
        "This demonstrates a modal DialogLayer without custom portal plumbing.",
    )
    .yes_text("Delete")
    .no_text("Keep")
    .yes_hotkey(KeySpec::plain('y'))
    .no_hotkey(KeySpec::plain('n'))
    .on_outcome(Msg::ConfirmationFinished);

    let root = DialogLayer::new(base, confirmation)
        .active(false)
        .fit_content()
        .fit_content_max(60, 12)
        .backdrop(DialogBackdrop::dim().amount(0.55));

    TreeApp::new(root)
        .initial_focus(FocusRequest::FirstChild)
        .on_message(handle_message)
        .run()
}
