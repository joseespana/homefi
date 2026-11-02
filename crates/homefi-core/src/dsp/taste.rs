//! Sounds: a curve that plays for any number of targets (songs, albums,
//! artists, playlists, genres) and, optionally, only on some outputs, on top
//! of the output's own profile. A sound is a [`Tilt`] (three broad knobs)
//! and / or a parametric curve of up to [`MAX_TASTE_BANDS`] [`TasteBand`]s.
//! The most specific target that matches what plays wins:
//! track > album > artist > playlist > genre. One target belongs to at most
//! one sound per output.
//!
//! Persisted like [`super::EqSettings`]: stable camelCase JSON, and an
//! unreadable rule is dropped instead of failing the whole file.

use serde::{Deserialize, Deserializer, Serialize};

use super::{EqBand, EqBandKind, Tilt};

/// Targets one sound can hold (bounded growth; a full sound refuses more).
pub const MAX_TARGETS: usize = 500;
/// `TasteSettings::version` written by this release (0 = files from 1.5.0 and older).
pub const TASTE_VERSION: u8 = 1;
/// Each taste adjustment (bass, treble, clarity) stays within ± this (dB).
pub const MAX_TILT_DB: f32 = 6.0;
/// Rules kept at most (oldest manual ones are never dropped silently: callers cap first).
pub const MAX_RULES: usize = 200;
/// Bands of a rule's parametric curve.
pub const MAX_TASTE_BANDS: usize = 8;
/// Each taste band boosts or cuts at most this (dB).
pub const MAX_TASTE_BAND_DB: f32 = 9.0;
/// The whole taste layer (bands and tilt together) never boosts more than
/// this anywhere (dB); larger boosts are scaled down.
pub const MAX_TASTE_BOOST_DB: f32 = 12.0;
/// Width of taste peaks: from very broad to surgical (no needle-thin notches).
pub const MIN_TASTE_Q: f32 = 0.3;
pub const MAX_TASTE_Q: f32 = 8.0;
/// Shelves and pass filters past this Q ring (a bump at the corner).
pub const MAX_TASTE_SLOPE_Q: f32 = 2.0;
/// A taste high-pass only trims rumble / what a small speaker can't play...
pub const MAX_TASTE_HIGHPASS_HZ: f32 = 300.0;
/// ...and a low-pass only tames the very top.
pub const MIN_TASTE_LOWPASS_HZ: f32 = 4_000.0;
/// Below this, small speakers get no boost: it moves to [`super::SMALL_SPEAKER_BASS_HZ`].
pub const SMALL_SPEAKER_MIN_BOOST_HZ: f32 = 150.0;

/// A Tidal id (or a file path for local tracks) and a name. Two refs are the
/// same thing when both ids are set and equal, else when the names match
/// (case-insensitive): local files only know names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ItemRef {
    pub id: String,
    pub name: String,
}

impl ItemRef {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self { id: id.into(), name: name.into() }
    }

    /// A ref known only by name (local tags).
    pub fn named(name: impl Into<String>) -> Self {
        Self { id: String::new(), name: name.into() }
    }

    pub fn is_empty(&self) -> bool {
        self.id.trim().is_empty() && self.name.trim().is_empty()
    }

    /// See the type's docs.
    pub fn matches(&self, other: &ItemRef) -> bool {
        let (a, b) = (self.id.trim(), other.id.trim());
        if !a.is_empty() && !b.is_empty() {
            return a == b;
        }
        let (a, b) = (self.name.trim(), other.name.trim());
        !a.is_empty() && a.to_lowercase() == b.to_lowercase()
    }
}

/// What a rule applies to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TasteScope {
    Genre {
        name: String,
    },
    Playlist {
        id: String,
        name: String,
    },
    Artist {
        id: String,
        name: String,
    },
    Album {
        id: String,
        name: String,
    },
    /// `id` is the Tidal id, or the file path of a local track.
    Track {
        id: String,
        name: String,
    },
}

impl TasteScope {
    /// Higher wins: track 5, album 4, artist 3, playlist 2, genre 1.
    pub fn specificity(&self) -> u8 {
        match self {
            Self::Track { .. } => 5,
            Self::Album { .. } => 4,
            Self::Artist { .. } => 3,
            Self::Playlist { .. } => 2,
            Self::Genre { .. } => 1,
        }
    }

    /// `genre`, `playlist`, `artist`, `album` or `track`.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Genre { .. } => "genre",
            Self::Playlist { .. } => "playlist",
            Self::Artist { .. } => "artist",
            Self::Album { .. } => "album",
            Self::Track { .. } => "track",
        }
    }

    /// The genre's name, or the item's.
    pub fn name(&self) -> &str {
        match self {
            Self::Genre { name } => name,
            Self::Playlist { name, .. } | Self::Artist { name, .. } | Self::Album { name, .. } => name,
            Self::Track { name, .. } => name,
        }
    }

    fn item(&self) -> Option<ItemRef> {
        match self {
            Self::Genre { .. } => None,
            Self::Playlist { id, name }
            | Self::Artist { id, name }
            | Self::Album { id, name }
            | Self::Track { id, name } => Some(ItemRef::new(id.clone(), name.clone())),
        }
    }

    /// The same music: same kind and matching genre / item.
    pub fn same_as(&self, other: &TasteScope) -> bool {
        if self.kind() != other.kind() {
            return false;
        }
        match (self, other) {
            (Self::Genre { name: a }, Self::Genre { name: b }) => genre_matches(a, b),
            _ => match (self.item(), other.item()) {
                (Some(a), Some(b)) => a.matches(&b),
                _ => false,
            },
        }
    }

    /// How well the rule fits `ctx`: `None` when it doesn't apply, else a rank
    /// where higher is better (specificity first, then the main genre / artist).
    fn fit(&self, ctx: &NowPlayingContext) -> Option<(u8, i32)> {
        let rank = self.specificity();
        let first_of = |pos: Option<usize>| pos.map(|p| (rank, -(p as i32)));
        match self {
            Self::Genre { name } => first_of(ctx.genres.iter().position(|g| genre_matches(g, name))),
            Self::Artist { .. } => {
                let me = self.item()?;
                first_of(ctx.artists.iter().position(|a| a.matches(&me)))
            }
            Self::Album { .. } => {
                let me = self.item()?;
                ctx.album.as_ref().filter(|a| a.matches(&me)).map(|_| (rank, 0))
            }
            Self::Playlist { .. } => {
                let me = self.item()?;
                ctx.playlist().filter(|p| p.matches(&me)).map(|_| (rank, 0))
            }
            Self::Track { id, name } => {
                let id = id.trim();
                let hit = !id.is_empty()
                    && (ctx.track_id.as_deref().map(str::trim) == Some(id)
                        || ctx.path.as_deref().map(str::trim) == Some(id));
                // a track known only by its title (a rule made from a list without ids)
                let by_name = id.is_empty()
                    && !name.trim().is_empty()
                    && ctx.title.as_deref().is_some_and(|t| t.trim().eq_ignore_ascii_case(name.trim()));
                (hit || by_name).then_some((rank, 0))
            }
        }
    }
}

/// Who made a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum TasteCreator {
    #[default]
    Manual,
    Ai,
    Guided,
    /// A "How does it sound?" reaction.
    Reaction,
}

/// One band of a taste curve: an [`EqBand`] (same JSON fields) and, when the
/// assistant made it, one plain line on what it does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TasteBand {
    #[serde(flatten)]
    pub band: EqBand,
    /// "Softens the harsh edge of the cymbals" (may be empty).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

impl TasteBand {
    pub fn new(kind: EqBandKind, freq_hz: f32, gain_db: f32, q: f32) -> Self {
        Self { band: EqBand::new(kind, freq_hz, gain_db, q), note: String::new() }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = note.into();
        self
    }

    /// The same filter at about the same place: same kind, within a third of
    /// an octave (peaks also of a similar width). Used to edit a curve instead
    /// of stacking a second band on top of the first.
    pub fn same_place(&self, other: &EqBand) -> bool {
        let (a, b) = (&self.band, other);
        a.kind == b.kind
            && (a.freq_hz / b.freq_hz).log2().abs() <= 1.0 / 3.0
            && (a.kind != EqBandKind::Peak || (a.q / b.q).log2().abs() <= 1.5)
    }
}

impl From<EqBand> for TasteBand {
    fn from(band: EqBand) -> Self {
        Self { band, note: String::new() }
    }
}

/// One piece of music a sound applies to, and when / by whom it was added.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TasteTarget {
    pub scope: TasteScope,
    /// Unix ms; 0 = unknown (migrated).
    #[serde(default)]
    pub added_ms: u64,
    #[serde(default, deserialize_with = "lenient_creator")]
    pub added_by: TasteCreator,
}

impl TasteTarget {
    /// `scope`, added now by `by`.
    fn now(scope: TasteScope, by: TasteCreator) -> Self {
        Self { scope, added_ms: now_ms(), added_by: by }
    }
}

/// Unix ms now (0 if the clock is before 1970).
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// A "sound": a curve that plays for its targets on its outputs.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TasteRule {
    pub id: String,
    /// The three broad knobs (simple rules; kept for old rules).
    pub tilt: Tilt,
    /// The parametric curve, on top of the tilt (at most [`MAX_TASTE_BANDS`]).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub bands: Vec<TasteBand>,
    /// What the user sees ("More punch for Electronic").
    pub label: String,
    /// Why, in a sentence or two (the assistant's words; may be empty).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub explanation: String,
    pub created_by: TasteCreator,
    /// Kept for old files; no UI toggle in 1.5.1. Disabled sounds never play.
    pub enabled: bool,
    /// Music it applies to (any number, may be empty: an empty sound is kept).
    pub targets: Vec<TasteTarget>,
    /// Output ids it plays on; empty = every output.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<String>,
}

/// What a saved sound looks like on disk, old (`scope`) and new (`targets`).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RuleJson {
    id: String,
    tilt: Tilt,
    #[serde(deserialize_with = "lenient_bands")]
    bands: Vec<TasteBand>,
    label: String,
    explanation: String,
    #[serde(deserialize_with = "lenient_creator")]
    created_by: TasteCreator,
    enabled: bool,
    scope: Option<serde_json::Value>,
    targets: Option<Vec<serde_json::Value>>,
    outputs: Option<Vec<String>>,
}

impl Default for RuleJson {
    fn default() -> Self {
        Self {
            id: String::new(),
            tilt: Tilt::default(),
            bands: Vec::new(),
            label: String::new(),
            explanation: String::new(),
            created_by: TasteCreator::Manual,
            enabled: true,
            scope: None,
            targets: None,
            outputs: None,
        }
    }
}

impl<'de> Deserialize<'de> for TasteRule {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let j = RuleJson::deserialize(d)?;
        // unreadable targets (a kind from a newer version) are dropped, the sound is kept
        let targets = match (j.targets, j.scope) {
            (Some(list), _) => list.into_iter().filter_map(|v| serde_json::from_value(v).ok()).collect(),
            (None, Some(scope)) => serde_json::from_value::<TasteScope>(scope)
                .map(|scope| vec![TasteTarget { scope, added_ms: 0, added_by: j.created_by }])
                .unwrap_or_default(),
            (None, None) => Vec::new(),
        };
        Ok(Self {
            id: j.id,
            tilt: j.tilt,
            bands: j.bands,
            label: j.label,
            explanation: j.explanation,
            created_by: j.created_by,
            enabled: j.enabled,
            targets,
            outputs: clean_outputs(j.outputs.unwrap_or_default()),
        })
    }
}

/// Output ids trimmed, empties dropped, duplicates removed (order kept).
fn clean_outputs(outputs: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(outputs.len());
    for o in outputs {
        let o = o.trim();
        if !o.is_empty() && !out.iter().any(|x| x == o) {
            out.push(o.to_owned());
        }
    }
    out
}

/// `targets` without repeats (first wins), at most [`MAX_TARGETS`].
fn clean_targets(targets: Vec<TasteTarget>) -> Vec<TasteTarget> {
    let mut out: Vec<TasteTarget> = Vec::with_capacity(targets.len().min(MAX_TARGETS));
    for t in targets {
        if out.len() >= MAX_TARGETS {
            break;
        }
        if !out.iter().any(|o| o.scope.same_as(&t.scope)) {
            out.push(t);
        }
    }
    out
}

impl TasteRule {
    /// A new, enabled sound with a fresh id for every output; the tilt is
    /// brought in range, the targets deduped and added now by `created_by`.
    pub fn new(targets: Vec<TasteScope>, tilt: Tilt, label: impl Into<String>, created_by: TasteCreator) -> Self {
        let targets = clean_targets(targets.into_iter().map(|s| TasteTarget::now(s, created_by)).collect());
        Self {
            id: new_id(),
            tilt: tilt.clamped(MAX_TILT_DB),
            bands: Vec::new(),
            label: label.into(),
            explanation: String::new(),
            created_by,
            enabled: true,
            targets,
            outputs: Vec::new(),
        }
    }

    /// A sound for one target.
    pub fn for_target(scope: TasteScope, tilt: Tilt, label: impl Into<String>, created_by: TasteCreator) -> Self {
        Self::new(vec![scope], tilt, label, created_by)
    }

    /// The rule with `bands` as its curve (brought in range, see [`clamp_taste_curve`]).
    pub fn with_bands(mut self, bands: Vec<TasteBand>) -> Self {
        self.bands = bands;
        self.clamp();
        self
    }

    /// Tilt and curve in range, in place (any output; see [`clamp_taste_curve`]).
    pub fn clamp(&mut self) {
        let (bands, tilt, _) = clamp_taste_curve(&self.bands, self.tilt, false);
        self.bands = bands;
        self.tilt = tilt;
    }

    /// Everything a stored sound must satisfy: curve in range, targets
    /// deduped and capped, outputs clean.
    fn normalize(&mut self) {
        self.clamp();
        self.targets = clean_targets(std::mem::take(&mut self.targets));
        self.outputs = clean_outputs(std::mem::take(&mut self.outputs));
    }

    /// `true` when it changes nothing (a neutral tilt and no audible band).
    pub fn is_neutral(&self) -> bool {
        self.tilt.is_neutral() && self.bands.iter().all(|b| b.band.is_neutral())
    }

    /// The curve's filters for the kind of output (clamped, audible ones only).
    pub fn curve_for(&self, small_speaker: bool) -> (Vec<EqBand>, Tilt) {
        let (bands, tilt, _) = clamp_taste_curve(&self.bands, self.tilt, small_speaker);
        (bands.into_iter().map(|b| b.band).filter(|b| !b.is_neutral()).collect(), tilt)
    }

    /// Whether `scope` is one of its targets.
    pub fn covers(&self, scope: &TasteScope) -> bool {
        self.targets.iter().any(|t| t.scope.same_as(scope))
    }

    /// Whether it plays on `output_id` (no outputs listed = on every one).
    pub fn plays_on(&self, output_id: &str) -> bool {
        self.outputs.is_empty() || self.outputs.iter().any(|o| o == output_id)
    }

    /// Whether the two can play at the same time on some output.
    pub fn overlaps_outputs(&self, other: &TasteRule) -> bool {
        self.outputs.is_empty() || other.outputs.is_empty() || self.outputs.iter().any(|o| other.outputs.contains(o))
    }

    /// First target (for one-line labels and prompts); `None` when empty.
    pub fn main_target(&self) -> Option<&TasteScope> {
        self.targets.first().map(|t| &t.scope)
    }
}

/// `bands` and `tilt` brought within the taste caps, plus what had to change
/// (plain English, for the assistant to explain):
/// - at most [`MAX_TASTE_BANDS`] bands; every value finite and in range;
/// - each band within ±[`MAX_TASTE_BAND_DB`]; peaks Q [`MIN_TASTE_Q`]..[`MAX_TASTE_Q`],
///   shelves and passes up to [`MAX_TASTE_SLOPE_Q`];
/// - a high-pass at most at [`MAX_TASTE_HIGHPASS_HZ`], a low-pass at least at [`MIN_TASTE_LOWPASS_HZ`];
/// - on a `small_speaker`, boosts below [`SMALL_SPEAKER_MIN_BOOST_HZ`] become a
///   ≤ [`super::SMALL_SPEAKER_MAX_BASS_DB`] lift at [`super::SMALL_SPEAKER_BASS_HZ`]
///   with a high-pass below (they can't play deep bass);
/// - the summed boost of bands and tilt at most [`MAX_TASTE_BOOST_DB`] anywhere.
pub fn clamp_taste_curve(bands: &[TasteBand], tilt: Tilt, small_speaker: bool) -> (Vec<TasteBand>, Tilt, Vec<String>) {
    use super::{SMALL_SPEAKER_BASS_HZ, SMALL_SPEAKER_HIGHPASS_HZ, SMALL_SPEAKER_MAX_BASS_DB};
    let mut notes = Vec::new();
    let mut tilt = tilt.clamped(MAX_TILT_DB);
    if small_speaker && tilt.bass_db > SMALL_SPEAKER_MAX_BASS_DB {
        tilt.bass_db = SMALL_SPEAKER_MAX_BASS_DB;
    }
    if bands.len() > MAX_TASTE_BANDS {
        notes.push(format!("only the first {MAX_TASTE_BANDS} bands are kept"));
    }
    let mut out: Vec<TasteBand> = Vec::with_capacity(bands.len().min(MAX_TASTE_BANDS));
    for tb in bands.iter().take(MAX_TASTE_BANDS) {
        let asked = tb.band;
        let mut b = asked.clamped();
        let at = hz(b.freq_hz);
        if b.kind.has_gain() && b.gain_db.abs() > MAX_TASTE_BAND_DB {
            notes.push(format!(
                "{at}: {:+.1} dB lowered to {:+.0} dB (each band ±{MAX_TASTE_BAND_DB:.0})",
                b.gain_db,
                MAX_TASTE_BAND_DB.copysign(b.gain_db)
            ));
            b.gain_db = b.gain_db.clamp(-MAX_TASTE_BAND_DB, MAX_TASTE_BAND_DB);
        }
        let (lo, hi) =
            if b.kind == EqBandKind::Peak { (MIN_TASTE_Q, MAX_TASTE_Q) } else { (MIN_TASTE_Q, MAX_TASTE_SLOPE_Q) };
        if b.q < lo || b.q > hi {
            let q = b.q.clamp(lo, hi);
            notes.push(format!("{at}: Q {:.2} set to {q:.2} ({lo}..{hi} for this kind)", b.q));
            b.q = q;
        }
        if b.kind == EqBandKind::HighPass && b.freq_hz > MAX_TASTE_HIGHPASS_HZ {
            notes.push(format!(
                "high-pass moved from {at} to {MAX_TASTE_HIGHPASS_HZ:.0} Hz (it only trims the lowest bass)"
            ));
            b.freq_hz = MAX_TASTE_HIGHPASS_HZ;
        }
        if b.kind == EqBandKind::LowPass && b.freq_hz < MIN_TASTE_LOWPASS_HZ {
            notes
                .push(format!("low-pass moved from {at} to {} (it only tames the very top)", hz(MIN_TASTE_LOWPASS_HZ)));
            b.freq_hz = MIN_TASTE_LOWPASS_HZ;
        }
        if small_speaker
            && b.enabled
            && b.gain_db > 0.0
            && matches!(b.kind, EqBandKind::Peak | EqBandKind::LowShelf)
            && b.freq_hz < SMALL_SPEAKER_MIN_BOOST_HZ
        {
            notes.push(format!(
                "built-in speaker: the {at} boost became a {SMALL_SPEAKER_BASS_HZ:.0} Hz lift (they can't play deep bass)"
            ));
            b = EqBand::new(EqBandKind::Peak, SMALL_SPEAKER_BASS_HZ, b.gain_db.min(SMALL_SPEAKER_MAX_BASS_DB), 0.9);
        }
        out.push(TasteBand { band: b, note: tb.note.trim().chars().take(140).collect() });
    }
    let moved = notes.iter().any(|n| n.starts_with("built-in speaker"));
    if moved
        && !out.iter().any(|b| b.band.enabled && b.band.kind == EqBandKind::HighPass)
        && out.len() < MAX_TASTE_BANDS
    {
        out.push(TasteBand::new(EqBandKind::HighPass, SMALL_SPEAKER_HIGHPASS_HZ, 0.0, 0.707));
    }
    if scale_boosts_down(&mut out, &mut tilt, MAX_TASTE_BOOST_DB) {
        notes.push(format!("boosts scaled down: the whole taste curve stays within +{MAX_TASTE_BOOST_DB:.0} dB"));
    }
    (out, tilt, notes)
}

/// Scales every boost of `bands` and `tilt` down until the curve boosts at
/// most `max_boost_db` anywhere; `true` when it had to.
fn scale_boosts_down(bands: &mut [TasteBand], tilt: &mut Tilt, max_boost_db: f32) -> bool {
    let mut scaled = false;
    for _ in 0..6 {
        let peak = taste_peak_db(bands, *tilt, max_boost_db);
        if peak <= max_boost_db + 0.05 {
            break;
        }
        let k = (max_boost_db - 0.05) / peak;
        for b in bands.iter_mut().filter(|b| b.band.kind.has_gain() && b.band.gain_db > 0.0) {
            b.band.gain_db = (b.band.gain_db * k * 10.0).floor() / 10.0;
        }
        for v in [&mut tilt.bass_db, &mut tilt.treble_db] {
            if *v > 0.0 {
                *v = (*v * k * 10.0).floor() / 10.0;
            }
        }
        if let Some(c) = tilt.clarity_db.as_mut().filter(|c| **c > 0.0) {
            *c = (*c * k * 10.0).floor() / 10.0;
        }
        scaled = true;
    }
    scaled
}

/// Largest boost (dB) of a taste curve: its bands and tilt shelves summed.
fn taste_peak_db(bands: &[TasteBand], tilt: Tilt, max_boost_db: f32) -> f32 {
    // cheap bound first (a band never lifts much past its gain): most rules stop here
    let boosts = |v: f32| v.max(0.0);
    let gains = bands.iter().filter(|b| b.band.enabled && b.band.kind.has_gain()).map(|b| boosts(b.band.gain_db));
    let bound =
        gains.sum::<f32>() + boosts(tilt.bass_db) + boosts(tilt.treble_db) + boosts(tilt.clarity_db.unwrap_or(0.0));
    if bound * 1.25 <= max_boost_db {
        return bound;
    }
    let p = super::taste_profile(bands, tilt);
    super::response_db(&p, &super::log_freqs(256), 192_000).into_iter().fold(0f32, f32::max)
}

/// `120 Hz`, `3.2 kHz`, `10 kHz` (for messages).
pub fn hz(f: f32) -> String {
    if f >= 1000.0 {
        let k = f / 1000.0;
        let s = if k >= 10.0 { format!("{k:.0}") } else { format!("{k:.1}") };
        format!("{} kHz", s.trim_end_matches(".0"))
    } else {
        format!("{f:.0} Hz")
    }
}

/// `3.2k`, `120 Hz` (for short summaries).
fn hz_short(f: f32) -> String {
    if f >= 1000.0 {
        hz(f).replace(" kHz", "k")
    } else {
        format!("{f:.0} Hz")
    }
}

/// `-2 dB 3.2k`, `HP 80 Hz`.
pub fn band_words(b: &EqBand) -> String {
    match b.kind {
        EqBandKind::HighPass => format!("HP {}", hz_short(b.freq_hz)),
        EqBandKind::LowPass => format!("LP {}", hz_short(b.freq_hz)),
        _ => format!("{} dB {}", db_words(b.gain_db), hz_short(b.freq_hz)),
    }
}

/// `+1.5`, `-2`.
fn db_words(db: f32) -> String {
    format!("{db:+.1}").replace(".0", "")
}

/// `+2 dB bass, -1 dB treble` / `+2 dB de bajo, -1 dB de agudos`.
pub fn tilt_words(t: Tilt, es: bool) -> String {
    let mut parts = Vec::new();
    let mut push = |db: f32, en: &str, es_word: &str| {
        if db.abs() >= 0.05 {
            let v = format!("{db:+.1}").replace(".0", "");
            parts.push(if es { format!("{v} dB de {es_word}") } else { format!("{v} dB {en}") });
        }
    };
    push(t.bass_db, "bass", "bajo");
    push(t.treble_db, "treble", "agudos");
    push(t.clarity_db.unwrap_or(0.0), "clarity", "claridad");
    if parts.is_empty() {
        return if es { "sin cambios" } else { "no change" }.into();
    }
    parts.join(", ")
}

/// A rule's curve in a line: `3 bands · -2 dB 3.2k, +1.5 dB 120 Hz, HP 80 Hz`
/// (biggest first, at most three, then `…`), with the tilt after it when it
/// changes something. Spanish when `es`.
pub fn curve_summary(rule: &TasteRule, es: bool) -> String {
    let bands: Vec<&EqBand> = rule.bands.iter().map(|b| &b.band).filter(|b| !b.is_neutral()).collect();
    let mut parts = Vec::new();
    if !bands.is_empty() {
        let mut sorted = bands.clone();
        let weight = |b: &EqBand| if b.kind.has_gain() { b.gain_db.abs() } else { 0.5 };
        sorted.sort_by(|a, b| weight(b).total_cmp(&weight(a)));
        let mut list: Vec<String> = sorted.iter().take(3).map(|b| band_words(b)).collect();
        if sorted.len() > 3 {
            list.push("…".into());
        }
        let n = bands.len();
        let count = match (n, es) {
            (1, false) => "1 band".to_owned(),
            (1, true) => "1 banda".to_owned(),
            (n, false) => format!("{n} bands"),
            (n, true) => format!("{n} bandas"),
        };
        parts.push(format!("{count} · {}", list.join(", ")));
    }
    if !rule.tilt.is_neutral() {
        parts.push(tilt_words(rule.tilt, es));
    }
    if parts.is_empty() {
        return if es { "sin cambios" } else { "no change" }.into();
    }
    parts.join(" · ")
}

/// A fresh rule id (time and a counter, unique within a run and across runs).
pub fn new_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    format!("t{:x}{:x}", now.as_millis(), COUNTER.fetch_add(1, Ordering::Relaxed) & 0xfff)
}

/// How strong the sounds the assistant, the reactions and the guided wizard
/// PROPOSE are. It only narrows what they suggest (see [`TasteIntensity::limits`]);
/// rules the listener saves or edits keep the hard caps above.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum TasteIntensity {
    /// Light touch: changes you notice only side by side.
    Subtle,
    /// A clearly audible change that is still musical.
    #[default]
    Balanced,
    /// Big, unmistakable changes (up to the hard caps).
    Bold,
}

/// Factor applied to the fixed steps of the guided wizard for [`TasteIntensity::Subtle`].
const SUBTLE_SCALE: f32 = 0.6;
/// ...and for [`TasteIntensity::Bold`] (the prompts say "Subtle ×0.6, Bold ×1.5").
const BOLD_SCALE: f32 = 1.5;

/// What a [`TasteIntensity`] lets a proposal do. Never beyond the hard caps
/// ([`MAX_TASTE_BAND_DB`], [`MAX_TILT_DB`], [`MAX_TASTE_BOOST_DB`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TasteLimits {
    /// The main move of a proposal (its biggest band), as `(smallest, largest)` dB:
    /// what "clearly audible" means at this intensity.
    pub main_move_db: (f32, f32),
    /// Each proposed band stays within ± this (dB).
    pub band_db: f32,
    /// Each proposed tilt knob stays within ± this (dB).
    pub tilt_db: f32,
    /// A proposed curve never boosts more than this anywhere (dB).
    pub boost_db: f32,
    /// A reaction moves each value at most this much per tap (dB).
    pub step_db: f32,
    /// A reaction changes at most this many curve bands per tap.
    pub bands_per_tap: usize,
}

impl TasteIntensity {
    pub const ALL: [TasteIntensity; 3] = [Self::Subtle, Self::Balanced, Self::Bold];

    pub const fn limits(self) -> TasteLimits {
        match self {
            Self::Subtle => TasteLimits {
                main_move_db: (1.5, 3.0),
                band_db: 5.0,
                tilt_db: 4.0,
                boost_db: 6.0,
                step_db: 2.0,
                bands_per_tap: 2,
            },
            Self::Balanced => TasteLimits {
                main_move_db: (3.0, 5.0),
                band_db: 7.0,
                tilt_db: 6.0,
                boost_db: 9.0,
                step_db: 3.0,
                bands_per_tap: 2,
            },
            Self::Bold => TasteLimits {
                main_move_db: (5.0, 8.0),
                band_db: MAX_TASTE_BAND_DB,
                tilt_db: MAX_TILT_DB,
                boost_db: MAX_TASTE_BOOST_DB,
                step_db: 4.0,
                bands_per_tap: 3,
            },
        }
    }

    /// Multiplier of a Balanced step (Subtle 0.6, Balanced 1, Bold 1.5).
    pub const fn scale(self) -> f32 {
        match self {
            Self::Subtle => SUBTLE_SCALE,
            Self::Balanced => 1.0,
            Self::Bold => BOLD_SCALE,
        }
    }

    /// `subtle`, `balanced` or `bold` (what the prompts and the settings call it).
    pub const fn id(self) -> &'static str {
        match self {
            Self::Subtle => "subtle",
            Self::Balanced => "balanced",
            Self::Bold => "bold",
        }
    }
}

/// `bands` and `tilt` narrowed to what `limits` allows a proposal, plus what
/// had to change (plain English, for the assistant to explain). Run it after
/// [`clamp_taste_curve`]: it only ever narrows.
pub fn clamp_taste_to(bands: &[TasteBand], tilt: Tilt, limits: &TasteLimits) -> (Vec<TasteBand>, Tilt, Vec<String>) {
    let mut notes = Vec::new();
    let mut out = bands.to_vec();
    for tb in out.iter_mut().filter(|b| b.band.kind.has_gain() && b.band.gain_db.abs() > limits.band_db) {
        let to = limits.band_db.copysign(tb.band.gain_db);
        notes.push(format!(
            "{}: {:+.1} dB lowered to {to:+.0} dB (each band ±{:.0} at this intensity)",
            hz(tb.band.freq_hz),
            tb.band.gain_db,
            limits.band_db
        ));
        tb.band.gain_db = to;
    }
    let mut tilt_out = tilt.clamped(limits.tilt_db);
    if tilt_out != tilt {
        notes.push(format!("tilt knobs limited to ±{:.0} dB at this intensity", limits.tilt_db));
    }
    if scale_boosts_down(&mut out, &mut tilt_out, limits.boost_db) {
        notes.push(format!("boosts scaled down: the curve stays within +{:.0} dB at this intensity", limits.boost_db));
    }
    (out, tilt_out, notes)
}

impl TasteRule {
    /// Narrows the rule to `limits` (see [`clamp_taste_to`]); returns what changed.
    pub fn clamp_to(&mut self, limits: &TasteLimits) -> Vec<String> {
        let (bands, tilt, notes) = clamp_taste_to(&self.bands, self.tilt, limits);
        self.bands = bands;
        self.tilt = tilt;
        notes
    }
}

/// What changed in the taste rules, not what they are now: an assistant turn
/// or a reaction reports this, and the app merges it into the settings as
/// they are at that moment ([`TasteSettings::apply`]). A copy of the whole
/// settings taken earlier would bring back rules the user deleted since.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct TasteChange {
    /// Sounds to add, or to replace by id.
    pub upsert: Vec<TasteRule>,
    /// Ids of the rules to remove.
    pub remove: Vec<String>,
    /// Switch every layer on or off; `None` keeps it.
    pub enabled: Option<bool>,
}

impl TasteChange {
    /// Adds or replaces `rule`.
    pub fn upserting(rule: TasteRule) -> Self {
        let mut c = Self::default();
        c.push_upsert(rule);
        c
    }

    /// Removes the rule `id`.
    pub fn removing(id: impl Into<String>) -> Self {
        let mut c = Self::default();
        c.push_remove(id);
        c
    }

    pub fn is_empty(&self) -> bool {
        self.upsert.is_empty() && self.remove.is_empty() && self.enabled.is_none()
    }

    /// Records an upsert after the changes already here (a later one for the
    /// same rule replaces the earlier; a removal of it is forgotten).
    pub fn push_upsert(&mut self, rule: TasteRule) {
        self.remove.retain(|id| *id != rule.id);
        match self.upsert.iter().position(|r| r.id == rule.id) {
            Some(i) => self.upsert[i] = rule,
            None => self.upsert.push(rule),
        }
    }

    /// Records a removal after the changes already here.
    pub fn push_remove(&mut self, id: impl Into<String>) {
        let id = id.into();
        self.upsert.retain(|r| r.id != id);
        if !self.remove.contains(&id) {
            self.remove.push(id);
        }
    }
}

/// A target taken out of a sound by an assignment (for Undo).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetMove {
    pub target: TasteScope,
    /// Id of the sound it was taken from.
    pub from: String,
}

/// A sound as saved, and the targets that moved out of other sounds to make room.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedSound {
    pub rule: TasteRule,
    pub moves: Vec<TargetMove>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TasteError {
    #[error("That sound doesn't exist any more.")]
    NoSuchSound,
    #[error("A sound holds at most {MAX_TARGETS} songs, albums, artists, playlists and genres.")]
    TooManyTargets,
}

/// Every sound of an app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct TasteSettings {
    /// Kept for old files; nothing reads it for playback (the master switch is `EqSettings::enabled`).
    pub enabled: bool,
    #[serde(deserialize_with = "lenient_rules")]
    pub rules: Vec<TasteRule>,
    /// How strong what the assistant, the reactions and the wizard propose is.
    #[serde(deserialize_with = "lenient_intensity")]
    pub intensity: TasteIntensity,
    /// [`TASTE_VERSION`] when last saved by this release (0 = 1.5.0 and older).
    pub version: u8,
}

impl TasteSettings {
    /// The sound holding exactly `scope` that plays on `output_id` (`None` =
    /// on any output). Output-specific sounds first.
    pub fn sound_covering(&self, scope: &TasteScope, output_id: Option<&str>) -> Option<&TasteRule> {
        self.rules
            .iter()
            .filter(|r| r.covers(scope) && output_id.is_none_or(|o| r.plays_on(o)))
            .min_by_key(|r| r.outputs.is_empty())
    }

    /// Outputs for a NEW sound made for `scope` by the AI / a reaction:
    /// `[active]` when another sound already holds `scope` on some output (so
    /// the new one doesn't steal it there), else `[]`.
    pub fn outputs_for_new(&self, scope: &TasteScope, active_output: Option<&str>) -> Vec<String> {
        match active_output.map(str::trim).filter(|a| !a.is_empty()) {
            Some(active) if self.rules.iter().any(|r| r.covers(scope)) => vec![active.to_owned()],
            _ => Vec::new(),
        }
    }

    /// Takes `scope` out of every sound but `keep` whose outputs overlap its.
    fn take_from_others(&mut self, keep: usize, scope: &TasteScope, moves: &mut Vec<TargetMove>) {
        let Some(mine) = self.rules.get(keep).cloned() else { return };
        for (i, r) in self.rules.iter_mut().enumerate() {
            if i != keep && r.overlaps_outputs(&mine) {
                if let Some(at) = r.targets.iter().position(|t| t.scope.same_as(scope)) {
                    let gone = r.targets.remove(at);
                    moves.push(TargetMove { target: gone.scope, from: r.id.clone() });
                }
            }
        }
    }

    /// Puts `scope` in sound `to` (added now by `by`) and takes it out of every
    /// other sound whose outputs overlap `to`'s. Idempotent (already there →
    /// `Ok(vec![])`).
    pub fn assign(&mut self, to: &str, scope: TasteScope, by: TasteCreator) -> Result<Vec<TargetMove>, TasteError> {
        let at = self.rules.iter().position(|r| r.id == to).ok_or(TasteError::NoSuchSound)?;
        if self.rules[at].covers(&scope) {
            return Ok(Vec::new());
        }
        if self.rules[at].targets.len() >= MAX_TARGETS {
            return Err(TasteError::TooManyTargets);
        }
        let mut moves = Vec::new();
        self.take_from_others(at, &scope, &mut moves);
        self.rules[at].targets.push(TasteTarget::now(scope, by));
        self.version = TASTE_VERSION;
        Ok(moves)
    }

    /// Takes `scope` out of `from`; `true` when it was there.
    pub fn unassign(&mut self, from: &str, scope: &TasteScope) -> bool {
        let Some(r) = self.rules.iter_mut().find(|r| r.id == from) else { return false };
        let before = r.targets.len();
        r.targets.retain(|t| !t.scope.same_as(scope));
        before != r.targets.len()
    }

    /// New outputs for a sound (trimmed, deduped); its targets win every
    /// conflict that appears.
    pub fn set_outputs(&mut self, id: &str, outputs: Vec<String>) -> Result<Vec<TargetMove>, TasteError> {
        let at = self.rules.iter().position(|r| r.id == id).ok_or(TasteError::NoSuchSound)?;
        self.rules[at].outputs = clean_outputs(outputs);
        let mine: Vec<TasteScope> = self.rules[at].targets.iter().map(|t| t.scope.clone()).collect();
        let mut moves = Vec::new();
        for scope in &mine {
            self.take_from_others(at, scope, &mut moves);
        }
        self.version = TASTE_VERSION;
        Ok(moves)
    }

    /// Puts each moved target back in its `from` sound (newest first; skipped
    /// if that sound is gone or full). The original date is not kept (0).
    pub fn undo_moves(&mut self, moves: &[TargetMove]) {
        for m in moves.iter().rev() {
            if let Some(r) = self.rules.iter_mut().find(|r| r.id == m.from) {
                if !r.covers(&m.target) && r.targets.len() < MAX_TARGETS {
                    r.targets.push(TasteTarget { scope: m.target.clone(), added_ms: 0, added_by: r.created_by });
                }
            }
        }
    }

    /// Adds `rule`, or replaces the one with its id. Clamped, targets deduped
    /// and capped at [`MAX_TARGETS`], empty id → new id; its targets win
    /// conflicts with overlapping sounds. At most [`MAX_RULES`] are kept.
    pub fn upsert(&mut self, mut rule: TasteRule) -> SavedSound {
        rule.normalize();
        if rule.id.trim().is_empty() {
            rule.id = new_id();
        }
        if self.rules.is_empty() {
            self.enabled = true;
        }
        match self.rules.iter().position(|r| r.id == rule.id) {
            Some(i) => self.rules[i] = rule.clone(),
            None => {
                self.rules.push(rule.clone());
                if self.rules.len() > MAX_RULES {
                    self.rules.remove(0);
                }
            }
        }
        let mut moves = Vec::new();
        if let Some(at) = self.rules.iter().position(|r| r.id == rule.id) {
            for t in &rule.targets {
                self.take_from_others(at, &t.scope, &mut moves);
            }
        }
        self.version = TASTE_VERSION;
        SavedSound { rule, moves }
    }

    /// Merges `change` into these settings as they are now: removals, then
    /// upserts (by id), then the old switch. Moves are discarded.
    pub fn apply(&mut self, change: &TasteChange) {
        for id in &change.remove {
            self.remove(id);
        }
        for rule in &change.upsert {
            self.upsert(rule.clone());
        }
        if let Some(on) = change.enabled {
            self.enabled = on;
        }
    }

    /// Removes the sound `id`; `true` when there was one.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.rules.len();
        self.rules.retain(|r| r.id != id);
        before != self.rules.len()
    }

    /// "Keep": if a sound with `rule.id` exists, its curve (tilt, bands,
    /// explanation, non-empty label) is replaced by `rule`'s and `scope` is
    /// assigned to it (`only_on` ignored). Else a NEW sound = `rule` with
    /// `targets = [scope]` (added now by `by`) and `outputs = only_on`.
    pub fn keep(
        &mut self,
        rule: &TasteRule,
        scope: TasteScope,
        only_on: Option<String>,
        by: TasteCreator,
    ) -> Result<SavedSound, TasteError> {
        if self.rules.iter().any(|r| r.id == rule.id) {
            let moves = self.assign(&rule.id, scope, by)?;
            let stored = self.rules.iter_mut().find(|r| r.id == rule.id).ok_or(TasteError::NoSuchSound)?;
            stored.tilt = rule.tilt;
            stored.bands = rule.bands.clone();
            stored.explanation = rule.explanation.clone();
            if !rule.label.trim().is_empty() {
                stored.label = rule.label.clone();
            }
            stored.clamp();
            return Ok(SavedSound { rule: stored.clone(), moves });
        }
        let mut fresh = rule.clone();
        fresh.targets = vec![TasteTarget::now(scope, by)];
        fresh.outputs = only_on.into_iter().collect();
        Ok(self.upsert(fresh))
    }

    /// Every curve in range, ids filled, targets deduped, and
    /// `version = TASTE_VERSION` (whatever an app saves through this is
    /// already migrated, even if its UI drops the field).
    pub fn clamped(&self) -> Self {
        let mut s = self.clone();
        for r in &mut s.rules {
            r.normalize();
            if r.id.trim().is_empty() {
                r.id = new_id();
            }
        }
        s.version = TASTE_VERSION;
        s
    }

    /// "Hear without": the same settings with no sounds.
    pub fn without_sounds(&self) -> Self {
        Self { enabled: self.enabled, rules: Vec::new(), intensity: self.intensity, version: self.version }
    }
}

/// A "Try on this song": a sound heard on one track only, never saved. While
/// the master is on it wins over every saved sound; it ends when another
/// track plays.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TasteAudition {
    pub rule: TasteRule,
    /// Always the [`TasteScope::Track`] of the track tried on.
    pub track: TasteScope,
}

impl TasteAudition {
    /// `None` when `ctx` has neither a track id nor a path.
    pub fn new(mut rule: TasteRule, ctx: &NowPlayingContext) -> Option<Self> {
        let track = ctx.scopes().into_iter().find(|s| matches!(s, TasteScope::Track { .. }))?;
        rule.clamp();
        Some(Self { rule, track })
    }

    /// Whether `ctx` is the same track.
    pub fn applies_to(&self, ctx: &NowPlayingContext) -> bool {
        self.track.fit(ctx).is_some()
    }
}

/// What a track hears: the sound, the target that matched, its tilt (in
/// range), and whether it is a "Try" rather than a saved sound.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedTaste<'a> {
    pub rule: &'a TasteRule,
    pub target: &'a TasteScope,
    pub tilt: Tilt,
    pub auditioning: bool,
}

/// Where a search result comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TargetSource {
    Library,
    Tidal,
}

/// A piece of music offered by "Add" (target search).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetHit {
    pub scope: TasteScope,
    /// Shown under the name ("Song · Steely Dan"); the shell writes it in its language.
    #[serde(default)]
    pub subtitle: String,
    pub source: TargetSource,
    /// Id of the sound already covering it on the output asked about (filled by [`rank_targets`]).
    #[serde(default)]
    pub in_sound: Option<String>,
}

/// Order of kinds in search results (the mockup's).
fn kind_order(scope: &TasteScope) -> u8 {
    match scope {
        TasteScope::Artist { .. } => 0,
        TasteScope::Track { .. } => 1,
        TasteScope::Album { .. } => 2,
        TasteScope::Playlist { .. } => 3,
        TasteScope::Genre { .. } => 4,
    }
}

/// How well `name` (lowercase) matches `query` (lowercase, non-empty): exact
/// 0, prefix 1, a later word's prefix 2, contains 3; `None` when it doesn't.
fn match_rank(name: &str, query: &str) -> Option<u8> {
    if name == query {
        return Some(0);
    }
    if name.starts_with(query) {
        return Some(1);
    }
    let mut found = false;
    for (at, _) in name.match_indices(query) {
        found = true;
        if name[..at].chars().next_back().is_some_and(|c| !c.is_alphanumeric()) {
            return Some(2);
        }
    }
    found.then_some(3)
}

/// Target search: keeps hits whose name contains `query` (case-insensitive,
/// trimmed; blank → empty), dedupes with `same_as` (library beats Tidal),
/// orders by match quality, kind (artist, song, album, playlist, genre),
/// library before Tidal, then name, caps at `limit`, and fills `in_sound`.
pub fn rank_targets(
    query: &str,
    hits: Vec<TargetHit>,
    taste: &TasteSettings,
    output_id: Option<&str>,
    limit: usize,
) -> Vec<TargetHit> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    // (match rank, hit, lowercase name)
    let mut kept: Vec<(u8, TargetHit, String)> = Vec::with_capacity(hits.len());
    for hit in hits {
        let name = hit.scope.name().trim().to_lowercase();
        let Some(rank) = match_rank(&name, &query) else { continue };
        match kept.iter().position(|(_, k, _)| k.scope.same_as(&hit.scope)) {
            Some(i) if kept[i].1.source == TargetSource::Tidal && hit.source == TargetSource::Library => {
                kept[i] = (rank, hit, name)
            }
            Some(_) => {}
            None => kept.push((rank, hit, name)),
        }
    }
    kept.sort_by_cached_key(|(rank, hit, name)| {
        (*rank, kind_order(&hit.scope), matches!(hit.source, TargetSource::Tidal), name.clone())
    });
    kept.into_iter()
        .take(limit)
        .map(|(_, mut hit, _)| {
            hit.in_sound = taste.sound_covering(&hit.scope, output_id).map(|r| r.id.clone());
            hit
        })
        .collect()
}

/// Where the queue came from (`album`, `playlist`, `artist`, `mix`, `library`,
/// `folder`, `search`...). An album in order plays with album loudness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SourceContext {
    pub kind: String,
    pub id: String,
    pub name: String,
}

impl SourceContext {
    pub fn new(kind: impl Into<String>, id: impl Into<String>, name: impl Into<String>) -> Self {
        Self { kind: kind.into(), id: id.into(), name: name.into() }
    }

    pub fn is(&self, kind: &str) -> bool {
        self.kind.eq_ignore_ascii_case(kind)
    }
}

/// What is playing, as far as taste rules care.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct NowPlayingContext {
    /// Tidal id, when it is a Tidal track.
    pub track_id: Option<String>,
    /// File path of the track.
    pub path: Option<String>,
    pub title: Option<String>,
    pub album: Option<ItemRef>,
    /// Main artist first.
    pub artists: Vec<ItemRef>,
    /// Main genre first (see [`split_genres`]).
    pub genres: Vec<String>,
    /// Where the queue came from.
    pub source: Option<SourceContext>,
}

impl NowPlayingContext {
    /// The playlist the queue came from, if it did.
    pub fn playlist(&self) -> Option<ItemRef> {
        self.source.as_ref().filter(|s| s.is("playlist")).map(|s| ItemRef::new(s.id.clone(), s.name.clone()))
    }

    pub fn main_genre(&self) -> Option<&str> {
        self.genres.iter().map(|g| g.trim()).find(|g| !g.is_empty())
    }

    pub fn main_artist(&self) -> Option<&ItemRef> {
        self.artists.iter().find(|a| !a.is_empty())
    }

    /// The scopes this track offers, most specific first (for "Sound for this…" pickers).
    pub fn scopes(&self) -> Vec<TasteScope> {
        let mut out = Vec::new();
        let id = self.track_id.clone().filter(|i| !i.trim().is_empty()).or_else(|| self.path.clone());
        if let Some(id) = id.filter(|i| !i.trim().is_empty()) {
            out.push(TasteScope::Track { id, name: self.title.clone().unwrap_or_default() });
        }
        if let Some(a) = self.album.as_ref().filter(|a| !a.is_empty()) {
            out.push(TasteScope::Album { id: a.id.clone(), name: a.name.clone() });
        }
        if let Some(a) = self.main_artist() {
            out.push(TasteScope::Artist { id: a.id.clone(), name: a.name.clone() });
        }
        if let Some(p) = self.playlist().filter(|p| !p.is_empty()) {
            out.push(TasteScope::Playlist { id: p.id, name: p.name });
        }
        if let Some(g) = self.main_genre() {
            out.push(TasteScope::Genre { name: g.to_owned() });
        }
        out
    }
}

/// The sound for `ctx` on `output_id`. Candidates: enabled, plays on the
/// output, has a target that fits `ctx`. Best = most specific target, then a
/// sound limited to outputs over an all-outputs one, then the track's main
/// genre / artist, then the later sound. `settings.enabled` is NOT read (the
/// master switch is the caller's). Neutral curves are returned.
pub fn resolve_taste<'a>(
    settings: &'a TasteSettings,
    output_id: &str,
    ctx: &NowPlayingContext,
) -> Option<ResolvedTaste<'a>> {
    settings
        .rules
        .iter()
        .enumerate()
        .filter(|(_, r)| r.enabled && r.plays_on(output_id))
        .filter_map(|(i, r)| {
            let (fit, target) = r
                .targets
                .iter()
                .filter_map(|t| t.scope.fit(ctx).map(|fit| (fit, &t.scope)))
                .max_by_key(|(fit, _)| *fit)?;
            Some(((fit.0, u8::from(!r.outputs.is_empty()), fit.1, i), r, target))
        })
        .max_by_key(|(key, _, _)| *key)
        .map(|(_, r, target)| ResolvedTaste { rule: r, target, tilt: r.tilt.clamped(MAX_TILT_DB), auditioning: false })
}

/// [`resolve_taste`], but an audition that applies to `ctx` wins.
pub fn resolve_taste_with<'a>(
    settings: &'a TasteSettings,
    output_id: &str,
    audition: Option<&'a TasteAudition>,
    ctx: &NowPlayingContext,
) -> Option<ResolvedTaste<'a>> {
    match audition.filter(|a| a.applies_to(ctx)) {
        Some(a) => Some(ResolvedTaste {
            rule: &a.rule,
            target: &a.track,
            tilt: a.rule.tilt.clamped(MAX_TILT_DB),
            auditioning: true,
        }),
        None => resolve_taste(settings, output_id, ctx),
    }
}

/// A GENRE tag as a list: `Electronic, House` / `Rock; Indie` → two genres.
pub fn split_genres(tag: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for g in tag.split([',', ';', '\u{0}']).map(str::trim).filter(|g| !g.is_empty()) {
        if !out.iter().any(|o| o.eq_ignore_ascii_case(g)) {
            out.push(g.to_owned());
        }
    }
    out
}

/// `Electronic` matches `electronic`, and `Hip-Hop/Rap` matches `Hip-Hop` (either way round).
pub fn genre_matches(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.trim().to_lowercase();
    let (a, b) = (norm(a), norm(b));
    if a.is_empty() || b.is_empty() {
        return false;
    }
    if a == b {
        return true;
    }
    let parts = |s: &str| s.split('/').map(|p| p.trim().to_owned()).filter(|p| !p.is_empty()).collect::<Vec<_>>();
    let (pa, pb) = (parts(&a), parts(&b));
    pa.contains(&b) || pb.contains(&a)
}

/// An unknown intensity (from a newer version) reads as the default.
fn lenient_intensity<'de, D: Deserializer<'de>>(d: D) -> Result<TasteIntensity, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Any {
        Known(TasteIntensity),
        Other(serde::de::IgnoredAny),
    }
    Ok(match Any::deserialize(d)? {
        Any::Known(i) => i,
        Any::Other(_) => TasteIntensity::default(),
    })
}

fn lenient_creator<'de, D: Deserializer<'de>>(d: D) -> Result<TasteCreator, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Any {
        Known(TasteCreator),
        Other(serde::de::IgnoredAny),
    }
    Ok(match Any::deserialize(d)? {
        Any::Known(c) => c,
        Any::Other(_) => TasteCreator::Manual,
    })
}

/// Unreadable bands are dropped (an unknown kind reads as a peak, see [`EqBand`]).
fn lenient_bands<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<TasteBand>, D::Error> {
    let values = Option::<Vec<serde_json::Value>>::deserialize(d)?.unwrap_or_default();
    Ok(values.into_iter().filter_map(|v| serde_json::from_value(v).ok()).collect())
}

/// Unreadable rules (an unknown scope from a newer version...) are dropped.
fn lenient_rules<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<TasteRule>, D::Error> {
    let values = Option::<Vec<serde_json::Value>>::deserialize(d)?.unwrap_or_default();
    Ok(values.into_iter().filter_map(|v| serde_json::from_value(v).ok()).collect())
}

/// [`TasteSettings`] falling back to the defaults when unreadable
/// (`#[serde(deserialize_with = "homefi_core::dsp::lenient_taste")]`).
pub fn lenient_taste<'de, D: Deserializer<'de>>(d: D) -> Result<TasteSettings, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Any {
        Known(TasteSettings),
        Other(serde::de::IgnoredAny),
    }
    Ok(match Any::deserialize(d)? {
        Any::Known(s) => s,
        Any::Other(_) => TasteSettings::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bass(db: f32) -> Tilt {
        Tilt { bass_db: db, ..Default::default() }
    }

    fn ctx() -> NowPlayingContext {
        NowPlayingContext {
            track_id: Some("111".into()),
            path: Some("/m/a.flac".into()),
            title: Some("Song".into()),
            album: Some(ItemRef::new("22", "Discovery")),
            artists: vec![ItemRef::named("Daft Punk"), ItemRef::named("Romanthony")],
            genres: vec!["Electronic".into(), "House".into()],
            source: Some(SourceContext::new("playlist", "pl-1", "Friday")),
        }
    }

    fn rule(scope: TasteScope, db: f32) -> TasteRule {
        TasteRule::for_target(scope, bass(db), "", TasteCreator::Manual)
    }

    fn genre(name: &str) -> TasteScope {
        TasteScope::Genre { name: name.into() }
    }

    fn playlist() -> TasteScope {
        TasteScope::Playlist { id: "pl-1".into(), name: "Friday".into() }
    }

    /// Saves a sound with a known id, targets and outputs.
    fn sound(s: &mut TasteSettings, id: &str, scopes: Vec<TasteScope>, outputs: &[&str], db: f32) -> TasteRule {
        let mut r = TasteRule::new(scopes, bass(db), "", TasteCreator::Manual);
        r.id = id.into();
        r.outputs = outputs.iter().map(|o| (*o).to_owned()).collect();
        s.upsert(r).rule
    }

    fn other_track() -> NowPlayingContext {
        NowPlayingContext { track_id: Some("222".into()), path: Some("/m/b.flac".into()), ..ctx() }
    }

    #[test]
    fn the_most_specific_target_wins() {
        let mut s = TasteSettings::default();
        s.upsert(rule(genre("electronic"), 1.0));
        s.upsert(rule(TasteScope::Artist { id: String::new(), name: "daft punk".into() }, 2.0));
        s.upsert(rule(playlist(), 3.0));
        s.upsert(rule(TasteScope::Album { id: "22".into(), name: "Other name".into() }, 4.0));
        s.upsert(rule(TasteScope::Track { id: "/m/a.flac".into(), name: "Song".into() }, 5.0));
        assert!(s.enabled, "the first rule turns the old switch on");
        let c = ctx();
        // track > album > artist > playlist > genre
        let mut expect = [5.0, 4.0, 2.0, 3.0, 1.0].into_iter();
        while let Some(heard) = resolve_taste(&s, "", &c) {
            assert_eq!(heard.tilt.bass_db, expect.next().unwrap(), "{:?}", heard.target);
            assert!(!heard.auditioning);
            let id = heard.rule.id.clone();
            s.remove(&id);
        }
        assert_eq!(expect.next(), None);
    }

    #[test]
    fn ties_go_to_the_main_genre_and_the_old_switch_is_ignored() {
        let mut s = TasteSettings::default();
        s.upsert(rule(genre("House"), 2.0));
        s.upsert(rule(genre("Electronic"), 1.0));
        assert_eq!(resolve_taste(&s, "", &ctx()).unwrap().tilt.bass_db, 1.0);
        s.rules[1].enabled = false;
        assert_eq!(resolve_taste(&s, "", &ctx()).unwrap().tilt.bass_db, 2.0, "disabled sounds never play");
        s.enabled = false;
        assert!(resolve_taste(&s, "", &ctx()).is_some(), "taste.enabled is not read any more");
        // ids win over names: another album with the same title doesn't match
        let mut s = TasteSettings::default();
        s.upsert(rule(TasteScope::Album { id: "99".into(), name: "Discovery".into() }, 3.0));
        assert!(resolve_taste(&s, "", &ctx()).is_none());
        // nothing matches a track of another genre
        let jazz = NowPlayingContext { genres: vec!["Jazz".into()], ..Default::default() };
        assert!(resolve_taste(&TasteSettings::default(), "", &jazz).is_none());
    }

    #[test]
    fn outputs_decide_who_plays() {
        let mut s = TasteSettings::default();
        // two sounds for the same music can only exist in files written by other versions: build them directly
        let mut all = rule(genre("Electronic"), 1.0);
        all.id = "all".into();
        let mut wh = rule(genre("Electronic"), 2.0);
        wh.id = "wh".into();
        wh.outputs = vec!["WH".into()];
        s.rules = vec![all, wh];
        let id_on = |s: &TasteSettings, out: &str| resolve_taste(s, out, &ctx()).map(|h| h.rule.id.clone());
        assert_eq!(id_on(&s, "WH").as_deref(), Some("wh"), "the output's own sound beats the all-outputs one");
        assert_eq!(id_on(&s, "speakers").as_deref(), Some("all"));
        // a sound limited to speakers is silent on the headphones
        let mut only = TasteSettings::default();
        sound(&mut only, "sp", vec![genre("Electronic")], &["speakers"], 1.0);
        assert!(resolve_taste(&only, "WH", &ctx()).is_none());
        assert!(resolve_taste(&only, "speakers", &ctx()).is_some());
    }

    #[test]
    fn upsert_replaces_by_id_clamps_and_moves_the_targets() {
        let mut s = TasteSettings::default();
        let a = s.upsert(rule(genre("Rock"), 2.0)).rule;
        let mut again = rule(genre("rock"), 40.0);
        again.id = a.id.clone();
        let b = s.upsert(again).rule;
        assert_eq!((s.rules.len(), a.id.as_str()), (1, b.id.as_str()));
        assert_eq!(s.rules[0].tilt.bass_db, MAX_TILT_DB);
        assert_eq!(s.version, TASTE_VERSION);
        // another sound for the same music takes it over (same outputs): never matched by scope
        let saved = s.upsert(rule(genre("rock"), 1.0));
        assert_eq!(s.rules.len(), 2);
        assert_eq!(saved.moves, vec![TargetMove { target: genre("rock"), from: a.id.clone() }]);
        assert!(s.rules[0].targets.is_empty(), "an empty sound is kept");
        assert!(s.remove(&a.id) && !s.remove(&a.id));
        // an empty id gets one
        let mut blank = rule(genre("Pop"), 1.0);
        blank.id.clear();
        assert!(!s.upsert(blank).rule.id.is_empty());
    }

    #[test]
    fn json_is_stable_and_lenient() {
        let mut s = TasteSettings::default();
        let mut r = rule(TasteScope::Artist { id: "7".into(), name: "Bach".into() }, 2.0);
        r.id = "r1".into();
        r.label = "Warmer Bach".into();
        r.created_by = TasteCreator::Reaction;
        r.targets[0].added_ms = 5;
        s.upsert(r);
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "enabled": true,
                "intensity": "balanced",
                "version": 1,
                "rules": [{
                    "id": "r1",
                    "tilt": {"bassDb": 2.0, "trebleDb": 0.0, "clarityDb": null},
                    "label": "Warmer Bach",
                    "createdBy": "reaction",
                    "enabled": true,
                    "targets": [{"scope": {"kind": "artist", "id": "7", "name": "Bach"}, "addedMs": 5, "addedBy": "manual"}]
                }]
            })
        );
        assert_eq!(serde_json::from_value::<TasteSettings>(json).unwrap(), s);
        // an unknown creator reads as manual; a rule with no scope is kept, empty
        let s: TasteSettings = serde_json::from_str(
            r#"{"enabled":true,"rules":[{"id":"a","scope":{"kind":"mood","name":"x"}},{"id":"b","scope":{"kind":"genre","name":"Pop"},"createdBy":"robot"}]}"#,
        )
        .unwrap();
        assert_eq!(s.rules.len(), 2);
        assert!(s.rules[0].targets.is_empty());
        assert_eq!(s.rules[1].created_by, TasteCreator::Manual);
        assert!(s.rules[1].enabled);
        #[derive(Deserialize)]
        struct W {
            #[serde(default, deserialize_with = "lenient_taste")]
            taste: TasteSettings,
        }
        let w: W = serde_json::from_str(r#"{"taste":{"enabled":"yes"}}"#).unwrap();
        assert_eq!(w.taste, TasteSettings::default());
        // an intensity from a newer version reads as the default; a known one survives
        let s: TasteSettings = serde_json::from_str(r#"{"intensity":"extreme"}"#).unwrap();
        assert_eq!(s.intensity, TasteIntensity::Balanced);
        let s: TasteSettings = serde_json::from_str(r#"{"enabled":true,"intensity":"bold"}"#).unwrap();
        assert_eq!(s.intensity, TasteIntensity::Bold);
        // old files have no intensity and no version
        let s: TasteSettings = serde_json::from_str(r#"{"enabled":true,"rules":[]}"#).unwrap();
        assert_eq!((s.intensity, s.version), (TasteIntensity::Balanced, 0));
    }

    #[test]
    fn old_rules_with_a_scope_become_one_target_and_new_ones_round_trip() {
        let old: TasteRule = serde_json::from_str(
            r#"{"id":"r1","scope":{"kind":"genre","name":"Pop"},"tilt":{"bassDb":2.0,"trebleDb":0.0,"clarityDb":null},"label":"x","createdBy":"ai","enabled":true}"#,
        )
        .unwrap();
        assert_eq!(old.targets, vec![TasteTarget { scope: genre("Pop"), added_ms: 0, added_by: TasteCreator::Ai }]);
        assert!(old.outputs.is_empty() && old.enabled);
        let json = serde_json::to_value(&old).unwrap();
        assert!(json.get("scope").is_none(), "{json}");
        assert!(json.get("outputs").is_none());
        assert_eq!(serde_json::from_value::<TasteRule>(json).unwrap(), old);

        let new = r#"{"id":"t1","label":"Warm and airy","tilt":{"bassDb":2,"trebleDb":1.5,"clarityDb":null},
            "bands":[{"kind":"peak","freqHz":3000,"gainDb":-2,"q":2,"enabled":true,"note":"trims the glare"}],
            "createdBy":"ai","enabled":true,
            "targets":[{"scope":{"kind":"artist","id":"","name":"Eagles"},"addedMs":1761700000000,"addedBy":"ai"}],
            "outputs":["WH-1000XM5"]}"#;
        let r: TasteRule = serde_json::from_str(new).unwrap();
        assert_eq!(
            (r.targets.len(), r.targets[0].added_ms, r.outputs.clone()),
            (1, 1_761_700_000_000, vec!["WH-1000XM5".to_owned()])
        );
        assert_eq!(r.bands[0].note, "trims the glare");
        let back = serde_json::to_value(&r).unwrap();
        assert_eq!(back["targets"][0]["addedMs"], 1_761_700_000_000u64);
        assert_eq!(
            (back["targets"][0]["addedBy"].as_str(), back["outputs"][0].as_str()),
            (Some("ai"), Some("WH-1000XM5"))
        );
        assert!(back.get("scope").is_none());
        assert_eq!(serde_json::from_value::<TasteRule>(back).unwrap(), r);

        // neither scope nor targets: the sound is kept, empty
        let r: TasteRule = serde_json::from_str(r#"{"id":"e","label":"Empty"}"#).unwrap();
        assert!(r.targets.is_empty() && r.enabled);
        // a target of a kind from a newer version is dropped, the rest stay; outputs are cleaned
        let r: TasteRule = serde_json::from_str(
            r#"{"id":"m","targets":[{"scope":{"kind":"mood","name":"x"}},{"scope":{"kind":"genre","name":"Pop"}}],
                "outputs":[" WH ","", "WH"]}"#,
        )
        .unwrap();
        assert_eq!(r.targets.len(), 1);
        assert_eq!(r.outputs, ["WH"]);
    }

    #[test]
    fn assign_moves_targets_only_between_overlapping_outputs() {
        let mut s = TasteSettings::default();
        let p = playlist();
        sound(&mut s, "A", vec![p.clone()], &["WH"], 1.0);
        sound(&mut s, "B", vec![p.clone()], &["speakers"], 2.0);
        assert!(s.rules[0].covers(&p) && s.rules[1].covers(&p), "different outputs coexist");
        sound(&mut s, "C", vec![], &[], 3.0);
        let moves = s.assign("C", p.clone(), TasteCreator::Manual).unwrap();
        assert_eq!(moves.len(), 2);
        assert!(s.rules[2].covers(&p) && !s.rules[0].covers(&p) && !s.rules[1].covers(&p));
        assert_eq!(s.assign("C", p.clone(), TasteCreator::Manual), Ok(vec![]), "idempotent");
        s.undo_moves(&moves);
        assert!(s.rules[0].covers(&p) && s.rules[1].covers(&p));
        assert_eq!(s.assign("nope", p.clone(), TasteCreator::Manual), Err(TasteError::NoSuchSound));
        // a sound is full at MAX_TARGETS
        let many = (0..MAX_TARGETS).map(|i| TasteScope::Track { id: format!("t{i}"), name: String::new() }).collect();
        sound(&mut s, "D", many, &[], 1.0);
        assert_eq!(s.rules[3].targets.len(), MAX_TARGETS);
        assert_eq!(s.assign("D", genre("Pop"), TasteCreator::Manual), Err(TasteError::TooManyTargets));
        assert!(s.unassign("D", &TasteScope::Track { id: "t0".into(), name: String::new() }));
        assert!(!s.unassign("D", &TasteScope::Track { id: "t0".into(), name: String::new() }));
        assert!(s.assign("D", genre("Pop"), TasteCreator::Manual).is_ok());
    }

    #[test]
    fn new_outputs_win_the_conflicts_they_create() {
        let mut s = TasteSettings::default();
        let p = playlist();
        sound(&mut s, "A", vec![p.clone()], &["WH"], 1.0);
        sound(&mut s, "B", vec![p.clone()], &["speakers"], 2.0);
        let moves = s.set_outputs("B", vec![" WH ".into(), "speakers".into(), "WH".into()]).unwrap();
        assert_eq!(moves, vec![TargetMove { target: p.clone(), from: "A".into() }]);
        assert_eq!(s.rules[1].outputs, ["WH", "speakers"]);
        assert!(!s.rules[0].covers(&p) && s.rules[1].covers(&p));
        assert_eq!(s.set_outputs("nope", vec![]), Err(TasteError::NoSuchSound));
    }

    #[test]
    fn sound_covering_prefers_the_output_specific_one() {
        let mut s = TasteSettings::default();
        let g = genre("Rock");
        let mut all = rule(g.clone(), 1.0);
        all.id = "all".into();
        let mut wh = rule(g.clone(), 2.0);
        wh.id = "wh".into();
        wh.outputs = vec!["WH".into()];
        s.rules = vec![all, wh];
        assert_eq!(s.sound_covering(&g, Some("WH")).unwrap().id, "wh");
        assert_eq!(s.sound_covering(&g, Some("speakers")).unwrap().id, "all");
        assert_eq!(s.sound_covering(&g, None).unwrap().id, "wh");
        assert!(s.sound_covering(&genre("Pop"), None).is_none());
    }

    #[test]
    fn outputs_for_new_does_not_steal_a_held_target() {
        let mut s = TasteSettings::default();
        let g = genre("Rock");
        assert!(s.outputs_for_new(&g, Some("WH")).is_empty(), "nothing holds it");
        sound(&mut s, "A", vec![g.clone()], &[], 1.0);
        assert_eq!(s.outputs_for_new(&g, Some("WH")), ["WH"]);
        assert!(s.outputs_for_new(&g, None).is_empty());
        assert!(s.outputs_for_new(&genre("Pop"), Some("WH")).is_empty());
    }

    #[test]
    fn keep_adds_to_a_saved_sound_or_makes_a_new_one() {
        let mut s = TasteSettings::default();
        let artist = TasteScope::Artist { id: String::new(), name: "Eagles".into() };
        let saved = sound(&mut s, "s1", vec![artist.clone()], &[], 1.0);
        // a saved sound + a new scope: the same sound gains the target and the proposal's curve
        let mut tweaked = saved.clone();
        tweaked.tilt = bass(3.0);
        tweaked.label = "Warmer".into();
        let r = s.keep(&tweaked, genre("Rock"), Some("WH".into()), TasteCreator::Reaction).unwrap();
        assert_eq!((r.rule.id.as_str(), r.rule.tilt.bass_db, r.rule.label.as_str()), ("s1", 3.0, "Warmer"));
        assert!(r.rule.covers(&artist) && r.rule.covers(&genre("Rock")) && r.rule.outputs.is_empty());
        assert_eq!(s.rules.len(), 1);

        // an unsaved proposal limited to one output: a new sound with only that target
        let proposal = rule(genre("Jazz"), 2.0);
        let r = s.keep(&proposal, genre("Jazz"), Some("WH".into()), TasteCreator::Ai).unwrap();
        assert_eq!((r.rule.id.as_str(), r.rule.outputs.clone()), (proposal.id.as_str(), vec!["WH".to_owned()]));
        assert_eq!(r.rule.targets.len(), 1);
        assert_eq!(r.rule.targets[0].added_by, TasteCreator::Ai);
        assert!(r.moves.is_empty() && s.rules.len() == 2);

        // a conflict comes back as a move (for Undo)
        let proposal = rule(genre("Rock"), 1.0);
        let r = s.keep(&proposal, genre("Rock"), None, TasteCreator::Manual).unwrap();
        assert_eq!(r.moves, vec![TargetMove { target: genre("Rock"), from: "s1".into() }]);
        s.undo_moves(&r.moves);
        assert!(s.sound_covering(&genre("Rock"), None).is_some());
        // a missing id is not "saved": a full sound is refused
        let full: Vec<TasteScope> =
            (0..MAX_TARGETS).map(|i| TasteScope::Track { id: format!("t{i}"), name: String::new() }).collect();
        let big = sound(&mut s, "big", full, &[], 1.0);
        assert_eq!(s.keep(&big, genre("Pop"), None, TasteCreator::Manual), Err(TasteError::TooManyTargets));
    }

    #[test]
    fn a_try_plays_on_one_track_only() {
        let mut s = TasteSettings::default();
        sound(&mut s, "s1", vec![genre("Electronic")], &[], 1.0);
        let attempt = rule(genre("Jazz"), 4.0);
        let a = TasteAudition::new(attempt, &ctx()).unwrap();
        assert!(matches!(a.track, TasteScope::Track { .. }));
        let heard = resolve_taste_with(&s, "", Some(&a), &ctx()).unwrap();
        assert!(heard.auditioning);
        assert_eq!((heard.tilt.bass_db, heard.target), (4.0, &a.track));
        // another track hears the saved sound (the try ends)
        assert!(!a.applies_to(&other_track()));
        let heard = resolve_taste_with(&s, "", Some(&a), &other_track()).unwrap();
        assert!(!heard.auditioning && heard.rule.id == "s1");
        // nothing to try on without a track
        let nothing = NowPlayingContext { genres: vec!["Jazz".into()], ..Default::default() };
        assert!(TasteAudition::new(rule(genre("Jazz"), 1.0), &nothing).is_none());
    }

    #[test]
    fn target_search_ranks_dedupes_and_marks_taken_music() {
        let hit = |scope: TasteScope, source| TargetHit { scope, subtitle: String::new(), source, in_sound: None };
        let artist = |name: &str| TasteScope::Artist { id: String::new(), name: name.into() };
        let tidal_artist = |id: &str, name: &str| TasteScope::Artist { id: id.into(), name: name.into() };
        let track = |name: &str| TasteScope::Track { id: format!("id-{name}"), name: name.into() };
        let hits = vec![
            hit(TasteScope::Album { id: "1".into(), name: "Redan Days".into() }, TargetSource::Tidal),
            hit(tidal_artist("9", "Steely Dan"), TargetSource::Tidal),
            hit(artist("Steely Dan"), TargetSource::Library),
            hit(artist("Dan Fogelberg"), TargetSource::Tidal),
            hit(track("Dan"), TargetSource::Tidal),
            hit(artist("Zebra"), TargetSource::Tidal),
        ];
        let mut taste = TasteSettings::default();
        sound(&mut taste, "s1", vec![artist("Steely Dan")], &["WH"], 1.0);
        let ranked = rank_targets("  DAN ", hits.clone(), &taste, Some("WH"), 10);
        let names: Vec<&str> = ranked.iter().map(|h| h.scope.name()).collect();
        // exact, prefix, word prefix, contains; Zebra is out and the duplicate artist is the library's
        assert_eq!(names, ["Dan", "Dan Fogelberg", "Steely Dan", "Redan Days"]);
        assert_eq!(ranked[2].source, TargetSource::Library);
        assert_eq!(ranked[2].in_sound.as_deref(), Some("s1"));
        assert!(ranked.iter().filter(|h| h.in_sound.is_some()).count() == 1);
        // on another output nothing covers it
        let ranked = rank_targets("dan", hits.clone(), &taste, Some("speakers"), 10);
        assert!(ranked.iter().all(|h| h.in_sound.is_none()));
        assert_eq!(rank_targets("dan", hits.clone(), &taste, None, 2).len(), 2);
        assert!(rank_targets("   ", hits, &taste, None, 10).is_empty());

        // same match: artist, song, album, playlist, genre, then library before Tidal, then name
        let kinds = vec![
            hit(genre("Abstract"), TargetSource::Library),
            hit(TasteScope::Playlist { id: "p".into(), name: "Abd".into() }, TargetSource::Tidal),
            hit(TasteScope::Album { id: "a".into(), name: "Abc".into() }, TargetSource::Tidal),
            hit(track("Abbey"), TargetSource::Tidal),
            hit(tidal_artist("2", "Abba"), TargetSource::Tidal),
            hit(artist("Abacus"), TargetSource::Library),
        ];
        let ranked = rank_targets("ab", kinds, &TasteSettings::default(), None, 10);
        let order: Vec<&str> = ranked.iter().map(|h| h.scope.name()).collect();
        assert_eq!(order, ["Abacus", "Abba", "Abbey", "Abc", "Abd", "Abstract"]);
    }

    #[test]
    fn apply_merges_by_id_and_keeps_the_rest() {
        let mut s = TasteSettings::default();
        let jazz = s.upsert(rule(genre("Jazz"), 1.0)).rule;
        let rock = s.upsert(rule(genre("Rock"), 2.0)).rule;
        // the Rock sound edited (same id) replaces it; a new one is added; Jazz goes
        let mut edited = rock.clone();
        edited.tilt = bass(4.0);
        let mut change = TasteChange::upserting(edited);
        change.push_upsert(rule(genre("Pop"), 3.0));
        change.push_remove(jazz.id.clone());
        s.apply(&change);
        assert_eq!(s.rules.len(), 2);
        let stored = s.sound_covering(&genre("Rock"), None).unwrap();
        assert_eq!((stored.id.as_str(), stored.tilt.bass_db), (rock.id.as_str(), 4.0));
        assert!(s.sound_covering(&genre("Jazz"), None).is_none());
        // an upsert is matched by id only: a copy with a new id is another sound
        let mut change = TasteChange::upserting(rule(genre("Rock"), 1.0));
        change.push_upsert(rule(genre("Rock"), 2.0));
        assert_eq!(change.upsert.len(), 2);
        // an empty change changes nothing; the first rule turns the old switch on; the switch is explicit
        let before = s.clone();
        s.apply(&TasteChange::default());
        assert_eq!(s, before);
        assert!(TasteChange::default().is_empty() && !change.is_empty());
        let mut fresh = TasteSettings::default();
        fresh.apply(&TasteChange::upserting(rule(genre("Rock"), 1.0)));
        assert!(fresh.enabled);
        fresh.apply(&TasteChange { enabled: Some(false), ..Default::default() });
        assert!(!fresh.enabled && fresh.rules.len() == 1);
        // recording keeps a change coherent: a later removal cancels an upsert, a later upsert cancels a removal
        let mut c = TasteChange::upserting(rule(genre("Rock"), 1.0));
        let id = c.upsert[0].id.clone();
        c.push_remove(id.clone());
        assert!(c.upsert.is_empty() && c.remove == [id.clone()]);
        let mut back = rule(genre("Rock"), 2.0);
        back.id = id;
        c.push_upsert(back);
        assert!(c.remove.is_empty() && c.upsert.len() == 1);
        // JSON is camelCase and every field optional
        let json = serde_json::to_value(TasteChange::removing("x")).unwrap();
        assert_eq!(json, serde_json::json!({"upsert": [], "remove": ["x"], "enabled": null}));
        assert!(serde_json::from_str::<TasteChange>("{}").unwrap().is_empty());
    }

    #[test]
    fn clamped_stamps_the_version_and_cleans_sounds() {
        let mut s = TasteSettings::default();
        let mut r = rule(genre("Rock"), 40.0);
        r.id.clear();
        r.outputs = vec![" a ".into(), "a".into()];
        s.rules.push(r);
        let c = s.clamped();
        assert_eq!(c.version, TASTE_VERSION);
        assert!(!c.rules[0].id.is_empty());
        assert_eq!((c.rules[0].tilt.bass_db, c.rules[0].outputs.clone()), (MAX_TILT_DB, vec!["a".to_owned()]));
        assert!(c.without_sounds().rules.is_empty());
    }

    #[test]
    fn intensity_limits_stay_inside_the_hard_caps_and_narrow_proposals() {
        for i in TasteIntensity::ALL {
            let l = i.limits();
            assert!(l.band_db <= MAX_TASTE_BAND_DB && l.tilt_db <= MAX_TILT_DB && l.boost_db <= MAX_TASTE_BOOST_DB);
            assert!(l.main_move_db.0 < l.main_move_db.1 && l.main_move_db.1 <= l.band_db + 1.0);
        }
        let ladder = TasteIntensity::ALL.map(|i| i.limits().main_move_db.0);
        assert!(ladder.windows(2).all(|w| w[0] < w[1]), "each step is bigger: {ladder:?}");
        assert_eq!(TasteIntensity::default(), TasteIntensity::Balanced);
        assert_eq!(TasteIntensity::Balanced.scale(), 1.0);
        // a wild proposal is narrowed, and says so; Bold leaves it at the hard caps
        let wild = vec![
            TasteBand::new(EqBandKind::LowShelf, 90.0, 9.0, 0.7),
            TasteBand::new(EqBandKind::Peak, 3000.0, -9.0, 1.0),
        ];
        let tilt = Tilt { bass_db: 6.0, ..Default::default() };
        let subtle = TasteIntensity::Subtle.limits();
        let (bands, tilt_out, notes) = clamp_taste_to(&wild, tilt, &subtle);
        assert_eq!(bands[1].band.gain_db, -subtle.band_db);
        assert!(tilt_out.bass_db > 0.0 && tilt_out.bass_db <= subtle.tilt_db, "{tilt_out:?}");
        assert!(taste_peak_db(&bands, tilt_out, subtle.boost_db) <= subtle.boost_db + 0.1, "{bands:?}");
        assert!(notes.len() >= 2, "{notes:?}");
        let bold = TasteIntensity::Bold.limits();
        let (hard, hard_tilt, _) = clamp_taste_curve(&wild, tilt, false);
        let (same, same_tilt, notes) = clamp_taste_to(&hard, hard_tilt, &bold);
        assert_eq!((same, same_tilt), (hard, hard_tilt));
        assert!(notes.is_empty(), "{notes:?}");
        // a rule narrows in place
        let mut r = rule(TasteScope::Genre { name: "Rock".into() }, 6.0).with_bands(wild);
        assert!(!r.clamp_to(&subtle).is_empty());
        assert!(r.tilt.bass_db <= subtle.tilt_db && r.bands[0].band.gain_db <= subtle.band_db);
    }

    #[test]
    fn curves_load_old_rules_and_round_trip() {
        // a rule from before curves: no bands, no explanation
        let old = r#"{"id":"r1","scope":{"kind":"genre","name":"Pop"},"tilt":{"bassDb":2.0,"trebleDb":0.0,"clarityDb":null},"label":"x","createdBy":"ai","enabled":true}"#;
        let r: TasteRule = serde_json::from_str(old).unwrap();
        assert!(r.bands.is_empty() && r.explanation.is_empty());
        let json = serde_json::to_value(&r).unwrap();
        assert!(json.get("bands").is_none() && json.get("explanation").is_none());
        assert_eq!(serde_json::from_value::<TasteRule>(json).unwrap(), r);
        // a curve: the band's own fields plus its note; bad bands are dropped
        let r: TasteRule = serde_json::from_str(
            r#"{"id":"r2","scope":{"kind":"artist","id":"","name":"David Guetta"},"explanation":"Punch",
                "bands":[{"kind":"peak","freqHz":3200,"gainDb":-2,"q":3,"note":"softens the cymbals"},
                         {"kind":"lowShelf","freqHz":"nope"},{"kind":"hyperShelf","freqHz":60,"gainDb":1}]}"#,
        )
        .unwrap();
        assert_eq!(r.bands.len(), 2);
        assert_eq!(r.bands[0].note, "softens the cymbals");
        assert_eq!((r.bands[0].band.freq_hz, r.bands[0].band.gain_db, r.bands[0].band.q), (3200.0, -2.0, 3.0));
        assert!(r.bands[0].band.enabled);
        assert_eq!(r.bands[1].band.kind, EqBandKind::Peak, "unknown kinds read as a peak");
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["bands"][0]["note"], "softens the cymbals");
        assert_eq!(json["bands"][0]["freqHz"], 3200.0);
        assert!(json["bands"][1].get("note").is_none());
        assert_eq!(serde_json::from_value::<TasteRule>(json).unwrap(), r);
    }

    #[test]
    fn taste_curves_stay_within_the_caps() {
        let wild = vec![
            TasteBand::new(EqBandKind::Peak, 1000.0, 20.0, 30.0),
            TasteBand::new(EqBandKind::HighPass, 2000.0, 0.0, 0.7),
            TasteBand::new(EqBandKind::LowPass, 1000.0, 0.0, 0.7),
            TasteBand::new(EqBandKind::HighShelf, 8000.0, -15.0, 5.0),
            TasteBand::new(EqBandKind::Peak, f32::NAN, f32::NAN, f32::NAN),
        ];
        let (b, _, notes) = clamp_taste_curve(&wild, Tilt::default(), false);
        assert_eq!((b[0].band.gain_db, b[0].band.q), (MAX_TASTE_BAND_DB, MAX_TASTE_Q));
        assert_eq!(b[1].band.freq_hz, MAX_TASTE_HIGHPASS_HZ);
        assert_eq!(b[2].band.freq_hz, MIN_TASTE_LOWPASS_HZ);
        assert_eq!((b[3].band.gain_db, b[3].band.q), (-MAX_TASTE_BAND_DB, MAX_TASTE_SLOPE_Q));
        assert!(b[4].band.freq_hz.is_finite() && b[4].band.gain_db == 0.0);
        assert!(notes.len() >= 5, "{notes:?}");
        // more than 8 bands: the first 8
        let many: Vec<TasteBand> =
            (0..12).map(|i| TasteBand::new(EqBandKind::Peak, 100.0 * (i + 1) as f32, -1.0, 1.0)).collect();
        let (b, _, notes) = clamp_taste_curve(&many, Tilt::default(), false);
        assert_eq!(b.len(), MAX_TASTE_BANDS);
        assert!(notes.iter().any(|n| n.contains("first 8")));
        // stacked boosts: scaled until the whole curve fits in +12 dB
        let stacked = vec![
            TasteBand::new(EqBandKind::Peak, 100.0, 9.0, 0.7),
            TasteBand::new(EqBandKind::LowShelf, 120.0, 9.0, 0.7),
            TasteBand::new(EqBandKind::Peak, 3000.0, -4.0, 2.0),
        ];
        let tilt = Tilt { bass_db: 6.0, ..Default::default() };
        let (b, t, notes) = clamp_taste_curve(&stacked, tilt, false);
        let peak = crate::dsp::response_db(&crate::dsp::taste_profile(&b, t), &crate::dsp::log_freqs(512), 48_000)
            .into_iter()
            .fold(0f32, f32::max);
        assert!(peak <= MAX_TASTE_BOOST_DB + 0.1, "{peak}");
        assert!(peak > MAX_TASTE_BOOST_DB - 1.5, "not scaled more than needed: {peak}");
        assert_eq!(b[2].band.gain_db, -4.0, "cuts stay");
        assert!(notes.iter().any(|n| n.contains("+12 dB")));
    }

    #[test]
    fn small_speakers_get_no_deep_boosts_in_curves() {
        let deep = vec![
            TasteBand::new(EqBandKind::LowShelf, 80.0, 6.0, 0.7).with_note("thump"),
            TasteBand::new(EqBandKind::Peak, 60.0, -3.0, 1.0),
        ];
        let (b, t, notes) = clamp_taste_curve(&deep, Tilt { bass_db: 6.0, ..Default::default() }, true);
        assert_eq!((b[0].band.kind, b[0].band.freq_hz), (EqBandKind::Peak, crate::dsp::SMALL_SPEAKER_BASS_HZ));
        assert!(b[0].band.gain_db <= crate::dsp::SMALL_SPEAKER_MAX_BASS_DB);
        assert_eq!(b[0].note, "thump");
        assert_eq!(b[1].band.gain_db, -3.0, "a cut stays a cut");
        assert!(b.iter().any(|b| b.band.kind == EqBandKind::HighPass));
        assert_eq!(t.bass_db, crate::dsp::SMALL_SPEAKER_MAX_BASS_DB);
        assert!(notes.iter().any(|n| n.starts_with("built-in speaker")));
    }

    #[test]
    fn curve_summaries() {
        let mut r = rule(TasteScope::Genre { name: "Rock".into() }, 0.0).with_bands(vec![
            TasteBand::new(EqBandKind::Peak, 3200.0, -2.0, 3.0),
            TasteBand::new(EqBandKind::LowShelf, 120.0, 1.5, 0.7),
            TasteBand::new(EqBandKind::HighPass, 30.0, 0.0, 0.7),
            TasteBand::new(EqBandKind::Peak, 7000.0, -1.0, 4.0),
        ]);
        assert_eq!(curve_summary(&r, false), "4 bands · -2 dB 3.2k, +1.5 dB 120 Hz, -1 dB 7k, …");
        r.bands.truncate(1);
        r.tilt.bass_db = 2.0;
        assert_eq!(curve_summary(&r, true), "1 banda · -2 dB 3.2k · +2 dB de bajo");
        assert_eq!(hz(10_000.0), "10 kHz");
        assert_eq!(hz(3150.0), "3.2 kHz");
        assert!(!r.is_neutral());
        assert!(rule(TasteScope::Genre { name: "x".into() }, 0.0).is_neutral());
    }

    #[test]
    fn genres_split_and_match() {
        assert_eq!(split_genres("Electronic, House;electronic"), ["Electronic", "House"]);
        assert!(genre_matches("Hip-Hop/Rap", "hip-hop"));
        assert!(genre_matches("rap", "Hip-Hop/Rap"));
        assert!(!genre_matches("Pop", "K-Pop"));
        assert!(!genre_matches("", ""));
        let scopes = ctx().scopes();
        let kinds: Vec<&str> = scopes.iter().map(TasteScope::kind).collect();
        assert_eq!(kinds, ["track", "album", "artist", "playlist", "genre"]);
    }
}
