//! The provider's own web search: what to add to a request to turn it on, and
//! how to read the pages the model cited from each API's answer.
//!
//! The request side is a handful of small functions that patch the JSON body a
//! client already built (so searchless chats keep their exact bodies); the
//! answer side reads sources leniently and always runs, because it costs
//! nothing when the fields are absent.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Most searches one call may ask for.
pub const MAX_WEB_SEARCHES: u8 = 10;

/// Anthropic's server-side search tool.
pub(crate) const ANTHROPIC_WEB_SEARCH_TOOL: &str = "web_search_20250305";
/// How many results OpenRouter's web plugin attaches.
pub(crate) const OPENROUTER_WEB_RESULTS: u8 = 5;

/// The provider's own web search, asked for on one call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebSearch {
    /// Searches the model may run: 1..=10 (clamped). Anthropic `max_uses`, OpenAI `max_tool_calls`.
    pub max_uses: u8,
}

impl WebSearch {
    /// [`max_uses`](Self::max_uses) inside `1..=`[`MAX_WEB_SEARCHES`].
    pub fn uses(&self) -> u8 {
        self.max_uses.clamp(1, MAX_WEB_SEARCHES)
    }
}

/// A page the model read and cited.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub url: String,
    pub title: String,
}

// -------------------------------------------------------------------- request

fn push_tool(body: &mut Value, tool: Value) {
    match body.get_mut("tools").and_then(Value::as_array_mut) {
        Some(tools) => tools.push(tool),
        None => body["tools"] = json!([tool]),
    }
}

pub(crate) fn anthropic_request(body: &mut Value, search: &WebSearch) {
    push_tool(body, json!({ "type": ANTHROPIC_WEB_SEARCH_TOOL, "name": "web_search", "max_uses": search.uses() }));
}

pub(crate) fn openai_request(body: &mut Value, search: &WebSearch) {
    push_tool(body, json!({ "type": "web_search", "search_context_size": "low" }));
    match body.get_mut("include").and_then(Value::as_array_mut) {
        Some(include) => include.push(json!("web_search_call.action.sources")),
        None => body["include"] = json!(["web_search_call.action.sources"]),
    }
    body["max_tool_calls"] = json!(search.uses());
}

/// xAI's Responses API takes the bare tool and none of OpenAI's extras.
pub(crate) fn xai_request(body: &mut Value) {
    if let Some(map) = body.as_object_mut() {
        map.remove("include");
    }
    push_tool(body, json!({ "type": "web_search" }));
}

pub(crate) fn gemini_request(body: &mut Value) {
    push_tool(body, json!({ "google_search": {} }));
}

pub(crate) fn openrouter_request(body: &mut Value) {
    body["plugins"] = json!([{ "id": "web", "max_results": OPENROUTER_WEB_RESULTS }]);
}

// --------------------------------------------------------------------- answer

/// Pages in first-seen order, deduplicated by URL.
#[derive(Default)]
pub(crate) struct SourceList(Vec<Source>);

impl SourceList {
    /// Skips empty URLs and repeats; a missing title becomes the URL.
    pub(crate) fn add(&mut self, url: Option<&str>, title: Option<&str>) {
        let url = url.map(str::trim).unwrap_or_default();
        if url.is_empty() || self.0.iter().any(|s| s.url == url) {
            return;
        }
        let title = title.map(str::trim).filter(|t| !t.is_empty()).unwrap_or(url);
        self.0.push(Source { url: url.to_owned(), title: title.to_owned() });
    }

    fn add_value(&mut self, v: &Value) {
        self.add(v["url"].as_str(), v["title"].as_str());
    }

    pub(crate) fn extend(&mut self, others: Vec<Source>) {
        for s in others {
            self.add(Some(&s.url), Some(&s.title));
        }
    }

    pub(crate) fn into_vec(self) -> Vec<Source> {
        self.0
    }
}

/// Anthropic: the citations on `text` blocks, then the pages the search returned.
pub(crate) fn anthropic_sources(blocks: &[Value]) -> Vec<Source> {
    let mut out = SourceList::default();
    for b in blocks.iter().filter(|b| b["type"] == "text") {
        b["citations"].as_array().into_iter().flatten().for_each(|c| out.add_value(c));
    }
    for b in blocks.iter().filter(|b| b["type"] == "web_search_tool_result") {
        // an error object instead of a list is not an error for us
        for r in b["content"].as_array().into_iter().flatten().filter(|r| r["type"] == "web_search_result") {
            out.add_value(r);
        }
    }
    out.into_vec()
}

/// OpenAI and xAI Responses: `url_citation` annotations, the search calls' sources and
/// (xAI) the root `citations`, which are plain URLs or `{url, title}` objects.
pub(crate) fn responses_sources(items: &[Value], root: &Value) -> Vec<Source> {
    let mut out = SourceList::default();
    for item in items.iter().filter(|i| i["type"] == "message") {
        for part in item["content"].as_array().into_iter().flatten() {
            let notes = part["annotations"].as_array().into_iter().flatten();
            notes.filter(|a| a["type"] == "url_citation").for_each(|a| out.add_value(a));
        }
    }
    for item in items.iter().filter(|i| i["type"] == "web_search_call") {
        item.pointer("/action/sources").and_then(Value::as_array).into_iter().flatten().for_each(|s| out.add_value(s));
    }
    for c in root["citations"].as_array().into_iter().flatten() {
        match c.as_str() {
            Some(url) => out.add(Some(url), None),
            None => out.add_value(c),
        }
    }
    out.into_vec()
}

/// Gemini: the grounding chunks, and Google's Search Suggestions HTML, which must be shown.
pub(crate) fn gemini_grounding(answer: &Value) -> (Vec<Source>, Option<String>) {
    let Some(meta) = answer.pointer("/candidates/0/groundingMetadata") else {
        return (Vec::new(), None);
    };
    let mut out = SourceList::default();
    for chunk in meta["groundingChunks"].as_array().into_iter().flatten() {
        out.add(chunk.pointer("/web/uri").and_then(Value::as_str), chunk.pointer("/web/title").and_then(Value::as_str));
    }
    let widget = meta.pointer("/searchEntryPoint/renderedContent").and_then(Value::as_str);
    (out.into_vec(), widget.filter(|h| !h.trim().is_empty()).map(str::to_owned))
}

/// OpenRouter: `url_citation` annotations on the message, nested or flat.
pub(crate) fn chat_sources(message: &Value) -> Vec<Source> {
    let mut out = SourceList::default();
    for a in message["annotations"].as_array().into_iter().flatten().filter(|a| a["type"] == "url_citation") {
        out.add_value(a.get("url_citation").filter(|c| c.is_object()).unwrap_or(a));
    }
    out.into_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn urls(sources: &[Source]) -> Vec<&str> {
        sources.iter().map(|s| s.url.as_str()).collect()
    }

    #[test]
    fn uses_are_clamped() {
        assert_eq!(WebSearch { max_uses: 0 }.uses(), 1);
        assert_eq!(WebSearch { max_uses: 4 }.uses(), 4);
        assert_eq!(WebSearch { max_uses: 50 }.uses(), MAX_WEB_SEARCHES);
    }

    #[test]
    fn requests_add_the_tool_without_losing_the_client_ones() {
        let mut b = json!({ "tools": [{ "name": "f" }] });
        anthropic_request(&mut b, &WebSearch { max_uses: 3 });
        assert_eq!(b["tools"][0]["name"], "f");
        assert_eq!(b["tools"][1], json!({ "type": "web_search_20250305", "name": "web_search", "max_uses": 3 }));
        let mut none = json!({});
        gemini_request(&mut none);
        assert_eq!(none["tools"], json!([{ "google_search": {} }]));
        let mut openai = json!({ "include": ["reasoning.encrypted_content"] });
        openai_request(&mut openai, &WebSearch { max_uses: 2 });
        assert_eq!(openai["include"], json!(["reasoning.encrypted_content", "web_search_call.action.sources"]));
        assert_eq!(openai["max_tool_calls"], 2);
        let mut xai = json!({ "include": ["reasoning.encrypted_content"], "store": false });
        xai_request(&mut xai);
        assert!(xai.get("include").is_none() && xai.get("max_tool_calls").is_none());
        assert_eq!(xai["tools"], json!([{ "type": "web_search" }]));
    }

    #[test]
    fn anthropic_sources_dedupe_and_skip_errors() {
        let blocks = vec![
            json!({"type": "web_search_tool_result", "content": [
                {"type": "web_search_result", "url": "https://www.example.com/b", "title": "B"},
                {"type": "web_search_result", "url": "https://www.example.com/a", "title": "A again"}]}),
            json!({"type": "web_search_tool_result", "content": {"type": "web_search_tool_result_error", "error_code": "max_uses_exceeded"}}),
            json!({"type": "text", "text": "x", "citations": [
                {"type": "web_search_result_location", "url": "https://www.example.com/a", "title": "A"},
                {"url": ""}]}),
        ];
        let got = anthropic_sources(&blocks);
        assert_eq!(urls(&got), ["https://www.example.com/a", "https://www.example.com/b"]);
        assert_eq!(got[0].title, "A");
    }

    #[test]
    fn a_missing_title_falls_back_to_the_url() {
        let items =
            vec![json!({"type": "web_search_call", "action": {"sources": [{"url": "https://news.example.org/x"}]}})];
        let got = responses_sources(
            &items,
            &json!({"citations": ["https://news.example.org/x", "https://www.example.com/y"]}),
        );
        assert_eq!(urls(&got), ["https://news.example.org/x", "https://www.example.com/y"]);
        assert_eq!(got[0].title, "https://news.example.org/x");
    }

    #[test]
    fn responses_read_annotations_and_object_citations() {
        let items = vec![json!({"type": "message", "content": [{"type": "output_text", "text": "t", "annotations": [
            {"type": "url_citation", "url": "https://www.example.com/1", "title": "One"},
            {"type": "file_citation", "url": "https://www.example.com/no"}]}]})];
        let got =
            responses_sources(&items, &json!({"citations": [{"url": "https://www.example.com/2", "title": "Two"}]}));
        assert_eq!(urls(&got), ["https://www.example.com/1", "https://www.example.com/2"]);
    }

    #[test]
    fn gemini_grounding_reads_chunks_and_the_widget() {
        let answer = json!({"candidates": [{"groundingMetadata": {
            "groundingChunks": [{"web": {"uri": "https://www.example.com/g", "title": "G"}}],
            "searchEntryPoint": {"renderedContent": "<div>Google</div>"}}}]});
        let (sources, widget) = gemini_grounding(&answer);
        assert_eq!(urls(&sources), ["https://www.example.com/g"]);
        assert_eq!(widget.as_deref(), Some("<div>Google</div>"));
        let empty = json!({"candidates": [{"groundingMetadata": {"searchEntryPoint": {"renderedContent": " "}}}]});
        assert_eq!(gemini_grounding(&empty), (Vec::new(), None));
        assert_eq!(gemini_grounding(&json!({})), (Vec::new(), None));
    }

    #[test]
    fn chat_annotations_nested_or_flat() {
        let msg = json!({"annotations": [
            {"type": "url_citation", "url_citation": {"url": "https://www.example.com/n", "title": "N"}},
            {"type": "url_citation", "url": "https://www.example.com/f", "title": "F"}]});
        assert_eq!(urls(&chat_sources(&msg)), ["https://www.example.com/n", "https://www.example.com/f"]);
    }
}
