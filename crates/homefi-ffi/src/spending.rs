//! What the AI costs, for Android and iOS: the spending history, the "ask before spending" guard
//! and the monthly limit ([`homefi_core::ai::usage`]). The ledger lives in the app folder; it is
//! opened by [`crate::HomefiEngine::new`].

use homefi_core::ai::{self, usage};
use homefi_core::humane::Lang;

use crate::ai::AiConfig;
use crate::{HomefiError, Result};

/// The thing the user did that called the AI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AiFeature {
    RideChat,
    RideBuild,
    RideSwap,
    Concert,
    PartyPlaylist,
    ChatPlaylist,
    SoundAssistant,
    SoundReaction,
    Other,
}

impl From<AiFeature> for usage::AiFeature {
    fn from(f: AiFeature) -> Self {
        match f {
            AiFeature::RideChat => Self::RideChat,
            AiFeature::RideBuild => Self::RideBuild,
            AiFeature::RideSwap => Self::RideSwap,
            AiFeature::Concert => Self::Concert,
            AiFeature::PartyPlaylist => Self::PartyPlaylist,
            AiFeature::ChatPlaylist => Self::ChatPlaylist,
            AiFeature::SoundAssistant => Self::SoundAssistant,
            AiFeature::SoundReaction => Self::SoundReaction,
            AiFeature::Other => Self::Other,
        }
    }
}

impl From<usage::AiFeature> for AiFeature {
    fn from(f: usage::AiFeature) -> Self {
        match f {
            usage::AiFeature::RideChat => Self::RideChat,
            usage::AiFeature::RideBuild => Self::RideBuild,
            usage::AiFeature::RideSwap => Self::RideSwap,
            usage::AiFeature::Concert => Self::Concert,
            usage::AiFeature::PartyPlaylist => Self::PartyPlaylist,
            usage::AiFeature::ChatPlaylist => Self::ChatPlaylist,
            usage::AiFeature::SoundAssistant => Self::SoundAssistant,
            usage::AiFeature::SoundReaction => Self::SoundReaction,
            usage::AiFeature::Other => Self::Other,
        }
    }
}

/// How sure a cost is: exact (the provider said it), estimated (shown with "≈") or unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AiCostKind {
    Exact,
    Estimated,
    Unknown,
}

impl From<usage::CostKind> for AiCostKind {
    fn from(k: usage::CostKind) -> Self {
        match k {
            usage::CostKind::Exact => Self::Exact,
            usage::CostKind::Estimated => Self::Estimated,
            usage::CostKind::Unknown => Self::Unknown,
        }
    }
}

/// When to ask before an AI action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AiAskMode {
    Never,
    Always,
    /// When it usually costs more than [`AiSpendGuard::ask_above_usd`].
    Above,
}

/// "Ask before spending" and the monthly limit (`None`: no limit).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct AiSpendGuard {
    pub ask: AiAskMode,
    pub ask_above_usd: f64,
    pub monthly_limit_usd: Option<f64>,
}

impl From<usage::SpendGuard> for AiSpendGuard {
    fn from(g: usage::SpendGuard) -> Self {
        let ask = match g.ask {
            usage::AskMode::Never => AiAskMode::Never,
            usage::AskMode::Always => AiAskMode::Always,
            usage::AskMode::Above => AiAskMode::Above,
        };
        Self { ask, ask_above_usd: g.ask_above_usd, monthly_limit_usd: g.monthly_limit_usd }
    }
}

impl From<AiSpendGuard> for usage::SpendGuard {
    fn from(g: AiSpendGuard) -> Self {
        let ask = match g.ask {
            AiAskMode::Never => usage::AskMode::Never,
            AiAskMode::Always => usage::AskMode::Always,
            AiAskMode::Above => usage::AskMode::Above,
        };
        Self { ask, ask_above_usd: g.ask_above_usd, monthly_limit_usd: g.monthly_limit_usd }
    }
}

/// One line of the per-feature breakdown.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct AiFeatureSpend {
    pub feature: AiFeature,
    pub label: String,
    pub times: u32,
    pub total_usd: f64,
    pub average_usd: Option<f64>,
    pub estimated: bool,
    /// "$0.12 in 4 times · ≈$0.03 each", ready to show.
    pub text: String,
}

/// One history line.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct AiSpendItem {
    pub id: String,
    pub feature: AiFeature,
    pub label: String,
    pub at_ms: i64,
    pub provider: String,
    pub model: String,
    pub calls: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub web_searches: u32,
    pub cost_usd: Option<f64>,
    pub cost_kind: AiCostKind,
    /// "≈$0.04", "$0.04" or "Cost unknown".
    pub cost_text: String,
    /// "model · 3 calls · 12.3k tokens · 2 web searches".
    pub detail: String,
}

/// What the AI menu shows.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct AiSpendSummary {
    pub month_total_usd: f64,
    pub month_estimated: bool,
    pub month_unknown: u32,
    pub month_actions: u32,
    pub month_text: String,
    pub features: Vec<AiFeatureSpend>,
    /// Newest first.
    pub history: Vec<AiSpendItem>,
    pub guard: AiSpendGuard,
}

impl From<usage::SpendSummary> for AiSpendSummary {
    fn from(s: usage::SpendSummary) -> Self {
        Self {
            month_total_usd: s.month_total_usd,
            month_estimated: s.month_estimated,
            month_unknown: s.month_unknown,
            month_actions: s.month_actions,
            month_text: s.month_text,
            features: s
                .features
                .into_iter()
                .map(|f| AiFeatureSpend {
                    feature: f.feature.into(),
                    label: f.label,
                    times: f.times,
                    total_usd: f.total_usd,
                    average_usd: f.average_usd,
                    estimated: f.estimated,
                    text: f.text,
                })
                .collect(),
            history: s
                .history
                .into_iter()
                .map(|h| AiSpendItem {
                    id: h.id,
                    feature: h.feature.into(),
                    label: h.label,
                    at_ms: h.at_ms,
                    provider: h.provider,
                    model: h.model,
                    calls: h.calls,
                    input_tokens: h.input_tokens,
                    output_tokens: h.output_tokens,
                    web_searches: h.web_searches,
                    cost_usd: h.cost_usd,
                    cost_kind: h.cost_kind.into(),
                    cost_text: h.cost_text,
                    detail: h.detail,
                })
                .collect(),
            guard: s.guard.into(),
        }
    }
}

/// What to do before an AI action: go on, ask with `title`/`text`, or say the limit is reached.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum AiSpendDecision {
    Allow,
    Confirm { estimate_usd: Option<f64>, month_total_usd: f64, title: String, text: String },
    Blocked { month_total_usd: f64, limit_usd: f64, title: String, text: String },
}

impl From<usage::SpendDecision> for AiSpendDecision {
    fn from(d: usage::SpendDecision) -> Self {
        match d {
            usage::SpendDecision::Allow => Self::Allow,
            usage::SpendDecision::Confirm { estimate_usd, month_total_usd, prompt, .. } => {
                Self::Confirm { estimate_usd, month_total_usd, title: prompt.title, text: prompt.text }
            }
            usage::SpendDecision::Blocked { month_total_usd, limit_usd, prompt } => {
                Self::Blocked { month_total_usd, limit_usd, title: prompt.title, text: prompt.text }
            }
        }
    }
}

fn not_open() -> HomefiError {
    HomefiError::Failed { message: "The AI spending history isn't ready yet.".into() }
}

/// The month's total, the breakdown and the history, in `language`.
#[uniffi::export]
pub fn ai_spending_summary(language: String) -> AiSpendSummary {
    let lang = Lang::from_code(&language);
    match usage::ledger() {
        Some(ledger) => ledger.summary(lang).into(),
        None => AiSpendSummary {
            month_total_usd: 0.0,
            month_estimated: false,
            month_unknown: 0,
            month_actions: 0,
            month_text: "$0.00".into(),
            features: Vec::new(),
            history: Vec::new(),
            guard: usage::SpendGuard::default().into(),
        },
    }
}

/// Ask before starting `feature`: allow, confirm (with the words) or blocked by the monthly limit.
#[uniffi::export]
pub fn ai_spend_check(feature: AiFeature, language: String) -> AiSpendDecision {
    match usage::ledger() {
        Some(ledger) => ledger.check(feature.into(), Lang::from_code(&language)).into(),
        None => AiSpendDecision::Allow,
    }
}

/// Saves "ask before spending" and the monthly limit (numbers are clamped to their ranges).
#[uniffi::export]
pub fn ai_spending_set_guard(guard: AiSpendGuard) -> Result<()> {
    Ok(usage::ledger().ok_or_else(not_open)?.set_guard(guard.into())?)
}

/// Empties the spending history (the guard stays).
#[uniffi::export]
pub fn ai_spending_clear() -> Result<()> {
    Ok(usage::ledger().ok_or_else(not_open)?.clear()?)
}

/// What's left on the account in USD, for providers that say it (OpenRouter); `None` otherwise.
/// Read-only, spends nothing.
#[uniffi::export]
pub async fn ai_balance(config: AiConfig, api_key: Option<String>) -> Result<Option<f64>> {
    crate::runtime::run(async move { Ok(ai::balance(&crate::ai::core_config(config, api_key)).await?) }).await
}

/// The reaction asks the AI (when one is set up): "perfect" and the loudness ones never do, so they
/// need no "ask before spending".
#[uniffi::export]
pub fn sound_reaction_uses_ai(reaction: crate::ai::SoundReaction) -> bool {
    ai::SoundReaction::from(reaction).uses_ai()
}

/// "$1.20" as the spending screens write money.
#[uniffi::export]
pub fn ai_usd_text(usd: f64) -> String {
    usage::usd_text(usd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_feature_maps_both_ways() {
        for f in usage::AiFeature::ALL {
            assert_eq!(usage::AiFeature::from(AiFeature::from(f)), f);
        }
        let g = usage::SpendGuard { ask: usage::AskMode::Above, ask_above_usd: 0.2, monthly_limit_usd: Some(3.0) };
        assert_eq!(usage::SpendGuard::from(AiSpendGuard::from(g)), g);
    }
}
