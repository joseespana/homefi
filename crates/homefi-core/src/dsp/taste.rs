//! Taste layers: a sound that applies only to some music (a genre, a
//! playlist, an artist, an album or one track), on top of the output's own
//! profile. A rule is a [`Tilt`] (three broad knobs) and / or a parametric
//! curve of up to [`MAX_TASTE_BANDS`] [`TasteBand`]s. The most specific rule
//! that matches what plays wins: track > album > playlist > artist > genre.
//!
//! Persisted like [`super::EqSettings`]: stable camelCase JSON, and an
//! unreadable rule is dropped instead of failing the whole file.

use serde::{Deserialize, Deserializer, Serialize};

use super::{EqBand, EqBandKind, Tilt};

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
    /// Higher wins: track 5, album 4, playlist 3, artist 2, genre 1.
    pub fn specificity(&self) -> u8 {
        match self {
            Self::Track { .. } => 5,
            Self::Album { .. } => 4,
            Self::Playlist { .. } => 3,
            Self::Artist { .. } => 2,
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

/// A taste adjustment for some music.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TasteRule {
    pub id: String,
    pub scope: TasteScope,
    /// The three broad knobs (simple rules; kept for old rules).
    #[serde(default)]
    pub tilt: Tilt,
    /// The parametric curve, on top of the tilt (at most [`MAX_TASTE_BANDS`]).
    #[serde(default, deserialize_with = "lenient_bands", skip_serializing_if = "Vec::is_empty")]
    pub bands: Vec<TasteBand>,
    /// What the user sees ("More punch for Electronic").
    #[serde(default)]
    pub label: String,
    /// Why, in a sentence or two (the assistant's words; may be empty).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub explanation: String,
    #[serde(default, deserialize_with = "lenient_creator")]
    pub created_by: TasteCreator,
    #[serde(default = "yes")]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

impl TasteRule {
    /// A new, enabled rule with a fresh id; the tilt is brought in range.
    pub fn new(scope: TasteScope, tilt: Tilt, label: impl Into<String>, created_by: TasteCreator) -> Self {
        Self {
            id: new_id(),
            scope,
            tilt: tilt.clamped(MAX_TILT_DB),
            bands: Vec::new(),
            label: label.into(),
            explanation: String::new(),
            created_by,
            enabled: true,
        }
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

    /// `true` when it changes nothing (a neutral tilt and no audible band).
    pub fn is_neutral(&self) -> bool {
        self.tilt.is_neutral() && self.bands.iter().all(|b| b.band.is_neutral())
    }

    /// The curve's filters for the kind of output (clamped, audible ones only).
    pub fn curve_for(&self, small_speaker: bool) -> (Vec<EqBand>, Tilt) {
        let (bands, tilt, _) = clamp_taste_curve(&self.bands, self.tilt, small_speaker);
        (bands.into_iter().map(|b| b.band).filter(|b| !b.is_neutral()).collect(), tilt)
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
    /// Rules to add, or to replace by id (else by scope).
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
        match self.upsert.iter().position(|r| r.id == rule.id || r.scope.same_as(&rule.scope)) {
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

/// Every taste rule of an app, and the switch for all of them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct TasteSettings {
    /// Off until the first rule is made ([`TasteSettings::upsert`] turns it on).
    pub enabled: bool,
    #[serde(deserialize_with = "lenient_rules")]
    pub rules: Vec<TasteRule>,
    /// How strong what the assistant, the reactions and the wizard propose is.
    #[serde(deserialize_with = "lenient_intensity")]
    pub intensity: TasteIntensity,
}

impl TasteSettings {
    /// The enabled rule for exactly `scope`, if any.
    pub fn rule_for(&self, scope: &TasteScope) -> Option<&TasteRule> {
        self.rules.iter().find(|r| r.scope.same_as(scope))
    }

    /// Adds `rule`, or replaces the one with its id or its scope (keeping that
    /// one's id). The first rule turns the layers on. Returns the stored rule.
    pub fn upsert(&mut self, mut rule: TasteRule) -> TasteRule {
        rule.clamp();
        if self.rules.is_empty() {
            self.enabled = true;
        }
        let at = self
            .rules
            .iter()
            .position(|r| r.id == rule.id)
            .or_else(|| self.rules.iter().position(|r| r.scope.same_as(&rule.scope)));
        match at {
            Some(i) => {
                rule.id = self.rules[i].id.clone();
                self.rules[i] = rule.clone();
            }
            None => {
                self.rules.push(rule.clone());
                if self.rules.len() > MAX_RULES {
                    self.rules.remove(0);
                }
            }
        }
        rule
    }

    /// Merges `change` into these settings as they are now: removals, then
    /// upserts (by id, else by scope, keeping the stored rule's id), then the switch.
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

    /// Removes the rule `id`; `true` when there was one.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.rules.len();
        self.rules.retain(|r| r.id != id);
        before != self.rules.len()
    }

    /// Every tilt and curve in range, ids filled in.
    pub fn clamped(&self) -> Self {
        let mut s = self.clone();
        for r in &mut s.rules {
            r.clamp();
            if r.id.trim().is_empty() {
                r.id = new_id();
            }
        }
        s
    }
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
        if let Some(p) = self.playlist().filter(|p| !p.is_empty()) {
            out.push(TasteScope::Playlist { id: p.id, name: p.name });
        }
        if let Some(a) = self.main_artist() {
            out.push(TasteScope::Artist { id: a.id.clone(), name: a.name.clone() });
        }
        if let Some(g) = self.main_genre() {
            out.push(TasteScope::Genre { name: g.to_owned() });
        }
        out
    }
}

/// The rule that applies to `ctx` and its tilt (in range); `None` when the
/// layers are off or nothing matches. Most specific wins: track > album >
/// playlist > artist > genre; between two of the same kind, the track's main
/// genre / artist (the first one) wins. Cheap: run it on track change.
pub fn resolve_taste<'a>(settings: &'a TasteSettings, ctx: &NowPlayingContext) -> Option<(&'a TasteRule, Tilt)> {
    if !settings.enabled {
        return None;
    }
    settings
        .rules
        .iter()
        .filter(|r| r.enabled)
        .filter_map(|r| r.scope.fit(ctx).map(|fit| (fit, r)))
        // ties: the later rule (newer) wins
        .max_by(|(a, _), (b, _)| a.cmp(b))
        .map(|(_, r)| (r, r.tilt.clamped(MAX_TILT_DB)))
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
        TasteRule::new(scope, bass(db), "", TasteCreator::Manual)
    }

    #[test]
    fn the_most_specific_rule_wins() {
        let mut s = TasteSettings::default();
        s.upsert(rule(TasteScope::Genre { name: "electronic".into() }, 1.0));
        s.upsert(rule(TasteScope::Artist { id: String::new(), name: "daft punk".into() }, 2.0));
        s.upsert(rule(TasteScope::Playlist { id: "pl-1".into(), name: "Friday".into() }, 3.0));
        s.upsert(rule(TasteScope::Album { id: "22".into(), name: "Other name".into() }, 4.0));
        s.upsert(rule(TasteScope::Track { id: "/m/a.flac".into(), name: "Song".into() }, 5.0));
        assert!(s.enabled, "the first rule turns the layers on");
        let c = ctx();
        let mut expect = [5.0, 4.0, 3.0, 2.0, 1.0].into_iter();
        while let Some((r, t)) = resolve_taste(&s, &c) {
            assert_eq!(t.bass_db, expect.next().unwrap(), "{:?}", r.scope);
            let id = r.id.clone();
            s.remove(&id);
        }
        assert_eq!(expect.next(), None);
    }

    #[test]
    fn ties_go_to_the_main_genre_and_off_means_off() {
        let mut s = TasteSettings::default();
        s.upsert(rule(TasteScope::Genre { name: "House".into() }, 2.0));
        s.upsert(rule(TasteScope::Genre { name: "Electronic".into() }, 1.0));
        assert_eq!(resolve_taste(&s, &ctx()).unwrap().1.bass_db, 1.0);
        s.rules[1].enabled = false;
        assert_eq!(resolve_taste(&s, &ctx()).unwrap().1.bass_db, 2.0);
        s.enabled = false;
        assert!(resolve_taste(&s, &ctx()).is_none());
        // ids win over names: another album with the same title doesn't match
        let mut s = TasteSettings::default();
        s.upsert(rule(TasteScope::Album { id: "99".into(), name: "Discovery".into() }, 3.0));
        assert!(resolve_taste(&s, &ctx()).is_none());
        // nothing matches a track of another genre
        let jazz = NowPlayingContext { genres: vec!["Jazz".into()], ..Default::default() };
        assert!(resolve_taste(&TasteSettings::default(), &jazz).is_none());
    }

    #[test]
    fn upsert_replaces_the_same_scope_and_clamps() {
        let mut s = TasteSettings::default();
        let a = s.upsert(rule(TasteScope::Genre { name: "Rock".into() }, 2.0));
        let b = s.upsert(rule(TasteScope::Genre { name: "rock".into() }, 40.0));
        assert_eq!(s.rules.len(), 1);
        assert_eq!(a.id, b.id);
        assert_eq!(s.rules[0].tilt.bass_db, MAX_TILT_DB);
        assert!(s.remove(&a.id) && !s.remove(&a.id));
    }

    #[test]
    fn json_is_stable_and_lenient() {
        let mut s = TasteSettings::default();
        let mut r = rule(TasteScope::Artist { id: "7".into(), name: "Bach".into() }, 2.0);
        r.id = "r1".into();
        r.label = "Warmer Bach".into();
        r.created_by = TasteCreator::Reaction;
        s.upsert(r);
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "enabled": true,
                "intensity": "balanced",
                "rules": [{
                    "id": "r1",
                    "scope": {"kind": "artist", "id": "7", "name": "Bach"},
                    "tilt": {"bassDb": 2.0, "trebleDb": 0.0, "clarityDb": null},
                    "label": "Warmer Bach",
                    "createdBy": "reaction",
                    "enabled": true
                }]
            })
        );
        assert_eq!(serde_json::from_value::<TasteSettings>(json).unwrap(), s);
        // unknown scopes / creators from newer versions
        let s: TasteSettings = serde_json::from_str(
            r#"{"enabled":true,"rules":[{"id":"a","scope":{"kind":"mood","name":"x"}},{"id":"b","scope":{"kind":"genre","name":"Pop"},"createdBy":"robot"}]}"#,
        )
        .unwrap();
        assert_eq!(s.rules.len(), 1);
        assert_eq!(s.rules[0].created_by, TasteCreator::Manual);
        assert!(s.rules[0].enabled);
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
        // old files have none
        let s: TasteSettings = serde_json::from_str(r#"{"enabled":true,"rules":[]}"#).unwrap();
        assert_eq!(s.intensity, TasteIntensity::Balanced);
    }

    #[test]
    fn apply_merges_by_scope_and_keeps_the_id() {
        let mut s = TasteSettings::default();
        let jazz = s.upsert(rule(TasteScope::Genre { name: "Jazz".into() }, 1.0));
        let rock = s.upsert(rule(TasteScope::Genre { name: "Rock".into() }, 2.0));
        // another copy of the Rock rule (new id, same scope) replaces it and keeps the stored id
        let again = rule(TasteScope::Genre { name: "rock".into() }, 4.0);
        assert_ne!(again.id, rock.id);
        let mut change = TasteChange::upserting(again);
        change.push_upsert(rule(TasteScope::Genre { name: "Pop".into() }, 3.0));
        change.push_remove(jazz.id.clone());
        s.apply(&change);
        assert_eq!(s.rules.len(), 2);
        let stored = s.rule_for(&TasteScope::Genre { name: "Rock".into() }).unwrap();
        assert_eq!((stored.id.as_str(), stored.tilt.bass_db), (rock.id.as_str(), 4.0));
        assert!(s.rule_for(&TasteScope::Genre { name: "Jazz".into() }).is_none());
        // an empty change changes nothing; the first rule turns the layers on; the switch is explicit
        let before = s.clone();
        s.apply(&TasteChange::default());
        assert_eq!(s, before);
        assert!(TasteChange::default().is_empty() && !change.is_empty());
        let mut fresh = TasteSettings::default();
        fresh.apply(&TasteChange::upserting(rule(TasteScope::Genre { name: "Rock".into() }, 1.0)));
        assert!(fresh.enabled);
        fresh.apply(&TasteChange { enabled: Some(false), ..Default::default() });
        assert!(!fresh.enabled && fresh.rules.len() == 1);
        // recording keeps a change coherent: a later removal cancels an upsert, a later upsert cancels a removal
        let mut c = TasteChange::upserting(rule(TasteScope::Genre { name: "Rock".into() }, 1.0));
        let id = c.upsert[0].id.clone();
        c.push_remove(id.clone());
        assert!(c.upsert.is_empty() && c.remove == [id.clone()]);
        let mut back = rule(TasteScope::Genre { name: "Rock".into() }, 2.0);
        back.id = id;
        c.push_upsert(back);
        assert!(c.remove.is_empty() && c.upsert.len() == 1);
        // JSON is camelCase and every field optional
        let json = serde_json::to_value(TasteChange::removing("x")).unwrap();
        assert_eq!(json, serde_json::json!({"upsert": [], "remove": ["x"], "enabled": null}));
        assert!(serde_json::from_str::<TasteChange>("{}").unwrap().is_empty());
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
        // a rule from before curves: no bands, no explanation, same JSON back
        let old = r#"{"id":"r1","scope":{"kind":"genre","name":"Pop"},"tilt":{"bassDb":2.0,"trebleDb":0.0,"clarityDb":null},"label":"x","createdBy":"ai","enabled":true}"#;
        let r: TasteRule = serde_json::from_str(old).unwrap();
        assert!(r.bands.is_empty() && r.explanation.is_empty());
        assert_eq!(serde_json::to_value(&r).unwrap(), serde_json::from_str::<serde_json::Value>(old).unwrap());
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
        assert_eq!(kinds, ["track", "album", "playlist", "artist", "genre"]);
    }
}
