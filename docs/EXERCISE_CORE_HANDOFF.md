# Exercise mode: core hand-off (phases 0–1 done, 2026-11-09)

Read with `docs/EXERCISE_PLAN.md`. The Rust core and FFI are built and tested (`make check` green).
This note is what the app phases (Tauri/Svelte, Kotlin, Android TV) consume and what is left.

## Core API (`homefi_core::exercise`)

- `RideRequest { minutes (10..=90, step 5), style: PresetId, wish: MusicWish { genres, mix, sentence }, workout: Option<Workout> }`.
  `PresetId`: `FirstRide`, `Easy`, `Classic`, `Intervals` (styles at any length).
- `preset(style, minutes, &settings)`, `beginner_caps(..)`.
- `build(&RideRequest, &BuildContext { settings, ai, api, library, analyzer: Option<&dyn Analyzer>, lang }, &progress)
  -> RideBuilt { plan: RidePlan, playlist: HomePlaylist }` (150 s deadline; drop the future to cancel; progress
  `Searching | Resolving | Analyzing | Matching`). The core can't decode: the caller injects an `Analyzer`.
- `RidePlan { workout, songs: Vec<PlannedSong>, notes, cues, wish, profile: Vec<ProfilePoint { t_s, elevation 0..=1 }> }`.
- `RideClock::view(&plan, queue_index, position_ms, now_ms) -> RideView` (pure; fields include `finished, segment_index,
  kind, position, effort, target_rpm, range, remaining_s, elapsed_s, total_s, progress, marker, interval, notice, next,
  pulse { source, first_beat_ms, beat_ms, stroke_ms, ratio, song_position_ms }, cues [{ id, kind, in_ms }]`).
- Cues: `cues::{render, wav, wav_16bit_mono}`, `CueKind::ALL`. Profile: `profile(&Workout)`, `elevation_at`.
- AI chat: `ExerciseSession::{new, start, reply}` (tool `set_workout`); one `propose_ride_songs` call per build.
- Texts: `health_notice(lang)`, `needs_ai_message(lang)`, `style_name(lang, style)`.
- Settings: `ExerciseSettings` (+ `lenient_exercise`, `clamped()`): show_entry, beginner_mode, default_preset,
  ride_minutes 30, cadence_tolerance_rpm 4, allow_half_time, allow_double_time, sources, analysis_budget 12,
  allow_explicit, countdown_seconds 3, segment_notice_seconds 10, audible_cues, cue_volume 0.8, metronome_click,
  pulse_style ring|bar|flash, pulse_offset_ms, keep_screen_on, tv_enabled, tv_audio tv|phone, tv_port 47801,
  tv_window_fullscreen, tv_window_display ("auto" or a screen name), ride_always_dark (true), haptic_cues (true, Android
  only), health_notice_seen, tempo_nudge_pct (phase 2).
- TV: `RideTvView { v: 1, server_now_ms, sampled_ms, playing, title, audio, listen: Option<RideListen { port, code, name }>,
  profile, parts [{ kind, start_s, seconds }], ride, text, pulse_style, paused, countdown_s }` (+ `from_json`;
  the last four additions are additive, `v` stays 1: `paused` = a ride runs and the music is paused, `countdown_s` = the 3-2-1
  digit due within the last second, `pulse_style` = ring|bar|flash). `TvSession` gained `parts` (`TvPart::of(&workout)`) and `pulse_style`, `render_page`, `is_lan`, `ride_token()`, `listen_permissions()`.
  Behind `share`: `TvServer::start(TvConfig { port, name, lang, announce })` → `begin(TvSession)`, `publish(view, playing)`,
  `clear`, `url`, `stop`; routes `/` (embedded `tv/ride.html`, visual only) and `/ride.json`; mDNS `_homefi-ride._tcp`
  (TXT v, name, port; never the token); `discover(timeout)`, `ride_hub_config(name, port, token)`.

## FFI (`crates/homefi-ffi/src/exercise.rs`)

Settings json helpers; `exercise_presets(lang)`, `exercise_preset`, `exercise_default_request`, `exercise_ride_minutes`;
`HomefiEngine.exercise_build(request, settings, ai, api_key, language, library, listener)`; `ExerciseRide { new(plan),
view(..), schedule(..) }`; `exercise_ride_view`; `exercise_cue_wav(kind, rate)`; `ExerciseSession`; `ExerciseTv { start(port,
name, announce, language) -> url, begin, publish(ride, queue_index, position_ms, now_ms, playing), clear, stop }`;
TV client: `exercise_tv_discover(timeout_ms)`, `exercise_tv_parse(json)`, `exercise_tv_protocol_version()`;
`exercise_ride_token()`, `exercise_listen_permissions()`; `exercise_health_notice`, `exercise_needs_ai_message`.
`LibraryRef` gained `bpm` and `genre` (defaults).

## Left for the app phases

- Tauri: implement `Analyzer` (player decoder + DJ analysis cache); `bpm` in `TrackView` (library_refs passes `bpm: None`
  today); commands + `ride` event on the same clock as the player; second TV window (`index.html?tv`); settings.
- Tv audio (`tv_audio = tv`): start the share hub for the ride with `ride_hub_config` (code = `ride_token()`, stream_audio on,
  `exercise_listen_permissions()`), feed it from the ride queue, pass `listen` into `begin`, stop it with the ride.
- Kotlin: multicast lock for discovery/announce; `ExerciseTv` inside the foreground service; cues via `exercise_cue_wav`
  + SoundPool; Android TV launcher activity as a pure client (discover → poll `/ride.json` at 2 Hz → `exercise_tv_parse`,
  join the hub with `GuestClient` when `listen` is present and draw the pulse from its own playback position).
- Google Cast custom receiver: not built (one-time registration fee; José decides).

## Added with the UI spec (desktop phase)

- `tv/ride.html` is the single TV renderer (browser, desktop TV window, Android TV `WebView`): zone-coloured mountain per part, next
  part dashed and flagged, 3-2-1 overlay, Paused, Reconnecting banner, bigger text, seated/standing icon, pulse `ring|bar|flash`,
  `?motion=off` (and `prefers-reduced-motion`), richer Finished screen and the hook `window.homefiSongPositionMs()` (a TV that plays
  the audio injects its own playback position; the pulse then follows it).
- `dj::analyze_file` / `decode_mono` (core, `dj/file.rs`): the DJ's decode + analysis cache, now shared by Android (FFI), and the
  desktop Exercise analyzer.
- FFI: `ExerciseSettings` (+3 fields), `RideTvView` (+`parts`, `pulse_style`, `paused`, `countdown_s`), `TvSession` (+`parts`,
  `pulse_style`), `TvPart`, `ExerciseRide.tv_parts()`.

## Saved rides and DJ setups (2026-11-09)

Core: `exercise::{RideStore, SavedRide, RideSummary, export_ride, import_ride, auto_name}`, `dj::{DjLayout, DjSetupStore,
DjSetup, export_setup, import_setup}`, `export::{wrap, unwrap, ExportKind}` (the shared envelope
`{"homefi_export": 1, "kind": "ride" | "dj_setup", "app_version", "exported_ms", "data"}`; `.homefiride`, `.homefidj`;
both plain JSON; no tokens, keys or absolute local paths: ride songs keep file names only and are matched again in the
importer's library by ISRC, then artist and title). Files: `~/.homefi/exercise/rides/<id>.json`,
`~/.homefi/dj/setups/<id>.json` (atomic, a bad file is skipped). Every successful build is auto-saved (`auto = true`,
name "30 min · Classic · Rock + Metal"); `ExerciseSettings.ride_history_max` (5..=200, default 30) prunes the oldest
automatic rides, never kept ones or favorites. Riding a saved ride again = load its plan and start it: no build, no AI.
`DjSettings.layout` = section order/visibility + the sound of each of the 8 pads (`DjMixer::with_pads`; pads are
rendered when DJ mode starts).

Desktop IPC (Tauri commands, errors are plain-language strings):

| Command | Returns |
| --- | --- |
| `exercise_build(...)` | `RideBuiltView { plan, playlist, saved: RideSummary? }` (auto-saved) |
| `exercise_rides()` | `RideSummary[]` (favorites first, then recent) |
| `exercise_ride_load(id)` | `RideLoaded { request, plan, playlist, saved }`; stage becomes `ready` (refused while riding/building) |
| `exercise_ride_keep(id, name?)` / `exercise_ride_rename(id, name)` / `exercise_ride_favorite(id, favorite)` | `RideSummary` |
| `exercise_ride_delete(id)` | `bool` (removes HomeFi's own file, no Trash) |
| `exercise_ride_export(id, path)` | path written (`.homefiride` appended if missing) |
| `exercise_ride_import(path)` | `RideSummary` (new kept ride) |
| `dj_setups()` | `DjSetupSummary[] { id, name, created_ms, updated_ms }` |
| `dj_setup_save(name)` / `dj_setup_overwrite(id)` / `dj_setup_rename(id, name)` | `DjSetupSummary` |
| `dj_setup_apply(id)` | `{ settings: DjSettings, restart_needed }` (also the `settings` event; pads apply at next DJ start) |
| `dj_setup_delete(id)` / `dj_setup_export(id, path)` / `dj_setup_import(path)` | `bool` / path / `DjSetupSummary` |

`RideSummary { id, name, minutes, style, genres, songs, created_ms, last_ridden_ms, times_ridden, favorite, auto,
profile }`. `ExerciseState.saved_id` names the saved ride of the current plan. Events: `exercise-rides` and `dj-setups`
(no payload) when a list changed. `exercise_start` counts the ride (`mark_ridden`). A local file that is gone plays from
TIDAL when the song has an id, otherwise it is skipped and `ExerciseState.notice` says how many.

FFI (Android): `ExerciseRides(app_dir)` (`list, get, auto_save, keep, rename, set_favorite, mark_ridden, delete, prune,
export -> String, import(json, library)`), `DjSetups(app_dir)` (`list, save, overwrite, apply -> {settings, layout},
rename, delete, export -> String, import(json)`), `dj_default_layout`, `exercise_ride_extension`, `dj_setup_extension`.
