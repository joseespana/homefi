# Changelog

All notable changes to HomeFi are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/). One version number covers the CLI,
the core library and the Android app.

## [Unreleased]

## [1.5.11] - 2026-11-10

### Added
- Android TV: the Train setup now has the phone's genres, how famous the songs are, the era and
  "My rides" (ride a saved one again with no AI call), all with the remote; the form scrolls.
  Liked and avoided artists and the AI chat stay on the phone.

### Fixed
- Desktop: the update notice, the notifications and the "nearby" banner were see-through in the
  Immersive skin and hard to read over the cover. They now have a solid background.
- Exercise: the ride's sounds (countdown, part change, sprint, finish) were loud and sharp enough to
  startle. They are 6 dB quieter, lower and softer (a gentle swell, no horn), and the default cue
  volume is 60 %. The volume and "Sounds" switch stay in the exercise settings.
- Exercise: "Build my ride" and "New songs" could come back in a moment without asking the AI,
  with leftover songs (some outside the genres picked): they reused what an earlier build had found.
  Every build now searches again: the AI is asked each time (on "New songs" it is told what it
  proposed before, so it picks others) and the songs already heard go last.

## [1.5.10] - 2026-11-10

### Added
- Android TV: Settings, two panes (pages on the left, rows on the right), every setting reachable
  with the remote: look, player, sound and DJ, downloads, TIDAL, party, this TV, lyrics, AI and
  About, including every TV setting (the player controls hide time reads "Never" when 0). The AI
  page sets the provider, key (typed with the TV keyboard, shown as dots, kept in the Keystore and
  never logged), model and server, so the AI chats work on the TV; the playlist chat's "set up an
  AI" opens it. The TV keyboard gained capitals and a symbols page (punctuation, Spanish letters).
- Android TV: a download going to a USB drive asks for "All files access" every time it is
  missing and explains why; "Use the app folder" is the way to say no (asked again next time, but
  not for the rest of that queue). "What's new" can be reopened from Settings, About.
- Android TV: DJ mode, with a mouse. The section opens the phone's two decks over the TV when a
  mouse (USB or Bluetooth) is connected, or always when "DJ mode" is set to On.
- Android TV: Train runs on the TV itself. Pick a length and a style with the remote, build the
  ride, start it and watch it on the TV's own screen (the ride page the phone also sends to TVs),
  with pause, next part and end on the remote, and a summary at the end.

- Exercise: three quick taste questions under the genres, on the phone and the desktop: how famous
  ("Big hits", the default, "Mix" or "Discover"), the era (any, or the 60s to the 2020s) and,
  folded away, the artists you like (with suggestions from your library) and artists never to
  play. Every answer is saved as a setting, so the next ride starts from it. "Big hits" now really
  picks the genre's famous songs by its best-known acts.

### Fixed
- AI answers cut off at the output limit no longer build an empty ride: long rides ask for fewer songs per
  segment and a bigger limit, each provider's "cut off" signal is read, and the user sees a clear message.
- AI prompts treat what you type (genres, favourite and avoided artists, notes, chat words) as data:
  line breaks are flattened and the text sits in tags that cannot be closed from inside. Song prompts no
  longer translate titles, named artists beat the "only hits" rule, and an avoided artist is also avoided
  as a guest. A model that writes its songs as JSON text instead of a tool call is still understood.
- The sound reaction sends the AI only the sounds around what plays (not every saved sound) and the
  line shown is always the app's own, so it can't contradict what changed.
- Concert research tells the AI today's date, skips the second AI call when nothing was found, and only
  links web (http/https) pages. Anthropic caches the EQ assistant's fixed instructions between rounds.
- Desktop: the queue panel no longer freezes on long queues. It draws only the rows near the
  screen (5,000 songs opened in 2.6 s, now about 70 ms); dragging to reorder keeps working across
  the whole queue with auto-scroll, and the panel opens on the playing song.
- Android starts faster: no false "last run ended" crash report at every start (it ran logcat on
  the main thread), the Rust library loads in the background, the second player only exists when
  crossfade is on and the TV server is built when a ride needs it. The release APK drops x86_64
  (about half the size).
- Desktop player: the audio callback no longer allocates (its buffer is sized when the stream opens),
  the level meters fold surround by side without a per-sample modulo, the gapless hand-over opens
  the next file once and no longer enumerates audio devices, and the queue is shared between
  snapshots instead of copied four times a second.
- Exercise: a ride for a genre such as metal could be filled with songs by little-known artists.
  The AI's well-known picks lost their place when a long part of the ride was split in two, songs
  found by a search ("metal workout") competed on tempo alone, and a song the AI asked for could
  turn into a cover by another artist. Now the AI's picks keep their ranking, unknown artists only
  fill in when nothing famous fits, and a match by a different artist is left out.
- Exercise: "New songs" (was "Rebuild") stays on the ride's preview with its progress there,
  instead of leaving it; a failure or Cancel keeps the plan you had. It is also much quicker: it
  reuses the songs the last build found and plays the ones it did not, asking the AI again only
  for the parts with nothing new left.

## [1.5.9] - 2026-11-09

### Added
- HomeFi for Android TV and Google TV (first part). The same app shows a TV interface on a TV:
  sign in to TIDAL with a QR or a code, Home, your library, search (on-screen keyboard, voice
  where the TV has it), your TIDAL collection, downloads to the TV or a USB drive (paused when
  the drive is pulled out), a full-screen player with lyrics and karaoke, an ambient mode while
  music plays, your playlists, the sound settings, a party others join from their phones, and
  updates by itself. The player's controls stay on screen (hiding them is a setting).
- Rides on the TV without typing anything: the TV shows a QR, the phone scans it once, and from
  then on every ride opens on the TV by itself. The ride screen on the TV was redesigned and can
  show the song's lyrics in time with the music (a setting). "Put HomeFi on your TV" in the
  phone's settings explains how to install it with the free Downloader app.
- Saved rides: every ride you build is saved by itself, so you can ride it again without asking
  the AI; keep, rename, favourite, delete, and export or import them as files (to a folder,
  Google Drive or anything the share menu offers).
- DJ mode setups: rearrange or hide the sections, choose the sound on each sampler pad, rebind
  the keys, and save it all as named setups you can export and load on another computer, phone
  or tablet.
- Exercise: the AI favours the well-known songs of the bands and songs you name.
- Exercise: a new course mountain on the ride screen, setup, preview, summary and the TV. Height and ground colour show
  effort, the zone is a thin line on the ridge and a ribbon at the base, with a cyclist, a pin for the next part and
  summit glow. Settings: Course look (Ribbon, Alpine, Stage) and Summit times.
- DJ mode: Homifito DJ. The little house with headphones stands behind his own booth (two turntables, a
  mixer, speakers and lights) and mixes with you: he bobs on the beat, scratches, reaches for the crossfader
  during a mix, cheers on air horns, jumps on drops and lifts one headphone cup while the other deck is
  cued. Off, Booth or Big stage, above or below the decks, with optional Homifito colours for the whole
  board; the button in the DJ top bar turns him on and off. He follows Look → Motion.
- DJ mode works on narrow windows: no more "make the window wider". The board lays itself out by its own
  width (decks side by side with the mixer under them, or stacked, the sampler wrapping).
- Exercise mode, starting with indoor cycling (desktop and Android). Pick how long (10 to 90
  minutes), a style (first ride, easy, classic, intervals) and the music of the day: one genre or a
  mix, plus a sentence for the AI. HomeFi builds the ride with songs whose beat matches the pedaling
  (checked by BPM from the file, TIDAL or its own analysis) and shows the target RPM, a pulse on the
  beat, effort, seated or standing, the time left, what's next and the whole class as a mountain.
  Sounds count you in and mark the changes. On a TV: a fullscreen window on the desktop, any TV
  browser by its address, or the new HomeFi app for Android TV and Chromecast with Google TV, which
  finds the ride on its own and can play the music too (or leave it on the phone, for headphones).
- The sidebar folds what doesn't fit into a "More" menu on short windows, and page headers fold
  their secondary buttons into icons on narrow windows instead of breaking the labels.
- Player beside the app on wide screens. On a tablet or a phone in landscape the player sits on
  the right while you browse, with a button for full screen (and one back to the side). On a wide
  desktop window, "Now playing" shows in a column on the right: the cover, the song, its actions
  and what comes next; its button opens the full room. Automatic: it measures the screen and sizes
  itself. To remove it, a switch in the player's menu on Android, the side player button on the
  desktop, or Settings on both.
- Save HomeFi's playlists to TIDAL (desktop and Android). A playlist made with AI, for a party or
  for a concert gets a "Save to TIDAL" button: it's added to your TIDAL account, so it shows up on
  every device signed in to it. Afterwards "Update on TIDAL" makes that copy match HomeFi's again
  (same songs, same order). Songs from your library are found on TIDAL by ISRC or by title and
  artist; the ones TIDAL doesn't have are left out and listed. Settings → Playlists → Save to TIDAL
  can save them automatically and delete the TIDAL copy when you delete the playlist in HomeFi
  (both off by default).
- Android: simple DJ mode. Two decks with the song's waveform, Cue / Play / Sync, one-tap "Mix to B"
  (or A) that fades over 8, 16 or 32 beats lined up on the beat with the bass swapped, a crossfader,
  an EQ and filter sheet, four sampler pads (you pick which of the eight sounds) and, in the full
  view, tempo with "keep the pitch", hot cues and loops. Songs come from your library, the queue or
  TIDAL. Everything is a setting (Settings → DJ), and the Simple view is the default.
- Desktop: DJ mode, made for the mouse. Two decks with a zoomed, scrolling waveform (beat grid,
  hot cues, loop) and the whole song at a glance (click to jump, drag to scrub), Cue / Play / Sync,
  a mixer with three-band EQ and kill, filter, trim, channel faders with meters and a crossfader
  with Smooth / Linear / Cut curves, and a big "Mix to B" (or A) that brings the other song in on the
  beat over 8, 16 or 32 beats with the bass swapped. Eight synthesized sampler pads, hot cues, loops,
  a tempo fader with nudge buttons, and a crate to drag songs from your library or a TIDAL search
  onto a deck. Starts in the Simple view (Sync on, the pro controls hidden; "Show all controls"
  brings them back), with a first-mix card. Every knob and fader drags up and down (Shift for fine),
  takes the wheel and resets on double-click; keyboard shortcuts are optional and can be changed.
  While DJ mode is on the normal player pauses (it comes back where it was) and a song you play
  anywhere in the app goes to the free deck, a question or the crate, as you choose. Everything is a
  setting (Settings → DJ), including the DJ button in the sidebar. DJ mode is never bit-perfect; a
  shared session or party shows nothing playing while it is on.
- Mini player (PiP): volume. Point at the speaker for a slider, or use the wheel or ↑ ↓; a click
  still mutes.

## [1.5.8] - 2026-11-06

### Fixed
- macOS: after an update HomeFi opens on the Dock's pinned icon instead of a second one. The new
  version now opens only once the old one has quit (takes effect from the update after this one).

## [1.5.7] - 2026-11-05

### Added
- Android Auto: lyrics in the car. The line being sung shows under the title in the car's player,
  one line at a time, when the song has synced lyrics (Settings → Playback → Android Auto → Lyrics in
  the car, on by default; the phone's own screens are unchanged once the car disconnects).
- Android Auto: albums and HomeFi's playlists open as a grid of covers, and songs in lists show their
  cover (read from the files and handed to the car read-only).

## [1.5.6] - 2026-11-05

### Added
- Android Auto: HomeFi shows up in the car. Browse Recently played, the playlists HomeFi made, Albums,
  Artists and Songs, and search by voice ("play … on HomeFi"); picking a song in a list plays the
  list from it. The car shows its own player with the cover and controls (lyrics stay on the phone:
  cars don't let music apps draw their own screens). Settings → Playback → Android Auto
  turns it off or hides any section.

### Fixed
- macOS: the mini player opens again. It waited for a first frame before showing itself, and macOS
  draws no frames in a hidden window, so it closed itself after a few seconds.

## [1.5.5] - 2026-11-05

### Changed
- Playlist with AI: a centred, responsive screen. The chat is one card with the field at its foot,
  what the AI understood (title, the songs that open it, length, energy) shows as tags beside Build,
  and errors read as a clear note. A message the AI couldn't answer goes back to the field to send
  again, instead of staying in the chat unanswered.
- With the player buttons on top, "How does it sound?" sits with them in the top bar; the player bar
  keeps the output, Sound and the volume. The output button has the same corners as its neighbours.

### Fixed
- Playlist with AI: the AI answers in its own words again (models that call their tools first used to
  leave only "Got it: … Anything else?"). A build where none of the AI's songs could be found (TIDAL's
  searches failing or timing out, or no songs suggested) now says so instead of giving a playlist of
  just the song you named; a playlist that is short because TIDAL didn't answer says how many searches
  failed.
- macOS: HomeFi still closed by itself now and then (a crash inside CoreAudio while listing outputs
  or reading their sample rates). The cause was in the audio library: it handed CoreAudio a size the
  optimised build treated as zero, so the device list was written to an invalid address. Fixed in
  HomeFi's copy of the library.
- macOS: the mini player could open off screen (leaving only the minimised main window) when it
  started with lyrics or Homefito on; it now always opens whole in the screen's corner and stays
  on screen when it grows.

## [1.5.4] - 2026-11-05

### Changed
- One set of rounded corners across the desktop app (small, medium, large, extra and pill), with
  nested corners that follow the shape around them; the Settings page list marks the open page with
  a soft tile and shows its scrollbar only when pointed at; scrollbars are clean capsules.

### Fixed
- macOS: HomeFi could close by itself when the list of audio outputs was read while a song was
  starting (a crash inside CoreAudio). Device queries now happen one at a time.
- Settings: when you scroll, the page title and search sit on a floating frosted pill, like a song
  row's hover, instead of a half-width box.
- macOS: updating keeps the HomeFi icon you pinned in the Dock. The update now replaces what is inside
  `HomeFi.app` instead of the whole app, so the Dock still recognises it (this applies from the next
  update after this version is installed).

## [1.5.3] - 2026-11-05

### Fixed
- Playlist with AI: songs you name in the chat ("alors dance") are recognised (Alors on danse by Stromae),
  open the playlist and set its sound; the rest has to match their genre, era, language and energy, not
  just the mood. The AI says back what it understood, and a playlist can no longer be built before your
  request reached it (it used to come out as a generic list). Same on Android.
- The Tidal mark in the side bar is centred.
- Search: the quality and "versions" tags of Tracks and Albums are no longer cut off; a row's hidden
  buttons take no room until you point at it.

## [1.5.2] - 2026-11-04

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

