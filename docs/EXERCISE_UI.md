# Exercise mode: UI spec (desktop + Android phone + TV)

Status: design for phases 2 (desktop) and 3 (Android + TV) of `docs/EXERCISE_PLAN.md`; data from
`docs/EXERCISE_CORE_HANDOFF.md`. Visual reference: `docs/exercise-mockup.html` (dark classic by default; the page toolbar
switches screens, light/dark and motion; open in Chrome at 1500 px wide; sample data only).

## 1. Rationale

- **One idea per screen.** José is a beginner on a bike, sweating, looking at a screen 1 to 3 m away. A ride screen answers
  five questions at a glance, in this order of size: *how fast do I pedal* (RPM, the biggest thing), *am I on the beat* (the
  pulse inside the RPM ring), *how long until it changes* (timer), *how hard* (effort bars + word), *what comes next*
  (next card, and the mountain). Everything else is small or hidden.
- **The mountain is the map.** The whole class is one elevation profile (`RidePlan.profile`, points `t_s` + `elevation 0..1`):
  climbs go up, flat stays flat, recovery and cool-down slide down. A marker rides along it; the next part is outlined
  (dashed) and flagged "Jumps in 2:00 · 84 RPM" so a climb is never a surprise. The same drawing is used in the preview, the
  ride screen, the TV and the summary (one component, one draw function).
- **Colour = kind of effort, never alone.** Each of the 8 segment kinds has a zone colour; text, icon and the number of
  effort bars repeat the information (colour-blind safe, sec. 8). The marker, ring, RPM digits, effort bars, timer bar and
  the current part of the mountain all use the current zone colour, so one glance gives the zone.
- **Music-first like InBike.** The pulse is the beat of the song at the pedal ratio (`pulse.stroke_ms`): "pedal when the ring
  closes". The plan preview shows the BPM fit per song ("176 BPM · half-time -> 88 ok") so the logic is visible and
  trustworthy, with "Another song" per part.
- **Mouse first on desktop, thumb first on the phone.** Desktop: controls >= 44 px, End needs a 1 s hold or a confirm.
  Phone: every target >= 56 dp, the pause button is 64 dp and full width, End needs a hold, nothing is a drag-only gesture.
- **Plain words, no jargon.** "Part" instead of "segment", "Pedal" instead of "target cadence" where space is tight; RPM stays.
  Effort is words plus number ("Hard 7"): a resistance cue, not a measurement; no calories, no heart rate.
- **Everything is a setting** (sec. 9), on desktop and Android, same names.
- **Safe by default.** Beginner mode on, the health notice before the first ride, ride screen always dark, no flashing faster
  than 3 per second.

## 2. Entry points

- **Desktop**: rail item **Exercise** (icon `bike`, new in `Icon.svelte`), placed after Downloads, before Settings; setting
  `show_entry`. Active state = the usual `.item.active`; while a ride is on, a green dot on the icon (like DJ's busy dot) and
  the label "Exercise · ride". `app.view.name === "exercise"` with internal state `setup | building | preview | ride | done`
  (kept in `lib/exercise.svelte.ts`, so leaving the view during a ride or a build does not stop it; the mini player and the
  rest of the app keep working; the rail dot returns you).
- **Android**: a 5th tab **Train / Entrenar** in the bottom bar (`Icons.Rounded.DirectionsBike`), after Library (order:
  Library, Search, **Train**, Downloads, Settings; 5 items is the Material 3 maximum), gated by `show_entry`; plus a "Start a
  ride" card on Home (`HomeScreen`). While a ride runs, the tab shows a small badge dot and the ride screen reopens on tap.
- **Android TV**: HomeFi's TV launcher activity (leanback launcher intent) shows "HomeFi Train" ("Waiting for a ride") and finds
  rides by mDNS (sec. 7.4).
- **Deep link**: the Android MediaStyle notification of a running ride opens the ride screen.

## 3. Desktop screens

Container `.exercise` (padding 20 x 24 px, gap 14 px). Min window width 1000 px (same rule as DJ: under it show "Make the
window wider to use Exercise"). Reuse `.card`, `.chip`, `Toggle`, segmented control, `.note`, `.dialog`, `Skeleton`, `Icon`,
`Cover`. New components in `components/exercise/`: `RideSetup`, `StylePicker`, `GenreChips`, `BuildProgress`, `PlanPreview`,
`Mountain`, `RideScreen`, `PulseRing`, `EffortBars`, `NextCard`, `RideSummary`, `HealthNotice`, `TvView` (see 7.3).
Canvas/SVG components draw only; state lives in `lib/exercise.svelte.ts`.

### 3.1 Setup (mock-up 1)

Grid `minmax(0,1fr) 380px`, gap 16.

Left column, three cards:

1. **Length**: the number in 56 px bold + "min"; quick chips 20 / 30 / 45 / 60 (aria-pressed); a native-styled slider 10 to 90, step
   5 (`aria-valuetext` "30 minutes"), height 44 px; the ends labelled. Arrow keys step 5, PageUp/PageDown 15. Default
   `ride_minutes`.
2. **Style**: 4 radio cards (`role="radiogroup"`): First ride, Easy, Classic, Intervals, each with a 44 px mini mountain of
   that style at the chosen length (computed from `exercise_preset`), name and one line. Selected = 2 px `--primary` border +
   `--surface-3`. First ride is recommended while `health_notice_seen` is false and no ride has ever been built ("Good for
   your first time" tag).
3. **Music**: label "Genres for today (pick one or more)"; chips (toggle, aria-pressed with a check mark when on): Rock, Metal,
   Pop, Dance, Hip-hop, Latin, Funk, 80s, Indie, Punk, **Any** (default when nothing is picked; picking one clears Any).
   Extra genres: the "+" chip opens a small text input. A switch **Mix the genres** appears when 2 or more are picked
   (default on): "Each genre goes where its tempo fits: fast for sprints, heavy for climbs, calm for warm-up." Then an input
   "Or tell the AI in a sentence (optional)" (placeholder: "Classic rock for the climbs, something heavy for the sprints"), with
   a status line: `AI on: <provider>` (tag, `--ai-tag-*`) or `No AI: genres only` with a link "Set up AI" (opens Settings).
   Without AI the sentence field is disabled with the tooltip "Needs your AI key. Genres still work." (`needs_ai_message`).
   Under the genres, the **taste** questions (`TasteChips` on desktop, `TasteBlock` on Android), compact with smart
   defaults: **How famous** (segmented Big hits / Mix / Discover, default Big hits), **Era** (chips Any, 60s ... 90s,
   2000s, 2010s, 2020s; several allowed, none is Any) and a folded **Artists (optional)**: "Artists you like" (removable
   chips, dashed "+ name" suggestions from the library's most-owned artists in the picked genres, a field that adds on
   Enter) and "Never play" (same, struck through). Every answer is saved at once in `exercise.taste` (a setting on both
   platforms), and the build always uses the saved taste.

Right column:

- **Your ride** card: the live mountain of the chosen style and length (120 px), the summary "30 min · Easy · Rock + Metal" and
  a one-line description (counts of climbs, jumps, interval blocks, cadence and effort caps if beginner mode).
- **Where to watch**: segmented "No TV | TV window | TV app / browser" and "Music plays: Here (headphones) | On the TV"
  (`tv_audio`). Under it, depending on the choice: TV window: "Opens on Display 2, fullscreen." with a monitor picker when
  more than one; TV app / browser: the URL, a copy button and a QR (112 px, white quiet zone) plus "Open this on the TV's
  browser or in HomeFi for Android TV." If a HomeFi TV is found on the network, a chip "Living room TV found" appears.
- **Build my ride** (primary pill, 56 px high, full width) + "Takes about 20 to 30 seconds."

States: first run: the health notice (sec. 3.7) opens before building (not at app start). AI missing: see above. Offline:
note "TIDAL is offline. Only your library will be used." (does not block). Library empty and offline: Build disabled with
"Connect to TIDAL or add music to your library to build a ride." Disabled button: `aria-disabled`, tooltip says why.

### 3.2 Building (mock-up 2)

Same page; the setup collapses into a header ("Building your ride · 30 min · Easy · Rock + Metal"). A card holds:

- the ghost mountain (opacity .55, no marker) so the shape is there from the start;
- three steps from `Progress`: **Searching songs** (`Searching`, "64 candidates"), **Finding them on TIDAL and in your
  library** (`Resolving`, "41 found"), **Checking the tempo** (`Analyzing`, "Song 7 of 12"), then **Matching songs to each
  part** (`Matching`). Each row: tag Done / Now / Next, name, count. A determinate bar with the overall percentage
  (`role="progressbar"`, `aria-valuenow`), live region polite announcing the step;
- **Cancel** (drops the future; returns to setup with everything kept). A line "You can leave this screen; the build keeps
  going." (it does; the rail shows a spinner dot).

Error: inline `.note` danger: "Could not build the ride. <humane reason>." with "Try again" and "Change setup". Timeout (150 s):
"This is taking too long. Try fewer genres or a shorter ride." Partial result (notes from the builder, e.g. a metronome
fallback) goes to the preview as notes, not an error.

### 3.3 Preview (mock-up 3)

Header: title "Your ride is ready", subtitle "30 min · Easy · Rock + Metal · 8 songs"; actions right: **Change setup**,
**New songs** (same request, songs the last build did not play; the preview stays with an inline
progress bar and Cancel, a failure keeps the plan), **Start ride** (primary pill 56 px).

- The **mountain** (160 px, full width) with part boundaries as ticks. Hover a part = tooltip (name, time, RPM, song) and the
  matching row highlights; click = scrolls to the row. Parts are keyboard focusable (roving, arrows).
- The **plan table** (one row per part, 60 px): zone dot, **Part**, **Time**, **Pedal** (RPM), **Effort** (n/10), **Song**
  (cover 44, title, artist), **Beat** tag, "Another song" icon button (36 px hit area plus 44 px padding region).
  Beat tag text `176 BPM · half-time -> 88 ok` in `--ok` when the error is within tolerance, `--warn` when widened (`... (off by
  2)`), neutral grey "metronome" when no song fit and the metronome pulse is used. Tooltip: "The song's beat, how it maps to
  your pedaling, and how close it is to the target." Overflow menu per row: "Wrong tempo (x2 / half)", "Source: library tag /
  TIDAL / analyzed". Source shown as a tiny tag only in the menu, to keep rows calm.
- **Notes** from `RidePlan.notes` below the table, each one with a tag "Note" (plain language).
- The playlist is saved automatically (`PlaylistOrigin::Ride`); a quiet link "Saved in Playlists as 'Easy 30 · Rock + Metal'".

### 3.4 Ride screen (mock-up 4)

The ride view **replaces** the page content (rail stays, so the user can leave without stopping). Min 1000 x 700; designed at
1440 x 900. Three rows `auto 1fr auto`, gap 12:

**Top bar (44 to 56 px)**: zone chip (filled zone colour, `--z-ink` text, 20 px bold, bike icon + part name), position pill
(outline 2 px zone colour, icon seated/standing + "Seated" / "Standing" / "Seated, stand on the beat"), spacer, now-playing
(cover 44 + title/artist, links to nothing; no controls duplicated), **TV** button (state: off / connected / not found; menu),
**Pause** (56 px circle, primary), **Skip to next part** (44 px, tooltip), **End** (44 px, outlined danger; hold 1 s with a fill
ring, or click opens a confirm popover "End the ride? [Keep riding] [End ride]").

**Main row** `300px 1fr 340px`, vertically centred:

- *Left*: label "Effort", 10 bars (22 x 34 px, gap 6; lit bars = zone colour, unlit `--track`), under it the word + number in
  28 px bold ("Hard 7") and one hint line ("Turn the resistance until it feels hard.").
- *Center*: the **RPM ring**, 380 px, with the **beat pulse**: RPM digits 150 px / 900 weight in zone colour (3 digits: 120 px),
  "RPM" 18 px letter-spaced, and the range small under it ("66 to 74"). The ring (12 px border, zone colour) and a glow disc
  inside pulse at `stroke_ms`: scale 1.07 -> 1.0 over 60 % of the period and glow opacity .34 -> 0 (ease-out). Styles by
  `pulse_style`: **ring** (this), **bar** (a 12 px bar under the digits that sweeps and flashes), **flash** (the whole ring
  disc flashes at .22 max). During an interval the ring label above reads **GO!** (work) in `--z-sprint` or **Rest** in
  `--z-recov`, plus "Round 3 / 4".
- *Right*: "Time left in this part" label, the **timer** 96 px / 800 weight tabular (in intervals: time left in the
  work/rest phase, the part's remaining time in 14 px under it), then the **Next card** (dashed 2 px border in the next
  part's colour; "NEXT" label, name in zone colour 22 px bold, "84 RPM · Seated, stand on the beat · effort 6"). From
  `segment_notice_seconds` before the change the card fills solid (tinted 14 %) and shows "in 10 s" counting; the last
  `countdown_seconds` show a big 3-2-1 over the ring (72 px digits, same place as the RPM number, the RPM dims to 30 %).

**Bottom**: card with the **mountain** (250 px high at 900 px window; 170 px under 800 px) + a thin progress bar with
elapsed "12:00" and total "30:00" (the progress bar is the same as the area under the mountain; both are visible so
touch/mouse users see time, not only altitude). Click on the mountain does nothing (a ride cannot be scrubbed, only skipped);
hover shows part tooltips like the preview.

Window < 1200 px: left column moves under the ring (effort becomes a row), mountain stays full width.

### 3.5 Paused (mock-up 5)

The screen dims (`rgb(5 10 18 / .78)`), a dialog (460 px): "Paused", "The music and the timer are stopped.", current part and time
left, **Resume** (primary pill, autofocus), **End ride**. The pulse stops, the marker holds, cues are cancelled, the TV shows
"Paused" in 10 vh over the dimmed screen. Space = pause/resume everywhere on the ride screen.

### 3.6 Finished summary (mock-up 6)

"Ride complete. Nice work!" (34 px), the whole mountain fully lit (marker at the end), four stats (Time, Songs, Climbs, Sprint
blocks; 56 px numbers), the line "This is a guide for effort, not a medical measurement.", buttons **Ride again** (primary,
same request rebuilt), **Same ride, new songs**, **Done**. Confetti: none. A short rising chord cue (`Finish`) plays once. TV shows
the same in "Ride complete" (sec. 7.1).

### 3.7 Health notice (mock-up 10)

A `.dialog` shown once before the first ride (when Build or Start is pressed and `health_notice_seen` is false), focus on the
primary button, Esc = not now (the ride is not built). Text from `exercise_health_notice(lang)`: "HomeFi suggests a pace and an
effort, it does not measure your body or give medical advice. Listen to yourself, and stop if you feel pain, dizziness or
shortness of breath. If in doubt, check with a doctor." Button **I understand** sets `health_notice_seen`. Settings has "Show it
again".

## 4. Android phone screens (portrait 360 x 640 dp first; SR35 720 x 1280)

`ui/exercise/`: `TrainScreen` (setup, build, preview inside one screen with a state), `RideScreen`, `RideSummary`,
`MountainCanvas` (Compose `Canvas`), `PulseRing`. Material 3, `LocalSkin`, motion and haptics settings. Scroll column with
the primary action pinned in a bottom bar. All targets >= 48 dp; ride controls >= 56 dp.

- **Setup**: top "Train". Cards as on desktop but stacked: **Length** (number 44 sp + minus / plus buttons 56 dp + slider; the buttons make
  it usable with a wet thumb), **Style** (chips 48 dp high, wrap), **Music today** (genre chips 48 dp, wrap; "Mix the genres"
  switch row 56 dp; sentence field with a mic-free keyboard; AI tag), **TV** (a row "Show on TV: Off / On this network" -> bottom
  sheet with options TV audio, TV found, QR / URL). Bottom bar: **Build my ride** (64 dp pill).
- **Building**: full screen: ghost mountain, steps list, progress bar, Cancel. Keeps going if the app goes to the background
  (foreground service notification "Building your ride").
- **Preview**: top mountain (90 dp), list of parts (row 56 dp: dot, "Warm-up · 5:00 · 88 RPM", song, BPM tag), tap a row = bottom
  sheet with song detail, fit explanation, "Another song", "Wrong tempo". Bottom bar: **Start ride** (64 dp).
- **Ride** (mock-up 8, middle): always dark when `ride_always_dark` (default); keep-screen-on (`keep_screen_on`, window flag +
  released on exit); system bars hidden (immersive, swipe to show); phone on handlebar assumed: portrait and landscape both work.
  - Row 1: zone chip (nowrap, 15 sp) + position pill + timer 40 sp tabular.
  - Optional banner when the next part is within `segment_notice_seconds`: "Jumps in 10 s · 84 RPM · Seated, stand on the beat"
    (dashed zone border), TalkBack announces it once.
  - RPM ring 250 dp with the pulse (digits 96 sp; 3 digits 80 sp).
  - Effort bars (16 x 24 dp) + "Hard 7" 20 sp bold.
  - Mountain 130 dp; flag "Jumps in 0:10 · 84 RPM" only in the last `segment_notice_seconds` (saves space otherwise).
  - Bottom row: **End** (56 dp, hold 1.2 s with a fill ring, tap = tooltip toast "Hold to end"), **Pause** (flex, 64 dp, 18 sp),
    **Skip** (56 dp, tap opens a bottom sheet "Skip to Jumps? [Skip] [Keep riding]"). Gesture margins 16 dp so the palm doesn't hit
    system gestures.
  - Landscape (>= 600 dp wide): left = ring (260 dp) + effort; right = timer, next, mountain full width below; controls on the
    right edge as a vertical stack (64 dp).
  - Haptics (`haptic_cues`, on): confirm on each part change, light tick on 3-2-1, none on every stroke (battery, annoyance).
  - Media: a notification with Pause and the part name; the lock screen shows the MediaStyle card of the ride playlist.
- **Pause** = full overlay like desktop: Resume (64 dp) and End (hold).
- **Summary** = desktop's, stacked; stats 2 x 2.
- Skins: surfaces from the skin; zone colours do not change per skin (they carry meaning); `retro_amp` and `luna` use their button
  faces on Pause/Skip/End and keep square corners on the ring chip; the ring stays a circle in every skin.

## 5. Components and states

| Component | States |
| --- | --- |
| `StylePicker` | rest, hover (`--surface-3`), selected (2 px primary), focus (2 px outline offset 2), disabled (opacity .5, only while building) |
| `GenreChips` | off, on (check + primary fill), hover, focus, "Any" exclusive; max 6 selected (a 7th shows the note "Pick up to 6") |
| `BuildProgress` | idle, running, error, cancelled, done |
| `Mountain` | ghost (building), preview (all parts tinted .42), live (past .5, current 1, future .38, next dashed), summary (all 1), hover part, focus part, empty (profile missing: flat line + "Profile not available") |
| `PulseRing` | beating, paused (static), countdown (digits), interval work / rest, metronome fallback (same look, tag "Metronome"), motion off (see 8) |
| `EffortBars` | 1..10 lit; the word is always shown |
| `TvButton` | off, starting, connected (green dot), waiting for TV, error (port in use: "Port 47801 is busy. Using 47802.") |
| Ride buttons | rest, hover, active, focus, hold-progress (End), disabled (Skip on the last part) |

Ride error states: song fails to load -> the part continues with the next queue item and a toast "Skipped a song that could not be
played"; Tidal drops mid-ride -> toast, the ride keeps going from local buffers; the TV loses the link -> TV shows
"Reconnecting..." (7.1); the app loses audio output -> pause with the reason.

## 6. Zone colours (tokens, new)

Add to `app.css` `:root` (dark) and the light block; skins never override them (semantic colours), only `studio` stays dark and
`luna`/`aluminum`/`web2000` use the light set through their `data-mode`. All values give >= 4.5:1 as text on `--surface` and
>= 3:1 as filled shapes against `--bg` in their mode; `--z-ink` is the text on a filled zone (>= 4.5:1).

| Token | Part | Dark | Light |
| --- | --- | --- | --- |
| `--z-warm` | Warm-up | `#60a5fa` | `#1d4ed8` |
| `--z-flat` | Seated flat | `#4ade80` | `#15803d` |
| `--z-climb` | Seated climb | `#fbbf24` | `#b45309` |
| `--z-stand` | Standing climb | `#fb923c` | `#c2410c` |
| `--z-jumps` | Jumps | `#c084fc` | `#7e22ce` |
| `--z-sprint` | Sprints | `#fb7185` | `#be123c` |
| `--z-recov` | Recovery | `#2dd4bf` | `#0f766e` |
| `--z-cool` | Cool-down | `#a5b4fc` | `#4338ca` |
| `--z-ink` | text on a zone fill | `#06121a` | `#ffffff` |
| `--track` | unlit bars, bar tracks | `#2a3547` | `#cbd8de` |

The TV page and the ride screen use the **dark** set always (the `ride_always_dark` setting forces `data-mode="dark"` on the
ride container; the mock-up shows it). Android: `ui/theme/Zones.kt` with the same 8 pairs, picked by `isSystemInDarkTheme()` /
the ride's dark override.

Interpretation (shown in a legend on first run and in the preview as a small key): cool colours (blue, teal, indigo) = easy;
green = steady; amber/orange = climbs; purple = jumps; red-pink = all out. Effort is also the number of bars, and each part has a
distinct elevation shape, so the mountain alone is enough to tell them apart.

Other tokens: `--ride-ring: 380px` (desktop), `--ride-rpm: 150px`, `--ride-hit: 44px`, `--ride-hit-touch: 56px`. Corners: only
`--r-*`. Ring and marker stay circles. Fonts: `--font` with tabular numerals; the RPM and the timer use weight 800 to 900.

## 7. TV

### 7.1 TV screen (mock-up 7), 1920 x 1080, readable from 3 m

Always dark (`#05070a`), no cursor, no controls, wake lock; layout in the `vh` units of today's `ride.html` (the mock-up draws it
at 1080 px). The minimum text size is **44 px** (4 vh) at 1080p (about 1 inch cap height on a 50" TV viewed from 3 m); labels use
>= 4.4 vh, numbers 17 to 38 vh. Contrast of secondary text `#9fb0c6` on `#05070a` is 8:1.

| Element | Spec |
| --- | --- |
| Part name | top-left, 9.6 vh / 800, zone colour |
| Position pill | under the name, 5 vh, icon + "Seated" / "Standing", pill `#1b2330`, text `#cfe3ff` (standing: `#ffc58a`) |
| Timer | top-right, 18 vh / 800 tabular; under it 4.4 vh "left in this part" or "Round 3 / 4" in intervals |
| RPM ring | centre, 50 vh diameter, 1.5 vh border, digits 31 vh (3 digits 25 vh), "RPM" 5.5 vh; pulse as the app; **GO!** / **REST** 8 vh above the ring |
| Effort | left, 10 bars (3.2 x 6 vh) + word and number 6 vh |
| Next | right, "NEXT" 4 vh, name 6.7 vh in its zone colour, "84 RPM · Seated" 4.4 vh; `soon` state (notice window) opacity 1 + a coloured left border, otherwise opacity .75 (not .55) |
| Mountain | bottom, left/right 5 vw, **24 vh high** (today 15 vh), part polygons in zone colours (past .5, current 1, future .38, next dashed), ridge line 3 px white at .85, marker 4.2 vh ring with glow, flag "Jumps in 2:00 · 84 RPM" 3.2 vh |
| Progress bar | 1.8 vh at the very bottom, zone colour; elapsed / total small at its ends |
| Countdown | last `countdown_seconds`: dim overlay and 40 vh digits with the next part's name and RPM |
| Song | optional (not by default): 3.6 vh line under the bar |
| States | **Waiting** ("Waiting for a ride", 10 vh, centred, with the HomeFi mark and the URL), **Paused** (dimmed + "Paused"), **Finished** ("Ride complete. Nice work!" with the mountain full and the stats), **Reconnecting** (banner "Reconnecting..." 5 vh after 5 s without data, with a 1.6 vh status dot that is always shown: green / red) |

Interval: while a sprint block runs the ring tint and the digits flip to the work cadence (110) in `--z-sprint` with **GO!**, and to the
rest cadence (85) in `--z-recov` with **REST**; the timer counts the phase, the round counter shows 3 / 4.

### 7.2 How the in-app TV view should match and improve `ride.html`

`ride.html` already has the right bones: vh-scaled layout, zone colours, interval GO/REST, computed pulse, profile drawing, wake lock,
offset parameter, 2 Hz poll with clock offset, waiting and finished states. Keep it as the **single TV renderer** and improve it:

1. **One page, three places (DRY).** Desktop TV window = a second Tauri webview that loads the local TV server URL (`http://127.0.0.1:<port>/`),
   not a new Svelte `TvView`; the Android TV app = a `WebView` on the discovered URL; the browser/QR path is the same page. One layout to
   maintain; the Svelte/Compose code only manages windows and discovery. Add one optional hook so the pulse can follow the *TV's* playback
   in `tv_audio = tv`: if `window.homefiSongPositionMs()` is defined, `position()` uses it instead of the polled estimate (the Android TV app
   injects it from its `GuestClient` player). Fallback is the current estimate.
2. **Mountain bigger and in colour**: 15 vh -> 24 vh, polygon per part in its zone colour instead of one blue "current segment", the next part
   outlined and flagged (today the upcoming climb is only implied), marker larger (3.2 -> 4.2 vh) with the zone colour ring.
3. **Contrast and legibility**: unlit effort bars `#1b2330` (nearly invisible) -> `#2a3547`; "next" idle opacity .55 -> .75; the range text `#7c8aa0`
   -> `#9fb0c6`; the stale-link dot is tiny: add the "Reconnecting..." banner.
4. **3-2-1 countdown** and **Paused** states (today missing); `data-go` / `data-rest` words already exist, add `data-paused`, `data-count`.
5. **3-digit RPM fit** (110 to 120 overflows the ring today at 38 vh): 31 vh, 25 vh for 3 digits, and widen the box to the ring.
6. **Position pill** gets the seated / standing icon (colour is not the only cue), and "Seated, stand on the beat" for alternating jumps.
7. **Motion**: `?motion=off` (and `prefers-reduced-motion`) stops the pulse scale and glow; the ring shows a static dot that steps once per
   stroke at most 2 Hz. A `?theme=` is not needed (always dark).
8. **Pulse style** (`ring | bar | flash`) is sent in `RideTvView.text`/a `style` field so the TV matches the setting (flash capped to .22 opacity and <= 3 per second).
9. **Finished** shows the full mountain and the stats instead of only "Done".
10. **Elapsed/total** text at the ends of the bottom bar; a small "HomeFi" mark top-left of the waiting screen.
Contract impact: `RideTvView` needs `pulse_style`, `paused`, `countdown_s` (or derive from `ride.cues`) and the zone colours stay on the page.
`v` stays 1 (all additive); the page ignores unknown fields.

### 7.3 Desktop TV window and the TV button

- The ride's **TV** button opens a menu: "TV window" (toggle; fullscreen on the chosen display; remembers the display), "Open in browser"
  (copies the URL), "Show QR", "Music plays: Here / On the TV". Esc closes the window; the window has no chrome and `cursor: none`.
- Setup remembers the choice (`tv_enabled`, `tv_window_fullscreen`, `tv_audio`). Starting a ride with TV on opens it with a 1 s fade
  (motion: full) or instantly; ending a ride shows the finished screen 20 s, then "Waiting for a ride".
- In `tv_audio = tv` the app shows "Music is playing on the TV" with a "Play here instead" action in the ride top bar.

### 7.4 Android TV app and the phone's casting UI

- Launcher activity "HomeFi Train" (banner 320 x 180): shows the Waiting screen with the TV's name and a list "Rides on your network"
  once found (D-pad focus, big 6 vh rows, a clear focus ring 0.6 vh in `--primary`); one ride = auto-open after 2 s with "Starting {name}...".
  Back leaves. No text input; the pair step is automatic. If `listen` is present the TV plays the audio and says "Playing the music
  here"; otherwise "Visual only".
- Phone: in the Where to watch sheet: "Living room TV (Chromecast with Google TV)" appears when found (mDNS), choose it and the TV opens
  the ride by itself via the announce (the phone says "Sent to the TV"). Fallbacks listed under "Other ways": Android "Cast screen"
  (zero code), TV browser URL, QR.

## 8. Motion and accessibility

- **Motion** (`data-motion` / `Motion.kt`): full = pulse ring scale (+7 %) and glow, marker moves with a 400 ms linear interpolation between
  2 Hz updates, page transitions 160 ms; subtle = glow only (no scale), 120 ms colour fades, marker steps; off = no animation at all, the pulse is
  a static filled dot on the ring that changes colour on each stroke (at most 2 per second), no marker interpolation, no countdown scale. System
  reduced motion maps to subtle (`lib/motion.ts`). The pulse is a single CSS animation (`animation-duration` = `stroke_ms`, restarted
  on part change) or a Compose `Animatable` driven from one `withFrameNanos` loop: **no state per frame**; the marker and the mountain
  re-draw only on the 10 Hz / 2 Hz view. No blur, no heavy shadows.
- **Safety**: no flash faster than 3 per second or covering more than a quarter of the screen at more than .25 opacity (the `flash` style is
  capped); sprint cadence stays <= 2 strokes per second so the pulse can't strobe.
- **Colour independence**: zone colour is always accompanied by the part name, the position icon, the effort number and the mountain shape.
  Checked in grayscale and with deuteranopia/protanopia filters.
- **Contrast**: all text >= 4.5:1 (zone colours on `--surface` dark and light), large numbers >= 3:1 against their background in every skin;
  tested on the SR35 in sunlight-ish brightness in dark mode.
- **Focus**: visible 2 px outline everywhere; focus order on the ride screen: Pause, Skip, End, TV, then (non-interactive content is read via
  a live region). Setup order: Length, Style, Genres, Mix, Sentence, Where to watch, Build. Esc closes menus and the paused overlay; Space
  = pause/resume; `N` = next part (hint in the tooltip); keys are optional.
- **Screen readers**: the mountain is `role="img"` with a text alternative ("Course: warm-up 5 minutes, ... now in seated climb at 12 minutes of 30");
  the ring has `aria-label="Target 70 RPM"`; a polite live region announces part changes ("Next: Jumps, 84 RPM, seated, stand on the beat") and the
  3-2-1 (the 3-2-1 is not announced by the screen reader: audible cues already exist, so the countdown text is `aria-hidden`).
  Timer is `role="timer"` with `aria-live="off"` (not read every second).
- **Touch targets** >= 48 dp (setup) and >= 56 dp (ride); no drag-only controls; hold-to-end has a visible progress and also a confirm dialog when
  TalkBack is on (holding is hard with a screen reader).
- Reading size follows the system font scale up to 1.3 on the phone ride screen (ring and digits fixed in dp so they don't overflow; labels scale).
- Cues are audible; with `audible_cues` off, the pulse and the banner are the only signals. `metronome_click` is an alternative for those who
  cannot see the screen.

## 9. Settings page "Exercise" (desktop) and "Train" section (Android Settings)

Desktop: new page `exercise` in Settings, cards in this order; Android: a section "Train" with the same rows (`ui/settings/ExerciseSettings.kt`).
Layout as the mock-up 9: rows `min-height 52`, label + small helper, control right.

| Card | Setting (key) | Control | Default |
| --- | --- | --- | --- |
| Ride | Show Exercise in the sidebar / tabs (`show_entry`) | switch | on |
| Ride | Beginner mode (`beginner_mode`) | switch | on |
| Ride | Default style (`default_preset`) | segmented | First ride |
| Ride | Default length (`ride_minutes`) | stepper 10 to 90, step 5 | 30 |
| Music | Pick songs from (`sources`) | segmented Library first / TIDAL first / Library only | Library first |
| Music | Tempo tolerance (`cadence_tolerance_rpm`) | slider 2 to 12 RPM | 4 |
| Music | Allow half-time / double-time (`allow_half_time`, `allow_double_time`) | switches | on / off |
| Music | Allow explicit songs (`allow_explicit`) | switch | on |
| Music | Tempo check budget (`analysis_budget`) | stepper 0 to 50 | 12 |
| Music | Tempo nudge (`tempo_nudge_pct`, phase 2) | slider 0 to 8 % | 0, hidden until phase 2 |
| Cues and beat | Sounds for changes (`audible_cues`) | switch | on |
| Cues and beat | Cue volume (`cue_volume`) | slider | 80 % |
| Cues and beat | Countdown before a change (`countdown_seconds`) | stepper 0 to 10 s | 3 |
| Cues and beat | Next-part notice (`segment_notice_seconds`) | stepper 5 to 30 s | 10 |
| Cues and beat | Click on the beat (`metronome_click`) | switch | off |
| Cues and beat | Beat pulse (`pulse_style`) | segmented Ring / Bar / Flash | Ring |
| Cues and beat | Pulse offset (`pulse_offset_ms`) | slider -500 to 500 ms + "Match my lyrics offset" | 0 |
| Screen | Always dark during a ride (**new**, `ride_always_dark`) | switch | on |
| Screen | Keep the screen on (`keep_screen_on`) | switch | on |
| Screen | Buzz on part changes (**new**, Android only, `haptic_cues`) | switch | on |
| TV | TV (`tv_enabled`) | switch | off |
| TV | Music plays (`tv_audio`) | segmented On the TV / Here | Here |
| TV | TV window on (**new**, desktop, `tv_window_display`) | select of displays | the second display |
| TV | TV window fullscreen (`tv_window_fullscreen`, desktop) | switch | on |
| TV | TV port (`tv_port`, advanced) | number 1024 to 65535 | 47801 |
| Health | Health notice (`health_notice_seen`) | button "Show it again" | |

New keys to add to `ExerciseSettings` (core, lenient, clamped): `ride_always_dark`, `haptic_cues`, `tv_window_display`. Every row has the same wording on both platforms; Android adds nothing
the desktop does not have except `haptic_cues`. "Reset to defaults" button at the end of the page.

## 10. Copy

Desktop English; Android strings keys `ex_*` in `strings_exercise.xml` (EN and ES).

| key | EN | ES |
| --- | --- | --- |
| ex_title | Exercise (desktop) / Train (Android) | Entrenar |
| ex_tab | Train | Entrenar |
| ex_setup_sub | Choose how long, what kind of ride and what you want to hear. HomeFi builds the ride and the music. | Elige cuánto dura, qué tipo de rutina y qué quieres escuchar. HomeFi arma la rutina y la música. |
| ex_length | Length | Duración |
| ex_minutes | min | min |
| ex_style | Style | Estilo |
| ex_style_first | First ride | Primera vez |
| ex_style_first_d | 20 min, gentle | 20 min, suave |
| ex_style_easy | Easy | Fácil |
| ex_style_easy_d | Climbs and jumps | Subidas y saltos |
| ex_style_classic | Classic | Clásica |
| ex_style_classic_d | Two climbs, intervals | Dos subidas, intervalos |
| ex_style_intervals | Intervals | Intervalos |
| ex_style_intervals_d | Sprints and rests | Sprints y descansos |
| ex_music | Music | Música |
| ex_genres | Genres for today (pick one or more) | Géneros de hoy (elige uno o varios) |
| ex_genre_any | Any | Cualquiera |
| ex_mix | Mix the genres | Mezclar los géneros |
| ex_mix_d | Each genre goes where its tempo fits: fast for sprints, heavy for climbs, calm for warm-up. | Cada género va donde mejor encaja su ritmo: rápido para sprints, pesado para subidas, calmado para calentar. |
| ex_sentence | Or tell the AI in a sentence (optional) | O cuéntaselo a la IA en una frase (opcional) |
| ex_sentence_ph | Classic rock for the climbs, something heavy for the sprints | Rock clásico para las subidas, algo pesado para los sprints |
| ex_ai_on | AI on: %1$s | IA activa: %1$s |
| ex_ai_off | No AI: genres only | Sin IA: solo géneros |
| ex_ai_needs | Needs your AI key. Genres still work. | Necesita tu clave de IA. Los géneros funcionan igual. |
| ex_watch | Where to watch | Dónde verlo |
| ex_tv_none | No TV | Sin TV |
| ex_tv_window | TV window | Ventana de TV |
| ex_tv_app | TV app / browser | App de TV / navegador |
| ex_tv_audio | Music plays | La música suena |
| ex_tv_audio_here | Here (headphones) | Aquí (audífonos) |
| ex_tv_audio_tv | On the TV | En la TV |
| ex_tv_found | Living room TV found | TV de la sala encontrada |
| ex_tv_sent | Sent to the TV | Enviado a la TV |
| ex_build | Build my ride | Armar mi rutina |
| ex_build_time | Takes about 20 to 30 seconds. | Tarda unos 20 a 30 segundos. |
| ex_building | Building your ride | Armando tu rutina |
| ex_step_search | Searching songs | Buscando canciones |
| ex_step_resolve | Finding them on TIDAL and in your library | Buscándolas en TIDAL y en tu biblioteca |
| ex_step_tempo | Checking the tempo | Revisando el ritmo |
| ex_step_match | Matching songs to each part | Asignando canciones a cada parte |
| ex_cancel | Cancel | Cancelar |
| ex_keeps_going | You can leave this screen; the build keeps going. | Puedes salir de esta pantalla; sigue armándose. |
| ex_err_build | Could not build the ride. %1$s | No se pudo armar la rutina. %1$s |
| ex_err_slow | This is taking too long. Try fewer genres or a shorter ride. | Está tardando demasiado. Prueba con menos géneros o una rutina más corta. |
| ex_err_offline | TIDAL is offline. Only your library will be used. | TIDAL no está disponible. Solo se usará tu biblioteca. |
| ex_err_nomusic | Connect to TIDAL or add music to your library to build a ride. | Conecta TIDAL o agrega música a tu biblioteca para armar una rutina. |
| ex_ready | Your ride is ready | Tu rutina está lista |
| ex_start | Start ride | Empezar |
| ex_change | Change setup | Cambiar |
| ex_another_mix | New songs | Canciones nuevas |
| ex_col_part / time / pedal / effort / song / beat | Part / Time / Pedal / Effort / Song / Beat | Parte / Tiempo / Pedaleo / Esfuerzo / Canción / Ritmo |
| ex_another | Another song | Otra canción |
| ex_wrong_tempo | Wrong tempo (double / half) | Tempo equivocado (doble / mitad) |
| ex_fit_ok | %1$d BPM · %2$s -> %3$d RPM | %1$d BPM · %2$s -> %3$d RPM |
| ex_ratio_one / half / double | 1:1 / half-time / double-time | 1:1 / a medio tiempo / a doble tiempo |
| ex_fit_off | (off by %1$d) | (a %1$d de diferencia) |
| ex_metronome | Metronome | Metrónomo |
| ex_note | Note | Nota |
| ex_saved_as | Saved in Playlists as "%1$s" | Guardada en Playlists como "%1$s" |
| ex_zone_warm | Warm-up | Calentamiento |
| ex_zone_flat | Seated flat | Plano sentado |
| ex_zone_climb | Seated climb | Subida sentado |
| ex_zone_stand | Standing climb | Subida de pie |
| ex_zone_jumps | Jumps | Saltos |
| ex_zone_sprint | Sprints | Sprints |
| ex_zone_recov | Recovery | Recuperación |
| ex_zone_cool | Cool-down | Enfriamiento |
| ex_seated | Seated | Sentado |
| ex_standing | Standing | De pie |
| ex_jump_beat | Seated, stand on the beat | Sentado, párate con el ritmo |
| ex_rpm | RPM | RPM |
| ex_effort | Effort | Esfuerzo |
| ex_eff_1..5 (effort 1-2 / 3-4 / 5-6 / 7-8 / 9-10) | Very easy / Comfortable / Steady / Hard / All out | Muy fácil / Cómodo / Constante / Duro / A tope |
| ex_effort_hint | Turn the resistance until it feels %1$s. | Ajusta la resistencia hasta que se sienta %1$s. |
| ex_time_left | Time left in this part | Tiempo en esta parte |
| ex_next | Next | Sigue |
| ex_next_in | %1$s in %2$s · %3$d RPM | %1$s en %2$s · %3$d RPM |
| ex_go / ex_rest | GO! / REST | ¡VAMOS! / DESCANSO |
| ex_round | Round %1$d / %2$d | Ronda %1$d / %2$d |
| ex_pause | Pause | Pausa |
| ex_resume | Resume | Continuar |
| ex_skip | Skip to next part | Saltar a la siguiente parte |
| ex_skip_q | Skip to %1$s? | ¿Saltar a %1$s? |
| ex_keep | Keep riding | Seguir pedaleando |
| ex_end | End ride | Terminar |
| ex_end_hold | Hold to end | Mantén para terminar |
| ex_end_q | End the ride? | ¿Terminar la rutina? |
| ex_paused | Paused | En pausa |
| ex_paused_d | The music and the timer are stopped. | La música y el cronómetro están detenidos. |
| ex_done | Ride complete. Nice work! | Rutina completada. ¡Buen trabajo! |
| ex_stat_time / songs / climbs / sprints | Time / Songs / Climbs / Sprint blocks | Tiempo / Canciones / Subidas / Bloques de sprint |
| ex_again | Ride again | Otra vez |
| ex_same_new | Same ride, new songs | Misma rutina, otras canciones |
| ex_finish | Done | Listo |
| ex_guide | This is a guide for effort, not a medical measurement. | Es una guía de esfuerzo, no una medición médica. |
| ex_health_title | Before your first ride | Antes de tu primera rutina |
| ex_health_body | HomeFi suggests a pace and an effort, it does not measure your body or give medical advice. Listen to yourself, and stop if you feel pain, dizziness or shortness of breath. If in doubt, check with a doctor. | HomeFi sugiere un ritmo y un esfuerzo, no mide tu cuerpo ni da consejos médicos. Escúchate y para si sientes dolor, mareo o falta de aire. Si tienes dudas, consulta a un médico. |
| ex_understand | I understand | Entendido |
| ex_tv_waiting | Waiting for a ride | Esperando una rutina |
| ex_tv_reconnect | Reconnecting... | Reconectando... |
| ex_tv_visual | Visual only | Solo imagen |
| ex_tv_playing | Playing the music here | Reproduciendo la música aquí |
| ex_tv_port_busy | Port %1$d is busy. Using %2$d. | El puerto %1$d está ocupado. Usando %2$d. |
| ex_wide | Make the window wider to use Exercise. | Haz la ventana más ancha para usar Entrenar. |
| ex_pace_hint | Pedal when the ring closes. | Pedalea cuando el aro se cierra. |

Desktop tooltips: Pause "Pause the ride [Space]"; Skip "Skip to the next part [N]"; End "End the ride (hold 1 second)"; TV "Show on a TV";
Beat tag "The song's beat, how it maps to your pedaling, and how close it is to the target."; Another song "Pick a different song for this part".

## 11. Per-skin notes

- Surfaces, rail, cards, buttons, dialog: existing tokens (`--surface*`, `--border`, `--primary`, `--glass*`); corners `--r-*`.
- **classic**: as the mock-up. **immersive**: cards `--glass`, ride screen keeps a solid dark background (readability over album blur);
  **vinyl**: serif headings on titles, warm paper only on setup and settings, ride stays dark; **studio**: always dark, mono numerals for RPM and timer,
  `--r-*` capped at 4 px (chips become rectangles; ring stays a circle); **retro_amp**: square buttons with bevels, the effort bars become LED
  segments, steps easing for the pulse (hard on/off at the beat); **luna / aluminum / web2000** (light): setup, preview, summary and settings in the skin's light
  glossy look; the ride screen is dark if `ride_always_dark` (default) else light with the light zone set (contrast verified above).
- Zone colours never change with the skin.

## 12. Acceptance screenshots

Vite mock `?exercise` in headless Chrome (fixtures: a built plan, a ride at a seated climb with the next part in 10 s, a sprint block, a finished
ride), then Android emulator 720 x 1280 (and 1080 x 2400 for sanity):

1. Desktop 1440 x 900 dark classic: setup (default), setup with 3 genres + sentence, AI missing variant, building (step 3), preview (with a warn row and a
   metronome row), ride (seated climb), ride (next in 10 s with solid next card), ride (3-2-1), sprint GO!, rest, paused, finished, health notice.
2. Same ride and setup in light classic (ride stays dark with `ride_always_dark`, plus one with it off).
3. 1000 x 700 minimum: setup, ride (no overlaps, mountain 170 px).
4. One ride screenshot per skin: immersive, vinyl, studio, retro_amp, luna, aluminum, web2000.
5. `data-motion="off"` ride: static pulse dot; and "subtle".
6. Focus rings: Build, Pause, a genre chip, a style card, a plan row.
7. TV 1920 x 1080 headless: seated climb, sprint GO!, 3-2-1, paused, finished, waiting, reconnecting; check text >= 44 px; a 3 m readability photo-test on the real TV.
8. Android 720 x 1280: setup (scrolled and bottom bar), building, preview, ride (portrait), ride (landscape), pause, skip sheet, summary; EN and ES; light (setup) and dark (ride);
   TalkBack on for the ride; font scale 1.3.
9. Android TV: waiting, rides found list with D-pad focus, ride, reconnecting (on a Chromecast with Google TV or the TV emulator).
10. Settings page on both platforms, EN and ES.
11. Contrast checks: all zone colours as text on `--surface` and `--bg` in both modes (>= 4.5:1), numbers over the glow disc.

## 13. Decisions taken (and what to confirm with José)

1. Sidebar/tab name: Exercise (desktop), **Entrenar / Train** (Android tab). The plan said "Entrenar" in the rail and tabs; the desktop UI stays English
   ("Exercise"). Confirm.
2. The TV is one HTML page reused by browser, desktop window and Android TV `WebView` (7.2); if the architect prefers a native Compose TV client,
   the layout above is the spec for it.
3. The ride cannot be scrubbed, only paused or skipped; End needs a hold (touch) or a confirm (mouse).
4. Zone colours are fixed semantic colours across skins.
5. New settings beyond the core list: `ride_always_dark`, `haptic_cues`, `tv_window_display`; ask `rust-pro` to add them to `ExerciseSettings`.
6. The `RideTvView` additions (`pulse_style`, `paused`, `countdown_s`) are additive; `v` stays 1.
7. Out of this spec on purpose: voice cues, tempo nudge UI, BLE cadence, ride history (phases 2 and 3); the setup leaves a place for "Sensor" in the
   Where to watch card later.
