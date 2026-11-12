//! The TV's HTTP server (page and `/ride.json`), its mDNS announcement and the client's discovery.
//! Separate from `share::Hub` on purpose: no guests, no code, just a read-only view of the ride.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, State};
use axum::http::{header, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use super::{is_lan, listen_permissions, render_page, RideTvView, TvFrame, TvLyrics, TvSession, TV_PROTOCOL_VERSION};
use crate::error::{Error, Result};
use crate::exercise::ride::RideView;
use crate::exercise::settings::DEFAULT_TV_PORT;
use crate::exercise::words::{self, Problem};
use crate::humane::Lang;
use crate::playlist::now_ms;
use crate::share::HubConfig;

/// DNS-SD service type of the ride TV.
pub const RIDE_SERVICE_TYPE: &str = "_homefi-ride._tcp.local.";
/// Longest the server waits to stop before giving up on the task.
const STOP_WAIT: Duration = Duration::from_secs(2);
const SERVICE_NAME_MAX_BYTES: usize = 63;

/// What the server needs to start.
#[derive(Debug, Clone)]
pub struct TvConfig {
    /// 1024..=65535; 0 picks a free one (tests). A port in use falls to the next one, once.
    pub port: u16,
    /// Shown by the page and announced on the network ("HomeFi ride").
    pub name: String,
    pub lang: Lang,
    /// Announce `_homefi-ride._tcp` with mDNS.
    pub announce: bool,
}

impl Default for TvConfig {
    fn default() -> Self {
        Self { port: DEFAULT_TV_PORT, name: "HomeFi ride".into(), lang: Lang::En, announce: true }
    }
}

/// The hub config of a ride played on the TV (audio `tv`): listen-only guests, the audio served, the join
/// code a one-ride token (see [`super::ride_token`]).
pub fn ride_hub_config(name: &str, port: u16, token: &str) -> HubConfig {
    let mut config = HubConfig::new(name, "HomeFi");
    config.code = token.to_owned();
    config.bind = SocketAddr::from(([0, 0, 0, 0], port));
    config.permissions = listen_permissions();
    config.stream_audio = true;
    config
}

struct Shared {
    session: RwLock<Arc<TvSession>>,
    frame: RwLock<Option<Arc<TvFrame>>>,
    /// The song's lyrics, apart from the session: they change with every song.
    lyrics: RwLock<Option<Arc<TvLyrics>>>,
    lang: Lang,
    page: String,
}

/// A running TV server. Dropping it stops the server (prefer [`TvServer::stop`]).
pub struct TvServer {
    shared: Arc<Shared>,
    addr: SocketAddr,
    shutdown: Mutex<Option<oneshot::Sender<()>>>,
    task: Mutex<Option<JoinHandle<()>>>,
    announcement: Mutex<Option<Announcement>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl TvServer {
    pub async fn start(config: TvConfig) -> Result<Self> {
        let listener = bind(config.port, config.lang).await?;
        let addr = listener.local_addr()?;
        let shared = Arc::new(Shared {
            session: RwLock::new(Arc::new(TvSession { title: config.name.clone(), ..TvSession::default() })),
            frame: RwLock::new(None),
            lyrics: RwLock::new(None),
            lang: config.lang,
            page: render_page(&config.name, config.lang),
        });
        let app = Router::new().route("/", get(page)).route("/ride.json", get(ride_json)).with_state(shared.clone());
        let (tx, rx) = oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            let service = app.into_make_service_with_connect_info::<SocketAddr>();
            let result = axum::serve(listener, service)
                .with_graceful_shutdown(async {
                    let _ = rx.await;
                })
                .await;
            if let Err(e) = result {
                tracing::warn!("ride TV server stopped: {e}");
            }
        });
        let announcement = config.announce.then(|| Announcement::start(&config.name, addr.port())).flatten();
        Ok(Self {
            shared,
            addr,
            shutdown: Mutex::new(Some(tx)),
            task: Mutex::new(Some(task)),
            announcement: Mutex::new(announcement),
        })
    }

    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    /// Where a TV browser opens the page: one URL per LAN address this computer has (the best guess first).
    pub fn url(&self) -> Option<String> {
        crate::share::local_ip_guess().map(|ip| format!("http://{}:{}/", host_text(ip), self.port()))
    }

    /// A ride starts: its title, profile and how the audio goes. Clears the last frame (not the lyrics: `begin` may be
    /// called again while the ride runs, e.g. when the hub is up).
    pub fn begin(&self, session: TvSession) {
        *self.shared.session.write().unwrap_or_else(|e| e.into_inner()) = Arc::new(session);
        *self.shared.frame.write().unwrap_or_else(|e| e.into_inner()) = None;
    }

    /// The lyrics of the song that plays now (`None` clears them: call it as soon as the song changes so a
    /// stale text never shows). Cheap: the handlers clone an `Arc`.
    pub fn set_lyrics(&self, lyrics: Option<TvLyrics>) {
        *self.shared.lyrics.write().unwrap_or_else(|e| e.into_inner()) = lyrics.map(Arc::new);
    }

    /// The newest state of the ride. Cheap: the handlers clone an `Arc`.
    pub fn publish(&self, view: RideView, playing: bool) {
        let frame = Arc::new(TvFrame { view, playing, sampled_ms: now_ms() });
        *self.shared.frame.write().unwrap_or_else(|e| e.into_inner()) = Some(frame);
    }

    /// The ride is over: the page goes back to waiting.
    pub fn clear(&self) {
        *self.shared.frame.write().unwrap_or_else(|e| e.into_inner()) = None;
        self.set_lyrics(None);
    }

    pub async fn stop(self) {
        drop(lock(&self.announcement).take());
        if let Some(tx) = lock(&self.shutdown).take() {
            let _ = tx.send(());
        }
        let task = lock(&self.task).take();
        if let Some(mut task) = task {
            if tokio::time::timeout(STOP_WAIT, &mut task).await.is_err() {
                task.abort();
            }
        }
    }
}

impl Drop for TvServer {
    fn drop(&mut self) {
        if let Some(task) = lock(&self.task).take() {
            task.abort();
        }
    }
}

/// `port`, then the next one once; port 0 is always free.
async fn bind(port: u16, lang: Lang) -> Result<tokio::net::TcpListener> {
    let try_bind = |p: u16| tokio::net::TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], p)));
    match try_bind(port).await {
        Ok(listener) => Ok(listener),
        Err(first) if first.kind() == std::io::ErrorKind::AddrInUse && port != 0 => match port.checked_add(1) {
            Some(next) => try_bind(next).await.map_err(|_| words::problem(lang, Problem::TvBusy(port))),
            None => Err(words::problem(lang, Problem::TvBusy(port))),
        },
        Err(e) => Err(Error::Io(e)),
    }
}

fn host_text(ip: IpAddr) -> String {
    match ip {
        IpAddr::V6(v6) => format!("[{v6}]"),
        v4 => v4.to_string(),
    }
}

fn no_store(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn page(State(shared): State<Arc<Shared>>) -> Response {
    no_store(([(header::CONTENT_TYPE, "text/html; charset=utf-8")], shared.page.clone()).into_response())
}

async fn ride_json(State(shared): State<Arc<Shared>>, ConnectInfo(peer): ConnectInfo<SocketAddr>) -> Response {
    let session = shared.session.read().unwrap_or_else(|e| e.into_inner()).clone();
    let frame = shared.frame.read().unwrap_or_else(|e| e.into_inner()).clone();
    let lyrics = shared.lyrics.read().unwrap_or_else(|e| e.into_inner()).clone();
    let mut view = RideTvView::build(&session, frame.as_deref(), now_ms(), shared.lang, is_lan(peer.ip()));
    // only while a ride shows: the idle page has no song
    view.lyrics = lyrics.filter(|_| view.ride.is_some()).map(|l| (*l).clone());
    no_store(
        (
            [
                (header::CONTENT_TYPE, "application/json"),
                // a page served elsewhere (a casting receiver) may read it too
                (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
            ],
            view.to_json(),
        )
            .into_response(),
    )
}

// -------------------------------------------------------------------- mDNS

fn fit_bytes(text: &str, max: usize) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if out.len() + c.len_utf8() > max {
            break;
        }
        out.push(c);
    }
    out
}

fn host_label(name: &str) -> String {
    let label: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let label = fit_bytes(&label, 50);
    format!("{}.local.", if label.is_empty() { "homefi-ride" } else { &label })
}

/// The TXT records: protocol version, display name and port. Never the audio token.
fn txt_records(name: &str, port: u16) -> HashMap<String, String> {
    HashMap::from([
        ("v".to_owned(), TV_PROTOCOL_VERSION.to_string()),
        ("name".to_owned(), fit_bytes(name, 200)),
        ("port".to_owned(), port.to_string()),
    ])
}

/// The ride TV on the network until dropped.
struct Announcement {
    daemon: ServiceDaemon,
    fullname: String,
}

impl Announcement {
    fn start(name: &str, port: u16) -> Option<Self> {
        let daemon = ServiceDaemon::new().map_err(|e| tracing::warn!("ride TV: mDNS unavailable: {e}")).ok()?;
        let instance = fit_bytes(&name.replace('.', " "), SERVICE_NAME_MAX_BYTES);
        let instance = if instance.trim().is_empty() { "HomeFi ride".to_owned() } else { instance };
        let info = ServiceInfo::new(RIDE_SERVICE_TYPE, &instance, &host_label(name), (), port, txt_records(name, port))
            .map(ServiceInfo::enable_addr_auto)
            .map_err(|e| tracing::warn!("ride TV: bad mDNS record: {e}"))
            .ok()?;
        let fullname = info.get_fullname().to_owned();
        daemon.register(info).map_err(|e| tracing::warn!("ride TV: could not announce: {e}")).ok()?;
        Some(Self { daemon, fullname })
    }
}

impl Drop for Announcement {
    fn drop(&mut self) {
        let _ = self.daemon.unregister(&self.fullname);
        let _ = self.daemon.shutdown();
    }
}

/// A ride TV found on the network (what an Android TV client lists).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RideTvEndpoint {
    pub name: String,
    pub host: String,
    pub port: u16,
    /// The protocol version it announced.
    pub version: u8,
}

impl RideTvEndpoint {
    /// `http://host:port/ride.json`.
    pub fn ride_url(&self) -> String {
        let host = match self.host.parse::<IpAddr>() {
            Ok(ip) => host_text(ip),
            Err(_) => self.host.clone(),
        };
        format!("http://{host}:{}/ride.json", self.port)
    }
}

/// Looks for ride TVs for `timeout` and returns what answered (IPv4 first, deduplicated).
pub async fn discover(timeout: Duration) -> Vec<RideTvEndpoint> {
    tokio::task::spawn_blocking(move || discover_blocking(timeout)).await.unwrap_or_default()
}

fn discover_blocking(timeout: Duration) -> Vec<RideTvEndpoint> {
    let Ok(daemon) = ServiceDaemon::new() else { return Vec::new() };
    let Ok(events) = daemon.browse(RIDE_SERVICE_TYPE) else {
        let _ = daemon.shutdown();
        return Vec::new();
    };
    let deadline = Instant::now() + timeout;
    let mut found: Vec<RideTvEndpoint> = Vec::new();
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match events.recv_timeout(left) {
            Ok(ServiceEvent::ServiceResolved(service)) => {
                let addresses: Vec<IpAddr> = service.get_addresses().iter().map(|a| a.to_ip_addr()).collect();
                let Some(host) = addresses.iter().find(|a| a.is_ipv4()).or_else(|| addresses.first()) else { continue };
                let port = match service.get_port() {
                    0 => match service.get_property_val_str("port").and_then(|p| p.parse().ok()) {
                        Some(p) => p,
                        None => continue,
                    },
                    p => p,
                };
                let endpoint = RideTvEndpoint {
                    name: service.get_property_val_str("name").unwrap_or("HomeFi ride").to_owned(),
                    host: host.to_string(),
                    port,
                    version: service.get_property_val_str("v").and_then(|v| v.parse().ok()).unwrap_or(1),
                };
                found.retain(|e| (&e.host, e.port) != (&endpoint.host, endpoint.port));
                found.push(endpoint);
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    let _ = daemon.stop_browse(RIDE_SERVICE_TYPE);
    let _ = daemon.shutdown();
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_announcement_never_carries_the_token() {
        let txt = txt_records("Sala de José", 47801);
        assert_eq!(txt.len(), 3);
        assert_eq!((txt["v"].as_str(), txt["port"].as_str()), ("1", "47801"));
        assert_eq!(host_label("Sala de José"), "sala-de-jos.local.");
        assert_eq!(
            RideTvEndpoint { name: "x".into(), host: "::1".into(), port: 9, version: 1 }.ride_url(),
            "http://[::1]:9/ride.json"
        );
    }

    #[tokio::test]
    async fn the_server_serves_the_page_and_the_ride() {
        use crate::exercise::builder::{test_plan, CueSettings};
        use crate::exercise::model::{Segment, Workout};
        use crate::exercise::ride::RideClock;
        use crate::exercise::settings::TvAudio;
        let server = TvServer::start(TvConfig { port: 0, announce: false, ..TvConfig::default() }).await.unwrap();
        let base = format!("http://127.0.0.1:{}", server.port());
        let http = reqwest::Client::new();

        let page = http.get(format!("{base}/")).send().await.unwrap();
        assert_eq!(page.headers()["cache-control"], "no-store");
        assert!(page.text().await.unwrap().contains("/ride.json"));

        let idle: RideTvView = http.get(format!("{base}/ride.json")).send().await.unwrap().json().await.unwrap();
        assert!(idle.ride.is_none() && idle.v == 1);

        let plan =
            test_plan(Workout { segments: vec![Segment::default()], ..Workout::default() }, CueSettings::default());
        server.begin(TvSession {
            title: "Ride".into(),
            profile: plan.profile.clone(),
            parts: crate::exercise::TvPart::of(&plan.workout),
            audio: TvAudio::Tv,
            listen: Some(super::super::RideListen { port: 1, code: "ABC".into(), name: "n".into() }),
            pulse_style: Default::default(),
            course_look: Default::default(),
            course_summits: false,
            ..TvSession::default()
        });
        server.publish(RideClock::view(&plan, 0, 10_000, 0), true);
        let response = http.get(format!("{base}/ride.json")).send().await.unwrap();
        assert_eq!(response.headers()["cache-control"], "no-store");
        let live: RideTvView = response.json().await.unwrap();
        assert!(live.playing && live.ride.is_some_and(|r| r.target_rpm == 87));
        assert_eq!(live.listen.unwrap().code, "ABC", "loopback is on the local network");
        assert!(live.sampled_ms > 0 && live.server_now_ms >= live.sampled_ms);

        // lyrics ride along while set, and go with set_lyrics(None) or the end of the ride
        let lyrics = crate::lyrics::Lyrics::parse("[00:01.00] Hello <b>world</b>\n[00:05.00] Bye", None).unwrap();
        server.set_lyrics(super::super::TvLyrics::new("t:1", &lyrics, 250, 180_000));
        let sung: RideTvView = http.get(format!("{base}/ride.json")).send().await.unwrap().json().await.unwrap();
        let tv_lyrics = sung.lyrics.expect("lyrics are served");
        assert_eq!((tv_lyrics.song_key.as_str(), tv_lyrics.synced, tv_lyrics.offset_ms), ("t:1", true, 250));
        assert_eq!(tv_lyrics.lines[0].text, "Hello <b>world</b>", "text goes out raw: the page uses textContent");
        server.set_lyrics(None);
        let gone: RideTvView = http.get(format!("{base}/ride.json")).send().await.unwrap().json().await.unwrap();
        assert!(gone.lyrics.is_none());
        server.set_lyrics(super::super::TvLyrics::new("t:2", &lyrics, 0, 0));

        server.clear();
        let cleared: RideTvView = http.get(format!("{base}/ride.json")).send().await.unwrap().json().await.unwrap();
        assert!(cleared.ride.is_none() && cleared.lyrics.is_none());
        server.stop().await;
    }

    #[tokio::test]
    async fn a_busy_port_falls_to_the_next_then_gives_a_humane_error() {
        let first = tokio::net::TcpListener::bind("0.0.0.0:0").await.unwrap();
        let port = first.local_addr().unwrap().port();
        // port in use: the next one is taken (if it is free; the test only checks it did not fail with the first)
        let second = TvServer::start(TvConfig { port, announce: false, ..TvConfig::default() }).await;
        if let Ok(server) = second {
            assert_ne!(server.port(), port);
            // both `port` and `port + 1` are now taken
            let third =
                TvServer::start(TvConfig { port, announce: false, lang: Lang::En, ..TvConfig::default() }).await;
            assert!(third.err().is_some_and(|e| e.to_string().contains("in use")));
            server.stop().await;
        }
    }

    #[test]
    fn the_ride_hub_is_listen_only_and_streams() {
        let config = ride_hub_config("Ride", 47802, "K7M2QXPA");
        assert!(config.stream_audio);
        assert_eq!((config.code.as_str(), config.bind.port()), ("K7M2QXPA", 47802));
        assert!(config.permissions.check_add(&Default::default(), 0).is_err());
    }
}
