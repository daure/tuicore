use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::components::typography::wrapped_text_line_count;
use crate::components::{Tab, Tabs, TextInput, TextareaInput};
use crate::event::{Key, KeyEvent, KeyModifiers, TuiEvent};
use crate::{
    AnimationSettings, EventCtx, EventOutcome, FocusCtx, FocusTarget, KeySpec, LayoutCtx,
    LayoutResult, LifecycleCtx, ScrollOffset, TickResult, TuiNode, line_width, paragraph_scroll,
    theme,
};

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub is_user: bool,
    pub text: String,
    pub is_system: bool,
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
            settings: Arc::new(Mutex::new(("openai".into(), "model".into()))),
            keybindings: Arc::new(Mutex::new(AiDockKeyBindings::default())),
        };
        let mut event_ctx = EventCtx::default();
        body.event(&TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut event_ctx);

        let mut layout_ctx = LayoutCtx::new();
        body.layout(Rect::new(0, 0, 40, 10), &mut layout_ctx);

        let target = layout_ctx
            .focus_targets()
            .iter()
            .find(|target| target.id.as_str() == "ai-dock-input")
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
            .find(|target| target.id.as_str() == "ai-dock-input")
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
            .find(|target| target.id.as_str() == "ai-dock-input")
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
    approve: Vec<KeySpec>,
    deny: Vec<KeySpec>,
}

impl Default for AiDockKeyBindings {
    fn default() -> Self {
        Self {
            chat_tab: alt_alpha('c'),
            tools_tab: alt_alpha('t'),
            model_tab: alt_alpha('m'),
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
    waiting: Arc<Mutex<bool>>,
    pending_approval: Arc<Mutex<Option<PendingApproval>>>,
    request_id: u64,
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

        let messages = Arc::new(Mutex::new(vec![ChatMessage {
            is_user: false,
            text: "Welcome to AI Assistant! Ask me anything.".to_string(),
            is_system: true,
        }]));

        let input = Arc::new(Mutex::new(
            TextareaInput::new()
                .placeholder("Prompt...")
                .on_submit(move |prompt| AiDockMsg::SubmitPrompt(prompt)),
        ));

        let tools = Arc::new(Mutex::new(Vec::new()));
        let tool_policies = Arc::new(Mutex::new(HashMap::new()));

        let settings = Arc::new(Mutex::new((
            "openai".to_string(),
            "gpt-5.3-codex-spark".to_string(),
        )));

        let model_input = Arc::new(Mutex::new(
            TextInput::new()
                .value("gpt-5.3-codex-spark")
                .placeholder("OpenAI model name"),
        ));

        let pending_approval = Arc::new(Mutex::new(None));
        let waiting = Arc::new(Mutex::new(false));
        let keybindings = Arc::new(Mutex::new(AiDockKeyBindings::default()));

        let chat_body = ChatTabBody {
            messages: messages.clone(),
            input: input.clone(),
            pending_approval: pending_approval.clone(),
            settings: settings.clone(),
            keybindings: keybindings.clone(),
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
            waiting,
            pending_approval,
            request_id: 0,
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
        }
        ctx.request_layout();

        if let Ok(mut msgs) = self.messages.lock() {
            msgs.push(ChatMessage {
                is_user: true,
                text: prompt.clone(),
                is_system: false,
            });

            // Placeholder for streaming assistant response
            msgs.push(ChatMessage {
                is_user: false,
                text: String::new(),
                is_system: false,
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
                    if let Ok(mut msgs) = self.messages.lock() {
                        msgs.push(ChatMessage {
                            is_user: false,
                            text: status,
                            is_system: true,
                        });
                        msgs.push(ChatMessage {
                            is_user: false,
                            text: String::new(),
                            is_system: false,
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
                LlmEventKind::Complete { history, text } => {
                    {
                        *self.waiting.lock().unwrap() = false;
                    }
                    self.history = history;
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
                        });
                    }
                }
                LlmEventKind::ApprovalRequired {
                    tool_name,
                    args,
                    response_tx,
                } => {
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
        if let Ok(mut pending) = self.pending_approval.lock() {
            if let Some(mut approval) = pending.take() {
                if let Some(tx) = approval.response_tx.take() {
                    let _ = tx.send(true);
                }
            }
        }
    }

    fn deny_pending(&mut self) {
        if let Ok(mut pending) = self.pending_approval.lock() {
            if let Some(mut approval) = pending.take() {
                if let Some(tx) = approval.response_tx.take() {
                    let _ = tx.send(false);
                }
            }
        }
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
}

fn chat_content_height(messages: &[ChatMessage], width: u16) -> usize {
    messages
        .iter()
        .map(|msg| {
            let prefix_width = line_width(&Line::from(message_prefix(msg))).min(u16::MAX as usize);
            let text_width = width.saturating_sub(prefix_width.min(u16::MAX as usize) as u16);
            wrapped_text_line_count(&msg.text, text_width, usize::MAX) + 1
        })
        .sum()
}

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
        Self {
            request_id,
            kind: LlmEventKind::Complete {
                history,
                text: text.into(),
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
            if let Some(index) = self.tab_index_for_key(*key) {
                self.tabs.select_index(index);
                ctx.request_redraw();
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
                s.1 = m_inp.current_value().to_string();
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
    settings: Arc<Mutex<(String, String)>>,
    keybindings: Arc<Mutex<AiDockKeyBindings>>,
}

impl TuiNode<AiDockMsg> for ChatTabBody {
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        let lines_count = if let Ok(inp) = self.input.lock() {
            inp.current_value().split('\n').count()
        } else {
            1
        };
        let input_height = (lines_count + 2).clamp(3, 10) as u16;

        let [_log_area, input_outer_area] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(input_height)]).areas(area);

        let prompt_insert_mode = self
            .input
            .lock()
            .map(|input| input.insert_mode())
            .unwrap_or(false);
        ctx.register_focusable(crate::FocusId::new("ai-dock-input"), input_outer_area, true);
        ctx.set_focus_suppresses_global_hotkeys(
            crate::FocusId::new("ai-dock-input"),
            prompt_insert_mode,
        );
        ctx.set_focus_receives_events_before_global_hotkeys(
            crate::FocusId::new("ai-dock-input"),
            prompt_insert_mode,
        );

        if let Ok(mut inp) = self.input.lock() {
            let mut child_ctx = LayoutCtx::new();
            let inner_rect = Block::bordered().inner(input_outer_area);
            inp.layout(inner_rect, &mut child_ctx);
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

        let [log_area, input_outer_area] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(input_height)]).areas(area);

        let messages = self.messages.lock().unwrap();
        let mut message_lines = Vec::new();
        for msg in messages.iter() {
            if msg.is_system {
                message_lines.push(Line::from(vec![
                    Span::styled("[System] ", Style::default().fg(theme.muted_fg())),
                    Span::styled(&msg.text, Style::default().fg(theme.muted_fg())),
                ]));
            } else if msg.is_user {
                message_lines.push(Line::from(vec![
                    Span::styled(
                        "You: ",
                        Style::default()
                            .fg(theme.accent_fg())
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(&msg.text, Style::default().fg(theme.text_fg())),
                ]));
            } else {
                message_lines.push(Line::from(vec![
                    Span::styled(
                        "AI: ",
                        Style::default()
                            .fg(theme.success_fg())
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(&msg.text, Style::default().fg(theme.text_fg())),
                ]));
            }
            message_lines.push(Line::raw(""));
        }

        let total_lines = chat_content_height(&messages, log_area.width);
        let scroll_offset = total_lines.saturating_sub(log_area.height as usize);

        let log_paragraph = Paragraph::new(Text::from(message_lines))
            .wrap(Wrap { trim: false })
            .scroll(paragraph_scroll(ScrollOffset::new(0, scroll_offset)));
        frame.render_widget(log_paragraph, log_area);

        let settings_guard = self.settings.lock().unwrap();
        let (provider, model_name) = &*settings_guard;
        let bottom_label = format!(" {} · {} ", provider, model_name);

        let input_block = Block::bordered()
            .border_style(Style::default().fg(theme.border_fg()))
            .title_bottom(Span::styled(
                bottom_label,
                Style::default().fg(theme.muted_fg()),
            ));
        frame.render_widget(input_block.clone(), input_outer_area);

        if let Ok(inp) = self.input.lock() {
            inp.render(frame, input_block.inner(input_outer_area));
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
                        return EventOutcome::Handled;
                    }
                    if keys.deny_matches(*key) {
                        ctx.emit(AiDockMsg::DenyPending);
                        return EventOutcome::Handled;
                    }
                }
                return EventOutcome::Handled;
            }
        }

        if let Ok(mut inp) = self.input.lock() {
            if !inp.insert_mode() {
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
            let before_lines = inp.current_value().split('\n').count();
            let outcome = inp.event(event, ctx);
            let after_lines = inp.current_value().split('\n').count();
            if before_lines != after_lines {
                ctx.request_layout();
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

        ctx.register_focusable(crate::FocusId::new("model-tab-model"), model_area, true);
        ctx.set_focus_suppresses_global_hotkeys(
            crate::FocusId::new("model-tab-model"),
            model_insert_mode,
        );
        ctx.set_focus_receives_events_before_global_hotkeys(
            crate::FocusId::new("model-tab-model"),
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
