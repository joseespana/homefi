//! Google's Gemini API (`POST /v1beta/models/{model}:generateContent`, key in
//! `x-goog-api-key`, tools as `functionDeclarations` with
//! `parametersJsonSchema`, calls as `functionCall` parts, results as
//! `functionResponse` parts). The model's parts are replayed as they came,
//! because Gemini needs its `thoughtSignature`s back.

use serde_json::{json, Value};

use super::chat::{arguments, send, ChatReply, Message, Native, System, Tool, ToolCall};
use super::web_search::{self, WebSearch};
use super::{http, AiConfig, AiError, AiErrorKind, Api, ModelInfo, CHAT_TIMEOUT, LIST_TIMEOUT};

fn request(config: &AiConfig, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    match config.key() {
        Some(key) => builder.header("x-goog-api-key", key),
        None => builder,
    }
}

/// `gemini-x` or `models/gemini-x` → `models/gemini-x`.
fn model_path(model: &str) -> String {
    let model = model.trim();
    if model.starts_with("models/") || model.starts_with("tunedModels/") {
        model.to_owned()
    } else {
        format!("models/{model}")
    }
}

pub(crate) fn body(config: &AiConfig, system: &System, messages: &[Message], tools: &[Tool], max_tokens: u32) -> Value {
    let mut contents: Vec<Value> = Vec::new();
    let push_user = |contents: &mut Vec<Value>, parts: Vec<Value>| match contents.last_mut() {
        Some(last) if last["role"] == "user" => last["parts"].as_array_mut().expect("parts").extend(parts),
        _ => contents.push(json!({ "role": "user", "parts": parts })),
    };
    for m in messages {
        match m {
            Message::User { text } => push_user(&mut contents, vec![json!({ "text": text })]),
            Message::Tool { results } => push_user(
                &mut contents,
                results
                    .iter()
                    .map(|r| {
                        // the response must be an object: JSON results as they are, words wrapped
                        let value = serde_json::from_str::<Value>(&r.content).unwrap_or_else(|_| json!(r.content));
                        let response = match value {
                            Value::Object(_) if !r.is_error => value,
                            v if r.is_error => json!({ "error": v }),
                            v => json!({ "result": v }),
                        };
                        let mut call = json!({ "name": r.name, "response": response });
                        if !r.call_id.starts_with("gemini_") {
                            call["id"] = json!(r.call_id);
                        }
                        json!({ "functionResponse": call })
                    })
                    .collect(),
            ),
            Message::Assistant { text, tool_calls, native } => {
                let content = match Native::for_config(native, config) {
                    Some(content) => content.clone(),
                    None => {
                        let mut parts = Vec::new();
                        if !text.is_empty() {
                            parts.push(json!({ "text": text }));
                        }
                        for c in tool_calls {
                            parts.push(json!({ "functionCall": { "name": c.name, "args": c.arguments } }));
                        }
                        json!({ "role": "model", "parts": parts })
                    }
                };
                contents.push(content);
            }
        }
    }
    let mut body = json!({
        "systemInstruction": { "parts": [{ "text": system.joined() }] },
        "contents": contents,
        "generationConfig": { "maxOutputTokens": max_tokens },
    });
    if !tools.is_empty() {
        let declarations: Vec<Value> = tools
            .iter()
            .map(|t| json!({ "name": t.name, "description": t.description, "parametersJsonSchema": t.parameters }))
            .collect();
        body["tools"] = json!([{ "functionDeclarations": declarations }]);
    }
    body
}

pub(crate) fn parse(config: &AiConfig, answer: Value) -> Result<ChatReply, AiError> {
    if answer.pointer("/candidates/0/finishReason") == Some(&json!("MAX_TOKENS")) {
        return Err(AiError::truncated(config));
    }
    let Some(content) = answer.pointer("/candidates/0/content").filter(|c| c.is_object()) else {
        // blocked prompts come back without candidates
        let reason = answer.pointer("/promptFeedback/blockReason").and_then(Value::as_str).unwrap_or("no candidates");
        return Err(AiError::new(config, AiErrorKind::BadAnswer, reason));
    };
    let mut text = Vec::new();
    let mut tool_calls = Vec::new();
    for (i, part) in content["parts"].as_array().into_iter().flatten().enumerate() {
        if part["thought"] == Value::Bool(true) {
            continue;
        }
        if let Some(t) = part["text"].as_str() {
            text.push(t.to_owned());
        }
        if let Some(call) = part.get("functionCall") {
            tool_calls.push(ToolCall {
                // older models send no id: one of ours, never sent back
                id: call["id"]
                    .as_str()
                    .filter(|id| !id.is_empty())
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("gemini_{i}")),
                name: call["name"].as_str().unwrap_or_default().to_owned(),
                arguments: arguments(call.get("args")),
            });
        }
    }
    let mut content = content.clone();
    if content.get("role").is_none() {
        content["role"] = json!("model");
    }
    let (sources, search_widget_html) = web_search::gemini_grounding(&answer);
    Ok(ChatReply {
        text: text.join("").trim().to_owned(),
        tool_calls,
        native: Some(Native::new(Api::Gemini, config, content)),
        sources,
        search_widget_html,
    })
}

pub(crate) async fn chat(
    config: &AiConfig,
    system: &System<'_>,
    messages: &[Message],
    tools: &[Tool],
    search: Option<&WebSearch>,
    max_tokens: u32,
    light: bool,
) -> Result<ChatReply, AiError> {
    let url = format!("{}/{}:generateContent", config.base(), model_path(&config.model));
    let mut payload = body(config, system, messages, tools, max_tokens);
    if light {
        light_thinking(&config.model, &mut payload);
    }
    if search.is_some() {
        web_search::gemini_request(&mut payload);
    }
    let req = request(config, http().post(url)).timeout(CHAT_TIMEOUT).json(&payload);
    parse(config, send(config, req, false).await?)
}

/// A quick task: Gemini's thinking models think little, so the thinking does not use up the answer's tokens.
/// Gemini 2.x takes a token budget; later models take a level.
fn light_thinking(model: &str, payload: &mut Value) {
    let thinking = if model.contains("gemini-2") {
        json!({ "thinkingBudget": LIGHT_THINKING_BUDGET })
    } else {
        json!({ "thinkingLevel": "low" })
    };
    payload["generationConfig"]["thinkingConfig"] = thinking;
}

/// Thinking tokens a Gemini 2.x model gets for a quick task (Pro needs at least 128).
const LIGHT_THINKING_BUDGET: u32 = 512;

pub(crate) async fn list_models(config: &AiConfig) -> Result<Vec<ModelInfo>, AiError> {
    let mut models = Vec::new();
    let mut token: Option<String> = None;
    for _ in 0..5 {
        let mut req = request(config, http().get(format!("{}/models", config.base())))
            .timeout(LIST_TIMEOUT)
            .query(&[("pageSize", "1000")]);
        if let Some(t) = &token {
            req = req.query(&[("pageToken", t)]);
        }
        let page = send(config, req, true).await?;
        for m in page["models"].as_array().into_iter().flatten() {
            let chats =
                m["supportedGenerationMethods"].as_array().is_none_or(|g| g.iter().any(|v| v == "generateContent"));
            let Some(name) = m["name"].as_str().filter(|_| chats) else { continue };
            let id = name.strip_prefix("models/").unwrap_or(name);
            let display = m["displayName"].as_str().filter(|n| !n.is_empty()).unwrap_or(id);
            models.push(ModelInfo::new(id, display));
        }
        match page["nextPageToken"].as_str().filter(|t| !t.is_empty()) {
            Some(t) => token = Some(t.to_owned()),
            None => break,
        }
    }
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::chat::ToolResult;
    use crate::ai::AiProvider;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn cfg(base: &str) -> AiConfig {
        AiConfig::new(AiProvider::Gemini, "gemini-flash-x", Some("AIza-test".into())).with_base_url(base)
    }

    #[test]
    fn results_become_function_responses() {
        let history = vec![
            Message::User { text: "hi".into() },
            Message::Assistant {
                text: String::new(),
                tool_calls: vec![ToolCall { id: "gemini_0".into(), name: "f".into(), arguments: json!({}) }],
                native: None,
            },
            Message::Tool {
                results: vec![
                    ToolResult {
                        call_id: "gemini_0".into(),
                        name: "f".into(),
                        content: "[1,2]".into(),
                        is_error: false,
                    },
                    ToolResult {
                        call_id: "abc".into(),
                        name: "g".into(),
                        content: "{\"ok\":1}".into(),
                        is_error: false,
                    },
                ],
            },
            Message::User { text: "ok".into() },
        ];
        let tools = [Tool { name: "f".into(), description: "d".into(), parameters: json!({"type": "object"}) }];
        let b = body(&cfg("http://x"), &"sys".into(), &history, &tools, 4096);
        assert_eq!(b["systemInstruction"]["parts"][0]["text"], "sys");
        assert_eq!(b["tools"][0]["functionDeclarations"][0]["parametersJsonSchema"]["type"], "object");
        let c = b["contents"].as_array().unwrap();
        assert_eq!(c.len(), 3);
        assert_eq!(c[1]["role"], "model");
        assert_eq!(c[2]["parts"][0]["functionResponse"]["response"]["result"], json!([1, 2]));
        assert!(c[2]["parts"][0]["functionResponse"].get("id").is_none());
        assert_eq!(c[2]["parts"][1]["functionResponse"]["id"], "abc");
        assert_eq!(c[2]["parts"][2]["text"], "ok");
    }

    #[tokio::test]
    async fn keeps_the_thought_signature_for_the_next_request() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/models/gemini-flash-x:generateContent"))
            .and(header("x-goog-api-key", "AIza-test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "candidates": [{"content": {"role": "model", "parts": [
                    {"text": "thinking…", "thought": true},
                    {"text": "Veamos."},
                    {"functionCall": {"name": "search_headphones", "args": {"query": "HD 600"}}, "thoughtSignature": "c2ln"}
                ]}, "finishReason": "STOP"}]
            })))
            .mount(&server)
            .await;
        let c = cfg(&server.uri());
        let reply =
            chat(&c, &"s".into(), &[Message::User { text: "hi".into() }], &[], None, 4096, false).await.unwrap();
        assert_eq!(reply.text, "Veamos.");
        assert_eq!(reply.tool_calls[0].id, "gemini_2");
        assert_eq!(reply.tool_calls[0].arguments["query"], "HD 600");
        let again = body(&c, &"s".into(), &[reply.into_message()], &[], 4096);
        assert_eq!(again["contents"][0]["parts"][2]["thoughtSignature"], "c2ln");
    }

    #[test]
    fn quick_tasks_ask_thinking_models_to_think_little() {
        let mut new = json!({ "generationConfig": { "maxOutputTokens": 10 } });
        light_thinking("gemini-3.8-flash", &mut new);
        assert_eq!(new["generationConfig"]["thinkingConfig"]["thinkingLevel"], "low");
        assert_eq!(new["generationConfig"]["maxOutputTokens"], 10);
        let mut old = json!({});
        light_thinking("models/gemini-2.5-pro", &mut old);
        assert_eq!(old["generationConfig"]["thinkingConfig"]["thinkingBudget"], LIGHT_THINKING_BUDGET);
    }

    #[tokio::test]
    async fn lists_only_models_that_generate_content() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models": [
                {"name": "models/gemini-flash-x", "displayName": "Gemini Flash X", "supportedGenerationMethods": ["generateContent", "countTokens"]},
                {"name": "models/embedder", "displayName": "Embedder", "supportedGenerationMethods": ["embedContent"]}
            ]})))
            .mount(&server)
            .await;
        let models = list_models(&cfg(&server.uri())).await.unwrap();
        assert_eq!(models, vec![ModelInfo::new("gemini-flash-x", "Gemini Flash X")]);
    }

    #[test]
    fn answers_cut_at_the_limit_are_errors() {
        let c = cfg("http://x");
        let cut = json!({"candidates": [{"finishReason": "MAX_TOKENS", "content": {"parts": [{"text": "{\"songs"}]}}]});
        assert!(parse(&c, cut).unwrap_err().is_truncated());
        let done = json!({"candidates": [{"finishReason": "STOP", "content": {"parts": [{"text": "ok"}]}}]});
        assert_eq!(parse(&c, done).unwrap().text, "ok");
        let b = body(&c, &System::new("rules. ", "state"), &[], &[], 999);
        assert_eq!(b["generationConfig"]["maxOutputTokens"], 999);
        assert_eq!(b["systemInstruction"]["parts"][0]["text"], "rules. state");
    }
}
