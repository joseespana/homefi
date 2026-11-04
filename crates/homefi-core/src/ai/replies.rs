//! The tap-able answers of a chat question, shared by every assistant that
//! asks the user things (EQ interview, party playlist chat).

use serde_json::{json, Value};

use super::chat::Tool;

/// Tap-able answers per question.
pub(crate) const MAX_QUICK_REPLIES: usize = 4;
/// Longest quick reply kept (chars).
const MAX_REPLY_CHARS: usize = 48;
/// Name of the tool the model calls to show them.
pub(crate) const SUGGEST_REPLIES: &str = "suggest_replies";

/// The `suggest_replies` tool: the model calls it with every question.
pub(crate) fn suggest_replies_tool() -> Tool {
    Tool {
        name: SUGGEST_REPLIES.into(),
        description:
            "Show 2 to 4 short answers the user can tap for the question you just asked. Call it with every question."
                .into(),
        parameters: json!({
            "type": "object",
            "properties": { "options": { "type": "array", "items": { "type": "string" }, "maxItems": MAX_QUICK_REPLIES } },
            "required": ["options"]
        }),
    }
}

/// The `options` argument cleaned: trimmed, no empties or repeats, short, at most [`MAX_QUICK_REPLIES`].
pub(crate) fn quick_replies(v: Option<&Value>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for o in v.and_then(Value::as_array).into_iter().flatten() {
        let Some(s) = o.as_str().map(str::trim).filter(|s| !s.is_empty()) else { continue };
        let s: String = s.chars().take(MAX_REPLY_CHARS).collect();
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out.truncate(MAX_QUICK_REPLIES);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_replies_are_cleaned() {
        assert_eq!(quick_replies(Some(&json!(["a", " a ", "", "b", "c", "d", "e"]))), ["a", "b", "c", "d"]);
        assert!(quick_replies(None).is_empty());
    }

    #[test]
    fn the_tool_is_named_for_the_model() {
        assert_eq!(suggest_replies_tool().name, "suggest_replies");
    }
}
