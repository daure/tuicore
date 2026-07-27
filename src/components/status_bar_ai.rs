use std::path::PathBuf;
use std::sync::mpsc;
use std::{env, thread};

use futures::StreamExt;
use rig::agent::{MultiTurnStreamItem, Text as RigText};
use rig::client::CompletionClient;
use rig::providers::chatgpt;
use rig::streaming::{StreamedAssistantContent, StreamingPrompt};

use super::LlmEvent;

pub(super) fn default_ai_runner(
    prompt: String,
    history: Vec<rig::message::Message>,
    sender: mpsc::Sender<LlmEvent>,
    request_id: u64,
    provider: String,
    model: String,
) {
    thread::spawn(move || {
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = sender.send(LlmEvent::error(
                    request_id,
                    format!("Tokio runtime error: {error}"),
                ));
                return;
            }
        };

        runtime.block_on(async move {
            if !provider.is_empty() && provider != "openai" {
                let _ = sender.send(LlmEvent::error(
                    request_id,
                    format!("Unsupported default AI provider: {provider}"),
                ));
                return;
            }

            let model = resolve_chatgpt_model(model);
            let status_sender = sender.clone();
            let token_dir = chatgpt_token_dir();
            let client = match chatgpt::Client::builder()
                .oauth()
                .token_dir(token_dir.clone())
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
                Ok(client) => client,
                Err(error) => {
                    let _ = sender.send(LlmEvent::error(
                        request_id,
                        format!("Failed to build ChatGPT client: {error}"),
                    ));
                    return;
                }
            };

            let _ = sender.send(LlmEvent::status(request_id, "Authorizing..."));
            if let Err(error) = client.authorize().await {
                let _ = sender.send(LlmEvent::error(
                    request_id,
                    format!("ChatGPT OAuth failed: {error}"),
                ));
                return;
            }

            let model_name = model.strip_prefix("openai/").unwrap_or(&model).to_string();
            let agent = client
                .agent(&model_name)
                .preamble("You are a concise assistant inside a terminal UI. Help with the current app workflow and keep answers practical.")
                .build();

            let _ = sender.send(LlmEvent::status(
                request_id,
                format!("Calling {model_name}..."),
            ));
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
                    Ok(MultiTurnStreamItem::StreamAssistantItem(
                        StreamedAssistantContent::Text(RigText { text, .. }),
                    )) => {
                        output.push_str(&text);
                        let _ = sender.send(LlmEvent::chunk(request_id, text));
                    }
                    Ok(MultiTurnStreamItem::FinalResponse(final_response)) => {
                        usage = final_response
                            .completion_calls()
                            .last()
                            .map(|call| call.usage)
                            .unwrap_or_else(|| final_response.usage());
                        usage.total_tokens =
                            usage.input_tokens.saturating_add(usage.output_tokens);
                        if let Some(history) = final_response.history() {
                            updated_history = history.to_vec();
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(LlmEvent::error(
                            request_id,
                            format!("Stream error: {error}"),
                        ));
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
}

fn resolve_chatgpt_model(model: String) -> String {
    if model.is_empty() {
        env::var("LLM_MODEL").unwrap_or_else(|_| "openai/gpt-5.5".to_string())
    } else if model.contains('/') {
        model
    } else {
        format!("openai/{model}")
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
