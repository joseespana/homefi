//! Exercise mode end to end against mock servers: the build from genres (no AI), from the AI's proposals,
//! from the library's BPM tags, with analysis, the widening ladder and the chat. Nothing here touches the
//! real network.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use homefi_core::ai::{AiConfig, AiProvider};
use homefi_core::api::{TidalApi, TidalClient};
use homefi_core::dj::DjAnalysis;
use homefi_core::exercise::{
    beginner_caps, build, build_with_memory, Analyzer, BpmSource, BuildContext, ExerciseSession, ExerciseSettings,
    Interval, MusicWish, Position, PresetId, PulseSource, RideProgress, RideRequest, RpmRange, Segment, SegmentKind,
    SourcesMode, Target, Workout,
};
use homefi_core::humane::Lang;
use homefi_core::playlist::{LibraryIndex, LibraryRef, PlaylistEntry, PlaylistOrigin};
use serde_json::{json, Value};
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const SONG_SECONDS: i64 = 200;

fn tidal_api(server: &MockServer) -> TidalApi {
    let client =
        TidalClient::new("token").with_base_urls(format!("{}/v1", server.uri()), format!("{}/v2", server.uri()));
    TidalApi::new(Arc::new(client), "1", "US")
}

fn ai_config(server: &MockServer) -> AiConfig {
    AiConfig::new(AiProvider::Anthropic, "model-x", Some("sk-test".into())).with_base_url(server.uri())
}

fn track(id: i64, title: &str, artist: &str, bpm: Option<i64>, seconds: i64) -> Value {
    json!({
        "id": id, "title": title, "duration": seconds, "allowStreaming": true, "explicit": false, "popularity": 50,
        "bpm": bpm, "isrc": format!("TEST{id}"),
        "artist": { "id": 7, "name": artist, "type": "MAIN" },
        "artists": [{ "id": 7, "name": artist, "type": "MAIN" }],
        "album": { "id": 5, "title": "Album", "cover": null }
    })
}

fn page(items: Vec<Value>) -> Value {
    json!({ "tracks": { "limit": 50, "offset": 0, "totalNumberOfItems": items.len(), "items": items } })
}

/// Tidal's search by genre: "rock" gives slow songs, "metal" fast ones, each titled `<genre>-<n>`.
struct GenreSearch {
    queries: Mutex<Vec<String>>,
}

impl Respond for GenreSearch {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let query =
            request.url.query_pairs().find(|(k, _)| k == "query").map(|(_, v)| v.into_owned()).unwrap_or_default();
        self.queries.lock().unwrap().push(query.clone());
        let table: &[(&str, &[i64])] =
            &[("rock", &[85, 92, 98, 110, 112, 76]), ("metal", &[170, 150, 160, 180, 125, 165])];
        let genre = table.iter().find(|(g, _)| query.starts_with(g));
        let items = genre
            .map(|(g, bpms)| {
                bpms.iter()
                    .enumerate()
                    .map(|(n, bpm)| {
                        track(
                            1000 + n as i64 + if *g == "metal" { 100 } else { 0 },
                            &format!("{g}-{n}"),
                            &format!("{g} band {n}"),
                            Some(*bpm),
                            SONG_SECONDS,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        ResponseTemplate::new(200).set_body_json(page(items))
    }
}

fn seg(kind: SegmentKind, rpm: (u16, u16), position: Position, interval: Option<Interval>) -> Segment {
    Segment {
        kind,
        seconds: SONG_SECONDS as u32,
        target: Target::Cadence { min_rpm: rpm.0, max_rpm: rpm.1 },
        position,
        interval,
        ..Segment::default()
    }
}

/// Warm-up, flat, a seated climb and sprints: each as long as a test song.
fn four_segments() -> Workout {
    Workout {
        id: "w-test".into(),
        segments: vec![
            seg(SegmentKind::WarmUp, (80, 95), Position::Seated, None),
            seg(SegmentKind::SeatedFlat, (85, 100), Position::Seated, None),
            seg(SegmentKind::SeatedClimb, (60, 80), Position::Seated, None),
            seg(
                SegmentKind::Sprint,
                (100, 120),
                Position::Seated,
                Some(Interval { work_s: 20, rest_s: 40, rounds: 3, rest_cadence: RpmRange::new(80, 90) }),
            ),
        ],
        ..Workout::default()
    }
}

struct Fixture {
    settings: ExerciseSettings,
    library: LibraryIndex,
}

impl Fixture {
    fn new() -> Self {
        Self { settings: ExerciseSettings::default(), library: LibraryIndex::default() }
    }

    fn ctx<'a>(
        &'a self,
        api: Option<&'a TidalApi>,
        ai: Option<&'a AiConfig>,
        analyzer: Option<&'a dyn Analyzer>,
    ) -> BuildContext<'a> {
        BuildContext { settings: &self.settings, ai, api, library: &self.library, analyzer, lang: Lang::En }
    }
}

fn request(workout: Workout, genres: &[&str], mix: bool) -> RideRequest {
    RideRequest {
        minutes: 15,
        wish: MusicWish { genres: genres.iter().map(|g| (*g).to_owned()).collect(), mix, ..MusicWish::default() },
        workout: Some(workout),
        ..RideRequest::default()
    }
}

#[tokio::test]
async fn without_an_ai_the_genres_drive_tidal_and_the_tempo_decides() {
    let server = MockServer::start().await;
    let search = Arc::new(GenreSearch { queries: Mutex::new(Vec::new()) });
    struct Shared(Arc<GenreSearch>);
    impl Respond for Shared {
        fn respond(&self, r: &Request) -> ResponseTemplate {
            self.0.respond(r)
        }
    }
    Mock::given(method("GET")).and(path("/v1/search")).respond_with(Shared(search.clone())).mount(&server).await;
    let api = tidal_api(&server);
    let fx = Fixture::new();
    let progress = Mutex::new(Vec::new());
    let built = build(&request(four_segments(), &["rock", "metal"], true), &fx.ctx(Some(&api), None, None), &|p| {
        progress.lock().unwrap().push(p)
    })
    .await
    .unwrap();

    let plan = &built.plan;
    assert_eq!(plan.songs.len(), 4);
    for (song, seg) in plan.songs.iter().zip(&plan.workout.segments) {
        let fit = song.fit.expect("every song fits its cadence");
        assert!(
            seg.cadence().distance(fit.rpm) <= 4.0,
            "{} at {} RPM for {:?}",
            song.entry.title,
            fit.rpm,
            seg.cadence()
        );
        assert_eq!(song.source, Some(BpmSource::Tidal));
        assert_eq!(song.pulse, PulseSource::Metronome, "no grid without analysis");
    }
    // the climb (60-80 RPM) takes a song whose tempo (or half of it) lands there; the mix uses both genres
    let climb = &plan.songs[2];
    assert!((60.0..=80.0).contains(&climb.fit.unwrap().rpm), "{:?}", climb.fit);
    let genres: std::collections::HashSet<&str> =
        plan.songs.iter().map(|s| s.entry.title.split('-').next().unwrap()).collect();
    assert_eq!(genres.len(), 2, "a mix spreads the genres: {genres:?}");
    // the playlist is the ride, in order
    assert_eq!(built.playlist.entries.len(), 4);
    assert_eq!(built.playlist.entries[0].title, plan.songs[0].entry.title);
    let PlaylistOrigin::Ride(origin) = &built.playlist.origin else { panic!("a ride playlist") };
    assert_eq!((origin.workout_id.as_str(), origin.wish.mix), ("w-test", true));
    assert!(plan.notes.iter().any(|n| n.contains("No AI is set up")));
    assert!(plan.profile.len() == 5);
    // the sprint block follows its song: rounds fit
    let iv = plan.workout.segments[3].interval.unwrap();
    assert!(iv.total_s() <= plan.workout.segments[3].seconds);
    assert!(progress.lock().unwrap().contains(&RideProgress::Matching));
    let queries = search.queries.lock().unwrap().clone();
    assert!(queries.iter().any(|q| q == "rock") && queries.iter().any(|q| q == "metal"));
}

/// Anthropic's answer that calls `propose_ride_songs` with these songs for segment 0..n.
fn proposals(per_segment: &[&[(&str, &str)]]) -> ResponseTemplate {
    let segments: Vec<Value> = per_segment
        .iter()
        .enumerate()
        .map(|(i, songs)| {
            json!({ "segment": i, "songs": songs.iter().map(|(t, a)| json!({ "title": t, "artist": a })).collect::<Vec<_>>() })
        })
        .collect();
    ResponseTemplate::new(200).set_body_json(json!({
        "id": "msg_p", "type": "message", "role": "assistant", "model": "claude-x", "stop_reason": "tool_use",
        "content": [{ "type": "tool_use", "id": "toolu_p", "name": "propose_ride_songs", "input": { "segments": segments } }],
        "usage": { "input_tokens": 1, "output_tokens": 1 }
    }))
}

/// Tidal's search for "<artist> <title>": one track with the tempo of the table.
struct SongSearch;

const CATALOG: &[(&str, &str, Option<i64>)] = &[
    ("Easy Morning", "Calm Co", Some(88)),
    ("Steady Road", "Road Band", Some(95)),
    ("Hill Crusher", "Heavy Co", Some(144)),
    ("Final Push", "Fast Co", Some(110)),
    ("Wrong Tempo", "Odd Co", Some(133)),
    ("Unknown Tempo", "Mystery", None),
];

impl Respond for SongSearch {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let query =
            request.url.query_pairs().find(|(k, _)| k == "query").map(|(_, v)| v.into_owned()).unwrap_or_default();
        let items = CATALOG
            .iter()
            .enumerate()
            .filter(|(_, (title, artist, _))| query == format!("{artist} {title}"))
            .map(|(i, (title, artist, bpm))| track(500 + i as i64, title, artist, *bpm, SONG_SECONDS))
            .collect();
        ResponseTemplate::new(200).set_body_json(page(items))
    }
}

#[tokio::test]
async fn the_ais_candidates_are_resolved_and_kept_only_when_their_tempo_fits() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/v1/search")).respond_with(SongSearch).mount(&server).await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .and(body_string_contains("propose_ride_songs"))
        .respond_with(proposals(&[
            &[("Wrong Tempo", "Odd Co"), ("Easy Morning", "Calm Co"), ("Made Up Song", "Nobody")],
            &[("Steady Road", "Road Band")],
            &[("Hill Crusher", "Heavy Co"), ("Wrong Tempo", "Odd Co")],
            &[("Final Push", "Fast Co")],
        ]))
        .mount(&server)
        .await;
    let api = tidal_api(&server);
    let ai = ai_config(&server);
    let fx = Fixture::new();
    let built = build(&request(four_segments(), &["rock"], false), &fx.ctx(Some(&api), Some(&ai), None), &|_| {})
        .await
        .unwrap();
    let titles: Vec<&str> = built.plan.songs.iter().map(|s| s.entry.title.as_str()).collect();
    assert_eq!(
        titles,
        ["Easy Morning", "Steady Road", "Hill Crusher", "Final Push"],
        "133 BPM fits nothing and is left out"
    );
    assert_eq!(built.plan.songs[2].fit.unwrap().ratio, homefi_core::exercise::BeatRatio::Half);
    assert!(built.plan.notes.iter().all(|n| !n.contains("No AI")));
    // the prompt carried the segments and their tempo windows, and the key stayed in the header
    let sent = server.received_requests().await.unwrap();
    let body = sent
        .iter()
        .find(|r| r.method.as_str() == "POST")
        .map(|r| String::from_utf8_lossy(&r.body).into_owned())
        .unwrap();
    assert!(body.contains("segment 2") && body.contains("BPM") && body.contains("only <genres>rock</genres>"));
    assert!(!body.contains("sk-test"));
}

#[tokio::test]
async fn an_ai_that_fails_falls_back_to_the_genres_with_a_note() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/search"))
        .respond_with(GenreSearch { queries: Mutex::new(Vec::new()) })
        .mount(&server)
        .await;
    Mock::given(method("POST")).respond_with(ResponseTemplate::new(500)).mount(&server).await;
    let api = tidal_api(&server);
    let ai = ai_config(&server);
    let fx = Fixture::new();
    let built =
        build(&request(four_segments(), &["rock", "metal"], true), &fx.ctx(Some(&api), Some(&ai), None), &|_| {})
            .await
            .unwrap();
    assert_eq!(built.plan.songs.len(), 4);
    assert!(built.plan.notes.iter().any(|n| n.contains("from your genres")), "{:?}", built.plan.notes);
}

fn library_song(n: usize, genre: &str, bpm: Option<u16>, seconds: u64) -> LibraryRef {
    LibraryRef {
        path: format!("/music/{n}.flac"),
        title: format!("Song {n}"),
        artist: format!("Artist {n}"),
        album: "Album".into(),
        duration_ms: seconds * 1000,
        bpm,
        genre: genre.into(),
        ..LibraryRef::default()
    }
}

#[tokio::test]
async fn the_library_tag_is_the_first_source_and_genre_tags_filter() {
    let fx = Fixture {
        settings: ExerciseSettings { sources: SourcesMode::LibraryOnly, ..ExerciseSettings::default() },
        library: LibraryIndex::new(vec![
            library_song(0, "Hard Rock", Some(90), 200),
            library_song(1, "Rock", Some(96), 200),
            library_song(2, "Metal", Some(150), 200),
            library_song(3, "Rock", Some(112), 200),
            library_song(4, "Jazz", Some(88), 200),
        ]),
    };
    let built =
        build(&request(four_segments(), &["rock", "metal"], true), &fx.ctx(None, None, None), &|_| {}).await.unwrap();
    let paths: Vec<&str> = built.plan.songs.iter().map(|s| s.entry.local_path.as_deref().unwrap()).collect();
    assert!(!paths.contains(&"/music/4.flac"), "jazz is not in the wish");
    assert_eq!(built.plan.songs.len(), 4);
    assert!(built.plan.songs.iter().all(|s| s.source == Some(BpmSource::Tag)));
    assert!(built.plan.notes.iter().any(|n| n.contains("Connect Tidal")), "no Tidal, said so");
}

struct FakeAnalyzer {
    calls: AtomicUsize,
}

impl Analyzer for FakeAnalyzer {
    fn analyze<'a>(
        &'a self,
        _entry: &'a PlaylistEntry,
    ) -> Pin<Box<dyn Future<Output = Option<DjAnalysis>> + Send + 'a>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {
            Some(DjAnalysis {
                bpm: 92.0,
                first_beat_ms: 120.0,
                beat_ms: 60_000.0 / 92.0,
                confidence: 0.9,
                ..DjAnalysis::default()
            })
        })
    }
}

#[tokio::test]
async fn songs_without_a_tempo_are_analyzed_within_the_budget() {
    let songs: Vec<LibraryRef> = (0..10).map(|n| library_song(n, "Rock", None, 200)).collect();
    let fx = Fixture {
        settings: ExerciseSettings {
            sources: SourcesMode::LibraryOnly,
            analysis_budget: 3,
            ..ExerciseSettings::default()
        },
        library: LibraryIndex::new(songs),
    };
    let analyzer = FakeAnalyzer { calls: AtomicUsize::new(0) };
    let workout = Workout { segments: four_segments().segments[..2].to_vec(), ..four_segments() };
    let built =
        build(&request(workout, &["rock"], false), &fx.ctx(None, None, Some(&analyzer)), &|_| {}).await.unwrap();
    assert!(analyzer.calls.load(Ordering::SeqCst) <= 3, "budget 3, called {}", analyzer.calls.load(Ordering::SeqCst));
    let song = &built.plan.songs[0];
    assert_eq!((song.source, song.bpm), (Some(BpmSource::Analysis), Some(92)));
    assert_eq!(song.pulse, PulseSource::SongBeat, "the analysis gave the grid");
    assert!(song.grid.is_some());

    // budget 0 or no analyzer: no tempo known, so only the metronome rung can pick them
    let none = Fixture {
        settings: ExerciseSettings {
            sources: SourcesMode::LibraryOnly,
            analysis_budget: 0,
            ..ExerciseSettings::default()
        },
        library: LibraryIndex::new((0..4).map(|n| library_song(n, "Rock", None, 200)).collect()),
    };
    let analyzer = FakeAnalyzer { calls: AtomicUsize::new(0) };
    let built = build(&request(four_segments(), &["rock"], false), &none.ctx(None, None, Some(&analyzer)), &|_| {})
        .await
        .unwrap();
    assert_eq!(analyzer.calls.load(Ordering::SeqCst), 0);
    assert!(built.plan.songs.iter().all(|s| s.fit.is_none() && s.pulse == PulseSource::Metronome));
    assert!(built.plan.notes.iter().any(|n| n.contains("metronome")));
}

#[tokio::test]
async fn a_nearly_fitting_song_widens_the_ladder_and_nothing_at_all_is_a_worded_error() {
    // 72 BPM is 8 RPM under a 80-95 warm-up: only the third rung (+6) is not enough, double-time is not it either;
    // 78 is 2 under: the first rung takes it
    let fx = Fixture {
        settings: ExerciseSettings { sources: SourcesMode::LibraryOnly, ..ExerciseSettings::default() },
        library: LibraryIndex::new(vec![library_song(0, "Rock", Some(73), 200)]),
    };
    let workout =
        Workout { segments: vec![seg(SegmentKind::WarmUp, (80, 95), Position::Seated, None)], ..four_segments() };
    let built = build(&request(workout.clone(), &[], false), &fx.ctx(None, None, None), &|_| {}).await.unwrap();
    assert!(built.plan.notes.iter().any(|n| n.contains("closest")), "{:?}", built.plan.notes);
    assert!(built.plan.songs[0].fit.is_some_and(|f| f.error_rpm > 4.0));

    let empty = Fixture::new();
    let err = build(&request(workout.clone(), &[], false), &empty.ctx(None, None, None), &|_| {}).await.unwrap_err();
    assert!(err.to_string().contains("Connect Tidal"), "{err}");
    let err = build(&request(Workout::default(), &[], false), &fx.ctx(None, None, None), &|_| {}).await.unwrap_err();
    assert!(err.to_string().contains("no segments"), "{err}");
}

/// A library with a song of every length and tempo the presets can ask for.
fn big_library() -> LibraryIndex {
    let mut songs = Vec::new();
    let mut n = 0;
    for bpm in [70u16, 90, 110, 140, 170] {
        for seconds in (60..=600u64).step_by(15) {
            songs.push(library_song(n, "Rock", Some(bpm), seconds));
            n += 1;
        }
    }
    LibraryIndex::new(songs)
}

#[tokio::test]
async fn every_style_at_any_length_builds_close_to_the_request_and_keeps_the_caps() {
    let fx = Fixture {
        settings: ExerciseSettings { sources: SourcesMode::LibraryOnly, ..ExerciseSettings::default() },
        library: big_library(),
    };
    for style in PresetId::ALL {
        for minutes in [10u16, 25, 47, 90] {
            let req = RideRequest { minutes, style, ..RideRequest::default() };
            let built = build(&req, &fx.ctx(None, None, None), &|_| {}).await.unwrap();
            let asked = u32::from(homefi_core::exercise::round_minutes(minutes)) * 60;
            let got = built.plan.workout.total_seconds();
            assert!(got.abs_diff(asked) * 5 <= asked, "{style:?} {minutes} min: {got} s vs {asked} s");
            assert_eq!(built.plan.songs.len(), built.plan.workout.segments.len(), "{style:?} {minutes}");
            // the caps hold after the songs snapped the segments
            assert_eq!(beginner_caps(&built.plan.workout, &fx.settings), built.plan.workout, "{style:?} {minutes}");
            for (song, seg) in built.plan.songs.iter().zip(&built.plan.workout.segments) {
                assert!(seg.interval.is_none_or(|iv| iv.total_s() <= seg.seconds));
                assert!(song.entry.duration_ms > 0);
            }
        }
    }
}

#[tokio::test]
async fn the_chat_turns_words_into_a_request_and_a_failed_turn_leaves_it_clean() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .and(body_string_contains("set_workout"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "msg_c", "type": "message", "role": "assistant", "model": "claude-x", "stop_reason": "tool_use",
            "content": [
                { "type": "text", "text": "A 45 minute ride with rock and metal. How hard?" },
                { "type": "tool_use", "id": "t1", "name": "set_workout",
                  "input": { "title": "Rock and metal", "minutes": 45, "style": "classic", "genres": ["rock", "metal"], "mix": true } },
                { "type": "tool_use", "id": "t2", "name": "suggest_replies", "input": { "options": ["Easy", "Hard"] } }
            ],
            "usage": { "input_tokens": 1, "output_tokens": 1 }
        })))
        .mount(&server)
        .await;
    let mut session = ExerciseSession::new(ai_config(&server), RideRequest::default(), Lang::En);
    let opener = session.start();
    assert!(!opener.quick_replies.is_empty(), "the opener needs no AI call");
    assert_eq!(server.received_requests().await.unwrap().len(), 0);
    let turn = session.reply("45 minutes, rock and metal").await.unwrap();
    assert_eq!(turn.text, "A 45 minute ride with rock and metal. How hard?");
    assert_eq!(turn.quick_replies, ["Easy", "Hard"]);
    assert_eq!(
        (turn.request.minutes, turn.request.style, turn.title.as_str()),
        (45, PresetId::Classic, "Rock and metal")
    );
    assert_eq!(turn.request.wish.genres, ["rock", "metal"]);
    assert!(turn.request.wish.mix);

    let broken = MockServer::start().await;
    Mock::given(method("POST")).respond_with(ResponseTemplate::new(500)).mount(&broken).await;
    let mut failing = ExerciseSession::new(ai_config(&broken), RideRequest::default(), Lang::En);
    assert!(failing.reply("hello").await.is_err());
    assert_eq!(failing.request(), &RideRequest::default());
}

/// Sixty songs of one length at tempos for every cadence (a song plays once per ride).
fn uniform_library(seconds: u64) -> LibraryIndex {
    let bpms = [70u16, 78, 85, 90, 95, 100, 105, 110, 120, 128, 140, 150, 160, 170, 176, 65, 75, 88, 98, 115];
    LibraryIndex::new((0..60).map(|n| library_song(n, "Rock", Some(bpms[n % bpms.len()]), seconds)).collect())
}

#[tokio::test]
async fn the_ride_ends_close_to_the_length_asked_whatever_the_songs_and_songs_are_cut_to_their_parts() {
    use homefi_core::exercise::RideClock;
    for song_seconds in [200u64, 360] {
        let fx = Fixture {
            settings: ExerciseSettings { sources: SourcesMode::LibraryOnly, ..ExerciseSettings::default() },
            library: uniform_library(song_seconds),
        };
        for style in PresetId::ALL {
            for minutes in [10u16, 25, 47, 90] {
                let req = RideRequest { minutes, style, ..RideRequest::default() };
                let built = build(&req, &fx.ctx(None, None, None), &|_| {}).await.unwrap();
                let plan = &built.plan;
                let asked = u32::from(homefi_core::exercise::round_minutes(minutes)) * 60;
                let got = plan.workout.total_seconds();
                let label = format!("{style:?} {minutes} min, {song_seconds} s songs: {got} s vs {asked} s");
                assert!(got.abs_diff(asked) * 5 <= asked, "{label}");
                assert_eq!(plan.songs.len(), plan.workout.segments.len(), "{label}");

                // the clock's segment boundaries are the plan's: cut songs skip at the segment's end
                let mut start_s = 0;
                for (i, (song, seg)) in plan.songs.iter().zip(&plan.workout.segments).enumerate() {
                    let seg_ms = u64::from(seg.seconds) * 1000;
                    if song.entry.duration_ms > seg_ms {
                        assert_eq!(song.play_ms, Some(seg_ms), "{label} part {i}");
                    } else {
                        assert_eq!(song.play_ms, None, "{label} part {i}");
                    }
                    let inside = RideClock::view(plan, i, seg_ms.saturating_sub(1_000), 0);
                    assert_eq!((inside.segment_index as usize, inside.segment_start_s), (i, start_s), "{label}");
                    assert!(!inside.skip_to_next, "{label} part {i}");
                    if let Some(end) = song.play_ms {
                        let cut = RideClock::view(plan, i, end, 0);
                        assert!(cut.skip_to_next && cut.remaining_s == 0, "{label} part {i}");
                        assert_eq!(cut.elapsed_s, start_s + seg.seconds, "{label} part {i}");
                    }
                    start_s += seg.seconds;
                }
                assert_eq!(start_s, got, "{label}");
            }
        }
    }
}

/// Tidal's search for "<artist> <title>" as the AI wrote it: the track that comes back may be by someone
/// else (a cover band). Rows: (artist asked, title, artist of the track found, tempo).
struct Catalog(&'static [(&'static str, &'static str, &'static str, i64)]);

impl Respond for Catalog {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let query =
            request.url.query_pairs().find(|(k, _)| k == "query").map(|(_, v)| v.into_owned()).unwrap_or_default();
        let items = self
            .0
            .iter()
            .enumerate()
            .filter(|(_, (asked, title, _, _))| query == format!("{asked} {title}"))
            .map(|(i, (_, title, found, bpm))| track(700 + i as i64, title, found, Some(*bpm), SONG_SECONDS))
            .collect();
        ResponseTemplate::new(200).set_body_json(page(items))
    }
}

fn two_flats() -> Workout {
    let flat = seg(SegmentKind::SeatedFlat, (85, 100), Position::Seated, None);
    Workout { segments: vec![flat.clone(), flat], ..four_segments() }
}

async fn requests_to(server: &MockServer, verb: &str) -> Vec<String> {
    let sent = server.received_requests().await.unwrap();
    sent.iter().filter(|r| r.method.as_str() == verb).map(|r| String::from_utf8_lossy(&r.body).into_owned()).collect()
}

#[tokio::test]
async fn new_songs_searches_again_asks_the_ai_for_others_and_plays_new_songs_first() {
    let server = MockServer::start().await;
    const ROWS: &[(&str, &str, &str, i64)] = &[
        ("Metallica", "Enter Sandman", "Metallica", 92),
        ("Iron Maiden", "The Trooper", "Iron Maiden", 90),
        ("Megadeth", "Symphony of Destruction", "Megadeth", 94),
        ("Slayer", "Raining Blood", "Slayer", 96),
    ];
    Mock::given(method("GET")).and(path("/v1/search")).respond_with(Catalog(ROWS)).mount(&server).await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(proposals(&[
            &[("Enter Sandman", "Metallica"), ("The Trooper", "Iron Maiden")],
            &[("Symphony of Destruction", "Megadeth"), ("Raining Blood", "Slayer")],
        ]))
        .mount(&server)
        .await;
    let api = tidal_api(&server);
    let ai = ai_config(&server);
    let fx = Fixture::new();
    let ctx = fx.ctx(Some(&api), Some(&ai), None);
    let ask = request(two_flats(), &["metal"], false);
    let titles = |b: &homefi_core::exercise::RideBuilt| -> Vec<String> {
        b.plan.songs.iter().map(|s| s.entry.title.clone()).collect()
    };

    let (first, memory) = build_with_memory(&ask, &ctx, &|_| {}, None).await.unwrap();
    assert_eq!(titles(&first), ["Enter Sandman", "Symphony of Destruction"], "the AI's best-known first");
    let (searches, ai_calls) = (requests_to(&server, "GET").await.len(), requests_to(&server, "POST").await.len());
    assert_eq!((searches, ai_calls), (4, 1));

    // "new songs": everything is searched again, the AI is told what it proposed, and unheard songs come first
    let (second, memory) = build_with_memory(&ask, &ctx, &|_| {}, Some(memory)).await.unwrap();
    let posts = requests_to(&server, "POST").await;
    assert_eq!(posts.len(), 2, "the AI is asked again");
    assert!(posts[1].contains("do not propose any of these again") && posts[1].contains("Metallica - Enter Sandman"));
    assert!(requests_to(&server, "GET").await.len() > searches, "Tidal is searched again");
    assert_eq!(titles(&second), ["The Trooper", "Raining Blood"]);

    // other inputs: the memory is not used, the AI is not told about the other ride
    let other = request(two_flats(), &["rock"], false);
    build_with_memory(&other, &ctx, &|_| {}, Some(memory)).await.unwrap();
    let posts = requests_to(&server, "POST").await;
    assert_eq!(posts.len(), 3);
    assert!(!posts[2].contains("do not propose any of these again"));
}

#[tokio::test]
async fn a_proposal_that_resolves_to_another_artist_is_left_out() {
    let server = MockServer::start().await;
    const ROWS: &[(&str, &str, &str, i64)] = &[
        // Tidal answers a lullaby cover for the asked song: same title, someone else
        ("Metallica", "Enter Sandman", "Rockabye Baby!", 92),
        ("Iron Maiden", "The Trooper", "Iron Maiden", 90),
    ];
    Mock::given(method("GET")).and(path("/v1/search")).respond_with(Catalog(ROWS)).mount(&server).await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(proposals(&[&[("Enter Sandman", "Metallica"), ("The Trooper", "Iron Maiden")]]))
        .mount(&server)
        .await;
    let api = tidal_api(&server);
    let ai = ai_config(&server);
    let fx = Fixture::new();
    let workout = Workout { segments: two_flats().segments[..1].to_vec(), ..four_segments() };
    let built =
        build(&request(workout, &["metal"], false), &fx.ctx(Some(&api), Some(&ai), None), &|_| {}).await.unwrap();
    assert_eq!(built.plan.songs[0].entry.artist, "Iron Maiden", "the cover is not the song asked for");
}

/// Reads only songs with a file, like the phone's analyzer.
struct FilesOnly(FakeAnalyzer);

impl Analyzer for FilesOnly {
    fn can_read(&self, entry: &PlaylistEntry) -> bool {
        entry.local_path.is_some()
    }

    fn analyze<'a>(
        &'a self,
        entry: &'a PlaylistEntry,
    ) -> Pin<Box<dyn Future<Output = Option<DjAnalysis>> + Send + 'a>> {
        self.0.analyze(entry)
    }
}

#[tokio::test]
async fn songs_the_analyzer_cannot_read_do_not_use_up_the_budget() {
    let server = MockServer::start().await;
    // the AI's first pick is a Tidal-only song with no tempo, its second a library file with no tag
    Mock::given(method("GET")).and(path("/v1/search")).respond_with(SongSearch).mount(&server).await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(proposals(&[&[("Unknown Tempo", "Mystery"), ("Song 0", "Artist 0")]]))
        .mount(&server)
        .await;
    let api = tidal_api(&server);
    let ai = ai_config(&server);
    let fx = Fixture {
        settings: ExerciseSettings { analysis_budget: 1, ..ExerciseSettings::default() },
        library: LibraryIndex::new(vec![library_song(0, "Metal", None, SONG_SECONDS as u64)]),
    };
    let analyzer = FilesOnly(FakeAnalyzer { calls: AtomicUsize::new(0) });
    let workout = Workout { segments: two_flats().segments[..1].to_vec(), ..four_segments() };
    let built = build(&request(workout, &["metal"], false), &fx.ctx(Some(&api), Some(&ai), Some(&analyzer)), &|_| {})
        .await
        .unwrap();
    let song = &built.plan.songs[0];
    assert_eq!(song.entry.title, "Song 0");
    assert_eq!(song.source, Some(BpmSource::Analysis), "the one analysis went to the file, not the Tidal-only song");
    assert_eq!(analyzer.0.calls.load(Ordering::SeqCst), 1);
}
