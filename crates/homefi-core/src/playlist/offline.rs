//! A playlist (a ride) with no internet: which file each entry plays, and which TIDAL songs still
//! have to be downloaded before it can play without a connection.
//!
//! The rule, once: an entry plays its own `local_path` while that file is there; otherwise the
//! library file with the same ISRC (a TIDAL song of the plan downloaded after the plan was made);
//! otherwise it streams from TIDAL. Android mirrors it in `LibraryRepository.localCopy`, over the
//! same ISRC keys (the scanner and the resolver already store them as [`isrc_key`] makes them).

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use super::model::PlaylistEntry;

/// An ISRC as HomeFi compares them: trimmed and upper case; `None` when blank.
pub fn isrc_key(raw: &str) -> Option<String> {
    Some(raw.trim().to_uppercase()).filter(|key| !key.is_empty())
}

/// The library's files by ISRC: how an entry made from a TIDAL song finds the file downloaded later.
#[derive(Debug, Clone, Default)]
pub struct LocalCopies {
    by_isrc: HashMap<String, String>,
}

impl LocalCopies {
    /// From the library's `(path, isrc)` pairs; the first file wins when two share an ISRC.
    pub fn new<'a>(files: impl IntoIterator<Item = (&'a str, Option<&'a str>)>) -> Self {
        let mut by_isrc = HashMap::new();
        for (path, isrc) in files {
            if let Some(key) = isrc.and_then(isrc_key) {
                by_isrc.entry(key).or_insert_with(|| path.to_owned());
            }
        }
        Self { by_isrc }
    }

    /// The library file with this ISRC.
    pub fn by_isrc(&self, isrc: &str) -> Option<&str> {
        self.by_isrc.get(&isrc_key(isrc)?).map(String::as_str)
    }

    /// The file `entry` plays: its own `local_path` while `has_file` says it is there, else the library
    /// file with the same ISRC; `None` streams (or skips) it.
    pub fn file_of<'a>(&'a self, entry: &'a PlaylistEntry, has_file: impl Fn(&str) -> bool) -> Option<&'a str> {
        entry
            .local_path
            .as_deref()
            .filter(|path| has_file(path))
            .or_else(|| entry.isrc.as_deref().and_then(|isrc| self.by_isrc(isrc)))
    }

    /// Points every entry at the file it plays ([`Self::file_of`]); entries with none keep what they had.
    /// The number of entries changed.
    pub fn relink<'e>(
        &self,
        entries: impl IntoIterator<Item = &'e mut PlaylistEntry>,
        has_file: impl Fn(&str) -> bool,
    ) -> usize {
        let mut changed = 0;
        for entry in entries {
            let Some(found) = self.file_of(entry, &has_file) else { continue };
            if entry.local_path.as_deref() != Some(found) {
                entry.local_path = Some(found.to_owned());
                changed += 1;
            }
        }
        changed
    }

    /// [`offline_status`] of `entries` through [`Self::file_of`].
    pub fn status<'e>(
        &self,
        entries: impl IntoIterator<Item = &'e PlaylistEntry>,
        has_file: impl Fn(&str) -> bool,
    ) -> OfflineStatus {
        offline_status(entries, |entry| self.file_of(entry, &has_file).is_some())
    }
}

/// How ready a list of songs is to play with no internet.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct OfflineStatus {
    /// Entries that can play at all (a file or a TIDAL song).
    pub songs: usize,
    /// Of those, the ones with a file here.
    pub ready: usize,
    /// TIDAL ids of the others, once each, in order: what "download for offline" queues.
    pub to_download: Vec<String>,
}

impl OfflineStatus {
    /// Every song that can play has a file: the list plays with no internet.
    pub fn is_ready(&self) -> bool {
        self.songs > 0 && self.ready == self.songs
    }
}

/// Counts the entries that play from a file (`is_local`) and lists the TIDAL songs to download for the
/// rest. An entry with neither a file nor a TIDAL id can't play and is left out.
pub fn offline_status<'e>(
    entries: impl IntoIterator<Item = &'e PlaylistEntry>,
    is_local: impl Fn(&PlaylistEntry) -> bool,
) -> OfflineStatus {
    let mut status = OfflineStatus::default();
    let mut queued = HashSet::new();
    for entry in entries {
        if is_local(entry) {
            status.songs += 1;
            status.ready += 1;
        } else if let Some(id) = entry.tidal_id.as_deref().filter(|id| !id.trim().is_empty()) {
            status.songs += 1;
            if queued.insert(id) {
                status.to_download.push(id.to_owned());
            }
        }
    }
    status
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(tidal_id: Option<&str>, isrc: Option<&str>, local_path: Option<&str>) -> PlaylistEntry {
        PlaylistEntry {
            title: "Song".into(),
            tidal_id: tidal_id.map(Into::into),
            isrc: isrc.map(Into::into),
            local_path: local_path.map(Into::into),
            ..PlaylistEntry::default()
        }
    }

    fn library() -> LocalCopies {
        LocalCopies::new([
            ("/m/a.flac", Some("USAAA0000001")),
            ("/m/b.flac", None),
            ("/m/dup.flac", Some(" usaaa0000001 ")),
            ("/m/c.flac", Some("GBCCC0000003")),
        ])
    }

    const EVERY_FILE: fn(&str) -> bool = |_| true;
    const NO_FILE: fn(&str) -> bool = |_| false;

    #[test]
    fn isrc_keys_are_trimmed_upper_case_and_never_blank() {
        assert_eq!(isrc_key(" usaaa0000001\n").as_deref(), Some("USAAA0000001"));
        assert_eq!(isrc_key("   "), None);
        assert_eq!(isrc_key(""), None);
    }

    #[test]
    fn an_entry_plays_its_own_file_first_then_the_file_with_its_isrc() {
        let copies = library();
        let own = entry(Some("1"), Some("USAAA0000001"), Some("/m/mine.flac"));
        assert_eq!(copies.file_of(&own, EVERY_FILE), Some("/m/mine.flac"), "its own file while it is there");
        assert_eq!(copies.file_of(&own, NO_FILE), Some("/m/a.flac"), "gone: the file with the same ISRC");

        let downloaded_later = entry(Some("2"), Some("gbccc0000003"), None);
        assert_eq!(copies.file_of(&downloaded_later, NO_FILE), Some("/m/c.flac"), "ISRC case does not matter");

        let tidal_only = entry(Some("3"), Some("ZZZZZ0000000"), None);
        assert_eq!(copies.file_of(&tidal_only, EVERY_FILE), None, "no file: it streams");
        assert_eq!(copies.file_of(&entry(Some("4"), None, None), EVERY_FILE), None, "no ISRC, no match");
    }

    #[test]
    fn the_first_file_wins_when_two_share_an_isrc() {
        assert_eq!(library().by_isrc("usaaa0000001"), Some("/m/a.flac"));
        assert_eq!(library().by_isrc("  "), None);
    }

    #[test]
    fn relink_points_entries_at_the_files_they_play_and_counts_the_changes() {
        let copies = library();
        let mut entries = [
            entry(Some("1"), Some("USAAA0000001"), None),
            entry(Some("2"), Some("GBCCC0000003"), Some("/m/c.flac")),
            entry(Some("3"), None, Some("/gone.flac")),
            entry(Some("4"), Some("ZZZZZ0000000"), None),
        ];
        let changed = copies.relink(entries.iter_mut(), |path| path != "/gone.flac");
        assert_eq!(changed, 1, "only the first entry changes");
        assert_eq!(entries[0].local_path.as_deref(), Some("/m/a.flac"));
        assert_eq!(entries[1].local_path.as_deref(), Some("/m/c.flac"));
        assert_eq!(entries[2].local_path.as_deref(), Some("/gone.flac"), "nothing better: left as it was");
        assert_eq!(entries[3].local_path, None);
        assert_eq!(copies.relink(entries.iter_mut(), |path| path != "/gone.flac"), 0, "a second pass changes nothing");
    }

    #[test]
    fn the_status_counts_local_songs_and_lists_each_tidal_song_to_download_once() {
        let copies = library();
        let entries = [
            entry(Some("1"), Some("USAAA0000001"), None),
            entry(Some("9"), None, None),
            entry(Some("9"), None, None),
            entry(None, None, None),
            entry(None, None, Some("/m/b.flac")),
            entry(Some(" "), None, None),
        ];
        let status = copies.status(&entries, |path| path == "/m/b.flac");
        assert_eq!(status, OfflineStatus { songs: 4, ready: 2, to_download: vec!["9".into()] });
        assert!(!status.is_ready());
    }

    #[test]
    fn a_list_is_ready_offline_only_when_every_song_has_a_file() {
        let copies = library();
        let all_local = [entry(Some("1"), Some("USAAA0000001"), None), entry(None, None, Some("/m/b.flac"))];
        let status = copies.status(&all_local, |path| path == "/m/b.flac");
        assert!(status.is_ready() && status.to_download.is_empty());
        assert!(!OfflineStatus::default().is_ready(), "an empty list is not \"ready offline\"");
    }
}
