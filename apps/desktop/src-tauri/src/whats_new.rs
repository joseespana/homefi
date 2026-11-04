//! "What's new" after an update: the CHANGELOG notes, once per version.
//!
//! The rules and the parser live in `homefi_core::update`; this is the thin
//! Tauri layer plus the per-launch switch. The desktop UI is English only.

use std::sync::atomic::{AtomicBool, Ordering};

use homefi_core::humane::Lang;
use homefi_core::update::{self, ReleaseNotes};
use tauri::{AppHandle, Manager};

use crate::commands::AppState;
use crate::settings::DesktopSettings;

/// What the What's new dialog needs from this launch.
pub struct WhatsNew {
    /// `desktop.json` existed before this launch: an update, or a run of an older HomeFi
    /// (1.5.1 and older have no `last_seen_version`).
    upgraded: bool,
    /// The decision is made once per launch (a webview reload must not show it again).
    checked: AtomicBool,
}

impl WhatsNew {
    pub fn new(upgraded: bool) -> Self {
        Self { upgraded, checked: AtomicBool::new(false) }
    }
}

/// The notes to show and the version to record (`None` = already recorded).
fn decide(settings: &DesktopSettings, upgraded: bool) -> (Vec<ReleaseNotes>, Option<String>) {
    let plan = update::whats_new(settings.last_seen_version.as_deref(), upgraded, settings.updates.whats_new, Lang::En);
    let to_record =
        (settings.last_seen_version.as_deref() != Some(plan.record_version.as_str())).then_some(plan.record_version);
    (plan.notes, to_record)
}

/// The notes to show now (empty = nothing). The first call per launch decides and records
/// `last_seen_version`; later calls return [].
#[tauri::command]
pub fn whats_new_check(app: AppHandle) -> Vec<ReleaseNotes> {
    let gate = app.state::<WhatsNew>();
    if gate.checked.swap(true, Ordering::AcqRel) {
        return Vec::new();
    }
    let state = app.state::<AppState>();
    let (notes, to_record) = decide(&state.settings(), gate.upgraded);
    if let Some(version) = to_record {
        if state.save_settings(|s| s.last_seen_version = Some(version)).is_err() {
            tracing::warn!("couldn't record the version whose notes were shown");
        }
    }
    notes
}

/// Settings → About → What's new: the notes of the version running.
#[tauri::command]
pub fn whats_new_latest() -> Option<ReleaseNotes> {
    update::latest_notes(Lang::En)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(last_seen: Option<&str>, enabled: bool) -> DesktopSettings {
        let mut s = DesktopSettings { last_seen_version: last_seen.map(str::to_owned), ..Default::default() };
        s.updates.whats_new = enabled;
        s
    }

    #[test]
    fn a_fresh_install_shows_nothing_and_records() {
        let (notes, record) = decide(&settings(None, true), false);
        assert!(notes.is_empty());
        assert_eq!(record.as_deref(), Some(homefi_core::VERSION));
    }

    #[test]
    fn an_upgrade_from_unknown_shows_the_latest() {
        let (notes, record) = decide(&settings(None, true), true);
        assert_eq!(notes.len(), 1);
        assert_eq!(record.as_deref(), Some(homefi_core::VERSION));
    }

    #[test]
    fn an_already_seen_version_shows_nothing_and_records_nothing() {
        let (notes, record) = decide(&settings(Some(homefi_core::VERSION), true), true);
        assert!(notes.is_empty());
        assert_eq!(record, None);
    }

    #[test]
    fn disabled_shows_nothing_but_records() {
        let (notes, record) = decide(&settings(Some("1.0.0"), false), true);
        assert!(notes.is_empty());
        assert_eq!(record.as_deref(), Some(homefi_core::VERSION));
    }
}
