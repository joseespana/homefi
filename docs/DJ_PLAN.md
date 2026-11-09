# DJ mode: plan

Status: approved direction (José, 2026-11-06). Built in phases; each phase ships without breaking the
normal player.

## Decisions

- **Mouse first, no controllers.** No MIDI. Every control must feel good with a mouse or trackpad:
  drag faders and knobs (vertical drag, Shift = fine, double-click = reset), the wheel on any control,
  click the overview waveform to jump, drag the zoomed waveform to nudge or scrub. Keyboard shortcuts
  are optional extras (rebindable), never required.
- Vinyl tempo in the MVP (pitch follows tempo, our own Hermite varispeed). Key lock later
  (signalsmith-stretch, MIT, desktop feature; Android has Sonic).
- Headphone pre-listen (second output / channels 3–4) in phase 2.
- A song picked while DJ mode is on loads into the free deck (setting `load_target`: free_deck / ask / crate).
- 8 sampler pads, synthesized by us (no samples shipped): air horn, siren, laser, rewind, drop, scratch,
  riser, hit.
- DJ mode on = never bit-perfect; the normal engine is untouched when it's off.
- Everything is a setting on desktop and Android. Defaults and ranges live in `homefi_core::dj::settings` only.
- Party/share: while DJ mode is on, guests see "DJ set" and get no audio until a master-mix stream exists (later).

## Architecture

- `homefi-core/src/dj/` (pure, Android-safe): `settings.rs` (`DjSettings`, lenient serde, `clamped()`),
  `analysis.rs` (`analyze(mono, rate) -> DjAnalysis { bpm, first_beat_ms, beat_ms, confidence,
  overview: Vec<OverviewSlice{peak, low, mid, high}>, loudness_db }`, spectral-flux onsets →
  autocorrelation 60–200 BPM → phase fit; cache `~/.homefi/dj/analysis/<key>.json`, atomic write),
  `mixer_math.rs` (`channel_eq_profile`, `CrossfaderCurve { Smooth, Linear, Cut }::gains`),
  `sfx.rs` (`render(SfxKind, rate) -> Vec<f32>`, deterministic, ≤ 3 s, peak ≤ 0.9), `keymap.rs`
  (defaults + conflicts), `sync.rs` (`tempo_for_sync`, `phase_offset_ms`).
- Prep: the symphonia decoder moves from `homefi-player/src/decoder.rs` to `homefi-core::audio::decode`
  (feature `decode`) so player, FFI and disc share it.
- `homefi-player/src/dj/` (desktop, real time): a second engine that owns the cpal stream while DJ mode
  is on. `DjMixer::render(out, channels)` runs in the callback over two RAM-resident decks
  (`Arc<DeckAudio>` at source rate), mixer and sampler; pure over its state so it's tested offline.
  Commands by a lock-free ring (`try_pop` only); replaced `Arc`s go back on a return ring and are dropped
  off the audio thread; no allocation or lock in the callback; positions/peaks published as atomics;
  `DjSnapshot` assembled on the engine thread. Fixed buffer from `DjSettings.latency` (low ≈ 256 /
  safe ≈ 1024 frames) with fallback; every device call under `output.rs::device_queries()`.
  Memory budget `DjSettings.memory_mb` (1024 desktop / 256 Android); over budget → humane refusal.
- Tauri `src-tauri/src/dj.rs`: `dj_enable`, `dj_state`, `dj_load(deck, Path|Tidal{id})` (Tidal via the
  existing stream cache), `dj_control(DjControl)`, `dj_visual`, `dj_overview(deck)`; events `dj` (20 Hz)
  and `dj-analysis`. Settings in `DesktopSettings.dj`.
- Svelte: `lib/dj.svelte.ts` (state, IPC, keys), `components/dj/*` (draw only), canvas waveforms fed
  by non-reactive arrays, view `dj` in the rail, mock `?dj`.
- Android (simple DJ): `homefi-ffi/src/dj.rs` (analysis, sfx, channel EQ, crossfader gains, sync, settings)
  and `player/dj/DjSession.kt` on the two ExoPlayers of `Crossfade.kt`; `ui/dj/DjScreen.kt`.

## MVP features

Decks A/B: load, play/pause, cue, 8 hot cues, auto loops 1/2/4/8 beats + in/out/exit, tempo fader
(±8 % default), pitch bend, sync (tempo + phase), BPM/beat grid, colored overview + zoomed scrolling
waveform. Mixer: trim, 3-band EQ with kill, bipolar filter, channel faders, crossfader with curve,
master, soft limiter, auto-gain. Sampler: 8 pads. Android: two deck cards, Cue/Play/Sync, 4 hot cues,
loop 4/8, tempo with key lock, volume, EQ sheet, filter, crossfader, 4 pads.

## Phases

0. Prep: shared decoder, `dj` settings. (rust-pro)
1. Core math: analysis + cache, mixer math, sfx, keymap, sync. (rust-pro)
2. Real-time engine `homefi-player::dj`. (rust-pro)
3. Tauri commands, events, settings. (backend-developer)
4. Desktop UI (ui-designer spec first, then frontend-developer).
5. Android simple DJ. (kotlin-specialist)
6. Phase 2: key lock, headphone cue, echo / filter-sweep FX.
7. Later: record the mix, reverb, Auto-DJ with the AI, master stream to party guests.
