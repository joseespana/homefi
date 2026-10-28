//! Album versions for Kotlin: one card per album (explicit / clean, stereo /
//! Dolby Atmos, remasters, deluxe...), the preferences that pick the card's
//! release, and the version picker's rows.

use homefi_core::album_versions::{EditionPref, ExplicitPref, Lang, SpatialPref, VersionPrefs};
use homefi_core::catalog::VersionHit;

/// One release of an album, for the version picker.
#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct AlbumVersionHit {
    /// Tidal album id: `album/<id>` opens, plays or downloads it.
    pub id: String,
    pub title: String,
    pub cover_url: Option<String>,
    pub explicit: bool,
    /// `Hi-Res`, `Lossless`, `Dolby Atmos` when known.
    pub quality: Option<String>,
    pub year: Option<u32>,
    pub tracks: u32,
    /// Deluxe / box set / expanded: tracks over the standard edition.
    pub extra_tracks: Option<u32>,
    /// Dolby Atmos: needs an Atmos device (downloads skip Atmos tracks).
    pub atmos: bool,
    /// The release the card shows by default.
    pub primary: bool,
    /// `explicit`, `clean`, `dolby_atmos`, `hi_res`, `lossless`, `remastered`,
    /// `deluxe`, `box_set`, `expanded`, `anniversary`, `live`, `original`.
    pub label_codes: Vec<String>,
    /// Chip texts in the language given to `set_album_versions`.
    pub labels: Vec<String>,
    /// `Explícita · Hi-Res 24-bit · 12 canciones · 2023`.
    pub description: String,
}

impl From<VersionHit> for AlbumVersionHit {
    fn from(v: VersionHit) -> Self {
        Self {
            id: v.id,
            title: v.title,
            cover_url: v.cover_url,
            explicit: v.explicit,
            quality: v.quality,
            year: v.year,
            tracks: v.tracks,
            extra_tracks: v.extra_tracks,
            atmos: v.atmos,
            primary: v.primary,
            label_codes: v.label_codes,
            labels: v.labels,
            description: v.description,
        }
    }
}

#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlbumExplicitPref {
    /// The artist's original (Tidal's default).
    Explicit,
    Clean,
}

#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlbumSpatialPref {
    Stereo,
    /// Needs an Atmos device.
    DolbyAtmos,
}

#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlbumEditionPref {
    Original,
    /// The newest remaster.
    Remaster,
}

/// Which release an album card shows; the highest quality wins among the
/// rest, deluxe editions and box sets only when nothing else is left.
#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct AlbumVersionPrefs {
    /// One card per album; off lists every release as Tidal does.
    #[uniffi(default = true)]
    pub group: bool,
    pub explicit: AlbumExplicitPref,
    pub spatial: AlbumSpatialPref,
    pub edition: AlbumEditionPref,
}

impl From<&AlbumVersionPrefs> for VersionPrefs {
    fn from(p: &AlbumVersionPrefs) -> Self {
        Self {
            group: p.group,
            explicit: match p.explicit {
                AlbumExplicitPref::Explicit => ExplicitPref::Explicit,
                AlbumExplicitPref::Clean => ExplicitPref::Clean,
            },
            spatial: match p.spatial {
                AlbumSpatialPref::Stereo => SpatialPref::Stereo,
                AlbumSpatialPref::DolbyAtmos => SpatialPref::Atmos,
            },
            edition: match p.edition {
                AlbumEditionPref::Original => EditionPref::Original,
                AlbumEditionPref::Remaster => EditionPref::Remaster,
            },
        }
    }
}

/// The defaults: grouped, explicit, stereo, original.
#[uniffi::export]
pub fn album_version_default_prefs() -> AlbumVersionPrefs {
    AlbumVersionPrefs {
        group: true,
        explicit: AlbumExplicitPref::Explicit,
        spatial: AlbumSpatialPref::Stereo,
        edition: AlbumEditionPref::Original,
    }
}

/// What the engine groups with, set from the app's settings.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Grouping {
    pub prefs: VersionPrefs,
    pub lang: Lang,
}

impl Grouping {
    pub fn new(prefs: &AlbumVersionPrefs, language: &str) -> Self {
        Self { prefs: prefs.into(), lang: Lang::from_code(language) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefs_map_to_the_core() {
        let mut p = album_version_default_prefs();
        assert_eq!(VersionPrefs::from(&p), VersionPrefs::default());
        p.spatial = AlbumSpatialPref::DolbyAtmos;
        p.edition = AlbumEditionPref::Remaster;
        p.explicit = AlbumExplicitPref::Clean;
        let g = Grouping::new(&p, "es-MX");
        assert_eq!(g.prefs.spatial, SpatialPref::Atmos);
        assert_eq!(g.prefs.edition, EditionPref::Remaster);
        assert_eq!(g.prefs.explicit, ExplicitPref::Clean);
        assert_eq!(g.lang, Lang::Es);
    }
}
