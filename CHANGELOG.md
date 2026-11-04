# Changelog

All notable changes to HomeFi are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/). One version number covers the CLI,
the core library and the Android app.

## [Unreleased]

### Added
- Playlists: "Make a playlist with AI", on desktop and Android. A simple chat asks what kind of
  playlist you want today; say anything ("I'm feeling down, 90s music in English", "happy and
  upbeat, 30 minutes") and your own AI suggests directions, then builds it. Every song is checked
  against your library and TIDAL, and you can keep chatting to change it. Settings → Playlists → AI
  playlists sets the default length, energy and explicit lyrics.
- "Playlist like this": from the song playing or any song's menu, start that chat with the song
  (its artist, year and genre) so your AI makes a playlist with the same style, era and mood,
  starting with it. It can be hidden in Settings → Playlists → AI playlists.
- Discs, on desktop: rip audio CDs to your library. HomeFi asks what the disc is (artist, album,
  year, genre, cover, and whether to name the songs; blanks become "Disc 1", "Track 01"…), fills
  it in from the disc's CD-Text when it has one, and rips in secure mode (re-reads until the reads
  agree) or fast mode to FLAC (verified by decoding it back), WAV, or ALAC / MP3 / AAC / Opus with
  ffmpeg. Files are tagged and named like downloads. Looking discs up on MusicBrainz is an option,
  off by default. Copy-protected DVDs and Blu-rays are recognized and left alone: HomeFi doesn't
  bypass copy protection. Settings → Discs.
- Homefito's mosh mode, on desktop and Android: long hair with real physics, headbanging locked to
  the song's kick drum, horns, air guitar and fist pumps that change with the bars, a windmill when
  the song drops, a calm nod in quiet parts. Settings → Mascot: hair (auto / long / short), follow
  the beat, and headbang strength (gentle / normal / wild). "Mosh" can be pinned in the dance list.
- Mini player: karaoke lyrics and Homefito dancing, each with its own button (Settings → Listening
  room → Mini player). Android gets picture-in-picture with the same lyrics and Homefito options,
  and previous / play / next.
- Playlists: "Get ready for a concert", on desktop and Android. Type an artist and HomeFi asks your
  own AI (Claude, OpenAI, Gemini, Grok or OpenRouter) to search the web for the setlists of their
  latest shows, then builds the tour set in show order: each song is tagged Always, Usually,
  Sometimes or Encore, checked against your library and TIDAL (studio versions first, live ones
  when that's all there is), and comes with the links it was found in. When your AI can't search
  the web, or finds nothing, you get TIDAL's most played songs by the artist, clearly labeled as
  not the real setlist. Play it, shuffle it, queue it or save it.
- Playlists made by HomeFi are saved in HomeFi only (JSON in `~/.homefi/playlists`), never in your
  TIDAL account, and Sounds apply to them like to any playlist. Deleting one moves it to the Trash
  on desktop; on Android it can be undone.
- Settings → Playlists: how many recent shows to consider, the Always / Usually / Sometimes /
  Encore thresholds, the AI's search budget, preferring downloaded songs, skipping recorded intros,
  covers under the original artist, showing sources and the "Made by HomeFi" section.
- AI: web search through the provider you already set up, with Google's search suggestions shown
  for Gemini, as Google requires.
- Playlists: "Make a party playlist", on desktop and Android. Tap who's coming, their ages, the
  vibe, how long it lasts, the energy and whether explicit lyrics are fine, then have an optional
  short chat with your own AI. HomeFi checks every song against your library and TIDAL the same way
  as concert mode, so songs the AI makes up are left out. Preview it, play it and save it. Needs an
  AI set up, nothing else.
- Settings → Playlists → Party playlists: the default length, energy and explicit lyrics, and how
  many songs one artist can have.
- What's new: after an update, desktop and Android show once what changed since the version you
  last ran, read from this changelog inside the app, so it works offline. Settings → About shows it
  again, or turns it off.

### Changed
- Party and AI playlists talk in chat bubbles, with a typing indicator and quick replies under the
  last answer.
- Desktop: one Sound button in the player bar (the AI button is gone; the A key and the button's
  right-click menu still ask the AI), lyrics, queue and mini player move to a bar above the content
  (Settings → Listening room → Player buttons puts them back below), and the tag under the song now
  shows only the sound's name in a calm tone, with the full sentence on hover. On Android the same
  setting puts the Sound button in the top bar (long-press asks the AI).
- Search: the Top hit row spans the page so its title and quality badge are readable; quality
  badges never get cut.
- Desktop: the Settings header is a solid surface instead of a blur, lined up with the top bar.

### Fixed
- The signal chain names the equalizer after the sound playing ("Warm and airy") instead of the
  output's curve ("Flat"), and its card says who the sound is for. The output's curve only shows
  beside it when it actually changes something. Same on Android.

## [1.5.1] - 2026-11-02


One Sound screen instead of the equalizer and its taste rules, built around what is playing now, on
desktop and Android.

### Added
- Sound: one sound can apply to many songs, albums, artists, playlists and genres, and can be limited
  to some outputs ("only on my WH-1000XM5"). Add or remove music from the Playing now card, the
  Edit sound panel (search your library and TIDAL) or the new "Sound ▸" menu on any song, album,
  artist or playlist. Moving music between sounds can be undone for 8 seconds.
- Try: hear a sound (yours or one the AI proposes) on the song playing without saving it; Keep saves it
  for that song, album, artist or playlist. Leaving the song ends the try.
- "By music": every song, album, artist, playlist and genre with its sound, sortable and filterable,
  to move or remove them in one place.
- Ask the AI from the Sound screen (inline chat), the player ("Ask the AI" button, a setting) and the
  `A` key on desktop; a floating button and a bottom sheet on Android.
- Settings → Sound: the "Ask the AI" player button, and the bass / treble / clarity sliders inside
  Edit curve (now on Android too).

### Changed
- One switch: Sound on or off. Off is truly bit-perfect: no headphone correction, no sounds and no
  loudness normalization.
- Precedence is now song > album > artist > playlist > genre > everything else (an artist's sound
  wins over a playlist's).
- Settings from 1.5.0 are converted once: each taste rule becomes a sound for its music on all
  outputs, and Sound is switched on if your taste layers were on. If you had the equalizer on but
  taste layers off, your saved layers now play: remove them in Sound → By music if you don't want them.

### Removed
- The separate taste layers switch and the equalizer's "Taste" tab (everything lives in Sound).

### Fixed
- Desktop: the equalizer said "Off · bit-perfect" while a taste layer was still playing.
- The AI suggestion card no longer stays on its loading dots when the AI doesn't answer.

## [1.5.0] - 2026-10-30

### Fixed
- Desktop (Windows): surround and virtual surround outputs such as Razer's "7.1 Surround Sound"
  played with heavy interference. Streams now carry the device's speaker positions instead of
  none, through a patched copy of cpal 0.16 (`third_party/cpal`, see its `HOMEFI.md`).

## [1.4.0] - 2026-10-30


A bolder, more reliable AI sound assistant, Settings redesigned to fit any window or phone, and
HomeFito takes requests: pick its dance while the music plays. Desktop and Android installs from
1.2.0 on offer this update by themselves.

### Added
- Mascot: a dances button next to it (and a long-press on Android, a right-click on desktop) opens
  its actions: Wave, Next style, Put it back, Surprise me and each famous dance, with the one going
  on highlighted. Settings → Mascot: what a long-press / right-click does (its actions, the next
  style or nothing) and the dances button on or off.
- Sound: "Change size" for the AI assistant and the guided taste flow: Subtle, Balanced or Bold
  moves (still within the safe limits: ±9 dB a band, +12 dB in total, gentle on small speakers).
- Settings width (comfortable or wide), on desktop and Android.

### Changed
- The AI sound assistant thinks like a sound engineer: a frequency vocabulary, genre starting
  points and an intensity ladder, so "more bass" is a move you can hear, and one calibration
  question (more / less / perfect) instead of timid half-steps.
- Settings, redesigned on both platforms: a list and its page on phones and narrow windows, an
  icon rail, then a full side navigation, and two columns of groups on very wide screens. Every
  skin restyles it through its own tokens. The page title turns to frosted glass only while the
  page scrolls under it.
- Album cards: on hover the record slides out over the next card, with grooves, a sheen and a
  turning label (still with reduced motion).

### Fixed
- AI sound: a rule you deleted came back after the next suggestion; changes now apply one by one
  and never rewrite the whole list.
- Android: the mascot never got angry, because quick taps were taken in pairs as a double tap;
  every tap counts now, and a tap on the mascot never flips the player to the next view.

## [1.3.0] - 2026-10-29

Clean sound on surround and virtual 7.1 outputs like THX Spatial Audio, and a mute button in the
mini player. Desktop and Android installs from 1.2.0 on offer this update by themselves.

### Added
- Mini player: a mute button (and the M key) that silences the sound without changing the saved
  volume; every window follows it, and turning the volume brings the sound back.

### Fixed
- Surround and virtual 7.1 outputs (THX Spatial Audio and similar): the engine could put part of a
  frame in the output buffer when it was nearly full, shifting every later sample onto the next
  channel, heard as chirps. Only whole frames go in and out now.
- The play button on numbered track rows sits in the middle of its circle.

## [1.2.0] - 2026-10-29

HomeFi now updates itself: desktop and Android offer each new version (signed and checked)
and never cut the music without asking. Taste layers become full parametric curves the AI
designs band by band, and big playlist libraries no longer push the CPU to the top.

### Added
- Updates on every platform. Desktop: "HomeFi X is available · What's new · Update now · Later"
  with the release notes, a download progress bar and "Restart now" / "When the song ends" (the
  music is never stopped without asking); macOS, Windows (setup and MSI) and Linux (AppImage,
  .deb, .rpm) update in place through signed packages, other installs get a Download button.
  Settings → About → Updates: check automatically (on launch and every 6 hours), include
  pre-releases, install "Ask" or "When HomeFi is idle", Check now, last checked. Android (the
  official app): the same card and options; the APK is checked against the release's
  `SHA256SUMS` before Android's installer opens. Other builds (Astell&Kern, debug) say updates
  come by USB. CLI: a quiet once-a-day notice on stderr (`HOMEFI_NO_UPDATE_CHECK=1` or
  `update_check = false` turns it off). The only calls are to GitHub's public releases.
- Releases carry the updater's `latest.json` and minisign signatures (a tag fails without the
  `TAURI_SIGNING_PRIVATE_KEY` secrets).
- Taste layers get full parametric curves. A rule keeps its bass / treble / clarity knobs and now
  also holds up to 8 bands (peak, low / high shelf, high / low pass), each with an optional note
  from the assistant. Old rules load unchanged. The rule editor (desktop and Android) has
  **Simple** (the three sliders) and **Advanced** (the curve: drag, add, remove, Q with the wheel or
  the band editor) drawn over the output's profile with what plays dashed; rule cards summarize the
  curve ("3 bands · -2 dB 3.2k, +1.5 dB 65 Hz…"), and "Ask the AI to refine it" opens the assistant
  on that rule. "Sound for this…" gets the same editor; Settings → Sound picks which tab opens.
- Safety for curves: each band ±9 dB, peaks Q 0.3-8, a high-pass at most at 300 Hz and a low-pass
  from 4 kHz, the whole layer at most +12 dB anywhere (boosts are scaled down), and on built-in
  speakers no boost below 150 Hz (it becomes a lift at 200 Hz with a high-pass). The player runs
  the output profile and the layer together (up to 22 filters, still no allocation in the audio
  callback) with the automatic preamp computed on the combined response.
- The EQ assistant designs curves: `set_taste_rule` / new `propose_taste_rule` take `bands`, the new
  `analyze_sound` shows what already plays (device profile, layer, summed response at 31 points)
  so it edits instead of stacking bands, and whatever was clamped is reported back for it to
  explain. A proposed layer shows as a card with its curve, each band's reason, Apply and Keep
  talking. "How does it sound?" with AI may change up to 2 bands per tap (3 dB each); without AI,
  "too boomy" cuts 120 Hz and "too bright" softens 8 kHz and the harsh 3.5 kHz edge in turn.

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

