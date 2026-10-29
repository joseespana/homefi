//! Bring-your-own-key AI (`src/lib/api.ts` → "AI"): providers, keys in the OS
//! keychain (macOS Keychain, Windows Credential Manager, Linux Secret
//! Service), model lists and the EQ assistant.
//!
//! Keys never touch `desktop.json`, the log or the web UI: the UI may send a
//! key (to save or test it) but only ever gets "set / not set" back. The
//! chosen provider and model live in the settings (`ai`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use homefi_core::ai::{
    self, AiConfig, AiProvider, AssistantContext, AssistantTurn, AudioOutput, EqSession, ModelInfo, OutputKind,
    ProviderInfo,
};
use homefi_core::dsp::autoeq::AutoEq;
use homefi_player::list_devices;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::{err, AppState};
use crate::settings::AiSettings;

type CmdResult<T> = Result<T, String>;

/// Keychain service of the entries; the account is the provider id.
const SERVICE: &str = "com.perseusoft.homefi.ai";

/// AI state of the app: keys read once from the keychain, the conversation.
pub struct Ai {
    /// Provider id → key (or `None`: not in the keychain), so the keychain
    /// is asked once per provider and run.
    keys: Mutex<HashMap<String, Option<String>>>,
    session: tokio::sync::Mutex<Option<EqSession>>,
    autoeq: Arc<AutoEq>,
}

impl Ai {
    pub fn new(app_dir: &std::path::Path) -> Self {
        Self {
            keys: Mutex::default(),
            session: tokio::sync::Mutex::new(None),
            autoeq: Arc::new(AutoEq::new(app_dir.join(crate::eq::AUTOEQ_CACHE))),
        }
    }
}

fn entry(provider: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, provider).map_err(keychain_error)
}

/// Keychain failures in plain words (the technical text goes to the log, never a key).
fn keychain_error(e: keyring::Error) -> String {
    tracing::warn!("keychain: {e}");
    if homefi_core::humane::language() == homefi_core::humane::Lang::Es {
        "No se pudo usar el llavero del sistema para guardar la llave.".into()
    } else {
        "Couldn't use the system keychain to keep the key.".into()
    }
}

fn provider(id: &str, base_url: Option<&str>) -> CmdResult<AiProvider> {
    AiProvider::from_id(id, base_url).ok_or_else(|| format!("unknown AI provider `{id}`"))
}

/// The stored key of `provider`, from memory or the keychain (off the async threads).
async fn stored_key(ai: &Ai, provider: &str) -> CmdResult<Option<String>> {
    if let Some(known) = ai.keys.lock().unwrap().get(provider) {
        return Ok(known.clone());
    }
    let id = provider.to_owned();
    let key = tauri::async_runtime::spawn_blocking(move || match entry(&id)?.get_password() {
        Ok(k) => Ok(Some(k)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(keychain_error(e)),
    })
    .await
    .map_err(err)??;
    ai.keys.lock().unwrap().insert(provider.to_owned(), key.clone());
    Ok(key)
}

/// The call config for `provider` (the key from the keychain unless `key` is given).
async fn config(
    ai: &Ai,
    provider_id: &str,
    model: &str,
    base_url: Option<String>,
    key: Option<String>,
) -> CmdResult<AiConfig> {
    let base_url = base_url.map(|u| u.trim().to_owned()).filter(|u| !u.is_empty());
    let provider = provider(provider_id, base_url.as_deref())?;
    let key = match key.map(|k| k.trim().to_owned()).filter(|k| !k.is_empty()) {
        Some(k) => Some(k),
        None => stored_key(ai, provider.id()).await?,
    };
    let mut config = AiConfig::new(provider, model.trim(), key);
    config.base_url = base_url;
    Ok(config)
}

#[tauri::command]
pub fn ai_providers() -> Vec<ProviderInfo> {
    ai::providers()
}

/// Saves `key` for `provider` in the OS keychain.
#[tauri::command]
pub async fn ai_key_set(ai: State<'_, Ai>, provider: String, key: String) -> CmdResult<()> {
    let id = self::provider(&provider, None)?.id().to_owned();
    let key = key.trim().to_owned();
    if key.is_empty() {
        return Err("empty key".into());
    }
    let (account, secret) = (id.clone(), key.clone());
    tauri::async_runtime::spawn_blocking(move || entry(&account)?.set_password(&secret).map_err(keychain_error))
        .await
        .map_err(err)??;
    ai.keys.lock().unwrap().insert(id, Some(key));
    Ok(())
}

/// Forgets the key of `provider`.
#[tauri::command]
pub async fn ai_key_clear(ai: State<'_, Ai>, provider: String) -> CmdResult<()> {
    let id = self::provider(&provider, None)?.id().to_owned();
    let account = id.clone();
    tauri::async_runtime::spawn_blocking(move || match entry(&account)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(keychain_error(e)),
    })
    .await
    .map_err(err)??;
    ai.keys.lock().unwrap().insert(id, None);
    Ok(())
}

/// Which providers have a key saved (never the keys themselves).
#[tauri::command]
pub async fn ai_key_status(ai: State<'_, Ai>) -> CmdResult<HashMap<String, bool>> {
    let mut status = HashMap::new();
    for p in AiProvider::all() {
        let set = stored_key(&ai, p.id()).await?.is_some();
        status.insert(p.id().to_owned(), set);
    }
    Ok(status)
}

/// The chat models the key can use.
#[tauri::command]
pub async fn ai_list_models(
    ai: State<'_, Ai>,
    provider: String,
    base_url: Option<String>,
) -> CmdResult<Vec<ModelInfo>> {
    let config = config(&ai, &provider, "", base_url, None).await?;
    ai::list_models(&config).await.map_err(err)
}

/// Checks a key without spending tokens: `key` when given (before saving it),
/// else the saved one.
#[tauri::command]
pub async fn ai_test_key(
    ai: State<'_, Ai>,
    provider: String,
    base_url: Option<String>,
    key: Option<String>,
) -> CmdResult<()> {
    let config = config(&ai, &provider, "", base_url, key).await?;
    ai::test_key(&config).await.map_err(err)
}

/// The model used when the settings name none.
#[tauri::command]
pub async fn ai_default_model(ai: State<'_, Ai>, provider: String, base_url: Option<String>) -> CmdResult<String> {
    let config = config(&ai, &provider, "", base_url, None).await?;
    ai::resolve_model(&config).await.map_err(err)
}

// -------------------------------------------------------------- EQ assistant

/// What an output is, from its name (the OS gives nothing better).
pub(crate) fn guess_kind(name: &str) -> OutputKind {
    let n = name.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    if has(&["airpods", "bluetooth", "wh-", "wf-", "buds", "beats", "bose", "jbl", "sony", "jabra", "galaxy"]) {
        OutputKind::Bluetooth
    } else if has(&["hdmi", "displayport", "display audio", "tv"]) {
        OutputKind::Hdmi
    } else if has(&[
        "usb",
        "dac",
        "fiio",
        "topping",
        "schiit",
        "dragonfly",
        "focusrite",
        "scarlett",
        "motu",
        "audient",
        "chord",
    ]) {
        OutputKind::UsbDac
    } else if has(&["headphone", "auricular", "audífono", "external headphones"]) {
        OutputKind::Headphones
    } else if has(&["speaker", "bocina", "altavoz", "macbook", "built-in", "realtek", "internal"]) {
        OutputKind::BuiltIn
    } else {
        OutputKind::Other
    }
}

async fn context(
    app: &AppHandle,
    language: &str,
    notes: Option<String>,
    focus_rule: Option<String>,
) -> AssistantContext {
    let listening = crate::sound::summary(app).await;
    let state = app.state::<AppState>();
    let settings = state.settings();
    let playing = state.player.snapshot().stream.map(|s| s.device);
    let now = crate::sound::sound_now(app.clone());
    // asking the audio system for its devices blocks: not on an async worker
    let outputs = tokio::task::spawn_blocking(list_devices)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|d| AudioOutput {
            kind: guess_kind(&d.name),
            id: d.name.clone(),
            name: d.name,
            sample_rates: d.sample_rates,
        })
        .collect();
    AssistantContext {
        language: language.to_owned(),
        outputs,
        current: settings.eq,
        notes: notes.filter(|n| !n.trim().is_empty()),
        taste: settings.taste,
        loudness: settings.loudness,
        listening: Some(listening).filter(|l| !l.is_empty()),
        now_playing: now.context,
        active_output: playing.or(settings.output_device),
        focus_rule: focus_rule.filter(|r| !r.trim().is_empty()),
    }
}

/// The AI of the settings with its key; `None` when none is set up (or the keychain fails).
pub(crate) async fn settings_config(app: &AppHandle) -> Option<AiConfig> {
    let settings: AiSettings = app.state::<AppState>().settings().ai;
    if settings.provider.is_empty() {
        return None;
    }
    let ai = app.state::<Ai>();
    let config = config(&ai, &settings.provider, &settings.model, Some(settings.base_url.clone()), None).await.ok()?;
    (!config.provider.needs_key() || config.api_key.is_some()).then_some(config)
}

/// A proactive taste suggestion: opens a conversation (AI or guided) whose
/// answers go to [`eq_assistant_reply`].
pub(crate) async fn suggest(app: &AppHandle, language: &str) -> CmdResult<AssistantTurn> {
    let ai = app.state::<Ai>();
    let ctx = context(app, language, None, None).await;
    let mut session = match settings_config(app).await {
        Some(config) => EqSession::ai(config, ctx),
        None => EqSession::guided(ctx),
    }
    .with_autoeq(ai.autoeq.clone());
    let turn = session.suggest().await.map_err(err)?;
    *ai.session.lock().await = Some(session);
    Ok(turn)
}

/// Saves a confirmed proposal as its output's profile and turns the equalizer on.
fn apply_confirmed(app: &AppHandle, turn: &AssistantTurn) -> CmdResult<()> {
    let state = app.state::<AppState>();
    if turn.taste.is_some() || turn.loudness.is_some() {
        let (taste, loudness) = (turn.taste.clone(), turn.loudness);
        let saved = state.save_settings(|s| {
            if let Some(t) = taste {
                s.taste = t.clamped();
            }
            if let Some(l) = loudness {
                s.loudness = l.clamped();
            }
        })?;
        state.player.set_taste(saved.taste.clone());
        state.player.set_loudness(saved.loudness);
        let _ = app.emit("settings", &saved);
    }
    let Some(p) = turn.proposal.as_ref().filter(|_| turn.confirmed) else { return Ok(()) };
    let saved = state.save_settings(|s| {
        s.eq.per_output.insert(p.output_id.clone(), p.profile.clone());
        s.eq.enabled = true;
        s.eq.auto_preamp = true;
    })?;
    crate::eq::apply(app, &saved.eq);
    let _ = app.emit("settings", &saved);
    Ok(())
}

/// Starts a conversation: with the AI of the settings, or the guided wizard
/// when `guided` is set or no provider is chosen. `language` is the UI's
/// (`es`, `en`); `notes` anything the user wants to tell up front;
/// `focus_rule` the id of the taste rule to refine ("Ask the AI to refine").
#[tauri::command]
pub async fn eq_assistant_start(
    app: AppHandle,
    guided: bool,
    language: String,
    notes: Option<String>,
    focus_rule: Option<String>,
) -> CmdResult<AssistantTurn> {
    let ai = app.state::<Ai>();
    let ctx = context(&app, &language, notes, focus_rule).await;
    let settings: AiSettings = app.state::<AppState>().settings().ai;
    let mut session = if guided || settings.provider.is_empty() {
        EqSession::guided(ctx)
    } else {
        let base_url = Some(settings.base_url.clone());
        EqSession::ai(config(&ai, &settings.provider, &settings.model, base_url, None).await?, ctx)
    }
    .with_autoeq(ai.autoeq.clone());
    let turn = session.start().await.map_err(err)?;
    *ai.session.lock().await = Some(session);
    Ok(turn)
}

/// The user's answer. A confirmed proposal is saved and applied right away.
#[tauri::command]
pub async fn eq_assistant_reply(app: AppHandle, text: String) -> CmdResult<AssistantTurn> {
    let ai = app.state::<Ai>();
    let mut slot = ai.session.lock().await;
    let Some(session) = slot.as_mut() else {
        return Err("the assistant isn't open".into());
    };
    let turn = session.reply(&text).await.map_err(err)?;
    apply_confirmed(&app, &turn)?;
    Ok(turn)
}

/// Closes the conversation.
#[tauri::command]
pub async fn eq_assistant_close(ai: State<'_, Ai>) -> CmdResult<()> {
    *ai.session.lock().await = None;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_settings_have_no_ai_and_keys_never_land_there() {
        let old: crate::settings::DesktopSettings = serde_json::from_str(r#"{"theme":"dark"}"#).unwrap();
        assert_eq!(old.ai, AiSettings::default());
        let mut s = old.clone();
        s.ai = AiSettings { provider: "gemini".into(), model: "gemini-x".into(), base_url: String::new() };
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["ai"], serde_json::json!({"provider": "gemini", "model": "gemini-x", "base_url": ""}));
    }

    #[test]
    fn output_kinds_from_names() {
        assert_eq!(guess_kind("WH-1000XM5"), OutputKind::Bluetooth);
        assert_eq!(guess_kind("AirPods Pro"), OutputKind::Bluetooth);
        assert_eq!(guess_kind("FiiO K7"), OutputKind::UsbDac);
        assert_eq!(guess_kind("External Headphones"), OutputKind::Headphones);
        assert_eq!(guess_kind("MacBook Air Speakers"), OutputKind::BuiltIn);
        assert_eq!(guess_kind("LG TV"), OutputKind::Hdmi);
        assert_eq!(guess_kind("Razer Seiren V2 X"), OutputKind::Other);
    }
}
