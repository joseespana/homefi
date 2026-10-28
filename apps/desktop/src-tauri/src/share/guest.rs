//! This computer as a guest of someone else's hub (Android's `ShareManager`
//! guest side, ported): remote control of the hub's queue and, in a party,
//! the hub's music played here in sync ([`PartyPlayer`]).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use homefi_core::share::{GuestClient, GuestEvent, GuestMessage, HubMessage, Page, State};
use homefi_player::{Status, Visual};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::party::PartyPlayer;
use super::{emit_guest, notice, refresh_discovery, Share};
use crate::commands::AppState;

const PARTY_TICK: Duration = Duration::from_millis(250);
/// The user's own music right after the party started is the app's player still pausing.
const OWN_MUSIC_GRACE: Duration = Duration::from_secs(2);
/// Hub levels older than this: the meters fall back to rest.
const LEVELS_FRESH: Duration = Duration::from_millis(600);

/// The hub's last `levels` and when they came: left, right, bands.
type Levels = (Instant, u8, u8, Vec<u8>);

/// How the guest side looks to the UI.
#[derive(Debug, Clone, Default, Serialize)]
pub struct GuestView {
    /// `idle`, `connecting`, `connected` or `disconnected`.
    pub status: &'static str,
    pub session_name: String,
    pub guest_id: String,
    pub host: String,
    pub port: u16,
    /// The hub's state, covers made absolute.
    pub state: Option<State>,
    /// Where the hub's song is right now (on the hub's clock).
    pub position_ms: u64,
    /// Why joining failed or the hub closed the connection.
    pub error: Option<String>,
    /// "Play on this computer" (party mode).
    pub play_along: bool,
    pub playing_here: bool,
    /// Clock precision: the round trip of the sample in use.
    pub round_trip_ms: Option<u64>,
    /// What reaches this computer's output while playing here (signal chain).
    pub stream: Option<homefi_player::StreamInfo>,
}

pub struct GuestSession {
    pub client: GuestClient,
    pub session_name: String,
    pub guest_id: String,
    host: String,
    port: u16,
    /// Bumped on leaving: late events of this session are ignored.
    pub seq: u64,
    state: Mutex<State>,
    play_along: AtomicBool,
    playing_here: AtomicBool,
    party: Mutex<PartyPlayer>,
    levels: Mutex<Option<Levels>>,
    party_started: Mutex<Option<Instant>>,
    ready_sent: Mutex<Option<u64>>,
    tasks: Mutex<Vec<JoinHandle<()>>>,
}

impl GuestSession {
    /// Joins the hub at `host:port`. Fails with the hub's reason ("Wrong join code", ...).
    pub async fn join(
        app: &AppHandle,
        seq: u64,
        host: &str,
        port: u16,
        name: &str,
        color: &str,
        code: &str,
    ) -> Result<Arc<Self>, String> {
        let (client, welcome, rx) =
            GuestClient::connect(host, port, name, color, code).await.map_err(crate::commands::err)?;
        let state = app.state::<AppState>();
        let dir = state.settings_path.with_file_name("party-cache");
        let session = Arc::new(Self {
            client,
            session_name: welcome.session_name,
            guest_id: welcome.guest_id,
            host: host.to_owned(),
            port,
            seq,
            state: Mutex::new(welcome.state),
            play_along: AtomicBool::new(state.settings().share.play_along),
            playing_here: AtomicBool::new(false),
            party: Mutex::new(PartyPlayer::new(dir)),
            levels: Mutex::new(None),
            party_started: Mutex::new(None),
            ready_sent: Mutex::new(None),
            tasks: Mutex::new(Vec::new()),
        });
        let tasks = vec![
            tokio::spawn(pump(app.clone(), session.clone(), rx)),
            tokio::spawn(party_loop(app.clone(), session.clone())),
        ];
        *session.tasks.lock().unwrap() = tasks;
        Ok(session)
    }

    /// Leaves: closes the connection, stops the party here and deletes its songs.
    pub fn close(&self) {
        for t in self.tasks.lock().unwrap().drain(..) {
            t.abort();
        }
        self.party.lock().unwrap().release();
        self.client.close();
    }

    pub fn send(&self, msg: GuestMessage) -> Result<(), String> {
        self.client.send(msg).map_err(crate::commands::err)
    }

    pub fn set_play_along(&self, on: bool) {
        self.play_along.store(on, Ordering::SeqCst);
        if !on {
            self.party.lock().unwrap().release();
            self.playing_here.store(false, Ordering::SeqCst);
        }
    }

    pub fn set_volume(&self, volume: f32, device: Option<String>) {
        self.party.lock().unwrap().set_output(volume, device);
    }

    pub fn playing_here(&self) -> bool {
        self.playing_here.load(Ordering::SeqCst)
    }

    /// Hub paths (covers) made absolute, for the UI.
    fn resolved(&self, mut state: State) -> State {
        let fix = |url: &mut Option<String>| {
            if let Some(u) = url.as_mut() {
                *u = self.client.resolve_url(u);
            }
        };
        if let Some(now) = state.now_playing.as_mut() {
            fix(&mut now.track.cover_url);
        }
        for item in &mut state.queue {
            fix(&mut item.track.cover_url);
        }
        state
    }

    pub fn view(&self) -> GuestView {
        let state = self.state.lock().unwrap().clone();
        let party = self.party.lock().unwrap();
        let here = self.playing_here();
        GuestView {
            status: "connected",
            session_name: self.session_name.clone(),
            guest_id: self.guest_id.clone(),
            host: self.host.clone(),
            port: self.port,
            position_ms: self.client.expected_position_ms(&state),
            state: Some(self.resolved(state)),
            error: None,
            play_along: self.play_along.load(Ordering::SeqCst),
            playing_here: here,
            round_trip_ms: self.client.clock_round_trip_ms(),
            stream: if here { party.stream() } else { None },
        }
    }

    /// Meters for the room: this computer's party player while it plays here,
    /// otherwise the hub's levels.
    pub fn visual(&self, bands: usize) -> Visual {
        if self.playing_here() {
            if let Some(v) = self.party.lock().unwrap().visual(bands) {
                return v;
            }
        }
        let levels = self.levels.lock().unwrap().clone();
        match levels {
            Some((at, l, r, b)) if at.elapsed() < LEVELS_FRESH => levels_visual(l, r, &b, bands),
            _ => Visual { left: 0.0, right: 0.0, spectrum: vec![0.0; bands], wave: vec![0.0; 256] },
        }
    }
}

/// The hub's `levels` (0..=255, its own band count) as a visualizer frame with
/// `bands` bands; the waveform is made up from the spectrum so the scope moves too.
pub fn levels_visual(l: u8, r: u8, b: &[u8], bands: usize) -> Visual {
    let spectrum: Vec<f32> = (0..bands)
        .map(|i| {
            if b.is_empty() {
                return 0.0;
            }
            // linear between the hub's bands
            let x = if bands > 1 { i as f32 * (b.len() - 1) as f32 / (bands - 1) as f32 } else { 0.0 };
            let (lo, hi) = (x.floor() as usize, (x.ceil() as usize).min(b.len() - 1));
            let f = x - lo as f32;
            (f32::from(b[lo]) * (1.0 - f) + f32::from(b[hi]) * f) / 255.0
        })
        .collect();
    let t = EPOCH.elapsed().as_secs_f32();
    let bass = spectrum.iter().take(bands / 6 + 1).copied().fold(0.0, f32::max);
    let mid = spectrum.iter().skip(bands / 3).take(bands / 3 + 1).copied().fold(0.0, f32::max);
    let wave = (0..256)
        .map(|i| {
            let x = i as f32 / 256.0;
            (0.5 * bass * (x * 12.0 + t * 9.0).sin() + 0.3 * mid * (x * 47.0 + t * 23.0).sin()).clamp(-1.0, 1.0)
        })
        .collect();
    Visual { left: f32::from(l) / 255.0, right: f32::from(r) / 255.0, spectrum, wave }
}

static EPOCH: std::sync::LazyLock<Instant> = std::sync::LazyLock::new(Instant::now);

/// Everything the hub sends, to the UI.
async fn pump(app: AppHandle, session: Arc<GuestSession>, mut rx: mpsc::UnboundedReceiver<GuestEvent>) {
    while let Some(event) = rx.recv().await {
        match event {
            GuestEvent::Message(HubMessage::State(state)) => {
                *session.state.lock().unwrap() = state;
                emit_guest(&app);
            }
            GuestEvent::Message(HubMessage::Results(mut results)) => {
                for t in &mut results.tracks {
                    t.cover_url = t.cover_url.as_ref().map(|u| session.client.resolve_url(u));
                }
                for c in results.albums.iter_mut().chain(&mut results.artists).chain(&mut results.playlists) {
                    c.cover_url = c.cover_url.as_ref().map(|u| session.client.resolve_url(u));
                }
                let _ = app.emit("share-results", &results);
            }
            GuestEvent::Message(HubMessage::Page(mut page)) => {
                resolve_page(&session, &mut page);
                let _ = app.emit("share-page", &page);
            }
            GuestEvent::Message(HubMessage::Notice { text }) => notice(&app, &text, None, "guest"),
            GuestEvent::Message(HubMessage::Denied { reason }) => {
                let _ = app.emit("share-denied", &reason);
                notice(&app, &reason, None, "error");
            }
            GuestEvent::Message(HubMessage::Levels { l, r, b }) => {
                *session.levels.lock().unwrap() = Some((Instant::now(), l, r, b));
            }
            GuestEvent::Message(_) => {}
            GuestEvent::Disconnected { reason } => {
                disconnected(&app, &session, reason);
                break;
            }
        }
    }
}

fn resolve_page(session: &GuestSession, page: &mut Page) {
    page.header.cover_url = page.header.cover_url.as_ref().map(|u| session.client.resolve_url(u));
    for t in &mut page.tracks {
        t.cover_url = t.cover_url.as_ref().map(|u| session.client.resolve_url(u));
    }
}

/// The hub closed the connection (kicked, stopped) or it was lost.
fn disconnected(app: &AppHandle, session: &Arc<GuestSession>, reason: Option<String>) {
    let share = app.state::<Share>();
    if share.end_guest(session.seq) {
        session.party.lock().unwrap().release();
        share.set_guest_view(GuestView {
            status: "disconnected",
            session_name: session.session_name.clone(),
            error: Some(reason.unwrap_or_else(|| "The connection to the hub was lost".into())),
            play_along: session.play_along.load(Ordering::SeqCst),
            ..Default::default()
        });
        emit_guest(app);
        refresh_discovery(app);
    }
}

/// Party playback follows the hub, 4 times a second.
async fn party_loop(app: AppHandle, session: Arc<GuestSession>) {
    let mut ticks = 0u32;
    let mut was_here = false;
    let mut had_player = false;
    loop {
        tokio::time::sleep(PARTY_TICK).await;
        ticks += 1;
        let state = app.state::<AppState>();
        let settings = state.settings();
        let s = &settings.share;
        let hub_state = session.state.lock().unwrap().clone();
        let play_along = session.play_along.load(Ordering::SeqCst);
        let own_playing = state.player.snapshot().status == Status::Playing;

        // the user put on their own music: stop playing along rather than both at once
        let settled = session.party_started.lock().unwrap().is_none_or(|t| t.elapsed() > OWN_MUSIC_GRACE);
        if had_player && settled && own_playing {
            session.set_play_along(false);
            had_player = false;
            notice(&app, "Your own music is playing: the party stopped on this computer", None, "guest");
            emit_guest(&app);
            continue;
        }

        // not playing along: never make the others wait for this computer
        let now_id = hub_state.now_playing.as_ref().map(|n| n.queue_id);
        {
            let mut sent = session.ready_sent.lock().unwrap();
            if now_id.is_some() && now_id != *sent && (!play_along || hub_state.audio.is_none()) {
                *sent = now_id;
                if let Some(id) = now_id {
                    let _ = session.send(GuestMessage::Ready { queue_id: id });
                }
            }
        }

        let started = {
            let mut party = session.party.lock().unwrap();
            if !play_along {
                if had_player {
                    party.release();
                    had_player = false;
                }
                false
            } else if hub_state.audio.is_none() {
                party.pause();
                false
            } else {
                party.set_output(settings.volume, settings.output_device.clone());
                party.set_eq(&settings.eq);
                let started = party.tick(&session.client, &hub_state, s.speaker_delay_ms, s.preload_songs);
                had_player |= party.active();
                started
            }
        };
        if started {
            *session.party_started.lock().unwrap() = Some(Instant::now());
            state.player.pause();
            notice(&app, "Playing the party on this computer", None, "guest");
        }
        let here =
            play_along && hub_state.audio.is_some() && hub_state.playing && session.party.lock().unwrap().playing();
        session.playing_here.store(here, Ordering::SeqCst);
        // the room's clock and position, now and then (and at once when this changes)
        if here != was_here || ticks % 4 == 0 {
            was_here = here;
            emit_guest(&app);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hub_levels_become_a_visualizer_frame() {
        let v = levels_visual(255, 0, &[0, 255], 3);
        assert_eq!((v.left, v.right), (1.0, 0.0));
        assert_eq!(v.spectrum.len(), 3);
        assert!((v.spectrum[1] - 0.5).abs() < 0.01, "{:?}", v.spectrum);
        assert_eq!(v.spectrum[2], 1.0);
        assert_eq!(v.wave.len(), 256);
        assert!(v.wave.iter().all(|w| (-1.0..=1.0).contains(w)));
        // nothing from the hub: flat
        let v = levels_visual(0, 0, &[], 4);
        assert_eq!(v.spectrum, vec![0.0; 4]);
    }
}
