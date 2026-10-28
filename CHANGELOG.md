# Changelog

All notable changes to HomeFi are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/). One version number covers the CLI,
the core library and the Android app.

## [Unreleased]

## [1.1.0] - 2026-10-28

Sound you set up by talking: an equalizer per output with measured headphone corrections,
an AI assistant (your own key, many providers) or guided questions, taste layers that follow
what's playing, loudness normalization and crossfade. Surround output fixes playback on 7.1
systems, album versions are grouped, and the app no longer freezes on big Tidal lists.

### Added
- Taste layers: a bass / treble / clarity tilt for one genre, playlist, artist, album or track,
  on top of each output's equalizer profile (the most specific one plays: track > album >
  playlist > artist > genre). Applied on every track change with a 20 ms glide, at the track's
  first sample when playing gapless. Desktop: an EQ tab "Taste rules", "Sound for this…" on
  albums, playlists, artists and tracks, the layers playing next to the EQ chip.
- "How does it sound?": one tap (needs bass, too bright, can't hear the vocals, perfect...)
  makes or adjusts the rule for the music playing, with a one-line note and Undo. With an AI key
  the model picks where it goes and words the note; without, fixed safe steps (±2 dB, capped at
  ±6). Built-in speakers never get deep bass: a gentle lift around 200 Hz and a high-pass.
- Loudness normalization (off by default, so playback stays bit-perfect): per song, per album or
  automatic, to a target loudness, never clipping, from Tidal's ReplayGain (now written to
  downloaded and streamed files as `REPLAYGAIN_*` tags) or the file's own (`R128_*` for Opus).
- The EQ assistant offers taste layers for what you listen to most, can set loudness, and
  suggests one on a card; the guided setup offers one for your top genre.
- Album versions: the same album listed several times on Tidal (explicit / clean, stereo / Dolby
  Atmos, 16 / 24-bit, remasters, deluxe box sets) is one card on artist pages, search and favorite
  albums, with an "E" badge for explicit and a "N versions" chip that lists every release with its
  labels and a short description (Play / Open / Download each). Album pages show their labels and
  "Other versions". Grouped in `homefi-core` (`album_versions`: same main artist and kind, base title
  without version suffixes, live and studio apart, years and track counts as tie-breakers). Settings,
  desktop and Android: one card per album on / off, explicit or clean, stereo or Atmos, original or
  latest remaster.
- Crossfade (off by default: gapless and bit-perfect stay the default), desktop and Android with
  the same settings (`crossfade`, camelCase JSON in `homefi-core::dsp::crossfade`): 1–12 s,
  equal-power / S-curve / linear, smart trim of trailing and leading silence (below -60 dBFS),
  albums in order kept gapless, a short fade (0.5–3 s) on next / previous / picking a song, a
  minimum song length (twice the fade by default). Each song keeps its own equalizer, taste and
  loudness while they overlap. Desktop: the engine reads each song's end ahead, mixes the next
  one in the audio thread (resampled to the open device rate when it differs) with a soft limiter
  so overlapping peaks never clip; a "Fade" tag in the player bar, `fading` in the player state.
  Android: a second player for the overlap that becomes the session's player when the fade
  starts (notification, lock screen and queue follow), with a low-memory / lag fallback.
  Settings → Playback on both.
- Surround devices on the desktop: stereo on 5.1 / 7.1 speakers plays on the front pair, on all
  speakers (-3 dB) or upmixed (center, subwoofer below 120 Hz, rears at -6 dB); 5.1 and 7.1 files
  map 1:1 in the standard speaker order, and fold down on smaller layouts. Settings → Playback,
  shown when the device has more than two channels.
- Resampler quality on the desktop: High (default) or Fast, for computers where resampling is tight.

### Fixed
- Desktop: the sound broke up on 7.1 devices (Windows "Speakers (7.1 Surround Sound)", WASAPI shared
  mode at 48 kHz): every track was spread to 8 channels before resampling, so the sinc filter ran on
  6 silent channels too. It now resamples at the track's own channel count and then maps the
  channels (about a quarter of the CPU), and the output buffer is 1 s on Windows and surround
  devices (0.5 s elsewhere).

## [1.0.0] - 2026-10-28

First public version: the desktop app (macOS, Windows, Linux), the Android app and the CLI, on
one Rust core. Play your own files and your TIDAL subscription bit-perfect, with synced lyrics
and karaoke, eight listening-room layouts that theme the whole app, shared control and party
mode between devices, and offline copies in max quality. Every look and behaviour is a setting
on both apps.

### Added
- Personal, non-commercial use notice: the desktop and Android apps ask you to accept it before
  connecting a TIDAL account (and once, after updating, if already signed in); the CLI prints it
  on `auth login`. Also in the README, NOTICE and every release.
- Release workflow: the Android APK is built and signed in CI again, and every installer also
  ships with a version-free name so the README's download links always serve the latest release.
- Rust rewrite of the tiddl downloader: `homefi-core` (Tidal auth, API client,
  DASH/HLS stream parsing, path templates, FLAC/M4A/MP3 tagging, concurrent
  download engine) and the `homefi` CLI (`auth`, `download url|fav|search`, `config`).
- Android app (Kotlin + Jetpack Compose) using the same core through UniFFI:
  device login, search, link sharing, background downloads with notification.
- Android music player (Media3): Library tab with songs / albums / artists, embedded covers,
  format and resolution labels, mini player, full-screen player with queue, shuffle and repeat,
  background playback with media notification, "Open with HomeFi" for audio files, and tap-to-play
  in the downloads list. Works without a Tidal account.
- ALAC (and AC-3 / E-AC-3 / DTS / TrueHD) playback on Android through Jellyfin's Media3 FFmpeg
  decoder; platform decoders stay preferred. The APK is now GPL-3.0 (see NOTICE).
- `homefi_core::library`: recursive scanner with cache, tags and audio properties (FLAC, MP3, WAV,
  AAC, ALAC, Vorbis, Opus, FLAC-in-MP4, Matroska) and cover extraction (embedded or `cover.jpg`).
- Desktop: play straight from Tidal (tracks, albums, playlists, favorites, search) without adding
  them to the library. Each track is fetched just ahead of the player into a 3 GB cache, keeping
  playback bit-perfect and gapless; tracks you already downloaded play from the library.
  `Player::append` adds tracks to a running queue.
- Desktop mascot: "Bop", a standard head nod on the beat. Every dance now follows the estimated
  tempo; the mood comes from the genre tag, then words in the title/album (remix, acoustic...),
  then the tempo and loudness of the whole track.
- Desktop: your Tidal collection, editable: heart tracks / albums / artists / playlists (also the
  track playing in the deck), create playlists, add tracks (one or a whole album) to your playlists,
  remove tracks from and delete playlists you own. Core: `TidalApi::add_favorite`,
  `remove_favorite`, `create_playlist`, `add_to_playlist`, `remove_from_playlist`, `delete_playlist`.
- Desktop: Settings → Appearance: System, Light or Dark (also the native title bar).
- macOS builds are ad-hoc signed, so they open on Apple Silicon without an Apple Developer account.
- Release workflow: optional upload of every build to Cloudflare R2 (`releases/<version>`,
  `latest`, `main/<label>`, `main/latest`, each with a `manifest.json`).
- Desktop: the listening room and the deck show what the track being heard is: "In your library",
  or a Download button with its progress when it streams from Tidal; plus like and "Add to
  playlist". Local files find their Tidal id by ISRC (`TidalApi::tracks_by_isrc`).
- Desktop: delete downloads (one song, or everything of an album / playlist) — files go to the
  system Trash, and only files of the library can be deleted. Album / playlist pages say
  "Downloaded" when every track is local and "Download N missing" (only those) when some are.
- Desktop: deleting downloads shows "Deleting…" on each row, then the rows crumble to dust one
  after another ("Thanos snap", `lib/snap.ts`) and come back without their downloaded mark.
- Desktop downloads show their state on every row: waiting (queued behind another job), a progress
  ring while downloading, done or failed (click to retry); lists show "N of M downloaded".
  Download events now carry the Tidal `id` of the item.
- Desktop queue: album covers and drag-and-drop reordering (`Player::move_track`).
- Desktop mini player: a floating, always-on-top window (cover, controls, progress) that follows
  the album colors; the main window minimizes while it is open.
- Desktop mascot costumes per genre: horns, studded collar and Bowie bolt (rock/metal), mohawk and
  safety pin (punk), bow and glitter (pop), LED headband and glow sticks (electronic), hibiscus and
  hoops (latin), backwards cap and gold chain (hip-hop), beret, round glasses and bow tie (jazz/chill).
- Brand colors moved from violet to blue → teal → green; the icon gets sound waves, a softer
  house gradient, a band highlight and a glow.
- New mascot icon (a house dancing with headphones) generated by `assets/brand/build_icon.py`:
  logo, wordmark, adaptive + themed Android icon, notification icon and in-app logo.
- HomeFi logo, adaptive Android icon, release automation (`cargo xtask bump`,
  GitHub Actions release workflow).

### Fixed
- Desktop light theme: the room background stayed dark and the album accent was used for text,
  so pages were hard to read. The palette now also derives a dark "ink" for text and a light
  "paper" background from the cover.
- Desktop: clicking the track that is playing restarted it; it now pauses / resumes, and its
  row shows a pause button (library lists and Tidal lists).
- Desktop: playing the same Tidal track twice at once (double click, a new play while the first
  was still fetching) failed with "No such file or directory"; fetches of one track now wait for
  each other, and a replaced play no longer reports errors.
- Desktop: loading and error messages no longer squeeze under the transport controls; they are
  floating notifications in the bottom-right corner (`Notifications.svelte`).
- Desktop: playlist pages showed the track count twice.
- Desktop: tracks deleted outside HomeFi kept showing as downloaded; the library now drops missing
  files when the window comes to the front and every minute (`prune_library`).
- Desktop: rail tooltips were clipped and drawn behind the content.
- My Tidal: favorites and playlists failed with "Too big page" (Tidal allows 50 per page).

