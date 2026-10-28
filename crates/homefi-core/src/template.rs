//! Path templates such as `{album.artist}/{album.title}/{item.number:02d}. {item.title}`.
//!
//! The syntax follows Python's `str.format` (which tiddl used), so existing
//! templates keep working: `{field}`, `{field:spec}`, `{{` / `}}` escapes.
//! See `docs/templating.md` for the available fields.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::LazyLock;

use chrono::{NaiveDate, NaiveDateTime};
use regex::Regex;

use crate::error::{Error, Result};
use crate::models::{Album, MediaItem, Playlist};

/// A value that can be referenced from a template.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Str(String),
    Date(NaiveDateTime),
    /// `E` / `explicit` / `clean` depending on the format spec.
    Explicit(Option<bool>),
    /// Renders its format spec when `true`, nothing otherwise
    /// (e.g. `{item.dolby:(Dolby Atmos)}`).
    Flag(bool),
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Str(s.to_owned())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Str(s)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int(v)
    }
}

fn min_date() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(1, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap()
}

/// Parses the date formats Tidal uses (`2001-03-13`,
/// `2019-05-14T19:51:12.000+0000`, RFC 3339, ...).
pub fn parse_datetime(s: &str) -> Option<NaiveDateTime> {
    let s = s.trim();
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return d.and_hms_opt(0, 0, 0);
    }
    for fmt in ["%Y-%m-%dT%H:%M:%S%.f%z", "%Y-%m-%dT%H:%M:%S%z", "%Y-%m-%d %H:%M:%S%.f%z"] {
        if let Ok(dt) = chrono::DateTime::parse_from_str(s, fmt) {
            return Some(dt.naive_local());
        }
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.naive_local());
    }
    for fmt in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S"] {
        if let Ok(dt) = NaiveDateTime::parse_from_str(s, fmt) {
            return Some(dt);
        }
    }
    None
}

#[derive(Debug, Clone)]
struct ItemFields {
    id: i64,
    title: String,
    title_version: String,
    number: i64,
    volume: i64,
    version: String,
    copyright: String,
    bpm: i64,
    isrc: String,
    artist: String,
    artists: String,
    features: String,
    artists_with_features: String,
    explicit: Option<bool>,
    dolby: bool,
}

#[derive(Debug, Clone)]
struct AlbumFields {
    id: i64,
    title: String,
    artist: String,
    artists: String,
    date: NaiveDateTime,
    explicit: Option<bool>,
    master: bool,
    release: String,
}

impl Default for AlbumFields {
    fn default() -> Self {
        Self {
            id: 0,
            title: String::new(),
            artist: String::new(),
            artists: String::new(),
            date: min_date(),
            explicit: None,
            master: false,
            release: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct PlaylistFields {
    uuid: String,
    title: String,
    index: i64,
    created: NaiveDateTime,
    updated: NaiveDateTime,
}

/// Values available to a template.
#[derive(Debug, Clone)]
pub struct TemplateData {
    quality: String,
    item: Option<ItemFields>,
    album: AlbumFields,
    playlist: Option<PlaylistFields>,
    extra: BTreeMap<String, Value>,
    now: NaiveDateTime,
}

impl TemplateData {
    /// `quality` is the upper-case preset (`HIGH`, `MAX`, `FHD`, ...).
    pub fn new(quality: impl Into<String>) -> Self {
        Self {
            quality: quality.into(),
            item: None,
            album: AlbumFields::default(),
            playlist: None,
            extra: BTreeMap::new(),
            now: chrono::Local::now().naive_local(),
        }
    }

    pub fn item(mut self, item: &MediaItem) -> Self {
        let mut main: Vec<String> = item.artists().iter().filter(|a| a.is_main()).map(|a| a.name.clone()).collect();
        let mut featured: Vec<String> =
            item.artists().iter().filter(|a| a.is_featured()).map(|a| a.name.clone()).collect();
        main.sort();
        featured.sort();

        let (version, copyright, bpm, isrc, dolby, explicit) = match item {
            MediaItem::Track(t) => (
                t.version.clone().unwrap_or_default(),
                t.copyright.clone().unwrap_or_default(),
                t.bpm.unwrap_or(0),
                t.isrc.clone().unwrap_or_default(),
                t.media_metadata.tags.iter().any(|t| t == "DOLBY_ATMOS"),
                t.explicit,
            ),
            MediaItem::Video(v) => (String::new(), String::new(), 0, String::new(), false, v.explicit),
        };

        let (title, number, volume) = match item {
            MediaItem::Track(t) => (t.title.clone(), t.track_number, t.volume_number),
            MediaItem::Video(v) => (v.title.clone(), v.track_number, v.volume_number),
        };

        self.item = Some(ItemFields {
            id: item.id(),
            title_version: if version.is_empty() { title.clone() } else { format!("{title} ({version})") },
            title,
            number,
            volume,
            version,
            copyright,
            bpm,
            isrc,
            artist: item.artist().map(|a| a.name.clone()).unwrap_or_default(),
            artists: main.join("; "),
            features: featured.join("; "),
            artists_with_features: main.iter().chain(&featured).cloned().collect::<Vec<_>>().join("; "),
            explicit: Some(explicit),
            dolby,
        });
        self
    }

    pub fn album(mut self, album: &Album) -> Self {
        self.album = AlbumFields {
            id: album.id,
            title: album.title.clone(),
            artist: album.artist.as_ref().map(|a| a.name.clone()).unwrap_or_default(),
            artists: album
                .artists
                .iter()
                .filter(|a| a.is_main())
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            date: album.release_date.as_deref().and_then(parse_datetime).unwrap_or_else(min_date),
            explicit: Some(album.explicit),
            master: album.media_metadata.tags.iter().any(|t| t == "HIRES_LOSSLESS") && self.quality == "MAX",
            release: album.kind.clone(),
        };
        self
    }

    pub fn album_opt(self, album: Option<&Album>) -> Self {
        match album {
            Some(a) => self.album(a),
            None => self,
        }
    }

    pub fn playlist(mut self, playlist: &Playlist, index: i64) -> Self {
        let date = |s: &Option<String>| s.as_deref().and_then(parse_datetime).unwrap_or_else(min_date);
        self.playlist = Some(PlaylistFields {
            uuid: playlist.uuid.clone(),
            title: playlist.title.clone(),
            index,
            created: date(&playlist.created),
            updated: date(&playlist.last_updated),
        });
        self
    }

    /// Adds a custom field such as `mix_id` or `type`.
    pub fn extra(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.extra.insert(key.into(), value.into());
        self
    }

    fn lookup(&self, field: &str) -> Result<Value> {
        let missing = || Error::Template(format!("unknown field `{field}`"));
        let (object, attr) = match field.split_once('.') {
            Some((o, a)) => (o, Some(a)),
            None => (field, None),
        };

        let value = match (object, attr) {
            ("now", None) => Value::Date(self.now),
            ("item", Some(attr)) => {
                let item =
                    self.item.as_ref().ok_or_else(|| Error::Template(format!("`{field}` is not available here")))?;
                match attr {
                    "id" => Value::Int(item.id),
                    "title" => item.title.as_str().into(),
                    "title_version" => item.title_version.as_str().into(),
                    "number" => Value::Int(item.number),
                    "volume" => Value::Int(item.volume),
                    "version" => item.version.as_str().into(),
                    "copyright" => item.copyright.as_str().into(),
                    "bpm" => Value::Int(item.bpm),
                    "isrc" => item.isrc.as_str().into(),
                    "quality" => self.quality.as_str().into(),
                    "artist" => item.artist.as_str().into(),
                    "artists" => item.artists.as_str().into(),
                    "features" => item.features.as_str().into(),
                    "artists_with_features" => item.artists_with_features.as_str().into(),
                    "explicit" => Value::Explicit(item.explicit),
                    "dolby" => Value::Flag(item.dolby),
                    _ => return Err(missing()),
                }
            }
            ("album", Some(attr)) => {
                let album = &self.album;
                match attr {
                    "id" => Value::Int(album.id),
                    "title" => album.title.as_str().into(),
                    "artist" => album.artist.as_str().into(),
                    "artists" => album.artists.as_str().into(),
                    "date" => Value::Date(album.date),
                    "explicit" => Value::Explicit(album.explicit),
                    "master" => Value::Flag(album.master),
                    "release" => album.release.as_str().into(),
                    _ => return Err(missing()),
                }
            }
            ("playlist", Some(attr)) => {
                let playlist = self
                    .playlist
                    .as_ref()
                    .ok_or_else(|| Error::Template(format!("`{field}` is not available here")))?;
                match attr {
                    "uuid" => playlist.uuid.as_str().into(),
                    "title" => playlist.title.as_str().into(),
                    "index" => Value::Int(playlist.index),
                    "created" => Value::Date(playlist.created),
                    "updated" => Value::Date(playlist.updated),
                    _ => return Err(missing()),
                }
            }
            (key, None) => self.extra.get(key).cloned().ok_or_else(missing)?,
            _ => return Err(missing()),
        };
        Ok(value)
    }
}

/// Python-style format spec: `[[fill]align][sign][#][0][width][,|_][.precision][type]`.
#[derive(Debug, Default)]
struct Spec {
    fill: Option<char>,
    align: Option<char>,
    sign: Option<char>,
    zero: bool,
    width: usize,
    grouping: Option<char>,
    precision: Option<usize>,
    kind: Option<char>,
}

fn parse_spec(spec: &str) -> Result<Spec> {
    let chars: Vec<char> = spec.chars().collect();
    let mut s = Spec::default();
    let mut i = 0;
    let is_align = |c: char| matches!(c, '<' | '>' | '^' | '=');

    if chars.len() >= 2 && is_align(chars[1]) {
        s.fill = Some(chars[0]);
        s.align = Some(chars[1]);
        i = 2;
    } else if !chars.is_empty() && is_align(chars[0]) {
        s.align = Some(chars[0]);
        i = 1;
    }
    if i < chars.len() && matches!(chars[i], '+' | '-' | ' ') {
        s.sign = Some(chars[i]);
        i += 1;
    }
    if i < chars.len() && chars[i] == '#' {
        i += 1;
    }
    if i < chars.len() && chars[i] == '0' {
        s.zero = true;
        i += 1;
    }
    let start = i;
    while i < chars.len() && chars[i].is_ascii_digit() {
        i += 1;
    }
    if i > start {
        s.width = chars[start..i].iter().collect::<String>().parse().unwrap_or(0);
    }
    if i < chars.len() && matches!(chars[i], ',' | '_') {
        s.grouping = Some(chars[i]);
        i += 1;
    }
    if i < chars.len() && chars[i] == '.' {
        i += 1;
        let start = i;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
        s.precision = Some(chars[start..i].iter().collect::<String>().parse().unwrap_or(0));
    }
    if i < chars.len() {
        s.kind = Some(chars[i]);
        i += 1;
    }
    if i != chars.len() {
        return Err(Error::Template(format!("invalid format specifier `{spec}`")));
    }
    Ok(s)
}

fn pad(body: String, spec: &Spec, default_align: char) -> String {
    let len = body.chars().count();
    if len >= spec.width {
        return body;
    }
    let fill_n = spec.width - len;
    let fill = spec.fill.unwrap_or(' ');
    let fill_str = |n: usize| std::iter::repeat_n(fill, n).collect::<String>();
    match spec.align.unwrap_or(default_align) {
        '<' => body + &fill_str(fill_n),
        '^' => fill_str(fill_n / 2) + &body + &fill_str(fill_n - fill_n / 2),
        '=' => {
            // padding goes after the sign
            let (sign, digits) = match body.chars().next() {
                Some(c @ ('-' | '+' | ' ')) => (c.to_string(), body[1..].to_owned()),
                _ => (String::new(), body),
            };
            sign + &fill_str(fill_n) + &digits
        }
        _ => fill_str(fill_n) + &body,
    }
}

fn group_digits(digits: &str, sep: char) -> String {
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(sep);
        }
        out.push(c);
    }
    out
}

fn format_int(v: i64, spec: &str) -> Result<String> {
    let mut s = parse_spec(spec)?;
    let abs = v.unsigned_abs();
    let digits = match s.kind {
        None | Some('d') | Some('n') => abs.to_string(),
        Some('x') => format!("{abs:x}"),
        Some('X') => format!("{abs:X}"),
        Some('o') => format!("{abs:o}"),
        Some('b') => format!("{abs:b}"),
        Some(k) => return Err(Error::Template(format!("unknown format code `{k}` for an integer"))),
    };
    let digits = match s.grouping {
        Some(sep) => group_digits(&digits, sep),
        None => digits,
    };
    let sign = match (v < 0, s.sign) {
        (true, _) => "-",
        (false, Some('+')) => "+",
        (false, Some(' ')) => " ",
        _ => "",
    };
    if s.zero && s.align.is_none() {
        s.fill = Some('0');
        s.align = Some('=');
    }
    Ok(pad(format!("{sign}{digits}"), &s, '>'))
}

fn format_str(v: &str, spec: &str) -> Result<String> {
    let s = parse_spec(spec)?;
    if !matches!(s.kind, None | Some('s')) {
        return Err(Error::Template(format!("unknown format code `{}` for a string", s.kind.unwrap())));
    }
    let body: String = match s.precision {
        Some(p) => v.chars().take(p).collect(),
        None => v.to_owned(),
    };
    Ok(pad(body, &s, '<'))
}

fn format_date(v: &NaiveDateTime, spec: &str) -> Result<String> {
    let fmt = if spec.is_empty() { "%Y-%m-%d %H:%M:%S" } else { spec };
    let mut out = String::new();
    write!(out, "{}", v.format(fmt)).map_err(|_| Error::Template(format!("invalid date format `{spec}`")))?;
    Ok(out)
}

fn format_explicit(v: Option<bool>, spec: &str) -> String {
    let Some(value) = v else { return String::new() };
    let features: Vec<&str> = spec.split("; ").collect();

    let base = if features.contains(&"long") {
        if value {
            "explicit"
        } else {
            ""
        }
    } else if features.contains(&"full") {
        if value {
            "explicit"
        } else {
            "clean"
        }
    } else if value {
        "E"
    } else {
        ""
    };

    if features.contains(&"upper") {
        base.to_uppercase()
    } else {
        base.to_owned()
    }
}

fn format_value(value: &Value, spec: &str) -> Result<String> {
    match value {
        Value::Int(v) => format_int(*v, spec),
        Value::Str(v) => format_str(v, spec),
        Value::Date(v) => format_date(v, spec),
        Value::Explicit(v) => Ok(format_explicit(*v, spec)),
        Value::Flag(v) => Ok(if *v { spec.to_owned() } else { String::new() }),
    }
}

/// Formats one template string without sanitizing it.
pub fn render(template: &str, data: &TemplateData) -> Result<String> {
    let mut out = String::new();
    let mut chars = template.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push('}');
            }
            '}' => return Err(Error::Template(format!("single `}}` in `{template}`"))),
            '{' => {
                let mut field = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(c) => field.push(c),
                        None => return Err(Error::Template(format!("unclosed `{{` in `{template}`"))),
                    }
                }
                let (name, spec) = field.split_once(':').unwrap_or((&field, ""));
                // `!r` / `!s` conversions are accepted and ignored
                let name = name.split('!').next().unwrap_or_default().trim();
                let value = data.lookup(name)?;
                out.push_str(&format_value(&value, spec)?);
            }
            c => out.push(c),
        }
    }
    Ok(out)
}

/// Removes characters that are invalid in file names.
pub fn sanitize(text: &str) -> String {
    static INVALID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"[\\/:"*?<>|]+"#).unwrap());
    INVALID.replace_all(text, "").into_owned()
}

/// Cleans one path segment so it is valid on Windows / NAS file systems.
pub fn clean_segment(text: &str) -> String {
    static DOTS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\.{2,}").unwrap());
    static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s{2,}").unwrap());

    let text = sanitize(text);
    let text = DOTS.replace_all(&text, ".");
    let text = text.trim_end_matches([' ', '.']);
    let text = SPACES.replace_all(text, " ");
    let text = text.trim();

    if text.is_empty() {
        "_".to_owned()
    } else {
        text.to_owned()
    }
}

/// Formats a path template. Every `/`-separated segment is rendered and
/// sanitized separately. With `asterisk_ext`, `.*` is appended so the
/// extension can be filled in once the stream format is known.
pub fn format_path(template: &str, data: &TemplateData, asterisk_ext: bool) -> Result<String> {
    let segments = template
        .split('/')
        .map(|segment| render(segment, data).map(|s| clean_segment(&s)))
        .collect::<Result<Vec<_>>>()?;

    let mut path = segments.join("/");
    if asterisk_ext {
        path.push_str(".*");
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ArtistRef, MediaMetadata, Track};

    fn artist(name: &str, kind: &str) -> ArtistRef {
        ArtistRef { id: 1, name: name.into(), kind: Some(kind.into()), picture: None }
    }

    fn track() -> MediaItem {
        let json = serde_json::json!({
            "id": 123, "title": "Harder Better Faster Stronger", "trackNumber": 4, "volumeNumber": 1,
            "version": "Remastered", "isrc": "GBDUW0000059", "explicit": true, "bpm": 123,
            "audioQuality": "LOSSLESS", "album": {"id": 9, "title": "Discovery"}
        });
        let mut t: Track = serde_json::from_value(json).unwrap();
        t.artist = Some(artist("Daft Punk", "MAIN"));
        t.artists = vec![artist("Daft Punk", "MAIN"), artist("Romanthony", "FEATURED")];
        t.media_metadata = MediaMetadata { tags: vec!["DOLBY_ATMOS".into()] };
        MediaItem::Track(t)
    }

    fn album() -> Album {
        let mut a: Album = serde_json::from_value(serde_json::json!({
            "id": 9, "title": "Discovery", "releaseDate": "2001-03-13", "type": "ALBUM",
            "mediaMetadata": {"tags": ["LOSSLESS", "HIRES_LOSSLESS"]}
        }))
        .unwrap();
        a.artist = Some(artist("Daft Punk", "MAIN"));
        a.artists = vec![artist("Daft Punk", "MAIN")];
        a
    }

    #[test]
    fn default_template() {
        let data = TemplateData::new("HIGH").item(&track()).album(&album());
        let path = format_path(crate::config::DEFAULT_TEMPLATE, &data, true).unwrap();
        assert_eq!(path, "Daft Punk/Discovery/Harder Better Faster Stronger.*");
    }

    #[test]
    fn format_specs() {
        let data = TemplateData::new("MAX").item(&track()).album(&album());
        let r = |t: &str| render(t, &data).unwrap();
        assert_eq!(r("{item.number:02d}"), "04");
        assert_eq!(r("{item.volume:03d}"), "001");
        assert_eq!(r("{item.number:>4}"), "   4");
        assert_eq!(r("{item.title:.6}"), "Harder");
        assert_eq!(r("{album.date:%Y}"), "2001");
        assert_eq!(r("{album.date}"), "2001-03-13 00:00:00");
        assert_eq!(r("{item.explicit}"), "E");
        assert_eq!(r("{item.explicit:long}"), "explicit");
        assert_eq!(r("{album.explicit:full}"), "clean");
        assert_eq!(r("{item.explicit:full; upper}"), "EXPLICIT");
        assert_eq!(r("{item.dolby:(Dolby Atmos)}"), "(Dolby Atmos)");
        assert_eq!(r("{album.master:[MAX]}"), "[MAX]");
        assert_eq!(r("{item.title_version}"), "Harder Better Faster Stronger (Remastered)");
        assert_eq!(r("{item.artists_with_features}"), "Daft Punk; Romanthony");
        assert_eq!(r("{item.quality}"), "MAX");
        assert_eq!(r("{{literal}}"), "{literal}");
    }

    #[test]
    fn errors() {
        let data = TemplateData::new("HIGH").album(&album());
        assert!(render("{item.title}", &data).is_err());
        assert!(render("{playlist.title}", &data).is_err());
        assert!(render("{album.nope}", &data).is_err());
        assert!(render("{album.title", &data).is_err());
        assert!(render("{album.id:q}", &data).is_err());
    }

    #[test]
    fn extra_fields_and_sanitizing() {
        let data = TemplateData::new("HIGH").album(&album()).extra("type", "album").extra("mix_id", "abc");
        assert_eq!(format_path("m3u/{type}/{mix_id}", &data, false).unwrap(), "m3u/album/abc");
        assert_eq!(clean_segment("AC/DC: Live?.. "), "ACDC Live");
        assert_eq!(clean_segment("Vol.. 2"), "Vol. 2");
        assert_eq!(clean_segment(" ... "), "_");
        assert_eq!(clean_segment("a   b"), "a b");
    }

    #[test]
    fn tidal_dates() {
        let d = parse_datetime("2019-05-14T19:51:12.000+0000").unwrap();
        assert_eq!(d.format("%Y-%m-%d %H:%M").to_string(), "2019-05-14 19:51");
        assert!(parse_datetime("2001-03-13").is_some());
        assert!(parse_datetime("2020-01-01T00:00:00Z").is_some());
        assert!(parse_datetime("nope").is_none());
    }
}
