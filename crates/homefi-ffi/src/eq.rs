//! Equalizer and AutoEq bindings (`homefi_core::dsp`).
//!
//! Android and iOS run the filters natively in their audio processors from
//! [`eq_output_biquads`] / [`eq_biquads`]; the curve, preamp and taste helpers
//! are the same code the desktop engine uses. Settings are persisted as the
//! core's camelCase JSON ([`eq_settings_to_json`]).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex};

use homefi_core::dsp::{self, autoeq::AutoEq};

use crate::{HomefiEngine, Result};

/// AutoEq downloads (index and profiles) are kept here, inside the app dir.
const AUTOEQ_CACHE: &str = "autoeq-cache";

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum EqBandKind {
    Peak,
    LowShelf,
    HighShelf,
    LowPass,
    HighPass,
}

/// One filter: `freq_hz` 20..=20000, `gain_db` -24..=24, `q` 0.1..=10.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct EqBand {
    pub kind: EqBandKind,
    pub freq_hz: f32,
    pub gain_db: f32,
    pub q: f32,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum EqSource {
    Manual,
    /// `model` as AutoEq lists it, `source` = who measured it.
    AutoEq {
        model: String,
        source: String,
    },
    Assistant,
    Guided,
}

/// A named equalizer setting (at most 10 bands).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct EqProfile {
    pub name: String,
    pub preamp_db: f32,
    pub bands: Vec<EqBand>,
    pub source: EqSource,
}

/// On/off and the profile of each output (by output id).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct EqSettings {
    pub enabled: bool,
    pub auto_preamp: bool,
    pub default_profile: Option<EqProfile>,
    pub per_output: HashMap<String, EqProfile>,
}

/// Taste adjustments: bass / treble shelves and an optional clarity bump (dB).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Tilt {
    pub bass_db: f32,
    pub treble_db: f32,
    pub clarity_db: Option<f32>,
}

/// Normalized biquad (`a0` = 1): run with
/// `y = b0·x + b1·x1 + b2·x2 - a1·y1 - a2·y2` (or transposed direct form II).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Biquad {
    pub b0: f64,
    pub b1: f64,
    pub b2: f64,
    pub a1: f64,
    pub a2: f64,
}

/// One headphone measurement in AutoEq.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct HeadphoneEntry {
    pub name: String,
    /// Who measured it (`oratory1990`, `crinacle`, ...).
    pub source: String,
    /// `over-ear`, `in-ear` or `earbud`.
    pub kind: String,
    /// Measurement rig when the source used several.
    pub rig: Option<String>,
    pub path: String,
}

// ------------------------------------------------------------- conversions

impl From<dsp::EqBandKind> for EqBandKind {
    fn from(k: dsp::EqBandKind) -> Self {
        match k {
            dsp::EqBandKind::Peak => Self::Peak,
            dsp::EqBandKind::LowShelf => Self::LowShelf,
            dsp::EqBandKind::HighShelf => Self::HighShelf,
            dsp::EqBandKind::LowPass => Self::LowPass,
            dsp::EqBandKind::HighPass => Self::HighPass,
        }
    }
}

impl From<EqBandKind> for dsp::EqBandKind {
    fn from(k: EqBandKind) -> Self {
        match k {
            EqBandKind::Peak => Self::Peak,
            EqBandKind::LowShelf => Self::LowShelf,
            EqBandKind::HighShelf => Self::HighShelf,
            EqBandKind::LowPass => Self::LowPass,
            EqBandKind::HighPass => Self::HighPass,
        }
    }
}

impl From<dsp::EqBand> for EqBand {
    fn from(b: dsp::EqBand) -> Self {
        Self { kind: b.kind.into(), freq_hz: b.freq_hz, gain_db: b.gain_db, q: b.q, enabled: b.enabled }
    }
}

impl From<EqBand> for dsp::EqBand {
    fn from(b: EqBand) -> Self {
        Self { kind: b.kind.into(), freq_hz: b.freq_hz, gain_db: b.gain_db, q: b.q, enabled: b.enabled }
    }
}

impl From<dsp::EqSource> for EqSource {
    fn from(s: dsp::EqSource) -> Self {
        match s {
            dsp::EqSource::Manual => Self::Manual,
            dsp::EqSource::AutoEq { model, source } => Self::AutoEq { model, source },
            dsp::EqSource::Assistant => Self::Assistant,
            dsp::EqSource::Guided => Self::Guided,
        }
    }
}

impl From<EqSource> for dsp::EqSource {
    fn from(s: EqSource) -> Self {
        match s {
            EqSource::Manual => Self::Manual,
            EqSource::AutoEq { model, source } => Self::AutoEq { model, source },
            EqSource::Assistant => Self::Assistant,
            EqSource::Guided => Self::Guided,
        }
    }
}

impl From<dsp::EqProfile> for EqProfile {
    fn from(p: dsp::EqProfile) -> Self {
        Self {
            name: p.name,
            preamp_db: p.preamp_db,
            bands: p.bands.into_iter().map(Into::into).collect(),
            source: p.source.into(),
        }
    }
}

impl From<EqProfile> for dsp::EqProfile {
    fn from(p: EqProfile) -> Self {
        Self {
            name: p.name,
            preamp_db: p.preamp_db,
            bands: p.bands.into_iter().map(Into::into).collect(),
            source: p.source.into(),
        }
    }
}

impl From<dsp::EqSettings> for EqSettings {
    fn from(s: dsp::EqSettings) -> Self {
        Self {
            enabled: s.enabled,
            auto_preamp: s.auto_preamp,
            default_profile: s.default_profile.map(Into::into),
            per_output: s.per_output.into_iter().map(|(k, v)| (k, v.into())).collect(),
        }
    }
}

impl From<EqSettings> for dsp::EqSettings {
    fn from(s: EqSettings) -> Self {
        Self {
            enabled: s.enabled,
            auto_preamp: s.auto_preamp,
            default_profile: s.default_profile.map(Into::into),
            per_output: s.per_output.into_iter().map(|(k, v)| (k, v.into())).collect(),
        }
    }
}

impl From<Tilt> for dsp::Tilt {
    fn from(t: Tilt) -> Self {
        Self { bass_db: t.bass_db, treble_db: t.treble_db, clarity_db: t.clarity_db }
    }
}

impl From<dsp::Biquad> for Biquad {
    fn from(b: dsp::Biquad) -> Self {
        Self { b0: b.b0, b1: b.b1, b2: b.b2, a1: b.a1, a2: b.a2 }
    }
}

impl From<dsp::autoeq::HeadphoneEntry> for HeadphoneEntry {
    fn from(e: dsp::autoeq::HeadphoneEntry) -> Self {
        Self { name: e.name, source: e.source, kind: e.kind, rig: e.rig, path: e.path }
    }
}

impl From<HeadphoneEntry> for dsp::autoeq::HeadphoneEntry {
    fn from(e: HeadphoneEntry) -> Self {
        Self { name: e.name, source: e.source, kind: e.kind, rig: e.rig, path: e.path }
    }
}

impl From<dsp::Tilt> for Tilt {
    fn from(t: dsp::Tilt) -> Self {
        Self { bass_db: t.bass_db, treble_db: t.treble_db, clarity_db: t.clarity_db }
    }
}

// ------------------------------------------------------------ taste layers

/// What a taste rule applies to. `id` is the Tidal id (a playlist's uuid; a
/// local track's file path) and may be empty: then the name is compared.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum TasteScope {
    Genre { name: String },
    Playlist { id: String, name: String },
    Artist { id: String, name: String },
    Album { id: String, name: String },
    Track { id: String, name: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TasteCreator {
    Manual,
    Ai,
    Guided,
    Reaction,
}

/// A tilt for some music, on top of the output's profile.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct TasteRule {
    pub id: String,
    pub scope: TasteScope,
    pub tilt: Tilt,
    pub label: String,
    pub created_by: TasteCreator,
    pub enabled: bool,
}

/// Every taste rule and the switch for all of them (off until the first rule).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct TasteSettings {
    pub enabled: bool,
    pub rules: Vec<TasteRule>,
}

/// An id (may be empty) and a name.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ItemRef {
    pub id: String,
    pub name: String,
}

/// Where the queue came from: `album`, `playlist`, `artist`, `mix`, `library`, `folder`...
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SourceContext {
    pub kind: String,
    pub id: String,
    pub name: String,
}

/// What plays, as taste rules see it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NowPlayingContext {
    pub track_id: Option<String>,
    pub path: Option<String>,
    pub title: Option<String>,
    pub album: Option<ItemRef>,
    /// Main artist first.
    pub artists: Vec<ItemRef>,
    /// Main genre first (a GENRE tag goes through [`taste_split_genres`]).
    pub genres: Vec<String>,
    pub source: Option<SourceContext>,
}

/// The rule that plays and its tilt (in range).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ResolvedTaste {
    pub rule: TasteRule,
    pub tilt: Tilt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum LoudnessMode {
    Off,
    Track,
    Album,
    /// Album gain while an album plays in order, else track gain.
    Auto,
}

/// Loudness normalization (off by default: it ends bit-perfect playback).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct LoudnessSettings {
    pub mode: LoudnessMode,
    /// -24..=-8 LUFS, default -14.
    pub target_lufs: f32,
    pub prevent_clipping: bool,
}

/// Gains in dB (ReplayGain 2.0, -18 LUFS reference), peaks linear.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct ReplayGain {
    pub track_gain_db: Option<f32>,
    pub track_peak: Option<f32>,
    pub album_gain_db: Option<f32>,
    pub album_peak: Option<f32>,
}

impl From<dsp::TasteScope> for TasteScope {
    fn from(s: dsp::TasteScope) -> Self {
        match s {
            dsp::TasteScope::Genre { name } => Self::Genre { name },
            dsp::TasteScope::Playlist { id, name } => Self::Playlist { id, name },
            dsp::TasteScope::Artist { id, name } => Self::Artist { id, name },
            dsp::TasteScope::Album { id, name } => Self::Album { id, name },
            dsp::TasteScope::Track { id, name } => Self::Track { id, name },
        }
    }
}

impl From<TasteScope> for dsp::TasteScope {
    fn from(s: TasteScope) -> Self {
        match s {
            TasteScope::Genre { name } => Self::Genre { name },
            TasteScope::Playlist { id, name } => Self::Playlist { id, name },
            TasteScope::Artist { id, name } => Self::Artist { id, name },
            TasteScope::Album { id, name } => Self::Album { id, name },
            TasteScope::Track { id, name } => Self::Track { id, name },
        }
    }
}

impl From<dsp::TasteCreator> for TasteCreator {
    fn from(c: dsp::TasteCreator) -> Self {
        match c {
            dsp::TasteCreator::Manual => Self::Manual,
            dsp::TasteCreator::Ai => Self::Ai,
            dsp::TasteCreator::Guided => Self::Guided,
            dsp::TasteCreator::Reaction => Self::Reaction,
        }
    }
}

impl From<TasteCreator> for dsp::TasteCreator {
    fn from(c: TasteCreator) -> Self {
        match c {
            TasteCreator::Manual => Self::Manual,
            TasteCreator::Ai => Self::Ai,
            TasteCreator::Guided => Self::Guided,
            TasteCreator::Reaction => Self::Reaction,
        }
    }
}

impl From<dsp::TasteRule> for TasteRule {
    fn from(r: dsp::TasteRule) -> Self {
        Self {
            id: r.id,
            scope: r.scope.into(),
            tilt: r.tilt.into(),
            label: r.label,
            created_by: r.created_by.into(),
            enabled: r.enabled,
        }
    }
}

impl From<TasteRule> for dsp::TasteRule {
    fn from(r: TasteRule) -> Self {
        Self {
            id: r.id,
            scope: r.scope.into(),
            tilt: r.tilt.into(),
            label: r.label,
            created_by: r.created_by.into(),
            enabled: r.enabled,
        }
    }
}

impl From<dsp::TasteSettings> for TasteSettings {
    fn from(s: dsp::TasteSettings) -> Self {
        Self { enabled: s.enabled, rules: s.rules.into_iter().map(Into::into).collect() }
    }
}

impl From<TasteSettings> for dsp::TasteSettings {
    fn from(s: TasteSettings) -> Self {
        Self { enabled: s.enabled, rules: s.rules.into_iter().map(Into::into).collect() }
    }
}

impl From<dsp::ItemRef> for ItemRef {
    fn from(r: dsp::ItemRef) -> Self {
        Self { id: r.id, name: r.name }
    }
}

impl From<ItemRef> for dsp::ItemRef {
    fn from(r: ItemRef) -> Self {
        Self { id: r.id, name: r.name }
    }
}

impl From<NowPlayingContext> for dsp::NowPlayingContext {
    fn from(c: NowPlayingContext) -> Self {
        Self {
            track_id: c.track_id,
            path: c.path,
            title: c.title,
            album: c.album.map(Into::into),
            artists: c.artists.into_iter().map(Into::into).collect(),
            genres: c.genres,
            source: c.source.map(|s| dsp::SourceContext { kind: s.kind, id: s.id, name: s.name }),
        }
    }
}

impl From<dsp::NowPlayingContext> for NowPlayingContext {
    fn from(c: dsp::NowPlayingContext) -> Self {
        Self {
            track_id: c.track_id,
            path: c.path,
            title: c.title,
            album: c.album.map(Into::into),
            artists: c.artists.into_iter().map(Into::into).collect(),
            genres: c.genres,
            source: c.source.map(|s| SourceContext { kind: s.kind, id: s.id, name: s.name }),
        }
    }
}

impl From<dsp::LoudnessMode> for LoudnessMode {
    fn from(m: dsp::LoudnessMode) -> Self {
        match m {
            dsp::LoudnessMode::Off => Self::Off,
            dsp::LoudnessMode::Track => Self::Track,
            dsp::LoudnessMode::Album => Self::Album,
            dsp::LoudnessMode::Auto => Self::Auto,
        }
    }
}

impl From<LoudnessMode> for dsp::LoudnessMode {
    fn from(m: LoudnessMode) -> Self {
        match m {
            LoudnessMode::Off => Self::Off,
            LoudnessMode::Track => Self::Track,
            LoudnessMode::Album => Self::Album,
            LoudnessMode::Auto => Self::Auto,
        }
    }
}

impl From<dsp::LoudnessSettings> for LoudnessSettings {
    fn from(l: dsp::LoudnessSettings) -> Self {
        Self { mode: l.mode.into(), target_lufs: l.target_lufs, prevent_clipping: l.prevent_clipping }
    }
}

impl From<LoudnessSettings> for dsp::LoudnessSettings {
    fn from(l: LoudnessSettings) -> Self {
        Self { mode: l.mode.into(), target_lufs: l.target_lufs, prevent_clipping: l.prevent_clipping }
    }
}

impl From<dsp::ReplayGain> for ReplayGain {
    fn from(r: dsp::ReplayGain) -> Self {
        Self {
            track_gain_db: r.track_gain_db,
            track_peak: r.track_peak,
            album_gain_db: r.album_gain_db,
            album_peak: r.album_peak,
        }
    }
}

impl From<ReplayGain> for dsp::ReplayGain {
    fn from(r: ReplayGain) -> Self {
        Self {
            track_gain_db: r.track_gain_db,
            track_peak: r.track_peak,
            album_gain_db: r.album_gain_db,
            album_peak: r.album_peak,
        }
    }
}

// --------------------------------------------------------------- functions

/// Response of the bands in dB at each of `freqs` (preamp not included).
#[uniffi::export]
pub fn eq_response(profile: EqProfile, freqs: Vec<f32>, sample_rate: u32) -> Vec<f32> {
    dsp::response_db(&profile.into(), &freqs, sample_rate)
}

/// `n` log-spaced frequencies from 20 Hz to 20 kHz, for drawing curves.
#[uniffi::export]
pub fn eq_log_freqs(n: u32) -> Vec<f32> {
    dsp::log_freqs(n.min(4096) as usize)
}

/// Preamp (dB, ≤ 0) that keeps the profile's largest boost from clipping.
#[uniffi::export]
pub fn eq_auto_preamp(profile: EqProfile) -> f32 {
    dsp::auto_preamp_db(&profile.into())
}

/// The profile with taste adjustments applied (see `homefi_core::dsp::with_taste`).
#[uniffi::export]
pub fn eq_with_taste(profile: EqProfile, tilt: Tilt) -> EqProfile {
    dsp::with_taste(&profile.into(), tilt.into()).into()
}

/// The profile with at most 10 bands and every value in range.
#[uniffi::export]
pub fn eq_clamp(profile: EqProfile) -> EqProfile {
    dsp::EqProfile::from(profile).clamped().into()
}

/// The profile's filters at `sample_rate`, to run in series; the first one is
/// the profile's preamp (`b0` = linear gain) when it is not 0 dB. Empty = bypass.
#[uniffi::export]
pub fn eq_biquads(profile: EqProfile, sample_rate: u32) -> Vec<Biquad> {
    dsp::biquads(&profile.into(), sample_rate).into_iter().map(Into::into).collect()
}

/// What to run on `output_id` at `sample_rate`: its profile (or the default)
/// with the automatic or own preamp, as in [`eq_biquads`]. Empty when the
/// equalizer is off or flat: the path stays bit-perfect.
#[uniffi::export]
pub fn eq_output_biquads(settings: EqSettings, output_id: String, sample_rate: u32) -> Vec<Biquad> {
    let settings = dsp::EqSettings::from(settings).clamped();
    let Some(profile) = settings.active_profile(&output_id) else { return Vec::new() };
    let profile = dsp::EqProfile { preamp_db: settings.preamp_db(profile), ..profile.clone() };
    dsp::biquads(&profile, sample_rate).into_iter().map(Into::into).collect()
}

/// The profile `output_id` uses (its own, else the default), on or off.
#[uniffi::export]
pub fn eq_profile_for(settings: EqSettings, output_id: String) -> Option<EqProfile> {
    dsp::EqSettings::from(settings).profile_for(&output_id).cloned().map(Into::into)
}

/// The settings as JSON (camelCase, the same format the desktop app stores).
#[uniffi::export]
pub fn eq_settings_to_json(settings: EqSettings) -> String {
    serde_json::to_string(&dsp::EqSettings::from(settings)).unwrap_or_else(|_| "{}".into())
}

/// Settings from [`eq_settings_to_json`]; unreadable JSON gives the defaults (off).
#[uniffi::export]
pub fn eq_settings_from_json(json: String) -> EqSettings {
    serde_json::from_str::<dsp::EqSettings>(&json).unwrap_or_default().clamped().into()
}

/// Defaults: off, automatic preamp, no profiles.
#[uniffi::export]
pub fn eq_default_settings() -> EqSettings {
    dsp::EqSettings::default().into()
}

// ------------------------------------------------- taste layers, loudness

/// The taste rule that applies to `now_playing` (most specific wins: track >
/// album > playlist > artist > genre); `None` when off or nothing matches.
#[uniffi::export]
pub fn resolve_taste(taste: TasteSettings, now_playing: NowPlayingContext) -> Option<ResolvedTaste> {
    let taste = dsp::TasteSettings::from(taste);
    let (rule, tilt) = dsp::resolve_taste(&taste, &now_playing.into())?;
    Some(ResolvedTaste { rule: rule.clone().into(), tilt: tilt.into() })
}

/// [`eq_output_biquads`] for one track: the output's profile (when the
/// equalizer is on) plus the taste rule for `now_playing` (when the layers are
/// on), automatic preamp. Run it on track change; empty = bit-perfect.
#[uniffi::export]
pub fn eq_output_biquads_with_taste(
    settings: EqSettings,
    taste: TasteSettings,
    output_id: String,
    now_playing: NowPlayingContext,
    sample_rate: u32,
) -> Vec<Biquad> {
    sound_biquads(settings, taste, output_id, now_playing, 0.0, sample_rate)
}

/// Everything for one track: [`eq_output_biquads_with_taste`] with the
/// loudness gain (`loudness_db`, from [`loudness_gain_db`]) added to the
/// preamp stage (first, `b0` = linear gain). Empty = bit-perfect.
#[uniffi::export]
pub fn sound_biquads(
    settings: EqSettings,
    taste: TasteSettings,
    output_id: String,
    now_playing: NowPlayingContext,
    loudness_db: f32,
    sample_rate: u32,
) -> Vec<Biquad> {
    let eq = dsp::EqSettings::from(settings).clamped();
    let taste = dsp::TasteSettings::from(taste).clamped();
    let plan = dsp::sound_plan(&eq, &taste, &output_id, &now_playing.into(), loudness_db, sample_rate);
    let mut out = Vec::with_capacity(plan.design.stages().len() + 1);
    if plan.design.gain() != 1.0 {
        out.push(dsp::Biquad::gain(plan.design.gain()).into());
    }
    out.extend(plan.design.stages().iter().copied().map(Biquad::from));
    out
}

/// Gain (dB) loudness normalization applies to a track with `replay_gain`;
/// `album_in_order`: an album plays in its order (for `Auto`). 0 when off.
#[uniffi::export]
pub fn loudness_gain_db(settings: LoudnessSettings, replay_gain: ReplayGain, album_in_order: bool) -> f32 {
    dsp::LoudnessSettings::from(settings).gain_db(&replay_gain.into(), album_in_order)
}

/// ReplayGain from a file's tags (`REPLAYGAIN_*`, Opus `R128_*`); downloads
/// and streamed tracks carry Tidal's.
#[uniffi::export]
pub fn read_replay_gain(path: String) -> Option<ReplayGain> {
    dsp::read_replay_gain(std::path::Path::new(&path)).map(Into::into)
}

/// A GENRE tag as a list (`Electronic, House` → two).
#[uniffi::export]
pub fn taste_split_genres(tag: String) -> Vec<String> {
    dsp::split_genres(&tag)
}

/// A new rule (fresh id, tilt in range, enabled).
#[uniffi::export]
pub fn taste_new_rule(scope: TasteScope, tilt: Tilt, label: String, created_by: TasteCreator) -> TasteRule {
    dsp::TasteRule::new(scope.into(), tilt.into(), label, created_by.into()).into()
}

/// Adds `rule` or replaces the one with its id / scope; the first rule turns the layers on.
#[uniffi::export]
pub fn taste_upsert(taste: TasteSettings, rule: TasteRule) -> TasteSettings {
    let mut t = dsp::TasteSettings::from(taste);
    t.upsert(rule.into());
    t.into()
}

#[uniffi::export]
pub fn taste_remove(taste: TasteSettings, id: String) -> TasteSettings {
    let mut t = dsp::TasteSettings::from(taste);
    t.remove(&id);
    t.into()
}

/// The scopes a track offers, most specific first (for "Sound for this…").
#[uniffi::export]
pub fn taste_scopes(now_playing: NowPlayingContext) -> Vec<TasteScope> {
    dsp::NowPlayingContext::from(now_playing).scopes().into_iter().map(Into::into).collect()
}

/// Same JSON as the desktop app stores (camelCase).
#[uniffi::export]
pub fn taste_settings_to_json(taste: TasteSettings) -> String {
    serde_json::to_string(&dsp::TasteSettings::from(taste)).unwrap_or_else(|_| "{}".into())
}

/// Unreadable JSON gives the defaults (off, no rules); unreadable rules are dropped.
#[uniffi::export]
pub fn taste_settings_from_json(json: String) -> TasteSettings {
    serde_json::from_str::<dsp::TasteSettings>(&json).unwrap_or_default().clamped().into()
}

#[uniffi::export]
pub fn loudness_settings_to_json(settings: LoudnessSettings) -> String {
    serde_json::to_string(&dsp::LoudnessSettings::from(settings)).unwrap_or_else(|_| "{}".into())
}

/// Unreadable JSON gives the defaults (off, -14 LUFS, prevent clipping).
#[uniffi::export]
pub fn loudness_settings_from_json(json: String) -> LoudnessSettings {
    serde_json::from_str::<dsp::LoudnessSettings>(&json).unwrap_or_default().clamped().into()
}

#[uniffi::export]
pub fn loudness_default_settings() -> LoudnessSettings {
    dsp::LoudnessSettings::default().into()
}

// ------------------------------------------------------------------ crossfade

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum CrossfadeMode {
    /// Gapless, bit-perfect transitions (the default).
    Off,
    On,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FadeCurve {
    /// Constant power: even loudness through the fade.
    EqualPower,
    Linear,
    /// Equal power on an eased clock.
    SCurve,
}

/// Crossfade between songs; the same settings (and JSON) as the desktop.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct CrossfadeSettings {
    pub mode: CrossfadeMode,
    /// 1..=12 s, default 6.
    pub seconds: f32,
    pub curve: FadeCurve,
    /// Skip trailing / leading silence (below -60 dBFS).
    pub smart: bool,
    /// Consecutive tracks of one album, in order, stay gapless.
    pub skip_album_runs: bool,
    /// A short fade on next / previous / picking a song.
    pub on_manual_skip: bool,
    /// 0.5..=3 s, default 1.5.
    pub manual_seconds: f32,
    /// Shorter tracks never fade (0 = twice the fade).
    pub min_track_seconds: f32,
    /// Normal transitions on low-memory phones or when two players lag.
    pub low_power_fallback: bool,
}

impl From<dsp::CrossfadeSettings> for CrossfadeSettings {
    fn from(c: dsp::CrossfadeSettings) -> Self {
        Self {
            mode: match c.mode {
                dsp::CrossfadeMode::Off => CrossfadeMode::Off,
                dsp::CrossfadeMode::On => CrossfadeMode::On,
            },
            seconds: c.seconds,
            curve: c.curve.into(),
            smart: c.smart,
            skip_album_runs: c.skip_album_runs,
            on_manual_skip: c.on_manual_skip,
            manual_seconds: c.manual_seconds,
            min_track_seconds: c.min_track_seconds,
            low_power_fallback: c.low_power_fallback,
        }
    }
}

impl From<CrossfadeSettings> for dsp::CrossfadeSettings {
    fn from(c: CrossfadeSettings) -> Self {
        Self {
            mode: match c.mode {
                CrossfadeMode::Off => dsp::CrossfadeMode::Off,
                CrossfadeMode::On => dsp::CrossfadeMode::On,
            },
            seconds: c.seconds,
            curve: c.curve.into(),
            smart: c.smart,
            skip_album_runs: c.skip_album_runs,
            on_manual_skip: c.on_manual_skip,
            manual_seconds: c.manual_seconds,
            min_track_seconds: c.min_track_seconds,
            low_power_fallback: c.low_power_fallback,
        }
    }
}

impl From<dsp::FadeCurve> for FadeCurve {
    fn from(c: dsp::FadeCurve) -> Self {
        match c {
            dsp::FadeCurve::EqualPower => Self::EqualPower,
            dsp::FadeCurve::Linear => Self::Linear,
            dsp::FadeCurve::SCurve => Self::SCurve,
        }
    }
}

impl From<FadeCurve> for dsp::FadeCurve {
    fn from(c: FadeCurve) -> Self {
        match c {
            FadeCurve::EqualPower => Self::EqualPower,
            FadeCurve::Linear => Self::Linear,
            FadeCurve::SCurve => Self::SCurve,
        }
    }
}

#[uniffi::export]
pub fn crossfade_default_settings() -> CrossfadeSettings {
    dsp::CrossfadeSettings::default().into()
}

/// Values in range.
#[uniffi::export]
pub fn crossfade_clamped(settings: CrossfadeSettings) -> CrossfadeSettings {
    dsp::CrossfadeSettings::from(settings).clamped().into()
}

#[uniffi::export]
pub fn crossfade_settings_to_json(settings: CrossfadeSettings) -> String {
    serde_json::to_string(&dsp::CrossfadeSettings::from(settings)).unwrap_or_else(|_| "{}".into())
}

/// Unreadable JSON gives the defaults (off).
#[uniffi::export]
pub fn crossfade_settings_from_json(json: String) -> CrossfadeSettings {
    serde_json::from_str::<dsp::CrossfadeSettings>(&json).unwrap_or_default().clamped().into()
}

/// `[outgoing, incoming]` gains at `t` (0 = fade starts, 1 = done).
#[uniffi::export]
pub fn crossfade_gains(curve: FadeCurve, t: f32) -> Vec<f32> {
    let (o, i) = dsp::crossfade::gains(curve.into(), t);
    vec![o, i]
}

/// Fade length in seconds for a natural transition or a skip (`manual`).
#[uniffi::export]
pub fn crossfade_seconds(settings: CrossfadeSettings, manual: bool) -> f32 {
    dsp::CrossfadeSettings::from(settings).fade_seconds(manual)
}

/// What the crossfade needs to know about one track.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FadeTrack {
    pub album: Option<ItemRef>,
    /// Album artist (or main artist).
    pub artist: Option<ItemRef>,
    pub duration_ms: Option<u64>,
}

/// Whether going from `from` to `to` fades (crossfade on, not an album run
/// kept gapless, both long enough); `manual` for a skip or a pick.
#[uniffi::export]
pub fn crossfade_fades(
    settings: CrossfadeSettings,
    from: FadeTrack,
    to: FadeTrack,
    shuffle: bool,
    manual: bool,
) -> bool {
    let (fa, fr) = (from.album.map(dsp::ItemRef::from), from.artist.map(dsp::ItemRef::from));
    let (ta, tr) = (to.album.map(dsp::ItemRef::from), to.artist.map(dsp::ItemRef::from));
    let f = dsp::crossfade::TrackInfo { album: fa.as_ref(), artist: fr.as_ref(), duration_ms: from.duration_ms };
    let t = dsp::crossfade::TrackInfo { album: ta.as_ref(), artist: tr.as_ref(), duration_ms: to.duration_ms };
    dsp::CrossfadeSettings::from(settings).fades(&f, &t, shuffle, manual)
}

#[uniffi::export]
pub fn taste_default_settings() -> TasteSettings {
    dsp::TasteSettings::default().into()
}

// ------------------------------------------------------------------ AutoEq

/// One client per cache folder, so the parsed index stays in memory.
static AUTOEQ: LazyLock<Mutex<Option<Arc<AutoEq>>>> = LazyLock::new(Default::default);

pub(crate) fn autoeq(dir: PathBuf) -> Arc<AutoEq> {
    let mut slot = AUTOEQ.lock().unwrap_or_else(|e| e.into_inner());
    match slot.as_ref() {
        Some(a) if a.cache_dir() == dir => a.clone(),
        _ => {
            let a = Arc::new(AutoEq::new(dir));
            *slot = Some(a.clone());
            a
        }
    }
}

#[uniffi::export]
impl HomefiEngine {
    /// Headphones in AutoEq matching `query`, best first. The list is
    /// downloaded at most once a day and works offline from the cache.
    pub async fn autoeq_search(self: Arc<Self>, query: String) -> Result<Vec<HeadphoneEntry>> {
        crate::runtime::run(async move {
            let client = autoeq(self.session.app_dir().join(AUTOEQ_CACHE));
            Ok(client.search(&query).await?.into_iter().map(Into::into).collect())
        })
        .await
    }

    /// The correction profile of `entry` (downloaded once, then cached).
    pub async fn autoeq_profile(self: Arc<Self>, entry: HeadphoneEntry) -> Result<EqProfile> {
        crate::runtime::run(async move {
            let client = autoeq(self.session.app_dir().join(AUTOEQ_CACHE));
            Ok(client.profile(&entry.into()).await?.into())
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hd650() -> EqProfile {
        EqProfile {
            name: "HD 650".into(),
            preamp_db: -6.0,
            bands: vec![EqBand { kind: EqBandKind::LowShelf, freq_hz: 105.0, gain_db: 6.0, q: 0.7, enabled: true }],
            source: EqSource::AutoEq { model: "Sennheiser HD 650".into(), source: "oratory1990".into() },
        }
    }

    #[test]
    fn records_round_trip_through_the_core() {
        let p = hd650();
        assert_eq!(EqProfile::from(dsp::EqProfile::from(p.clone())), p);
        let mut settings = eq_default_settings();
        settings.enabled = true;
        settings.per_output.insert("usb".into(), p.clone());
        let json = eq_settings_to_json(settings.clone());
        assert!(json.contains("\"perOutput\""), "{json}");
        assert_eq!(eq_settings_from_json(json), settings);
        assert_eq!(eq_settings_from_json("nonsense".into()), eq_default_settings());
    }

    #[test]
    fn taste_and_loudness_biquads() {
        let ctx = NowPlayingContext {
            track_id: None,
            path: Some("/m/a.flac".into()),
            title: None,
            album: None,
            artists: vec![],
            genres: taste_split_genres("Electronic, House".into()),
            source: None,
        };
        let rule = taste_new_rule(
            TasteScope::Genre { name: "house".into() },
            Tilt { bass_db: 3.0, treble_db: 0.0, clarity_db: None },
            "Punch".into(),
            TasteCreator::Reaction,
        );
        let taste = taste_upsert(taste_default_settings(), rule);
        assert!(taste.enabled);
        assert_eq!(resolve_taste(taste.clone(), ctx.clone()).unwrap().rule.label, "Punch");
        // equalizer off, taste on: a preamp and the shelf
        let b = eq_output_biquads_with_taste(eq_default_settings(), taste.clone(), "usb".into(), ctx.clone(), 48_000);
        assert_eq!(b.len(), 2);
        // nothing at all: bit-perfect; loudness alone: one gain stage
        let none = taste_default_settings();
        assert!(sound_biquads(eq_default_settings(), none.clone(), "usb".into(), ctx.clone(), 0.0, 48_000).is_empty());
        let g = sound_biquads(eq_default_settings(), none, "usb".into(), ctx.clone(), -6.0, 48_000);
        assert_eq!(g.len(), 1);
        assert!((g[0].b0 - 10f64.powf(-0.3)).abs() < 1e-6);
        let json = taste_settings_to_json(taste.clone());
        assert_eq!(taste_settings_from_json(json), taste);
        let l = LoudnessSettings { mode: LoudnessMode::Auto, ..loudness_default_settings() };
        assert_eq!(loudness_settings_from_json(loudness_settings_to_json(l)), l);
        let rg = ReplayGain {
            track_gain_db: Some(-10.0),
            track_peak: Some(1.0),
            album_gain_db: Some(-8.0),
            album_peak: None,
        };
        assert_eq!(loudness_gain_db(l, rg, true), -4.0);
        assert_eq!(loudness_gain_db(l, rg, false), -6.0);
        let x = CrossfadeSettings { mode: CrossfadeMode::On, curve: FadeCurve::SCurve, ..crossfade_default_settings() };
        assert_eq!(crossfade_settings_from_json(crossfade_settings_to_json(x)), x);
        assert_eq!(crossfade_settings_from_json("nope".into()), crossfade_default_settings());
        assert_eq!(crossfade_gains(FadeCurve::Linear, 0.25), vec![0.75, 0.25]);
        let album = Some(ItemRef { id: String::new(), name: "Album".into() });
        let t = |album: Option<ItemRef>| FadeTrack { album, artist: None, duration_ms: Some(200_000) };
        assert!(!crossfade_fades(x, t(album.clone()), t(album.clone()), false, false), "album run");
        assert!(crossfade_fades(x, t(album), t(None), false, false));
        assert_eq!(taste_scopes(ctx).len(), 2);
    }

    #[test]
    fn output_biquads_follow_the_settings() {
        let mut settings = eq_default_settings();
        settings.per_output.insert("usb".into(), hd650());
        assert!(eq_output_biquads(settings.clone(), "usb".into(), 48_000).is_empty(), "off");
        settings.enabled = true;
        let b = eq_output_biquads(settings.clone(), "usb".into(), 48_000);
        // automatic preamp first, then the shelf
        assert_eq!(b.len(), 2);
        assert!(b[0].b0 < 1.0 && b[0].a1 == 0.0);
        assert!(eq_output_biquads(settings, "speaker".into(), 48_000).is_empty(), "no profile");
        assert_eq!(eq_biquads(hd650(), 48_000).len(), 2);
        let curve = eq_response(hd650(), eq_log_freqs(8), 48_000);
        assert!(curve[0] > 5.0 && curve[7].abs() < 0.2, "{curve:?}");
    }
}
