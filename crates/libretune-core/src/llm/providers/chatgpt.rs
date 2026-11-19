//! Responses API transport for ChatGPT plan usage.

use crate::llm::{
    provider::Provider,
    types::{ChatRequest, ChatResponse, FinishReason, LlmError, MessageRole, ToolCall, Usage},
};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};

pub const RESOURCE: &str = "https://api.openai.com/v1";
const NAMESPACE: &str = "libretune";
type Reasoning = HashMap<String, Vec<Value>>;

pub(crate) struct ChatGptProvider {
    client: reqwest::Client,
    token: String,
    model: String,
    // Encrypted continuation items live only for this assistant turn.
    reasoning: std::sync::Mutex<Reasoning>,
}

impl ChatGptProvider {
    pub(crate) fn new(token: String, model: String) -> Result<Self, LlmError> {
        if token.is_empty() || model.trim().is_empty() {
            return Err(LlmError::Config(
                "Connect ChatGPT and select a model in AI Assistant settings.".into(),
            ));
        }
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .connect_timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            client,
            token,
            model,
            reasoning: Default::default(),
        })
    }
}

fn request_body(req: &ChatRequest, model: &str, reasoning: &Reasoning) -> Value {
    let mut input = Vec::new();
    let mut calls = HashMap::<&str, VecDeque<&ToolCall>>::new();
    for message in &req.messages {
        match message.role {
            MessageRole::Tool => {
                if let Some(call) = message
                    .tool_name
                    .as_deref()
                    .and_then(|name| calls.get_mut(name))
                    .and_then(VecDeque::pop_front)
                {
                    if let Some(items) = reasoning.get(&call.id) {
                        input.extend(items.iter().cloned());
                    }
                    // Only read calls get results. Proposals are retained by
                    // the orchestrator outside the wire conversation.
                    input.push(
                        json!({"type":"function_call","namespace":NAMESPACE,"call_id":call.id,
                        "name":call.name,"arguments":call.arguments}),
                    );
                    input.push(json!({"type":"function_call_output","call_id":call.id,"output":message.content}));
                }
            }
            _ => {
                if !message.content.is_empty() {
                    let role = match message.role {
                        MessageRole::System => "developer",
                        MessageRole::User => "user",
                        _ => "assistant",
                    };
                    input.push(json!({"role":role,"content":message.content}));
                }
                calls.clear();
                for call in &message.tool_calls {
                    calls.entry(&call.name).or_default().push_back(call);
                }
            }
        }
    }

    let mut body = json!({"model":model,"input":input,"store":false,"stream":true,
        "include":["reasoning.encrypted_content"]});
    if !req.tools.is_empty() {
        let tools: Vec<_> = req
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "type":"function","name":tool.name,"description":tool.description,
                    "parameters":tool.parameters,"strict":false
                })
            })
            .collect();
        body["tools"] = json!([{"type":"namespace","name":NAMESPACE,
            "description":"LibreTune tuning assistant tools","tools":tools}]);
        // The orchestrator stages proposals and only executes the read calls.
        body["parallel_tool_calls"] = json!(false);
    }
    body
}

fn completed_response(
    response: &Value,
    reasoning: &mut Reasoning,
) -> Result<ChatResponse, LlmError> {
    let output = response["output"]
        .as_array()
        .ok_or_else(|| LlmError::Parse("missing Responses output".into()))?;
    let mut content = String::new();
    let mut tool_calls = Vec::new();
    let mut pending_reasoning = Vec::new();
    for item in output {
        match item["type"].as_str() {
            Some("message") => {
                for part in item["content"].as_array().into_iter().flatten() {
                    if let Some(text) = part["text"].as_str().or_else(|| part["refusal"].as_str()) {
                        content.push_str(text);
                    }
                }
            }
            Some("reasoning") if item["encrypted_content"].is_string() => {
                pending_reasoning.push(item.clone());
            }
            Some("function_call") => {
                #[derive(serde::Deserialize)]
                struct FunctionCall {
                    call_id: String,
                    name: String,
                    arguments: String,
                }
                let call: FunctionCall = serde_json::from_value(item.clone())?;
                if !pending_reasoning.is_empty() {
                    reasoning.insert(call.call_id.clone(), std::mem::take(&mut pending_reasoning));
                }
                tool_calls.push(ToolCall {
                    id: call.call_id,
                    name: call.name,
                    arguments: call.arguments,
                });
            }
            _ => {}
        }
    }
    let usage = &response["usage"];
    let usage = usage.is_object().then(|| Usage {
        prompt_tokens: usage["input_tokens"].as_u64().unwrap_or_default() as u32,
        completion_tokens: usage["output_tokens"].as_u64().unwrap_or_default() as u32,
        total_tokens: usage["total_tokens"].as_u64().unwrap_or_default() as u32,
    });
    let finish_reason = if tool_calls.is_empty() {
        FinishReason::Stop
    } else {
        FinishReason::ToolCalls
    };
    Ok(ChatResponse {
        content,
        tool_calls,
        finish_reason,
        usage,
    })
}

fn response_error(code: Option<&str>, status: reqwest::StatusCode) -> LlmError {
    match code {
        Some("subscription_sharing_usage_limit_exceeded" | "subscription_sharing_usage_unavailable") =>
            LlmError::RateLimit("ChatGPT plan usage is unavailable. Check your usage and app limits in ChatGPT settings.".into()),
        _ if status == reqwest::StatusCode::UNAUTHORIZED =>
            LlmError::Config("ChatGPT session rejected. Reconnect in AI Assistant settings.".into()),
        _ if status == reqwest::StatusCode::TOO_MANY_REQUESTS =>
            LlmError::RateLimit("ChatGPT request limit reached.".into()),
        _ => LlmError::ApiError(format!("ChatGPT Responses request failed ({status}, {}).",
            code.unwrap_or("unknown error"))),
    }
}

#[async_trait]
impl Provider for ChatGptProvider {
    fn name(&self) -> &str {
        "chatgpt"
    }

    async fn chat(&self, req: &ChatRequest) -> Result<ChatResponse, LlmError> {
        let body = {
            let reasoning = self
                .reasoning
                .lock()
                .map_err(|_| LlmError::Parse("ChatGPT turn context is unavailable.".into()))?;
            request_body(req, &self.model, &reasoning)
        };
        let mut response = self
            .client
            .post(format!("{RESOURCE}/responses"))
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            let error = response.json::<Value>().await.unwrap_or_default();
            return Err(response_error(error["error"]["code"].as_str(), status));
        }
        let mut pending = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            pending.extend_from_slice(&chunk);
            let mut consumed = 0;
            for line in pending.split_inclusive(|byte| *byte == b'\n') {
                if !line.ends_with(b"\n") {
                    break;
                }
                consumed += line.len();
                let Some(data) = line.strip_prefix(b"data:") else {
                    continue;
                };
                if data.trim_ascii() == b"[DONE]" {
                    continue;
                }
                let event: Value = serde_json::from_slice(data)?;
                match event["type"].as_str() {
                    Some("response.completed") => {
                        let mut reasoning = self.reasoning.lock().map_err(|_| {
                            LlmError::Parse("ChatGPT turn context is unavailable.".into())
                        })?;
                        return completed_response(&event["response"], &mut reasoning);
                    }
                    Some("response.failed" | "error") => {
                        let code = event["response"]["error"]["code"]
                            .as_str()
                            .or_else(|| event["code"].as_str());
                        return Err(response_error(code, status));
                    }
                    Some("response.incomplete") => {
                        return Err(LlmError::ApiError(
                            "ChatGPT response was incomplete.".into(),
                        ))
                    }
                    _ => {}
                }
            }
            pending.drain(..consumed);
        }
        Err(LlmError::Network(
            "ChatGPT stream ended before completion.".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::types::{Message, ToolDef};

    #[test]
    fn plan_request_replays_read_results_and_omits_unanswered_proposals() {
        let read = ToolCall {
            id: "call_read".into(),
            name: "list_tables".into(),
            arguments: "{}".into(),
        };
        let propose = ToolCall {
            id: "call_propose".into(),
            name: "propose_table_edit".into(),
            arguments: "{}".into(),
        };
        let mut request = ChatRequest::new(vec![
            Message::system("Review every proposal."),
            Message::user("List tables"),
            Message {
                tool_calls: vec![read, propose],
                ..Message::assistant("")
            },
            Message {
                role: MessageRole::Tool,
                content: "[\"veTable1\"]".into(),
                tool_calls: Vec::new(),
                tool_name: Some("list_tables".into()),
            },
        ]);
        request.max_tokens = Some(100);
        request.temperature = Some(0.3);
        request.tools.push(ToolDef {
            name: "list_tables".into(),
            description: "List tables".into(),
            parameters: json!({"type":"object"}),
        });
        let body = request_body(&request, "test-model", &Reasoning::new());
        assert_eq!(body["store"], false);
        assert_eq!(body["stream"], true);
        assert_eq!(body["tools"][0]["type"], "namespace");
        assert!(body.get("max_output_tokens").is_none());
        assert!(body.get("temperature").is_none());
        let input = body["input"].as_array().unwrap();
        assert_eq!(input.len(), 4);
        assert_eq!(input[0]["role"], "developer");
        assert_eq!(input[2]["call_id"], "call_read");
        assert_eq!(input[3]["call_id"], "call_read");
        assert_eq!(input[3]["type"], "function_call_output");
    }

    #[test]
    fn completed_response_returns_tools_and_usage() {
        let response = completed_response(&json!({
            "output": [
                {"type":"message","content":[{"type":"output_text","text":"Found a table."}]},
                {"type":"function_call","namespace":"libretune","call_id":"call_123","name":"read_table","arguments":"{}"}
            ],
            "usage": {"input_tokens":22,"output_tokens":11,"total_tokens":33}
        }), &mut Reasoning::new()).unwrap();
        assert_eq!(response.content, "Found a table.");
        assert_eq!(response.tool_calls[0].id, "call_123");
        assert_eq!(response.tool_calls[0].name, "read_table");
        assert_eq!(response.usage.unwrap().total_tokens, 33);
    }
}
