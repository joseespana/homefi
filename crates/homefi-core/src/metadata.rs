//! Audio / video tags (FLAC, M4A, MP3, MP4) and cover art.

use std::path::{Path, PathBuf};

use lofty::config::WriteOptions;
use lofty::file::TaggedFileExt;
use lofty::id3::v2::Id3v2Tag;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::*;
use lofty::tag::{ItemKey, ItemValue, Tag, TagItem, TagType};
use unicode_normalization::UnicodeNormalization;

use crate::error::{Error, Result};
use crate::models::{CreditsEntry, Track, Video};
use crate::paths::with_suffix;
use crate::template::parse_datetime;

/// Tags written to a track file.
#[derive(Debug, Clone, Default)]
pub struct TrackTags {
    pub title: String,
    pub track_number: i64,
    pub disc_number: i64,
    pub copyright: Option<String>,
    pub album_artist: String,
    pub artists: Vec<String>,
    pub album_title: String,
    /// Release date as returned by Tidal (`2001-03-13`).
    pub date: Option<String>,
    pub isrc: Option<String>,
    pub bpm: Option<i64>,
    pub lyrics: Option<String>,
    pub credits: Vec<CreditsEntry>,
    pub cover: Option<Vec<u8>>,
    pub comment: String,
    pub genre: String,
    /// Written as `REPLAYGAIN_*` tags (Tidal's values), for loudness normalization.
    pub replay_gain: Option<crate::dsp::ReplayGain>,
}

impl TrackTags {
    pub fn from_track(track: &Track) -> Self {
        let mut artists: Vec<String> = track.artists.iter().map(|a| a.name.trim().to_owned()).collect();
        artists.sort();
        Self {
            title: match &track.version {
                Some(v) if !v.is_empty() => format!("{} ({v})", track.title),
                _ => track.title.clone(),
            },
            track_number: track.track_number,
            disc_number: track.volume_number,
            copyright: track.copyright.clone(),
            artists,
            album_title: track.album.title.clone().unwrap_or_default(),
            isrc: track.isrc.clone(),
            bpm: track.bpm.filter(|b| *b > 0),
            replay_gain: Some(crate::dsp::ReplayGain::from_track(track)).filter(|r| !r.is_empty()),
            ..Default::default()
        }
    }
}

/// Sorts contributors by surname and turns credit types into ASCII tag keys
/// (`Producer` -> `PRODUCER`, `Ingénieur` -> `INGENIEUR`).
pub fn normalize_credits(entries: &mut Vec<CreditsEntry>) {
    for entry in entries.iter_mut() {
        entry.contributors.sort_by_key(|c| c.name.split_whitespace().last().unwrap_or_default().to_lowercase());
        entry.kind = entry
            .kind
            .to_uppercase()
            .nfkd()
            .filter(|c| c.is_ascii() && *c != '=')
            .collect::<String>()
            .trim()
            .to_owned();
    }
    entries.retain(|e| !e.kind.is_empty());
}

fn meta_err(path: &Path, e: impl std::fmt::Display) -> Error {
    Error::Metadata { path: path.to_path_buf(), message: e.to_string() }
}

fn cover_picture(data: &[u8]) -> Picture {
    Picture::new_unchecked(PictureType::CoverFront, Some(MimeType::Jpeg), Some("Cover".into()), data.to_vec())
}

/// Fills the fields shared by every format into a generic tag.
fn fill_common(tag: &mut Tag, tags: &TrackTags, joined_artists: bool) {
    tag.set_title(tags.title.clone());
    tag.set_album(tags.album_title.clone());
    if tags.track_number > 0 {
        tag.set_track(tags.track_number as u32);
    }
    if tags.disc_number > 0 {
        tag.set_disk(tags.disc_number as u32);
    }
    if !tags.album_artist.is_empty() {
        tag.insert_text(ItemKey::AlbumArtist, tags.album_artist.clone());
    }

    if joined_artists || tags.artists.len() <= 1 {
        tag.set_artist(tags.artists.join("; "));
    } else {
        for artist in &tags.artists {
            tag.push(TagItem::new(ItemKey::TrackArtist, ItemValue::Text(artist.clone())));
        }
    }

    if let Some(date) = tags.date.as_deref().and_then(parse_datetime) {
        tag.insert_text(ItemKey::RecordingDate, date.format("%Y-%m-%d").to_string());
        tag.insert_text(ItemKey::Year, date.format("%Y").to_string());
    }
    if let Some(c) = tags.copyright.as_deref().filter(|c| !c.is_empty()) {
        tag.insert_text(ItemKey::CopyrightMessage, c.to_owned());
    }
    if let Some(isrc) = tags.isrc.as_deref().filter(|i| !i.is_empty()) {
        tag.insert_text(ItemKey::Isrc, isrc.to_owned());
    }
    if !tags.genre.is_empty() {
        tag.set_genre(tags.genre.clone());
    }
    if !tags.comment.is_empty() {
        tag.set_comment(tags.comment.clone());
    }
    if let Some(bpm) = tags.bpm {
        tag.insert_text(ItemKey::IntegerBpm, bpm.to_string());
    }
    if let Some(lyrics) = tags.lyrics.as_deref().filter(|l| !l.is_empty()) {
        tag.insert_text(ItemKey::Lyrics, lyrics.to_owned());
    }
    for (key, value) in replay_gain_items(tags) {
        tag.insert_text(key, value);
    }
    if let Some(cover) = &tags.cover {
        tag.push_picture(cover_picture(cover));
    }
}

/// `REPLAYGAIN_*` values of `tags` that are known.
fn replay_gain_items(tags: &TrackTags) -> Vec<(ItemKey, String)> {
    use crate::dsp::loudness::{format_gain, format_peak};
    let Some(rg) = tags.replay_gain else { return Vec::new() };
    [
        (ItemKey::ReplayGainTrackGain, rg.track_gain_db.map(format_gain)),
        (ItemKey::ReplayGainTrackPeak, rg.track_peak.map(format_peak)),
        (ItemKey::ReplayGainAlbumGain, rg.album_gain_db.map(format_gain)),
        (ItemKey::ReplayGainAlbumPeak, rg.album_peak.map(format_peak)),
    ]
    .into_iter()
    .filter_map(|(k, v)| v.map(|v| (k, v)))
    .collect()
}

fn write_m4a(path: &Path, tags: &TrackTags) -> Result<()> {
    let mut file = lofty::read_from_path(path).map_err(|e| meta_err(path, e))?;
    file.remove(TagType::Mp4Ilst);

    let mut tag = Tag::new(TagType::Mp4Ilst);
    fill_common(&mut tag, tags, true);

    file.insert_tag(tag);
    file.save_to_path(path, WriteOptions::default()).map_err(|e| meta_err(path, e))
}

/// FLAC goes through [`crate::flac_tags`], which keeps the audio frames
/// byte-identical (lofty's writer damaged frames when adding padding).
fn write_flac(path: &Path, tags: &TrackTags) -> Result<()> {
    let mut c: Vec<(String, String)> =
        vec![("TITLE".into(), tags.title.clone()), ("ALBUM".into(), tags.album_title.clone())];
    c.extend(tags.artists.iter().map(|a| ("ARTIST".to_owned(), a.clone())));
    c.push(("ALBUMARTIST".into(), tags.album_artist.clone()));
    if tags.track_number > 0 {
        c.push(("TRACKNUMBER".into(), tags.track_number.to_string()));
    }
    if tags.disc_number > 0 {
        c.push(("DISCNUMBER".into(), tags.disc_number.to_string()));
    }
    if let Some(date) = tags.date.as_deref().and_then(parse_datetime) {
        c.push(("DATE".into(), date.format("%Y-%m-%d").to_string()));
        c.push(("YEAR".into(), date.format("%Y").to_string()));
    }
    c.push(("COPYRIGHT".into(), tags.copyright.clone().unwrap_or_default()));
    c.push(("ISRC".into(), tags.isrc.clone().unwrap_or_default()));
    c.push(("GENRE".into(), tags.genre.clone()));
    c.push(("COMMENT".into(), tags.comment.clone()));
    if let Some(bpm) = tags.bpm {
        c.push(("BPM".into(), bpm.to_string()));
    }
    c.push(("LYRICS".into(), tags.lyrics.clone().unwrap_or_default()));
    for entry in &tags.credits {
        c.extend(entry.contributors.iter().map(|p| (entry.kind.clone(), p.name.clone())));
    }
    for (key, value) in replay_gain_items(tags) {
        let name = match key {
            ItemKey::ReplayGainTrackGain => "REPLAYGAIN_TRACK_GAIN",
            ItemKey::ReplayGainTrackPeak => "REPLAYGAIN_TRACK_PEAK",
            ItemKey::ReplayGainAlbumGain => "REPLAYGAIN_ALBUM_GAIN",
            _ => "REPLAYGAIN_ALBUM_PEAK",
        };
        c.push((name.into(), value));
    }
    crate::flac_tags::write(path, &c, tags.cover.as_deref())
}

fn write_mp3(path: &Path, tags: &TrackTags) -> Result<()> {
    let mut generic = Tag::new(TagType::Id3v2);
    fill_common(&mut generic, tags, true);

    let mut id3: Id3v2Tag = generic.into();
    for entry in &tags.credits {
        let names: Vec<&str> = entry.contributors.iter().map(|c| c.name.as_str()).collect();
        id3.insert_user_text(entry.kind.clone(), names.join("; "));
    }

    // ID3v2.3 has the widest player support (Windows Explorer, car stereos, ...)
    id3.save_to_path(path, WriteOptions::default().use_id3v23(true)).map_err(|e| meta_err(path, e))
}

/// Writes track tags based on the file extension.
pub fn write_track_tags(path: &Path, tags: &TrackTags) -> Result<()> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "flac" => write_flac(path, tags),
        "m4a" | "mp4" => write_m4a(path, tags),
        "mp3" => write_mp3(path, tags),
        other => Err(meta_err(path, format!("unsupported file extension `{other}`"))),
    }
}

/// Embeds `cover` as the front cover of a FLAC / M4A / MP3, keeping its tags.
pub fn embed_cover(path: &Path, cover: &[u8]) -> Result<()> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if ext == "flac" {
        return crate::flac_tags::set_cover(path, cover);
    }
    let mut file = lofty::read_from_path(path).map_err(|e| meta_err(path, e))?;
    let tag_type = file.primary_tag_type();
    if file.primary_tag().is_none() {
        file.insert_tag(Tag::new(tag_type));
    }
    let tag = file.primary_tag_mut().expect("tag inserted");
    tag.remove_picture_type(PictureType::CoverFront);
    tag.push_picture(cover_picture(cover));
    let opts = if ext == "mp3" { WriteOptions::default().use_id3v23(true) } else { WriteOptions::default() };
    file.save_to_path(path, opts).map_err(|e| meta_err(path, e))
}

/// Writes basic tags to a downloaded `.mp4` video.
pub fn write_video_tags(path: &Path, video: &Video) -> Result<()> {
    let mut file = lofty::read_from_path(path).map_err(|e| meta_err(path, e))?;
    file.remove(TagType::Mp4Ilst);

    let mut tag = Tag::new(TagType::Mp4Ilst);
    tag.set_title(video.title.clone());
    tag.set_artist(video.artists.iter().map(|a| a.name.trim()).collect::<Vec<_>>().join("; "));
    if let Some(artist) = &video.artist {
        tag.insert_text(ItemKey::AlbumArtist, artist.name.clone());
    }
    if let Some(title) = video.album.as_ref().and_then(|a| a.title.clone()) {
        tag.set_album(title);
    }
    if let Some(date) = video.stream_start_date.as_deref().and_then(parse_datetime) {
        tag.insert_text(ItemKey::RecordingDate, date.format("%Y-%m-%d").to_string());
    }
    if video.track_number > 0 {
        tag.set_track(video.track_number as u32);
    }
    if video.volume_number > 0 {
        tag.set_disk(video.volume_number as u32);
    }

    file.insert_tag(tag);
    file.save_to_path(path, WriteOptions::default()).map_err(|e| meta_err(path, e))
}

/// Tidal cover art.
#[derive(Debug, Clone)]
pub struct Cover {
    pub url: String,
}

impl Cover {
    pub const MAX_SIZE: u32 = 1280;

    /// `uid` is the cover id from the API (`ab12-...`).
    pub fn new(uid: &str, size: u32) -> Self {
        let size = if size > Self::MAX_SIZE {
            tracing::warn!("cover size can not be higher than {} (got {size})", Self::MAX_SIZE);
            Self::MAX_SIZE
        } else {
            size
        };
        Self { url: format!("https://resources.tidal.com/images/{}/{size}x{size}.jpg", uid.replace('-', "/")) }
    }

    pub async fn fetch(&self, http: &reqwest::Client) -> Option<Vec<u8>> {
        match http.get(&self.url).send().await.and_then(|r| r.error_for_status()) {
            Ok(res) => res.bytes().await.ok().map(|b| b.to_vec()).filter(|b| !b.is_empty()),
            Err(e) => {
                tracing::error!("could not download cover {}: {e}", self.url);
                None
            }
        }
    }

    /// Saves the cover as `<path>.jpg` unless it already exists.
    pub async fn save(&self, http: &reqwest::Client, data: Option<&[u8]>, path: &Path) -> Result<Option<PathBuf>> {
        let file = with_suffix(path, "jpg");
        if file.exists() {
            return Ok(None);
        }
        let fetched;
        let data = match data {
            Some(d) => d,
            None => match self.fetch(http).await {
                Some(d) => {
                    fetched = d;
                    &fetched
                }
                None => return Ok(None),
            },
        };
        if let Some(parent) = file.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&file, data).await?;
        Ok(Some(file))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Contributor;

    #[test]
    fn credits_are_normalized() {
        let mut credits = vec![
            CreditsEntry {
                kind: "Ingénieur".into(),
                contributors: vec![
                    Contributor { name: "Zed Alpha".into(), id: None },
                    Contributor { name: "Ann Beta".into(), id: None },
                ],
            },
            CreditsEntry { kind: "=".into(), contributors: vec![] },
        ];
        normalize_credits(&mut credits);
        assert_eq!(credits.len(), 1);
        assert_eq!(credits[0].kind, "INGENIEUR");
        assert_eq!(credits[0].contributors[0].name, "Zed Alpha");
    }

    #[test]
    fn cover_url() {
        let cover = Cover::new("ab12-cd34", 4000);
        assert_eq!(cover.url, "https://resources.tidal.com/images/ab12/cd34/1280x1280.jpg");
    }

    #[test]
    fn unsupported_extension() {
        assert!(write_track_tags(Path::new("song.wav"), &TrackTags::default()).is_err());
    }
}
