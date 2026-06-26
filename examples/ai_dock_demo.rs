use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::{env, path::PathBuf};

use futures::StreamExt;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use rig::agent::{MultiTurnStreamItem, Text as RigText};
use rig::client::CompletionClient;
use rig::completion::ToolDefinition;
use rig::providers::chatgpt;
use rig::schemars::JsonSchema;
use rig::streaming::{StreamedAssistantContent, StreamingPrompt};
use rig::tool::Tool;
use serde::{Deserialize, Serialize};

use tuicore::components::{AiDock, LlmEvent, ToolPolicy};
use tuicore::{
    Button, DialogBackdrop, DialogLayer, DockSpec, EventCtx, EventOutcome, EventRoute, FocusCtx,
    FocusId, FocusTarget, KeyEvent, LayoutCtx, LayoutResult, LifecycleCtx, TickResult, TuiEvent,
    TuiNode, theme,
};

#[derive(Debug)]
enum Msg {
    OpenAiDock,
    CloseAiDock,
}

struct DemoApp {
    dialog_layer: DialogLayer<BaseScreen, AiDock<Msg>>,
}

impl DemoApp {
    fn new() -> Self {
        let base = BaseScreen::new();

        // Construct runner for AiDock
        let runner = |prompt: String,
                      history: Vec<rig::message::Message>,
                      sender: mpsc::Sender<LlmEvent>,
                      request_id: u64,
                      _provider: String,
                      model: String| {
            thread::spawn(move || {
                let runtime = match tokio::runtime::Runtime::new() {
                    Ok(rt) => rt,
                    Err(err) => {
                        let _ = sender.send(LlmEvent::error(
                            request_id,
                            format!("Tokio runtime error: {}", err),
                        ));
                        return;
                    }
                };

                runtime.block_on(async {
                    let model = if model.is_empty() {
                        std::env::var("LLM_MODEL")
                            .unwrap_or_else(|_| "openai/gpt-4o".to_string())
                    } else {
                        if model.contains('/') {
                            model
                        } else {
                            format!("openai/{}", model)
                        }
                    };

                    let status_sender = sender.clone();
                    let token_dir = chatgpt_token_dir();
                    let client = match chatgpt::Client::builder()
                        .oauth()
                        .token_dir(token_dir)
                        .on_device_code(move |code| {
                            let _ = status_sender.send(LlmEvent::status(
                                request_id,
                                format!(
                                    "OAuth: Open {} and enter code {}",
                                    code.verification_uri, code.user_code
                                ),
                            ));
                        })
                        .build()
                    {
                        Ok(c) => c,
                        Err(err) => {
                            let _ = sender.send(LlmEvent::error(request_id, format!("Failed to build client: {}", err)));
                            return;
                        }
                    };

                    let _ = sender.send(LlmEvent::status(request_id, "Authorizing..."));
                    if let Err(err) = client.authorize().await {
                        let _ = sender.send(LlmEvent::error(request_id, format!("Auth failed: {}", err)));
                        return;
                    }

                    let model_name = model.strip_prefix("openai/").unwrap_or(&model).to_string();
                    let agent = client
                        .agent(&model_name)
                        .preamble("You are a helpful arithmetic assistant. Use the calculator tool for math operations. Summarize the tool result to the user.")
                        .tool(CalculatorTool {
                            sender: sender.clone(),
                            request_id,
                        })
                        .build();

                    let _ = sender.send(LlmEvent::status(request_id, format!("Calling {}...", model_name)));
                    let mut stream = agent
                        .stream_prompt(prompt)
                        .with_history(history)
                        .multi_turn(4)
                        .await;

                    let mut output = String::new();
                    let mut updated_history = Vec::new();
                    let mut usage = rig::completion::Usage::new();

                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(
                                RigText { text, .. },
                            ))) => {
                                output.push_str(&text);
                                let _ = sender.send(LlmEvent::chunk(request_id, text));
                            }
                            Ok(MultiTurnStreamItem::FinalResponse(final_response)) => {
                                usage = final_response
                                    .completion_calls()
                                    .last()
                                    .map(|call| call.usage)
                                    .unwrap_or_else(|| final_response.usage());
                                usage.total_tokens = usage.input_tokens.saturating_add(usage.output_tokens);
                                if let Some(hist) = final_response.history() {
                                    updated_history = hist.to_vec();
                                }
                            }
                            Err(err) => {
                                let _ = sender.send(LlmEvent::error(request_id, format!("Stream error: {}", err)));
                                return;
                            }
                            _ => {}
                        }
                    }

                    let _ = sender.send(LlmEvent::complete_with_usage(
                        request_id,
                        updated_history,
                        output,
                        usage,
                    ));
                });
            });
        };

        // Schema string helper for tools tab
        let calculator_schema = r#"{
  "type": "object",
  "required": ["op", "x", "y"],
  "properties": {
    "op": {
      "type": "string",
      "enum": ["add", "sub", "mul", "div"],
      "description": "The operation to perform"
    },
    "x": { "type": "number", "description": "First operand" },
    "y": { "type": "number", "description": "Second operand" }
  }
}"#;

        let ai_dock = AiDock::new(runner)
            .on_close(|| Msg::CloseAiDock)
            .tool(
                "calculator",
                "Perform simple mathematical calculations",
                calculator_schema,
            )
            .tool_policy("calculator", ToolPolicy::AskBeforeRun); // Test approval flow!

        let dialog_layer = DialogLayer::new(base, ai_dock)
            .docked(DockSpec::bottom(80).cross_percent(80))
            .backdrop(DialogBackdrop::dim().amount(0.55))
            .active(false);

        Self { dialog_layer }
    }

    fn handle_message(&mut self, msg: Msg, ctx: &mut EventCtx<Msg>) {
        match msg {
            Msg::OpenAiDock => {
                self.dialog_layer.set_active_with_context(true, ctx);
            }
            Msg::CloseAiDock => {
                self.dialog_layer.set_active_with_context(false, ctx);
            }
        }
    }
}

fn chatgpt_token_dir() -> PathBuf {
    if let Ok(dir) = env::var("TUICORE_CHATGPT_TOKEN_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(dir) = env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(dir).join("tuicore").join("rig-chatgpt");
    }
    if let Ok(dir) = env::var("APPDATA") {
        return PathBuf::from(dir).join("tuicore").join("rig-chatgpt");
    }
    if let Ok(home) = env::var("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("tuicore")
            .join("rig-chatgpt");
    }
    env::temp_dir().join("tuicore").join("rig-chatgpt")
}

impl TuiNode<Msg> for DemoApp {
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.dialog_layer.layout(area, ctx)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        self.dialog_layer.render(frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<Msg>) -> EventOutcome {
        if let TuiEvent::Key(KeyEvent { code, modifiers }) = event {
            if *code == tuicore::Key::Char('q')
                && modifiers.contains(tuicore::KeyModifiers::CONTROL)
            {
                ctx.request_quit();
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
        }
        self.dialog_layer.event(event, ctx)
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<Msg>,
    ) -> EventOutcome {
        self.dialog_layer.dispatch_event(route, event, ctx)
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<Msg>) {
        self.dialog_layer.dispatch_focus(target, focused, ctx);
    }

    fn tick(&mut self, dt: Duration, settings: tuicore::AnimationSettings) -> TickResult {
        self.dialog_layer.tick(dt, settings)
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        self.dialog_layer.init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        self.dialog_layer.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        self.dialog_layer.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        self.dialog_layer.destroy(ctx);
    }
}

struct BaseScreen {
    open_button: Button<Msg>,
    focused: bool,
}

impl BaseScreen {
    fn new() -> Self {
        let open_button = Button::new("Open AI Assistant Dock (a)")
            .hotkey("a")
            .on_press(|| Msg::OpenAiDock);

        Self {
            open_button,
            focused: false,
        }
    }
}

impl TuiNode<Msg> for BaseScreen {
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        // Draw centered button
        let button_w = 40;
        let button_h = 3;
        let button_x = area.x + area.width.saturating_sub(button_w) / 2;
        let button_y = area.y + area.height.saturating_sub(button_h) / 2;
        let button_area = Rect::new(button_x, button_y, button_w, button_h);

        let mut child_ctx = LayoutCtx::new();
        self.open_button.layout(button_area, &mut child_ctx);

        ctx.register_focusable(crate::FocusId::new("open-button"), button_area, true);
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        let theme = theme();
        // Clear background
        let paragraph = Paragraph::new(vec![
            Line::raw(""),
            Line::from(vec![Span::styled(
                "AiDock Component Demo",
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(theme.accent_fg()),
            )]),
            Line::raw(""),
            Line::raw("Press 'a' or click the button below to open the AI Assistant Dock."),
            Line::raw("Press Ctrl-q to quit the demo."),
        ])
        .alignment(ratatui::layout::Alignment::Center)
        .block(
            Block::bordered()
                .border_style(Style::default().fg(theme.border_fg()))
                .title(" Main Screen "),
        );

        frame.render_widget(paragraph, area);

        // Render button
        let button_w = 40;
        let button_h = 3;
        let button_x = area.x + area.width.saturating_sub(button_w) / 2;
        let button_y = area.y + area.height.saturating_sub(button_h) / 2;
        let button_area = Rect::new(button_x, button_y, button_w, button_h);

        self.open_button.render(frame, button_area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<Msg>) -> EventOutcome {
        self.open_button.event(event, ctx)
    }

    fn dispatch_event(
        &mut self,
        _route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<Msg>,
    ) -> EventOutcome {
        self.open_button.event(event, ctx)
    }

    fn dispatch_focus(&mut self, _target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<Msg>) {
        self.focused = focused;
        self.open_button.set_focused(focused, ctx.animation());
    }

    fn tick(&mut self, dt: Duration, settings: tuicore::AnimationSettings) -> TickResult {
        self.open_button.tick(dt, settings)
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        self.open_button.init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        self.open_button.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        self.open_button.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        self.open_button.destroy(ctx);
    }
}

// Rig Calculator Tool Definition
#[derive(Deserialize, Serialize, JsonSchema)]
struct CalculatorArgs {
    op: String,
    x: f64,
    y: f64,
}

struct CalculatorTool {
    sender: mpsc::Sender<LlmEvent>,
    request_id: u64,
}

impl Tool for CalculatorTool {
    const NAME: &'static str = "calculator";
    type Error = std::convert::Infallible;
    type Args = CalculatorArgs;
    type Output = f64;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Perform basic mathematical calculations: add, sub, mul, div.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "required": ["op", "x", "y"],
                "properties": {
                    "op": {
                        "type": "string",
                        "enum": ["add", "sub", "mul", "div"],
                        "description": "Operation to perform"
                    },
                    "x": { "type": "number", "description": "First number" },
                    "y": { "type": "number", "description": "Second number" }
                }
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let args_str = serde_json::to_string_pretty(&args).unwrap_or_default();

        let _ = self.sender.send(LlmEvent::approval(
            self.request_id,
            Self::NAME,
            args_str,
            tx,
        ));

        let approved = rx.await.unwrap_or(false);
        if !approved {
            let _ = self.sender.send(LlmEvent::status(
                self.request_id,
                "Tool call 'calculator' was denied by the user.",
            ));
            return Ok(0.0);
        }

        let _ = self.sender.send(LlmEvent::status(
            self.request_id,
            "Tool call 'calculator' approved. Executing...",
        ));

        let result = match args.op.as_str() {
            "add" => args.x + args.y,
            "sub" => args.x - args.y,
            "mul" => args.x * args.y,
            "div" => {
                if args.y == 0.0 {
                    0.0
                } else {
                    args.x / args.y
                }
            }
            _ => 0.0,
        };

        let _ = self.sender.send(LlmEvent::status(
            self.request_id,
            format!("Tool call 'calculator' completed. Result: {}", result),
        ));

        Ok(result)
    }
}

fn main() -> tuicore::Result<()> {
    dotenvy::dotenv().ok();
    tuicore::init();

    let app = DemoApp::new();
    tuicore::TreeApp::new(app)
        .on_message(|app, msg, ctx| app.handle_message(msg, ctx))
        .run()
}
