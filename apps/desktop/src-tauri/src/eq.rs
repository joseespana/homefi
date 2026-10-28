//! Equalizer commands (`src/lib/api.ts` → "equalizer"): the settings, curves
//! for drawing and AutoEq headphone corrections.

use homefi_core::dsp::autoeq::{AutoEq, HeadphoneEntry};
use homefi_core::dsp::{self, EqProfile, EqSettings, Tilt};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::{err, AppState};

type CmdResult<T> = Result<T, String>;

/// AutoEq downloads (index and profiles), inside the app dir.
pub const AUTOEQ_CACHE: &str = "autoeq-cache";
/// Most points a curve may ask for.
const MAX_POINTS: usize = 4096;

/// Applies `eq` to the player right away (the party player follows on its next tick).
pub fn apply(app: &AppHandle, eq: &EqSettings) {
    app.state::<AppState>().player.set_eq(eq.clone());
}

#[tauri::command(async)]
pub fn eq_get(state: State<'_, AppState>) -> EqSettings {
    state.settings().eq
}

/// Saves and applies the equalizer settings (values are brought in range).
#[tauri::command(async)]
pub fn eq_set(app: AppHandle, settings: EqSettings) -> CmdResult<EqSettings> {
    let state = app.state::<AppState>();
    let eq = settings.clamped();
    let saved = state.save_settings(|s| s.eq = eq)?;
    apply(&app, &saved.eq);
    let _ = app.emit("settings", &saved);
    Ok(saved.eq)
}

/// "Hear without EQ": while `on` the player runs flat; nothing is saved.
#[tauri::command(async)]
pub fn eq_bypass(app: AppHandle, on: bool) {
    let mut eq = app.state::<AppState>().settings().eq;
    if on {
        eq.enabled = false;
    }
    apply(&app, &eq);
    crate::sound::bypass(&app, on);
}

/// Response of `profile`'s bands in dB at `freqs` (preamp not included).
#[tauri::command]
pub fn eq_response(profile: EqProfile, freqs: Vec<f32>, sample_rate: u32) -> Vec<f32> {
    let freqs = &freqs[..freqs.len().min(MAX_POINTS)];
    dsp::response_db(&profile, freqs, sample_rate)
}

/// `n` log-spaced frequencies, 20 Hz to 20 kHz.
#[tauri::command]
pub fn eq_log_freqs(n: usize) -> Vec<f32> {
    dsp::log_freqs(n.min(MAX_POINTS))
}

/// Preamp (dB, ≤ 0) that keeps `profile`'s largest boost from clipping.
#[tauri::command]
pub fn eq_auto_preamp(profile: EqProfile) -> f32 {
    dsp::auto_preamp_db(&profile)
}

/// `profile` with taste adjustments (bass, treble, clarity) applied.
#[tauri::command]
pub fn eq_with_taste(profile: EqProfile, tilt: Tilt) -> EqProfile {
    dsp::with_taste(&profile, tilt)
}

/// Headphones in AutoEq matching `query`, best first (works offline from the cache).
#[tauri::command]
pub async fn autoeq_search(autoeq: State<'_, AutoEq>, query: String) -> CmdResult<Vec<HeadphoneEntry>> {
    autoeq.search(&query).await.map_err(err)
}

/// The correction profile of `entry` (downloaded once, then cached).
#[tauri::command]
pub async fn autoeq_profile(autoeq: State<'_, AutoEq>, entry: HeadphoneEntry) -> CmdResult<EqProfile> {
    autoeq.profile(&entry).await.map_err(err)
}
