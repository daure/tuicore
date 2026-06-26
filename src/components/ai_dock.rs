use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use super::dialog_layer::DockChrome;
use crate::components::typography::{ellipsized_text_lines, wrapped_text_line_count};
use crate::components::{
    Chip, InputChrome, InputPanelChrome, Spinner, Tab, Tabs, TextInput, TextareaInput,
};
use crate::event::{Key, KeyEvent, KeyModifiers, TuiEvent};
use crate::{
    AnimationSettings, EventCtx, EventOutcome, FocusCtx, FocusTarget, KeySpec, LayoutCtx,
    LayoutResult, LifecycleCtx, ScrollAxes, ScrollDelta, ScrollOffset, ScrollSize, ScrollState,
    TickResult, TuiNode, line_width, paragraph_scroll, preset, theme,
};

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub is_user: bool,
    pub text: String,
    pub is_system: bool,
    pub tool_calls: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_prompt_suppresses_global_hotkeys_while_typing() {
        let input = Arc::new(Mutex::new(TextareaInput::new()));
        let mut body = ChatTabBody {
            messages: Arc::new(Mutex::new(Vec::new())),
            input: input.clone(),
            pending_approval: Arc::new(Mutex::new(None)),
            keybindings: Arc::new(Mutex::new(AiDockKeyBindings::default())),
            log_scroll: ScrollState::from_preset(ScrollAxes::Vertical, preset().scroll()),
            log_area: Rect::default(),
            waiting: Arc::new(Mutex::new(false)),
            spinner: Spinner::new(),
            last_content_height: 0,
            last_input_value: String::new(),
            log_pending_top_prefix: false,
            follow_log_bottom: true,
            shared_follow_log_bottom: Arc::new(Mutex::new(true)),
        };
        let mut event_ctx = EventCtx::default();
        body.event(&TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut event_ctx);

        let mut layout_ctx = LayoutCtx::new();
        body.layout(Rect::new(0, 0, 40, 10), &mut layout_ctx);

        let target = layout_ctx
            .focus_targets()
            .iter()
            .find(|target| target.id.as_str() == "textarea" && target.suppress_global_hotkeys)
            .expect("chat prompt focus target should be registered");
        assert!(target.suppress_global_hotkeys);
        assert!(target.focused_events_before_global_hotkeys);
    }

    #[test]
    fn model_inputs_suppress_global_hotkeys_while_typing() {
        let model_input = Arc::new(Mutex::new(TextInput::new()));
        model_input.lock().unwrap().set_insert_mode(true);
        let mut body = ModelTabBody {
            model_input,
            focused: false,
        };

        let mut layout_ctx = LayoutCtx::new();
        body.layout(Rect::new(0, 0, 40, 10), &mut layout_ctx);

        let target = layout_ctx
            .focus_targets()
            .iter()
            .find(|target| target.id.as_str() == "model-tab-model")
            .expect("model input focus target should be registered");
        assert!(target.suppress_global_hotkeys);
        assert!(target.focused_events_before_global_hotkeys);
    }

    #[test]
    fn typing_in_chat_prompt_does_not_switch_tabs_or_close() {
        let mut dock = AiDock::<()>::new(|_, _, _, _, _, _| {});
        let mut layout_ctx = LayoutCtx::new();
        dock.layout(Rect::new(0, 0, 80, 24), &mut layout_ctx);
        let target = layout_ctx
            .focus_targets()
            .iter()
            .find(|target| target.id.as_str() == "textarea")
            .cloned()
            .expect("chat prompt focus target should be registered");
        let mut focus_ctx = FocusCtx::default();
        dock.dispatch_focus(&target, true, &mut focus_ctx);

        let mut event_ctx = EventCtx::default();
        dock.dispatch_event(
            &crate::EventRoute::new(target.path.clone()),
            &TuiEvent::Key(KeyEvent::from(Key::Enter)),
            &mut event_ctx,
        );

        let mut layout_ctx = LayoutCtx::new();
        dock.layout(Rect::new(0, 0, 80, 24), &mut layout_ctx);
        let target = layout_ctx
            .focus_targets()
            .iter()
            .find(|target| target.id.as_str() == "textarea" && target.suppress_global_hotkeys)
            .cloned()
            .expect("chat prompt focus target should still be registered");
        assert!(target.suppress_global_hotkeys);
        assert!(target.focused_events_before_global_hotkeys);

        for key in [
            Key::Char('m'),
            Key::Char('a'),
            Key::Char('t'),
            Key::Char('x'),
        ] {
            let mut event_ctx = EventCtx::default();
            dock.dispatch_event(
                &crate::EventRoute::new(target.path.clone()),
                &TuiEvent::Key(KeyEvent::from(key)),
                &mut event_ctx,
            );
        }

        assert_eq!(dock.tabs.selected_index(), 0);
        assert_eq!(dock.input.lock().unwrap().current_value(), "matx");
    }

    #[test]
    fn ai_dock_tab_hotkeys_use_tabs_selection_effects() {
        let mut dock = AiDock::<()>::new(|_, _, _, _, _, _| {});
        let mut layout_ctx = LayoutCtx::new();
        dock.layout(Rect::new(0, 0, 80, 24), &mut layout_ctx);

        let mut event_ctx = EventCtx::default();
        let outcome = dock.event(
            &TuiEvent::Key(KeyEvent {
                code: Key::Char('t'),
                modifiers: KeyModifiers::ALT,
            }),
            &mut event_ctx,
        );

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(dock.tabs.selected_index(), 1);
        assert!(event_ctx.redraw_requested());
        assert!(event_ctx.layout_requested());
        assert!(event_ctx.focus_request().is_some());
    }

    #[test]
    fn ctrl_n_resets_ai_chat() {
        let mut dock = AiDock::<()>::new(|_, _, _, _, _, _| {});
        dock.messages.lock().unwrap().push(ChatMessage {
            is_user: true,
            text: "hello".to_string(),
            is_system: false,
            tool_calls: Vec::new(),
        });
        dock.input.lock().unwrap().set_value("draft");
        *dock.waiting.lock().unwrap() = true;

        let mut event_ctx = EventCtx::default();
        let outcome = dock.event(
            &TuiEvent::Key(KeyEvent {
                code: Key::Char('n'),
                modifiers: KeyModifiers::CONTROL,
            }),
            &mut event_ctx,
        );

        assert_eq!(outcome, EventOutcome::Handled);
        assert!(dock.messages.lock().unwrap().is_empty());
        assert_eq!(dock.input.lock().unwrap().current_value(), "");
        assert!(!*dock.waiting.lock().unwrap());
        assert!(event_ctx.redraw_requested());
        assert!(event_ctx.layout_requested());
    }

    #[test]
    fn routed_ctrl_n_reaches_focused_prompt_before_new_chat() {
        let mut dock = AiDock::<()>::new(|_, _, _, _, _, _| {});
        dock.messages.lock().unwrap().push(ChatMessage {
            is_user: true,
            text: "hello".to_string(),
            is_system: false,
            tool_calls: Vec::new(),
        });
        dock.input.lock().unwrap().set_value("one\ntwo");
        dock.input.lock().unwrap().set_insert_mode(true);

        let mut layout_ctx = LayoutCtx::new();
        dock.layout(Rect::new(0, 0, 80, 24), &mut layout_ctx);
        let target = layout_ctx
            .focus_targets()
            .iter()
            .find(|target| target.id.as_str() == "textarea")
            .cloned()
            .expect("chat prompt focus target should be registered");

        let mut event_ctx = EventCtx::default();
        let outcome = dock.dispatch_event(
            &crate::EventRoute::new(target.path),
            &TuiEvent::Key(KeyEvent {
                code: Key::Char('n'),
                modifiers: KeyModifiers::CONTROL,
            }),
            &mut event_ctx,
        );

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(event_ctx.propagation(), crate::Propagation::Stopped);
        assert_eq!(dock.messages.lock().unwrap().len(), 1);
        assert_eq!(dock.input.lock().unwrap().current_value(), "one\ntwo");
    }

    #[test]
    fn pending_approval_keys_stop_propagation() {
        let (tx, _rx) = tokio::sync::oneshot::channel();
        let pending_approval = Arc::new(Mutex::new(Some(PendingApproval {
            tool_name: "tool".to_string(),
            args: "{}".to_string(),
            response_tx: Some(tx),
        })));
        let mut body = ChatTabBody {
            messages: Arc::new(Mutex::new(Vec::new())),
            input: Arc::new(Mutex::new(TextareaInput::new())),
            pending_approval,
            keybindings: Arc::new(Mutex::new(AiDockKeyBindings::default())),
            log_scroll: ScrollState::from_preset(ScrollAxes::Vertical, preset().scroll()),
            log_area: Rect::default(),
            waiting: Arc::new(Mutex::new(false)),
            spinner: Spinner::new(),
            last_content_height: 0,
            last_input_value: String::new(),
            log_pending_top_prefix: false,
            follow_log_bottom: true,
            shared_follow_log_bottom: Arc::new(Mutex::new(true)),
        };

        let mut event_ctx = EventCtx::default();
        let outcome = body.event(
            &TuiEvent::Key(KeyEvent::from(Key::Char('a'))),
            &mut event_ctx,
        );

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(event_ctx.propagation(), crate::Propagation::Stopped);
        assert!(matches!(event_ctx.messages(), [AiDockMsg::ApprovePending]));
    }

    #[test]
    fn interrupt_denies_pending_approval() {
        let mut dock = AiDock::<()>::new(|_, _, _, _, _, _| {});
        *dock.waiting.lock().unwrap() = true;
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        *dock.pending_approval.lock().unwrap() = Some(PendingApproval {
            tool_name: "tool".to_string(),
            args: "{}".to_string(),
            response_tx: Some(tx),
        });

        let mut event_ctx = EventCtx::default();
        assert_eq!(
            dock.event(&TuiEvent::Key(KeyEvent::from(Key::Esc)), &mut event_ctx),
            EventOutcome::Handled
        );
        assert_eq!(
            dock.event(&TuiEvent::Key(KeyEvent::from(Key::Esc)), &mut event_ctx),
            EventOutcome::Handled
        );

        assert!(dock.pending_approval.lock().unwrap().is_none());
        assert_eq!(rx.try_recv(), Ok(false));
    }

    #[test]
    fn prompt_input_reports_compact_input_usage_percent() {
        let usage = rig::completion::Usage {
            input_tokens: 44_800,
            output_tokens: 362_472,
            total_tokens: 407_272,
            cached_input_tokens: 0,
            cache_creation_input_tokens: 0,
            tool_use_prompt_tokens: 0,
            reasoning_tokens: 0,
        };

        assert_eq!(
            input_usage_label(usage, "openai/gpt-5.3-codex-spark"),
            "407.3k (102%)"
        );
    }

    #[test]
    fn complete_with_usage_drives_display_usage_with_current_draft() {
        let mut dock = AiDock::<()>::new(|_, _, _, _, _, _| {});
        dock.request_id = 1;
        let usage = rig::completion::Usage {
            input_tokens: 44_800,
            output_tokens: 362_472,
            total_tokens: 407_272,
            cached_input_tokens: 0,
            cache_creation_input_tokens: 0,
            tool_use_prompt_tokens: 0,
            reasoning_tokens: 0,
        };

        assert!(
            dock.responses_tx
                .send(LlmEvent::complete_with_usage(1, Vec::new(), "ok", usage))
                .is_ok()
        );
        assert!(dock.poll_responses());
        dock.input.lock().unwrap().set_value("next draft words");

        let display_usage = dock.display_usage();

        assert_eq!(display_usage.input_tokens, 44_804);
        assert_eq!(display_usage.output_tokens, 362_472);
        assert_eq!(display_usage.total_tokens, 407_276);
    }
}

#[derive(Debug, Clone)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub schema: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolPolicy {
    Auto,
    AskBeforeRun,
}

const DEFAULT_TOOL_POLICY: ToolPolicy = ToolPolicy::AskBeforeRun;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiDockKeyBindings {
    chat_tab: Vec<KeySpec>,
    tools_tab: Vec<KeySpec>,
    model_tab: Vec<KeySpec>,
    new_chat: Vec<KeySpec>,
    approve: Vec<KeySpec>,
    deny: Vec<KeySpec>,
}

impl Default for AiDockKeyBindings {
    fn default() -> Self {
        Self {
            chat_tab: alt_alpha('c'),
            tools_tab: alt_alpha('t'),
            model_tab: alt_alpha('m'),
            new_chat: vec![KeySpec::key_with_modifiers(
                Key::Char('n'),
                KeyModifiers::CONTROL,
            )],
            approve: vec![KeySpec::plain('a'), KeySpec::plain('y')],
            deny: vec![KeySpec::plain('d'), KeySpec::plain('n')],
        }
    }
}

impl AiDockKeyBindings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_chat_tab(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.chat_tab = keys.into_iter().collect();
    }

    pub fn with_chat_tab(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_chat_tab(keys);
        self
    }

    pub fn set_tools_tab(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.tools_tab = keys.into_iter().collect();
    }

    pub fn with_tools_tab(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_tools_tab(keys);
        self
    }

    pub fn set_model_tab(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.model_tab = keys.into_iter().collect();
    }

    pub fn with_model_tab(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_model_tab(keys);
        self
    }

    pub fn set_new_chat(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.new_chat = keys.into_iter().collect();
    }

    pub fn with_new_chat(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_new_chat(keys);
        self
    }

    pub fn set_approve(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.approve = keys.into_iter().collect();
    }

    pub fn with_approve(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_approve(keys);
        self
    }

    pub fn set_deny(&mut self, keys: impl IntoIterator<Item = KeySpec>) {
        self.deny = keys.into_iter().collect();
    }

    pub fn with_deny(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.set_deny(keys);
        self
    }

    pub fn chat_tab_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_key_specs(&self.chat_tab, key.into())
    }

    pub fn tools_tab_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_key_specs(&self.tools_tab, key.into())
    }

    pub fn model_tab_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_key_specs(&self.model_tab, key.into())
    }

    pub fn new_chat_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_key_specs(&self.new_chat, key.into())
    }

    pub fn approve_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_key_specs(&self.approve, key.into())
    }

    pub fn deny_matches(&self, key: impl Into<KeyEvent>) -> bool {
        matches_key_specs(&self.deny, key.into())
    }

    pub fn approve_label(&self) -> String {
        key_specs_label(&self.approve)
    }

    pub fn deny_label(&self) -> String {
        key_specs_label(&self.deny)
    }
}

pub struct PendingApproval {
    pub tool_name: String,
    pub args: String,
    pub response_tx: Option<tokio::sync::oneshot::Sender<bool>>,
}

#[derive(Debug, Clone)]
pub enum AiDockMsg {
    SubmitPrompt(String),
    ApprovePending,
    DenyPending,
    Close,
}

pub struct AiDock<M = ()> {
    tabs: Tabs<AiDockMsg>,
    messages: Arc<Mutex<Vec<ChatMessage>>>,
    tools: Arc<Mutex<Vec<ToolInfo>>>,
    tool_policies: Arc<Mutex<HashMap<String, ToolPolicy>>>,
    settings: Arc<Mutex<(String, String)>>, // (provider, model)
    input: Arc<Mutex<TextareaInput<AiDockMsg>>>,
    model_input: Arc<Mutex<TextInput<AiDockMsg>>>,
    keybindings: Arc<Mutex<AiDockKeyBindings>>,
    responses_rx: mpsc::Receiver<LlmEvent>,
    responses_tx: mpsc::Sender<LlmEvent>,
    history: Vec<rig::message::Message>,
    input_usage: rig::completion::Usage,
    waiting: Arc<Mutex<bool>>,
    pending_approval: Arc<Mutex<Option<PendingApproval>>>,
    follow_log_bottom: Arc<Mutex<bool>>,
    request_id: u64,
    pending_interrupt_escape: bool,
    runner: Box<
        dyn Fn(String, Vec<rig::message::Message>, mpsc::Sender<LlmEvent>, u64, String, String)
            + Send
            + Sync
            + 'static,
    >,
    on_close: Option<Box<dyn Fn() -> M>>,
}

impl<M> AiDock<M>
where
    M: 'static,
{
    pub fn new<F>(runner: F) -> Self
    where
        F: Fn(String, Vec<rig::message::Message>, mpsc::Sender<LlmEvent>, u64, String, String)
            + Send
            + Sync
            + 'static,
    {
        let (tx, rx) = mpsc::channel();

        let messages = Arc::new(Mutex::new(Vec::new()));

        let input = Arc::new(Mutex::new(
            TextareaInput::new()
                .placeholder("Prompt...")
                .style(prompt_input_chrome(None, "openai", "openai/gpt-5.5"))
                .hotkey("p")
                .min_rows(1)
                .max_rows(8)
                .on_submit(move |prompt| AiDockMsg::SubmitPrompt(prompt)),
        ));

        let tools = Arc::new(Mutex::new(Vec::new()));
        let tool_policies = Arc::new(Mutex::new(HashMap::new()));

        let settings = Arc::new(Mutex::new((
            "openai".to_string(),
            "openai/gpt-5.5".to_string(),
        )));

        let model_input = Arc::new(Mutex::new(
            TextInput::new()
                .value("openai/gpt-5.5")
                .placeholder("OpenAI model name"),
        ));

        let pending_approval = Arc::new(Mutex::new(None));
        let waiting = Arc::new(Mutex::new(false));
        let follow_log_bottom = Arc::new(Mutex::new(true));
        let keybindings = Arc::new(Mutex::new(AiDockKeyBindings::default()));

        let chat_body = ChatTabBody {
            messages: messages.clone(),
            input: input.clone(),
            pending_approval: pending_approval.clone(),
            keybindings: keybindings.clone(),
            log_scroll: ScrollState::from_preset(ScrollAxes::Vertical, preset().scroll()),
            log_area: Rect::default(),
            waiting: waiting.clone(),
            spinner: Spinner::new(),
            last_content_height: 0,
            last_input_value: String::new(),
            log_pending_top_prefix: false,
            follow_log_bottom: true,
            shared_follow_log_bottom: follow_log_bottom.clone(),
        };

        let tools_body = ToolsTabBody {
            tools: tools.clone(),
            policies: tool_policies.clone(),
        };

        let model_body = ModelTabBody {
            model_input: model_input.clone(),
            focused: false,
        };

        // Construct using tuicore's default Tabs snackbar!
        let tabs = Tabs::new(vec![
            Tab::new("Chat", chat_body).hotkey("c"),
            Tab::new("Tools", tools_body).hotkey("t"),
            Tab::new("Model", model_body).hotkey("m"),
        ])
        .modal()
        .edge_borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
        .on_close(|_| AiDockMsg::Close);

        Self {
            tabs,
            messages,
            tools,
            tool_policies,
            settings,
            input,
            model_input,
            keybindings,
            responses_rx: rx,
            responses_tx: tx,
            history: Vec::new(),
            input_usage: rig::completion::Usage::new(),
            waiting,
            pending_approval,
            follow_log_bottom,
            request_id: 0,
            pending_interrupt_escape: false,
            runner: Box::new(runner),
            on_close: None,
        }
    }

    pub fn on_close(mut self, handler: impl Fn() -> M + 'static) -> Self {
        self.on_close = Some(Box::new(handler));
        self
    }

    pub fn keybindings(self, keybindings: AiDockKeyBindings) -> Self {
        self.set_keybindings(keybindings);
        self
    }

    pub fn set_keybindings(&self, keybindings: AiDockKeyBindings) {
        if let Ok(mut keys) = self.keybindings.lock() {
            *keys = keybindings;
        }
    }

    pub fn set_dock_edge_borders(&mut self, borders: Borders) {
        self.tabs.set_edge_borders(borders);
    }

    pub fn tool(
        self,
        name: impl Into<String>,
        description: impl Into<String>,
        schema: impl Into<String>,
    ) -> Self {
        if let Ok(mut t) = self.tools.lock() {
            t.push(ToolInfo {
                name: name.into(),
                description: description.into(),
                schema: schema.into(),
            });
        }
        self
    }

    pub fn tool_policy(self, name: &str, policy: ToolPolicy) -> Self {
        if let Ok(mut p) = self.tool_policies.lock() {
            p.insert(name.to_string(), policy);
        }
        self
    }

    pub fn submit_prompt(&mut self, prompt: String, ctx: &mut EventCtx<M>) {
        let prompt = prompt.trim().to_string();
        let is_waiting = { *self.waiting.lock().unwrap() };
        let has_pending = { self.pending_approval.lock().unwrap().is_some() };

        if prompt.is_empty() || is_waiting || has_pending {
            return;
        }

        self.request_id += 1;
        {
            *self.waiting.lock().unwrap() = true;
        }
        if let Ok(mut inp) = self.input.lock() {
            inp.set_value("");
            inp.set_insert_mode(true);
        }
        ctx.request_layout();
        if let Ok(mut follow) = self.follow_log_bottom.lock() {
            *follow = true;
        }

        if let Ok(mut msgs) = self.messages.lock() {
            msgs.push(ChatMessage {
                is_user: true,
                text: prompt.clone(),
                is_system: false,
                tool_calls: Vec::new(),
            });

            // Placeholder for streaming assistant response
            msgs.push(ChatMessage {
                is_user: false,
                text: String::new(),
                is_system: false,
                tool_calls: Vec::new(),
            });
        }

        let runner = self.runner.as_ref();
        let history = self.history.clone();
        let sender = self.responses_tx.clone();
        let request_id = self.request_id;

        let (provider, model) = {
            let s = self.settings.lock().unwrap();
            (s.0.clone(), s.1.clone())
        };

        (runner)(prompt, history, sender, request_id, provider, model);
        ctx.request_redraw();
    }

    fn poll_responses(&mut self) -> bool {
        let mut changed = false;
        while let Ok(event) = self.responses_rx.try_recv() {
            if event.request_id != self.request_id {
                continue;
            }

            match event.kind {
                LlmEventKind::Status(status) => {
                    if transient_status_message(&status) {
                        continue;
                    }
                    if let Ok(mut msgs) = self.messages.lock() {
                        msgs.push(ChatMessage {
                            is_user: false,
                            text: status,
                            is_system: true,
                            tool_calls: Vec::new(),
                        });
                    }
                }
                LlmEventKind::Chunk(chunk) => {
                    if let Ok(mut msgs) = self.messages.lock() {
                        if let Some(msg) = msgs.last_mut() {
                            if !msg.is_user && !msg.is_system {
                                msg.text.push_str(&chunk);
                            }
                        }
                    }
                }
                LlmEventKind::Complete {
                    history,
                    text,
                    usage,
                } => {
                    {
                        *self.waiting.lock().unwrap() = false;
                    }
                    self.history = history;
                    self.input_usage =
                        normalize_usage_for_display(usage.unwrap_or_default(), &text);
                    self.sync_input_panel_chrome();
                    if !text.is_empty() {
                        if let Ok(mut msgs) = self.messages.lock() {
                            if let Some(msg) = msgs.last_mut() {
                                if !msg.is_user && !msg.is_system {
                                    msg.text = text;
                                }
                            }
                        }
                    }
                }
                LlmEventKind::Error(err) => {
                    {
                        *self.waiting.lock().unwrap() = false;
                    }
                    if let Ok(mut msgs) = self.messages.lock() {
                        msgs.push(ChatMessage {
                            is_user: false,
                            text: format!("Error: {}", err),
                            is_system: true,
                            tool_calls: Vec::new(),
                        });
                    }
                }
                LlmEventKind::ApprovalRequired {
                    tool_name,
                    args,
                    response_tx,
                } => {
                    if let Ok(mut msgs) = self.messages.lock() {
                        add_tool_call_chip(&mut msgs, &tool_name);
                    }
                    if self.tool_policy_for(&tool_name) == ToolPolicy::Auto {
                        let _ = response_tx.send(true);
                    } else if let Ok(mut pending) = self.pending_approval.lock() {
                        *pending = Some(PendingApproval {
                            tool_name,
                            args,
                            response_tx: Some(response_tx),
                        });
                    }
                }
            }
            changed = true;
        }
        changed
    }

    fn tool_policy_for(&self, name: &str) -> ToolPolicy {
        self.tool_policies
            .lock()
            .ok()
            .and_then(|policies| policies.get(name).copied())
            .unwrap_or(DEFAULT_TOOL_POLICY)
    }

    fn approve_pending(&mut self) {
        self.resolve_pending_approval(true);
    }

    fn deny_pending(&mut self) {
        self.resolve_pending_approval(false);
    }

    fn resolve_pending_approval(&self, approved: bool) {
        if let Ok(mut pending) = self.pending_approval.lock() {
            if let Some(mut approval) = pending.take() {
                if let Some(tx) = approval.response_tx.take() {
                    let _ = tx.send(approved);
                }
            }
        }
    }

    fn handle_interrupt_key(&mut self, key: KeyEvent, ctx: &mut EventCtx<M>) -> bool {
        if key.code != Key::Esc {
            self.pending_interrupt_escape = false;
            return false;
        }

        let is_waiting = *self.waiting.lock().unwrap();
        if !is_waiting {
            self.pending_interrupt_escape = false;
            return false;
        }

        if self.pending_interrupt_escape {
            self.pending_interrupt_escape = false;
            self.interrupt_current_request();
            ctx.request_redraw();
            return true;
        }

        self.pending_interrupt_escape = true;
        true
    }

    fn interrupt_current_request(&mut self) {
        self.request_id = self.request_id.saturating_add(1);
        *self.waiting.lock().unwrap() = false;
        self.resolve_pending_approval(false);
        if let Ok(mut messages) = self.messages.lock() {
            if messages.last().is_some_and(|message| {
                !message.is_user && !message.is_system && message.text.is_empty()
            }) {
                messages.pop();
            }
            messages.push(ChatMessage {
                is_user: false,
                text: "Interrupted.".to_string(),
                is_system: true,
                tool_calls: Vec::new(),
            });
        }
    }

    fn handle_new_chat_key(&mut self, key: KeyEvent, ctx: &mut EventCtx<M>) -> bool {
        let matches = self
            .keybindings
            .lock()
            .map(|keys| keys.new_chat_matches(key))
            .unwrap_or(false);
        if !matches {
            return false;
        }

        self.reset_chat();
        ctx.request_layout();
        ctx.request_redraw();
        true
    }

    fn reset_chat(&mut self) {
        self.request_id = self.request_id.saturating_add(1);
        self.pending_interrupt_escape = false;
        self.history.clear();
        self.input_usage = rig::completion::Usage::new();

        if let Ok(mut messages) = self.messages.lock() {
            messages.clear();
        }
        if let Ok(mut input) = self.input.lock() {
            input.set_value("");
        }
        if let Ok(mut waiting) = self.waiting.lock() {
            *waiting = false;
        }
        self.resolve_pending_approval(false);
        if let Ok(mut follow) = self.follow_log_bottom.lock() {
            *follow = true;
        }

        self.sync_input_panel_chrome();
    }

    fn tab_index_for_key(&self, key: KeyEvent) -> Option<usize> {
        let keys = self.keybindings.lock().ok()?;
        if keys.chat_tab_matches(key) {
            Some(0)
        } else if keys.tools_tab_matches(key) {
            Some(1)
        } else if keys.model_tab_matches(key) {
            Some(2)
        } else {
            None
        }
    }

    fn select_tab_from_event(&mut self, index: usize, ctx: &mut EventCtx<M>) {
        let mut child_ctx = EventCtx::new(ctx.animation());
        self.tabs.select_index_from_event(index, &mut child_ctx);
        ctx.forward_non_message_effects_from(&mut child_ctx);
    }

    fn sync_input_panel_chrome(&self) {
        let (provider, model) = self
            .settings
            .lock()
            .map(|settings| (settings.0.clone(), settings.1.clone()))
            .unwrap_or_else(|_| (String::new(), String::new()));
        let display_usage = self.display_usage();
        let usage_label = input_usage_label(display_usage, &model);

        if let Ok(mut input) = self.input.lock() {
            let usage_label = display_usage.has_values().then_some(usage_label.as_str());
            input.set_style(prompt_input_chrome(usage_label, &provider, &model));
        }
    }

    fn display_usage(&self) -> rig::completion::Usage {
        let mut usage = self.input_usage;
        if usage.has_values() {
            let draft_tokens = self.current_draft_tokens();
            usage.input_tokens = usage.input_tokens.saturating_add(draft_tokens);
            usage.total_tokens = usage.total_tokens.saturating_add(draft_tokens);
            return usage;
        }

        let estimate = self.estimated_context_tokens();
        rig::completion::Usage {
            input_tokens: estimate,
            total_tokens: estimate,
            ..rig::completion::Usage::new()
        }
    }

    fn current_draft_tokens(&self) -> u64 {
        self.input
            .lock()
            .map(|input| estimated_text_tokens(input.current_value()))
            .unwrap_or(0)
    }

    fn estimated_context_tokens(&self) -> u64 {
        let mut text = String::new();
        if let Ok(messages) = self.messages.lock() {
            for message in messages.iter().filter(|message| !message.is_system) {
                text.push_str(&message.text);
                text.push('\n');
            }
        }
        if let Ok(input) = self.input.lock() {
            text.push_str(input.current_value());
        }
        estimated_text_tokens(&text)
    }
}

impl<M> DockChrome for AiDock<M>
where
    M: 'static,
{
    fn set_dock_edge_borders(&mut self, borders: Borders) {
        Self::set_dock_edge_borders(self, borders);
    }
}

fn prompt_input_chrome(usage_label: Option<&str>, provider: &str, model: &str) -> InputChrome {
    let chrome = InputChrome::panel_chrome(
        InputPanelChrome::new().bottom_left(format!(" {provider} · {model} ")),
    );
    if let Some(usage_label) = usage_label {
        chrome.top_right(format!(" {usage_label} "))
    } else {
        chrome
    }
}

fn input_usage_label(usage: rig::completion::Usage, model: &str) -> String {
    let limit = model_input_token_limit(model).unwrap_or(usage.total_tokens);
    let used_tokens = usage.total_tokens.max(usage.input_tokens);
    let percent = if limit == 0 {
        0
    } else {
        ((used_tokens as f64 / limit as f64) * 100.0).round() as u64
    };
    format!("{} ({}%)", compact_token_count(used_tokens), percent)
}

fn normalize_usage_for_display(
    mut usage: rig::completion::Usage,
    assistant_text: &str,
) -> rig::completion::Usage {
    if usage.has_values() && usage.total_tokens <= usage.input_tokens && !assistant_text.is_empty()
    {
        let estimated_output = estimated_text_tokens(assistant_text);
        usage.output_tokens = usage.output_tokens.max(estimated_output);
        usage.total_tokens = usage.input_tokens.saturating_add(usage.output_tokens);
    }
    usage
}

fn estimated_text_tokens(text: &str) -> u64 {
    let word_estimate = ((text.split_whitespace().count() as f64) * 1.33).ceil() as u64;
    let char_estimate = ((text.chars().count() as f64) / 4.0).ceil() as u64;
    word_estimate.max(char_estimate)
}

fn model_input_token_limit(model: &str) -> Option<u64> {
    match model.strip_prefix("openai/").unwrap_or(model) {
        "gpt-5.3-codex-spark" => Some(400_000),
        "gpt-5.5" => Some(400_000),
        _ => None,
    }
}

fn compact_token_count(tokens: u64) -> String {
    if tokens >= 1_000 {
        format!("{:.1}k", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

fn chat_content_size(messages: &[ChatMessage], width: u16) -> ScrollSize {
    if messages.is_empty() {
        return ScrollSize::new(width as usize, 0);
    }
    ScrollSize::new(
        width as usize,
        chat_content_line_count(messages, width) + CHAT_VERTICAL_PADDING * 2,
    )
}

fn chat_content_line_count(messages: &[ChatMessage], width: u16) -> usize {
    messages
        .iter()
        .enumerate()
        .map(|(index, msg)| {
            let text_width = message_text_width(msg, width);
            let separator = usize::from(index > 0);
            if msg.is_system {
                return wrapped_text_line_count(&msg.text, text_width, usize::MAX) + separator;
            }
            let tool_lines = tool_chip_lines(&msg.tool_calls, width).len()
                + usize::from(!msg.tool_calls.is_empty());
            let message_lines = if should_render_message_block(msg) {
                wrapped_text_line_count(&msg.text, text_width, usize::MAX) + 2
            } else {
                1
            };
            tool_lines + message_lines + separator
        })
        .sum()
}

fn message_text_width(message: &ChatMessage, width: u16) -> u16 {
    let prefix_width = if message.is_system {
        line_width(&Line::from(message_prefix(message))).min(u16::MAX as usize) as u16
    } else {
        MESSAGE_GUTTER_WIDTH
    };
    width.saturating_sub(prefix_width)
}

fn message_block_lines(message: &ChatMessage, width: u16) -> Vec<Line<'static>> {
    if !should_render_message_block(message) {
        return Vec::new();
    }

    let theme = theme();
    let text_width = message_text_width(message, width);
    let wrapped = ellipsized_text_lines(&message.text, text_width, usize::MAX, true);
    let bar_style = Style::default().fg(if message.is_user {
        theme.accent_fg()
    } else {
        theme.success_fg()
    });
    let bubble_width = width;
    let left_offset = 0;
    let message_bg = Style::default().bg(theme.backdrop_bg());
    let text_style = Style::default().fg(theme.text_fg()).bg(theme.backdrop_bg());

    let mut lines = Vec::with_capacity(wrapped.len().saturating_add(2));
    lines.push(message_padding_line(
        message.is_user,
        left_offset,
        bubble_width,
        bar_style,
    ));
    for line in wrapped {
        let line_width = markdown_display_width(&line).min(u16::MAX as usize) as u16;
        let fill_width = text_width.saturating_sub(line_width);
        lines.push(if message.is_user {
            let mut spans = vec![
                Span::styled(" ".repeat(left_offset as usize), message_bg),
                Span::styled(MESSAGE_PADDING, message_bg),
                Span::styled(" ".repeat(fill_width as usize), message_bg),
            ];
            spans.extend(markdown_spans(&line, text_style));
            spans.extend([
                Span::styled(MESSAGE_PADDING, message_bg),
                Span::styled(MESSAGE_BAR, bar_style),
            ]);
            Line::from(spans)
        } else {
            let mut spans = vec![
                Span::styled(MESSAGE_BAR, bar_style),
                Span::styled(MESSAGE_PADDING, message_bg),
            ];
            spans.extend(markdown_spans(&line, text_style));
            spans.extend([
                Span::styled(" ".repeat(fill_width as usize), message_bg),
                Span::styled(MESSAGE_PADDING, message_bg),
            ]);
            Line::from(spans)
        });
    }
    lines.push(message_padding_line(
        message.is_user,
        left_offset,
        bubble_width,
        bar_style,
    ));
    lines
}

fn should_render_message_block(message: &ChatMessage) -> bool {
    message.is_user || !message.text.is_empty()
}

fn spinner_line(glyph: &str) -> Line<'static> {
    Line::from(Span::styled(
        glyph.to_string(),
        Style::default().fg(theme().muted_fg()),
    ))
}

fn centered_empty_line(text: &str, width: u16) -> Line<'static> {
    let theme = theme();
    let text_width = line_width(&Line::from(text)).min(u16::MAX as usize) as u16;
    let left = width.saturating_sub(text_width) / 2;
    Line::from(vec![
        Span::raw(" ".repeat(left as usize)),
        Span::styled(text.to_string(), Style::default().fg(theme.muted_fg())),
    ])
}

fn markdown_display_width(line: &str) -> usize {
    line_width(&Line::from(strip_bold_markers(line)))
}

fn strip_bold_markers(line: &str) -> String {
    let mut stripped = String::new();
    let mut rest = line;
    while let Some(start) = rest.find("**") {
        stripped.push_str(&rest[..start]);
        rest = &rest[start + 2..];
        if let Some(end) = rest.find("**") {
            stripped.push_str(&rest[..end]);
            rest = &rest[end + 2..];
        } else {
            stripped.push_str("**");
            stripped.push_str(rest);
            return stripped;
        }
    }
    stripped.push_str(rest);
    stripped
}

fn markdown_spans(line: &str, style: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find("**") {
        if start > 0 {
            spans.push(Span::styled(rest[..start].to_string(), style));
        }
        rest = &rest[start + 2..];
        let Some(end) = rest.find("**") else {
            spans.push(Span::styled(format!("**{rest}"), style));
            return spans;
        };
        spans.push(Span::styled(
            rest[..end].to_string(),
            style.add_modifier(Modifier::BOLD),
        ));
        rest = &rest[end + 2..];
    }
    if !rest.is_empty() {
        spans.push(Span::styled(rest.to_string(), style));
    }
    spans
}

fn message_padding_line(
    is_user: bool,
    left_offset: u16,
    bubble_width: u16,
    bar_style: Style,
) -> Line<'static> {
    let theme = theme();
    if is_user {
        Line::from(vec![
            Span::styled(
                " ".repeat(left_offset as usize),
                Style::default().bg(theme.backdrop_bg()),
            ),
            Span::styled(
                " ".repeat(bubble_width.saturating_sub(1) as usize),
                Style::default().bg(theme.backdrop_bg()),
            ),
            Span::styled(MESSAGE_BAR, bar_style),
        ])
    } else {
        Line::from(vec![
            Span::styled(MESSAGE_BAR, bar_style),
            Span::styled(
                " ".repeat(bubble_width.saturating_sub(1) as usize),
                Style::default().bg(theme.backdrop_bg()),
            ),
        ])
    }
}

fn transient_status_message(status: &str) -> bool {
    status == "Authorizing..."
        || status.starts_with("Calling ")
        || status.starts_with("Tool call '")
}

fn add_tool_call_chip(messages: &mut Vec<ChatMessage>, tool_name: &str) {
    if let Some(message) = messages
        .iter_mut()
        .rev()
        .find(|message| !message.is_user && !message.is_system)
    {
        message.tool_calls.push(tool_name.to_string());
        return;
    }

    messages.push(ChatMessage {
        is_user: false,
        text: String::new(),
        is_system: false,
        tool_calls: vec![tool_name.to_string()],
    });
}

fn tool_chip_lines(tool_calls: &[String], width: u16) -> Vec<Line<'static>> {
    if tool_calls.is_empty() {
        return Vec::new();
    }

    let max_width = width as usize;
    let mut lines = Vec::new();
    let mut spans = Vec::new();
    let mut current_width = 0;

    for tool_name in tool_calls {
        let chip_line = Chip::new(format_tool_chip_label(tool_name)).line();
        let chip_width = line_width(&chip_line);
        let separator_width = usize::from(!spans.is_empty());
        if !spans.is_empty() && current_width + separator_width + chip_width > max_width {
            lines.push(Line::from(spans));
            spans = Vec::new();
            current_width = 0;
        }
        if !spans.is_empty() {
            spans.push(Span::raw(" "));
            current_width += 1;
        }
        spans.extend(chip_line.spans);
        current_width += chip_width;
    }

    if !spans.is_empty() {
        lines.push(Line::from(spans));
    }

    lines
}

fn format_tool_chip_label(tool_name: &str) -> String {
    tool_name
        .split(['_', '-'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            format!("{}{}", first.to_uppercase(), chars.as_str().to_lowercase())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn disabled_animation_settings() -> AnimationSettings {
    AnimationSettings {
        enabled: false,
        ..AnimationSettings::default()
    }
}

const MESSAGE_BAR: &str = "┃";
const MESSAGE_PADDING: &str = " ";
const MESSAGE_GUTTER_WIDTH: u16 = 3;
const CHAT_VERTICAL_PADDING: usize = 1;

fn message_prefix(message: &ChatMessage) -> &'static str {
    if message.is_system {
        "[System] "
    } else if message.is_user {
        "You: "
    } else {
        "AI: "
    }
}

fn alt_alpha(c: char) -> Vec<KeySpec> {
    vec![
        KeySpec::key_with_modifiers(Key::Char(c), KeyModifiers::ALT),
        KeySpec::key_with_modifiers(Key::Char(c), KeyModifiers::ALT | KeyModifiers::SHIFT),
    ]
}

fn matches_key_specs(keys: &[KeySpec], key: KeyEvent) -> bool {
    keys.iter().copied().any(|spec| spec.matches(key))
}

fn key_specs_label(keys: &[KeySpec]) -> String {
    keys.first()
        .map(|key| key.label())
        .unwrap_or_else(|| "—".to_string())
}

pub struct LlmEvent {
    pub request_id: u64,
    pub kind: LlmEventKind,
}

pub enum LlmEventKind {
    Status(String),
    Chunk(String),
    Complete {
        history: Vec<rig::message::Message>,
        text: String,
        usage: Option<rig::completion::Usage>,
    },
    Error(String),
    ApprovalRequired {
        tool_name: String,
        args: String,
        response_tx: tokio::sync::oneshot::Sender<bool>,
    },
}

impl LlmEvent {
    pub fn status(request_id: u64, text: impl Into<String>) -> Self {
        Self {
            request_id,
            kind: LlmEventKind::Status(text.into()),
        }
    }
    pub fn chunk(request_id: u64, text: impl Into<String>) -> Self {
        Self {
            request_id,
            kind: LlmEventKind::Chunk(text.into()),
        }
    }
    pub fn complete(
        request_id: u64,
        history: Vec<rig::message::Message>,
        text: impl Into<String>,
    ) -> Self {
        Self::complete_with_usage(request_id, history, text, None)
    }

    pub fn complete_with_usage(
        request_id: u64,
        history: Vec<rig::message::Message>,
        text: impl Into<String>,
        usage: impl Into<Option<rig::completion::Usage>>,
    ) -> Self {
        Self {
            request_id,
            kind: LlmEventKind::Complete {
                history,
                text: text.into(),
                usage: usage.into(),
            },
        }
    }
    pub fn error(request_id: u64, text: impl Into<String>) -> Self {
        Self {
            request_id,
            kind: LlmEventKind::Error(text.into()),
        }
    }
    pub fn approval(
        request_id: u64,
        tool_name: impl Into<String>,
        args: impl Into<String>,
        response_tx: tokio::sync::oneshot::Sender<bool>,
    ) -> Self {
        Self {
            request_id,
            kind: LlmEventKind::ApprovalRequired {
                tool_name: tool_name.into(),
                args: args.into(),
                response_tx,
            },
        }
    }
}

impl<M> TuiNode<M> for AiDock<M>
where
    M: 'static,
{
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.tabs.layout(area, ctx)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        self.tabs.render(frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        if let TuiEvent::Key(key) = event {
            if self.handle_new_chat_key(*key, ctx) {
                return EventOutcome::Handled;
            }
            if self.handle_interrupt_key(*key, ctx) {
                return EventOutcome::Handled;
            }
            if let Some(index) = self.tab_index_for_key(*key) {
                self.select_tab_from_event(index, ctx);
                return EventOutcome::Handled;
            }
        }

        let mut child_ctx = EventCtx::new(ctx.animation());
        let outcome = self.tabs.event(event, &mut child_ctx);

        for msg in child_ctx.drain_messages() {
            match msg {
                AiDockMsg::SubmitPrompt(prompt) => {
                    self.submit_prompt(prompt, ctx);
                }
                AiDockMsg::ApprovePending => {
                    self.approve_pending();
                    ctx.request_redraw();
                }
                AiDockMsg::DenyPending => {
                    self.deny_pending();
                    ctx.request_redraw();
                }
                AiDockMsg::Close => {
                    if let Some(ref handler) = self.on_close {
                        ctx.emit(handler());
                    }
                }
            }
        }

        ctx.forward_non_message_effects_from(&mut child_ctx);

        outcome
    }

    fn dispatch_event(
        &mut self,
        route: &crate::EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        if route.path.is_empty() {
            return self.event(event, ctx);
        }

        if let TuiEvent::Key(key) = event
            && self.handle_interrupt_key(*key, ctx)
        {
            return EventOutcome::Handled;
        }

        let mut child_ctx = EventCtx::new(ctx.animation());
        let outcome = self.tabs.dispatch_event(route, event, &mut child_ctx);

        for msg in child_ctx.drain_messages() {
            match msg {
                AiDockMsg::SubmitPrompt(prompt) => {
                    self.submit_prompt(prompt, ctx);
                }
                AiDockMsg::ApprovePending => {
                    self.approve_pending();
                    ctx.request_redraw();
                }
                AiDockMsg::DenyPending => {
                    self.deny_pending();
                    ctx.request_redraw();
                }
                AiDockMsg::Close => {
                    if let Some(ref handler) = self.on_close {
                        ctx.emit(handler());
                    }
                }
            }
        }

        ctx.forward_non_message_effects_from(&mut child_ctx);

        outcome.bubble(ctx, |ctx| self.event(event, ctx))
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        let mut child_ctx = FocusCtx::new(ctx.animation());
        self.tabs.dispatch_focus(target, focused, &mut child_ctx);
        if child_ctx.redraw_requested() {
            ctx.request_redraw();
        }
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        let changed = self.poll_responses();
        let tabs_tick = self.tabs.tick(dt, settings);

        // Settings Sync
        if let Ok(mut s) = self.settings.lock() {
            if let Ok(m_inp) = self.model_input.lock() {
                let model_name = m_inp.current_value().to_string();
                if s.1 != model_name {
                    s.1 = model_name;
                    drop(s);
                    self.sync_input_panel_chrome();
                }
            }
        }

        TickResult {
            changed: changed || tabs_tick.changed,
            active: { *self.waiting.lock().unwrap() } || changed || tabs_tick.active,
        }
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<M>) {
        let mut child_ctx = LifecycleCtx::default();
        self.tabs.init(&mut child_ctx);
        if child_ctx.redraw_requested() {
            ctx.request_redraw();
        }
        if child_ctx.layout_requested() {
            ctx.request_layout();
        }
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<M>) {
        let mut child_ctx = LifecycleCtx::default();
        self.tabs.mount(&mut child_ctx);
        if child_ctx.redraw_requested() {
            ctx.request_redraw();
        }
        if child_ctx.layout_requested() {
            ctx.request_layout();
        }
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<M>) {
        let mut child_ctx = LifecycleCtx::default();
        self.tabs.unmount(&mut child_ctx);
        if child_ctx.redraw_requested() {
            ctx.request_redraw();
        }
        if child_ctx.layout_requested() {
            ctx.request_layout();
        }
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<M>) {
        let mut child_ctx = LifecycleCtx::default();
        self.tabs.destroy(&mut child_ctx);
        if child_ctx.redraw_requested() {
            ctx.request_redraw();
        }
        if child_ctx.layout_requested() {
            ctx.request_layout();
        }
    }
}

// Bodies implementation
struct ChatTabBody {
    messages: Arc<Mutex<Vec<ChatMessage>>>,
    input: Arc<Mutex<TextareaInput<AiDockMsg>>>,
    pending_approval: Arc<Mutex<Option<PendingApproval>>>,
    keybindings: Arc<Mutex<AiDockKeyBindings>>,
    log_scroll: ScrollState,
    log_area: Rect,
    waiting: Arc<Mutex<bool>>,
    spinner: Spinner,
    last_content_height: usize,
    last_input_value: String,
    log_pending_top_prefix: bool,
    follow_log_bottom: bool,
    shared_follow_log_bottom: Arc<Mutex<bool>>,
}

impl ChatTabBody {
    fn handle_log_scroll_key(&mut self, key: KeyEvent, ctx: &mut EventCtx<AiDockMsg>) -> bool {
        let Ok(messages) = self.messages.lock() else {
            return false;
        };
        let content = chat_content_size(&messages, self.log_width());
        drop(messages);

        let geometry = self.log_scroll.geometry(self.log_area, content);
        let outcome = self.handle_explicit_log_scroll_key(key, geometry.viewport, geometry.content);
        if outcome.handled {
            self.follow_log_bottom = self.is_log_at_bottom(geometry.viewport, geometry.content);
            ctx.request_redraw();
            ctx.stop_propagation();
        }
        outcome.handled
    }

    fn handle_explicit_log_scroll_key(
        &mut self,
        key: KeyEvent,
        viewport: ScrollSize,
        content: ScrollSize,
    ) -> crate::ScrollOutcome {
        let page = viewport.height.saturating_sub(1).max(1) as isize;
        let settings = disabled_animation_settings();
        let bindings = crate::keybindings();
        if bindings.top_prefix_matches(key) {
            if self.log_pending_top_prefix {
                self.log_pending_top_prefix = false;
                self.follow_log_bottom = false;
                return self.log_scroll.scroll_to(
                    ScrollOffset::new(0, 0),
                    viewport,
                    content,
                    settings,
                );
            }
            self.log_pending_top_prefix = true;
            return crate::ScrollOutcome {
                handled: true,
                changed: false,
                active: false,
            };
        }

        self.log_pending_top_prefix = false;
        if bindings.line_up_matches(key) {
            self.follow_log_bottom = false;
            self.log_scroll
                .scroll_by(ScrollDelta::new(0, -1), viewport, content, settings)
        } else if bindings.line_down_matches(key) {
            let outcome =
                self.log_scroll
                    .scroll_by(ScrollDelta::new(0, 1), viewport, content, settings);
            self.follow_log_bottom = self.is_log_at_bottom(viewport, content);
            outcome
        } else if bindings.page_up_matches(key) {
            self.follow_log_bottom = false;
            self.log_scroll
                .scroll_by(ScrollDelta::new(0, -page), viewport, content, settings)
        } else if bindings.page_down_matches(key) {
            let outcome =
                self.log_scroll
                    .scroll_by(ScrollDelta::new(0, page), viewport, content, settings);
            self.follow_log_bottom = self.is_log_at_bottom(viewport, content);
            outcome
        } else if bindings.home_matches(key) {
            self.follow_log_bottom = false;
            self.log_scroll
                .scroll_to(ScrollOffset::new(0, 0), viewport, content, settings)
        } else if bindings.bottom_matches(key) || bindings.end_matches(key) {
            self.follow_log_bottom = true;
            self.log_scroll.scroll_to(
                ScrollOffset::new(0, content.height.saturating_sub(viewport.height)),
                viewport,
                content,
                settings,
            )
        } else {
            crate::ScrollOutcome::idle()
        }
    }

    fn is_log_at_bottom(&self, viewport: ScrollSize, content: ScrollSize) -> bool {
        self.log_scroll.offset().y >= content.height.saturating_sub(viewport.height)
    }

    fn log_width(&self) -> u16 {
        self.log_area.width.saturating_sub(1).max(1)
    }

    fn scroll_log_to_bottom(&mut self, content: ScrollSize) -> bool {
        let geometry = self.log_scroll.geometry(self.log_area, content);
        let target = content.height.saturating_sub(geometry.viewport.height);
        self.log_scroll
            .scroll_to(
                ScrollOffset::new(0, target),
                geometry.viewport,
                geometry.content,
                disabled_animation_settings(),
            )
            .changed
    }
}

impl TuiNode<AiDockMsg> for ChatTabBody {
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        let lines_count = if let Ok(inp) = self.input.lock() {
            inp.current_value().split('\n').count()
        } else {
            1
        };
        let input_height = (lines_count + 2).clamp(3, 10) as u16;

        let [log_area, input_outer_area, _input_padding_area] = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(input_height),
            Constraint::Length(1),
        ])
        .areas(area);
        self.log_area = log_area;

        if self.follow_log_bottom {
            let content = self
                .messages
                .lock()
                .map(|messages| chat_content_size(&messages, self.log_width()))
                .unwrap_or_else(|_| ScrollSize::new(self.log_width() as usize, 0));
            self.scroll_log_to_bottom(content);
        }

        if let Ok(mut inp) = self.input.lock() {
            inp.layout(input_outer_area, ctx);
        }

        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        let theme = theme();
        let lines_count = if let Ok(inp) = self.input.lock() {
            inp.current_value().split('\n').count()
        } else {
            1
        };
        let input_height = (lines_count + 2).clamp(3, 10) as u16;

        let [log_area, input_outer_area, _input_padding_area] = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(input_height),
            Constraint::Length(1),
        ])
        .areas(area);

        let messages = self.messages.lock().unwrap();
        let log_width = log_area.width.saturating_sub(1).max(1);
        let mut message_lines = Vec::new();
        if messages.is_empty() {
            let top = log_area.height.saturating_sub(1) / 2;
            message_lines.extend((0..top).map(|_| Line::raw("")));
            message_lines.push(centered_empty_line("How can I help?", log_width));
        }
        if !messages.is_empty() {
            message_lines.push(Line::raw(""));
        }
        for (index, msg) in messages.iter().enumerate() {
            if index > 0 {
                message_lines.push(Line::raw(""));
            }
            if msg.is_system {
                message_lines.push(Line::from(vec![
                    Span::styled("[System] ", Style::default().fg(theme.muted_fg())),
                    Span::styled(&msg.text, Style::default().fg(theme.muted_fg())),
                ]));
            } else {
                message_lines.extend(tool_chip_lines(&msg.tool_calls, log_width));
                if !msg.tool_calls.is_empty() {
                    message_lines.push(Line::raw(""));
                }

                if !msg.is_user
                    && msg.text.is_empty()
                    && self.waiting.lock().map(|waiting| *waiting).unwrap_or(false)
                {
                    message_lines.push(spinner_line(self.spinner.glyph()));
                }

                message_lines.extend(message_block_lines(msg, log_width));
            }
        }
        if !messages.is_empty() {
            message_lines.push(Line::raw(""));
        }

        let content = chat_content_size(&messages, log_width);
        let geometry = self.log_scroll.geometry(log_area, content);
        let scroll_offset = self
            .log_scroll
            .offset()
            .y
            .min(content.height.saturating_sub(geometry.viewport.height));

        let log_paragraph = Paragraph::new(Text::from(message_lines))
            .scroll(paragraph_scroll(ScrollOffset::new(0, scroll_offset)));
        frame.render_widget(log_paragraph, geometry.layout.viewport);
        let input_insert_mode = self
            .input
            .lock()
            .map(|input| input.insert_mode())
            .unwrap_or(false);
        self.log_scroll.render_scrollbars(
            frame,
            geometry.layout,
            geometry.content,
            !input_insert_mode,
        );

        if let Ok(inp) = self.input.lock() {
            inp.render(frame, input_outer_area);
        }

        if let Ok(ref approval_opt) = self.pending_approval.lock() {
            if let Some(ref approval) = **approval_opt {
                let approval_block = Block::bordered()
                    .border_style(Style::default().fg(theme.warning_fg()))
                    .title(Span::styled(
                        " Tool Call Approval Required ",
                        Style::default()
                            .fg(theme.warning_fg())
                            .add_modifier(Modifier::BOLD),
                    ));

                let (approve_label, deny_label) = self
                    .keybindings
                    .lock()
                    .map(|keys| (keys.approve_label(), keys.deny_label()))
                    .unwrap_or_else(|_| ("A".to_string(), "D".to_string()));

                let approval_text = vec![
                    Line::from(vec![
                        Span::raw("Tool: "),
                        Span::styled(
                            &approval.tool_name,
                            Style::default()
                                .fg(theme.accent_fg())
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(vec![
                        Span::raw("Arguments: "),
                        Span::styled(&approval.args, Style::default().fg(theme.text_fg())),
                    ]),
                    Line::raw(""),
                    Line::from(vec![
                        Span::styled(
                            format!("[{approve_label}] Approve  "),
                            Style::default()
                                .fg(theme.success_fg())
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("[{deny_label}] Deny"),
                            Style::default()
                                .fg(theme.error_fg())
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]),
                ];

                let popup_area = Rect::new(
                    area.x + area.width / 6,
                    area.y + area.height / 4,
                    (area.width * 2) / 3,
                    (area.height / 2).max(6).min(area.height),
                );
                frame.render_widget(ratatui::widgets::Clear, popup_area);
                frame.render_widget(
                    Paragraph::new(approval_text)
                        .block(approval_block)
                        .wrap(Wrap { trim: false }),
                    popup_area,
                );
            }
        }
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<AiDockMsg>) -> EventOutcome {
        if let Ok(ref approval_opt) = self.pending_approval.lock() {
            if approval_opt.is_some() {
                if let TuiEvent::Key(key) = event
                    && let Ok(keys) = self.keybindings.lock()
                {
                    if keys.approve_matches(*key) {
                        ctx.emit(AiDockMsg::ApprovePending);
                        ctx.stop_propagation();
                        return EventOutcome::Handled;
                    }
                    if keys.deny_matches(*key) {
                        ctx.emit(AiDockMsg::DenyPending);
                        ctx.stop_propagation();
                        return EventOutcome::Handled;
                    }
                }
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
        }

        let input_insert_mode = self
            .input
            .lock()
            .map(|input| input.insert_mode())
            .unwrap_or(false);
        if !input_insert_mode {
            if let TuiEvent::Key(key) = event {
                if self.handle_log_scroll_key(*key, ctx) {
                    return EventOutcome::Handled;
                }
                let bindings = crate::keybindings();
                if bindings.tabs().previous_matches(*key)
                    || bindings.tabs().next_matches(*key)
                    || bindings.tabs().close_matches(*key)
                {
                    return EventOutcome::Ignored;
                }
            }
        }

        let input_outcome = if let Ok(mut inp) = self.input.lock() {
            let before_lines = inp.current_value().split('\n').count();
            let before_value = inp.current_value().to_string();
            let outcome = inp.event(event, ctx);
            let after_lines = inp.current_value().split('\n').count();
            let changed = before_value != inp.current_value();
            Some((outcome, before_lines != after_lines, changed))
        } else {
            None
        };

        if let Some((outcome, line_count_changed, changed)) = input_outcome {
            if line_count_changed {
                ctx.request_layout();
            }
            if changed {
                self.last_input_value = String::new();
            }
            if outcome.handled() {
                return EventOutcome::Handled;
            }
        }
        EventOutcome::Ignored
    }

    fn dispatch_event(
        &mut self,
        _route: &crate::EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<AiDockMsg>,
    ) -> EventOutcome {
        self.event(event, ctx)
    }

    fn dispatch_focus(
        &mut self,
        _target: &FocusTarget,
        focused: bool,
        _ctx: &mut FocusCtx<AiDockMsg>,
    ) {
        if let Ok(mut inp) = self.input.lock() {
            inp.set_focused(focused);
        }
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        let (content, input_value) = {
            let content = self
                .messages
                .lock()
                .map(|messages| chat_content_size(&messages, self.log_width()))
                .unwrap_or_else(|_| ScrollSize::new(self.log_width() as usize, 0));
            let input_value = self
                .input
                .lock()
                .map(|input| input.current_value().to_string())
                .unwrap_or_default();
            (content, input_value)
        };
        let content_changed = content.height != self.last_content_height;
        self.last_content_height = content.height;
        self.last_input_value = input_value;
        if let Ok(mut follow) = self.shared_follow_log_bottom.lock()
            && *follow
        {
            self.follow_log_bottom = true;
            *follow = false;
        }
        let scrolled =
            content_changed && self.follow_log_bottom && self.scroll_log_to_bottom(content);

        let waiting = self.waiting.lock().map(|waiting| *waiting).unwrap_or(false);
        let spinner_tick = if waiting {
            crate::Animated::tick(&mut self.spinner, dt, settings)
        } else {
            TickResult::IDLE
        };

        TickResult {
            changed: scrolled || spinner_tick.changed,
            active: spinner_tick.active,
        }
    }
}

struct ToolsTabBody {
    tools: Arc<Mutex<Vec<ToolInfo>>>,
    policies: Arc<Mutex<HashMap<String, ToolPolicy>>>,
}

impl TuiNode<AiDockMsg> for ToolsTabBody {
    fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        let theme = theme();
        let tools = self.tools.lock().unwrap();
        let policies = self.policies.lock().unwrap();

        let mut tool_lines = Vec::new();

        if tools.is_empty() {
            tool_lines.push(Line::from(vec![Span::styled(
                "No tools registered.",
                Style::default().fg(theme.muted_fg()),
            )]));
        } else {
            for tool in tools.iter() {
                let policy = policies
                    .get(&tool.name)
                    .copied()
                    .unwrap_or(DEFAULT_TOOL_POLICY);
                let policy_str = match policy {
                    ToolPolicy::Auto => "Auto-run",
                    ToolPolicy::AskBeforeRun => "Requires approval",
                };

                tool_lines.push(Line::from(vec![
                    Span::styled(
                        format!("🔧 {} ", tool.name),
                        Style::default()
                            .fg(theme.accent_fg())
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("({})", policy_str),
                        Style::default().fg(theme.muted_fg()),
                    ),
                ]));
                tool_lines.push(Line::from(vec![
                    Span::raw("  Description: "),
                    Span::styled(&tool.description, Style::default().fg(theme.text_fg())),
                ]));
                tool_lines.push(Line::from(vec![Span::styled(
                    "  Parameters Schema:",
                    Style::default().fg(theme.muted_fg()),
                )]));
                for line in tool.schema.lines() {
                    tool_lines.push(Line::from(vec![Span::styled(
                        format!("    {}", line),
                        Style::default().fg(theme.muted_fg()),
                    )]));
                }
                tool_lines.push(Line::raw(""));
            }
        }

        let paragraph = Paragraph::new(Text::from(tool_lines)).wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    }
}

struct ModelTabBody {
    model_input: Arc<Mutex<TextInput<AiDockMsg>>>,
    focused: bool,
}

impl TuiNode<AiDockMsg> for ModelTabBody {
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        let [model_area] = Layout::vertical([Constraint::Length(3)]).areas(area);
        let model_insert_mode = self
            .model_input
            .lock()
            .map(|input| input.insert_mode())
            .unwrap_or(false);

        ctx.register_text_entry_focusable(
            crate::FocusId::new("model-tab-model"),
            model_area,
            true,
            model_insert_mode,
        );

        if let Ok(mut model) = self.model_input.lock() {
            let mut child_ctx = LayoutCtx::new();
            let inner_rect = Block::bordered().inner(model_area);
            model.layout(inner_rect, &mut child_ctx);
        }

        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        let theme = theme();
        let [model_area] = Layout::vertical([Constraint::Length(3)]).areas(area);

        let model_block = Block::bordered()
            .border_style(Style::default().fg(if self.focused {
                theme.accent_fg()
            } else {
                theme.border_fg()
            }))
            .title(Span::styled(
                " OpenAI Model Name ",
                Style::default().fg(theme.muted_fg()),
            ));
        frame.render_widget(model_block.clone(), model_area);
        if let Ok(model) = self.model_input.lock() {
            model.render(frame, model_block.inner(model_area));
        }
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<AiDockMsg>) -> EventOutcome {
        let current_insert_mode = self
            .model_input
            .lock()
            .map(|model| model.insert_mode())
            .unwrap_or(false);

        if !current_insert_mode {
            if let TuiEvent::Key(key) = event {
                let bindings = crate::keybindings();
                if bindings.tabs().previous_matches(*key)
                    || bindings.tabs().next_matches(*key)
                    || bindings.tabs().close_matches(*key)
                {
                    return EventOutcome::Ignored;
                }
            }
        }

        if let Ok(mut model) = self.model_input.lock() {
            let outcome = model.event(event, ctx);
            if outcome.handled() {
                return EventOutcome::Handled;
            }
        }

        EventOutcome::Ignored
    }

    fn dispatch_event(
        &mut self,
        _route: &crate::EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<AiDockMsg>,
    ) -> EventOutcome {
        self.event(event, ctx)
    }

    fn dispatch_focus(
        &mut self,
        _target: &FocusTarget,
        focused: bool,
        _ctx: &mut FocusCtx<AiDockMsg>,
    ) {
        self.focused = focused;
        if let Ok(mut model) = self.model_input.lock() {
            model.set_focused(focused);
        }
    }
}
