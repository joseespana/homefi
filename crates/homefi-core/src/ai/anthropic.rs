//! Anthropic's Messages API (`POST /v1/messages`, tools as `input_schema`,
//! answers as `text` / `tool_use` blocks, results as `tool_result` blocks).

use serde_json::{json, Value};

use super::chat::{arguments, send, ChatReply, Message, Native, Tool, ToolCall, MAX_TOKENS};
use super::{http, AiConfig, AiError, AiErrorKind, ModelInfo, CHAT_TIMEOUT, LIST_TIMEOUT};

/// The API version this client is written against.
const VERSION: &str = "2026-10-21";

fn request(config: &AiConfig, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    let builder = builder.header("anthropic-version", VERSION);
    match config.key() {
        Some(key) => builder.header("x-api-key", key),
        None => builder,
    }
}

pub(crate) fn body(config: &AiConfig, system: &str, messages: &[Message], tools: &[Tool]) -> Value {
    let mut out: Vec<Value> = Vec::new();
    // consecutive user-side entries (tool results, then the user's words) go in one message
    let push_user = |out: &mut Vec<Value>, blocks: Vec<Value>| match out.last_mut() {
        Some(last) if last["role"] == "user" => last["content"].as_array_mut().expect("blocks").extend(blocks),
        _ => out.push(json!({ "role": "user", "content": blocks })),
    };
    for m in messages {
        match m {
            Message::User { text } => push_user(&mut out, vec![json!({ "type": "text", "text": text })]),
            Message::Tool { results } => push_user(
                &mut out,
                results
                    .iter()
                    .map(|r| json!({ "type": "tool_result", "tool_use_id": r.call_id, "content": r.content, "is_error": r.is_error }))
                    .collect(),
            ),
            Message::Assistant { text, tool_calls, native } => {
                let content = match Native::for_config(native, config) {
                    Some(blocks) => blocks.clone(),
                    None => {
                        let mut blocks = Vec::new();
                        if !text.is_empty() {
                            blocks.push(json!({ "type": "text", "text": text }));
                        }
                        for c in tool_calls {
                            blocks.push(json!({ "type": "tool_use", "id": c.id, "name": c.name, "input": c.arguments }));
                        }
                        Value::Array(blocks)
                    }
                };
                out.push(json!({ "role": "assistant", "content": content }));
            }
        }
    }
    let mut body = json!({
        "model": config.model.trim(),
        "max_tokens": MAX_TOKENS,
        "system": system,
        "messages": out,
    });
    if !tools.is_empty() {
        body["tools"] = tools
            .iter()
            .map(|t| json!({ "name": t.name, "description": t.description, "input_schema": t.parameters }))
            .collect();
    }
    body
}

pub(crate) fn parse(config: &AiConfig, answer: Value) -> Result<ChatReply, AiError> {
    let Some(blocks) = answer.get("content").and_then(Value::as_array) else {
        return Err(AiError::new(config, AiErrorKind::BadAnswer, "no content"));
    };
    let mut text = Vec::new();
    let mut tool_calls = Vec::new();
    for b in blocks {
        match b["type"].as_str() {
            Some("text") => text.extend(b["text"].as_str().map(str::to_owned)),
            Some("tool_use") => tool_calls.push(ToolCall {
                id: b["id"].as_str().unwrap_or_default().to_owned(),
                name: b["name"].as_str().unwrap_or_default().to_owned(),
                arguments: arguments(b.get("input")),
            }),
            // thinking and anything new: kept in `native` only
            _ => {}
        }
    }
    Ok(ChatReply {
        text: text.join("\n").trim().to_owned(),
        tool_calls,
        native: Some(Native::new(config, Value::Array(blocks.clone()))),
    })
}

pub(crate) async fn chat(
    config: &AiConfig,
    system: &str,
    messages: &[Message],
    tools: &[Tool],
) -> Result<ChatReply, AiError> {
    let url = format!("{}/messages", config.base());
    let req = request(config, http().post(url)).timeout(CHAT_TIMEOUT).json(&body(config, system, messages, tools));
    parse(config, send(config, req, false).await?)
}

pub(crate) async fn list_models(config: &AiConfig) -> Result<Vec<ModelInfo>, AiError> {
    let mut models = Vec::new();
    let mut after: Option<String> = None;
    // newest first, paged; a few pages is every model there is
    for _ in 0..5 {
        let mut req = request(config, http().get(format!("{}/models", config.base())))
            .timeout(LIST_TIMEOUT)
            .query(&[("limit", "1000")]);
        if let Some(id) = &after {
            req = req.query(&[("after_id", id)]);
        }
        let page = send(config, req, true).await?;
        for m in page["data"].as_array().into_iter().flatten() {
            let Some(id) = m["id"].as_str() else { continue };
            let name = m["display_name"].as_str().filter(|n| !n.is_empty()).unwrap_or(id);
            models.push(ModelInfo::new(id, name));
        }
        match (page["has_more"].as_bool(), page["last_id"].as_str()) {
            (Some(true), Some(last)) => after = Some(last.to_owned()),
            _ => break,
        }
    }
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{chat::ToolResult, AiProvider};
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn config(base: &str) -> AiConfig {
        AiConfig::new(AiProvider::Anthropic, "claude-sonnet-5-5", Some("sk-ant-test".into())).with_base_url(base)
    }

    fn tool() -> Tool {
        Tool {
            name: "search_headphones".into(),
            description: "Find".into(),
            parameters: json!({"type": "object", "properties": {"query": {"type": "string"}}}),
        }
    }

    #[test]
    fn tool_results_and_the_next_words_share_one_user_message() {
        let history = vec![
            Message::User { text: "hi".into() },
            Message::Assistant {
                text: "Let me look".into(),
                tool_calls: vec![ToolCall {
                    id: "toolu_1".into(),
                    name: "search_headphones".into(),
                    arguments: json!({"query": "hd600"}),
                }],
                native: None,
            },
            Message::Tool {
                results: vec![ToolResult {
                    call_id: "toolu_1".into(),
                    name: "search_headphones".into(),
                    content: "[]".into(),
                    is_error: false,
                }],
            },
            Message::User { text: "the first".into() },
        ];
        let b = body(&config("http://x"), "be nice", &history, &[tool()]);
        assert_eq!(b["system"], "be nice");
        assert_eq!(b["tools"][0]["input_schema"]["type"], "object");
        let msgs = b["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[1]["content"][1]["type"], "tool_use");
        assert_eq!(msgs[2]["content"][0]["type"], "tool_result");
        assert_eq!(msgs[2]["content"][0]["tool_use_id"], "toolu_1");
        assert_eq!(msgs[2]["content"][1]["text"], "the first");
    }

    #[tokio::test]
    async fn sends_the_version_and_key_and_reads_tool_use() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/messages"))
            .and(header("x-api-key", "sk-ant-test"))
            .and(header("anthropic-version", VERSION))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "msg_1", "type": "message", "role": "assistant", "model": "claude-sonnet-5-5",
                "content": [
                    {"type": "text", "text": "Looking that up."},
                    {"type": "tool_use", "id": "toolu_9", "name": "search_headphones", "input": {"query": "Sony WH-1000XM5"}}
                ],
                "stop_reason": "tool_use", "usage": {"input_tokens": 10, "output_tokens": 5}
            })))
            .mount(&server)
            .await;
        let reply =
            chat(&config(&server.uri()), "sys", &[Message::User { text: "hi".into() }], &[tool()]).await.unwrap();
        assert_eq!(reply.text, "Looking that up.");
        assert_eq!(reply.tool_calls[0].name, "search_headphones");
        assert_eq!(reply.tool_calls[0].arguments["query"], "Sony WH-1000XM5");
        // replayed verbatim next time
        let replay = body(&config("http://x"), "", &[reply.into_message()], &[]);
        assert_eq!(replay["messages"][0]["content"][1]["id"], "toolu_9");
    }

    #[tokio::test]
    async fn lists_models_across_pages() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .and(wiremock::matchers::query_param("after_id", "b"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    json!({"data": [{"id": "c", "display_name": "C"}], "has_more": false, "last_id": "c"}),
                ),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": [{"id": "a", "display_name": "Claude A"}, {"id": "b"}], "has_more": true, "last_id": "b"})))
            .mount(&server)
            .await;
        let models = list_models(&config(&server.uri())).await.unwrap();
        let ids: Vec<_> = models.iter().map(|m| (m.id.as_str(), m.name.as_str())).collect();
        assert_eq!(ids, [("a", "Claude A"), ("b", "b"), ("c", "C")]);
    }

    #[tokio::test]
    async fn a_bad_key_says_so() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(401).set_body_json(
                json!({"type": "error", "error": {"type": "authentication_error", "message": "invalid x-api-key"}}),
            ))
            .mount(&server)
            .await;
        let e = crate::ai::test_key(&config(&server.uri())).await.unwrap_err();
        assert_eq!(e.humane_in(crate::humane::Lang::Es), "La llave de Anthropic no es válida. Revísala en Ajustes.");
    }
}
