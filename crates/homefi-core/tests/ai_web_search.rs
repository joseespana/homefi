//! The AI client's web search against mock providers: the request each one gets
//! (captured, never a partial matcher) and the sources read back.

use homefi_core::ai::{chat, chat_with, AiConfig, AiErrorKind, AiProvider, ChatOptions, Message, WebSearch};
use homefi_core::Error;
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ANTHROPIC: &str = include_str!("fixtures/ai/anthropic_web_search.json");
const ANTHROPIC_PAUSE: &str = include_str!("fixtures/ai/anthropic_pause_turn.json");
const OPENAI: &str = include_str!("fixtures/ai/openai_web_search.json");
const XAI: &str = include_str!("fixtures/ai/xai_responses_citations.json");
const GEMINI: &str = include_str!("fixtures/ai/gemini_grounding.json");
const OPENROUTER: &str = include_str!("fixtures/ai/openrouter_citations.json");
const KEY: &str = "sk-secret-key-123";

fn json_response(fixture: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(serde_json::from_str::<Value>(fixture).unwrap())
}

fn config(provider: AiProvider, server: &MockServer) -> AiConfig {
    AiConfig::new(provider, "model-x", Some(KEY.into())).with_base_url(server.uri())
}

fn search(max_uses: u8) -> ChatOptions {
    ChatOptions { web_search: Some(WebSearch { max_uses }), ..Default::default() }
}

fn ask() -> Vec<Message> {
    vec![Message::User { text: "Find the recent shows of Example Band".into() }]
}

async fn bodies(server: &MockServer) -> Vec<Value> {
    let requests = server.received_requests().await.unwrap();
    requests.iter().map(|r| serde_json::from_slice(&r.body).unwrap()).collect()
}

fn urls(reply: &homefi_core::ai::ChatReply) -> Vec<&str> {
    reply.sources.iter().map(|s| s.url.as_str()).collect()
}

#[tokio::test]
async fn anthropic_searches_and_cites() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/messages")).respond_with(json_response(ANTHROPIC)).mount(&server).await;
    let c = config(AiProvider::Anthropic, &server);
    let reply = chat_with(&c, "sys", &ask(), &[], &search(3)).await.unwrap();
    assert_eq!(
        reply.text,
        "Let me look up the band's recent shows.\nThey opened with Blind and closed with Freak on a Leash."
    );
    assert_eq!(
        urls(&reply),
        [
            "https://www.example.com/band-2026-10-24",
            "https://www.example.com/band-2026-10-25",
            "https://news.example.org/band-tour",
        ]
    );
    assert_eq!(reply.sources[0].title, "Example Band live in Mexico City");
    assert!(reply.search_widget_html.is_none());

    // clamped on the way out: 0 -> 1, 50 -> 10
    chat_with(&c, "sys", &ask(), &[], &search(0)).await.unwrap();
    chat_with(&c, "sys", &ask(), &[], &search(50)).await.unwrap();
    let sent = bodies(&server).await;
    let tool = |i: usize| sent[i]["tools"].clone();
    assert_eq!(tool(0), json!([{"type": "web_search_20250305", "name": "web_search", "max_uses": 3}]));
    assert_eq!(tool(1)[0]["max_uses"], 1);
    assert_eq!(tool(2)[0]["max_uses"], 10);
}

#[tokio::test]
async fn anthropic_resumes_a_paused_turn() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(json_response(ANTHROPIC_PAUSE))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST")).and(path("/messages")).respond_with(json_response(ANTHROPIC)).mount(&server).await;
    let reply = chat_with(&config(AiProvider::Anthropic, &server), "sys", &ask(), &[], &search(5)).await.unwrap();

    let sent = bodies(&server).await;
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0]["messages"].as_array().unwrap().len(), 1);
    let second = sent[1]["messages"].as_array().unwrap();
    assert_eq!(second.len(), 2);
    let paused = second.last().unwrap();
    assert_eq!(paused["role"], "assistant");
    assert_eq!(paused["content"][2]["type"], "web_search_tool_result");
    assert_eq!(paused["content"][2]["content"][0]["encrypted_content"], "encp");

    assert!(reply.text.starts_with("Searching for the first shows.\n"));
    assert!(reply.text.ends_with("Freak on a Leash."));
    assert_eq!(urls(&reply)[0], "https://www.example.com/first-page");
    assert_eq!(reply.sources.len(), 4);
}

#[tokio::test]
async fn openai_asks_for_sources_and_a_budget() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/responses")).respond_with(json_response(OPENAI)).mount(&server).await;
    let reply = chat_with(&config(AiProvider::OpenAi, &server), "sys", &ask(), &[], &search(4)).await.unwrap();
    let sent = &bodies(&server).await[0];
    assert_eq!(sent["tools"], json!([{"type": "web_search", "search_context_size": "low"}]));
    assert_eq!(sent["include"], json!(["reasoning.encrypted_content", "web_search_call.action.sources"]));
    assert_eq!(sent["max_tool_calls"], 4);
    assert_eq!(sent["store"], false);
    assert_eq!(reply.text, "The band opened with Blind.");
    // annotations first, then the search call's sources; deduplicated
    assert_eq!(urls(&reply), ["https://www.example.com/band-2026-10-24", "https://news.example.org/band-review"]);
    assert_eq!(reply.sources[1].title, "https://news.example.org/band-review");
}

#[tokio::test]
async fn xai_searches_on_responses_not_chat_completions() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/responses")).respond_with(json_response(XAI)).mount(&server).await;
    let reply = chat_with(&config(AiProvider::XAi, &server), "sys", &ask(), &[], &search(5)).await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url.path(), "/responses");
    let sent = &bodies(&server).await[0];
    assert_eq!(sent["tools"], json!([{"type": "web_search"}]));
    assert!(sent.get("include").is_none());
    assert!(sent.get("max_tool_calls").is_none());
    assert_eq!(sent["store"], false);
    assert_eq!(urls(&reply), ["https://www.example.com/x-one", "https://news.example.org/x-two"]);
    // the answer is tagged as Responses, so it's never replayed to Chat Completions
    assert_eq!(reply.native.as_ref().unwrap().api, "openai_responses");
}

#[tokio::test]
async fn gemini_grounds_and_returns_the_widget() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/models/model-x:generateContent"))
        .respond_with(json_response(GEMINI))
        .mount(&server)
        .await;
    let reply = chat_with(&config(AiProvider::Gemini, &server), "sys", &ask(), &[], &search(5)).await.unwrap();
    let sent = &bodies(&server).await[0];
    assert_eq!(sent["tools"], json!([{"google_search": {}}]));
    assert_eq!(urls(&reply), ["https://www.example.com/band-2026-10-24", "https://news.example.org/band-tour"]);
    let fixture: Value = serde_json::from_str(GEMINI).unwrap();
    let expected = fixture.pointer("/candidates/0/groundingMetadata/searchEntryPoint/renderedContent").unwrap();
    assert_eq!(reply.search_widget_html.as_deref(), expected.as_str());
}

#[tokio::test]
async fn openrouter_uses_the_web_plugin() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(json_response(OPENROUTER))
        .mount(&server)
        .await;
    let reply = chat_with(&config(AiProvider::OpenRouter, &server), "sys", &ask(), &[], &search(5)).await.unwrap();
    let sent = &bodies(&server).await[0];
    assert_eq!(sent["plugins"], json!([{"id": "web", "max_results": 5}]));
    assert_eq!(urls(&reply), ["https://www.example.com/or-one", "https://news.example.org/or-two"]);
    assert_eq!(reply.sources[0].title, "OR One");
}

#[tokio::test]
async fn providers_that_cannot_search_fail_before_any_request() {
    let server = MockServer::start().await;
    for provider in [AiProvider::Mistral, AiProvider::Ollama, AiProvider::Groq, AiProvider::DeepSeek] {
        let err = chat_with(&config(provider, &server), "sys", &ask(), &[], &search(5)).await.unwrap_err();
        let Error::Ai(e) = err else { panic!("expected an AI error, got {err:?}") };
        assert_eq!(e.kind, AiErrorKind::NoWebSearch);
    }
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn plain_chat_sends_no_search_tool() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/messages")).respond_with(json_response(ANTHROPIC)).mount(&server).await;
    chat(&config(AiProvider::Anthropic, &server), "sys", &ask(), &[]).await.unwrap();
    assert!(bodies(&server).await[0].get("tools").is_none());
}

#[tokio::test]
async fn a_rejected_search_never_echoes_the_key() {
    let server = MockServer::start().await;
    let echo = json!({"error": {"type": "authentication_error", "message": format!("invalid x-api-key {KEY}")}});
    Mock::given(method("POST")).respond_with(ResponseTemplate::new(401).set_body_json(echo)).mount(&server).await;
    let err = chat_with(&config(AiProvider::Anthropic, &server), "sys", &ask(), &[], &search(2)).await.unwrap_err();
    let Error::Ai(e) = err else { panic!("expected an AI error") };
    assert_eq!(e.kind, AiErrorKind::InvalidKey);
    assert!(!format!("{e} {e:?} {}", e.humane_in(homefi_core::humane::Lang::En)).contains(KEY));
}
