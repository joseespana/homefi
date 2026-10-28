//! OpenAI's two APIs: Responses (`POST /responses`, used for OpenAI itself:
//! its newest models only call tools there) and Chat Completions
//! (`POST /chat/completions`, spoken by OpenRouter, Mistral, Groq, xAI,
//! DeepSeek, Ollama and most custom servers). Both list models at `GET /models`.

use serde_json::{json, Value};

use super::chat::{arguments, send, ChatReply, Message, Native, Tool, ToolCall, MAX_TOKENS};
use super::{http, AiConfig, AiError, AiErrorKind, AiProvider, ModelInfo, CHAT_TIMEOUT, LIST_TIMEOUT};

fn bearer(config: &AiConfig, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    match config.key() {
        Some(key) => builder.bearer_auth(key),
        None => builder,
    }
}

// ------------------------------------------------------------------ Responses

pub(crate) fn responses_body(config: &AiConfig, system: &str, messages: &[Message], tools: &[Tool]) -> Value {
    let mut input: Vec<Value> = Vec::new();
    for m in messages {
        match m {
            Message::User { text } => input.push(json!({ "role": "user", "content": text })),
            Message::Assistant { text, tool_calls, native } => {
                match Native::for_config(native, config).and_then(Value::as_array) {
                    // reasoning items must travel back with the calls they led to
                    Some(items) => input.extend(items.iter().cloned()),
                    None => {
                        if !text.is_empty() {
                            input.push(json!({ "role": "assistant", "content": text }));
                        }
                        for c in tool_calls {
                            input.push(json!({ "type": "function_call", "call_id": c.id, "name": c.name, "arguments": c.arguments.to_string() }));
                        }
                    }
                }
            }
            Message::Tool { results } => {
                for r in results {
                    input.push(json!({ "type": "function_call_output", "call_id": r.call_id, "output": r.content }));
                }
            }
        }
    }
    let mut body = json!({
        "model": config.model.trim(),
        "instructions": system,
        "input": input,
        "max_output_tokens": MAX_TOKENS,
        // nothing kept on OpenAI's side; the reasoning comes back encrypted so it can be replayed
        "store": false,
        "include": ["reasoning.encrypted_content"],
    });
    if !tools.is_empty() {
        body["tools"] = tools
            .iter()
            .map(|t| json!({ "type": "function", "name": t.name, "description": t.description, "parameters": t.parameters }))
            .collect();
    }
    body
}

pub(crate) fn parse_responses(config: &AiConfig, answer: Value) -> Result<ChatReply, AiError> {
    let Some(items) = answer.get("output").and_then(Value::as_array) else {
        return Err(AiError::new(config, AiErrorKind::BadAnswer, "no output"));
    };
    let mut text = Vec::new();
    let mut tool_calls = Vec::new();
    for item in items {
        match item["type"].as_str() {
            Some("message") => {
                for part in item["content"].as_array().into_iter().flatten() {
                    if part["type"] == "output_text" {
                        text.extend(part["text"].as_str().map(str::to_owned));
                    }
                }
            }
            Some("function_call") => tool_calls.push(ToolCall {
                id: item["call_id"].as_str().or(item["id"].as_str()).unwrap_or_default().to_owned(),
                name: item["name"].as_str().unwrap_or_default().to_owned(),
                arguments: arguments(item.get("arguments")),
            }),
            _ => {}
        }
    }
    Ok(ChatReply {
        text: text.join("\n").trim().to_owned(),
        tool_calls,
        native: Some(Native::new(config, Value::Array(items.clone()))),
    })
}

pub(crate) async fn responses(
    config: &AiConfig,
    system: &str,
    messages: &[Message],
    tools: &[Tool],
) -> Result<ChatReply, AiError> {
    let url = format!("{}/responses", config.base());
    let req =
        bearer(config, http().post(url)).timeout(CHAT_TIMEOUT).json(&responses_body(config, system, messages, tools));
    parse_responses(config, send(config, req, false).await?)
}

// ----------------------------------------------------------- Chat Completions

pub(crate) fn chat_body(config: &AiConfig, system: &str, messages: &[Message], tools: &[Tool]) -> Value {
    let mut out = vec![json!({ "role": "system", "content": system })];
    for m in messages {
        match m {
            Message::User { text } => out.push(json!({ "role": "user", "content": text })),
            Message::Assistant { text, tool_calls, native } => {
                let content =
                    if text.is_empty() && !tool_calls.is_empty() { Value::Null } else { Value::String(text.clone()) };
                let mut msg = json!({ "role": "assistant", "content": content });
                if !tool_calls.is_empty() {
                    msg["tool_calls"] = tool_calls
                        .iter()
                        .map(|c| json!({ "id": c.id, "type": "function", "function": { "name": c.name, "arguments": c.arguments.to_string() } }))
                        .collect();
                }
                // DeepSeek wants its reasoning back while tools are in play; OpenRouter its details
                if let Some(raw) = Native::for_config(native, config) {
                    for field in ["reasoning_content", "reasoning_details"] {
                        if let Some(v) = raw.get(field).filter(|v| !v.is_null()) {
                            msg[field] = v.clone();
                        }
                    }
                }
                out.push(msg);
            }
            Message::Tool { results } => {
                for r in results {
                    let mut msg = json!({ "role": "tool", "tool_call_id": r.call_id, "content": r.content });
                    if config.provider == AiProvider::Mistral {
                        msg["name"] = json!(r.name);
                    }
                    out.push(msg);
                }
            }
        }
    }
    let mut body = json!({ "model": config.model.trim(), "messages": out, "max_tokens": MAX_TOKENS });
    if !tools.is_empty() {
        body["tools"] = tools
            .iter()
            .map(|t| json!({ "type": "function", "function": { "name": t.name, "description": t.description, "parameters": t.parameters } }))
            .collect();
    }
    body
}

pub(crate) fn parse_chat(config: &AiConfig, answer: Value) -> Result<ChatReply, AiError> {
    let Some(msg) = answer.pointer("/choices/0/message") else {
        return Err(AiError::new(config, AiErrorKind::BadAnswer, "no choices"));
    };
    let text = match &msg["content"] {
        Value::String(s) => s.clone(),
        // some (Mistral) answer a list of chunks
        Value::Array(parts) => parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join(""),
        _ => String::new(),
    };
    let tool_calls = msg["tool_calls"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(i, c)| ToolCall {
            id: c["id"].as_str().filter(|id| !id.is_empty()).map(str::to_owned).unwrap_or_else(|| format!("call_{i}")),
            name: c.pointer("/function/name").and_then(Value::as_str).unwrap_or_default().to_owned(),
            arguments: arguments(c.pointer("/function/arguments")),
        })
        .collect();
    Ok(ChatReply { text: text.trim().to_owned(), tool_calls, native: Some(Native::new(config, msg.clone())) })
}

pub(crate) async fn chat_completions(
    config: &AiConfig,
    system: &str,
    messages: &[Message],
    tools: &[Tool],
) -> Result<ChatReply, AiError> {
    let url = format!("{}/chat/completions", config.base());
    let req = bearer(config, http().post(url)).timeout(CHAT_TIMEOUT).json(&chat_body(config, system, messages, tools));
    parse_chat(config, send(config, req, false).await?)
}

// --------------------------------------------------------------------- models

pub(crate) async fn list_models(config: &AiConfig) -> Result<Vec<ModelInfo>, AiError> {
    let req = bearer(config, http().get(format!("{}/models", config.base()))).timeout(LIST_TIMEOUT);
    let page = send(config, req, true).await?;
    let mut models = Vec::new();
    for m in page["data"].as_array().into_iter().flatten() {
        let Some(id) = m["id"].as_str() else { continue };
        // the conversation needs tools: drop the models that say they can't call them
        let tools_listed = m["supported_parameters"].as_array(); // OpenRouter
        if tools_listed.is_some_and(|p| !p.iter().any(|v| v == "tools")) {
            continue;
        }
        if m.pointer("/capabilities/function_calling") == Some(&Value::Bool(false)) {
            continue; // Mistral
        }
        if m["active"] == Value::Bool(false) {
            continue; // Groq
        }
        let name = m["name"].as_str().or(m["display_name"].as_str()).filter(|n| !n.is_empty()).unwrap_or(id);
        models.push(ModelInfo::new(id, name));
    }
    Ok(models)
}

/// OpenRouter lists models without a key, so the key is checked on `GET /key`.
pub(crate) async fn check_openrouter_key(config: &AiConfig) -> crate::Result<()> {
    let req = bearer(config, http().get(format!("{}/key", config.base()))).timeout(LIST_TIMEOUT);
    send(config, req, false).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::chat::ToolResult;
    use wiremock::matchers::{body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn cfg(p: AiProvider, base: &str) -> AiConfig {
        AiConfig::new(p, "m", Some("sk-test".into())).with_base_url(base)
    }

    fn history() -> Vec<Message> {
        vec![
            Message::User { text: "hola".into() },
            Message::Assistant {
                text: String::new(),
                tool_calls: vec![ToolCall { id: "call_1".into(), name: "f".into(), arguments: json!({"q": "x"}) }],
                native: None,
            },
            Message::Tool {
                results: vec![ToolResult {
                    call_id: "call_1".into(),
                    name: "f".into(),
                    content: "{\"ok\":true}".into(),
                    is_error: false,
                }],
            },
        ]
    }

    fn tool() -> Tool {
        Tool { name: "f".into(), description: "d".into(), parameters: json!({"type": "object"}) }
    }

    #[test]
    fn responses_requests_keep_nothing_on_the_server() {
        let b = responses_body(&cfg(AiProvider::OpenAi, "http://x"), "sys", &history(), &[tool()]);
        assert_eq!(b["instructions"], "sys");
        assert_eq!(b["store"], false);
        assert_eq!(b["tools"][0]["type"], "function");
        assert_eq!(b["tools"][0]["name"], "f");
        let input = b["input"].as_array().unwrap();
        assert_eq!(input[1]["type"], "function_call");
        assert_eq!(input[1]["arguments"], "{\"q\":\"x\"}");
        assert_eq!(input[2]["type"], "function_call_output");
        assert_eq!(input[2]["call_id"], "call_1");
    }

    #[test]
    fn chat_requests_use_the_classic_shape() {
        let b = chat_body(&cfg(AiProvider::Mistral, "http://x"), "sys", &history(), &[tool()]);
        let msgs = b["messages"].as_array().unwrap();
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[2]["content"], Value::Null);
        assert_eq!(msgs[2]["tool_calls"][0]["function"]["arguments"], "{\"q\":\"x\"}");
        assert_eq!(msgs[3]["role"], "tool");
        assert_eq!(msgs[3]["name"], "f");
        assert_eq!(b["tools"][0]["function"]["name"], "f");
        let groq = chat_body(&cfg(AiProvider::Groq, "http://x"), "sys", &history(), &[]);
        assert!(groq["messages"][3].get("name").is_none());
        assert!(groq.get("tools").is_none());
    }

    #[tokio::test]
    async fn responses_reads_calls_and_replays_reasoning() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/responses"))
            .and(header("authorization", "Bearer sk-test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "resp_1", "object": "response", "status": "completed",
                "output": [
                    {"id": "rs_1", "type": "reasoning", "summary": [], "encrypted_content": "gAAA"},
                    {"id": "msg_1", "type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "Buscando…", "annotations": []}]},
                    {"id": "fc_1", "type": "function_call", "call_id": "call_9", "name": "search_headphones", "arguments": "{\"query\":\"HD 600\"}", "status": "completed"}
                ]
            })))
            .mount(&server)
            .await;
        let c = cfg(AiProvider::OpenAi, &server.uri());
        let reply = responses(&c, "s", &[Message::User { text: "hi".into() }], &[tool()]).await.unwrap();
        assert_eq!(reply.text, "Buscando…");
        assert_eq!(
            reply.tool_calls,
            vec![ToolCall {
                id: "call_9".into(),
                name: "search_headphones".into(),
                arguments: json!({"query": "HD 600"})
            }]
        );
        let again = responses_body(&c, "s", &[reply.into_message()], &[]);
        assert_eq!(again["input"][0]["type"], "reasoning");
        assert_eq!(again["input"][0]["encrypted_content"], "gAAA");
    }

    #[tokio::test]
    async fn chat_completions_work_without_a_key_for_ollama() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .and(body_partial_json(json!({"model": "qwen3:8b"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "chatcmpl-1", "object": "chat.completion",
                "choices": [{"index": 0, "finish_reason": "tool_calls", "message": {
                    "role": "assistant", "content": "", "reasoning_content": "think",
                    "tool_calls": [{"id": "", "type": "function", "function": {"name": "f", "arguments": {"q": 1}}}]
                }}]
            })))
            .mount(&server)
            .await;
        let c = AiConfig::new(AiProvider::Ollama, "qwen3:8b", None).with_base_url(server.uri());
        let reply = chat_completions(&c, "s", &[Message::User { text: "hi".into() }], &[tool()]).await.unwrap();
        assert_eq!(reply.tool_calls[0].id, "call_0");
        assert_eq!(reply.tool_calls[0].arguments, json!({"q": 1}));
        // reasoning goes back to the same provider
        let again = chat_body(&c, "s", &[reply.into_message()], &[]);
        assert_eq!(again["messages"][1]["reasoning_content"], "think");
    }

    #[tokio::test]
    async fn models_drop_those_that_cannot_call_tools() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": [
                {"id": "anthropic/claude-x", "name": "Anthropic: Claude X", "supported_parameters": ["tools", "max_tokens"]},
                {"id": "some/plain", "name": "Plain", "supported_parameters": ["max_tokens"]},
                {"id": "text-embedding-3-small"}
            ]})))
            .mount(&server)
            .await;
        let c = cfg(AiProvider::OpenRouter, &server.uri());
        let models = crate::ai::list_models(&c).await.unwrap();
        // the only one, so also the recommended pick
        let expected = ModelInfo { recommended: true, ..ModelInfo::new("anthropic/claude-x", "Anthropic: Claude X") };
        assert_eq!(models, vec![expected]);
    }

    #[tokio::test]
    async fn openrouter_checks_the_key_itself() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/key"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_json(json!({"error": {"message": "No auth credentials found", "code": 401}})),
            )
            .mount(&server)
            .await;
        let e = crate::ai::test_key(&cfg(AiProvider::OpenRouter, &server.uri())).await.unwrap_err();
        assert_eq!(e.humane_in(crate::humane::Lang::En), "The OpenRouter key isn't valid. Check it in Settings.");
    }
}
