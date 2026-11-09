# Exercise mode: plan

Status: approved direction (José, 2026-11-09). Indoor cycling (spin class) first; weights and other workouts
later on the same model. Built in phases; each phase ships without touching the normal player.

## Decisions

- **José's answers (2026-11-09).** The music is picked by genre for the day: one ("rock", "metal")
  or a mix of several, so the ride gets different rhythms. This is phase 1, not a refinement:
  the setup screen has genre chips (multi-select, plus a free sentence) and the AI picks songs
  per segment from those genres to fit each segment's cadence (in a mix, a genre goes where its
  tempo fits best: e.g. metal for sprints, rock for climbs, something calmer for warm-up and
  cool-down). Without an AI key the same genres drive Tidal and library search, checked by BPM.
  Defaults taken for the other open questions until he says otherwise: library and Tidal
  together; both TV paths; no tempo nudge yet; "Entrenar" in the rail and tabs; the health
  notice once; synthesized cues.
- **Duration is his choice** (any length in a clamped range, 5-minute steps); presets are styles
  (easy, classic, intervals) that scale to it.
- **Chromecast and Android TV always in mind.** The free way: HomeFi for Android TV (a TV launcher
  activity in the same APK) on an Android TV or a Chromecast with Google TV finds the ride on the
  network (mDNS `_homefi-ride._tcp`) and shows it full screen; `/ride.json` is a versioned
  contract. Android's own screen cast to any Chromecast is the zero-code fallback. A Google Cast
  custom receiver (needed only for the older Chromecasts without Google TV) costs a one-time
  registration fee: asked to José, not built.
- **The course as a mountain.** The TV (and the ride screen) draws the workout as an elevation
  profile: hard segments are climbs, easy ones flat, recovery and cool-down descents, with a marker
  moving along it (the indoor-cycling classes José took; no brand copied).
- **Two TV modes (setting `tv_audio`):** the TV plays everything, music included (it joins the
  share hub as a listening guest on the same clock, with a one-time ride token), or visual only on
  the TV while the music stays on the phone or computer (headphones).
- **Beginner first, music drives the pedal.** A ride is a list of segments (warm-up, flat, climbs,
  sprints, recovery, cool-down). Each segment has a target cadence (RPM); HomeFi picks songs whose
  BPM matches it at 1:1 or half-time, so "pedal on the beat" is literally true. One song per segment
  (how real classes work); sub-song intervals (sprint / recovery) run inside one song, matched to
  the work phase.
- **AI first, in plain words, manual fallback.** "A 30-minute easy ride with 90s rock" → the user's
  own AI (BYOK, the `PartySession` pattern) turns it into a `Workout` + a music wish. Without AI:
  presets (20 / 30 / 45 min) and a simple segment editor. Songs are never invented by the AI: they
  come from the library and Tidal and are checked by BPM.
- **BPM from three free sources, in this order:** the library file's `BPM` tag, Tidal's `bpm` field
  (`models::Track.bpm`, already parsed), and our own analysis (`dj::analysis`, cached). No new
  dependency, no paid service.
- **The ride plays in the normal player** (desktop `homefi-player::Player`, Android Media3) with the
  ride saved as a HomeFi playlist (`PlaylistOrigin::Ride`), so Sounds, lyrics, Tidal sync and the
  queue just work and bit-perfect stays bit-perfect. Tempo nudging (a few %) is phase 2 and a
  setting that is off by default.
- **Beat pulse is computed, not detected:** the song's beat grid (`first_beat_ms`, `beat_ms`) plus
  the player position gives the phase; no microphone, no live follower. The TV page does the same
  math locally from a small JSON.
- **TV in phase 1 = two free paths:** desktop opens a second fullscreen window on the TV's screen
  (same pattern as the mini player, `index.html?mini`), and both platforms can serve a LAN page the
  TV browser opens by QR / URL. Google Cast custom receivers (registration fee + hosted page) and
  DLNA (media only) are out. "Mirror your screen" is the documented zero-code fallback.
- **Target only in phase 1.** Real cadence (BLE Cycling Speed & Cadence 0x1816 / Fitness Machine
  0x1826, both free standard GATT profiles) is phase 3. Phone motion sensors are not used (a phone
  in a pocket is not a cadence sensor).
- **Everything is a setting on desktop and Android;** defaults and ranges live once in
  `homefi_core::exercise::settings`.
- **Not medical advice.** A short notice (once, dismissible) before the first ride; no heart-rate or
  calorie claims.

## The domain (what a spin class looks like)

Practical numbers, general knowledge; all are defaults in `exercise::presets`, all configurable.

| Segment | Cadence (RPM) | Effort (RPE 1–10) | Position | Typical length |
| --- | --- | --- | --- | --- |
| Warm-up | 80–95 | 3–4 | seated | 5–8 min (one or two songs) |
| Seated flat | 85–100 | 5–6 | seated | 3–5 min (a song) |
| Seated climb | 60–80 | 6–8 | seated | 3–5 min |
| Standing climb | 55–70 | 7–8 | standing | 2–4 min |
| Jumps | 80–90 | 6–7 | alternating every 8 beats | 2–4 min |
| Sprints (intervals) | 100–120 work / 80–90 rest | 9 / 3–4 | seated | 20–30 s work, 60 s rest, 4–6 rounds |
| Recovery | 80–90 | 3–4 | seated | 1–3 min |
| Cool-down | 70–85, falling | 2–3 | seated | 3–5 min |

- **Music → cadence.** One pedal stroke (one leg down) per beat at **1:1** for songs of 60–110 BPM;
  **half-time** (a stroke every two beats) for 120–180 BPM songs → 60–90 RPM, the usual climb
  music; **double-time** only for very slow songs (50–60 BPM → 100–120 RPM), off by default.
  A song fits a segment if `|bpm × ratio − target| ≤ tolerance` (default 4 RPM) for an allowed ratio.
- **Phrases.** Pop and dance music moves in 8-beat bars / 32-beat phrases; jumps and interval
  changes land on phrase boundaries (the builder snaps work/rest lengths to whole bars).
- **Beginner caps** (`beginner_mode`, on): cadence 60–110, standing never under 60 RPM (knees),
  sprints ≤ 30 s with work:rest 1:2, effort ≤ 8, 20–30 min rides. Effort is shown as words plus the
  number ("Comfortable 4", "Hard 7"): a resistance cue, not a measurement.
- **Presets:** *First ride* 20 min (warm-up, flat, seated climb, recovery, cool-down), *Easy 30*
  (adds jumps and one short interval block), *Classic 45* (two climbs, two interval blocks).
  Weights later use the same `Segment` with a different `Target` (see "Later").

## Architecture (layered like DJ mode)

- `homefi-core/src/exercise/` (pure, Android-safe):
  - `model.rs`: `Workout { version, id, name, sport: Sport, segments: Vec<Segment>, origin: WorkoutOrigin }`,
    `Segment { kind: SegmentKind, seconds: u32, target: Target, effort: Effort, position: Position,
    interval: Option<Interval>, note: String }`, `SegmentKind { WarmUp, SeatedFlat, SeatedClimb,
    StandingClimb, Jumps, Sprint, Recovery, CoolDown }`, `Target::Cadence { min_rpm, max_rpm }`
    (tagged enum: weights add `Reps` later without a migration), `Effort(u8)` 1..=10,
    `Position { Seated, Standing, Alternating { beats: u8 } }`, `Interval { work_s, rest_s,
    rounds, rest_cadence }`, `Sport::IndoorCycling`, `WorkoutOrigin { Preset(PresetId), Ai { wish,
    provider_id, model }, Custom }`. Lenient serde (`#[serde(default)]`, `#[serde(other)]`).
  - `presets.rs`: the three presets + `beginner_caps(&Workout, &ExerciseSettings) -> Workout`.
  - `settings.rs`: `ExerciseSettings` + `clamped()` + the `MIN_/MAX_/DEFAULT_` constants.
  - `bpm.rs`: `BpmSource { Tag, Tidal, Analysis }`, `fit(bpm, &Target, &Tolerance) -> Option<Fit { ratio: BeatRatio, error_rpm }>`,
    `BeatRatio { Whole, Half, Double }` and the widening ladder (`tolerance → tolerance + 3 → + 6
    → allow double-time → Tidal search → metronome`), pure and tested.
  - `builder.rs`: `build(request: &RideRequest, ctx: &BuildContext<'_>, progress) -> Result<RideBuilt>`
    in the shape of `concert::build` (same `BuildContext { settings, ai, api, library, lang }`, a
    `Progress` callback, cancellation by `CancellationToken`). Candidates: library index with
    `TrackInfo.bpm` (phase 0 adds it), Tidal search by the wish/genre with `Track.bpm`, then
    `dj::analysis` only for the few candidates still unknown (`analysis_budget`, default 12 songs
    per build, sequential `spawn_blocking`, decode capped by `DjSettings.memory_mb`). Output
    `RidePlan { workout (durations snapped to the chosen songs, ±20 % max), songs: Vec<PlannedSong {
    segment_index, entry: PlaylistEntry, bpm, source: BpmSource, fit: Option<Fit>, grid:
    Option<Grid>, pulse: PulseSource { SongBeat, Metronome } }>, notes }`, plus the `HomePlaylist`
    (`PlaylistOrigin::Ride(RideOrigin { workout_id, wish })`) saved through `PlaylistStore`.
  - `ai.rs`: the AI tool `set_workout` (segments in plain fields, validated against the model and
    the beginner caps) reusing `party::session`'s chat loop; `words.rs` for the fixed texts (EN/ES).
  - Music wish = `MusicWish { genres: Vec<String>, mix: bool, sentence: String }` on the
    `RideRequest`. With AI: one call per build proposes candidate songs (title + artist, never
    ids) per segment from the wish and each segment's cadence; each is resolved on Tidal / the
    library (the `resolve` path of playlists) and kept only if its BPM fits. With `mix`, the
    prompt spreads the genres over the segments by tempo. Without AI: Tidal search by genre
    (+ library genre tag), then the same BPM fit.
  - `ride.rs`: `RideClock::view(plan, queue_index, position_ms, now_ms) -> RideView { segment,
    next, remaining_s, progress, target_rpm, effort, position, interval_phase: Option<Work|Rest{..}>,
    pulse: PulseView { first_beat_ms, beat_ms, ratio } , cues: Vec<Cue> }`. Pure: segment advances
    with the queue index, interval phases with the song position; pause = nothing moves.
  - `cues.rs`: `CueKind { Countdown3, Countdown2, Countdown1, Change, Standing, Seated, SprintGo,
    Recover, Finish, Click }` rendered with the synth helpers of `dj::sfx` (make them `pub(crate)`),
    deterministic, ≤ 1 s, peak ≤ 0.85. `schedule(&RideView) -> Vec<(at_ms, CueKind)>`.
  - `tv.rs`: `RideTvView` (the JSON the TV page polls: `RideView` + `server_now_ms`) and the
    embedded page (`include_str!("tv/ride.html")`, no framework, no network beyond the page).
    `TvServer::start(bind, ArcSwap<Option<Arc<RideTvView>>>) -> Result<TvServer>` on axum (already
    behind feature `share`), routes `/` and `/ride.json` (`Cache-Control: no-store`), default port
    `47801`, stops with the ride. Separate from `share::Hub` on purpose: no guests, no code, no mDNS.
- `homefi-ffi/src/exercise.rs`: records for the model, `exercise_presets()`, settings
  default/clamped/to_json/from_json (DJ pattern), `HomefiEngine::exercise_build(request, settings)
  -> RidePlan` (async, cancellable), `exercise_ride_view(plan, queue_index, position_ms, now_ms)`,
  `exercise_cue_wav(kind, rate)`, `ExerciseTv` object (`start(port) -> url`, `publish(view)`, `stop`),
  `ExerciseSession` for the AI chat.
- Desktop, Tauri `src-tauri/src/exercise.rs`: `exercise_presets`, `exercise_default_request`,
  `exercise_open/reply/close` (AI), `exercise_build` (progress by Channel like `concert_build`),
  `exercise_cancel`, `exercise_start(plan)` (saves the playlist, loads the queue with its
  `SourceContext`, keeps the plan in `AppState`), `exercise_stop`, `exercise_tv(on) -> url`,
  `exercise_tv_window(on)` (second webview, `index.html?tv`, fullscreen on the chosen monitor);
  event `ride` (`RideView`, emitted with every player snapshot and on segment change). Settings in
  `DesktopSettings.exercise`.
- Svelte: `lib/exercise.svelte.ts` (state, IPC, cue playback with WebAudio from `exercise_cue_wav`),
  `components/exercise/{RideSetup,RideScreen,TvView,SegmentList}.svelte` (draw only; the pulse
  uses `useSongClock` + `beat_ms`), view `exercise` in the rail, settings page `exercise`, mock `?ride`.
- Android: `player/exercise/RideSession.kt` on `PlayerController` (plan, 10 Hz view, cues through a
  `SoundPool` like `DjSampler.kt`, keep-screen-on, `ExerciseTv` lifecycle inside `PlaybackService`),
  `ui/exercise/{RideSetupScreen,RideScreen}.kt`, `ui/settings/ExerciseSettings.kt`, strings EN/ES.

Concurrency and memory: building runs on tokio with blocking decode in `spawn_blocking`, one
analysis at a time, cancellable between songs; the plan is small and shared as `Arc<RidePlan>`;
the TV view is published through `ArcSwap` (the HTTP handler clones an `Arc`, never the view);
nothing touches an audio callback. Failures: no fitting song → widen, then Tidal, then "any song
with a metronome pulse", always a humane note on the plan; Tidal offline → library only; TV port in
use → next free port once, then a humane error; analysis cache full → warn and continue.

## The ride screen

Big target RPM (the center of the range, the range small under it), a beat pulse (ring / bar /
flash, setting) that beats with the song at the chosen ratio, segment name and position icon
(seated / standing), effort as words + number, segment timer and "next: standing climb · 65 RPM"
from `segment_notice_seconds` before the change, a progress bar of the whole ride with segment
marks, the song that plays, and a 3-2-1 countdown before a change. Pause / skip segment / end ride.
Cues are synthesized sounds (phase 1); voice is later. The TV page and the desktop TV window show
the same, larger, without controls, dark by default, with the QR on the phone only.

## Settings (`ExerciseSettings`, desktop and Android)

`show_entry` (true), `beginner_mode` (true), `default_preset` (first_ride), `ride_minutes`
(20..=90 step 5, 30), `cadence_tolerance_rpm` (2..=12, 4), `allow_half_time` (true),
`allow_double_time` (false), `sources` (library_first / tidal_first / library_only, library_first),
`analysis_budget` (0..=50, 12), `allow_explicit` (true), `countdown_seconds` (0..=10, 3),
`segment_notice_seconds` (5..=30, 10), `audible_cues` (true), `cue_volume` (0..=1, 0.8),
`metronome_click` (false), `pulse_style` (ring / bar / flash, ring), `pulse_offset_ms`
(−500..=500, 0; prefilled from the lyrics offset for Bluetooth speakers), `keep_screen_on` (true),
`tv_enabled` (false), `tv_port` (1024..=65535, 47801), `tv_window_fullscreen` (desktop, true),
`health_notice_seen` (false), phase 2: `tempo_nudge_pct` (0..=8, 0). Tests: `defaults_follow_the_plan`,
clamping, lenient JSON like `dj::settings`.

## Phases

0. Prep (rust-pro): `TrackInfo.bpm: Option<u16>` from tags (`TBPM` / `BPM`) in `library.rs` and its
   cache; `dj::sfx` synth helpers `pub(crate)`; `PlaylistOrigin::Ride` (lenient, old JSON unaffected).
1. Core (rust-pro): `exercise/{model,presets,settings,bpm,builder,ride,cues,tv}` + FFI. Tests:
   presets respect the caps and sum to their length; `fit` picks half-time for 170 BPM at 85 RPM and
   refuses double-time by default; widening order is deterministic; `RideClock` segment/interval
   phases and pause; cue schedule; settings clamp; TV JSON shape and HTML escaping; builder with a
   fake library and wiremock Tidal (no network in CI).
2. Desktop (backend-developer for Tauri + TV server glue; ui-designer spec, then
   frontend-developer): commands, `ride` event, setup screen (preset / AI / editor), ride screen,
   TV window, settings page, mock `?ride`, headless screenshots.
3. Android (kotlin-specialist screens; mobile-developer for keep-screen-on, the TV server in the
   foreground service, QR): same features, strings EN/ES, emulator screenshots.
4. Phase 2: tempo nudge (Android `PlaybackParameters` with pitch kept; desktop through a varispeed
   in `homefi-player` or a single DJ deck, decided then), voice cues (platform TTS, offline, setting),
   AI chat refinement ("make the climbs longer").
5. Phase 3: real cadence by BLE CSC / FTMS (Android `BluetoothLeScanner`, permissions already in the
   manifest; desktop `btleplug`), live "a bit faster / slower" feedback, ride history.
6. Later: weights and other workouts (`Sport::Strength`, `Target::Reps { count, seconds_per_rep }`,
   music BPM maps to rep tempo, rest timers), HIIT / rowing presets, a ride shared to party guests.

## Risks

- Tidal's `bpm` is missing on many tracks and library files rarely carry a `BPM` tag, so the
  builder leans on analysis (1–3 s per song): the budget keeps a build under ~30 s; a "analyze my
  library in the background" job is a possible follow-up, not phase 1.
- Beat detection confidence under `MIN_GRID_CONFIDENCE` or a wrong octave (half/double) → the plan
  shows the fit and the pulse can fall back to the metronome; the user can tap "wrong tempo" (×2 / ÷2).
- Pulse accuracy on Bluetooth speakers: position-based pulse plus `pulse_offset_ms`; good enough for
  pedaling, not for DJ sync.
- TV browsers vary (Samsung / LG / Fire TV have one, Apple TV and many Android TVs do not): hence the
  desktop window and the mirroring fallback. Polling at 2 Hz keeps the page simple and robust.
- Sub-song intervals rely on the song keeping one tempo; songs with tempo changes are skipped when
  the analysis shows low confidence.

## Open questions for José

1. Which TV / box do you have (brand, Android TV, Chromecast, Apple TV)? Does it have a browser,
   or will the Mac be plugged by HDMI? This decides whether the LAN page or the desktop window is
   the first thing we polish.
2. Ride lengths and the three presets: do 20 / 30 / 45 and the cadence ranges above match what you
   remember from your classes? Anything you want harder or softer by default?
3. Music: your library first, Tidal first, or ask each time? Should the AI pick the vibe from a
   sentence, or do you prefer choosing genres / artists by hand on the setup screen?
4. Tempo nudge (phase 2): OK to speed songs a few % during a ride (not bit-perfect, like DJ mode)?
5. Sensors (phase 3): would you buy a BLE cadence sensor (~20 €), or does your bike have Bluetooth
   (FTMS) already?
6. Cues: synthesized sounds in phase 1; do you want a voice ("de pie", "sprint") later?
7. Where in the app: an "Entrenar" entry in the rail / tabs (proposed), or inside Playlists?
8. The health notice: one short sentence, shown once before the first ride. Fine?
