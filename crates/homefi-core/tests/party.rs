//! "Make a party playlist" end to end against mock servers: the chat, the
//! `propose_songs` build with fill rounds, Tidal's search, the library-only
//! path and the failure cases. Nothing here touches the real network.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use homefi_core::ai::{AiConfig, AiProvider};
use homefi_core::api::{TidalApi, TidalClient};
use homefi_core::humane::Lang;
use homefi_core::party::{
    AgeGroup, AskedSong, ChatBrief, EnergyCurve, PartyBrief, PartyContext, PartyProgress, PartySession, PartyVibe,
    MAX_CHAT_TURNS,
};
use homefi_core::playlist::{LibraryIndex, LibraryRef, PlaylistOrigin, PlaylistSettings};
use serde_json::{json, Value};
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const QUESTION: &str = include_str!("fixtures/party/question.json");
const ROUND1: &str = include_str!("fixtures/party/songs_round1.json");
const FEW: &str = include_str!("fixtures/party/songs_few.json");
const MORE: &str = include_str!("fixtures/party/songs_more.json");
const EXPLICIT: &str = include_str!("fixtures/party/songs_explicit.json");
const SET_PLAYLIST: &str = include_str!("fixtures/party/set_playlist.json");
const TEXT_ONLY: &str = include_str!("fixtures/party/text_only.json");
const KEY: &str = "sk-test";
const ARTISTS: [&str; 12] = [
    "Bad Bunny",
    "Daddy Yankee",
    "Shakira",
    "ABBA",
    "Queen",
    "Madonna",
    "Prince",
    "Rihanna",
    "Drake",
    "Adele",
    "Coldplay",
    "Maroon 5",
];
const TRACK_SECONDS: i64 = 200;

fn json_ok(fixture: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(serde_json::from_str::<Value>(fixture).unwrap())
}

/// An Anthropic answer that calls `propose_songs`.
fn proposal(songs: &[(&str, &str, u8)]) -> ResponseTemplate {
    let songs: Vec<Value> = songs.iter().map(|(t, a, e)| json!({ "title": t, "artist": a, "energy": e })).collect();
    ResponseTemplate::new(200).set_body_json(json!({
        "id": "msg_p", "type": "message", "role": "assistant", "model": "claude-x", "stop_reason": "tool_use",
        "content": [{ "type": "tool_use", "id": "toolu_p", "name": "propose_songs", "input": { "songs": songs } }],
        "usage": { "input_tokens": 1, "output_tokens": 1 }
    }))
}

/// Answers the n-th `propose_songs` call with the n-th fixture (the last one repeats).
struct Sequence {
    answers: Vec<String>,
    calls: AtomicUsize,
}

impl Respond for Sequence {
    fn respond(&self, _: &Request) -> ResponseTemplate {
        let n = self.calls.fetch_add(1, Ordering::SeqCst).min(self.answers.len() - 1);
        json_ok(&self.answers[n])
    }
}

/// Tidal's `/v1/search`: one studio track for "<artist> <title>". "Made Up Song"
/// is unknown and titles starting with "Explicit" are explicit.
struct TidalSearch;

impl Respond for TidalSearch {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let query =
            request.url.query_pairs().find(|(k, _)| k == "query").map(|(_, v)| v.into_owned()).unwrap_or_default();
        let artist = ARTISTS.iter().find(|a| query.starts_with(*a)).copied().unwrap_or("Nobody");
        let title = query.strip_prefix(artist).unwrap_or(&query).trim();
        let items = if title.is_empty() || title == "Made Up Song" { Vec::new() } else { vec![track(title, artist)] };
        let page = json!({ "limit": 50, "offset": 0, "totalNumberOfItems": items.len(), "items": items });
        ResponseTemplate::new(200).set_body_json(json!({ "tracks": page }))
    }
}

fn track(title: &str, artist: &str) -> Value {
    let id: i64 = format!("{artist}|{title}").bytes().fold(7i64, |h, b| (h * 31 + i64::from(b)) % 1_000_000_007);
    json!({
        "id": id, "title": title, "duration": TRACK_SECONDS, "allowStreaming": true,
        "explicit": title.starts_with("Explicit"), "popularity": 50, "isrc": format!("TEST{id}"),
        "artist": { "id": 7, "name": artist, "type": "MAIN" },
        "artists": [{ "id": 7, "name": artist, "type": "MAIN" }],
        "album": { "id": 5, "title": "Album", "cover": null }
    })
}

async fn mount_tidal(server: &MockServer) {
    Mock::given(method("GET")).and(path("/v1/search")).respond_with(TidalSearch).mount(server).await;
}

async fn mount_proposals(server: &MockServer, answers: &[&str]) {
    let answers = answers.iter().map(|a| (*a).to_owned()).collect();
    Mock::given(method("POST"))
        .and(path("/messages"))
        .and(body_string_contains("propose_songs"))
        .respond_with(Sequence { answers, calls: AtomicUsize::new(0) })
        .mount(server)
        .await;
}

async fn mount_chat(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/messages"))
        .and(body_string_contains("suggest_replies"))
        .respond_with(json_ok(QUESTION))
        .mount(server)
        .await;
}

fn tidal_api(server: &MockServer) -> TidalApi {
    let client =
        TidalClient::new("token").with_base_urls(format!("{}/v1", server.uri()), format!("{}/v2", server.uri()));
    TidalApi::new(Arc::new(client), "1", "US")
}

fn config(server: &MockServer) -> AiConfig {
    AiConfig::new(AiProvider::Anthropic, "model-x", Some(KEY.into())).with_base_url(server.uri())
}

/// A 30 minute party: about nine songs of 200 s.
fn half_hour() -> PartyBrief {
    PartyBrief { minutes: 30, ..PartyBrief::default() }
}

struct Fixture {
    settings: PlaylistSettings,
    library: LibraryIndex,
}

impl Fixture {
    fn new() -> Self {
        Self { settings: PlaylistSettings::default(), library: LibraryIndex::default() }
    }

    fn ctx<'a>(&'a self, api: Option<&'a TidalApi>) -> PartyContext<'a> {
        PartyContext { settings: &self.settings, api, library: &self.library }
    }
}

fn titles(entries: &[homefi_core::playlist::PlaylistEntry]) -> Vec<&str> {
    entries.iter().map(|e| e.title.as_str()).collect()
}

fn ai_bodies(all: &[Request]) -> Vec<String> {
    all.iter().filter(|r| r.method.as_str() == "POST").map(|r| String::from_utf8_lossy(&r.body).into_owned()).collect()
}

#[tokio::test]
async fn the_chat_asks_with_quick_replies_and_stops_after_the_limit() {
    let server = MockServer::start().await;
    mount_chat(&server).await;
    let mut session = PartySession::new(config(&server), half_hour(), Lang::En);

    let first = session.start().await.unwrap();
    assert_eq!(first.text, "Any artists everyone loves?");
    assert_eq!(first.quick_replies, ["Bad Bunny", "80s pop", "No idea"]);

    for n in 0..MAX_CHAT_TURNS {
        let turn = session.reply(&format!("answer {n}")).await.unwrap();
        assert_eq!(turn.quick_replies.len(), 3);
    }
    let sent = server.received_requests().await.unwrap().len();
    assert_eq!(sent, 1 + usize::from(MAX_CHAT_TURNS));

    let last = session.reply("one more").await.unwrap();
    assert_eq!(last.text, "That's plenty to go on. Build the playlist when you're ready.");
    assert!(last.quick_replies.is_empty());
    assert_eq!(server.received_requests().await.unwrap().len(), sent, "no AI call after the limit");

    let bodies = ai_bodies(&server.received_requests().await.unwrap());
    assert!(bodies[0].contains("a dance party") && bodies[0].contains("30 min"), "the answers are in the prompt");
    assert!(bodies[2].contains("answer 0") && bodies[2].contains("answer 1"));
    assert!(!bodies.iter().any(|b| b.contains(KEY)), "the key is never in a body");
}

#[tokio::test]
async fn a_failed_chat_turn_leaves_the_history_clean() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).respond_with(ResponseTemplate::new(500)).mount(&server).await;
    let mut session = PartySession::new(config(&server), half_hour(), Lang::En);
    assert!(session.start().await.is_err());
    assert!(session.reply("hello").await.is_err());
    // the second try restarted the chat: the failed words never reached the history
    let bodies = ai_bodies(&server.received_requests().await.unwrap());
    assert_eq!(bodies.len(), 2);
    assert!(!bodies[1].contains("hello"));
}

#[tokio::test]
async fn one_round_builds_the_playlist() {
    let server = MockServer::start().await;
    mount_proposals(&server, &[ROUND1]).await;
    mount_tidal(&server).await;
    let (api, fx) = (tidal_api(&server), Fixture::new());
    let brief = PartyBrief {
        ages: vec![AgeGroup::YoungAdults, AgeGroup::Adults],
        vibe: PartyVibe::Dance,
        curve: EnergyCurve::Arc,
        ..half_hour()
    };
    let mut session = PartySession::new(config(&server), brief, Lang::En);
    let log = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);

    let built = session.build(&fx.ctx(Some(&api)), &move |p| sink.lock().unwrap().push(p)).await.unwrap();

    let p = &built.playlist;
    assert_eq!(p.name, "Dance party · 30 min");
    assert_eq!(p.description, "For friends · 18–29, 30–49");
    assert_eq!(p.entries.len(), 9, "nine songs of 200 s fill 30 minutes");
    assert!(!titles(&p.entries).contains(&"Made Up Song"));
    assert!(p.unmatched.iter().any(|c| c.title == "Made Up Song"), "an invented song stays unmatched");
    assert_eq!(built.note, None);
    assert!(p.sources.is_empty() && p.search_widget_html.is_none());
    assert!(p.entries.iter().all(|e| e.tidal_id.is_some()));
    let notes: Vec<&str> = p.entries.iter().map(|e| e.note.as_str()).collect();
    assert_eq!(notes[0], "Warm-up");
    assert_eq!(notes[8], "Wind-down");
    let PlaylistOrigin::Party(origin) = &p.origin else { panic!("a party playlist") };
    assert_eq!((origin.provider_id.as_str(), origin.model.as_str()), ("anthropic", "model-x"));
    assert_eq!(origin.brief.minutes, 30);
    assert!(origin.built_ms > 0 && p.created_ms == origin.built_ms);

    let ctx = p.source_context();
    assert!(ctx.is("playlist") && ctx.id == p.id);

    {
        let log = log.lock().unwrap();
        assert_eq!(log.first(), Some(&PartyProgress::Asking { round: 0 }));
        assert!(log.iter().any(|p| matches!(p, PartyProgress::Resolving { total: 11, .. })));
    }
    let asked = ai_bodies(&server.received_requests().await.unwrap());
    assert_eq!(asked.len(), 1);
    assert!(asked[0].contains("Propose 12 real songs"), "12 = nine needed x 1.5, rounded up");
}

#[tokio::test]
async fn a_short_first_round_gets_a_fill_round() {
    let server = MockServer::start().await;
    mount_proposals(&server, &[FEW, MORE]).await;
    mount_tidal(&server).await;
    let (api, fx) = (tidal_api(&server), Fixture::new());
    let mut session = PartySession::new(config(&server), half_hour(), Lang::En);
    let log = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);

    let built = session.build(&fx.ctx(Some(&api)), &move |p| sink.lock().unwrap().push(p)).await.unwrap();

    assert_eq!(built.playlist.entries.len(), 9);
    let mut seen = titles(&built.playlist.entries);
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), 9, "the song proposed again in the second round counts once");
    assert!(log.lock().unwrap().contains(&PartyProgress::Asking { round: 1 }));
    let asked = ai_bodies(&server.received_requests().await.unwrap());
    assert_eq!(asked.len(), 2);
    assert!(asked[1].contains("Already in the playlist") && asked[1].contains("Bad Bunny – Song 0"));
}

#[tokio::test]
async fn explicit_songs_are_left_out_and_counted() {
    let server = MockServer::start().await;
    mount_proposals(&server, &[EXPLICIT]).await;
    mount_tidal(&server).await;
    let (api, fx) = (tidal_api(&server), Fixture::new());
    let brief = PartyBrief { allow_explicit: false, ..half_hour() };
    let mut session = PartySession::new(config(&server), brief, Lang::En);

    let built = session.build(&fx.ctx(Some(&api)), &|_| {}).await.unwrap();

    assert!(built.playlist.entries.iter().all(|e| !e.explicit && !e.title.starts_with("Explicit")));
    assert_eq!(built.note.as_deref(), Some("4 explicit songs were left out."));
    let asked = ai_bodies(&server.received_requests().await.unwrap());
    assert!(asked[0].contains("No explicit songs."));
}

#[tokio::test]
async fn at_most_the_configured_songs_per_artist() {
    let server = MockServer::start().await;
    let mut songs: Vec<(String, &str, u8)> = (0..6).map(|i| (format!("Queen Hit {i}"), "Queen", 3)).collect();
    songs.extend((0..8).map(|i| (format!("Other {i}"), ARTISTS[i % 4], 3)));
    let refs: Vec<(&str, &str, u8)> = songs.iter().map(|(t, a, e)| (t.as_str(), *a, *e)).collect();
    Mock::given(method("POST"))
        .and(body_string_contains("propose_songs"))
        .respond_with(proposal(&refs))
        .mount(&server)
        .await;
    mount_tidal(&server).await;
    let (api, mut fx) = (tidal_api(&server), Fixture::new());
    fx.settings.party_max_per_artist = 2;
    let mut session = PartySession::new(config(&server), half_hour(), Lang::En);

    let built = session.build(&fx.ctx(Some(&api)), &|_| {}).await.unwrap();

    let queen = built.playlist.entries.iter().filter(|e| e.artist == "Queen").count();
    assert_eq!(queen, 2);
}

#[tokio::test]
async fn without_tidal_the_library_is_used_and_the_note_says_so() {
    let server = MockServer::start().await;
    mount_proposals(&server, &[ROUND1]).await;
    let library = LibraryIndex::new(
        (0..3)
            .map(|i| LibraryRef {
                path: format!("/music/song{i}.flac"),
                title: format!("Song {i}"),
                artist: ARTISTS[i].into(),
                duration_ms: 200_000,
                ..LibraryRef::default()
            })
            .collect(),
    );
    let fx = Fixture { settings: PlaylistSettings::default(), library };
    let mut session = PartySession::new(config(&server), half_hour(), Lang::En);

    let built = session.build(&fx.ctx(None), &|_| {}).await.unwrap();

    assert_eq!(built.playlist.entries.len(), 3);
    assert!(built.playlist.entries.iter().all(|e| e.tidal_id.is_none() && e.local_path.is_some()));
    let note = built.note.unwrap();
    assert!(note.contains("min short of 30 min") && note.contains("Connect Tidal"), "{note}");
}

#[tokio::test]
async fn nothing_to_search_in_is_a_worded_error() {
    let server = MockServer::start().await;
    let fx = Fixture::new();
    let mut session = PartySession::new(config(&server), half_hour(), Lang::Es);
    let err = session.build(&fx.ctx(None), &|_| {}).await.unwrap_err();
    assert_eq!(err.to_string(), "Conecta TIDAL o agrega carpetas de música para que HomeFi encuentre las canciones.");
    assert!(server.received_requests().await.unwrap().is_empty(), "no AI call for nothing");
}

#[tokio::test]
async fn an_answer_without_songs_is_an_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).respond_with(json_ok(TEXT_ONLY)).mount(&server).await;
    mount_tidal(&server).await;
    let (api, fx) = (tidal_api(&server), Fixture::new());
    let mut session = PartySession::new(config(&server), half_hour(), Lang::En);
    let err = session.build(&fx.ctx(Some(&api)), &|_| {}).await.unwrap_err();
    assert!(!err.to_string().contains(KEY));
    assert_eq!(server.received_requests().await.unwrap().len(), 1, "a bad answer is not retried as a fill round");
}

#[tokio::test]
async fn songs_nobody_can_find_end_with_no_songs() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_string_contains("propose_songs"))
        .respond_with(proposal(&[("Made Up Song", "Bad Bunny", 3)]))
        .mount(&server)
        .await;
    mount_tidal(&server).await;
    let (api, fx) = (tidal_api(&server), Fixture::new());
    let mut session = PartySession::new(config(&server), half_hour(), Lang::En);
    let err = session.build(&fx.ctx(Some(&api)), &|_| {}).await.unwrap_err();
    assert_eq!(err.to_string(), "Your AI didn't suggest any songs HomeFi could find. Try again or change the answers.");
}

#[tokio::test]
async fn building_again_tells_the_ai_what_was_there() {
    let server = MockServer::start().await;
    mount_proposals(&server, &[ROUND1]).await;
    mount_chat(&server).await;
    mount_tidal(&server).await;
    let (api, fx) = (tidal_api(&server), Fixture::new());
    let mut session = PartySession::new(config(&server), half_hour(), Lang::Es);
    session.start().await.unwrap();
    session.reply("más reguetón").await.unwrap();
    session.build(&fx.ctx(Some(&api)), &|_| {}).await.unwrap();
    let second = session.build(&fx.ctx(Some(&api)), &|_| {}).await.unwrap();

    assert!(!second.playlist.entries.is_empty());
    let asked = ai_bodies(&server.received_requests().await.unwrap());
    let builds: Vec<&String> = asked.iter().filter(|b| b.contains("Propose ")).collect();
    assert_eq!(builds.len(), 2);
    assert!(!builds[0].contains("previous playlist"));
    assert!(builds[1].contains("previous playlist") && builds[1].contains("Bad Bunny – Song 0"));
    assert!(builds[1].contains("más reguetón"), "the chat is part of the build request");
    assert!(builds[1].contains("do not translate") && !builds[1].contains("Answer in"), "titles stay as credited");
}

/// A chat session whose AI named the playlist and set 28 minutes (rounded to 30).
async fn chatted(server: &MockServer, lang: Lang) -> PartySession {
    mount_chat_with_set_playlist(server).await;
    let mut session = PartySession::new(config(server), ChatBrief::default(), lang);
    session.start().await.unwrap();
    session
}

async fn mount_chat_with_set_playlist(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/messages"))
        .and(body_string_contains("set_playlist"))
        .respond_with(json_ok(SET_PLAYLIST))
        .mount(server)
        .await;
}

#[tokio::test]
async fn a_chat_build_names_the_playlist_and_saves_a_chat_origin() {
    let server = MockServer::start().await;
    // proposals first: the build request replays the chat, whose tool calls match the chat mocks too
    mount_proposals(&server, &[ROUND1]).await;
    let mut session = chatted(&server, Lang::En).await;
    mount_tidal(&server).await;
    let (api, fx) = (tidal_api(&server), Fixture::new());

    let turn = session.reply("Sad songs from the 90s in English").await.unwrap();
    assert_eq!(turn.quick_replies, ["Calmer", "More rock"]);
    let built = session.build(&fx.ctx(Some(&api)), &|_| {}).await.unwrap();

    let p = &built.playlist;
    assert_eq!(p.name, "Sad 90s in English · 30 min");
    assert_eq!(p.description, "Sad songs from the 90s in English");
    let PlaylistOrigin::Chat(origin) = &p.origin else { panic!("a chat playlist") };
    assert_eq!(origin.brief.title, "Sad 90s in English");
    assert_eq!((origin.brief.minutes, origin.brief.curve), (30, EnergyCurve::Steady));
    assert_eq!(origin.provider_id, "anthropic");
    // steady keeps the AI's order: no warm-up / peak notes
    assert!(p.entries.iter().all(|e| e.note.is_empty()));
    assert_eq!(p.entries[0].title, "Song 0");
    let asked = ai_bodies(&server.received_requests().await.unwrap());
    let build = asked.iter().find(|b| b.contains("Propose ")).unwrap();
    assert!(build.contains("for this playlist") && build.contains("Sad songs from the 90s in English"));
}

#[tokio::test]
async fn without_a_title_the_wish_names_it() {
    let server = MockServer::start().await;
    mount_proposals(&server, &[ROUND1]).await;
    mount_chat(&server).await;
    mount_tidal(&server).await;
    let (api, fx) = (tidal_api(&server), Fixture::new());
    let brief = ChatBrief { minutes: 30, ..ChatBrief::default() };
    let mut session = PartySession::new(config(&server), brief, Lang::En);
    session.reply("Something calm to read").await.unwrap();

    let built = session.build(&fx.ctx(Some(&api)), &|_| {}).await.unwrap();

    assert_eq!(built.playlist.name, "Something calm to read · 30 min");
}

#[tokio::test]
async fn the_chat_opener_is_recalled_when_keeping_chatting() {
    let server = MockServer::start().await;
    let brief = ChatBrief { wish: "sad 90s".into(), title: "Sad 90s".into(), ..ChatBrief::default() };
    let mut session = PartySession::new(config(&server), brief.clone(), Lang::Es);
    let turn = session.start().await.unwrap();
    assert!(turn.text.contains("La última vez pediste: “sad 90s”"));
    assert_eq!(turn.chat_brief, Some(brief));
    assert!(server.received_requests().await.unwrap().is_empty(), "the opener needs no AI");
}

#[tokio::test]
async fn named_songs_open_the_playlist_once_even_when_explicit() {
    let server = MockServer::start().await;
    mount_proposals(&server, &[ROUND1]).await;
    mount_tidal(&server).await;
    let (api, fx) = (tidal_api(&server), Fixture::new());
    let songs = vec![
        AskedSong { title: "Explicit Anthem".into(), artist: "Queen".into() },
        AskedSong { title: "Made Up Song".into(), artist: "Queen".into() },
        AskedSong { title: "Song 0".into(), artist: "Bad Bunny".into() },
    ];
    let brief = ChatBrief {
        wish: "Queen and similar".into(),
        minutes: 30,
        allow_explicit: false,
        curve: EnergyCurve::Arc,
        songs,
        ..ChatBrief::default()
    };
    let mut session = PartySession::new(config(&server), brief, Lang::En);

    let built = session.build(&fx.ctx(Some(&api)), &|_| {}).await.unwrap();

    let got = titles(&built.playlist.entries);
    assert_eq!(&got[..2], ["Explicit Anthem", "Song 0"], "named songs first, in order, explicit kept");
    assert_eq!(got.iter().filter(|t| **t == "Song 0").count(), 1, "the AI's copy is a repeat");
    assert!(built.playlist.unmatched.iter().any(|c| c.title == "Made Up Song"));
    let asked = ai_bodies(&server.received_requests().await.unwrap());
    assert!(asked
        .iter()
        .any(|b| b.contains("Songs to include (they open the playlist): <named_songs>Queen – Explicit Anthem")));
}

#[tokio::test]
async fn a_chat_playlist_needs_a_wish_before_it_builds() {
    let server = MockServer::start().await;
    mount_tidal(&server).await;
    let (api, fx) = (tidal_api(&server), Fixture::new());
    let mut session = PartySession::new(config(&server), ChatBrief::default(), Lang::Es);

    let err = session.build(&fx.ctx(Some(&api)), &|_| {}).await.unwrap_err();

    assert_eq!(err.to_string(), "Primero dile a tu IA qué te gustaría escuchar.");
    assert!(server.received_requests().await.unwrap().is_empty(), "no AI, no search");
}

#[tokio::test]
async fn only_the_named_songs_found_is_a_worded_error_not_a_playlist() {
    let server = MockServer::start().await;
    // the AI's own songs don't exist on Tidal; the named one does
    mount_proposals(&server, &[r#"{"id":"m","type":"message","role":"assistant","model":"x","stop_reason":"tool_use","content":[{"type":"tool_use","id":"t","name":"propose_songs","input":{"songs":[{"title":"Made Up Song","artist":"Queen","energy":3}]}}],"usage":{"input_tokens":1,"output_tokens":1}}"#]).await;
    mount_tidal(&server).await;
    let (api, fx) = (tidal_api(&server), Fixture::new());
    let brief = ChatBrief {
        wish: "Queen".into(),
        songs: vec![AskedSong { title: "Song 0".into(), artist: "Queen".into() }],
        ..ChatBrief::default()
    };
    let mut session = PartySession::new(config(&server), brief, Lang::En);

    let err = session.build(&fx.ctx(Some(&api)), &|_| {}).await.unwrap_err();

    assert_eq!(err.to_string(), "Your AI didn't suggest any songs HomeFi could find. Try again or change the answers.");
}

#[tokio::test]
async fn tidal_not_answering_is_said_as_such() {
    let server = MockServer::start().await;
    mount_proposals(&server, &[ROUND1]).await;
    Mock::given(method("GET")).and(path("/v1/search")).respond_with(ResponseTemplate::new(404)).mount(&server).await;
    let (api, fx) = (tidal_api(&server), Fixture::new());
    let brief = ChatBrief { wish: "anything".into(), minutes: 30, ..ChatBrief::default() };
    let mut session = PartySession::new(config(&server), brief, Lang::En);

    let err = session.build(&fx.ctx(Some(&api)), &|_| {}).await.unwrap_err().to_string();

    assert!(err.starts_with("TIDAL didn't answer ") && err.ends_with("Try again in a minute."), "{err}");
}
