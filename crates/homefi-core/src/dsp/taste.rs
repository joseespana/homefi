//! Taste layers: a [`Tilt`] that applies only to some music (a genre, a
//! playlist, an artist, an album or one track), on top of the output's own
//! profile. The most specific rule that matches what plays wins:
//! track > album > playlist > artist > genre.
//!
//! Persisted like [`super::EqSettings`]: stable camelCase JSON, and an
//! unreadable rule is dropped instead of failing the whole file.

use serde::{Deserialize, Deserializer, Serialize};

use super::Tilt;

/// Each taste adjustment (bass, treble, clarity) stays within ± this (dB).
pub const MAX_TILT_DB: f32 = 6.0;
/// Rules kept at most (oldest manual ones are never dropped silently: callers cap first).
pub const MAX_RULES: usize = 200;

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

/// A taste adjustment for some music.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TasteRule {
    pub id: String,
    pub scope: TasteScope,
    #[serde(default)]
    pub tilt: Tilt,
    /// What the user sees ("More punch for Electronic").
    #[serde(default)]
    pub label: String,
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
        Self { id: new_id(), scope, tilt: tilt.clamped(MAX_TILT_DB), label: label.into(), created_by, enabled: true }
    }
}

/// A fresh rule id (time and a counter, unique within a run and across runs).
pub fn new_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    format!("t{:x}{:x}", now.as_millis(), COUNTER.fetch_add(1, Ordering::Relaxed) & 0xfff)
}

/// Every taste rule of an app, and the switch for all of them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct TasteSettings {
    /// Off until the first rule is made ([`TasteSettings::upsert`] turns it on).
    pub enabled: bool,
    #[serde(deserialize_with = "lenient_rules")]
    pub rules: Vec<TasteRule>,
}

impl TasteSettings {
    /// The enabled rule for exactly `scope`, if any.
    pub fn rule_for(&self, scope: &TasteScope) -> Option<&TasteRule> {
        self.rules.iter().find(|r| r.scope.same_as(scope))
    }

    /// Adds `rule`, or replaces the one with its id or its scope (keeping that
    /// one's id). The first rule turns the layers on. Returns the stored rule.
    pub fn upsert(&mut self, mut rule: TasteRule) -> TasteRule {
        rule.tilt = rule.tilt.clamped(MAX_TILT_DB);
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

    /// Removes the rule `id`; `true` when there was one.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.rules.len();
        self.rules.retain(|r| r.id != id);
        before != self.rules.len()
    }

    /// Every tilt in range, ids filled in.
    pub fn clamped(&self) -> Self {
        let mut s = self.clone();
        for r in &mut s.rules {
            r.tilt = r.tilt.clamped(MAX_TILT_DB);
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
