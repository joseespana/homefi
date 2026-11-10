//! A conversation in one shape for every provider; each client turns it into
//! its own wire format.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::web_search::{Source, WebSearch};
use super::{anthropic, gemini, openai, AiConfig, AiError, AiErrorKind, Api};

/// A function the model may call. `parameters` is a JSON Schema object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// The model asked to run a tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    /// Always a JSON object (empty when the model sent nothing).
    pub arguments: Value,
}

/// What a tool returned, sent back to the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    pub call_id: String,
    pub name: String,
    /// JSON text (or plain words) for the model.
    pub content: String,
    #[serde(default)]
    pub is_error: bool,
}

/// One entry of the history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum Message {
    User {
        text: String,
    },
    Assistant {
        #[serde(default)]
        text: String,
        #[serde(default)]
        tool_calls: Vec<ToolCall>,
        /// The provider's own form of this answer (thinking blocks, Gemini's
        /// thought signatures, OpenAI reasoning items), replayed as is to the
        /// same API; other providers rebuild it from `text` and `tool_calls`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        native: Option<Native>,
    },
    Tool {
        results: Vec<ToolResult>,
    },
}

/// A provider's raw answer, tagged with the API (and, for the look-alike
/// OpenAI-compatible ones, the provider) it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Native {
    pub api: String,
    pub value: Value,
}

impl Native {
    /// `api` is the one the answer actually came from (xAI searches on Responses, not Chat Completions).
    pub(crate) fn new(api: Api, config: &AiConfig, value: Value) -> Self {
        Self { api: tag(api, config), value }
    }

    /// The raw value when it came from the same API as `config`'s.
    pub(crate) fn for_config<'a>(native: &'a Option<Native>, config: &AiConfig) -> Option<&'a Value> {
        let tag = tag(config.provider.api(), config);
        native.as_ref().filter(|n| n.api == tag).map(|n| &n.value)
    }
}

fn tag(api: Api, config: &AiConfig) -> String {
    match api {
        Api::Anthropic => "anthropic".into(),
        Api::OpenAiResponses => "openai_responses".into(),
        Api::Gemini => "gemini".into(),
        // each one adds its own extra fields (reasoning_content, reasoning_details...)
        Api::ChatCompletions => format!("chat_completions:{}", config.provider.id()),
    }
}

/// The model's answer to one request.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatReply {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    pub native: Option<Native>,
    /// Pages cited or read by the provider's web search (deduplicated by URL, in order).
    pub sources: Vec<Source>,
    /// Gemini only: Google's Search Suggestions HTML (`searchEntryPoint.renderedContent`),
    /// which must be shown with the grounded result.
    pub search_widget_html: Option<String>,
}

/// Extras for one call.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChatOptions {
    /// Let the model search the web with its provider's own tool.
    pub web_search: Option<WebSearch>,
    /// Cap on the answer's tokens for this call; `None` is [`MAX_TOKENS`].
    pub max_tokens: Option<u32>,
    /// A quick task (a list of songs): ask a thinking model for low reasoning effort, so its thinking does not
    /// use up the answer before the tool call (OpenAI and OpenRouter; the others think little unless asked).
    pub light_reasoning: bool,
}

impl ChatOptions {
    /// The cap for this call, never above [`MAX_OUTPUT_CEILING`].
    pub(crate) fn output_cap(&self) -> u32 {
        self.max_tokens.map_or(MAX_TOKENS, |t| t.clamp(MIN_OUTPUT_TOKENS, MAX_OUTPUT_CEILING))
    }
}

/// A system prompt in two parts: `fixed` is the same bytes on every round (so Anthropic can
/// cache it), `dynamic` changes with the app's state. Every other provider gets them joined,
/// fixed first.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct System<'a> {
    pub fixed: &'a str,
    pub dynamic: &'a str,
}

impl<'a> System<'a> {
    pub fn new(fixed: &'a str, dynamic: &'a str) -> Self {
        Self { fixed, dynamic }
    }

    /// Both parts as one text.
    pub fn joined(&self) -> std::borrow::Cow<'a, str> {
        match (self.fixed.is_empty(), self.dynamic.is_empty()) {
            (_, true) => self.fixed.into(),
            (true, false) => self.dynamic.into(),
            (false, false) => format!("{}{}", self.fixed, self.dynamic).into(),
        }
    }
}

impl<'a> From<&'a str> for System<'a> {
    fn from(whole: &'a str) -> Self {
        Self { fixed: "", dynamic: whole }
    }
}

impl ChatReply {
    /// The answer as a history entry.
    pub fn into_message(self) -> Message {
        Message::Assistant { text: self.text, tool_calls: self.tool_calls, native: self.native }
    }
}

/// Up to this many tokens per answer when the call doesn't ask for more or less.
pub(crate) const MAX_TOKENS: u32 = 4096;

/// A chat turn: a few sentences and one or two tool calls.
pub(crate) const CHAT_TURN_TOKENS: u32 = 2048;

/// A reaction to a tap: a short message and one small change.
pub(crate) const REACTION_TOKENS: u32 = 1024;

/// A call's own cap never goes below this (a cap too small only truncates)...
const MIN_OUTPUT_TOKENS: u32 = 256;

/// ...or above this: every provider's current models accept it, and it bounds what one call can cost.
pub const MAX_OUTPUT_CEILING: u32 = 16_384;

/// Sends the conversation and returns the model's next answer.
/// `config.model` must be set (see [`super::resolve_model`]).
pub async fn chat(config: &AiConfig, system: &str, messages: &[Message], tools: &[Tool]) -> crate::Result<ChatReply> {
    chat_with(config, system, messages, tools, &ChatOptions::default()).await
}

/// [`chat`] with options. Asking for web search from a provider that can't search is
/// `AiErrorKind::NoWebSearch`, before any request.
pub async fn chat_with(
    config: &AiConfig,
    system: &str,
    messages: &[Message],
    tools: &[Tool],
    options: &ChatOptions,
) -> crate::Result<ChatReply> {
    chat_system(config, &System::from(system), messages, tools, options).await
}

/// [`chat_with`] with the system prompt in a fixed and a dynamic part.
pub async fn chat_system(
    config: &AiConfig,
    system: &System<'_>,
    messages: &[Message],
    tools: &[Tool],
    options: &ChatOptions,
) -> crate::Result<ChatReply> {
    config.check()?;
    if config.model.trim().is_empty() {
        return Err(AiError::new(config, AiErrorKind::ModelNotFound, "no model chosen").into());
    }
    let search = options.web_search.as_ref();
    let api = match search {
        None => config.provider.api(),
        Some(_) => config
            .provider
            .search_api()
            .ok_or_else(|| AiError::new(config, AiErrorKind::NoWebSearch, "no web search"))?,
    };
    let cap = options.output_cap();
    let reply = match api {
        Api::Anthropic => anthropic::chat(config, system, messages, tools, search, cap).await?,
        Api::OpenAiResponses => {
            openai::responses(config, system, messages, tools, search, cap, options.light_reasoning).await?
        }
        Api::ChatCompletions => {
            openai::chat_completions(config, system, messages, tools, search, cap, options.light_reasoning).await?
        }
        Api::Gemini => gemini::chat(config, system, messages, tools, search, cap, options.light_reasoning).await?,
    };
    Ok(reply)
}

/// Sends a prepared request and returns the JSON answer, or the error sorted.
pub(crate) async fn send(config: &AiConfig, request: reqwest::RequestBuilder, listing: bool) -> Result<Value, AiError> {
    let response = request.send().await.map_err(|e| AiError::from_http(config, &e))?;
    let status = response.status().as_u16();
    let body = response.text().await.map_err(|e| AiError::from_http(config, &e))?;
    if !(200..300).contains(&status) {
        tracing::warn!(provider = config.provider.id(), status, "AI request failed");
        return Err(AiError::from_status(config, status, &body, listing));
    }
    serde_json::from_str(&body).map_err(|e| AiError::new(config, AiErrorKind::BadAnswer, format!("not JSON: {e}")))
}

/// Tool arguments as an object, whether the model sent a JSON string or an object.
pub(crate) fn arguments(raw: Option<&Value>) -> Value {
    let value = match raw {
        Some(Value::String(s)) if s.trim().is_empty() => Value::Null,
        Some(Value::String(s)) => serde_json::from_str(s).unwrap_or_else(|_| Value::String(s.clone())),
        Some(v) => v.clone(),
        None => Value::Null,
    };
    match value {
        Value::Object(_) => value,
        Value::Null => Value::Object(Default::default()),
        // not an object: hand it over so the tool can say what's wrong
        other => serde_json::json!({ "_raw": other }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn history_serializes_for_resuming() {
        let gemini = AiConfig::new(crate::ai::AiProvider::Gemini, "g", None);
        let history = vec![
            Message::User { text: "hola".into() },
            Message::Assistant {
                text: String::new(),
                tool_calls: vec![ToolCall {
                    id: "c1".into(),
                    name: "search_headphones".into(),
                    arguments: json!({"query": "hd 600"}),
                }],
                native: Some(Native::new(Api::Gemini, &gemini, json!({"parts": []}))),
            },
            Message::Tool {
                results: vec![ToolResult {
                    call_id: "c1".into(),
                    name: "search_headphones".into(),
                    content: "[]".into(),
                    is_error: false,
                }],
            },
        ];
        let text = serde_json::to_string(&history).unwrap();
        assert_eq!(serde_json::from_str::<Vec<Message>>(&text).unwrap(), history);
        let anthropic = AiConfig::new(crate::ai::AiProvider::Anthropic, "a", None);
        assert!(Native::for_config(&Some(Native::new(Api::Gemini, &gemini, json!(1))), &anthropic).is_none());
        let groq = AiConfig::new(crate::ai::AiProvider::Groq, "a", None);
        let deepseek = AiConfig::new(crate::ai::AiProvider::DeepSeek, "a", None);
        assert!(Native::for_config(&Some(Native::new(Api::ChatCompletions, &deepseek, json!(1))), &groq).is_none());
    }

    #[test]
    fn arguments_are_always_objects() {
        assert_eq!(arguments(Some(&json!("{\"a\":1}"))), json!({"a": 1}));
        assert_eq!(arguments(Some(&json!({"a": 1}))), json!({"a": 1}));
        assert_eq!(arguments(Some(&json!(""))), json!({}));
        assert_eq!(arguments(None), json!({}));
        assert_eq!(arguments(Some(&json!("oops"))), json!({"_raw": "oops"}));
    }

    #[test]
    fn the_output_cap_is_the_default_or_the_calls_within_bounds() {
        assert_eq!(ChatOptions::default().output_cap(), MAX_TOKENS);
        let cap = |t| ChatOptions { max_tokens: Some(t), ..Default::default() }.output_cap();
        assert_eq!(cap(1024), 1024);
        assert_eq!(cap(1), MIN_OUTPUT_TOKENS);
        assert_eq!(cap(u32::MAX), MAX_OUTPUT_CEILING);
    }

    #[test]
    fn the_system_parts_join_fixed_first() {
        assert_eq!(System::new("a", "b").joined(), "ab");
        assert_eq!(System::new("a", "").joined(), "a");
        assert_eq!(System::from("whole").joined(), "whole");
    }
}
