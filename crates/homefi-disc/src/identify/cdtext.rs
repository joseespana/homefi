//! A [`DiscMatch`] from the text stored on the disc.

use super::{AlbumMeta, DiscMatch, Source, TrackMeta};
use crate::cdtext::CdText;
use crate::toc::Toc;

/// It is the disc's own word: full confidence. `None` when the disc has no usable text.
pub fn from_cd_text(text: &CdText, toc: &Toc) -> Option<DiscMatch> {
    if text.is_empty() {
        return None;
    }
    let album_artist = text.artist.clone().unwrap_or_default();
    let tracks = toc
        .audio_tracks()
        .map(|entry| {
            let named = text.tracks.iter().find(|t| t.number == entry.number);
            let artist = named.and_then(|t| t.artist.clone()).unwrap_or_else(|| album_artist.clone());
            TrackMeta {
                number: entry.number,
                title: named.and_then(|t| t.title.clone()).unwrap_or_default(),
                artists: Some(artist).filter(|a| !a.is_empty()).into_iter().collect(),
                ..TrackMeta::default()
            }
        })
        .collect();
    Some(DiscMatch {
        source: Source::CdText,
        confidence: 1.0,
        album: AlbumMeta {
            title: text.title.clone().unwrap_or_default(),
            artist: album_artist,
            genre: text.genre.clone(),
            upc: text.upc.clone(),
            ..AlbumMeta::default()
        },
        tracks,
        cover: None,
        note: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cdtext::CdTextTrack;
    use crate::toc::{RawTocEntry, Toc};

    fn toc() -> Toc {
        let entries: Vec<RawTocEntry> =
            (1..=3).map(|n| RawTocEntry { number: n, control: 0, start_lba: u32::from(n - 1) * 1000 }).collect();
        Toc::from_entries(&entries, 3000)
    }

    #[test]
    fn titles_and_artists_follow_the_disc() {
        let text = CdText {
            title: Some("Korn".into()),
            artist: Some("Korn".into()),
            genre: None,
            upc: Some("0123".into()),
            tracks: vec![
                CdTextTrack { number: 1, title: Some("Blind".into()), artist: None },
                CdTextTrack { number: 3, title: Some("Need To".into()), artist: Some("Guest".into()) },
            ],
        };
        let found = from_cd_text(&text, &toc()).unwrap();
        assert_eq!(found.source, Source::CdText);
        assert_eq!(found.album.title, "Korn");
        assert_eq!(found.album.upc.as_deref(), Some("0123"));
        let titles: Vec<&str> = found.tracks.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, ["Blind", "", "Need To"]);
        assert_eq!(found.tracks[0].artists, ["Korn"]);
        assert_eq!(found.tracks[2].artists, ["Guest"]);
    }

    #[test]
    fn no_text_no_match() {
        assert!(from_cd_text(&CdText::default(), &toc()).is_none());
    }
}
