//! Headphone correction profiles from AutoEq
//! (<https://github.com/jaakkopasanen/AutoEq>, MIT, by Jaakko Pasanen).
//!
//! AutoEq publishes, for thousands of headphones and several measurement
//! sources, a `ParametricEQ.txt` in the Equalizer APO format:
//!
//! ```text
//! Preamp: -6.1 dB
//! Filter 1: ON LSC Fc 105 Hz Gain 6.4 dB Q 0.70
//! Filter 2: ON PK Fc 8800 Hz Gain 5.1 dB Q 1.42
//! ```
//!
//! and lists them all in `results/INDEX.md` as
//! `- [Model](./source/[rig ]kind/Model) by source[ on rig]`. [`AutoEq`]
//! downloads the index (at most once a day) and the profiles the user opens,
//! keeps them in a cache folder and works from there when offline.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use super::{EqBand, EqBandKind, EqProfile, EqSource, MAX_BANDS};
use crate::error::{Error, Result};
use crate::humane::{language, Lang};

/// Where AutoEq's results live (raw files of the `master` branch).
pub const DEFAULT_BASE_URL: &str = "https://raw.githubusercontent.com/jaakkopasanen/AutoEq/master/results";
/// The index is downloaded again when older than this.
pub const INDEX_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);
/// Most results [`AutoEq::search`] returns.
pub const SEARCH_LIMIT: usize = 50;
const INDEX_FILE: &str = "INDEX.md";
const PROFILES_DIR: &str = "profiles";
const PROFILE_SUFFIX: &str = " ParametricEQ.txt";
const TIMEOUT: Duration = Duration::from_secs(30);
/// Measurement sources first in the results, as AutoEq recommends them.
const PREFERRED_SOURCES: [&str; 5] = ["oratory1990", "crinacle", "Rtings", "Innerfidelity", "HypetheSonics"];

/// One headphone measurement listed by AutoEq.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadphoneEntry {
    /// Model as AutoEq names it (`Sennheiser HD 650`).
    pub name: String,
    /// Who measured it (`oratory1990`, `crinacle`, `Rtings`, ...).
    pub source: String,
    /// `over-ear`, `in-ear` or `earbud`.
    pub kind: String,
    /// Measurement rig when the source used several (`711`, `GRAS 43AG-7`).
    #[serde(default)]
    pub rig: Option<String>,
    /// Folder under `results/` (`oratory1990/over-ear/Sennheiser HD 650`).
    pub path: String,
}

/// AutoEq client with an on-disk cache in `cache_dir`.
pub struct AutoEq {
    dir: PathBuf,
    base: String,
    http: reqwest::Client,
    /// Parsed index and when it was loaded.
    index: Mutex<Option<(Instant, Arc<Vec<HeadphoneEntry>>)>>,
    /// One load of the index at a time: searches typed while it downloads wait
    /// for it instead of downloading it again.
    loading: tokio::sync::Mutex<()>,
}

impl AutoEq {
    pub fn new(cache_dir: impl Into<PathBuf>) -> Self {
        Self::with_base_url(cache_dir, DEFAULT_BASE_URL)
    }

    /// Another server with the same layout as `results/` (tests).
    pub fn with_base_url(cache_dir: impl Into<PathBuf>, base_url: &str) -> Self {
        let http = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .user_agent(concat!("HomeFi/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_default();
        Self {
            dir: cache_dir.into(),
            base: base_url.trim_end_matches('/').to_owned(),
            http,
            index: Mutex::new(None),
            loading: Default::default(),
        }
    }

    pub fn cache_dir(&self) -> &Path {
        &self.dir
    }

    /// Every headphone AutoEq lists. Downloaded at most once a day; the cached
    /// copy is used when it is fresh or when the download fails.
    pub async fn index(&self) -> Result<Arc<Vec<HeadphoneEntry>>> {
        if let Some(list) = self.loaded() {
            return Ok(list);
        }
        let _single = self.loading.lock().await;
        // someone else may have loaded it while we waited
        if let Some(list) = self.loaded() {
            return Ok(list);
        }

        let path = self.dir.join(INDEX_FILE);
        // parsed once each: the cached copy and, if needed, the download
        let cached = match tokio::fs::read_to_string(&path).await {
            Ok(text) => Some(parse_index_blocking(text).await).filter(|l| !l.is_empty()),
            Err(_) => None,
        };
        let fresh = cached.is_some() && age(&path).is_some_and(|a| a < INDEX_MAX_AGE);

        let list = match cached {
            Some(list) if fresh => list,
            cached => {
                let downloaded = match self.get(&format!("{}/{INDEX_FILE}", self.base)).await {
                    Ok(text) => {
                        let list = parse_index_blocking(text.clone()).await;
                        if list.is_empty() {
                            Err("the index has no entries".to_owned())
                        } else {
                            if let Err(e) = write_atomic(&path, &text).await {
                                tracing::warn!("could not cache the AutoEq index: {e}");
                            }
                            Ok(list)
                        }
                    }
                    Err(e) => Err(e.to_string()),
                };
                match downloaded {
                    Ok(list) => list,
                    Err(detail) => {
                        tracing::warn!("AutoEq index download failed: {detail}");
                        // offline: yesterday's list is still good
                        cached.ok_or_else(|| {
                            worded(
                                "Couldn't download the headphone list from AutoEq. Check your connection and try again.",
                                "No se pudo descargar la lista de audífonos de AutoEq. Revisa la conexión y vuelve a intentarlo.",
                            )
                        })?
                    }
                }
            }
        };
        let list = Arc::new(list);
        *self.index.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), list.clone()));
        Ok(list)
    }

    /// The index in memory, while it is fresh.
    fn loaded(&self) -> Option<Arc<Vec<HeadphoneEntry>>> {
        let index = self.index.lock().unwrap_or_else(|e| e.into_inner());
        index.as_ref().filter(|(at, _)| at.elapsed() < INDEX_MAX_AGE).map(|(_, list)| list.clone())
    }

    /// Headphones matching `query` (words in any order, accents and spacing
    /// ignored: `hd650` finds `Sennheiser HD 650`), best first, at most
    /// [`SEARCH_LIMIT`].
    pub async fn search(&self, query: &str) -> Result<Vec<HeadphoneEntry>> {
        Ok(search_entries(&self.index().await?, query, SEARCH_LIMIT))
    }

    /// The correction profile of `entry`. Downloaded once, then read from the cache.
    pub async fn profile(&self, entry: &HeadphoneEntry) -> Result<EqProfile> {
        let segments = path_segments(&entry.path)
            .ok_or_else(|| worded("That isn't an AutoEq headphone.", "Eso no es un audífono de AutoEq."))?;
        let file = self.dir.join(PROFILES_DIR).join(cache_name(&entry.path));
        if let Ok(text) = tokio::fs::read_to_string(&file).await {
            if let Ok(p) = parse_parametric_eq(&text, &entry.name, &entry.source) {
                return Ok(p);
            }
        }
        let url = profile_url(&self.base, &segments)
            .ok_or_else(|| worded("That isn't an AutoEq headphone.", "Eso no es un audífono de AutoEq."))?;
        let text = self.get(&url).await.map_err(|e| {
            tracing::warn!("AutoEq profile download failed: {e}");
            match e {
                Error::Http(e) if e.status().is_some_and(|s| s.as_u16() == 404) => worded(
                    "AutoEq doesn't have this profile anymore. Search for the headphones again.",
                    "AutoEq ya no tiene este perfil. Vuelve a buscar los audífonos.",
                ),
                _ => worded(
                    "Couldn't download this headphone's profile from AutoEq. Check your connection and try again.",
                    "No se pudo descargar el perfil de estos audífonos de AutoEq. Revisa la conexión y vuelve a intentarlo.",
                ),
            }
        })?;
        let profile = parse_parametric_eq(&text, &entry.name, &entry.source)?;
        if let Err(e) = write_atomic(&file, &text).await {
            tracing::warn!("could not cache the AutoEq profile: {e}");
        }
        Ok(profile)
    }

    async fn get(&self, url: &str) -> Result<String> {
        Ok(self.http.get(url).send().await?.error_for_status()?.text().await?)
    }
}

/// Parses AutoEq's `INDEX.md`; lines that don't look like entries are skipped.
pub fn parse_index(markdown: &str) -> Vec<HeadphoneEntry> {
    markdown.lines().filter_map(parse_index_line).collect()
}

fn parse_index_line(line: &str) -> Option<HeadphoneEntry> {
    let rest = line.trim().strip_prefix("- [")?;
    let (name, rest) = rest.split_once("](./")?;
    // the path may contain `)` itself: the link ends at the last `) by `
    let end = rest.rfind(") by ")?;
    let path = percent_decode(&rest[..end]);
    let [source, folder, model] = path_segments(&path)?;
    let (rig, kind) = match folder.rsplit_once(' ') {
        Some((rig, kind)) => (Some(rig.trim().to_owned()).filter(|r| !r.is_empty()), kind),
        None => (None, folder),
    };
    let name = if name.trim().is_empty() { model } else { name.trim() };
    Some(HeadphoneEntry {
        name: name.to_owned(),
        source: source.to_owned(),
        kind: kind.to_owned(),
        rig,
        path: path.clone(),
    })
}

/// Parses a `ParametricEQ.txt` (Equalizer APO format). Unsupported filter
/// types are skipped; at most [`MAX_BANDS`] filters are kept.
pub fn parse_parametric_eq(text: &str, model: &str, source: &str) -> Result<EqProfile> {
    let mut preamp = None;
    let mut bands = Vec::new();
    for line in text.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        let Some((head, rest)) = line.split_once(':') else { continue };
        let head = head.trim().to_ascii_lowercase();
        if head == "preamp" {
            preamp = rest.split_whitespace().find_map(number);
        } else if head.starts_with("filter") {
            if let Some(band) = parse_filter(rest) {
                bands.push(band);
            }
        }
    }
    if preamp.is_none() && bands.is_empty() {
        return Err(worded(
            "This AutoEq profile can't be read. Try another measurement.",
            "No se puede leer este perfil de AutoEq. Prueba con otra medición.",
        ));
    }
    bands.truncate(MAX_BANDS);
    Ok(EqProfile {
        name: model.to_owned(),
        preamp_db: preamp.unwrap_or(0.0),
        bands,
        source: EqSource::AutoEq { model: model.to_owned(), source: source.to_owned() },
    }
    .clamped())
}

/// `ON PK Fc 105 Hz Gain -2.1 dB Q 0.70`
fn parse_filter(spec: &str) -> Option<EqBand> {
    let tokens: Vec<&str> = spec.split_whitespace().collect();
    let enabled = tokens.first()?.eq_ignore_ascii_case("on");
    let kind = match tokens.get(1)?.to_ascii_uppercase().as_str() {
        "PK" | "PEQ" | "MODAL" => EqBandKind::Peak,
        "LSC" | "LS" | "LSQ" => EqBandKind::LowShelf,
        "HSC" | "HS" | "HSQ" => EqBandKind::HighShelf,
        "LP" | "LPQ" => EqBandKind::LowPass,
        "HP" | "HPQ" => EqBandKind::HighPass,
        other => {
            tracing::debug!("AutoEq filter type `{other}` is not supported, skipped");
            return None;
        }
    };
    let value = |key: &str| {
        tokens.iter().position(|t| t.eq_ignore_ascii_case(key)).and_then(|i| tokens.get(i + 1)).and_then(|t| number(t))
    };
    let freq = value("Fc")?;
    let default = EqBand::default();
    Some(EqBand {
        kind,
        freq_hz: freq,
        gain_db: value("Gain").unwrap_or(0.0),
        q: value("Q").unwrap_or(default.q),
        enabled,
    })
}

fn number(token: &str) -> Option<f32> {
    token.trim_end_matches(|c: char| c.is_ascii_alphabetic()).parse::<f32>().ok().filter(|v| v.is_finite())
}

/// Entries matching every word of `query`, best first (see [`AutoEq::search`]).
pub fn search_entries(entries: &[HeadphoneEntry], query: &str, limit: usize) -> Vec<HeadphoneEntry> {
    let query = normalize(query);
    let tokens: Vec<&str> = query.split_whitespace().collect();
    if tokens.is_empty() {
        return Vec::new();
    }
    let compact_query: String = tokens.concat();
    let mut scored: Vec<(i64, &HeadphoneEntry)> =
        entries.iter().filter_map(|e| score(e, &query, &compact_query, &tokens).map(|s| (s, e))).collect();
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.name.cmp(&b.1.name))
            .then_with(|| source_rank(&a.1.source).cmp(&source_rank(&b.1.source)))
    });
    scored.into_iter().take(limit).map(|(_, e)| e.clone()).collect()
}

fn score(e: &HeadphoneEntry, query: &str, compact_query: &str, tokens: &[&str]) -> Option<i64> {
    let name = normalize(&e.name);
    let compact: String = name.split_whitespace().collect();
    let words: Vec<&str> = name.split_whitespace().collect();
    let source = normalize(&e.source);
    let mut score = 0i64;
    for t in tokens {
        if words.iter().any(|w| w.starts_with(t)) {
            score += 30;
        } else if name.contains(t) || compact.contains(t) {
            score += 10;
        } else if source.contains(t) || e.rig.as_deref().is_some_and(|r| normalize(r).contains(t)) {
            score += 5;
        } else if t.len() >= 4 && words.iter().any(|w| near(w, t)) {
            score -= 40; // a typo
        } else {
            return None;
        }
    }
    if name == query || compact == compact_query {
        score += 1000;
    } else if compact.starts_with(compact_query) {
        score += 200;
    } else if compact.contains(compact_query) {
        score += 100;
    }
    // shorter names are closer to what was typed; known sources first
    score -= name.len() as i64 / 4;
    score += (PREFERRED_SOURCES.len() - source_rank(&e.source)) as i64 * 4;
    Some(score)
}

fn source_rank(source: &str) -> usize {
    PREFERRED_SOURCES.iter().position(|s| s.eq_ignore_ascii_case(source)).unwrap_or(PREFERRED_SOURCES.len())
}

/// Lowercase, without accents, words of letters and digits only.
fn normalize(s: &str) -> String {
    let mut plain = String::with_capacity(s.len());
    for c in s.nfd().filter(|c| !unicode_normalization::char::is_combining_mark(*c)).flat_map(char::to_lowercase) {
        match c {
            // letters that don't decompose
            'æ' => plain.push_str("ae"),
            'œ' => plain.push_str("oe"),
            'ø' => plain.push('o'),
            'ß' => plain.push_str("ss"),
            'ł' => plain.push('l'),
            'đ' | 'ð' => plain.push('d'),
            c if c.is_alphanumeric() => plain.push(c),
            _ => plain.push(' '),
        }
    }
    plain.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `true` when `word` (or its start) is one edit away from `token`.
fn near(word: &str, token: &str) -> bool {
    let w: Vec<char> = word.chars().collect();
    let t: Vec<char> = token.chars().collect();
    let prefix = &w[..w.len().min(t.len() + 1)];
    edit_distance(prefix, &t) <= 1 || edit_distance(&w[..w.len().min(t.len())], &t) <= 1
}

fn edit_distance(a: &[char], b: &[char]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut row = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            row[j + 1] = (prev[j] + usize::from(ca != cb)).min(prev[j + 1] + 1).min(row[j] + 1);
        }
        prev = row;
    }
    prev[b.len()]
}

/// `source/folder/model`, refusing anything that could leave `results/`.
fn path_segments(path: &str) -> Option<[&str; 3]> {
    let parts: Vec<&str> = path.split('/').collect();
    let ok = |s: &&str| !s.is_empty() && *s != "." && *s != ".." && !s.contains('\\');
    match parts.as_slice() {
        [a, b, c] if [a, b, c].iter().all(|s| ok(s)) => Some([a, b, c]),
        _ => None,
    }
}

fn profile_url(base: &str, segments: &[&str; 3]) -> Option<String> {
    let mut url = url::Url::parse(base).ok()?;
    url.path_segments_mut().ok()?.pop_if_empty().extend(segments).push(&format!("{}{PROFILE_SUFFIX}", segments[2]));
    Some(url.into())
}

/// File name of a cached profile: names may hold characters some systems refuse.
fn cache_name(path: &str) -> String {
    let hash = Sha256::digest(path.as_bytes());
    let hex: String = hash.iter().take(16).map(|b| format!("{b:02x}")).collect();
    format!("{hex}.txt")
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Thousands of lines: parsed on the blocking pool.
async fn parse_index_blocking(text: String) -> Vec<HeadphoneEntry> {
    tokio::task::spawn_blocking(move || parse_index(&text)).await.unwrap_or_default()
}

fn age(path: &Path) -> Option<Duration> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    SystemTime::now().duration_since(modified).ok()
}

async fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    // a unique temporary name: two windows saving the same profile don't collide
    let (path, text) = (path.to_path_buf(), text.to_owned());
    tokio::task::spawn_blocking(move || crate::paths::write_atomic(&path, text.as_bytes()))
        .await
        .map_err(std::io::Error::other)?
}

/// An error already in plain words, in the app's language.
fn worded(en: &str, es: &str) -> Error {
    Error::AutoEq(if language() == Lang::Es { es } else { en }.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) const HD650: &str = "Preamp: -6.1 dB\n\
        Filter 1: ON LSC Fc 105 Hz Gain 6.4 dB Q 0.70\n\
        Filter 2: ON PK Fc 8800 Hz Gain 5.1 dB Q 1.42\n\
        Filter 3: ON PK Fc 118 Hz Gain -3.1 dB Q 0.50\n\
        Filter 4: ON PK Fc 37 Hz Gain 0.7 dB Q 3.96\n\
        Filter 5: ON PK Fc 3169 Hz Gain -1.7 dB Q 3.89\n\
        Filter 6: ON HSC Fc 10000 Hz Gain -2.1 dB Q 0.70\n\
        Filter 7: ON PK Fc 1227 Hz Gain -1.2 dB Q 2.53\n\
        Filter 8: ON PK Fc 2055 Hz Gain 1.2 dB Q 3.23\n\
        Filter 9: ON PK Fc 587 Hz Gain 0.4 dB Q 1.19\n\
        Filter 10: ON PK Fc 5332 Hz Gain -1.1 dB Q 5.75\n";

    const INDEX: &str = "# Index\n\
        This is a list of all equalization profiles.\n\
        \n\
        - [1MORE Aero (ANC Off)](./HypetheSonics/GRAS%20RA0045%20in-ear/1MORE%20Aero%20(ANC%20Off)) by HypetheSonics on GRAS RA0045\n\
        - [Sennheiser HD 600](./crinacle/GRAS%2043AG-7%20over-ear/Sennheiser%20HD%20600) by crinacle on GRAS 43AG-7\n\
        - [Sennheiser HD 650](./Innerfidelity/over-ear/Sennheiser%20HD%20650) by Innerfidelity\n\
        - [Sennheiser HD 650](./oratory1990/over-ear/Sennheiser%20HD%20650) by oratory1990\n\
        - [Sennheiser HD 660 S](./oratory1990/over-ear/Sennheiser%20HD%20660%20S) by oratory1990\n\
        - [Apple AirPods Pro](./Rtings/HMS%20II.3%20in-ear/Apple%20AirPods%20Pro) by Rtings on HMS II.3\n\
        - [Dan Clark Audio Æon 2](./oratory1990/over-ear/Dan%20Clark%20Audio%20%C3%86on%202) by oratory1990\n\
        - [Shure SE215](./Rtings/Bruel%20&%20Kjaer%205128%20earbud/Shure%20SE215) by Rtings on Bruel & Kjaer 5128\n\
        not an entry\n";

    #[test]
    fn parses_parametric_eq() {
        let p = parse_parametric_eq(HD650, "Sennheiser HD 650", "oratory1990").unwrap();
        assert_eq!(p.name, "Sennheiser HD 650");
        assert_eq!(p.preamp_db, -6.1);
        assert_eq!(p.bands.len(), 10);
        assert_eq!(p.bands[0], EqBand::new(EqBandKind::LowShelf, 105.0, 6.4, 0.7));
        assert_eq!(p.bands[1], EqBand::new(EqBandKind::Peak, 8800.0, 5.1, 1.42));
        assert_eq!(p.bands[5].kind, EqBandKind::HighShelf);
        assert_eq!(p.source, EqSource::AutoEq { model: "Sennheiser HD 650".into(), source: "oratory1990".into() });
        // AutoEq's preamp already covers the boosts
        assert!(super::super::auto_preamp_db(&p) >= p.preamp_db - 0.3);
    }

    #[test]
    fn parser_is_lenient() {
        let text = "\u{feff}Preamp: -3 dB\r\n\
            Filter 1: OFF PK Fc 1000 Hz Gain 3 dB Q 1\r\n\
            Filter 2: ON NO Fc 50 Hz\r\n\
            Filter 3: ON HP Fc 20 Hz\r\n\
            Filter 4: ON PK Gain 3 dB\r\n";
        let p = parse_parametric_eq(text, "x", "y").unwrap();
        assert_eq!(p.preamp_db, -3.0);
        assert_eq!(p.bands.len(), 2);
        assert!(!p.bands[0].enabled);
        assert_eq!(p.bands[1].kind, EqBandKind::HighPass);
        assert!((p.bands[1].q - 0.707).abs() < 1e-3);
        assert!(parse_parametric_eq("<html>404</html>", "x", "y").is_err());
    }

    #[test]
    fn parses_the_index() {
        let list = parse_index(INDEX);
        assert_eq!(list.len(), 8);
        assert_eq!(
            list[0],
            HeadphoneEntry {
                name: "1MORE Aero (ANC Off)".into(),
                source: "HypetheSonics".into(),
                kind: "in-ear".into(),
                rig: Some("GRAS RA0045".into()),
                path: "HypetheSonics/GRAS RA0045 in-ear/1MORE Aero (ANC Off)".into(),
            }
        );
        assert_eq!(list[2].kind, "over-ear");
        assert_eq!(list[2].rig, None);
        assert_eq!(list[6].path, "oratory1990/over-ear/Dan Clark Audio Æon 2");
    }

    #[test]
    fn search_is_forgiving() {
        let list = parse_index(INDEX);
        let names = |q: &str| {
            search_entries(&list, q, 10).into_iter().map(|e| format!("{} / {}", e.name, e.source)).collect::<Vec<_>>()
        };
        // spacing, case, and the preferred source first
        assert_eq!(names("hd650"), ["Sennheiser HD 650 / oratory1990", "Sennheiser HD 650 / Innerfidelity"]);
        assert_eq!(names("HD 650")[0], "Sennheiser HD 650 / oratory1990");
        // words in any order, accents ignored
        assert_eq!(names("aeon clark"), ["Dan Clark Audio Æon 2 / oratory1990"]);
        assert_eq!(names("airpods pro")[0], "Apple AirPods Pro / Rtings");
        // a typo
        assert_eq!(names("senheiser 660")[0], "Sennheiser HD 660 S / oratory1990");
        // the source helps narrow
        assert_eq!(names("hd 650 innerfidelity"), ["Sennheiser HD 650 / Innerfidelity"]);
        assert!(names("zzz").is_empty());
        assert!(names("  ").is_empty());
        assert_eq!(search_entries(&list, "sennheiser", 2).len(), 2);
    }

    #[test]
    fn profile_urls_are_encoded() {
        let url = profile_url(DEFAULT_BASE_URL, &["crinacle", "GRAS 43AG-7 over-ear", "Sennheiser HD 600"]).unwrap();
        assert_eq!(
            url,
            "https://raw.githubusercontent.com/jaakkopasanen/AutoEq/master/results/crinacle/GRAS%2043AG-7%20over-ear/Sennheiser%20HD%20600/Sennheiser%20HD%20600%20ParametricEQ.txt"
        );
        assert!(path_segments("../../etc/passwd").is_none());
        assert!(path_segments("a/b").is_none());
        assert!(path_segments("a/../b").is_none());
        assert!(cache_name("a/b/c").ends_with(".txt") && !cache_name("a/b/c").contains('/'));
    }
}
