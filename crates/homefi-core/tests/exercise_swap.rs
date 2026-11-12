//! Swapping one song of a built ride: from what the build found, never the song sent away, the part following
//! the new song's length, and a targeted search when there is nothing to take from. No real network.

use std::sync::Arc;

use homefi_core::ai::{AiConfig, AiProvider};
use homefi_core::api::{TidalApi, TidalClient};
use homefi_core::exercise::{
    build_with_memory, swap_song, BuildContext, BuildMemory, ExerciseSettings, MusicWish, Position, RideBuilt,
    RideRequest, Segment, SegmentKind, SourcesMode, SwapRequest, Target, Workout,
};
use homefi_core::humane::Lang;
use homefi_core::playlist::{LibraryIndex, LibraryRef};
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SONG_SECONDS: u64 = 200;

fn flat() -> Segment {
    Segment {
        kind: SegmentKind::SeatedFlat,
        seconds: SONG_SECONDS as u32,
        target: Target::Cadence { min_rpm: 85, max_rpm: 100 },
        position: Position::Seated,
        ..Segment::default()
    }
}

fn request() -> RideRequest {
    RideRequest {
        minutes: 10,
        wish: MusicWish { genres: vec!["rock".into()], ..MusicWish::default() },
        workout: Some(Workout { id: "w-swap".into(), segments: vec![flat(), flat()], ..Workout::default() }),
        ..RideRequest::default()
    }
}

fn song(n: usize, bpm: u16, seconds: u64) -> LibraryRef {
    LibraryRef {
        path: format!("/music/{n}.flac"),
        title: format!("Song {n}"),
        artist: format!("Artist {n}"),
        album: "Album".into(),
        duration_ms: seconds * 1000,
        bpm: Some(bpm),
        genre: "Rock".into(),
        ..LibraryRef::default()
    }
}

fn library_only(songs: Vec<LibraryRef>) -> (ExerciseSettings, LibraryIndex) {
    (ExerciseSettings { sources: SourcesMode::LibraryOnly, ..ExerciseSettings::default() }, LibraryIndex::new(songs))
}

fn ctx<'a>(
    settings: &'a ExerciseSettings,
    library: &'a LibraryIndex,
    api: Option<&'a TidalApi>,
    ai: Option<&'a AiConfig>,
) -> BuildContext<'a> {
    BuildContext { settings, ai, api, library, analyzer: None, lang: Lang::En }
}

fn titles(built: &RideBuilt) -> Vec<String> {
    built.plan.songs.iter().map(|s| s.entry.title.clone()).collect()
}

async fn built_with_memory(ctx: &BuildContext<'_>) -> (RideBuilt, BuildMemory) {
    build_with_memory(&request(), ctx, &|_| {}, None).await.unwrap()
}

async fn swap(
    built: &RideBuilt,
    index: usize,
    at_ms: u64,
    ctx: &BuildContext<'_>,
    memory: Option<BuildMemory>,
) -> homefi_core::Result<(homefi_core::exercise::Swapped, BuildMemory)> {
    swap_song(&SwapRequest { built, segment_index: index, at_segment_ms: at_ms }, &request(), ctx, &|_| {}, memory)
        .await
}

#[tokio::test]
async fn swap_takes_an_unused_pool_song_that_fits() {
    let (settings, library) = library_only((0..5).map(|n| song(n, 90, SONG_SECONDS)).collect());
    let ctx = ctx(&settings, &library, None, None);
    let (built, memory) = built_with_memory(&ctx).await;
    let before = titles(&built);

    let (swapped, _) = swap(&built, 0, 0, &ctx, Some(memory)).await.unwrap();
    assert!(swapped.from_pool, "the build had spare songs: no search");
    let after = titles(&swapped.built);
    assert_ne!(after[0], before[0]);
    assert_ne!(after[0], before[1], "not the song already in the ride");
    assert_eq!(after[1], before[1], "the other part is untouched");
    assert_eq!(swapped.built.playlist.entries[0].title, after[0], "the playlist follows the plan");
    assert_eq!(swapped.song.entry.title, after[0]);
    assert_eq!(built.plan.songs[0].entry.title, before[0], "the original ride is not changed");
}

#[tokio::test]
async fn the_rejected_song_never_comes_back() {
    let (settings, library) = library_only((0..5).map(|n| song(n, 90, SONG_SECONDS)).collect());
    let ctx = ctx(&settings, &library, None, None);
    let (mut built, mut memory) = built_with_memory(&ctx).await;
    let mut seen: Vec<String> = titles(&built);
    // three songs are spare: three swaps use them, each one new to the ride
    for _ in 0..3 {
        let (swapped, next) = swap(&built, 0, 0, &ctx, Some(memory)).await.unwrap();
        let fresh = swapped.built.plan.songs[0].entry.title.clone();
        assert!(!seen.contains(&fresh), "{fresh} had been in the ride");
        seen.push(fresh);
        built = swapped.built;
        memory = next;
    }
    // nothing is left that was not sent away or is in the ride: a worded error, not a repeat
    let err = swap(&built, 0, 0, &ctx, Some(memory)).await.unwrap_err();
    assert!(err.to_string().contains("no other song"), "{err}");
}

#[tokio::test]
async fn a_shorter_replacement_shrinks_the_segment_at_most_20_pct_and_updates_profile_and_playlist() {
    // two 200 s songs make the ride; the spare ones are 170 s (fits: shrinks the part) and 150 s (75 %: does not)
    let mut songs: Vec<LibraryRef> = (0..2).map(|n| song(n, 90, SONG_SECONDS)).collect();
    songs.push(song(2, 92, 170));
    songs.push(song(3, 92, 150));
    let (settings, library) = library_only(songs);
    let ctx = ctx(&settings, &library, None, None);
    let (built, memory) = built_with_memory(&ctx).await;
    assert_eq!(titles(&built), ["Song 0", "Song 1"]);
    let total_before = built.plan.workout.total_seconds();

    let (swapped, memory) = swap(&built, 1, 0, &ctx, Some(memory)).await.unwrap();
    let plan = &swapped.built.plan;
    assert_eq!(plan.songs[1].entry.title, "Song 2");
    assert_eq!(plan.workout.segments[1].seconds, 170, "the part follows the song");
    assert_eq!(plan.workout.total_seconds(), total_before - 30);
    assert_eq!(plan.profile.last().map(|p| p.t_s), Some(plan.workout.total_seconds()), "the profile is redrawn");
    assert_eq!(plan.songs[1].play_ms, None, "the song ends with its part");
    assert_eq!(swapped.built.playlist.entries[1].title, "Song 2");
    assert!(plan.songs[1].entry.note.contains("RPM"), "the note is the new song's");

    // the 150 s song covers 75 % of a 200 s part: not enough
    let err = swap(&swapped.built, 0, 0, &ctx, Some(memory)).await.unwrap_err();
    assert!(err.to_string().contains("no other song"), "{err}");
}

#[tokio::test]
async fn a_live_swap_starts_the_song_at_its_beginning_and_covers_what_is_left() {
    let mut songs: Vec<LibraryRef> = (0..2).map(|n| song(n, 90, SONG_SECONDS)).collect();
    songs.push(song(2, 92, 150));
    let (settings, library) = library_only(songs);
    let ctx = ctx(&settings, &library, None, None);
    let (built, memory) = built_with_memory(&ctx).await;

    // 60 s into the part 140 s are left: a 150 s song covers them and is cut at the part's end
    let (swapped, _) = swap(&built, 0, 60_000, &ctx, Some(memory)).await.unwrap();
    let new = &swapped.built.plan.songs[0];
    assert_eq!(new.entry.title, "Song 2");
    assert_eq!(new.at_segment_ms, 60_000);
    assert_eq!(new.play_ms, Some(140_000));
    assert_eq!(swapped.built.plan.workout.segments[0].seconds, 200);
}

#[tokio::test]
async fn a_swap_is_off_when_the_rider_turned_it_off() {
    let (mut settings, library) = library_only((0..4).map(|n| song(n, 90, SONG_SECONDS)).collect());
    let (built, memory) = built_with_memory(&ctx(&settings, &library, None, None)).await;
    settings.ride_swap = false;
    let err = swap(&built, 0, 0, &ctx(&settings, &library, None, None), Some(memory)).await.unwrap_err();
    assert!(err.to_string().contains("turned off"), "{err}");
}

// ------------------------------------------------------------------ the search

fn track(id: i64, bpm: i64) -> Value {
    json!({
        "id": id, "title": format!("rock-{id}"), "duration": SONG_SECONDS, "allowStreaming": true, "explicit": false,
        "popularity": 50, "bpm": bpm, "isrc": format!("TEST{id}"),
        "artist": { "id": id, "name": format!("rock band {id}"), "type": "MAIN" },
        "artists": [{ "id": id, "name": format!("rock band {id}"), "type": "MAIN" }],
        "album": { "id": 5, "title": "Album", "cover": null }
    })
}

async fn tidal(server: &MockServer) -> TidalApi {
    let items: Vec<Value> = (0..6).map(|n| track(100 + n, 90 + n)).collect();
    let body = json!({ "tracks": { "limit": 50, "offset": 0, "totalNumberOfItems": 6, "items": items } });
    Mock::given(method("GET"))
        .and(path("/v1/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
    let client =
        TidalClient::new("token").with_base_urls(format!("{}/v1", server.uri()), format!("{}/v2", server.uri()));
    TidalApi::new(Arc::new(client), "1", "US")
}

async fn requests_to(server: &MockServer, verb: &str) -> usize {
    server.received_requests().await.unwrap().iter().filter(|r| r.method.as_str() == verb).count()
}

#[tokio::test]
async fn without_a_pool_the_search_runs() {
    let server = MockServer::start().await;
    let api = tidal(&server).await;
    let settings = ExerciseSettings::default();
    let library = LibraryIndex::default();
    let ctx = ctx(&settings, &library, Some(&api), None);
    // a saved ride ridden again, or an app restarted: the ride exists but its memory does not
    let (built, _) = built_with_memory(&ctx).await;
    let searched = requests_to(&server, "GET").await;

    let (swapped, memory) = swap(&built, 0, 0, &ctx, None).await.unwrap();
    assert!(!swapped.from_pool);
    assert!(requests_to(&server, "GET").await > searched, "Tidal was searched for the replacement");
    let after = titles(&swapped.built);
    assert_ne!(after[0], titles(&built)[0]);
    assert_ne!(after[0], after[1]);

    // the swap left a memory: the next swap needs no search
    let searched = requests_to(&server, "GET").await;
    let (again, _) = swap(&swapped.built, 1, 0, &ctx, Some(memory)).await.unwrap();
    assert!(again.from_pool);
    assert_eq!(requests_to(&server, "GET").await, searched);
}

#[tokio::test]
async fn the_ai_is_asked_for_a_replacement_only_when_the_rider_allows_it() {
    let server = MockServer::start().await;
    let api = tidal(&server).await;
    // the AI is down: whatever is asked of it fails and the search carries on
    Mock::given(method("POST")).and(path("/messages")).respond_with(ResponseTemplate::new(500)).mount(&server).await;
    let ai = AiConfig::new(AiProvider::Anthropic, "model-x", Some("sk-test".into())).with_base_url(server.uri());
    let mut settings = ExerciseSettings::default();
    let library = LibraryIndex::default();
    let (built, _) = built_with_memory(&ctx(&settings, &library, Some(&api), None)).await;

    settings.swap_uses_ai = false;
    let (swapped, _) = swap(&built, 0, 0, &ctx(&settings, &library, Some(&api), Some(&ai)), None).await.unwrap();
    assert_eq!(requests_to(&server, "POST").await, 0, "swap_uses_ai off: no AI call");
    assert_ne!(swapped.built.plan.songs[0].entry.title, built.plan.songs[0].entry.title);

    settings.swap_uses_ai = true;
    let (swapped, _) = swap(&built, 0, 0, &ctx(&settings, &library, Some(&api), Some(&ai)), None).await.unwrap();
    assert!(requests_to(&server, "POST").await >= 1, "swap_uses_ai on: the AI is asked");
    assert_ne!(swapped.built.plan.songs[0].entry.title, built.plan.songs[0].entry.title, "a failed AI still swaps");
}
