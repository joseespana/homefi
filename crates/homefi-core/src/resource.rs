//! Tidal links: `https://tidal.com/browse/album/123`, `album/123`, ...

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceKind {
    Track,
    Video,
    Album,
    Playlist,
    Artist,
    Mix,
}

impl ResourceKind {
    pub const ALL: [ResourceKind; 6] = [Self::Track, Self::Video, Self::Album, Self::Playlist, Self::Artist, Self::Mix];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Track => "track",
            Self::Video => "video",
            Self::Album => "album",
            Self::Playlist => "playlist",
            Self::Artist => "artist",
            Self::Mix => "mix",
        }
    }

    fn numeric_id(self) -> bool {
        matches!(self, Self::Track | Self::Video | Self::Album | Self::Artist)
    }
}

impl fmt::Display for ResourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ResourceKind {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().to_ascii_lowercase();
        let s = s.strip_suffix('s').unwrap_or(&s);
        Self::ALL
            .into_iter()
            .find(|k| k.as_str() == s)
            .ok_or_else(|| Error::Resource(format!("unknown resource type `{s}`")))
    }
}

/// A downloadable Tidal resource.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Resource {
    pub kind: ResourceKind,
    pub id: String,
}

impl Resource {
    pub fn new(kind: ResourceKind, id: impl Into<String>) -> Self {
        Self { kind, id: id.into() }
    }

    pub fn url(&self) -> String {
        format!("https://listen.tidal.com/{}/{}", self.kind, self.id)
    }

    /// Parses a Tidal URL or the short `type/id` form.
    pub fn parse(input: &str) -> Result<Self, Error> {
        let input = input.trim();
        let path = match url::Url::parse(input) {
            Ok(u) if u.has_host() => u.path().to_owned(),
            _ => input.split(['?', '#']).next().unwrap_or_default().to_owned(),
        };
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        let (pos, kind) = segments
            .iter()
            .enumerate()
            .find_map(|(i, s)| {
                ResourceKind::ALL.into_iter().find(|k| k.as_str() == s.to_ascii_lowercase()).map(|k| (i, k))
            })
            .ok_or_else(|| Error::Resource(format!("no resource type in `{input}`")))?;

        let id = segments.get(pos + 1).ok_or_else(|| Error::Resource(format!("no {kind} id in `{input}`")))?;

        if kind.numeric_id() && !id.chars().all(|c| c.is_ascii_digit()) {
            return Err(Error::Resource(format!("invalid {kind} id `{id}`")));
        }

        Ok(Self::new(kind, *id))
    }
}

impl fmt::Display for Resource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.kind, self.id)
    }
}

impl FromStr for Resource {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_urls_and_shorthands() {
        let cases = [
            ("https://tidal.com/browse/track/123", ResourceKind::Track, "123"),
            ("https://listen.tidal.com/album/456?u", ResourceKind::Album, "456"),
            ("http://www.tidal.com/playlist/0b1a-c2/x", ResourceKind::Playlist, "0b1a-c2"),
            ("https://tidal.com/browse/mix/00abc", ResourceKind::Mix, "00abc"),
            ("artist/7", ResourceKind::Artist, "7"),
            ("video/9", ResourceKind::Video, "9"),
        ];
        for (input, kind, id) in cases {
            assert_eq!(Resource::parse(input).unwrap(), Resource::new(kind, id), "{input}");
        }
    }

    #[test]
    fn rejects_invalid_inputs() {
        assert!(Resource::parse("https://tidal.com/browse").is_err());
        assert!(Resource::parse("track/").is_err());
        assert!(Resource::parse("track/abc").is_err());
        assert!(Resource::parse("song/1").is_err());
    }

    #[test]
    fn kinds_parse_plural() {
        assert_eq!("albums".parse::<ResourceKind>().unwrap(), ResourceKind::Album);
        assert_eq!("TRACKS".parse::<ResourceKind>().unwrap(), ResourceKind::Track);
    }
}
