//! One album, many releases.
//!
//! Tidal lists the same album several times on an artist page: explicit and
//! clean, stereo and Dolby Atmos, a 16-bit and a 24-bit copy, remasters,
//! deluxe box sets. [`group_album_versions`] puts those together, picks the one
//! to show by default from [`VersionPrefs`] and labels every version with what
//! makes it different ([`VersionLabel`], [`AlbumVersion::description`]).
//!
//! Grouping rules:
//! - Same main artist and same kind (albums, EPs and singles never mix).
//! - Same base title: case, accents and punctuation ignored, and version-ish
//!   suffixes in `()` / `[]` or after ` - ` dropped (remaster(ed) [year],
//!   deluxe, expanded, box set, anniversary, bonus, special edition, Atmos mix,
//!   Dolby Atmos, explicit, clean, edited, stereo...). Anything else in the
//!   suffix (a take, a demo, a radio edit) keeps the release apart.
//! - Live and studio stay apart, and so do two live recordings from different
//!   places (`Live at X`).
//! - Tie-breakers within the same title: releases more than two years apart,
//!   or with very different track counts, are different albums unless one of
//!   them says it is a remaster / reissue / bigger edition.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::models::Album;

/// Explicit or clean, when both exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExplicitPref {
    /// The artist's original (Tidal's default).
    #[default]
    Explicit,
    Clean,
}

/// Stereo or Dolby Atmos, when both exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SpatialPref {
    /// HomeFi plays stereo; Atmos needs an Atmos device.
    #[default]
    Stereo,
    Atmos,
}

/// The original release or a remaster, when both exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EditionPref {
    #[default]
    Original,
    /// The newest remaster.
    Remaster,
}

/// How album versions are grouped and which one is shown by default. The
/// highest quality always wins among otherwise equal versions; bigger editions
/// (deluxe, box sets) are only the default when nothing else is left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VersionPrefs {
    /// Show one card per album; off lists every release as Tidal does.
    pub group: bool,
    pub explicit: ExplicitPref,
    pub spatial: SpatialPref,
    pub edition: EditionPref,
}

impl Default for VersionPrefs {
    fn default() -> Self {
        Self {
            group: true,
            explicit: ExplicitPref::default(),
            spatial: SpatialPref::default(),
            edition: EditionPref::default(),
        }
    }
}

/// Language of labels and descriptions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    En,
    Es,
}

impl Lang {
    /// `es`, `es-MX`, `es_ES`... are Spanish; anything else is English.
    pub fn from_code(code: &str) -> Self {
        if code.trim().to_ascii_lowercase().starts_with("es") {
            Self::Es
        } else {
            Self::En
        }
    }
}

/// What tells a version apart from its siblings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VersionLabel {
    Explicit,
    /// Not explicit while a sibling is.
    Clean,
    DolbyAtmos,
    /// 24-bit.
    HiRes,
    /// 16-bit (CD).
    Lossless,
    Remastered {
        year: Option<u16>,
    },
    /// `extra_tracks` against the standard edition, when one is listed.
    Deluxe {
        extra_tracks: Option<u32>,
    },
    BoxSet {
        extra_tracks: Option<u32>,
    },
    Expanded {
        extra_tracks: Option<u32>,
    },
    Anniversary,
    Live,
    /// The first release, when a remaster of it is listed too.
    Original,
}

impl VersionLabel {
    /// Stable id for UIs (`explicit`, `clean`, `dolby_atmos`, `hi_res`,
    /// `lossless`, `remastered`, `deluxe`, `box_set`, `expanded`,
    /// `anniversary`, `live`, `original`).
    pub fn code(&self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::Clean => "clean",
            Self::DolbyAtmos => "dolby_atmos",
            Self::HiRes => "hi_res",
            Self::Lossless => "lossless",
            Self::Remastered { .. } => "remastered",
            Self::Deluxe { .. } => "deluxe",
            Self::BoxSet { .. } => "box_set",
            Self::Expanded { .. } => "expanded",
            Self::Anniversary => "anniversary",
            Self::Live => "live",
            Self::Original => "original",
        }
    }

    /// Short text for a chip: `Explicit`, `Hi-Res`, `Remastered 2023`, `Deluxe +16`...
    pub fn text(&self, lang: Lang) -> String {
        let es = lang == Lang::Es;
        let plus = |name: &str, extra: &Option<u32>| match extra {
            Some(n) => format!("{name} +{n}"),
            None => name.to_owned(),
        };
        match self {
            Self::Explicit => if es { "Explícita" } else { "Explicit" }.into(),
            Self::Clean => if es { "Limpia" } else { "Clean" }.into(),
            Self::DolbyAtmos => "Dolby Atmos".into(),
            Self::HiRes => "Hi-Res".into(),
            Self::Lossless => if es { "Sin pérdida" } else { "Lossless" }.into(),
            Self::Remastered { year } => {
                let name = if es { "Remasterizada" } else { "Remastered" };
                year.map_or_else(|| name.to_owned(), |y| format!("{name} {y}"))
            }
            Self::Deluxe { extra_tracks } => plus("Deluxe", extra_tracks),
            Self::BoxSet { extra_tracks } => plus(if es { "Caja" } else { "Box set" }, extra_tracks),
            Self::Expanded { extra_tracks } => plus(if es { "Ampliada" } else { "Expanded" }, extra_tracks),
            Self::Anniversary => if es { "Aniversario" } else { "Anniversary" }.into(),
            Self::Live => if es { "En vivo" } else { "Live" }.into(),
            Self::Original => "Original".into(),
        }
    }

    /// Longer text for a description line.
    fn long_text(&self, lang: Lang) -> String {
        let es = lang == Lang::Es;
        match self {
            Self::DolbyAtmos => {
                if es { "Dolby Atmos (requiere equipo Atmos)" } else { "Dolby Atmos (needs an Atmos device)" }.into()
            }
            Self::HiRes => "Hi-Res 24-bit".into(),
            Self::Lossless => if es { "Sin pérdida 16-bit" } else { "Lossless 16-bit" }.into(),
            Self::Remastered { year: Some(y) } => {
                if es {
                    format!("Remasterizada en {y}")
                } else {
                    format!("Remastered in {y}")
                }
            }
            Self::Deluxe { .. } => if es { "Edición deluxe" } else { "Deluxe edition" }.into(),
            Self::BoxSet { .. } => if es { "Caja de colección" } else { "Box set" }.into(),
            Self::Expanded { .. } => if es { "Edición ampliada" } else { "Expanded edition" }.into(),
            Self::Anniversary => if es { "Edición aniversario" } else { "Anniversary edition" }.into(),
            Self::Original => if es { "Edición original" } else { "Original release" }.into(),
            other => other.text(lang),
        }
    }

    /// Tracks a deluxe / box set / expanded edition has over the standard one.
    pub fn extra_tracks(&self) -> Option<u32> {
        match self {
            Self::Deluxe { extra_tracks } | Self::BoxSet { extra_tracks } | Self::Expanded { extra_tracks } => {
                *extra_tracks
            }
            _ => None,
        }
    }
}

/// One release of an album.
#[derive(Debug, Clone)]
pub struct AlbumVersion {
    pub album: Album,
    pub labels: Vec<VersionLabel>,
}

impl AlbumVersion {
    pub fn is_atmos(&self) -> bool {
        self.labels.contains(&VersionLabel::DolbyAtmos)
    }

    pub fn year(&self) -> Option<u32> {
        release_year(&self.album).and_then(|y| u32::try_from(y).ok())
    }

    /// One line: `Explicit · Hi-Res 24-bit · 12 tracks · 2023`,
    /// `Remasterizada en 2023 · 28 canciones (16 extra)`...
    pub fn description(&self, lang: Lang) -> String {
        let es = lang == Lang::Es;
        // live is the same for every sibling: the title says it already
        let mut parts: Vec<String> =
            self.labels.iter().filter(|l| **l != VersionLabel::Live).map(|l| l.long_text(lang)).collect();
        let tracks = u32::try_from(self.album.number_of_tracks).unwrap_or(0);
        if tracks > 0 {
            let word = match (es, tracks) {
                (true, 1) => "canción",
                (true, _) => "canciones",
                (false, 1) => "track",
                (false, _) => "tracks",
            };
            parts.push(match self.labels.iter().find_map(VersionLabel::extra_tracks) {
                Some(extra) => format!("{tracks} {word} ({extra} extra)"),
                None => format!("{tracks} {word}"),
            });
        }
        if let Some(year) = self.year() {
            parts.push(year.to_string());
        }
        parts.join(" · ")
    }
}

/// Releases of one album; `primary` is the one shown by default.
#[derive(Debug, Clone)]
pub struct AlbumGroup {
    /// Main artist, kind, live and base title (stable between calls).
    pub key: String,
    pub primary: Album,
    /// Index of `primary` in `versions`.
    pub primary_index: usize,
    /// In the order they came in.
    pub versions: Vec<AlbumVersion>,
}

impl AlbumGroup {
    pub fn primary_version(&self) -> &AlbumVersion {
        &self.versions[self.primary_index]
    }
}

/// Groups `albums` (an artist page, search results, favorites) into one entry
/// per album, in the order each album first appears. See the module docs for
/// the rules; `prefs.group` is not looked at (callers that don't group simply
/// don't call this).
pub fn group_album_versions(albums: &[Album], prefs: &VersionPrefs) -> Vec<AlbumGroup> {
    let parsed: Vec<Parsed> = albums.iter().map(Parsed::new).collect();
    // clusters of indexes into `albums`, in order of their first member
    let mut clusters: Vec<(String, Vec<usize>)> = Vec::new();
    let mut by_key: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, p) in parsed.iter().enumerate() {
        let candidates = by_key.entry(p.key.as_str()).or_default();
        let found = candidates.iter().copied().find(|&c| {
            let members = &clusters[c].1;
            members.iter().all(|&m| compatible(&parsed[m], p))
        });
        match found {
            Some(c) => clusters[c].1.push(i),
            None => {
                candidates.push(clusters.len());
                clusters.push((p.key.clone(), vec![i]));
            }
        }
    }
    clusters
        .into_iter()
        .map(|(key, members)| {
            let versions: Vec<AlbumVersion> = members
                .iter()
                .map(|&m| AlbumVersion { album: albums[m].clone(), labels: labels(&parsed[m], &members, &parsed) })
                .collect();
            let primary_index = pick(&members, &parsed, prefs);
            AlbumGroup { key, primary: versions[primary_index].album.clone(), primary_index, versions }
        })
        .collect()
}

// ------------------------------------------------------------------ parsing

/// What the title, flags and tags say about one release.
#[derive(Debug, Default)]
struct Parsed {
    key: String,
    year: Option<i32>,
    tracks: i64,
    explicit: bool,
    atmos: bool,
    hires: bool,
    lossless: bool,
    popularity: i64,
    remaster: bool,
    remaster_year: Option<u16>,
    deluxe: bool,
    box_set: bool,
    expanded: bool,
    anniversary: bool,
    live: bool,
}

impl Parsed {
    fn new(a: &Album) -> Self {
        let mut full = a.title.trim().to_owned();
        if let Some(v) = a.version.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            if !full.to_lowercase().contains(&v.to_lowercase()) {
                full = format!("{full} ({v})");
            }
        }
        let mut p = Parsed::default();
        let mut live_detail = String::new();
        let mut rest = full.as_str();
        loop {
            let t = rest.trim_end();
            if let Some((head, inner)) = trailing_bracket(t) {
                if p.mark(inner, &mut live_detail) {
                    rest = head;
                    continue;
                }
                break;
            }
            if let Some(at) = t.rfind(" - ") {
                if p.mark(&t[at + 3..], &mut live_detail) {
                    rest = &t[..at];
                    continue;
                }
            }
            break;
        }
        let tags = &a.media_metadata.tags;
        let has = |t: &str| tags.iter().any(|x| x == t);
        p.atmos |= has("DOLBY_ATMOS") || a.audio_modes.iter().any(|m| m == "DOLBY_ATMOS");
        p.hires = has("HIRES_LOSSLESS") || matches!(a.audio_quality.as_deref(), Some("HI_RES_LOSSLESS"));
        p.lossless = has("LOSSLESS") || matches!(a.audio_quality.as_deref(), Some("LOSSLESS" | "HI_RES"));
        p.explicit = a.explicit;
        p.year = release_year(a);
        p.tracks = a.number_of_tracks;
        p.popularity = a.popularity.unwrap_or(0);
        let main = a.artist.as_ref().or(a.artists.first());
        let artist = main.map_or_else(String::new, |m| if m.id != 0 { m.id.to_string() } else { normalize(&m.name) });
        let kind = match a.kind.to_ascii_uppercase().as_str() {
            "" => "ALBUM".to_owned(),
            k => k.to_owned(),
        };
        let mut base = normalize(rest);
        if base.is_empty() {
            // a title that is all suffix: keep it whole
            base = normalize(&full);
        }
        p.key = format!("{artist}|{kind}|{}|{live_detail}|{base}", if p.live { "live" } else { "studio" });
        p
    }

    /// Records what a suffix (`Remastered 2021`, `Atmos Mix`, `Live at X, City`)
    /// says; `false` (and nothing recorded) when it isn't about the version.
    fn mark(&mut self, suffix: &str, live_detail: &mut String) -> bool {
        let lower = suffix.trim().to_lowercase();
        let words: Vec<&str> = lower.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
        if words.first() == Some(&"live") {
            self.live = true;
            // the place, not the date: "Live at the Berkeley Community Theater, ..."
            let place = lower[lower.find("live").map_or(0, |i| i + 4)..].split([',', ';']).next().unwrap_or_default();
            let place = place.split(" - ").next().unwrap_or_default();
            *live_detail = normalize(place);
            return true;
        }
        let any = |list: &[&str]| words.iter().any(|w| list.contains(w));
        let mut found = false;
        if words.iter().any(|w| w.starts_with("remaster")) {
            self.remaster = true;
            self.remaster_year = words.iter().filter_map(|w| w.parse::<u16>().ok()).find(|y| (1950..=2100).contains(y));
            found = true;
        }
        if any(&["deluxe", "super"]) {
            self.deluxe = true;
            found = true;
        }
        if any(&["boxset"]) || lower.contains("box set") {
            self.box_set = true;
            found = true;
        }
        if any(&["expanded", "bonus", "collector", "collectors"]) || lower.contains("special edition") {
            self.expanded = true;
            found = true;
        }
        if any(&["anniversary"]) {
            self.anniversary = true;
            found = true;
        }
        if any(&["atmos", "spatial"]) {
            self.atmos = true;
            found = true;
        }
        if any(&["explicit", "clean", "stereo"]) || lower == "edited" || lower == "edited version" {
            found = true;
        }
        found
    }

    fn edition(&self) -> bool {
        self.deluxe || self.box_set || self.expanded || self.anniversary
    }

    fn reissue(&self) -> bool {
        self.remaster || self.edition() || self.atmos
    }
}

/// `(head, inner)` of a title ending in `(...)` or `[...]`.
fn trailing_bracket(t: &str) -> Option<(&str, &str)> {
    let close = t.chars().last()?;
    let open = match close {
        ')' => '(',
        ']' => '[',
        _ => return None,
    };
    let mut depth = 0;
    for (i, c) in t.char_indices().rev() {
        if c == close {
            depth += 1;
        } else if c == open {
            depth -= 1;
            if depth == 0 {
                return Some((&t[..i], &t[i + 1..t.len() - 1]));
            }
        }
    }
    None
}

/// Lower case, no accents, letters and digits only.
fn normalize(s: &str) -> String {
    s.nfkd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .filter(|c| c.is_alphanumeric())
        .collect()
}

fn release_year(a: &Album) -> Option<i32> {
    a.release_date.as_deref().and_then(|d| d.get(..4)).and_then(|y| y.parse().ok())
}

/// Two releases with the same base title are the same album.
fn compatible(a: &Parsed, b: &Parsed) -> bool {
    let reissue = a.reissue() || b.reissue();
    if let (Some(ya), Some(yb)) = (a.year, b.year) {
        if (ya - yb).abs() > 2 && !reissue {
            return false;
        }
    }
    if !(a.edition() || b.edition()) && a.tracks > 0 && b.tracks > 0 {
        let (lo, hi) = (a.tracks.min(b.tracks), a.tracks.max(b.tracks));
        if hi - lo > (hi / 4).max(2) {
            return false;
        }
    }
    true
}

fn labels(p: &Parsed, members: &[usize], parsed: &[Parsed]) -> Vec<VersionLabel> {
    let siblings = || members.iter().map(|&m| &parsed[m]);
    let mut out = Vec::new();
    if p.explicit {
        out.push(VersionLabel::Explicit);
    } else if siblings().any(|s| s.explicit && s.edition() == p.edition()) {
        // a box set flagged explicit for its bonus material doesn't make the album "clean"
        out.push(VersionLabel::Clean);
    }
    if p.atmos {
        out.push(VersionLabel::DolbyAtmos);
    } else if p.hires {
        out.push(VersionLabel::HiRes);
    } else if p.lossless {
        out.push(VersionLabel::Lossless);
    }
    if p.remaster {
        out.push(VersionLabel::Remastered { year: p.remaster_year });
    }
    let standard = siblings().filter(|s| !s.edition() && s.tracks > 0).map(|s| s.tracks).min();
    let extra = standard.and_then(|n| u32::try_from(p.tracks - n).ok()).filter(|&n| n > 0);
    if p.box_set {
        out.push(VersionLabel::BoxSet { extra_tracks: extra });
    } else if p.deluxe {
        out.push(VersionLabel::Deluxe { extra_tracks: extra });
    } else if p.expanded {
        out.push(VersionLabel::Expanded { extra_tracks: extra });
    }
    if p.anniversary {
        out.push(VersionLabel::Anniversary);
    }
    if p.live {
        out.push(VersionLabel::Live);
    }
    if !p.reissue() && siblings().any(|s| s.remaster) {
        out.push(VersionLabel::Original);
    }
    out
}

/// Index (into `members`) of the default version.
fn pick(members: &[usize], parsed: &[Parsed], prefs: &VersionPrefs) -> usize {
    let rank = |p: &Parsed| {
        let explicit = match prefs.explicit {
            ExplicitPref::Explicit => !p.explicit,
            ExplicitPref::Clean => p.explicit,
        };
        let spatial = match prefs.spatial {
            SpatialPref::Stereo => p.atmos,
            SpatialPref::Atmos => !p.atmos,
        };
        let remaster = match prefs.edition {
            EditionPref::Original => p.remaster,
            EditionPref::Remaster => !p.remaster,
        };
        let quality = if p.hires {
            0
        } else if p.lossless {
            1
        } else {
            2
        };
        // the newest remaster first when remasters are preferred, else the oldest release
        let year = i32::from(p.remaster_year.unwrap_or(0)).max(p.year.unwrap_or(0));
        let year = if prefs.edition == EditionPref::Remaster { -year } else { year };
        // bigger editions (box sets with bonus material) only when nothing else is left
        (p.edition(), explicit, spatial, remaster, quality, year, -p.popularity)
    };
    (0..members.len()).min_by_key(|&i| (rank(&parsed[members[i]]), i)).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ArtistRef, MediaMetadata};

    /// Metallica's "Albums" and "EPs & singles" as Tidal listed them
    /// (2026-10-28), titles / ids / flags only.
    fn metallica() -> Vec<(String, Album)> {
        let raw = include_str!("../tests/fixtures/metallica_albums.json");
        let list: Vec<serde_json::Value> = serde_json::from_str(raw).unwrap();
        list.into_iter()
            .map(|v| (v["filter"].as_str().unwrap().to_owned(), serde_json::from_value(v).unwrap()))
            .collect()
    }

    fn albums_only() -> Vec<Album> {
        metallica().into_iter().filter(|(f, _)| f == "ALBUMS").map(|(_, a)| a).collect()
    }

    fn singles() -> Vec<Album> {
        metallica().into_iter().filter(|(f, _)| f != "ALBUMS").map(|(_, a)| a).collect()
    }

    fn group_of(groups: &[AlbumGroup], id: i64) -> &AlbumGroup {
        groups.iter().find(|g| g.versions.iter().any(|v| v.album.id == id)).unwrap()
    }

    fn ids(g: &AlbumGroup) -> Vec<i64> {
        let mut v: Vec<i64> = g.versions.iter().map(|v| v.album.id).collect();
        v.sort_unstable();
        v
    }

    fn album(id: i64, title: &str, year: &str, tracks: i64) -> Album {
        serde_json::from_value(serde_json::json!({
            "id": id, "title": title, "releaseDate": format!("{year}-01-01"), "numberOfTracks": tracks,
            "type": "ALBUM", "artist": {"id": 1, "name": "Band"},
        }))
        .unwrap()
    }

    #[test]
    fn metallica_albums_collapse_to_one_card_each() {
        let groups = group_album_versions(&albums_only(), &VersionPrefs::default());
        let titles: Vec<&str> = groups.iter().map(|g| g.primary.title.as_str()).collect();
        assert_eq!(albums_only().len(), 43);
        assert_eq!(groups.len(), 20, "{titles:#?}");
        // 72 Seasons + its Atmos mix
        assert_eq!(ids(group_of(&groups, 288984589)), [288984589, 289103426]);
        // Reload / ReLoad (Remastered) x2 / Reload Hi-Res / the box set
        assert_eq!(group_of(&groups, 166789082).versions.len(), 5);
        // Load: Atmos, the box set and the remaster
        assert_eq!(group_of(&groups, 441830016).versions.len(), 3);
        // the hyphen doesn't split Hardwired
        assert_eq!(ids(group_of(&groups, 133173644)), [133173644, 166787393]);
        // S&M and S&M2 are different albums; the Tidal typo (199 / 1999) isn't
        assert_ne!(group_of(&groups, 186461110).key, group_of(&groups, 186461032).key);
        assert_eq!(ids(group_of(&groups, 186461032)), [167301995, 186461032]);
        // the self-titled album with its remaster and box set
        assert_eq!(group_of(&groups, 186460915).versions.len(), 4);
        // Lou Reed's Lulu stands alone
        assert_eq!(group_of(&groups, 34436068).versions.len(), 1);
        // order is Tidal's: the first card is still 72 Seasons
        assert_eq!(groups[0].primary.title, "72 Seasons");
    }

    #[test]
    fn singles_keep_takes_mixes_and_far_apart_live_recordings_apart() {
        let groups = group_album_versions(&singles(), &VersionPrefs::default());
        // Lux Æterna stereo + Atmos
        assert_eq!(ids(group_of(&groups, 263197168)), [263197168, 263412759]);
        // "Enter Sandman (Remastered)" and the 1990 demo are different singles
        assert_ne!(group_of(&groups, 188515350).key, group_of(&groups, 188515368).key);
        // Nothing Else Matters (Live) from 1992 and 2020 are different recordings
        assert_ne!(ids(group_of(&groups, 166786360)), ids(group_of(&groups, 186461133)), "1992 and 2020 live singles");
        // the two radio edits are the same single, apart from the full live take
        assert_eq!(ids(group_of(&groups, 150856344)), [150856344, 150856667]);
        // a 1-track "72 Seasons" single never joins the album
        let all: Vec<Album> = metallica().into_iter().map(|(_, a)| a).collect();
        let mixed = group_album_versions(&all, &VersionPrefs::default());
        assert_eq!(group_of(&mixed, 286265415).versions.len(), 1);
        // Fuel live at Reading stays apart from the studio remaster
        assert_ne!(group_of(&groups, 526412290).key, group_of(&groups, 526412274).key);
    }

    #[test]
    fn the_default_pick_follows_the_preferences() {
        let all = albums_only();
        let defaults = group_album_versions(&all, &VersionPrefs::default());
        // stereo and Hi-Res over Atmos
        assert_eq!(group_of(&defaults, 288984589).primary.id, 288984589);
        // original Reload, Hi-Res copy
        assert_eq!(group_of(&defaults, 166789082).primary.id, 186460967);
        // explicit St. Anger even though the clean one is Hi-Res
        assert_eq!(group_of(&defaults, 186460992).primary.id, 166786581);
        // the self-titled album: original, Hi-Res
        assert_eq!(group_of(&defaults, 186460915).primary.id, 186460915);

        let remaster = VersionPrefs { edition: EditionPref::Remaster, ..VersionPrefs::default() };
        let g = group_album_versions(&all, &remaster);
        // ReLoad (Remastered) stereo, never the box set
        assert_eq!(group_of(&g, 166789082).primary.id, 536860664);
        assert_eq!(group_of(&g, 186460915).primary.id, 197137267);

        let clean = VersionPrefs { explicit: ExplicitPref::Clean, ..VersionPrefs::default() };
        assert_eq!(group_of(&group_album_versions(&all, &clean), 186460992).primary.id, 186460992);

        let atmos = VersionPrefs { spatial: SpatialPref::Atmos, ..VersionPrefs::default() };
        let g = group_album_versions(&all, &atmos);
        assert_eq!(group_of(&g, 288984589).primary.id, 289103426);
        assert_eq!(group_of(&g, 166789082).primary.id, 536933966);
        // no Atmos version: still the usual one
        assert_eq!(group_of(&g, 186460915).primary.id, 186460915);
    }

    #[test]
    fn versions_are_labelled_and_described() {
        let groups = group_album_versions(&albums_only(), &VersionPrefs::default());
        let reload = group_of(&groups, 166789082);
        let version = |id: i64| reload.versions.iter().find(|v| v.album.id == id).unwrap();
        let codes = |id: i64| version(id).labels.iter().map(VersionLabel::code).collect::<Vec<_>>();
        assert_eq!(codes(166789082), ["explicit", "lossless", "original"]);
        assert_eq!(codes(536933966), ["explicit", "dolby_atmos", "remastered"]);
        assert_eq!(codes(536860794), ["explicit", "hi_res", "remastered", "box_set"]);
        assert_eq!(version(536860794).labels[3], VersionLabel::BoxSet { extra_tracks: Some(209) });
        assert_eq!(version(536860794).labels[3].text(Lang::En), "Box set +209");
        assert_eq!(
            version(186460967).description(Lang::En),
            "Explicit · Hi-Res 24-bit · Original release · 13 tracks · 1997"
        );
        assert_eq!(
            version(536860794).description(Lang::Es),
            "Explícita · Hi-Res 24-bit · Remasterizada · Caja de colección · 222 canciones (209 extra) · 1997"
        );
        assert_eq!(
            version(536933966).description(Lang::En),
            "Explicit · Dolby Atmos (needs an Atmos device) · Remastered · 13 tracks · 1997"
        );
        // clean only shows up next to an explicit sibling
        let st_anger = group_of(&groups, 186460992);
        let clean = st_anger.versions.iter().find(|v| v.album.id == 186460992).unwrap();
        assert_eq!(clean.labels[0], VersionLabel::Clean);
        assert_eq!(clean.labels[0].text(Lang::Es), "Limpia");
        let seasons = group_of(&groups, 288984589);
        assert!(seasons.versions.iter().all(|v| !v.labels.iter().any(|l| l.code() == "clean")));
        assert!(seasons.versions[1].is_atmos());
        // the self-titled box set is explicit, the album itself is not "clean"
        let black = group_of(&groups, 186460915);
        assert!(black.versions.iter().all(|v| !v.labels.contains(&VersionLabel::Clean)));
        // live is labelled
        assert!(group_of(&groups, 186461110).primary_version().labels.contains(&VersionLabel::Live));
    }

    #[test]
    fn suffixes_years_and_track_counts() {
        let list = vec![
            album(1, "Nevermind", "1991", 13),
            album(2, "Nevermind (Remastered 2011)", "2011", 13),
            album(3, "Nevermind (Super Deluxe Edition)", "2011", 70),
            album(4, "Nevermind - 30th Anniversary Edition", "2021", 94),
            album(5, "Nevermind [Explicit]", "1991", 13),
            // same title, a different album years later
            album(6, "Nevermind", "2019", 9),
            album(7, "Live at Reading", "2009", 24),
            album(8, "Café Tacvba (Dolby Atmos)", "1992", 14),
            album(9, "Cafe Tacuba", "1992", 14),
            album(10, "CAFÉ TACVBA", "1992", 14),
        ];
        let groups = group_album_versions(&list, &VersionPrefs::default());
        assert_eq!(ids(group_of(&groups, 1)), [1, 2, 3, 4, 5]);
        assert_eq!(ids(group_of(&groups, 6)), [6]);
        assert_eq!(ids(group_of(&groups, 7)), [7]);
        assert_eq!(ids(group_of(&groups, 8)), [8, 10]);
        assert_eq!(ids(group_of(&groups, 9)), [9]);
        let nevermind = group_of(&groups, 1);
        let label = |id: i64| nevermind.versions.iter().find(|v| v.album.id == id).unwrap().labels.clone();
        assert_eq!(label(2), [VersionLabel::Remastered { year: Some(2011) }]);
        assert_eq!(label(3), [VersionLabel::Deluxe { extra_tracks: Some(57) }]);
        assert_eq!(label(4), [VersionLabel::Anniversary]);
        assert_eq!(label(1), [VersionLabel::Original]);
        assert_eq!(nevermind.primary.id, 1);
        assert!(group_of(&groups, 8).versions[0].is_atmos());
        assert_eq!(nevermind.versions[2].description(Lang::Es), "Edición deluxe · 70 canciones (57 extra) · 2011");
    }

    #[test]
    fn version_field_kinds_and_artists() {
        let mut a = album(1, "Load", "1996", 14);
        a.version = Some("Remastered".into());
        let mut b = album(2, "Load", "1996", 14);
        b.artist = Some(ArtistRef { id: 2, name: "Other".into(), kind: None, picture: None });
        let mut ep = album(3, "Load", "1996", 4);
        ep.kind = "EP".into();
        let mut atmos = album(4, "Load", "1996", 14);
        atmos.audio_modes = vec!["DOLBY_ATMOS".into()];
        atmos.media_metadata = MediaMetadata { tags: vec!["DOLBY_ATMOS".into()] };
        let groups = group_album_versions(&[a, b, ep, atmos], &VersionPrefs::default());
        assert_eq!(groups.len(), 3);
        assert_eq!(ids(&groups[0]), [1, 4]);
        assert_eq!(groups[0].versions[0].labels, [VersionLabel::Remastered { year: None }]);
        assert_eq!(groups[0].primary.id, 1, "stereo first");
    }

    #[test]
    fn prefs_read_partial_settings() {
        let p: VersionPrefs = serde_json::from_str(r#"{"spatial":"atmos"}"#).unwrap();
        assert!(p.group);
        assert_eq!(p.spatial, SpatialPref::Atmos);
        assert_eq!(p.explicit, ExplicitPref::Explicit);
        assert_eq!(p.edition, EditionPref::Original);
        assert_eq!(Lang::from_code("es-MX"), Lang::Es);
        assert_eq!(Lang::from_code("en_US"), Lang::En);
    }
}
