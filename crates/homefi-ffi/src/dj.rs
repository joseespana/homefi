//! The Android "simple DJ" for the Kotlin side: analysis of a song (BPM, beat grid, overview,
//! loudness), the channel EQ, the crossfader curves, sync and the sampler sounds.
//!
//! Everything is `homefi_core::dj`; this file only converts and forwards, so the sound and the
//! defaults are the same as on desktop. The two ExoPlayers, the pads and the screen are Kotlin.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use homefi_core::dj as core_dj;
use homefi_core::dj::mixer_math::{self, EqGains};
use homefi_core::dj::{AnalysisCache, Grid};
use homefi_core::dsp;

use crate::eq::Biquad;
use crate::HomefiEngine;

/// Rate of the pads' sounds when the caller passes 0.
const SFX_RATE_HZ: u32 = 44_100;
const PCM_BITS: u16 = 16;
const WAV_HEADER_BYTES: usize = 44;
/// Cache folder of the analyses, inside the app dir.
const ANALYSIS_DIR: [&str; 2] = ["dj", "analysis"];

/// Why a song could not be prepared. `message` is plain words; the app shows its own text
/// per variant.
#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum DjError {
    /// The file could not be opened or decoded.
    #[error("{message}")]
    Open { message: String },
    /// The song does not fit in the memory limit.
    #[error("{message}")]
    Memory { message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum CrossfaderCurve {
    Smooth,
    Linear,
    Cut,
}

impl From<core_dj::CrossfaderCurve> for CrossfaderCurve {
    fn from(c: core_dj::CrossfaderCurve) -> Self {
        match c {
            core_dj::CrossfaderCurve::Smooth => Self::Smooth,
            core_dj::CrossfaderCurve::Linear => Self::Linear,
            core_dj::CrossfaderCurve::Cut => Self::Cut,
        }
    }
}

impl From<CrossfaderCurve> for core_dj::CrossfaderCurve {
    fn from(c: CrossfaderCurve) -> Self {
        match c {
            CrossfaderCurve::Smooth => Self::Smooth,
            CrossfaderCurve::Linear => Self::Linear,
            CrossfaderCurve::Cut => Self::Cut,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum LoadTarget {
    FreeDeck,
    Ask,
    Crate,
}

impl From<core_dj::LoadTarget> for LoadTarget {
    fn from(t: core_dj::LoadTarget) -> Self {
        match t {
            core_dj::LoadTarget::FreeDeck => Self::FreeDeck,
            core_dj::LoadTarget::Ask => Self::Ask,
            core_dj::LoadTarget::Crate => Self::Crate,
        }
    }
}

impl From<LoadTarget> for core_dj::LoadTarget {
    fn from(t: LoadTarget) -> Self {
        match t {
            LoadTarget::FreeDeck => Self::FreeDeck,
            LoadTarget::Ask => Self::Ask,
            LoadTarget::Crate => Self::Crate,
        }
    }
}

/// The eight sampler sounds, in pad order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum SfxKind {
    AirHorn,
    Siren,
    Laser,
    Rewind,
    Drop,
    Scratch,
    Riser,
    Hit,
}

impl From<core_dj::SfxKind> for SfxKind {
    fn from(k: core_dj::SfxKind) -> Self {
        match k {
            core_dj::SfxKind::AirHorn => Self::AirHorn,
            core_dj::SfxKind::Siren => Self::Siren,
            core_dj::SfxKind::Laser => Self::Laser,
            core_dj::SfxKind::Rewind => Self::Rewind,
            core_dj::SfxKind::Drop => Self::Drop,
            core_dj::SfxKind::Scratch => Self::Scratch,
            core_dj::SfxKind::Riser => Self::Riser,
            core_dj::SfxKind::Hit => Self::Hit,
        }
    }
}

impl From<SfxKind> for core_dj::SfxKind {
    fn from(k: SfxKind) -> Self {
        match k {
            SfxKind::AirHorn => Self::AirHorn,
            SfxKind::Siren => Self::Siren,
            SfxKind::Laser => Self::Laser,
            SfxKind::Rewind => Self::Rewind,
            SfxKind::Drop => Self::Drop,
            SfxKind::Scratch => Self::Scratch,
            SfxKind::Riser => Self::Riser,
            SfxKind::Hit => Self::Hit,
        }
    }
}

/// How big Homifito is on the DJ screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum HomifitoMode {
    Off,
    Booth,
    Big,
}

impl From<core_dj::HomifitoMode> for HomifitoMode {
    fn from(m: core_dj::HomifitoMode) -> Self {
        match m {
            core_dj::HomifitoMode::Off => Self::Off,
            core_dj::HomifitoMode::Booth => Self::Booth,
            core_dj::HomifitoMode::Big => Self::Big,
        }
    }
}

impl From<HomifitoMode> for core_dj::HomifitoMode {
    fn from(m: HomifitoMode) -> Self {
        match m {
            HomifitoMode::Off => Self::Off,
            HomifitoMode::Booth => Self::Booth,
            HomifitoMode::Big => Self::Big,
        }
    }
}

/// Which edge of the DJ screen Homifito stands at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum HomifitoPlace {
    Top,
    Bottom,
}

impl From<core_dj::HomifitoPlace> for HomifitoPlace {
    fn from(p: core_dj::HomifitoPlace) -> Self {
        match p {
            core_dj::HomifitoPlace::Top => Self::Top,
            core_dj::HomifitoPlace::Bottom => Self::Bottom,
        }
    }
}

impl From<HomifitoPlace> for core_dj::HomifitoPlace {
    fn from(p: HomifitoPlace) -> Self {
        match p {
            HomifitoPlace::Top => Self::Top,
            HomifitoPlace::Bottom => Self::Bottom,
        }
    }
}

/// The DJ options that matter on a phone (the rest of the core's `DjSettings` is desktop only).
/// Defaults and ranges come from the core.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct DjSettings {
    /// Tempo range, +/- percent (4..=50).
    pub tempo_range_pct: f32,
    /// Cue points and loops snap to the beat grid.
    pub quantize: bool,
    pub crossfader_curve: CrossfaderCurve,
    /// Each song's level is matched to a common one on load.
    pub auto_gain: bool,
    /// Largest boost of an EQ band (dB, 0..=12).
    pub eq_max_boost_db: f32,
    /// 0..=1.
    pub sampler_volume: f32,
    /// A song loaded while the other plays takes its tempo.
    pub sync_on_load: bool,
    /// Length of an automatic mix in beats: 8, 16 or 32.
    pub auto_mix_beats: u32,
    pub load_target: LoadTarget,
    /// Memory the decoded analysis of a song may use (MB).
    pub memory_mb: u32,
    pub waveform_colors: bool,
    pub show_beat_grid: bool,
    /// Homifito DJ, the mascot at his booth (the core's `homifito.mode`).
    pub homifito_mode: HomifitoMode,
    /// The core's `homifito.place`.
    pub homifito_place: HomifitoPlace,
    /// The board in Homifito's colours (the core's `homifito.look`).
    #[uniffi(default = false)]
    pub homifito_look: bool,
}

impl From<core_dj::DjSettings> for DjSettings {
    fn from(s: core_dj::DjSettings) -> Self {
        Self {
            tempo_range_pct: s.tempo_range_pct,
            quantize: s.quantize,
            crossfader_curve: s.crossfader_curve.into(),
            auto_gain: s.auto_gain,
            eq_max_boost_db: s.eq_max_boost_db,
            sampler_volume: s.sampler_volume,
            sync_on_load: s.sync_on_load,
            auto_mix_beats: s.auto_mix_beats,
            load_target: s.load_target.into(),
            memory_mb: s.memory_mb,
            waveform_colors: s.waveform_colors,
            show_beat_grid: s.show_beat_grid,
            homifito_mode: s.homifito.mode.into(),
            homifito_place: s.homifito.place.into(),
            homifito_look: s.homifito.look,
        }
    }
}

impl DjSettings {
    /// The core's settings with these values over the Android defaults.
    fn into_core(self) -> core_dj::DjSettings {
        self.onto(android_defaults())
    }

    /// `base` with these values over it (the desktop-only parts of `base` stay as they are).
    fn onto(self, base: core_dj::DjSettings) -> core_dj::DjSettings {
        core_dj::DjSettings {
            tempo_range_pct: self.tempo_range_pct,
            quantize: self.quantize,
            crossfader_curve: self.crossfader_curve.into(),
            auto_gain: self.auto_gain,
            eq_max_boost_db: self.eq_max_boost_db,
            sampler_volume: self.sampler_volume,
            sync_on_load: self.sync_on_load,
            auto_mix_beats: self.auto_mix_beats,
            load_target: self.load_target.into(),
            memory_mb: self.memory_mb,
            waveform_colors: self.waveform_colors,
            show_beat_grid: self.show_beat_grid,
            homifito: core_dj::HomifitoDj {
                mode: self.homifito_mode.into(),
                place: self.homifito_place.into(),
                look: self.homifito_look,
            },
            ..base
        }
    }
}

/// Core defaults, with the memory limit a phone can afford.
fn android_defaults() -> core_dj::DjSettings {
    core_dj::DjSettings { memory_mb: core_dj::settings::ANDROID_MEMORY_MB, ..core_dj::DjSettings::default() }
}

/// Where a beat grid starts and how long a beat is (ms).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct DjGrid {
    pub first_beat_ms: f64,
    pub beat_ms: f64,
}

impl From<Grid> for DjGrid {
    fn from(g: Grid) -> Self {
        Self { first_beat_ms: g.first_beat_ms, beat_ms: g.beat_ms }
    }
}

impl DjGrid {
    fn to_core(self) -> Option<Grid> {
        Grid::new(self.first_beat_ms, self.beat_ms)
    }
}

/// What the screen and the mix need to know about a song.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct DjAnalysis {
    /// 0 when no tempo was found.
    pub bpm: f64,
    pub first_beat_ms: f64,
    pub beat_ms: f64,
    /// 0..=1: how clearly the beats stand out.
    pub confidence: f32,
    /// The beat grid when it is worth trusting (Sync, beat-aligned mixes and loops need it).
    pub grid: Option<DjGrid>,
    /// 4 bytes per slice (peak, low, mid, high; 0..=255), `overview_per_second` slices a second.
    pub overview: Vec<u8>,
    pub overview_per_second: u32,
    /// Loudness of the louder parts (dBFS).
    pub loudness_db: f32,
    pub duration_ms: u64,
}

impl From<core_dj::DjAnalysis> for DjAnalysis {
    fn from(a: core_dj::DjAnalysis) -> Self {
        let overview = a.overview.iter().flat_map(|s| [s.peak, s.low, s.mid, s.high]).collect();
        Self {
            bpm: a.bpm,
            first_beat_ms: a.first_beat_ms,
            beat_ms: a.beat_ms,
            confidence: a.confidence,
            grid: a.grid().map(Into::into),
            overview,
            overview_per_second: core_dj::analysis::OVERVIEW_PER_SECOND,
            loudness_db: a.loudness_db,
            duration_ms: a.duration_ms.max(0.0).round() as u64,
        }
    }
}

/// The gains of the two decks at a crossfader position.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct DjGains {
    pub deck_a: f32,
    pub deck_b: f32,
}

// ------------------------------------------------------------------ analysis

/// Analysis with the disk cache under `cache_dir` (`homefi_core::dj::analyze_file`).
pub(crate) fn analyze_cached(path: &Path, cache_dir: PathBuf, memory_mb: u32) -> Result<core_dj::DjAnalysis, DjError> {
    core_dj::analyze_file(path, &AnalysisCache::new(cache_dir), memory_mb).map_err(|e| match e {
        core_dj::FileAnalysisError::Open(message) => DjError::Open { message },
        e @ core_dj::FileAnalysisError::TooBig => DjError::Memory { message: e.to_string() },
    })
}

#[uniffi::export]
impl HomefiEngine {
    /// BPM, beat grid, overview and loudness of a local file, kept in the app's cache so the
    /// second time is instant. `memory_mb` caps the decoded copy (the DJ setting).
    pub async fn dj_analyze_file(self: Arc<Self>, path: String, memory_mb: u32) -> Result<DjAnalysis, DjError> {
        let dir = ANALYSIS_DIR.iter().fold(self.session.app_dir().to_path_buf(), |p, part| p.join(part));
        crate::runtime::run(async move {
            tokio::task::spawn_blocking(move || analyze_cached(Path::new(&path), dir, memory_mb))
                .await
                .map_err(|e| DjError::Open { message: e.to_string() })?
                .map(DjAnalysis::from)
        })
        .await
    }
}

// ------------------------------------------------------------------ settings

#[uniffi::export]
pub fn dj_default_settings() -> DjSettings {
    android_defaults().into()
}

#[uniffi::export]
pub fn dj_settings_clamped(settings: DjSettings) -> DjSettings {
    settings.into_core().clamped().into()
}

#[uniffi::export]
pub fn dj_settings_to_json(settings: DjSettings) -> String {
    serde_json::to_string(&settings.into_core().clamped()).unwrap_or_else(|_| "{}".into())
}

/// Lenient: anything unreadable gives the defaults, values out of range are pulled in.
#[uniffi::export]
pub fn dj_settings_from_json(json: String) -> DjSettings {
    let base = android_defaults();
    // fields the JSON lacks keep the phone's defaults, not the desktop's
    let merged = serde_json::from_str::<serde_json::Value>(&json)
        .ok()
        .and_then(|v| match v {
            serde_json::Value::Object(given) => Some(given),
            _ => None,
        })
        .and_then(|given| {
            let mut defaults = serde_json::to_value(&base).ok()?;
            defaults.as_object_mut()?.extend(given);
            serde_json::from_value::<core_dj::DjSettings>(defaults).ok()
        })
        .unwrap_or(base);
    merged.clamped().into()
}

// ------------------------------------------------------------------ layout and setups

/// The parts of the DJ screen that can be moved or hidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum DjSectionKind {
    DeckA,
    Mixer,
    DeckB,
    Sampler,
    Crate,
}

impl From<core_dj::DjSectionKind> for DjSectionKind {
    fn from(k: core_dj::DjSectionKind) -> Self {
        match k {
            core_dj::DjSectionKind::DeckA => Self::DeckA,
            core_dj::DjSectionKind::Mixer => Self::Mixer,
            core_dj::DjSectionKind::DeckB => Self::DeckB,
            core_dj::DjSectionKind::Sampler => Self::Sampler,
            core_dj::DjSectionKind::Crate => Self::Crate,
        }
    }
}

impl From<DjSectionKind> for core_dj::DjSectionKind {
    fn from(k: DjSectionKind) -> Self {
        match k {
            DjSectionKind::DeckA => Self::DeckA,
            DjSectionKind::Mixer => Self::Mixer,
            DjSectionKind::DeckB => Self::DeckB,
            DjSectionKind::Sampler => Self::Sampler,
            DjSectionKind::Crate => Self::Crate,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct DjSection {
    pub kind: DjSectionKind,
    pub visible: bool,
}

/// Section order and visibility plus the sound of each sampler pad (a phone uses the first four pads).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DjLayout {
    /// Every section once, in screen order.
    pub sections: Vec<DjSection>,
    /// Eight sounds, pad by pad.
    pub pads: Vec<SfxKind>,
}

impl From<core_dj::DjLayout> for DjLayout {
    fn from(l: core_dj::DjLayout) -> Self {
        let l = l.clamped();
        Self {
            sections: l.sections.into_iter().map(|s| DjSection { kind: s.kind.into(), visible: s.visible }).collect(),
            pads: l.pads.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<DjLayout> for core_dj::DjLayout {
    fn from(l: DjLayout) -> Self {
        Self {
            sections: l
                .sections
                .into_iter()
                .map(|s| core_dj::DjSection { kind: s.kind.into(), visible: s.visible })
                .collect(),
            pads: l.pads.into_iter().map(Into::into).collect(),
        }
        .clamped()
    }
}

/// Today's layout.
#[uniffi::export]
pub fn dj_default_layout() -> DjLayout {
    core_dj::DjLayout::default().into()
}

/// Every section once and exactly eight pads.
#[uniffi::export]
pub fn dj_layout_clamped(layout: DjLayout) -> DjLayout {
    core_dj::DjLayout::from(layout).into()
}

/// A saved DJ setup, as a list shows it.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct DjSetupSummary {
    pub id: String,
    pub name: String,
    pub created_ms: u64,
    pub updated_ms: u64,
}

impl From<&core_dj::DjSetup> for DjSetupSummary {
    fn from(s: &core_dj::DjSetup) -> Self {
        Self { id: s.id.clone(), name: s.name.clone(), created_ms: s.created_ms, updated_ms: s.updated_ms }
    }
}

/// What applying a setup gives the phone: its options and its layout. Desktop-only parts of the
/// setup (the keymap...) stay in the saved file and are ignored here.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct DjSetupApplied {
    pub settings: DjSettings,
    pub layout: DjLayout,
}

/// The named DJ setups in `<app dir>/dj/setups`. Export gives the JSON text (share it with the
/// system share sheet or save it to a Drive document); import takes that text back.
#[derive(uniffi::Object)]
pub struct DjSetups {
    store: core_dj::DjSetupStore,
}

#[uniffi::export]
impl DjSetups {
    #[uniffi::constructor]
    pub fn new(app_dir: String) -> Arc<Self> {
        Arc::new(Self { store: core_dj::DjSetupStore::in_app_dir(Path::new(&app_dir)) })
    }

    /// By name.
    pub fn list(&self) -> Vec<DjSetupSummary> {
        self.store.list().iter().map(Into::into).collect()
    }

    /// Saves the current options and layout as a new setup.
    pub fn save(&self, name: String, settings: DjSettings, layout: DjLayout) -> crate::Result<DjSetupSummary> {
        let core = core_dj::DjSettings { layout: layout.into(), ..settings.into_core() };
        Ok((&self.store.save(&name, &core)?).into())
    }

    /// Replaces a setup's options and layout; what a phone can't edit (the desktop keymap) is kept.
    pub fn overwrite(&self, id: String, settings: DjSettings, layout: DjLayout) -> crate::Result<DjSetupSummary> {
        let base = self.store.get(&id)?.settings;
        let core = core_dj::DjSettings { layout: layout.into(), ..settings.onto(base) };
        Ok((&self.store.overwrite(&id, &core)?).into())
    }

    pub fn apply(&self, id: String) -> crate::Result<DjSetupApplied> {
        let core = self.store.apply(&id)?;
        Ok(DjSetupApplied { layout: core.layout.clone().into(), settings: core.into() })
    }

    pub fn rename(&self, id: String, name: String) -> crate::Result<DjSetupSummary> {
        Ok((&self.store.rename(&id, &name)?).into())
    }

    /// `false` when there was no such setup.
    pub fn delete(&self, id: String) -> crate::Result<bool> {
        Ok(self.store.delete(&id)?)
    }

    /// The `.homefidj` text.
    pub fn export(&self, id: String) -> crate::Result<String> {
        Ok(self.store.export(&id)?)
    }

    /// Saves a setup from `.homefidj` text as a new one; wrong, damaged or newer files are refused
    /// with a message for the screen.
    pub fn import(&self, json: String) -> crate::Result<DjSetupSummary> {
        Ok((&self.store.import(&json)?).into())
    }
}

/// File extension of an exported DJ setup (no dot).
#[uniffi::export]
pub fn dj_setup_extension() -> String {
    homefi_core::export::DJ_SETUP_EXTENSION.into()
}

// ----------------------------------------------------------------- the maths

/// The filters of one channel for the deck's audio processor at `sample_rate`: High / Mid / Low
/// gains in dB (-48 kills a band; boosts are limited to `max_boost_db`), then the bipolar
/// `filter` (-1 muffled, 0 off, +1 thin). Biquads in series; empty = nothing to run.
#[uniffi::export]
pub fn dj_channel_eq(
    low_db: f32,
    mid_db: f32,
    high_db: f32,
    filter: f32,
    max_boost_db: f32,
    sample_rate: u32,
) -> Vec<Biquad> {
    if sample_rate == 0 {
        return Vec::new();
    }
    let gains = EqGains { low_db, mid_db, high_db }.clamped(max_boost_db);
    let mut stages: Vec<dsp::Biquad> = if gains.is_flat() {
        Vec::new()
    } else {
        mixer_math::channel_eq_stages(gains, sample_rate).into_iter().filter(|b| *b != dsp::Biquad::IDENTITY).collect()
    };
    if let Some(band) = mixer_math::filter_band(filter) {
        stages.push(dsp::Biquad::design(&band.clamped(), sample_rate));
    }
    stages.into_iter().map(Into::into).collect()
}

/// Gains of deck A and B at a crossfader position (0 = all A, 1 = all B).
#[uniffi::export]
pub fn dj_crossfader_gains(position: f32, curve: CrossfaderCurve) -> DjGains {
    let (deck_a, deck_b) = core_dj::CrossfaderCurve::from(curve).gains(position);
    DjGains { deck_a, deck_b }
}

/// Level trim (dB) that brings a song of `loudness_db` to the common level.
#[uniffi::export]
pub fn dj_auto_gain_db(loudness_db: f32) -> f32 {
    mixer_math::auto_gain_db(loudness_db)
}

/// Playback speed (1.0 = normal) that gives a song of `own_bpm` the tempo `target_bpm`, taking
/// half or double time when closer; `None` when it does not fit in +/- `range_pct`.
#[uniffi::export]
pub fn dj_sync_tempo(own_bpm: f64, target_bpm: f64, range_pct: f64) -> Option<f64> {
    core_dj::sync::tempo_for_sync(own_bpm, target_bpm, range_pct)
}

/// How far (ms) the follower must move so its beat lines up with the leader's, the shortest way.
#[uniffi::export]
pub fn dj_phase_offset_ms(leader: DjGrid, leader_pos_ms: f64, follower: DjGrid, follower_pos_ms: f64) -> f64 {
    match (leader.to_core(), follower.to_core()) {
        (Some(l), Some(f)) => core_dj::sync::phase_offset_ms(&l, leader_pos_ms, &f, follower_pos_ms),
        _ => 0.0,
    }
}

/// The beat of the grid closest to `pos_ms` (the position itself without a valid grid).
#[uniffi::export]
pub fn dj_nearest_beat_ms(grid: DjGrid, pos_ms: f64) -> f64 {
    grid.to_core().map_or(pos_ms, |g| g.nearest_beat_ms(pos_ms))
}

/// The first beat after `pos_ms` (the position itself without a valid grid).
#[uniffi::export]
pub fn dj_next_beat_ms(grid: DjGrid, pos_ms: f64) -> f64 {
    grid.to_core().map_or(pos_ms, |g| g.next_beat_ms(pos_ms))
}

// ------------------------------------------------------------------- sampler

/// A pad's sound as a mono 16-bit WAV file's bytes (the app writes it once and plays it with
/// `SoundPool`).
#[uniffi::export]
pub fn dj_sfx_wav(kind: SfxKind, sample_rate: u32) -> Vec<u8> {
    let rate = if sample_rate == 0 { SFX_RATE_HZ } else { sample_rate };
    wav_16bit_mono(&core_dj::sfx::render(kind.into(), rate), rate)
}

fn wav_16bit_mono(samples: &[f32], rate: u32) -> Vec<u8> {
    let data_bytes = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(WAV_HEADER_BYTES + data_bytes as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&PCM_BITS.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_bytes.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setups_keep_the_desktop_parts_when_a_phone_overwrites_them() {
        let tmp = tempfile::tempdir().unwrap();
        let app_dir = tmp.path().to_string_lossy().into_owned();
        // a desktop setup with a rebound key arrives as text
        let mut desktop = core_dj::DjSettings::default();
        desktop.key_overrides.insert("deck_a.play".into(), "x".into());
        let text =
            core_dj::export_setup(&core_dj::DjSetup { name: "Club".into(), settings: desktop, ..Default::default() })
                .unwrap();
        let setups = DjSetups::new(app_dir.clone());
        let summary = setups.import(text.clone()).unwrap();
        let applied = setups.apply(summary.id.clone()).unwrap();
        assert_eq!(applied.layout, dj_default_layout());
        let mut layout = applied.layout.clone();
        layout.pads[0] = SfxKind::Hit;
        setups.overwrite(summary.id.clone(), applied.settings, layout).unwrap();
        let exported = setups.export(summary.id.clone()).unwrap();
        assert!(exported.contains("deck_a.play"), "the keymap survives a phone edit");
        assert!(exported.contains("hit"));
        assert!(setups.import("garbage".into()).is_err());
        assert_eq!(setups.list().len(), 1);
        assert!(setups.delete(summary.id).unwrap());
    }

    #[test]
    fn defaults_are_the_phones() {
        let d = dj_default_settings();
        assert_eq!(d.memory_mb, 256);
        assert_eq!(d.auto_mix_beats, 16);
        assert!(d.sync_on_load && d.quantize);
        assert_eq!(dj_settings_clamped(d.clone()), d);
    }

    #[test]
    fn settings_json_round_trip_and_garbage() {
        let custom = DjSettings { auto_mix_beats: 32, quantize: false, ..dj_default_settings() };
        assert_eq!(dj_settings_from_json(dj_settings_to_json(custom.clone())), custom);
        assert_eq!(dj_settings_from_json("not json".into()), dj_default_settings());
        assert_eq!(dj_settings_from_json("{}".into()), dj_default_settings());
        let wild = dj_settings_from_json(r#"{"tempo_range_pct": 999, "auto_mix_beats": 5}"#.into());
        assert_eq!(wild.tempo_range_pct, 50.0);
        assert_eq!(wild.auto_mix_beats, 8);
        assert_eq!(wild.memory_mb, 256, "missing fields keep the phone's defaults");
    }

    #[test]
    fn channel_eq_is_empty_when_flat_and_kills_a_band() {
        assert!(dj_channel_eq(0.0, 0.0, 0.0, 0.0, 6.0, 44_100).is_empty());
        assert!(dj_channel_eq(0.0, 0.0, 0.0, 0.0, 6.0, 0).is_empty());
        assert!(!dj_channel_eq(-48.0, 0.0, 0.0, 0.0, 6.0, 44_100).is_empty());
        assert_eq!(dj_channel_eq(0.0, 0.0, 0.0, 0.8, 6.0, 44_100).len(), 1);
    }

    #[test]
    fn crossfader_ends_are_one_deck() {
        let a = dj_crossfader_gains(0.0, CrossfaderCurve::Smooth);
        let b = dj_crossfader_gains(1.0, CrossfaderCurve::Smooth);
        assert!(a.deck_a > 0.99 && a.deck_b < 0.01);
        assert!(b.deck_b > 0.99 && b.deck_a < 0.01);
        let mid = dj_crossfader_gains(0.5, CrossfaderCurve::Cut);
        assert_eq!((mid.deck_a, mid.deck_b), (1.0, 1.0));
    }

    #[test]
    fn sync_and_grid_helpers() {
        assert!((dj_sync_tempo(120.0, 124.0, 8.0).unwrap() - 124.0 / 120.0).abs() < 1e-9);
        assert_eq!(dj_sync_tempo(100.0, 160.0, 8.0), None);
        let g = DjGrid { first_beat_ms: 100.0, beat_ms: 500.0 };
        assert_eq!(dj_next_beat_ms(g, 700.0), 1100.0);
        assert_eq!(dj_nearest_beat_ms(g, 560.0), 600.0);
        let bad = DjGrid { first_beat_ms: 0.0, beat_ms: 0.0 };
        assert_eq!(dj_next_beat_ms(bad, 42.0), 42.0);
        assert_eq!(dj_phase_offset_ms(bad, 1.0, g, 2.0), 0.0);
    }

    #[test]
    fn every_pad_sound_is_a_valid_wav() {
        for kind in [SfxKind::AirHorn, SfxKind::Laser, SfxKind::Hit] {
            let wav = dj_sfx_wav(kind, 44_100);
            assert_eq!(&wav[..4], b"RIFF");
            assert_eq!(&wav[8..12], b"WAVE");
            let data_len = u32::from_le_bytes(wav[40..44].try_into().unwrap()) as usize;
            assert_eq!(wav.len(), WAV_HEADER_BYTES + data_len);
            assert!(data_len > 1000);
        }
    }

    #[test]
    fn analysing_a_click_track_and_caching_it() {
        let dir = tempfile::tempdir().unwrap();
        let wav = dir.path().join("clicks.wav");
        let rate = 44_100u32;
        let seconds = 20usize;
        let mut samples = vec![0f32; rate as usize * seconds];
        let beat = (rate as f64 * 60.0 / 120.0) as usize;
        for start in (0..samples.len()).step_by(beat) {
            for k in 0..200 {
                if let Some(s) = samples.get_mut(start + k) {
                    *s = 0.8 * ((k as f32) * 0.14).sin();
                }
            }
        }
        std::fs::write(&wav, wav_16bit_mono(&samples, rate)).unwrap();
        let cache = dir.path().join("cache");
        let a = DjAnalysis::from(analyze_cached(&wav, cache.clone(), 256).unwrap());
        assert!((a.bpm - 120.0).abs() < 2.0, "bpm {}", a.bpm);
        assert!(a.grid.is_some());
        assert_eq!(a.overview.len() % 4, 0);
        assert!(!a.overview.is_empty());
        assert!(a.duration_ms > 19_000);
        assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 1, "stored in the cache");
        // the second time comes from the cache, even if the memory limit would refuse the file
        assert!(analyze_cached(&wav, cache, 0).is_ok());
    }

    #[test]
    fn a_song_over_the_memory_limit_is_refused_and_a_bad_file_is_an_open_error() {
        let dir = tempfile::tempdir().unwrap();
        let wav = dir.path().join("long.wav");
        std::fs::write(&wav, wav_16bit_mono(&vec![0.1; 44_100 * 4], 44_100)).unwrap();
        let cache = dir.path().join("cache");
        assert!(matches!(analyze_cached(&wav, cache.clone(), 0), Err(DjError::Memory { .. })));
        let junk = dir.path().join("junk.wav");
        std::fs::write(&junk, b"nope").unwrap();
        assert!(matches!(analyze_cached(&junk, cache.clone(), 256), Err(DjError::Open { .. })));
        assert!(matches!(analyze_cached(&dir.path().join("none.wav"), cache, 256), Err(DjError::Open { .. })));
    }
}
