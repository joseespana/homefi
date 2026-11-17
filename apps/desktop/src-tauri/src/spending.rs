//! What the AI costs (`src/lib/api.ts` → "AI spending"): the history, the breakdown, "ask before
//! spending" and the monthly limit. All of it lives in the core's ledger
//! ([`homefi_core::ai::usage`]), opened at startup; this is only the bridge.

use homefi_core::ai::usage::{self, AiFeature, SpendDecision, SpendGuard, SpendSummary};
use homefi_core::humane::Lang;
use tauri::AppHandle;

use crate::commands::err;

type CmdResult<T> = Result<T, String>;

fn ledger() -> CmdResult<std::sync::Arc<usage::Ledger>> {
    usage::ledger().ok_or_else(|| "The AI spending history isn't ready yet.".to_owned())
}

/// The month's total, the breakdown per feature and the history, in `language`.
#[tauri::command]
pub fn ai_spending_summary(language: String) -> CmdResult<SpendSummary> {
    Ok(ledger()?.summary(Lang::from_code(&language)))
}

/// Asked before an AI action: allow, confirm (with the words to show) or blocked by the monthly limit.
#[tauri::command]
pub fn ai_spend_check(feature: AiFeature, language: String) -> SpendDecision {
    match usage::ledger() {
        Some(ledger) => ledger.check(feature, Lang::from_code(&language)),
        None => SpendDecision::Allow,
    }
}

/// Saves "ask before spending" and the monthly limit (clamped to their ranges).
#[tauri::command]
pub fn ai_spending_set_guard(guard: SpendGuard) -> CmdResult<()> {
    ledger()?.set_guard(guard).map_err(err)
}

/// Empties the history; the guard stays.
#[tauri::command]
pub fn ai_spending_clear() -> CmdResult<()> {
    ledger()?.clear().map_err(err)
}

/// What's left on the AI account in USD (OpenRouter); `None` for providers that don't say.
/// Read-only, spends nothing.
#[tauri::command]
pub async fn ai_balance(app: AppHandle) -> CmdResult<Option<f64>> {
    let Some(config) = crate::ai::settings_config(&app).await else {
        return Ok(None);
    };
    homefi_core::ai::balance(&config).await.map_err(err)
}
