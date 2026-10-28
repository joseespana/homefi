//! AI failures, sorted the way people need them: a wrong key, no credit, the
//! provider is down... Never carries the key: the provider's own text is
//! scrubbed of it before it is kept.

use serde_json::Value;

use super::{AiConfig, AiProvider};
use crate::humane::Lang;

/// What went wrong talking to an AI provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiErrorKind {
    /// A key is required and there is none.
    NoKey,
    /// The provider rejected the key (401, or Google's "API key not valid").
    InvalidKey,
    /// The key works but may not do this (403).
    NotAllowed,
    /// No credit left or the plan's quota is used up (402, `insufficient_quota`).
    Quota,
    /// Too many requests in a short time (429).
    RateLimited,
    /// The model does not exist or the key can't use it.
    ModelNotFound,
    /// Nothing at that address (wrong custom URL).
    NotFound,
    /// The provider refused the request as malformed (400).
    BadRequest,
    /// The provider is failing or overloaded (5xx, 529).
    ProviderDown,
    /// No connection (or, for a local server, it isn't running).
    Offline,
    Timeout,
    /// An answer we couldn't read.
    BadAnswer,
}

/// An AI call that failed. `Display` is the technical text for logs;
/// [`AiError::humane_in`] is for screens.
#[derive(Debug, Clone, thiserror::Error)]
#[error("{} ({}): {kind:?}: {detail}", provider.short_name(), model)]
pub struct AiError {
    pub provider: AiProvider,
    pub model: String,
    pub kind: AiErrorKind,
    /// The provider's own words (scrubbed of the key), for the log.
    pub detail: String,
}

impl AiError {
    pub(crate) fn new(config: &AiConfig, kind: AiErrorKind, detail: impl Into<String>) -> Self {
        Self {
            provider: config.provider.clone(),
            model: config.model.clone(),
            kind,
            detail: scrub(detail.into(), config),
        }
    }

    /// From a failed request (no answer at all).
    pub(crate) fn from_http(config: &AiConfig, e: &reqwest::Error) -> Self {
        let kind = if e.is_timeout() {
            AiErrorKind::Timeout
        } else if e.is_decode() {
            AiErrorKind::BadAnswer
        } else {
            AiErrorKind::Offline
        };
        // reqwest's text has the URL, never the key (keys travel in headers)
        Self::new(config, kind, e.to_string())
    }

    /// From an error answer: the status and the provider's JSON (or text) body.
    pub(crate) fn from_status(config: &AiConfig, status: u16, body: &str, listing: bool) -> Self {
        let (message, code) = error_text(body);
        let all = format!("{message} {code}").to_lowercase();
        let key_words =
            ["api key", "api_key", "apikey", "x-api-key", "authentication", "unauthenticated", "invalid key"];
        let about_key = key_words.iter().any(|w| all.contains(w));
        let about_money =
            ["insufficient_quota", "quota", "credit", "billing", "balance", "payment"].iter().any(|w| all.contains(w));
        let about_model = all.contains("model")
            && ["not found", "not_found", "does not exist", "unknown", "invalid model", "not supported"]
                .iter()
                .any(|w| all.contains(w));
        let kind = match status {
            401 => AiErrorKind::InvalidKey,
            402 => AiErrorKind::Quota,
            403 if about_key && !about_money => AiErrorKind::InvalidKey,
            403 if about_money => AiErrorKind::Quota,
            403 => AiErrorKind::NotAllowed,
            404 if listing => AiErrorKind::NotFound,
            404 => AiErrorKind::ModelNotFound,
            429 if about_money => AiErrorKind::Quota,
            429 => AiErrorKind::RateLimited,
            400 | 422 if about_key && (all.contains("invalid") || all.contains("not valid")) => AiErrorKind::InvalidKey,
            400 | 422 if about_money => AiErrorKind::Quota,
            400 | 422 if about_model => AiErrorKind::ModelNotFound,
            408 => AiErrorKind::Timeout,
            500..=599 => AiErrorKind::ProviderDown,
            _ => AiErrorKind::BadRequest,
        };
        let detail = if message.is_empty() { format!("{status}") } else { format!("{status} {message}") };
        Self::new(config, kind, detail)
    }

    /// The message for a screen.
    pub fn humane_in(&self, lang: Lang) -> String {
        let es = lang == Lang::Es;
        let p = match (&self.provider, es) {
            (AiProvider::OpenAiCompatible { .. }, true) => "tu servidor",
            (AiProvider::OpenAiCompatible { .. }, false) => "your server",
            (p, _) => p.short_name(),
        };
        let model = self.model.as_str();
        let (en, sp) = match self.kind {
            AiErrorKind::NoKey => (
                format!("There's no {p} key yet. Add it in Settings."),
                format!("Falta la llave de {p}. Agrégala en Ajustes."),
            ),
            AiErrorKind::InvalidKey => (
                format!("The {p} key isn't valid. Check it in Settings."),
                format!("La llave de {p} no es válida. Revísala en Ajustes."),
            ),
            AiErrorKind::NotAllowed => (
                format!("Your {p} key isn't allowed to do this. Check its permissions."),
                format!("Tu llave de {p} no tiene permiso para esto. Revisa sus permisos."),
            ),
            AiErrorKind::Quota => (
                format!("Your {p} account is out of credit or reached its limit."),
                format!("Tu cuenta de {p} no tiene saldo o llegó a su límite."),
            ),
            AiErrorKind::RateLimited => (
                format!("{p} asked us to slow down. Wait a minute and try again."),
                format!("{p} pidió ir más despacio. Espera un minuto y vuelve a intentarlo."),
            ),
            AiErrorKind::ModelNotFound if !model.is_empty() => (
                format!("{p} doesn't offer the model “{model}”. Pick another one in Settings."),
                format!("{p} no ofrece el modelo «{model}». Elige otro en Ajustes."),
            ),
            AiErrorKind::ModelNotFound => (
                format!("{p} has no model to talk with. Pick one in Settings."),
                format!("{p} no tiene un modelo para conversar. Elige uno en Ajustes."),
            ),
            AiErrorKind::NotFound => (
                format!("Nothing answers at {p}'s address. Check it in Settings."),
                format!("No hay nada en la dirección de {p}. Revísala en Ajustes."),
            ),
            AiErrorKind::BadRequest => (
                format!("{p} didn't accept the request. Try another model."),
                format!("{p} no aceptó la petición. Prueba con otro modelo."),
            ),
            AiErrorKind::ProviderDown => (
                format!("{p} is having trouble right now. Try again in a few minutes."),
                format!("{p} está teniendo problemas ahora mismo. Vuelve a intentarlo en unos minutos."),
            ),
            AiErrorKind::Offline if self.provider.is_local() => (
                format!("Couldn't reach {p}. Check that it's running and the address is right."),
                format!("No se pudo conectar con {p}. Revisa que esté abierto y que la dirección sea correcta."),
            ),
            AiErrorKind::Offline => (
                format!("Couldn't reach {p}. Check your internet connection."),
                format!("No se pudo conectar con {p}. Revisa tu conexión a internet."),
            ),
            AiErrorKind::Timeout => (
                format!("{p} is taking too long to answer. Try again in a moment."),
                format!("{p} está tardando demasiado en responder. Vuelve a intentarlo en un momento."),
            ),
            AiErrorKind::BadAnswer => (
                format!("{p} sent something unexpected. Try again or pick another model."),
                format!("{p} respondió algo inesperado. Vuelve a intentarlo o elige otro modelo."),
            ),
        };
        if es {
            sp
        } else {
            en
        }
    }
}

/// The message and code of an error body in any provider's shape:
/// `{"error":{"message","type"|"code"|"status"}}`, `{"message"}`, or plain text.
fn error_text(body: &str) -> (String, String) {
    let Ok(json) = serde_json::from_str::<Value>(body) else {
        return (body.chars().take(300).collect(), String::new());
    };
    // some answer a list of errors (Gemini behind some proxies)
    let json = json.as_array().and_then(|a| a.first().cloned()).unwrap_or(json);
    let err = json.get("error").filter(|e| e.is_object()).unwrap_or(&json);
    let text = |v: Option<&Value>| match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    };
    let mut message = text(err.get("message"));
    if message.is_empty() {
        message = text(json.get("error")).chars().take(300).collect();
    }
    let mut code = [text(err.get("type")), text(err.get("code")), text(err.get("status"))].join(" ");
    // Google puts the precise reason (API_KEY_INVALID) in details
    if let Some(details) = err.get("details").and_then(Value::as_array) {
        for d in details {
            code.push(' ');
            code.push_str(&text(d.get("reason")));
        }
    }
    (message, code)
}

/// Removes the key from a provider's text (some echo part of it back).
fn scrub(text: String, config: &AiConfig) -> String {
    match config.key() {
        Some(key) if key.len() >= 4 && text.contains(key) => text.replace(key, "<redacted>"),
        _ => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(p: AiProvider) -> AiConfig {
        AiConfig::new(p, "m1", Some("sk-secret-123".into()))
    }

    #[test]
    fn sorts_every_provider_error_shape() {
        let anth = r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#;
        assert_eq!(AiError::from_status(&cfg(AiProvider::Anthropic), 401, anth, false).kind, AiErrorKind::InvalidKey);
        let oai = r#"{"error":{"message":"You exceeded your current quota","type":"insufficient_quota","code":"insufficient_quota"}}"#;
        assert_eq!(AiError::from_status(&cfg(AiProvider::OpenAi), 429, oai, false).kind, AiErrorKind::Quota);
        let gem = r#"{"error":{"code":400,"message":"API key not valid. Please pass a valid API key.","status":"INVALID_ARGUMENT","details":[{"@type":"type.googleapis.com/google.rpc.ErrorInfo","reason":"API_KEY_INVALID"}]}}"#;
        assert_eq!(AiError::from_status(&cfg(AiProvider::Gemini), 400, gem, false).kind, AiErrorKind::InvalidKey);
        let busy = r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#;
        assert_eq!(AiError::from_status(&cfg(AiProvider::Anthropic), 529, busy, false).kind, AiErrorKind::ProviderDown);
        let model = r#"{"error":{"message":"The model `gpt-x` does not exist","code":"model_not_found"}}"#;
        assert_eq!(AiError::from_status(&cfg(AiProvider::Groq), 404, model, false).kind, AiErrorKind::ModelNotFound);
        assert_eq!(
            AiError::from_status(&cfg(AiProvider::Groq), 429, "slow down", false).kind,
            AiErrorKind::RateLimited
        );
        assert_eq!(
            AiError::from_status(&cfg(AiProvider::Ollama), 404, "404 page not found", true).kind,
            AiErrorKind::NotFound
        );
    }

    #[test]
    fn the_key_is_scrubbed_and_messages_are_plain() {
        let echo = r#"{"error":{"message":"Incorrect API key provided: sk-secret-123"}}"#;
        let e = AiError::from_status(&cfg(AiProvider::OpenAi), 401, echo, false);
        assert!(!e.to_string().contains("sk-secret-123"));
        assert_eq!(e.humane_in(Lang::Es), "La llave de OpenAI no es válida. Revísala en Ajustes.");
        assert!(!e.humane_in(Lang::En).contains("401"));
        let local = AiError::new(&cfg(AiProvider::Ollama), AiErrorKind::Offline, "connection refused");
        assert!(local.humane_in(Lang::En).contains("running"));
    }
}
