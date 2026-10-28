//! Parametric equalizer shared by every platform.
//!
//! The types here are what the apps persist (camelCase JSON, stable): an
//! [`EqProfile`] is a preamp plus up to [`MAX_BANDS`] [`EqBand`]s, and
//! [`EqSettings`] says which profile each output device uses. The desktop
//! engine runs an [`Equalizer`] over the samples; Android and iOS run the
//! same filters natively from [`biquads`].
//!
//! Filters are the RBJ Audio-EQ-Cookbook biquads with `f64` coefficients.
//! [`autoeq`] turns headphone measurements of the AutoEq project into profiles.

pub mod autoeq;
mod biquad;
pub mod crossfade;
mod equalizer;
pub mod loudness;
pub mod taste;

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};

pub use biquad::Biquad;
pub use crossfade::{lenient_crossfade, CrossfadeMode, CrossfadeSettings, FadeCurve};
pub use equalizer::{glide_frames, EqDesign, Equalizer};
pub use loudness::{lenient_loudness, read_replay_gain, LoudnessMode, LoudnessSettings, ReplayGain};
pub use taste::{
    lenient_taste, resolve_taste, split_genres, ItemRef, NowPlayingContext, SourceContext, TasteCreator, TasteRule,
    TasteScope, TasteSettings, MAX_TILT_DB,
};

/// Bands a profile can hold (AutoEq profiles use exactly 10).
pub const MAX_BANDS: usize = 10;
pub const MIN_FREQ_HZ: f32 = 20.0;
pub const MAX_FREQ_HZ: f32 = 20_000.0;
pub const MIN_GAIN_DB: f32 = -24.0;
pub const MAX_GAIN_DB: f32 = 24.0;
pub const MIN_Q: f32 = 0.1;
pub const MAX_Q: f32 = 10.0;
/// Preamp range: low enough for several boosts stacked on one frequency.
pub const MIN_PREAMP_DB: f32 = -60.0;
pub const MAX_PREAMP_DB: f32 = 24.0;
/// Gains (dB) smaller than this count as "no change".
const NEUTRAL_DB: f32 = 0.05;

/// Filter shape of a band.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum EqBandKind {
    #[default]
    Peak,
    LowShelf,
    HighShelf,
    /// 12 dB/octave low-pass (`gain_db` unused).
    LowPass,
    /// 12 dB/octave high-pass (`gain_db` unused).
    HighPass,
}

impl EqBandKind {
    /// `true` for the kinds whose effect is set by `gain_db`.
    pub fn has_gain(self) -> bool {
        matches!(self, Self::Peak | Self::LowShelf | Self::HighShelf)
    }
}

/// One filter of a profile.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EqBand {
    #[serde(deserialize_with = "lenient_kind")]
    pub kind: EqBandKind,
    /// Centre (peak) or corner (shelves, passes) frequency, 20..=20000 Hz.
    pub freq_hz: f32,
    /// -24..=+24 dB.
    pub gain_db: f32,
    /// Width: higher is narrower, 0.1..=10 (0.707 is a gentle, standard slope).
    pub q: f32,
    pub enabled: bool,
}

impl Default for EqBand {
    fn default() -> Self {
        Self {
            kind: EqBandKind::Peak,
            freq_hz: 1000.0,
            gain_db: 0.0,
            q: std::f32::consts::FRAC_1_SQRT_2,
            enabled: true,
        }
    }
}

impl EqBand {
    pub fn new(kind: EqBandKind, freq_hz: f32, gain_db: f32, q: f32) -> Self {
        Self { kind, freq_hz, gain_db, q, enabled: true }
    }

    /// The band with every value in range (NaN becomes the default value).
    pub fn clamped(self) -> Self {
        let d = Self::default();
        Self {
            kind: self.kind,
            freq_hz: clamp_or(self.freq_hz, MIN_FREQ_HZ, MAX_FREQ_HZ, d.freq_hz),
            gain_db: clamp_or(self.gain_db, MIN_GAIN_DB, MAX_GAIN_DB, 0.0),
            q: clamp_or(self.q, MIN_Q, MAX_Q, d.q),
            enabled: self.enabled,
        }
    }

    /// `true` when the band changes nothing (off, or a 0 dB peak / shelf).
    pub fn is_neutral(&self) -> bool {
        !self.enabled || self.kind.has_gain() && !audible(self.gain_db)
    }
}

/// Where a profile came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EqSource {
    /// Made or edited by hand.
    #[default]
    Manual,
    /// A headphone correction from AutoEq: `model` as listed there, `source`
    /// is who measured it (`oratory1990`, `crinacle`, ...).
    AutoEq { model: String, source: String },
    /// Suggested by the EQ assistant.
    Assistant,
    /// Built by the guided (listening test) wizard.
    Guided,
}

/// A named equalizer setting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct EqProfile {
    pub name: String,
    /// Gain before the filters, usually negative to leave room for boosts.
    pub preamp_db: f32,
    pub bands: Vec<EqBand>,
    #[serde(deserialize_with = "lenient_source")]
    pub source: EqSource,
}

impl EqProfile {
    /// A profile that changes nothing.
    pub fn flat(name: impl Into<String>) -> Self {
        Self { name: name.into(), ..Default::default() }
    }

    /// The profile with at most [`MAX_BANDS`] bands and every value in range.
    pub fn clamped(&self) -> Self {
        let mut p = self.clone();
        p.clamp();
        p
    }

    /// Brings every value in range in place (see [`EqProfile::clamped`]).
    pub fn clamp(&mut self) {
        self.preamp_db = clamp_or(self.preamp_db, MIN_PREAMP_DB, MAX_PREAMP_DB, 0.0);
        self.bands.truncate(MAX_BANDS);
        for b in &mut self.bands {
            *b = b.clamped();
        }
    }

    /// `true` when the filters change nothing (the preamp is not looked at).
    pub fn bands_flat(&self) -> bool {
        self.bands.iter().all(EqBand::is_neutral)
    }

    /// `true` when neither the filters nor the preamp change the sound.
    pub fn is_flat(&self) -> bool {
        self.bands_flat() && !audible(self.preamp_db)
    }
}

/// Equalizer settings of an app: on/off, and the profile of each output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EqSettings {
    pub enabled: bool,
    /// Use [`auto_preamp_db`] instead of each profile's own preamp.
    pub auto_preamp: bool,
    /// For outputs without a profile of their own.
    pub default_profile: Option<EqProfile>,
    /// Profile per output device id (the desktop uses the device name).
    pub per_output: BTreeMap<String, EqProfile>,
}

impl Default for EqSettings {
    fn default() -> Self {
        Self { enabled: false, auto_preamp: true, default_profile: None, per_output: BTreeMap::new() }
    }
}

impl EqSettings {
    /// The profile `output_id` uses, else the default one (whether or not the
    /// equalizer is enabled).
    pub fn profile_for(&self, output_id: &str) -> Option<&EqProfile> {
        self.per_output.get(output_id).or(self.default_profile.as_ref())
    }

    /// The profile to run on `output_id`: `None` when the equalizer is off or
    /// the profile changes nothing (the path stays bit-perfect).
    pub fn active_profile(&self, output_id: &str) -> Option<&EqProfile> {
        let p = self.profile_for(output_id).filter(|_| self.enabled)?;
        let flat = if self.auto_preamp { p.bands_flat() } else { p.is_flat() };
        (!flat).then_some(p)
    }

    /// Preamp to apply with `profile`: automatic or the profile's own.
    pub fn preamp_db(&self, profile: &EqProfile) -> f32 {
        if self.auto_preamp {
            auto_preamp_db(profile)
        } else {
            clamp_or(profile.preamp_db, MIN_PREAMP_DB, MAX_PREAMP_DB, 0.0)
        }
    }

    /// The filters for `output_id` at `sample_rate` (bypass when inactive).
    pub fn design_for(&self, output_id: &str, sample_rate: u32) -> EqDesign {
        match self.active_profile(output_id) {
            Some(p) => EqDesign::new(p, self.preamp_db(p), sample_rate),
            None => EqDesign::BYPASS,
        }
    }

    /// Every profile in range.
    pub fn clamped(&self) -> Self {
        let mut s = self.clone();
        if let Some(p) = &mut s.default_profile {
            p.clamp();
        }
        for p in s.per_output.values_mut() {
            p.clamp();
        }
        s
    }
}

/// Taste adjustments on top of a profile (guided wizard, assistant): broad
/// shelves for bass and treble and an optional presence ("clarity") bump.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Tilt {
    /// Low shelf at [`TASTE_BASS_HZ`].
    pub bass_db: f32,
    /// High shelf at [`TASTE_TREBLE_HZ`].
    pub treble_db: f32,
    /// Wide peak at [`TASTE_CLARITY_HZ`].
    pub clarity_db: Option<f32>,
}

impl Tilt {
    /// Every value within ±`limit` dB (NaN becomes 0).
    pub fn clamped(self, limit: f32) -> Self {
        let c = |v: f32| if v.is_finite() { v.clamp(-limit, limit) } else { 0.0 };
        Self { bass_db: c(self.bass_db), treble_db: c(self.treble_db), clarity_db: self.clarity_db.map(c) }
    }

    /// `true` when it changes nothing.
    pub fn is_neutral(&self) -> bool {
        !audible(self.bass_db) && !audible(self.treble_db) && !self.clarity_db.is_some_and(audible)
    }

    /// Both tilts added up.
    pub fn plus(self, other: Tilt) -> Self {
        Self {
            bass_db: self.bass_db + other.bass_db,
            treble_db: self.treble_db + other.treble_db,
            clarity_db: match (self.clarity_db, other.clarity_db) {
                (None, None) => None,
                (a, b) => Some(a.unwrap_or(0.0) + b.unwrap_or(0.0)),
            },
        }
    }
}

/// Corner of the bass taste shelf (the same as AutoEq's low shelf).
pub const TASTE_BASS_HZ: f32 = 105.0;
/// Corner of the treble taste shelf (the same as AutoEq's high shelf).
pub const TASTE_TREBLE_HZ: f32 = 10_000.0;
/// Centre of the clarity (presence) peak.
pub const TASTE_CLARITY_HZ: f32 = 3_000.0;
const TASTE_SHELF_Q: f32 = 0.7;
const TASTE_CLARITY_Q: f32 = 0.9;

/// `profile` with `tilt` applied. Each adjustment joins a matching band of the
/// profile when there is one (a shelf of the same kind within an octave, e.g.
/// AutoEq's 105 Hz / 10 kHz shelves), else it is added as a new band; when the
/// profile is full it joins the nearest band of its kind, or replaces the band
/// that does the least. The preamp only goes down, if the boost needs room.
/// The source is kept.
pub fn with_taste(profile: &EqProfile, tilt: Tilt) -> EqProfile {
    let mut out = profile.clamped();
    let wanted = [
        (EqBandKind::LowShelf, TASTE_BASS_HZ, tilt.bass_db, TASTE_SHELF_Q),
        (EqBandKind::HighShelf, TASTE_TREBLE_HZ, tilt.treble_db, TASTE_SHELF_Q),
        (EqBandKind::Peak, TASTE_CLARITY_HZ, tilt.clarity_db.unwrap_or(0.0), TASTE_CLARITY_Q),
    ];
    let mut changed = false;
    for (kind, freq, gain, q) in wanted {
        if !audible(gain) {
            continue; // zero or NaN
        }
        add_taste(&mut out.bands, EqBand::new(kind, freq, gain, q).clamped());
        changed = true;
    }
    if changed {
        out.preamp_db = out.preamp_db.min(auto_preamp_db(&out));
    }
    out
}

/// Small speakers (phone, laptop, built-in) can't play deep bass: "more bass"
/// there is a gentle low-mid lift instead, capped at this (dB)...
pub const SMALL_SPEAKER_MAX_BASS_DB: f32 = 3.0;
/// ...around this frequency, with a high-pass below what they can play.
pub const SMALL_SPEAKER_BASS_HZ: f32 = 200.0;
pub const SMALL_SPEAKER_HIGHPASS_HZ: f32 = 90.0;

/// [`with_taste`] for the kind of output: on `small_speaker`s a bass boost is
/// a ≤ [`SMALL_SPEAKER_MAX_BASS_DB`] peak at [`SMALL_SPEAKER_BASS_HZ`] plus a
/// high-pass at [`SMALL_SPEAKER_HIGHPASS_HZ`] (deep bass would only eat
/// headroom and distort); cuts and the rest of the tilt are as usual.
pub fn with_taste_for(profile: &EqProfile, tilt: Tilt, small_speaker: bool) -> EqProfile {
    if !small_speaker || tilt.bass_db <= 0.0 || !audible(tilt.bass_db) {
        return with_taste(profile, tilt);
    }
    let mut out = with_taste(profile, Tilt { bass_db: 0.0, ..tilt });
    let lift = tilt.bass_db.min(SMALL_SPEAKER_MAX_BASS_DB);
    add_taste(&mut out.bands, EqBand::new(EqBandKind::Peak, SMALL_SPEAKER_BASS_HZ, lift, 0.9));
    if !out.bands.iter().any(|b| b.enabled && b.kind == EqBandKind::HighPass) {
        add_taste(&mut out.bands, EqBand::new(EqBandKind::HighPass, SMALL_SPEAKER_HIGHPASS_HZ, 0.0, 0.707));
    }
    out.preamp_db = out.preamp_db.min(auto_preamp_db(&out));
    out
}

/// A tilt safe for the kind of output: on small speakers the bass boost is
/// capped at [`SMALL_SPEAKER_MAX_BASS_DB`] (what [`with_taste_for`] will do anyway).
pub fn safe_tilt(tilt: Tilt, small_speaker: bool) -> Tilt {
    let t = tilt.clamped(MAX_TILT_DB);
    if small_speaker {
        Tilt { bass_db: t.bass_db.min(SMALL_SPEAKER_MAX_BASS_DB), ..t }
    } else {
        t
    }
}

/// Guess from an output's name whether it is a built-in / phone / laptop
/// speaker (the OS rarely says more).
pub fn is_built_in_speaker(name: &str) -> bool {
    let n = name.to_lowercase();
    let headphones = ["headphone", "auricular", "audífono", "audifono", "airpods", "buds", "usb", "dac", "bluetooth"];
    if headphones.iter().any(|w| n.contains(w)) {
        return false;
    }
    ["speaker", "bocina", "altavoz", "altavoces", "built-in", "builtin", "internal", "phone speaker", "earpiece"]
        .iter()
        .any(|w| n.contains(w))
}

fn add_taste(bands: &mut Vec<EqBand>, taste: EqBand) {
    let octaves = |b: &EqBand| (b.freq_hz / taste.freq_hz).log2().abs();
    let same_kind = |b: &&mut EqBand| b.enabled && b.kind == taste.kind;
    // a band of the same shape and place: add to it
    let similar = bands
        .iter_mut()
        .filter(same_kind)
        .find(|b| octaves(b) <= 1.0 && (taste.kind != EqBandKind::Peak || (b.q / taste.q).log2().abs() <= 0.6));
    if let Some(b) = similar {
        b.gain_db = (b.gain_db + taste.gain_db).clamp(MIN_GAIN_DB, MAX_GAIN_DB);
        return;
    }
    if bands.len() < MAX_BANDS {
        bands.push(taste);
        return;
    }
    let nearest = bands.iter_mut().filter(same_kind).min_by(|a, b| octaves(a).total_cmp(&octaves(b)));
    if let Some(b) = nearest {
        b.gain_db = (b.gain_db + taste.gain_db).clamp(MIN_GAIN_DB, MAX_GAIN_DB);
        return;
    }
    // full and nothing alike: replace the band that matters least
    let weight = |b: &EqBand| {
        if b.is_neutral() {
            -1.0
        } else if b.kind.has_gain() {
            b.gain_db.abs()
        } else {
            f32::MAX
        }
    };
    if let Some(b) = bands.iter_mut().min_by(|a, b| weight(a).total_cmp(&weight(b))) {
        *b = taste;
    }
}

/// Preamp (dB, ≤ 0) that leaves room for the largest boost of the summed
/// response of `profile`'s bands, so a full-scale signal never clips. Its own
/// `preamp_db` is not looked at.
pub fn auto_preamp_db(profile: &EqProfile) -> f32 {
    let profile = profile.clamped();
    if profile.bands_flat() {
        return 0.0;
    }
    let freqs = log_freqs(512);
    // 192 kHz approximates the analog curve, 44.1 kHz shows the warping near Nyquist
    let peak = [44_100, 192_000].into_iter().flat_map(|rate| response_db(&profile, &freqs, rate)).fold(0f32, f32::max);
    // round towards more headroom, to 0.1 dB
    -(peak * 10.0).ceil() / 10.0
}

/// Response of `profile`'s bands in dB at each of `freqs` (the preamp is not
/// included: add `preamp_db` to plot the output level). Always finite.
pub fn response_db(profile: &EqProfile, freqs: &[f32], sample_rate: u32) -> Vec<f32> {
    let stages: Vec<Biquad> = profile
        .bands
        .iter()
        .take(MAX_BANDS)
        .map(|b| b.clamped())
        .filter(|b| !b.is_neutral())
        .map(|b| Biquad::design(&b, sample_rate))
        .collect();
    freqs
        .iter()
        .map(|&f| {
            let db: f64 = stages.iter().map(|s| s.magnitude_db(f as f64, sample_rate)).sum();
            if db.is_finite() {
                db as f32
            } else {
                0.0
            }
        })
        .collect()
}

/// `n` log-spaced frequencies from 20 Hz to 20 kHz (both included), for curves.
pub fn log_freqs(n: usize) -> Vec<f32> {
    match n {
        0 => Vec::new(),
        1 => vec![MIN_FREQ_HZ],
        _ => {
            let (lo, hi) = ((MIN_FREQ_HZ as f64).ln(), (MAX_FREQ_HZ as f64).ln());
            (0..n).map(|i| (lo + (hi - lo) * i as f64 / (n - 1) as f64).exp() as f32).collect()
        }
    }
}

/// The filters of `profile` at `sample_rate` for a native audio processor:
/// run every stage in series. The first stage is the preamp
/// (`b0` = linear gain) when `preamp_db` is not 0; an empty list means bypass.
pub fn biquads(profile: &EqProfile, sample_rate: u32) -> Vec<Biquad> {
    let design = EqDesign::new(profile, profile.preamp_db, sample_rate);
    let mut out = Vec::with_capacity(design.stages().len() + 1);
    if design.gain() != 1.0 {
        out.push(Biquad::gain(design.gain()));
    }
    out.extend_from_slice(design.stages());
    out
}

/// What the player needs to know about a track for taste layers and loudness.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct TrackSound {
    pub context: NowPlayingContext,
    /// Known gains (Tidal); `None`: read from the file's tags.
    pub replay_gain: Option<ReplayGain>,
}

/// The profile to run on `output_id` for the track `ctx`: the output's own
/// (when the equalizer is on) plus the taste rule that applies (when taste
/// layers are on), with the preamp set (automatic or own, never less room
/// than the taste needs). `None` when nothing changes the sound. Returns the
/// rule used, too.
pub fn track_profile<'a>(
    eq: &EqSettings,
    taste: &'a TasteSettings,
    output_id: &str,
    ctx: &NowPlayingContext,
) -> Option<(EqProfile, Option<&'a TasteRule>)> {
    let base = if eq.enabled { eq.profile_for(output_id) } else { None };
    let rule = resolve_taste(taste, ctx).filter(|(_, t)| !t.is_neutral());
    let (mut profile, used) = match (base, rule) {
        (None, None) => return None,
        (Some(b), None) => (b.clamped(), None),
        (b, Some((r, tilt))) => {
            let base = b.cloned().unwrap_or_else(|| EqProfile::flat(""));
            (with_taste_for(&base, tilt, is_built_in_speaker(output_id)), Some(r))
        }
    };
    let flat = if eq.auto_preamp || base.is_none() { profile.bands_flat() } else { profile.is_flat() };
    if flat {
        return None;
    }
    profile.preamp_db = if base.is_none() { auto_preamp_db(&profile) } else { eq.preamp_db(&profile) };
    Some((profile, used))
}

/// Everything the player runs for one track: the filters (with the
/// equalizer preamp and the loudness gain in front) and what to show.
#[derive(Debug, Clone, PartialEq)]
pub struct SoundPlan {
    pub design: EqDesign,
    /// Filters change the sound (output profile and / or taste).
    pub eq_active: bool,
    /// Name of the output's profile in use.
    pub profile: Option<String>,
    /// The taste rule in use.
    pub taste_rule: Option<TasteRule>,
    /// Loudness gain applied (dB).
    pub loudness_db: f32,
}

impl SoundPlan {
    pub const BYPASS: Self =
        Self { design: EqDesign::BYPASS, eq_active: false, profile: None, taste_rule: None, loudness_db: 0.0 };
}

/// The [`SoundPlan`] of a track: [`track_profile`] plus `loudness_db` (from
/// [`LoudnessSettings::gain_db`]) added to the preamp, at `sample_rate`.
pub fn sound_plan(
    eq: &EqSettings,
    taste: &TasteSettings,
    output_id: &str,
    ctx: &NowPlayingContext,
    loudness_db: f32,
    sample_rate: u32,
) -> SoundPlan {
    let loudness_db = if loudness_db.is_finite() && audible(loudness_db) { loudness_db } else { 0.0 };
    let found = track_profile(eq, taste, output_id, ctx);
    let (design, eq_active, profile, rule) = match &found {
        Some((p, rule)) => {
            let own = if eq.enabled { eq.profile_for(output_id).map(|p| p.name.clone()) } else { None };
            (EqDesign::new(p, p.preamp_db + loudness_db, sample_rate), true, own, rule.cloned())
        }
        None => (EqDesign::new(&EqProfile::default(), loudness_db, sample_rate), false, None, None),
    };
    SoundPlan { design, eq_active, profile, taste_rule: rule, loudness_db }
}

/// `true` when a gain of `db` is heard (`false` for NaN).
fn audible(db: f32) -> bool {
    db.abs() >= NEUTRAL_DB
}

fn clamp_or(v: f32, lo: f32, hi: f32, nan: f32) -> f32 {
    if v.is_nan() {
        nan
    } else {
        v.clamp(lo, hi)
    }
}

/// A kind written by a newer version reads as a peak instead of failing the
/// whole settings file.
fn lenient_kind<'de, D: Deserializer<'de>>(d: D) -> Result<EqBandKind, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Any {
        Known(EqBandKind),
        Other(serde::de::IgnoredAny),
    }
    Ok(match Any::deserialize(d)? {
        Any::Known(k) => k,
        Any::Other(_) => EqBandKind::Peak,
    })
}

/// An unknown source reads as manual.
fn lenient_source<'de, D: Deserializer<'de>>(d: D) -> Result<EqSource, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Any {
        Known(EqSource),
        Other(serde::de::IgnoredAny),
    }
    Ok(match Any::deserialize(d)? {
        Any::Known(s) => s,
        Any::Other(_) => EqSource::Manual,
    })
}

/// Deserializes [`EqSettings`] falling back to the defaults when the value is
/// unreadable, so a bad `eq` section never resets the rest of a settings file
/// (`#[serde(deserialize_with = "homefi_core::dsp::lenient_settings")]`).
pub fn lenient_settings<'de, D: Deserializer<'de>>(d: D) -> Result<EqSettings, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Any {
        Known(EqSettings),
        Other(serde::de::IgnoredAny),
    }
    Ok(match Any::deserialize(d)? {
        Any::Known(s) => s,
        Any::Other(_) => EqSettings::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peak(freq: f32, gain: f32, q: f32) -> EqBand {
        EqBand::new(EqBandKind::Peak, freq, gain, q)
    }

    fn profile(bands: Vec<EqBand>) -> EqProfile {
        EqProfile { name: "test".into(), bands, ..Default::default() }
    }

    fn at(p: &EqProfile, freq: f32) -> f32 {
        response_db(p, &[freq], 48_000)[0]
    }

    #[test]
    fn json_is_camel_case_and_stable() {
        let p = EqProfile {
            name: "HD 650".into(),
            preamp_db: -6.1,
            bands: vec![EqBand::new(EqBandKind::LowShelf, 105.0, 6.4, 0.7)],
            source: EqSource::AutoEq { model: "Sennheiser HD 650".into(), source: "oratory1990".into() },
        };
        let json = serde_json::to_value(&p).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "name": "HD 650",
                "preampDb": -6.1f32,
                "bands": [{"kind": "lowShelf", "freqHz": 105.0, "gainDb": 6.4f32, "q": 0.7f32, "enabled": true}],
                "source": {"kind": "autoEq", "model": "Sennheiser HD 650", "source": "oratory1990"}
            })
        );
        assert_eq!(serde_json::from_value::<EqProfile>(json).unwrap(), p);

        let s = EqSettings { enabled: true, default_profile: Some(p.clone()), ..Default::default() };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"autoPreamp\":true") && json.contains("\"perOutput\":{}"), "{json}");
        assert_eq!(serde_json::from_str::<EqSettings>(&json).unwrap(), s);
    }

    #[test]
    fn json_is_lenient() {
        // missing fields take defaults, the band is enabled
        let p: EqProfile = serde_json::from_str(r#"{"bands":[{"freqHz":250,"gainDb":3}]}"#).unwrap();
        assert_eq!(p.bands[0].kind, EqBandKind::Peak);
        assert!(p.bands[0].enabled);
        assert_eq!(p.source, EqSource::Manual);
        // unknown kinds / sources from a newer version do not fail
        let p: EqProfile =
            serde_json::from_str(r#"{"bands":[{"kind":"notch","freqHz":250}],"source":{"kind":"cloud"}}"#).unwrap();
        assert_eq!(p.bands[0].kind, EqBandKind::Peak);
        assert_eq!(p.source, EqSource::Manual);
        assert_eq!(serde_json::from_str::<EqSettings>("{}").unwrap(), EqSettings::default());

        #[derive(Deserialize)]
        struct Wrapper {
            #[serde(default, deserialize_with = "lenient_settings")]
            eq: EqSettings,
        }
        let w: Wrapper = serde_json::from_str(r#"{"eq":{"enabled":"yes"}}"#).unwrap();
        assert_eq!(w.eq, EqSettings::default());
    }

    #[test]
    fn clamping_keeps_values_in_range() {
        let mut p = profile((0..14).map(|_| peak(f32::NAN, 99.0, 0.0)).collect());
        p.preamp_db = f32::INFINITY;
        p.clamp();
        assert_eq!(p.bands.len(), MAX_BANDS);
        assert_eq!(p.bands[0].freq_hz, 1000.0);
        assert_eq!(p.bands[0].gain_db, MAX_GAIN_DB);
        assert_eq!(p.bands[0].q, MIN_Q);
        assert_eq!(p.preamp_db, MAX_PREAMP_DB);
        let b = EqBand::new(EqBandKind::HighShelf, 5.0, -40.0, 50.0).clamped();
        assert_eq!((b.freq_hz, b.gain_db, b.q), (MIN_FREQ_HZ, MIN_GAIN_DB, MAX_Q));
    }

    #[test]
    fn flat_profiles_change_nothing() {
        let p = profile(vec![peak(1000.0, 0.0, 1.0), EqBand { enabled: false, ..peak(100.0, 6.0, 1.0) }]);
        assert!(p.is_flat());
        assert!(response_db(&p, &log_freqs(64), 48_000).iter().all(|&db| db == 0.0));
        assert_eq!(auto_preamp_db(&p), 0.0);
        assert!(biquads(&p, 48_000).is_empty());
    }

    #[test]
    fn peak_measures_its_gain_at_the_centre() {
        let p = profile(vec![peak(1000.0, 6.0, 1.0)]);
        assert!((at(&p, 1000.0) - 6.0).abs() < 0.01, "{}", at(&p, 1000.0));
        assert!(at(&p, 50.0).abs() < 0.2);
        assert!(at(&p, 15_000.0).abs() < 0.2);
        let cut = profile(vec![peak(3000.0, -9.0, 4.0)]);
        assert!((at(&cut, 3000.0) + 9.0).abs() < 0.01);
    }

    #[test]
    fn shelves_and_passes() {
        let low = profile(vec![EqBand::new(EqBandKind::LowShelf, 105.0, 6.0, 0.7)]);
        assert!((at(&low, 20.0) - 6.0).abs() < 0.3, "{}", at(&low, 20.0));
        assert!((at(&low, 105.0) - 3.0).abs() < 0.3, "half gain at the corner: {}", at(&low, 105.0));
        assert!(at(&low, 5000.0).abs() < 0.1);

        let high = profile(vec![EqBand::new(EqBandKind::HighShelf, 10_000.0, -4.0, 0.7)]);
        assert!((at(&high, 20_000.0) + 4.0).abs() < 0.5, "{}", at(&high, 20_000.0));
        assert!(at(&high, 200.0).abs() < 0.1);

        let hp = profile(vec![EqBand::new(EqBandKind::HighPass, 100.0, 0.0, 0.707)]);
        assert!((at(&hp, 100.0) + 3.0).abs() < 0.1);
        assert!(at(&hp, 25.0) < -20.0);
        assert!(at(&hp, 5000.0).abs() < 0.1);
        let lp = profile(vec![EqBand::new(EqBandKind::LowPass, 5000.0, 0.0, 0.707)]);
        assert!(at(&lp, 18_000.0) < -15.0);
        assert!(at(&lp, 100.0).abs() < 0.1);
    }

    #[test]
    fn auto_preamp_leaves_room_for_the_summed_boost() {
        // two boosts that overlap sum to more than either
        let p = profile(vec![peak(1000.0, 6.0, 0.7), peak(1400.0, 6.0, 0.7)]);
        let max = response_db(&p, &log_freqs(2000), 48_000).into_iter().fold(f32::MIN, f32::max);
        assert!(max > 6.5);
        let pre = auto_preamp_db(&p);
        assert!(pre <= -max && pre > -max - 0.2, "preamp {pre}, max {max}");
        // only cuts: no preamp needed
        assert_eq!(auto_preamp_db(&profile(vec![peak(1000.0, -6.0, 1.0)])), 0.0);
    }

    #[test]
    fn response_is_nan_safe() {
        let p = profile(vec![peak(f32::NAN, f32::NAN, f32::NAN), peak(1000.0, 3.0, 1.0)]);
        let r = response_db(&p, &[f32::NAN, -5.0, 0.0, 1000.0, 1e9], 0);
        assert!(r.iter().all(|v| v.is_finite()), "{r:?}");
        assert!(auto_preamp_db(&p).is_finite());
    }

    #[test]
    fn log_freqs_span_the_audible_range() {
        let f = log_freqs(31);
        assert_eq!(f.len(), 31);
        assert!((f[0] - 20.0).abs() < 1e-3 && (f[30] - 20_000.0).abs() < 0.5);
        assert!((f[15] - 632.5).abs() < 1.0, "geometric middle {}", f[15]);
        assert!(f.windows(2).all(|w| w[0] < w[1]));
        assert!(log_freqs(0).is_empty());
    }

    #[test]
    fn settings_pick_the_profile_of_the_output() {
        let hd650 = profile(vec![peak(1000.0, 3.0, 1.0)]);
        let mut s = EqSettings { enabled: true, default_profile: Some(EqProfile::flat("flat")), ..Default::default() };
        s.per_output.insert("USB DAC".into(), hd650.clone());
        assert_eq!(s.profile_for("USB DAC"), Some(&hd650));
        assert_eq!(s.profile_for("Speakers").map(|p| p.name.as_str()), Some("flat"));
        assert!(s.active_profile("USB DAC").is_some());
        // the default is flat: nothing to run, bit-perfect
        assert!(s.active_profile("Speakers").is_none());
        assert!(s.design_for("Speakers", 48_000).is_bypass());
        assert!(!s.design_for("USB DAC", 48_000).is_bypass());
        s.enabled = false;
        assert!(s.active_profile("USB DAC").is_none());
        // a manual preamp alone changes the level
        let mut s = EqSettings { enabled: true, auto_preamp: false, ..Default::default() };
        s.default_profile = Some(EqProfile { preamp_db: -3.0, ..Default::default() });
        assert!(s.active_profile("any").is_some());
    }

    #[test]
    fn taste_joins_autoeq_shelves() {
        let base = EqProfile {
            preamp_db: -2.0,
            bands: vec![
                EqBand::new(EqBandKind::LowShelf, 105.0, 2.0, 0.7),
                peak(3169.0, -1.7, 3.89),
                EqBand::new(EqBandKind::HighShelf, 10_000.0, -2.0, 0.7),
            ],
            ..Default::default()
        };
        let t = with_taste(&base, Tilt { bass_db: 3.0, treble_db: 1.0, clarity_db: Some(2.0) });
        // shelves absorbed the tilt; the narrow peak did not take the broad clarity bump
        assert_eq!(t.bands.len(), 4);
        assert_eq!(t.bands[0].gain_db, 5.0);
        assert_eq!(t.bands[2].gain_db, -1.0);
        assert_eq!(t.bands[1].gain_db, -1.7);
        assert_eq!(t.bands[3].kind, EqBandKind::Peak);
        assert_eq!(t.bands[3].freq_hz, TASTE_CLARITY_HZ);
        // the bigger boost needs more headroom
        assert!(t.preamp_db <= auto_preamp_db(&t) && t.preamp_db < -2.0);
        // nothing to do
        assert_eq!(with_taste(&base, Tilt::default()), base);
    }

    #[test]
    fn taste_never_exceeds_the_band_limit() {
        let full = profile((0..MAX_BANDS).map(|i| peak(200.0 * (i + 1) as f32, 1.0 + i as f32, 4.0)).collect());
        let t = with_taste(&full, Tilt { bass_db: 4.0, treble_db: -3.0, clarity_db: None });
        assert_eq!(t.bands.len(), MAX_BANDS);
        assert!(t.bands.iter().any(|b| b.kind == EqBandKind::LowShelf && b.gain_db == 4.0));
        assert!(t.bands.iter().any(|b| b.kind == EqBandKind::HighShelf && b.gain_db == -3.0));
        // the two smallest boosts made way
        assert!(!t.bands.iter().any(|b| b.gain_db == 1.0 || b.gain_db == 2.0));
    }

    fn genre_ctx(genre: &str) -> NowPlayingContext {
        NowPlayingContext { genres: vec![genre.into()], ..Default::default() }
    }

    fn electronic_bass(db: f32) -> TasteSettings {
        let mut t = TasteSettings::default();
        t.upsert(TasteRule::new(
            TasteScope::Genre { name: "Electronic".into() },
            Tilt { bass_db: db, ..Default::default() },
            "More punch",
            TasteCreator::Manual,
        ));
        t
    }

    #[test]
    fn device_and_taste_layers_combine() {
        let hd650 = EqProfile {
            name: "HD 650".into(),
            bands: vec![EqBand::new(EqBandKind::LowShelf, 105.0, 6.0, 0.7)],
            ..Default::default()
        };
        let mut eq = EqSettings { enabled: true, ..Default::default() };
        eq.per_output.insert("USB DAC".into(), hd650.clone());
        let taste = electronic_bass(3.0);
        // electronic: the device shelf plus the taste
        let (p, rule) = track_profile(&eq, &taste, "USB DAC", &genre_ctx("Electronic")).unwrap();
        assert_eq!(rule.unwrap().label, "More punch");
        assert_eq!(p.bands[0].gain_db, 9.0);
        assert_eq!(p.preamp_db, auto_preamp_db(&p));
        // jazz: the device alone
        let (p, rule) = track_profile(&eq, &taste, "USB DAC", &genre_ctx("Jazz")).unwrap();
        assert!(rule.is_none() && p.bands[0].gain_db == 6.0);
        // the equalizer off: the taste layer still plays, on a flat base
        eq.enabled = false;
        let (p, _) = track_profile(&eq, &taste, "USB DAC", &genre_ctx("Electronic")).unwrap();
        assert_eq!(p.bands.len(), 1);
        assert!(track_profile(&eq, &taste, "USB DAC", &genre_ctx("Jazz")).is_none(), "bit-perfect");
        // layers off: nothing
        let mut off = taste.clone();
        off.enabled = false;
        assert!(track_profile(&eq, &off, "USB DAC", &genre_ctx("Electronic")).is_none());

        let plan = sound_plan(&eq, &taste, "USB DAC", &genre_ctx("Jazz"), -4.0, 48_000);
        assert!(!plan.eq_active && !plan.design.is_bypass(), "loudness alone is a gain stage");
        assert!((plan.design.gain() - 10f64.powf(-0.2)).abs() < 1e-6);
        assert!(sound_plan(&eq, &taste, "USB DAC", &genre_ctx("Jazz"), 0.0, 48_000).design.is_bypass());
        let plan = sound_plan(&eq, &taste, "USB DAC", &genre_ctx("Electronic"), 0.0, 48_000);
        assert!(plan.eq_active && plan.taste_rule.is_some() && plan.profile.is_none());
    }

    #[test]
    fn small_speakers_get_a_low_mid_lift_instead_of_deep_bass() {
        assert!(is_built_in_speaker("MacBook Pro Speakers"));
        assert!(is_built_in_speaker("Phone speaker"));
        assert!(!is_built_in_speaker("External Headphones"));
        assert!(!is_built_in_speaker("USB DAC"));
        let p = with_taste_for(&EqProfile::flat(""), Tilt { bass_db: 6.0, ..Default::default() }, true);
        assert!(!p.bands.iter().any(|b| b.kind == EqBandKind::LowShelf), "{:?}", p.bands);
        let lift = p.bands.iter().find(|b| b.kind == EqBandKind::Peak).unwrap();
        assert_eq!((lift.freq_hz, lift.gain_db), (SMALL_SPEAKER_BASS_HZ, SMALL_SPEAKER_MAX_BASS_DB));
        assert!(p.bands.iter().any(|b| b.kind == EqBandKind::HighPass && b.freq_hz == SMALL_SPEAKER_HIGHPASS_HZ));
        // deep bass is not boosted at all
        assert!(at(&p, 40.0) < 0.0, "{}", at(&p, 40.0));
        // a cut stays a cut, and headphones keep the shelf
        let cut = with_taste_for(&EqProfile::flat(""), Tilt { bass_db: -2.0, ..Default::default() }, true);
        assert_eq!(cut.bands[0].kind, EqBandKind::LowShelf);
        let hp = with_taste_for(&EqProfile::flat(""), Tilt { bass_db: 4.0, ..Default::default() }, false);
        assert_eq!(hp.bands[0].kind, EqBandKind::LowShelf);
        assert_eq!(safe_tilt(Tilt { bass_db: 5.0, ..Default::default() }, true).bass_db, 3.0);
        // taste rules on a laptop's speakers
        let taste = electronic_bass(4.0);
        let (p, _) =
            track_profile(&EqSettings::default(), &taste, "MacBook Air Speakers", &genre_ctx("Electronic")).unwrap();
        assert!(p.bands.iter().any(|b| b.kind == EqBandKind::HighPass));
    }

    #[test]
    fn biquads_put_the_preamp_first() {
        let p = EqProfile { preamp_db: -6.0, bands: vec![peak(1000.0, 6.0, 1.0)], ..Default::default() };
        let b = biquads(&p, 48_000);
        assert_eq!(b.len(), 2);
        assert!((b[0].b0 - 10f64.powf(-6.0 / 20.0)).abs() < 1e-12);
        assert_eq!((b[0].b1, b[0].b2, b[0].a1, b[0].a2), (0.0, 0.0, 0.0, 0.0));
        assert!((b[1].magnitude_db(1000.0, 48_000) - 6.0).abs() < 1e-6);
    }
}
