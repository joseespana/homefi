//! Party mode on a guest computer: plays the hub's song on this computer's
//! output and keeps it on the hub's clock (see `docs/shared-control.md`).
//!
//! - **Local first.** The song playing now and, ahead, the next and the previous
//!   are downloaded into `<app dir>/party-cache` (HTTP range requests, so an
//!   interrupted download resumes); songs change from disk at once and a busy
//!   network doesn't cut the music. The folder is deleted on leaving.
//! - **Seek only.** The desktop engine can't change its playback rate, so gaps
//!   are closed by jumping: the gap is the median of the last few measurements,
//!   a jump needs [`MIN_SAMPLES_TO_JUMP`] that agree, the position settles for
//!   [`SETTLE`] after each one and the time a jump takes is learnt for the next.
//! - **Starting together.** A new song scheduled in the future is loaded paused
//!   at its start, `ready` is sent, and it starts when the hub clock gets there.
//!
//! A second [`Player`] (its own output stream), so the user's own queue is left
//! as it was and comes back after the party.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use homefi_core::dsp::EqSettings;
use homefi_core::share::sync::{sync_action, SyncAction};
use homefi_core::share::{AudioStream, GuestClient, GuestMessage, State};
use homefi_player::{Player, Snapshot, Status, Visual};
use tokio::io::AsyncWriteExt;
use tokio::task::JoinHandle;

/// After a jump, the position is left alone this long before it counts.
const SETTLE: Duration = Duration::from_millis(700);
/// Gap samples for the median (4 a second).
const GAP_SAMPLES: usize = 7;
const MIN_SAMPLES_TO_JUMP: usize = 4;
/// A jump that lands further off than this teaches the lead.
const LEAD_TOLERANCE_MS: i64 = 40;
const MAX_LEAD_MS: i64 = 3_000;
/// Without speed changes, gaps the core would close by speed are jumped once
/// they get this big: a smaller echo between speakers isn't worth a glitch.
pub const SEEK_ONLY_OVER_MS: i64 = 80;
/// Paused: further off than this, move to where the hub is.
const PAUSED_TOLERANCE_MS: i64 = 250;
/// Closer than this to a scheduled start, it is timed exactly instead of on a tick.
const START_AHEAD_MS: i64 = 400;
const HTTP_TIMEOUT: Duration = Duration::from_secs(8);

/// How far this computer's player is from the hub, and when to jump. Pure, so
/// it is tested on its own.
#[derive(Debug, Default)]
pub struct SyncState {
    gaps: VecDeque<i64>,
    /// Just started or jumped: waiting for the position to settle.
    seeking: bool,
    settle_until: Option<Instant>,
    /// How long a jump takes to be heard; added to where it aims.
    pub seek_lead_ms: i64,
}

impl SyncState {
    /// Call after starting a song or jumping.
    pub fn jumped(&mut self) {
        self.seeking = true;
        self.settle_until = None;
        self.gaps.clear();
    }

    /// One measurement (target = where the hub is plus this speaker's delay,
    /// actual = where the player is). Returns where to jump, if it should.
    pub fn step(&mut self, now: Instant, target: u64, actual: u64) -> Option<u64> {
        if self.seeking {
            let until = *self.settle_until.get_or_insert(now + SETTLE);
            if now < until {
                return None;
            }
            self.seeking = false;
            self.settle_until = None;
            // how late the jump landed, so the next one aims further ahead
            let gap = target as i64 - actual as i64;
            if gap.abs() > LEAD_TOLERANCE_MS {
                self.seek_lead_ms = (self.seek_lead_ms + gap).clamp(0, MAX_LEAD_MS);
            }
        }
        self.gaps.push_back(target as i64 - actual as i64);
        while self.gaps.len() > GAP_SAMPLES {
            self.gaps.pop_front();
        }
        let mut sorted: Vec<i64> = self.gaps.iter().copied().collect();
        sorted.sort_unstable();
        let gap = sorted[sorted.len() / 2];
        let steady = self.gaps.len() >= MIN_SAMPLES_TO_JUMP;
        let seek = match sync_action(target, (target as i64 - gap).max(0) as u64) {
            SyncAction::Keep => false,
            // no speed control on the desktop: only a bigger gap is worth a jump
            SyncAction::Rate(_) => steady && gap.abs() > SEEK_ONLY_OVER_MS,
            SyncAction::Seek(_) => steady,
        };
        if !seek {
            return None;
        }
        self.jumped();
        Some((target as i64 + self.seek_lead_ms).max(0) as u64)
    }
}

/// File extension for a served song, so the decoder gets a good hint.
fn extension(mime: &str, url: &str) -> &'static str {
    match mime {
        "audio/flac" | "audio/x-flac" => "flac",
        "audio/mp4" | "audio/m4a" | "audio/x-m4a" | "audio/aac" => "m4a",
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/ogg" | "audio/vorbis" => "ogg",
        "audio/opus" => "opus",
        "audio/wav" | "audio/x-wav" | "audio/wave" => "wav",
        "audio/aiff" | "audio/x-aiff" => "aiff",
        _ if url.contains(".flac") => "flac",
        _ => "audio",
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Wanted {
    queue_id: u64,
    url: String,
    size: u64,
    path: PathBuf,
}

/// The songs kept on this computer: downloaded one after the other in the order
/// wanted (the one playing first), resuming where an earlier try stopped.
struct Downloads {
    dir: PathBuf,
    wanted: Arc<Mutex<Vec<Wanted>>>,
    done: Arc<Mutex<HashSet<u64>>>,
    /// Set when the wanted list changes, so the worker re-checks its order.
    changed: Arc<tokio::sync::Notify>,
    worker: Option<JoinHandle<()>>,
}

impl Downloads {
    fn new(dir: PathBuf) -> Self {
        Self { dir, wanted: Default::default(), done: Default::default(), changed: Default::default(), worker: None }
    }

    fn path_of(&self, audio: &AudioStream) -> PathBuf {
        self.dir.join(format!("{}.{}", audio.queue_id, extension(&audio.mime, &audio.url)))
    }

    /// `list`: (stream, absolute URL), most urgent first. Files no longer wanted are deleted.
    fn want(&mut self, list: Vec<(AudioStream, String)>) {
        let wanted: Vec<Wanted> = list
            .into_iter()
            .map(|(a, url)| Wanted { path: self.path_of(&a), queue_id: a.queue_id, url, size: a.size })
            .collect();
        {
            let mut current = self.wanted.lock().unwrap();
            if *current == wanted {
                return;
            }
            *current = wanted.clone();
        }
        let keep: HashSet<PathBuf> = wanted.iter().flat_map(|w| [w.path.clone(), part(&w.path)]).collect();
        if let Ok(entries) = std::fs::read_dir(&self.dir) {
            for e in entries.flatten() {
                if !keep.contains(&e.path()) {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
        let _ = std::fs::create_dir_all(&self.dir);
        let ids: HashSet<u64> = wanted.iter().map(|w| w.queue_id).collect();
        self.done.lock().unwrap().retain(|id| ids.contains(id));
        self.changed.notify_one();
        if self.worker.as_ref().is_none_or(|w| w.is_finished()) {
            self.worker = Some(tokio::spawn(work(self.wanted.clone(), self.done.clone(), self.changed.clone())));
        }
    }

    fn file(&self, queue_id: u64) -> Option<PathBuf> {
        if !self.done.lock().unwrap().contains(&queue_id) {
            return None;
        }
        self.wanted.lock().unwrap().iter().find(|w| w.queue_id == queue_id).map(|w| w.path.clone())
    }

    fn stop(&mut self) {
        if let Some(w) = self.worker.take() {
            w.abort();
        }
        self.wanted.lock().unwrap().clear();
        self.done.lock().unwrap().clear();
    }
}

fn part(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".part");
    PathBuf::from(name)
}

async fn work(wanted: Arc<Mutex<Vec<Wanted>>>, done: Arc<Mutex<HashSet<u64>>>, changed: Arc<tokio::sync::Notify>) {
    let client = match reqwest::Client::builder().connect_timeout(HTTP_TIMEOUT).build() {
        Ok(c) => c,
        Err(e) => return tracing::warn!("party: no HTTP client: {e}"),
    };
    let mut failures = 0u32;
    loop {
        let next = {
            let done = done.lock().unwrap();
            wanted.lock().unwrap().iter().find(|w| !done.contains(&w.queue_id)).cloned()
        };
        let Some(item) = next else {
            changed.notified().await;
            continue;
        };
        match fetch(&client, &item, &wanted).await {
            Ok(true) => {
                failures = 0;
                done.lock().unwrap().insert(item.queue_id);
            }
            // something more urgent came up: the partial file stays for later
            Ok(false) => {}
            Err(e) => {
                failures += 1;
                tracing::debug!("party: download of {} failed: {e}", item.queue_id);
                // back off, but come back at once when the list changes
                let wait = Duration::from_millis(500 * u64::from(failures.min(10)));
                let _ = tokio::time::timeout(wait, changed.notified()).await;
            }
        }
    }
}

/// Downloads `item` (resuming a partial file). `Ok(false)` when it stopped
/// because another song became more urgent.
async fn fetch(client: &reqwest::Client, item: &Wanted, wanted: &Mutex<Vec<Wanted>>) -> Result<bool, String> {
    if item.size > 0 && std::fs::metadata(&item.path).is_ok_and(|m| m.len() == item.size) {
        return Ok(true);
    }
    let tmp = part(&item.path);
    let have = std::fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0);
    let mut request = client.get(&item.url);
    if have > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={have}-"));
    }
    let mut response = request.send().await.map_err(crate::commands::err)?;
    let resumed = response.status() == reqwest::StatusCode::PARTIAL_CONTENT;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(resumed)
        .truncate(!resumed)
        .open(&tmp)
        .await
        .map_err(crate::commands::err)?;
    let mut written = if resumed { have } else { 0 };
    let mut since_check = 0u64;
    loop {
        let chunk = tokio::time::timeout(HTTP_TIMEOUT, response.chunk())
            .await
            .map_err(|_| "the hub stopped sending".to_owned())?
            .map_err(crate::commands::err)?;
        let Some(chunk) = chunk else { break };
        file.write_all(&chunk).await.map_err(crate::commands::err)?;
        written += chunk.len() as u64;
        since_check += chunk.len() as u64;
        // every 512 KiB: the song playing now changed? download that one first
        if since_check >= 512 * 1024 {
            since_check = 0;
            if more_urgent(wanted, item.queue_id) {
                file.flush().await.map_err(crate::commands::err)?;
                return Ok(false);
            }
        }
    }
    file.flush().await.map_err(crate::commands::err)?;
    drop(file);
    if item.size > 0 && written != item.size {
        return Err(format!("got {written} of {} bytes", item.size));
    }
    tokio::fs::rename(&tmp, &item.path).await.map_err(crate::commands::err)?;
    Ok(true)
}

/// `queue_id` is no longer wanted, or the first song wanted is another one that isn't here yet.
fn more_urgent(wanted: &Mutex<Vec<Wanted>>, queue_id: u64) -> bool {
    let list = wanted.lock().unwrap();
    if !list.iter().any(|w| w.queue_id == queue_id) {
        return true;
    }
    list.first().is_some_and(|w| {
        w.queue_id != queue_id && !(w.size > 0 && std::fs::metadata(&w.path).is_ok_and(|m| m.len() == w.size))
    })
}

/// The party player of a guest. Lives behind a mutex in the guest session;
/// every method returns at once (the engine runs on its own thread).
pub struct PartyPlayer {
    player: Option<Player>,
    /// The engine's last snapshot and when it came, for a precise position.
    last: Arc<Mutex<Option<(Instant, Snapshot)>>>,
    downloads: Downloads,
    loaded: Option<u64>,
    sync: SyncState,
    ready_for: Option<u64>,
    start: Option<JoinHandle<()>>,
    /// The scheduled start fired: the next tick counts it as a jump.
    started: Arc<AtomicBool>,
    volume: f32,
    device: Option<String>,
    /// The app's equalizer, so the party sounds like the rest of the music here.
    eq: EqSettings,
}

impl PartyPlayer {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            player: None,
            last: Default::default(),
            downloads: Downloads::new(dir),
            loaded: None,
            sync: SyncState::default(),
            ready_for: None,
            start: None,
            started: Default::default(),
            volume: 0.8,
            device: None,
            eq: EqSettings::default(),
        }
    }

    /// Plays a song here right now.
    pub fn active(&self) -> bool {
        self.player.is_some() && self.loaded.is_some()
    }

    pub fn playing(&self) -> bool {
        self.active() && self.snapshot().is_some_and(|(_, s)| s.status == Status::Playing)
    }

    /// What reaches this computer's output (codec, rates, device), for the signal chain.
    pub fn stream(&self) -> Option<homefi_player::StreamInfo> {
        self.snapshot().and_then(|(_, s)| s.stream)
    }

    pub fn visual(&self, bands: usize) -> Option<Visual> {
        self.player.as_ref().filter(|_| self.loaded.is_some()).map(|p| p.visual(bands))
    }

    /// Same volume and output as the app's own player.
    pub fn set_output(&mut self, volume: f32, device: Option<String>) {
        if let Some(p) = &self.player {
            if (volume - self.volume).abs() > f32::EPSILON {
                p.set_volume(volume);
            }
            if device != self.device {
                p.set_device(device.clone());
            }
        }
        self.volume = volume;
        self.device = device;
    }

    /// Same equalizer as the app's own player.
    pub fn set_eq(&mut self, eq: &EqSettings) {
        if *eq == self.eq {
            return;
        }
        self.eq = eq.clone();
        if let Some(p) = &self.player {
            p.set_eq(eq.clone());
        }
    }

    fn snapshot(&self) -> Option<(Instant, Snapshot)> {
        self.last.lock().unwrap().clone()
    }

    /// Where the engine is now: its last report plus the time since, while playing.
    fn position(&self) -> Option<(Status, u64)> {
        let (at, s) = self.snapshot()?;
        let extra = if s.status == Status::Playing { at.elapsed().as_millis() as u64 } else { 0 };
        Some((s.status, s.position_ms + extra))
    }

    fn player(&mut self) -> Player {
        if let Some(p) = &self.player {
            return p.clone();
        }
        let last = self.last.clone();
        let listener = Arc::new(move |s: &Snapshot| *last.lock().unwrap() = Some((Instant::now(), s.clone())));
        let p = Player::new(self.volume, self.device.clone(), listener);
        p.set_eq(self.eq.clone());
        self.player = Some(p.clone());
        p
    }

    /// One sync step (every 250 ms). `delay_ms` is this computer's speaker latency;
    /// `preload` downloads the next and previous songs. Returns `true` when the
    /// party just started playing here.
    pub fn tick(&mut self, guest: &GuestClient, state: &State, delay_ms: i32, preload: bool) -> bool {
        let Some(audio) = state.audio.clone() else {
            self.pause();
            return false;
        };
        let mut list = vec![(audio.clone(), guest.resolve_url(&audio.url))];
        if preload {
            list.extend(state.prefetch.iter().map(|a| (a.clone(), guest.resolve_url(&a.url))));
        }
        self.downloads.want(list);

        let target = (guest.expected_position_ms(state) as i64 + i64::from(delay_ms)).max(0) as u64;
        let hub_now = guest.hub_now_ms();
        let starts_in = match hub_now {
            Some(now) if state.playing => state.clock_ms as i64 - now as i64,
            _ => 0,
        };

        if self.loaded != Some(audio.queue_id) {
            // the song isn't here yet: keep waiting (the old one stops)
            let Some(path) = self.downloads.file(audio.queue_id) else {
                if let Some(p) = &self.player {
                    p.pause();
                }
                return false;
            };
            let first = self.loaded.is_none();
            self.cancel_start();
            let player = self.player();
            self.loaded = Some(audio.queue_id);
            *self.last.lock().unwrap() = None;
            if starts_in > 0 || !state.playing {
                // load at the start, paused and silent, so it can start at once
                player.set_volume(0.0);
                player.load(vec![path], 0, target, true);
                player.pause();
                player.set_volume(self.volume);
            } else {
                player.load(vec![path], 0, target + self.sync.seek_lead_ms as u64, true);
            }
            self.sync.jumped();
            return first;
        }
        let Some(player) = self.player.clone() else { return false };
        if self.started.swap(false, Ordering::SeqCst) {
            self.sync.jumped();
        }
        // the engine hasn't reported since the last command
        let Some((status, actual)) = self.position() else { return false };

        if starts_in > 0 {
            // a new song scheduled for everyone at once: wait at the start, say we're ready
            if status == Status::Playing {
                player.pause();
            }
            if (actual as i64 - target as i64).abs() > LEAD_TOLERANCE_MS {
                player.seek(target);
            }
            if self.ready_for != Some(audio.queue_id) {
                self.ready_for = Some(audio.queue_id);
                let _ = guest.send(GuestMessage::Ready { queue_id: audio.queue_id });
            }
            if starts_in <= START_AHEAD_MS {
                self.start_at(starts_in as u64, player);
            }
            return false;
        }
        if !state.playing {
            if status == Status::Playing {
                player.pause();
            }
            if (actual as i64 - target as i64).abs() > PAUSED_TOLERANCE_MS {
                player.seek(target);
            }
            return false;
        }
        if status != Status::Playing {
            // the hub plays (again): resume where it is
            if (actual as i64 - target as i64).abs() > PAUSED_TOLERANCE_MS {
                player.seek(target + self.sync.seek_lead_ms as u64);
            }
            player.play();
            self.sync.jumped();
            return false;
        }
        if let Some(to) = self.sync.step(Instant::now(), target, actual) {
            player.seek(to);
        }
        false
    }

    /// Starts the loaded song in `in_ms`, to the millisecond rather than on the next tick.
    fn start_at(&mut self, in_ms: u64, player: Player) {
        if self.start.as_ref().is_some_and(|s| !s.is_finished()) {
            return;
        }
        let started = self.started.clone();
        self.start = Some(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(in_ms)).await;
            player.play();
            started.store(true, Ordering::SeqCst);
        }));
    }

    fn cancel_start(&mut self) {
        if let Some(s) = self.start.take() {
            s.abort();
        }
    }

    pub fn pause(&mut self) {
        self.cancel_start();
        if let Some(p) = &self.player {
            p.pause();
        }
    }

    /// Stops and deletes the songs kept here (leaving the party, or "play here" off).
    pub fn release(&mut self) {
        self.cancel_start();
        self.downloads.stop();
        if let Some(p) = self.player.take() {
            p.stop();
        }
        self.loaded = None;
        self.ready_for = None;
        self.sync = SyncState::default();
        *self.last.lock().unwrap() = None;
        let _ = std::fs::remove_dir_all(&self.downloads.dir);
    }
}

impl Drop for PartyPlayer {
    fn drop(&mut self) {
        self.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn small_gaps_are_left_alone_big_steady_ones_jump() {
        let t0 = Instant::now();
        let mut s = SyncState::default();
        // within the speed range but under the seek-only threshold: nothing
        for i in 0..10 {
            assert_eq!(s.step(t0 + ms(i * 250), 10_000, 9_950), None);
        }
        // 200 ms behind: only once enough samples agree (the median moves after 4 of 7)
        let mut s = SyncState::default();
        assert_eq!(s.step(t0, 10_000, 9_800), None);
        assert_eq!(s.step(t0 + ms(250), 10_250, 10_050), None);
        assert_eq!(s.step(t0 + ms(500), 10_500, 10_300), None);
        assert_eq!(s.step(t0 + ms(750), 10_750, 10_550), Some(10_750));
    }

    #[test]
    fn one_outlier_does_not_move_the_median() {
        let t0 = Instant::now();
        let mut s = SyncState::default();
        for i in 0..6 {
            assert_eq!(s.step(t0 + ms(i * 250), 10_000 + i * 250, 10_000 + i * 250 - 5), None);
        }
        // a late clock sample says 900 ms: ignored
        assert_eq!(s.step(t0 + ms(1500), 11_500, 10_600), None);
    }

    #[test]
    fn jumps_settle_then_teach_the_lead() {
        let t0 = Instant::now();
        let mut s = SyncState::default();
        s.jumped();
        // settling: nothing counts
        assert_eq!(s.step(t0, 5_000, 1_000), None);
        assert_eq!(s.step(t0 + ms(600), 5_600, 1_600), None);
        // landed 120 ms late: the next jump aims 120 ms further
        assert_eq!(s.step(t0 + ms(700), 5_700, 5_580), None);
        assert_eq!(s.seek_lead_ms, 120);
        for i in 1..4 {
            let at = t0 + ms(700 + i * 250);
            let target = 5_700 + i * 250;
            let got = s.step(at, target, target - 2_000);
            if i < 3 {
                assert_eq!(got, None);
            } else {
                assert_eq!(got, Some(target + 120));
            }
        }
        // a lead never goes negative
        let mut s = SyncState::default();
        s.jumped();
        s.step(t0, 1_000, 1_000);
        s.step(t0 + ms(700), 1_000, 2_000);
        assert_eq!(s.seek_lead_ms, 0);
    }

    /// A short 16-bit stereo WAV.
    fn wav(path: &Path, seconds: u32) -> Vec<u8> {
        let rate = 44_100u32;
        let frames = rate * seconds;
        let data_len = frames * 4;
        let mut b = Vec::with_capacity(44 + data_len as usize);
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + data_len).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&rate.to_le_bytes());
        b.extend_from_slice(&(rate * 4).to_le_bytes());
        b.extend_from_slice(&4u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&data_len.to_le_bytes());
        for i in 0..frames {
            let v = ((i as f32 * 440.0 * 2.0 * std::f32::consts::PI / rate as f32).sin() * 3000.0) as i16;
            b.extend_from_slice(&v.to_le_bytes());
            b.extend_from_slice(&v.to_le_bytes());
        }
        std::fs::write(path, &b).unwrap();
        b
    }

    struct NoHost;

    impl homefi_core::share::HubHost for NoHost {
        fn search(
            &self,
            _q: String,
        ) -> futures::future::BoxFuture<'_, homefi_core::Result<homefi_core::share::SearchResults>> {
            Box::pin(async { Ok(Default::default()) })
        }
        fn browse(
            &self,
            _kind: homefi_core::share::BrowseKind,
            _id: String,
        ) -> futures::future::BoxFuture<'_, homefi_core::Result<homefi_core::share::Page>> {
            Box::pin(async { Ok(Default::default()) })
        }
        fn cover(&self, _id: String) -> futures::future::BoxFuture<'_, Option<homefi_core::share::CoverImage>> {
            Box::pin(async { None })
        }
        fn command(&self, _command: homefi_core::share::HostCommand) {}
    }

    /// The whole guest path against a real hub on this machine: join, clock,
    /// download the song (resuming a partial file), load it at the scheduled start
    /// and tell the hub it is ready. Runs without an audio device too (the engine
    /// then reports an error, the download and `ready` still happen).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn plays_along_with_a_hub_on_this_machine() {
        use homefi_core::share::{GuestEvent, Hub, HubConfig, HubMessage, ShareTrack, Source};

        let tmp = tempfile::tempdir().unwrap();
        let song = tmp.path().join("song.wav");
        let bytes = wav(&song, 2);
        let mut config = HubConfig::new("Test party", "Host");
        config.bind = std::net::SocketAddr::from(([127, 0, 0, 1], 0));
        config.stream_audio = true;
        let hub = Hub::start(config, Arc::new(NoHost)).await.unwrap();
        let track =
            ShareTrack { source: Source::Hub(song.display().to_string()), title: "Tone".into(), ..Default::default() };
        hub.set_now_playing(None, Some(track), 0, true);
        hub.set_audio(Some(song.clone()));

        let (guest, welcome, mut rx) =
            GuestClient::connect("127.0.0.1", hub.local_addr().port(), "Guest", "#0891b2", &hub.code()).await.unwrap();
        assert_eq!(welcome.session_name, "Test party");
        let queue_id = welcome.state.audio.as_ref().expect("streaming").queue_id;
        // the clock is known after the first answers
        for _ in 0..40 {
            if guest.hub_now_ms().is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(guest.hub_now_ms().is_some(), "no clock sync");

        // a new song for everyone, 3 s ahead
        hub.schedule_start(3_000);
        let mut state = welcome.state;
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
                Ok(Some(GuestEvent::Message(HubMessage::State(s)))) => {
                    let scheduled = s.clock_ms > guest.hub_now_ms().unwrap_or(0);
                    state = s;
                    if scheduled {
                        break;
                    }
                }
                Ok(Some(_)) => {}
                _ => break,
            }
        }
        assert!(state.clock_ms > guest.hub_now_ms().unwrap(), "the start is in the future");

        // half the file is already here from an earlier try: the rest comes with a range request
        let dir = tmp.path().join("party");
        std::fs::create_dir_all(&dir).unwrap();
        let mut party = PartyPlayer::new(dir.clone());
        let audio = state.audio.clone().unwrap();
        let file = party.downloads.path_of(&audio);
        std::fs::write(part(&file), &bytes[..bytes.len() / 2]).unwrap();

        let deadline = Instant::now() + Duration::from_secs(8);
        while Instant::now() < deadline && hub.ready_guests(queue_id).0 == 0 {
            party.tick(&guest, &state, 0, true);
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert_eq!(std::fs::read(&file).unwrap(), bytes, "downloaded intact");
        assert_eq!(party.loaded, Some(queue_id));
        assert_eq!(hub.ready_guests(queue_id), (1, 1), "the guest said it is ready");

        party.release();
        assert!(!dir.exists(), "the party's songs are deleted on leaving");
        guest.close();
        hub.stop().await;
    }

    #[test]
    fn served_songs_get_a_decoder_friendly_name() {
        assert_eq!(extension("audio/flac", "/audio/1?k=x"), "flac");
        assert_eq!(extension("audio/mp4", ""), "m4a");
        assert_eq!(extension("application/octet-stream", "/a.flac"), "flac");
        assert_eq!(part(Path::new("/c/7.flac")), PathBuf::from("/c/7.flac.part"));
    }
}
