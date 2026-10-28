//! Turns Tidal playback manifests into downloadable segment URLs.

use base64::Engine;
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::models::{TrackStream, VideoStream};

const DOLBY_CODECS: [&str; 2] = ["eac3", "ac4"];
const DASH_NS: &str = "urn:mpeg:dash:schema:mpd:2011";

/// Segment URLs of a track and the extension of the downloaded data.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackSource {
    pub urls: Vec<String>,
    pub codecs: String,
    /// `flac` or `m4a`.
    pub extension: &'static str,
}

fn decode_manifest(manifest: &str) -> Result<String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(manifest.trim())
        .map_err(|e| Error::Stream(format!("manifest is not base64: {e}")))?;
    String::from_utf8(bytes).map_err(|e| Error::Stream(format!("manifest is not utf-8: {e}")))
}

/// Parses an MPEG-DASH manifest into segment URLs (`$Number$` = 0 is the
/// initialization segment) and the codecs string.
pub fn parse_dash_manifest(xml: &str) -> Result<(Vec<String>, String)> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| Error::Stream(format!("invalid DASH xml: {e}")))?;
    fn child<'a, 'i>(node: roxmltree::Node<'a, 'i>, name: &str) -> Option<roxmltree::Node<'a, 'i>> {
        node.children()
            .find(|n| n.is_element() && n.tag_name().name() == name && n.tag_name().namespace() == Some(DASH_NS))
    }
    let missing = |what: &str| Error::Stream(format!("{what} element not found in DASH manifest"));

    let period = child(doc.root_element(), "Period").ok_or_else(|| missing("Period"))?;
    let set = child(period, "AdaptationSet").ok_or_else(|| missing("AdaptationSet"))?;
    let repr = child(set, "Representation").ok_or_else(|| missing("Representation"))?;
    let codecs = repr.attribute("codecs").unwrap_or_default().to_owned();

    let template = child(repr, "SegmentTemplate").ok_or_else(|| missing("SegmentTemplate"))?;
    let media = template.attribute("media").ok_or_else(|| Error::Stream("no `media` in SegmentTemplate".into()))?;
    let timeline = child(template, "SegmentTimeline").ok_or_else(|| missing("SegmentTimeline"))?;

    let mut total: u64 = 0;
    let mut found = false;
    for s in timeline.children().filter(|n| n.is_element() && n.tag_name().name() == "S") {
        found = true;
        total += 1;
        if let Some(r) = s.attribute("r") {
            total += r.parse::<u64>().map_err(|_| Error::Stream(format!("invalid repeat count `{r}`")))?;
        }
    }
    if !found {
        return Err(missing("SegmentTimeline/S"));
    }

    let urls = (0..=total).map(|i| media.replace("$Number$", &i.to_string())).collect();
    Ok((urls, codecs))
}

/// Parses the URLs and file extension of a track stream.
///
/// | Quality         | Manifest                  | Data       |
/// | --------------- | ------------------------- | ---------- |
/// | LOW / HIGH      | application/vnd.tidal.bts | AAC (m4a)  |
/// | LOSSLESS        | application/vnd.tidal.bts | FLAC       |
/// | HI_RES_LOSSLESS | application/dash+xml      | FLAC in MP4|
pub fn parse_track_stream(stream: &TrackStream) -> Result<TrackSource> {
    #[derive(Deserialize)]
    struct BtsManifest {
        codecs: String,
        urls: Vec<String>,
    }

    let manifest = decode_manifest(&stream.manifest)?;
    let (urls, codecs) = match stream.manifest_mime_type.as_str() {
        "application/vnd.tidal.bts" => {
            let m: BtsManifest = serde_json::from_str(&manifest)?;
            (m.urls, m.codecs)
        }
        "application/dash+xml" => parse_dash_manifest(&manifest)?,
        other => return Err(Error::Stream(format!("unsupported manifest type `{other}`"))),
    };

    let extension = if codecs == "flac" {
        if stream.audio_quality == "HI_RES_LOSSLESS" {
            "m4a"
        } else {
            "flac"
        }
    } else if codecs.starts_with("mp4") || DOLBY_CODECS.contains(&codecs.as_str()) {
        "m4a"
    } else {
        return Err(Error::Stream(format!("unknown codecs `{codecs}` (track {})", stream.track_id)));
    };

    Ok(TrackSource { urls, codecs, extension })
}

fn resolve(base: &str, uri: &str) -> String {
    url::Url::parse(base).and_then(|b| b.join(uri)).map(|u| u.to_string()).unwrap_or_else(|_| uri.to_owned())
}

/// Last (highest quality) variant URI of an HLS master playlist.
pub fn hls_best_variant(master: &str, base: &str) -> Option<String> {
    let mut best = None;
    let mut lines = master.lines().map(str::trim);
    while let Some(line) = lines.next() {
        if line.starts_with("#EXT-X-STREAM-INF") {
            if let Some(uri) = lines.by_ref().find(|l| !l.is_empty() && !l.starts_with('#')) {
                best = Some(resolve(base, uri));
            }
        }
    }
    best
}

/// Segment URIs of an HLS media playlist.
pub fn hls_segments(playlist: &str, base: &str) -> Vec<String> {
    playlist
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|uri| resolve(base, uri))
        .collect()
}

/// Resolves the segment URLs of the best quality rendition of a video.
pub async fn video_segments(http: &reqwest::Client, stream: &VideoStream) -> Result<Vec<String>> {
    #[derive(Deserialize)]
    struct VideoManifest {
        urls: Vec<String>,
    }

    let manifest: VideoManifest = serde_json::from_str(&decode_manifest(&stream.manifest)?)?;
    let master_url = manifest.urls.first().ok_or_else(|| Error::Stream("video manifest has no urls".into()))?;

    let master = http.get(master_url).send().await?.error_for_status()?.text().await?;
    let variant = hls_best_variant(&master, master_url)
        .ok_or_else(|| Error::Stream("M3U8 master playlist has no variants".into()))?;

    let playlist = http.get(&variant).send().await?.error_for_status()?.text().await?;
    let segments = hls_segments(&playlist, &variant);
    if segments.is_empty() {
        return Err(Error::Stream("M3U8 playlist is empty".into()));
    }
    Ok(segments)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b64(s: &str) -> String {
        base64::engine::general_purpose::STANDARD.encode(s)
    }

    fn stream(mime: &str, quality: &str, manifest: &str) -> TrackStream {
        TrackStream {
            track_id: 1,
            audio_mode: "STEREO".into(),
            audio_quality: quality.into(),
            manifest_mime_type: mime.into(),
            manifest: b64(manifest),
            bit_depth: None,
            sample_rate: None,
        }
    }

    #[test]
    fn bts_manifest() {
        let m = r#"{"mimeType":"audio/flac","codecs":"flac","encryptionType":"NONE","urls":["https://a/1"]}"#;
        let src = parse_track_stream(&stream("application/vnd.tidal.bts", "LOSSLESS", m)).unwrap();
        assert_eq!(src.urls, vec!["https://a/1"]);
        assert_eq!(src.extension, "flac");

        let m = r#"{"mimeType":"audio/mp4","codecs":"mp4a.40.2","encryptionType":"NONE","urls":["u"]}"#;
        assert_eq!(parse_track_stream(&stream("application/vnd.tidal.bts", "HIGH", m)).unwrap().extension, "m4a");
    }

    #[test]
    fn dash_manifest() {
        let xml = r#"<?xml version="1.0"?>
<MPD xmlns="urn:mpeg:dash:schema:mpd:2011">
  <Period><AdaptationSet><Representation codecs="flac">
    <SegmentTemplate media="https://x/$Number$.mp4" initialization="https://x/0.mp4">
      <SegmentTimeline><S d="1" r="2"/><S d="1"/></SegmentTimeline>
    </SegmentTemplate>
  </Representation></AdaptationSet></Period>
</MPD>"#;
        let src = parse_track_stream(&stream("application/dash+xml", "HI_RES_LOSSLESS", xml)).unwrap();
        assert_eq!(src.urls.len(), 5);
        assert_eq!(src.urls[0], "https://x/0.mp4");
        assert_eq!(src.urls[4], "https://x/4.mp4");
        assert_eq!(src.extension, "m4a");
    }

    #[test]
    fn unknown_codecs_fail() {
        let m = r#"{"codecs":"opus","urls":[]}"#;
        assert!(parse_track_stream(&stream("application/vnd.tidal.bts", "HIGH", m)).is_err());
        assert!(parse_track_stream(&stream("text/plain", "HIGH", m)).is_err());
    }

    #[test]
    fn hls_playlists() {
        let master =
            "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\nlow.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=2\nhttps://cdn/high.m3u8\n";
        assert_eq!(hls_best_variant(master, "https://cdn/master.m3u8").unwrap(), "https://cdn/high.m3u8");

        let media = "#EXTM3U\n#EXTINF:4,\nseg1.ts\n#EXTINF:4,\nseg2.ts\n#EXT-X-ENDLIST\n";
        assert_eq!(
            hls_segments(media, "https://cdn/v/high.m3u8"),
            vec!["https://cdn/v/seg1.ts", "https://cdn/v/seg2.ts"]
        );
    }
}
