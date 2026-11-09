//! Saved rides: every successful build is saved automatically (so a ride you liked plays again
//! without building it, or paying the AI, again); the rider can keep, rename and favorite them.
//! One JSON file per ride in `<app dir>/exercise/rides/<id>.json`, written atomically and read
//! leniently (a file that doesn't parse is skipped, never fatal). These are HomeFi's own data
//! files: deleting one removes it, it is not a library file.
//!
//! A ride exports to the shared envelope (`export::wrap`, kind `ride`, `.homefiride`) with the
//! local absolute paths cut to file names, and imports back as a new kept ride whose local songs
//! are matched again in the importer's library.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::builder::RidePlan;
use super::model::{round_minutes, PresetId, RideRequest, MAX_NAME_CHARS, MAX_SEGMENTS};
use super::profile::{profile, ProfilePoint};
use super::settings::{MAX_RIDE_HISTORY, MIN_RIDE_HISTORY};
use super::words;
use crate::error::{Error, Result};
use crate::export::{self, ExportKind};
use crate::humane::{language, Lang};
use crate::json_dir::{new_id, JsonDir};
use crate::playlist::{now_ms, LibraryIndex};

pub const SAVED_RIDE_FORMAT_VERSION: u32 = 1;
/// Folder of the saved rides inside the app folder.
pub const RIDES_DIR: [&str; 2] = ["exercise", "rides"];
/// Most songs an imported ride may carry.
const MAX_IMPORTED_SONGS: usize = 200;
const SECONDS_PER_MINUTE: u32 = 60;

/// A ride kept on disk: what was asked and the plan that was built, ready to ride again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SavedRide {
    /// Format version of this file.
    pub v: u32,
    /// `[A-Za-z0-9_-]{1,64}`.
    pub id: String,
    pub name: String,
    pub created_ms: u64,
    pub last_ridden_ms: Option<u64>,
    pub times_ridden: u32,
    pub favorite: bool,
    /// Saved by itself after a build; `false` once the rider keeps it (or imported it).
    pub auto: bool,
    pub request: RideRequest,
    pub plan: RidePlan,
    /// The playlist the player plays for this ride, so riding again reuses it.
    pub playlist_id: Option<String>,
}

impl Default for SavedRide {
    fn default() -> Self {
        Self {
            v: SAVED_RIDE_FORMAT_VERSION,
            id: String::new(),
            name: String::new(),
            created_ms: 0,
            last_ridden_ms: None,
            times_ridden: 0,
            favorite: false,
            auto: true,
            request: RideRequest::default(),
            plan: RidePlan::default(),
            playlist_id: None,
        }
    }
}

/// A saved ride as a list shows it: no plan, but the profile for a mini mountain.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RideSummary {
    pub id: String,
    pub name: String,
    /// Whole minutes of the ride.
    pub minutes: u32,
    pub style: PresetId,
    pub genres: Vec<String>,
    pub songs: usize,
    pub created_ms: u64,
    pub last_ridden_ms: Option<u64>,
    pub times_ridden: u32,
    pub favorite: bool,
    /// Saved by itself after a build (it may be pruned); `false` once the rider keeps it.
    pub auto: bool,
    pub profile: Vec<ProfilePoint>,
}

impl SavedRide {
    pub fn summary(&self) -> RideSummary {
        let genres = if self.plan.wish.genres.is_empty() { &self.request.wish.genres } else { &self.plan.wish.genres };
        RideSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            minutes: self.minutes(),
            style: self.request.style,
            genres: genres.clone(),
            songs: self.plan.songs.len(),
            created_ms: self.created_ms,
            last_ridden_ms: self.last_ridden_ms,
            times_ridden: self.times_ridden,
            favorite: self.favorite,
            auto: self.auto,
            profile: self.plan.profile.clone(),
        }
    }

    /// Length of the ride in whole minutes (from its workout).
    pub fn minutes(&self) -> u32 {
        (self.plan.workout.total_seconds() + SECONDS_PER_MINUTE / 2) / SECONDS_PER_MINUTE
    }

    /// The moment that orders rides by recency: the last ride, or when it was saved.
    fn stamp(&self) -> u64 {
        self.last_ridden_ms.unwrap_or(0).max(self.created_ms)
    }

    /// Can it be pruned: saved automatically and not a favorite.
    fn prunable(&self) -> bool {
        self.auto && !self.favorite
    }
}

fn clean_name(name: &str) -> String {
    name.trim().chars().take(MAX_NAME_CHARS).collect()
}

/// "30 min · Classic · Rock + Metal", in `lang`.
pub fn auto_name(lang: Lang, request: &RideRequest, plan: &RidePlan) -> String {
    let minutes = (plan.workout.total_seconds() + SECONDS_PER_MINUTE / 2) / SECONDS_PER_MINUTE;
    let style = words::style_short(lang, request.style);
    let genres = if plan.wish.genres.is_empty() { &request.wish.genres } else { &plan.wish.genres };
    words::saved_ride_name(minutes, style, genres)
}

#[derive(Debug, Clone)]
pub struct RideStore {
    files: JsonDir,
}

impl RideStore {
    pub fn new(dir: impl Into<std::path::PathBuf>) -> Self {
        Self { files: JsonDir::new(dir) }
    }

    /// The store inside the app folder (`<app dir>/exercise/rides`).
    pub fn in_app_dir(app_dir: &Path) -> Self {
        Self::new(RIDES_DIR.iter().fold(app_dir.to_path_buf(), |dir, part| dir.join(part)))
    }

    /// Every readable ride: favorites first, then the most recent (ridden or saved) first.
    pub fn list(&self) -> Vec<SavedRide> {
        let mut rides: Vec<SavedRide> =
            self.files.list::<SavedRide>().into_iter().filter(|r| !r.id.is_empty()).collect();
        rides.sort_by(|a, b| {
            b.favorite.cmp(&a.favorite).then_with(|| b.stamp().cmp(&a.stamp())).then_with(|| a.id.cmp(&b.id))
        });
        rides
    }

    pub fn get(&self, id: &str) -> Result<SavedRide> {
        self.files.load(id)
    }

    pub fn save(&self, ride: &SavedRide) -> Result<()> {
        self.files.save(&ride.id, ride)
    }

    /// Saves a finished build as an automatic ride, then prunes the history to `history_max`
    /// (5..=200). `playlist_id` is the playlist the builder made for it.
    pub fn auto_save(
        &self,
        request: &RideRequest,
        plan: &RidePlan,
        playlist_id: Option<String>,
        lang: Lang,
        history_max: u16,
    ) -> Result<SavedRide> {
        let ride = SavedRide {
            id: new_id("r"),
            name: auto_name(lang, request, plan),
            created_ms: now_ms(),
            request: request.clone(),
            plan: plan.clone(),
            playlist_id,
            ..SavedRide::default()
        };
        self.save(&ride)?;
        if let Err(error) = self.prune(history_max) {
            tracing::warn!("could not prune the ride history: {error}");
        }
        Ok(ride)
    }

    /// Removes the oldest automatic rides beyond `history_max` (clamped to 5..=200); kept rides and
    /// favorites are never touched. Returns the ids removed.
    pub fn prune(&self, history_max: u16) -> Result<Vec<String>> {
        let max = usize::from(history_max.clamp(MIN_RIDE_HISTORY, MAX_RIDE_HISTORY));
        let mut autos: Vec<SavedRide> = self.list().into_iter().filter(SavedRide::prunable).collect();
        if autos.len() <= max {
            return Ok(Vec::new());
        }
        autos.sort_by(|a, b| a.stamp().cmp(&b.stamp()).then_with(|| a.id.cmp(&b.id)));
        let excess = autos.len() - max;
        let mut removed = Vec::with_capacity(excess);
        for ride in autos.into_iter().take(excess) {
            if self.files.remove(&ride.id)? {
                removed.push(ride.id);
            }
        }
        Ok(removed)
    }

    fn update(&self, id: &str, change: impl FnOnce(&mut SavedRide)) -> Result<SavedRide> {
        let mut ride = self.get(id)?;
        change(&mut ride);
        self.save(&ride)?;
        Ok(ride)
    }

    /// The rider keeps it: it stops being automatic (so pruning leaves it), optionally with a name.
    pub fn keep(&self, id: &str, name: Option<&str>) -> Result<SavedRide> {
        self.update(id, |r| {
            r.auto = false;
            if let Some(name) = name.map(clean_name).filter(|n| !n.is_empty()) {
                r.name = name;
            }
        })
    }

    /// A blank name leaves the name as it is. Renaming is also keeping: a ride the rider named matters.
    pub fn rename(&self, id: &str, name: &str) -> Result<SavedRide> {
        self.keep(id, Some(name))
    }

    pub fn set_favorite(&self, id: &str, favorite: bool) -> Result<SavedRide> {
        self.update(id, |r| r.favorite = favorite)
    }

    /// Counts a ride and stamps it as the most recent.
    pub fn mark_ridden(&self, id: &str) -> Result<SavedRide> {
        self.update(id, |r| {
            r.times_ridden = r.times_ridden.saturating_add(1);
            r.last_ridden_ms = Some(now_ms());
        })
    }

    /// Deletes the ride's file (HomeFi's own data); `false` when there is none.
    pub fn delete(&self, id: &str) -> Result<bool> {
        self.files.remove(id)
    }
}

// ------------------------------------------------------------------ export / import

/// What `data` of a ride export holds: no id, no counters, no playlist, no absolute paths.
#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
struct RideFile {
    v: u32,
    name: String,
    request: RideRequest,
    plan: RidePlan,
}

/// The last part of a path written on any system.
fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// The shareable text of `ride` (`.homefiride`). Local files are named, not located: only their
/// file names leave this computer.
pub fn export_ride(ride: &SavedRide) -> Result<String> {
    let mut plan = ride.plan.clone();
    for song in &mut plan.songs {
        song.entry.local_path = song.entry.local_path.as_deref().map(|p| file_name(p).to_owned());
    }
    let file = RideFile { v: SAVED_RIDE_FORMAT_VERSION, name: ride.name.clone(), request: ride.request.clone(), plan };
    export::wrap(ExportKind::Ride, &file)
}

/// A kept ride from an export file. Local songs are found again by ISRC, then artist and title in
/// `library` (when it is known); the others stay as Tidal ids or as title and artist only. A file
/// from a newer HomeFi, another kind of file or one with nothing to play is refused with words.
pub fn import_ride(text: &str, library: Option<&LibraryIndex>) -> Result<SavedRide> {
    let lang = language();
    let file: RideFile = export::unwrap(ExportKind::Ride, text)?;
    if file.v > SAVED_RIDE_FORMAT_VERSION {
        return Err(Error::Export(words::ride_file_newer(lang)));
    }
    let mut plan = file.plan;
    plan.workout = plan.workout.sanitized();
    let segments = plan.workout.segments.len().min(MAX_SEGMENTS);
    plan.songs.retain(|s| s.segment_index < segments);
    plan.songs.truncate(MAX_IMPORTED_SONGS);
    if plan.songs.is_empty() {
        return Err(Error::Export(words::ride_file_empty(lang)));
    }
    for song in &mut plan.songs {
        let entry = &mut song.entry;
        // a file name is not a place: it only counts when the library has the song
        entry.local_path = library.and_then(|lib| {
            entry
                .isrc
                .as_deref()
                .and_then(|isrc| lib.by_isrc(isrc))
                .or_else(|| lib.find(&entry.artist, &entry.title))
                .map(|found| found.path.clone())
        });
    }
    plan.profile = profile(&plan.workout);
    plan.wish = plan.wish.clamped();
    let mut request = file.request;
    request.minutes = round_minutes(request.minutes);
    request.wish = request.wish.clamped();
    let name = clean_name(&file.name);
    let name = if name.is_empty() { auto_name(lang, &request, &plan) } else { name };
    Ok(SavedRide { id: new_id("r"), name, created_ms: now_ms(), auto: false, request, plan, ..SavedRide::default() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exercise::settings::DEFAULT_RIDE_HISTORY;
    use crate::exercise::{presets, ExerciseSettings, PlannedSong, PresetId};
    use crate::playlist::{LibraryRef, PlaylistEntry};

    fn plan_with(songs: &[(&str, &str, Option<&str>, Option<&str>)]) -> RidePlan {
        let workout = presets::preset(PresetId::Classic, 30, &ExerciseSettings::default());
        let songs = songs
            .iter()
            .enumerate()
            .map(|(i, (title, artist, tidal, local))| PlannedSong {
                segment_index: i,
                entry: PlaylistEntry {
                    title: (*title).into(),
                    artist: (*artist).into(),
                    tidal_id: tidal.map(Into::into),
                    local_path: local.map(Into::into),
                    isrc: Some(format!("US{i:010}")),
                    ..PlaylistEntry::default()
                },
                bpm: Some(120),
                ..PlannedSong::default()
            })
            .collect();
        RidePlan { version: 1, profile: profile(&workout), workout, songs, ..RidePlan::default() }
    }

    fn request() -> RideRequest {
        let mut r = RideRequest { minutes: 30, style: PresetId::Classic, ..RideRequest::default() };
        r.wish.genres = vec!["Rock".into(), "Metal".into()];
        r
    }

    fn store() -> (tempfile::TempDir, RideStore) {
        let tmp = tempfile::tempdir().unwrap();
        let store = RideStore::in_app_dir(tmp.path());
        (tmp, store)
    }

    fn put(store: &RideStore, id: &str, auto: bool, favorite: bool, created_ms: u64) {
        store
            .save(&SavedRide { id: id.into(), name: id.into(), auto, favorite, created_ms, ..SavedRide::default() })
            .unwrap();
    }

    #[test]
    fn auto_save_names_the_ride_and_loads_back() {
        let (_tmp, store) = store();
        let plan = plan_with(&[("A", "X", Some("1"), None)]);
        let saved = store.auto_save(&request(), &plan, Some("p1".into()), Lang::En, 30).unwrap();
        assert!(saved.auto && !saved.favorite && saved.times_ridden == 0);
        assert_eq!(saved.name, "30 min · Classic · Rock + Metal");
        let es = auto_name(Lang::Es, &request(), &plan);
        assert_eq!(es, "30 min · Clásico · Rock + Metal");
        assert_eq!(store.get(&saved.id).unwrap(), saved);
        assert_eq!(store.list().len(), 1);
        assert!(RideStore::in_app_dir(_tmp.path()).files.path().ends_with("exercise/rides"));
    }

    #[test]
    fn favorites_come_first_then_the_most_recent() {
        let (_tmp, store) = store();
        put(&store, "old", true, false, 100);
        put(&store, "new", true, false, 300);
        put(&store, "fav", true, true, 1);
        put(&store, "ridden", true, false, 50);
        store.mark_ridden("ridden").unwrap();
        let order: Vec<String> = store.list().into_iter().map(|r| r.id).collect();
        assert_eq!(order, ["fav", "ridden", "new", "old"]);
        let ridden = store.get("ridden").unwrap();
        assert_eq!(ridden.times_ridden, 1);
        assert!(ridden.last_ridden_ms.is_some());
    }

    #[test]
    fn pruning_drops_the_oldest_automatic_rides_only() {
        let (_tmp, store) = store();
        for i in 0..8u64 {
            put(&store, &format!("a{i}"), true, false, 100 + i);
        }
        put(&store, "kept", false, false, 1);
        put(&store, "fav", true, true, 2);
        let removed = store.prune(5).unwrap();
        assert_eq!(removed, ["a0", "a1", "a2"]);
        let left: Vec<String> = store.list().into_iter().map(|r| r.id).collect();
        assert!(left.contains(&"kept".into()) && left.contains(&"fav".into()));
        assert_eq!(left.len(), 7);
        assert!(store.prune(5).unwrap().is_empty());
        // the cap is clamped: 0 means 5, not "delete everything"
        assert!(store.prune(0).unwrap().is_empty());
    }

    #[test]
    fn auto_save_prunes_but_keeps_the_new_ride() {
        let (_tmp, store) = store();
        for i in 0..5u64 {
            put(&store, &format!("a{i}"), true, false, 100 + i);
        }
        let plan = plan_with(&[("A", "X", Some("1"), None)]);
        let saved = store.auto_save(&request(), &plan, None, Lang::En, 5).unwrap();
        let ids: Vec<String> = store.list().into_iter().map(|r| r.id).collect();
        assert_eq!(ids.len(), 5);
        assert!(ids.contains(&saved.id) && !ids.contains(&"a0".to_string()));
    }

    #[test]
    fn keep_rename_favorite_delete() {
        let (_tmp, store) = store();
        put(&store, "r1", true, false, 1);
        assert!(!store.keep("r1", None).unwrap().auto);
        assert_eq!(store.rename("r1", "  Sunday  ").unwrap().name, "Sunday");
        assert_eq!(store.rename("r1", "   ").unwrap().name, "Sunday", "a blank name changes nothing");
        assert!(store.set_favorite("r1", true).unwrap().favorite);
        assert!(store.get("nope").is_err() && store.get("../x").is_err());
        assert!(store.delete("r1").unwrap() && !store.delete("r1").unwrap());
        assert!(store.list().is_empty());
    }

    #[test]
    fn a_bad_file_does_not_break_the_list() {
        let (tmp, store) = store();
        put(&store, "good", true, false, 1);
        std::fs::write(tmp.path().join("exercise/rides/bad.json"), "{ nope").unwrap();
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn export_strips_local_paths_and_import_rematches_them() {
        let plan = plan_with(&[
            ("Song One", "Band", Some("11"), Some("/Users/jose/Music/Band/one.flac")),
            ("Song Two", "Band", None, Some("C:\\Music\\two.flac")),
            ("Song Three", "Other", Some("33"), None),
        ]);
        let ride = SavedRide {
            id: "r1".into(),
            name: "Mine".into(),
            favorite: true,
            times_ridden: 4,
            playlist_id: Some("p9".into()),
            request: request(),
            plan,
            ..SavedRide::default()
        };
        let text = export_ride(&ride).unwrap();
        assert!(!text.contains("/Users/jose") && !text.contains("C:\\\\Music"), "{text}");
        assert!(text.contains("one.flac") && text.contains("\"homefi_export\": 1"));
        assert!(!text.contains("p9") && !text.contains("times_ridden"));

        // nobody's library: local songs lose their place, Tidal ids stay
        let bare = import_ride(&text, None).unwrap();
        assert!(!bare.auto && !bare.favorite && bare.times_ridden == 0 && bare.playlist_id.is_none());
        assert_ne!(bare.id, "r1");
        assert_eq!(bare.name, "Mine");
        assert!(bare.plan.songs.iter().all(|s| s.entry.local_path.is_none()));
        assert_eq!(bare.plan.songs[0].entry.tidal_id.as_deref(), Some("11"));
        assert_eq!(bare.plan.songs[1].entry.title, "Song Two");
        assert_eq!(bare.request, ride.request);
        assert_eq!(bare.plan.workout, ride.plan.workout);

        // a library with Song Two (by title and artist) and Song One (by ISRC)
        let lib = LibraryIndex::new(vec![
            LibraryRef {
                path: "/Music/Band/two.flac".into(),
                title: "Song Two".into(),
                artist: "Band".into(),
                ..LibraryRef::default()
            },
            LibraryRef {
                path: "/Other/one.flac".into(),
                title: "Different".into(),
                artist: "Nobody".into(),
                isrc: Some("US0000000000".into()),
                ..LibraryRef::default()
            },
        ]);
        let matched = import_ride(&text, Some(&lib)).unwrap();
        assert_eq!(matched.plan.songs[0].entry.local_path.as_deref(), Some("/Other/one.flac"));
        assert_eq!(matched.plan.songs[1].entry.local_path.as_deref(), Some("/Music/Band/two.flac"));
        assert_eq!(matched.plan.songs[2].entry.local_path, None);
    }

    #[test]
    fn imports_are_refused_with_words() {
        let ride = SavedRide {
            id: "r".into(),
            request: request(),
            plan: plan_with(&[("A", "B", Some("1"), None)]),
            ..SavedRide::default()
        };
        let text = export_ride(&ride).unwrap();
        // newer envelope
        let newer = text.replace("\"homefi_export\": 1", "\"homefi_export\": 2");
        assert!(import_ride(&newer, None).unwrap_err().to_string().to_lowercase().contains("newer"));
        // newer ride format
        let newer_ride = text.replacen("\"v\": 1", "\"v\": 7", 1);
        assert!(import_ride(&newer_ride, None).unwrap_err().to_string().to_lowercase().contains("newer"));
        // garbage and the wrong kind
        assert!(import_ride("hello", None).is_err());
        let dj = export::wrap(ExportKind::DjSetup, &1).unwrap();
        assert!(import_ride(&dj, None).is_err());
        // nothing to play
        let empty = SavedRide { request: request(), plan: plan_with(&[]), ..SavedRide::default() };
        assert!(import_ride(&export_ride(&empty).unwrap(), None).is_err());
    }

    #[test]
    fn the_default_history_is_inside_its_range() {
        assert!((MIN_RIDE_HISTORY..=MAX_RIDE_HISTORY).contains(&DEFAULT_RIDE_HISTORY));
    }
}
