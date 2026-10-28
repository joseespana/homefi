//! Shared control end to end: a hub with a fake host and in-process guests
//! over real sockets on 127.0.0.1.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::future::BoxFuture;
use futures::FutureExt;
use homefi_core::share::{
    AddMode, BrowseKind, Collection, CoverImage, GuestClient, GuestEvent, GuestMessage, HostCommand, Hub, HubConfig,
    HubEvent, HubHost, HubMessage, Page, Permissions, SearchResults, ShareTrack, SkipMode, Source, State,
};
use tokio::sync::mpsc::UnboundedReceiver;

const WAIT: Duration = Duration::from_secs(5);
const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3];

#[derive(Default)]
struct FakeHost {
    commands: Mutex<Vec<HostCommand>>,
    events: Mutex<Vec<HubEvent>>,
}

impl HubHost for FakeHost {
    fn search(&self, q: String) -> BoxFuture<'_, homefi_core::Result<SearchResults>> {
        async move {
            Ok(SearchResults {
                q: String::new(),
                tracks: vec![track(&format!("{q} song"))],
                albums: vec![Collection { kind: BrowseKind::Album, id: "a1".into(), title: q, ..Default::default() }],
                ..Default::default()
            })
        }
        .boxed()
    }

    fn browse(&self, kind: BrowseKind, id: String) -> BoxFuture<'_, homefi_core::Result<Page>> {
        async move {
            Ok(Page {
                header: Collection { kind, id, title: "Album".into(), ..Default::default() },
                tracks: vec![track("one"), track("two")],
            })
        }
        .boxed()
    }

    fn cover(&self, id: String) -> BoxFuture<'_, Option<CoverImage>> {
        async move { (id == "c1").then(|| CoverImage::new(JPEG.to_vec())) }.boxed()
    }

    fn command(&self, command: HostCommand) {
        self.commands.lock().unwrap().push(command);
    }

    fn event(&self, event: HubEvent) {
        self.events.lock().unwrap().push(event);
    }
}

impl FakeHost {
    fn last_queue(&self) -> Option<Vec<String>> {
        self.commands.lock().unwrap().iter().rev().find_map(|c| match c {
            HostCommand::Queue(items) => Some(items.iter().map(|i| i.track.title.clone()).collect()),
            _ => None,
        })
    }

    fn skips(&self) -> usize {
        self.commands.lock().unwrap().iter().filter(|c| **c == HostCommand::Skip).count()
    }
}

fn track(title: &str) -> ShareTrack {
    ShareTrack { source: Source::Tidal(title.replace(' ', "-")), title: title.into(), ..Default::default() }
}

async fn start(permissions: Permissions) -> (Hub, Arc<FakeHost>) {
    let host = Arc::new(FakeHost::default());
    let mut config = HubConfig::new("Música de <José>", "José");
    config.bind = SocketAddr::from(([127, 0, 0, 1], 0));
    config.code = "4821".into();
    config.permissions = permissions;
    let hub = Hub::start(config, host.clone()).await.unwrap();
    (hub, host)
}

async fn join(hub: &Hub, name: &str) -> (GuestClient, UnboundedReceiver<GuestEvent>) {
    let (client, welcome, rx) =
        GuestClient::connect("127.0.0.1", hub.local_addr().port(), name, "#ff8800", "4821").await.unwrap();
    assert_eq!(welcome.session_name, "Música de <José>");
    assert_eq!(welcome.guest_id, client.guest_id());
    (client, rx)
}

/// Next hub message matching `pred` (others are skipped).
async fn expect<T>(rx: &mut UnboundedReceiver<GuestEvent>, mut pred: impl FnMut(&GuestEvent) -> Option<T>) -> T {
    tokio::time::timeout(WAIT, async {
        loop {
            let ev = rx.recv().await.expect("event stream ended");
            if let Some(t) = pred(&ev) {
                return t;
            }
        }
    })
    .await
    .expect("timed out waiting for an event")
}

async fn state_where(rx: &mut UnboundedReceiver<GuestEvent>, pred: impl Fn(&State) -> bool) -> State {
    expect(rx, |ev| match ev {
        GuestEvent::Message(HubMessage::State(s)) if pred(s) => Some(s.clone()),
        _ => None,
    })
    .await
}

async fn denied(rx: &mut UnboundedReceiver<GuestEvent>) -> String {
    expect(rx, |ev| match ev {
        GuestEvent::Message(HubMessage::Denied { reason }) => Some(reason.clone()),
        _ => None,
    })
    .await
}

async fn notice(rx: &mut UnboundedReceiver<GuestEvent>) -> String {
    expect(rx, |ev| match ev {
        GuestEvent::Message(HubMessage::Notice { text }) => Some(text.clone()),
        _ => None,
    })
    .await
}

async fn until(cond: impl Fn() -> bool) {
    tokio::time::timeout(WAIT, async {
        while !cond() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("condition not met in time");
}

fn titles(s: &State) -> Vec<&str> {
    s.queue.iter().map(|i| i.track.title.as_str()).collect()
}

#[tokio::test]
async fn join_needs_the_right_code() {
    let (hub, host) = start(Permissions::default()).await;
    let port = hub.local_addr().port();

    let err = GuestClient::connect("127.0.0.1", port, "Eve", "#000000", "0000").await.err().unwrap();
    assert_eq!(err.to_string(), "Wrong join code");
    assert!(hub.guests().is_empty());

    let (_ana, mut rx) = join(&hub, "Ana").await;
    let s = state_where(&mut rx, |s| s.guests.len() == 1).await;
    assert_eq!(s.guests[0].name, "Ana");
    assert_eq!(s.guests[0].color, "#ff8800");
    assert!(matches!(&host.events.lock().unwrap()[0], HubEvent::GuestJoined(m) if m.name == "Ana"));

    // Guessing the 4-digit code is cut short (Ana's join reset the count).
    for _ in 0..5 {
        assert!(GuestClient::connect("127.0.0.1", port, "Eve", "", "1111").await.is_err());
    }
    let err = GuestClient::connect("127.0.0.1", port, "Eve", "", "4821").await.err().unwrap();
    assert_eq!(err.to_string(), "Too many wrong join codes");

    let info = hub.join_info("192.168.49.1");
    assert_eq!((info.port, info.code.as_str()), (port, "4821"));
    hub.stop().await;
}

#[tokio::test]
async fn adds_are_interleaved_by_turns_and_broadcast_to_everyone() {
    let (hub, host) = start(Permissions::default()).await;
    let (ana, mut ana_rx) = join(&hub, "Ana").await;
    let (ben, mut ben_rx) = join(&hub, "Ben").await;

    hub.add(track("h1"), false);
    hub.add(track("h2"), false);
    for t in ["a1", "a2"] {
        ana.send(GuestMessage::Add { item: track(t), next: false }).unwrap();
    }
    state_where(&mut ben_rx, |s| s.queue.len() == 4).await;
    ben.send(GuestMessage::Add { item: track("b1"), next: false }).unwrap();

    let expected = ["h1", "a1", "b1", "h2", "a2"];
    let s = state_where(&mut ana_rx, |s| s.queue.len() == 5).await;
    assert_eq!(titles(&s), expected);
    assert_eq!(s.queue[1].added_by.name, "Ana");
    assert_eq!(s.queue[2].added_by.name, "Ben");
    assert_eq!(s.queue[0].added_by.guest_id, "hub");
    let s = state_where(&mut ben_rx, |s| s.queue.len() == 5).await;
    assert_eq!(titles(&s), expected);
    until(|| host.last_queue().is_some_and(|q| q == expected)).await;

    // The host starts the first song: it leaves the queue and becomes now playing.
    hub.set_now_playing(Some(s.queue[0].queue_id), Some(s.queue[0].track.clone()), 0, true);
    let s = state_where(&mut ben_rx, |s| s.now_playing.as_ref().is_some_and(|n| n.track.title == "h1")).await;
    assert_eq!(titles(&s), ["a1", "b1", "h2", "a2"]);
    assert!(s.playing);
    hub.stop().await;
}

#[tokio::test]
async fn a_guest_picks_what_plays_now_when_allowed() {
    let (hub, host) = start(Permissions::default()).await;
    let (ana, mut rx) = join(&hub, "Ana").await;
    for t in ["h1", "h2", "h3"] {
        hub.add(track(t), false);
    }
    let s = state_where(&mut rx, |s| s.queue.len() == 3).await;
    let h3 = s.queue[2].queue_id;

    // by default guests vote to skip: choosing what plays now is the host's
    ana.send(GuestMessage::Play { queue_id: h3 }).unwrap();
    assert_eq!(denied(&mut rx).await, "Only the host can choose what plays now");

    hub.set_permissions(Permissions { skip: SkipMode::Everyone, ..Permissions::default() });
    ana.send(GuestMessage::Play { queue_id: h3 }).unwrap();
    let s = state_where(&mut rx, |s| s.queue.first().is_some_and(|q| q.queue_id == h3)).await;
    assert_eq!(titles(&s), ["h3", "h1", "h2"]);
    // the player gets the new order, then skips to it
    until(|| host.skips() == 1).await;
    assert_eq!(host.last_queue().unwrap(), ["h3", "h1", "h2"]);
    hub.stop().await;
}

#[tokio::test]
async fn vote_skip_needs_half_the_guests() {
    let (hub, host) = start(Permissions::default()).await;
    let (ana, mut ana_rx) = join(&hub, "Ana").await;
    let (ben, _ben_rx) = join(&hub, "Ben").await;
    let (_cy, _cy_rx) = join(&hub, "Cy").await;

    ana.send(GuestMessage::VoteSkip).unwrap();
    assert_eq!(denied(&mut ana_rx).await, "Nothing is playing");

    hub.set_now_playing(None, Some(track("song")), 1000, true);
    ana.send(GuestMessage::VoteSkip).unwrap();
    let s = state_where(&mut ana_rx, |s| s.skip_votes == 1).await;
    assert_eq!(s.skip_votes_needed, 2);
    ana.send(GuestMessage::Skip).unwrap(); // `skip` counts as a vote in vote mode
    assert_eq!(notice(&mut ana_rx).await, "You already voted");
    assert_eq!(host.skips(), 0);

    ben.send(GuestMessage::VoteSkip).unwrap();
    until(|| host.skips() == 1).await;

    // Votes belong to one song.
    ana.send(GuestMessage::VoteSkip).unwrap();
    state_where(&mut ana_rx, |s| s.skip_votes == 1).await;
    hub.set_now_playing(None, Some(track("next song")), 0, true);
    state_where(&mut ana_rx, |s| s.skip_votes == 0 && s.now_playing.as_ref().unwrap().track.title == "next song").await;
    hub.stop().await;
}

#[tokio::test]
async fn permissions_are_enforced_by_the_hub() {
    let perms = Permissions { allow_explicit: false, ..Default::default() };
    let (hub, host) = start(perms).await;
    let (ana, mut ana_rx) = join(&hub, "Ana").await;
    let (ben, mut ben_rx) = join(&hub, "Ben").await;

    ana.send(GuestMessage::Toggle).unwrap();
    assert_eq!(denied(&mut ana_rx).await, "Only the host can pause or change the volume");
    ana.send(GuestMessage::Add { item: ShareTrack { explicit: true, ..track("x") }, next: false }).unwrap();
    assert_eq!(denied(&mut ana_rx).await, "Explicit songs are not allowed");

    ana.send(GuestMessage::Add { item: track("a1"), next: false }).unwrap();
    let s = state_where(&mut ben_rx, |s| s.queue.len() == 1).await;
    let a1 = s.queue[0].queue_id;
    ben.send(GuestMessage::Remove { queue_id: a1 }).unwrap();
    assert_eq!(denied(&mut ben_rx).await, "You can only change your own songs");
    ana.send(GuestMessage::Remove { queue_id: a1 }).unwrap();
    state_where(&mut ben_rx, |s| s.queue.is_empty()).await;

    // Rules change while connected and are part of the state.
    hub.set_permissions(Permissions { playback: true, ..Default::default() });
    state_where(&mut ana_rx, |s| s.permissions.playback).await;
    ana.send(GuestMessage::Volume { value: 3.0 }).unwrap();
    until(|| host.commands.lock().unwrap().contains(&HostCommand::Volume(1.0))).await;
    hub.stop().await;
}

#[tokio::test]
async fn approval_mode_holds_songs_for_the_host() {
    let (hub, host) = start(Permissions { add: AddMode::Approve, ..Default::default() }).await;
    let (ana, mut ana_rx) = join(&hub, "Ana").await;

    ana.send(GuestMessage::Add { item: track("please"), next: false }).unwrap();
    assert_eq!(notice(&mut ana_rx).await, "Waiting for José to approve \u{201c}please\u{201d}");
    until(|| hub.pending().len() == 1).await;
    assert!(host.events.lock().unwrap().iter().any(|e| matches!(e, HubEvent::ApprovalRequested(_))));

    let id = hub.pending()[0].queue_id;
    assert!(hub.approve(id));
    let s = state_where(&mut ana_rx, |s| s.queue.len() == 1).await;
    assert_eq!(s.queue[0].queue_id, id);
    assert!(!hub.reject(id));
    hub.stop().await;
}

#[tokio::test]
async fn kicked_guests_are_disconnected_and_can_be_banned() {
    let (hub, host) = start(Permissions::default()).await;
    let (ana, mut ana_rx) = join(&hub, "Ana").await;
    let (_ben, mut ben_rx) = join(&hub, "Ben").await;
    state_where(&mut ana_rx, |s| s.guests.len() == 2).await;

    assert!(hub.kick(ana.guest_id(), true));
    let reason = expect(&mut ana_rx, |ev| match ev {
        GuestEvent::Disconnected { reason } => Some(reason.clone()),
        _ => None,
    })
    .await;
    assert_eq!(reason.as_deref(), Some("You were removed from the session"));
    let s = state_where(&mut ben_rx, |s| s.guests.len() == 1).await;
    assert_eq!(s.guests[0].name, "Ben");
    assert!(ana.send(GuestMessage::Skip).is_err() || !ana.is_connected());
    assert!(!hub.kick(ana.guest_id(), false));
    assert!(host.events.lock().unwrap().iter().any(|e| matches!(e, HubEvent::GuestLeft(m) if m.name == "Ana")));

    // Banned by address (everyone is 127.0.0.1 here).
    let err = GuestClient::connect("127.0.0.1", hub.local_addr().port(), "Ana", "", "4821").await.err().unwrap();
    assert_eq!(err.to_string(), "You were removed from this session");
    hub.stop().await;
}

#[tokio::test]
async fn search_browse_covers_and_page_over_http() {
    let (hub, _host) = start(Permissions::default()).await;
    let (ana, mut rx) = join(&hub, "Ana").await;

    ana.send(GuestMessage::Search { q: "queen".into() }).unwrap();
    let results = expect(&mut rx, |ev| match ev {
        GuestEvent::Message(HubMessage::Results(r)) => Some(r.clone()),
        _ => None,
    })
    .await;
    assert_eq!(results.q, "queen");
    assert_eq!(results.tracks[0].title, "queen song");

    ana.send(GuestMessage::Browse { kind: BrowseKind::Album, id: "a1".into() }).unwrap();
    let page = expect(&mut rx, |ev| match ev {
        GuestEvent::Message(HubMessage::Page(p)) => Some(p.clone()),
        _ => None,
    })
    .await;
    assert_eq!(page.header.id, "a1");
    assert_eq!(page.tracks.len(), 2);

    let http = reqwest::Client::new();
    let cover = http.get(ana.resolve_url("/cover/c1")).send().await.unwrap();
    assert_eq!(cover.status(), 200);
    assert_eq!(cover.headers()["content-type"], "image/jpeg");
    assert_eq!(cover.bytes().await.unwrap().as_ref(), JPEG);
    let missing = http.get(ana.resolve_url("/cover/nope")).send().await.unwrap();
    assert_eq!(missing.status(), 404);
    let page = http.get(ana.base_url()).send().await.unwrap().text().await.unwrap();
    assert!(page.contains("Música de &lt;José&gt;"));
    hub.stop().await;
}

#[tokio::test]
async fn stopping_the_hub_disconnects_guests() {
    let (hub, _host) = start(Permissions::default()).await;
    let port = hub.local_addr().port();
    let (_ana, mut rx) = join(&hub, "Ana").await;
    hub.stop().await;
    let reason = expect(&mut rx, |ev| match ev {
        GuestEvent::Disconnected { reason } => Some(reason.clone()),
        _ => None,
    })
    .await;
    assert_eq!(reason.as_deref(), Some("The host stopped sharing"));
    assert!(GuestClient::connect("127.0.0.1", port, "Ana", "", "4821").await.is_err());
}

#[tokio::test]
async fn hello_tells_a_guest_with_no_camera_it_found_a_hub() {
    let (hub, _host) = start(Permissions::default()).await;
    let url = format!("http://127.0.0.1:{}/hello", hub.local_addr().port());
    let hello: serde_json::Value = reqwest::get(&url).await.unwrap().json().await.unwrap();
    assert_eq!(hello["app"], "homefi");
    assert_eq!(hello["party"], false);
    assert!(hello["name"].is_string());
    assert!(!hello.to_string().contains("4821"), "never the join code");
    hub.stop().await;
}

#[tokio::test]
async fn a_new_song_starts_for_everyone_together() {
    let (hub, _host) = start(Permissions::default()).await;
    let (ana, mut rx) = join(&hub, "Ana").await;
    let (_bo, _rx_bo) = join(&hub, "Bo").await;
    hub.set_now_playing(None, Some(track("Flowers")), 0, true);
    let s = state_where(&mut rx, |s| s.now_playing.is_some()).await;
    let id = s.now_playing.unwrap().queue_id;

    // the hub announces a start in the future: guests wait at 0 until then
    let at = hub.schedule_start(2_000);
    let s = state_where(&mut rx, |s| s.clock_ms == at).await;
    assert!(s.playing && s.position_ms == 0);
    until(|| ana.hub_now_ms().is_some()).await;
    assert_eq!(ana.expected_position_ms(&s), 0);

    // and starts as soon as everyone has the song loaded
    assert_eq!(hub.ready_guests(id), (0, 2));
    ana.send(GuestMessage::Ready { queue_id: id }).unwrap();
    until(|| hub.ready_guests(id) == (1, 2)).await;
    // ready for another song doesn't count
    _bo.send(GuestMessage::Ready { queue_id: id + 1 }).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(hub.ready_guests(id), (1, 2));

    // the music's levels reach guests without audio, for their meters
    hub.send_levels(200, 180, vec![10, 20, 30]);
    let (l, r, b) = expect(&mut rx, |ev| match ev {
        GuestEvent::Message(HubMessage::Levels { l, r, b }) => Some((*l, *r, b.clone())),
        _ => None,
    })
    .await;
    assert_eq!((l, r, b), (200, 180, vec![10, 20, 30]));
    hub.stop().await;
}

#[tokio::test]
async fn party_mode_serves_the_song_and_shares_the_clock() {
    let (hub, _host) = start(Permissions::default()).await;
    let (ana, mut rx) = join(&hub, "Ana").await;

    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("song.flac");
    let bytes: Vec<u8> = (0..=255u8).cycle().take(10_000).collect();
    std::fs::write(&song, &bytes).unwrap();

    // not streaming yet: no audio in the state, nothing served
    hub.set_now_playing(None, Some(track("Flowers")), 1_000, true);
    hub.set_audio(Some(song.clone()));
    let s = state_where(&mut rx, |s| s.now_playing.is_some()).await;
    assert_eq!(s.audio, None);

    hub.set_streaming(true);
    let s = state_where(&mut rx, |s| s.audio.is_some()).await;
    let audio = s.audio.clone().unwrap();
    assert_eq!(audio.queue_id, s.now_playing.as_ref().unwrap().queue_id);
    assert_eq!((audio.mime.as_str(), audio.size), ("audio/flac", 10_000));
    assert!(s.playing && s.position_ms == 1_000);

    // range requests, so a player can start (and seek) before it is all there
    let http = reqwest::Client::new();
    let url = ana.resolve_url(&audio.url);
    let part = http.get(&url).header("range", "bytes=100-199").send().await.unwrap();
    assert_eq!(part.status(), 206);
    assert_eq!(part.headers()["content-type"], "audio/flac");
    assert_eq!(part.bytes().await.unwrap().as_ref(), &bytes[100..200]);
    let whole = http.get(&url).send().await.unwrap();
    assert_eq!(whole.bytes().await.unwrap().len(), 10_000);
    // only with the session key, and only the song playing now
    let wrong_key = url.replace("?k=", "?k=x");
    assert_eq!(http.get(&wrong_key).send().await.unwrap().status(), 404);
    let other = ana.resolve_url(&format!("/audio/{}?k=", audio.queue_id + 1));
    assert_eq!(http.get(&other).send().await.unwrap().status(), 404);

    // the clock: within a few ms of the hub's on the same machine
    until(|| ana.hub_now_ms().is_some()).await;
    let gap = ana.hub_now_ms().unwrap() as i64 - hub.now_ms() as i64;
    assert!(gap.abs() < 30, "clock off by {gap} ms");
    // and it says where the song is now: 1 s in, plus the time since then
    tokio::time::sleep(Duration::from_millis(200)).await;
    let at = ana.expected_position_ms(&s);
    assert!((1_150..1_600).contains(&at), "{at}");

    // a new song without a file stops the stream
    hub.set_now_playing(None, Some(track("Next one")), 0, true);
    hub.set_audio(None);
    let s = state_where(&mut rx, |s| s.now_playing.as_ref().is_some_and(|n| n.track.title == "Next one")).await;
    assert_eq!(s.audio, None);
    // the song before stays as "previous", so guests keep it ready
    assert_eq!(s.prefetch.iter().map(|a| a.queue_id).collect::<Vec<_>>(), vec![audio.queue_id]);
    assert_eq!(http.get(&url).send().await.unwrap().status(), 200);

    // the next song is served ahead, only while it is next
    let next = hub.add(track("After that"), false);
    let later = dir.path().join("later.m4a");
    std::fs::write(&later, b"m4a").unwrap();
    hub.set_next_audio(next.queue_id, Some(later.clone()));
    let s = state_where(&mut rx, |s| s.prefetch.len() == 2).await;
    assert_eq!(s.prefetch[0].queue_id, next.queue_id);
    assert_eq!(s.prefetch[0].mime, "audio/mp4");
    let next_url = ana.resolve_url(&s.prefetch[0].url);
    assert_eq!(http.get(&next_url).send().await.unwrap().bytes().await.unwrap().as_ref(), b"m4a");
    // stopping the stream stops all of it
    hub.set_streaming(false);
    let s = state_where(&mut rx, |s| s.prefetch.is_empty()).await;
    assert_eq!(s.audio, None);
    assert_eq!(http.get(&next_url).send().await.unwrap().status(), 404);
    assert_eq!(http.get(&url).send().await.unwrap().status(), 404);
    hub.stop().await;
}
