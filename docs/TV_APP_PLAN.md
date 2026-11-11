# HomeFi for Android TV / Google TV: plan

Status: proposed (architect, 2026-11-09), waiting for José's answers below. Built in phases; each
phase ships in the same APK without touching the phone app. Target: Chromecast with Google TV and
any Android TV 8+ (minSdk 26 already), 1080p and 4K, D-pad remote only.

## Decisions

- **One APK, one app graph, a second UI.** Everything in `HomefiApplication` (engine, account,
  settings, downloads, library, player + `PlaybackService`, playlists, sound/EQ, AI, share, ride,
  lyrics, updater, what's new) and every ViewModel is reused as is. The TV gets its own package
  `tv/` of composables that only draw state and send intents; no product rule moves. Rule for
  reuse: a `ui/*` composable is used on TV only if it has no touch-only interaction (swipe, drag,
  pinch, long press, bottom sheets) and reads at 10 ft; otherwise `tv/*` draws the same state.
- **Compose for TV (`androidx.tv:tv-material` 1.1.0, stable) over plain Material 3 + hand-made
  focus.** It gives D-pad focus visuals (scale, border, glow), `NavigationDrawer`, `Card`,
  `ListItem`, `Button` and a TV type scale for free; writing our own focus system on Material 3
  would be a second home for the same rule. `tv-foundation` is **not** added (its lazy layouts were
  removed; standard `LazyColumn/LazyRow` are the TV way now). Material 3 stays for the shared
  draw-only pieces (`Mascot`, `EqCurve`, `ProgressBars`, `SignalChain`, `RoomViews`, `QrCode`,
  karaoke lines) through a `TvTheme` that maps `Skin` into both `MaterialTheme`s, so `LocalSkin`,
  motion and skins keep working.
- **TV or phone UI is one decision in one place:** `DeviceForm { PHONE, TV }` computed once in
  `HomefiApplication` from `UiModeManager.currentModeType == UI_MODE_TYPE_TELEVISION ||
  hasSystemFeature(FEATURE_LEANBACK)`, overridable by the setting `uiForm` (auto / phone / tv).
  `TvActivity` (already the `LEANBACK_LAUNCHER`) becomes the TV app's entry; `MainActivity` on a
  TV form forwards its intent (Tidal link, `EXTRA_OPEN_PLAYER`, opened file) to `TvActivity` and
  finishes. `PlaybackService`, `DownloadService` and `ShareService` build their notification
  intents for the activity of the current form. The ride client (today's `TvScreen`) becomes the
  "Train" section's "Watch a ride from your phone".
- **DJ mode on TV only with a mouse** (`tvDjEntry` auto / on / off, default auto: shown when an
  `InputDevice` with `SOURCE_MOUSE` is connected). With it the phone's `DjScreen` runs as is
  (Compose pointer input works with a mouse); without it the section says "DJ mode needs a mouse
  (USB or Bluetooth)". No D-pad DJ.
- **Login = QR + code only.** `LoginViewModel` already does TIDAL's device-code flow
  (`engine.startLogin()` → `userCode`, `url`, poll). The TV shows the code big, the QR
  (`QrCode` reused) and "open link.tidal.com on your phone"; the in-app `TidalWebLogin` is not
  offered on TV. The personal-use notice comes first, as on the phone.
- **Downloads on a TV: internal first, USB when there is no room.** The free-space rule lives
  once in core (`homefi_core::downloads::space`, `libc::statvfs`), used by desktop and Android.
  USB writing: with "All files access" anywhere on the drive; without it, the app's own folder on
  that volume (`getExternalFilesDirs()` on the USB, no permission needed, deleted with the app).
  USB removed = downloads pause and songs on it are marked unavailable, never deleted from the cache.
- **Voice where typing hurts:** search and the AI chats (sound assistant, playlist chat, party,
  concert) get a mic button (`RecognizerIntent.ACTION_RECOGNIZE_SPEECH`, the Google TV mic; hidden
  when no activity answers) plus quick replies (the chips that already exist). No Assistant
  integration beyond the media session in phase 1.
- **Party on the TV is the hub with the big speakers:** `ShareManager.startHub(party, LAN)`; the
  QR and code stay on the Party screen and in ambient mode; approvals are TV dialogs. Wi-Fi
  Direct and Bluetooth are hidden on TV (no radios on most boxes); LAN only.
- **Ambient mode while music plays** (`tvAmbientSeconds`, 0 = off): dimmed blurred cover, drifting
  title (burn-in), optional karaoke line and HomeFito; any key wakes. Keep-screen-on while playing
  (`tvKeepAwakeWhilePlaying`), released after `tvAwakeAfterPauseMinutes` so the system screensaver
  can take over.
- **Everything is a setting**, in `AppSettings.tv` (Android, both forms show the section; on the
  phone it says "when HomeFi runs on a TV"). Desktop gets nothing new: nothing new runs there
  (ambient mode on desktop is an open question, not assumed).
- **Updates: the same in-app updater**, one universal APK for phone and TV. Play Store later with a
  `play` flavor (see "Easy install on a TV").

### José's addition (2026-11-09): app AND receiver, no typing

- The TV app is both the full HomeFi app and the receiver of a phone's (or the desktop's) ride, with
  nothing to type. Pairing is by a **local QR on the TV**: the TV shows a QR with
  `homefi://tv?host=<tv-ip>&port=<port>&code=<one-time code>`; the phone scans it (HomeFi's own scanner
  in Train/Party, or the system camera, which opens the app by that deep link) and tells the TV where
  its ride is (`POST /pair` on a small LAN endpoint the TV app runs while waiting), so the TV opens the
  ride by itself. The paired phone is remembered: next time the TV connects to it as soon as it starts
  a ride (mDNS first, then the remembered address). Typing the address stays only as a last resort.
- The same QR pairing serves Party (the phone joins the TV hub) and, later, sending a song or a
  playlist from the phone to the TV ("play this on the TV").
- Phase 1 includes it (shell + login + player + receiver pairing).

## Architecture

```
tv/TvActivity.kt            entry (LEANBACK_LAUNCHER), DeviceForm, keep-awake, key routing
tv/TvRoot.kt                drawer + sections + now-playing bar + overlays (player, ambient, dialogs)
tv/TvTheme.kt               Skin -> tv-material + material3 themes; TvDimens (safe margins, sizes)
tv/TvFocus.kt               focusRestorer, first-focus, long-press (DirectionCenter) -> context menu
tv/home/TvHome.kt           recents / continue (HomeScreen state)
tv/library/TvLibrary.kt     songs / albums / artists / folders / .m3u (LibraryRepository)
tv/search/TvSearch.kt       SearchViewModel + mic
tv/tidal/TvTidal.kt         collection, artist / album / playlist pages (TidalViewModel)
tv/playlists/TvPlaylists.kt HomePlaylists, chat / party / concert (their ViewModels) with voice
tv/downloads/TvDownloads.kt DownloadRepository state, target picker, USB notices
tv/sound/TvSound.kt         SoundLayers / Equalizer / EqViewModel, D-pad band editing, assistant
tv/share/TvParty.kt         hub (QR, code, guests, approvals), join as guest (GuestPlayer)
tv/exercise/TvTrain.kt      watch a ride (today's TvScreen) · start a ride here (RideSession)
tv/player/TvNowPlaying.kt   cover, progress, controls, panel (lyrics/karaoke · mascot · visualizer · queue)
tv/player/TvAmbient.kt      screensaver while playing
tv/settings/TvSettings.kt   two-pane settings, every AppSettings field + AppSettings.tv
tv/install/                 nothing: the "Put HomeFi on your TV" helper lives in the phone's Settings
```

- **Shared, untouched:** `data/*`, `player/*`, `share/*`, `library/*`, `lyrics/*`, all ViewModels
  (`LoginViewModel`, `SearchViewModel`, `TidalViewModel`, `EqViewModel`, `AssistantViewModel`,
  `PartyConversation`, `ConcertViewModel`), `ui/theme/*`, `ui/player/{Mascot,Dance,Mood,Hair,Mosh,
  BeatFollower users}`, `ui/player/ProgressBars`, `SignalChain`, `RoomViews` (visualizer, VU),
  `ui/eq/EqCurve`, `ui/login/{QrCode,TermsNotice}`, `ui/ReleaseNotesDialog`, `ui/UpdateCard` state.
- **Phase 0 splits (SRP, no behaviour change on the phone):** `LyricsView.kt` keeps the touch
  screen but exports `LyricsLines(frame, onLine)` and the karaoke line composables (`KaraokeLine`,
  the `karaokeFrame` math at lines 559–600) so TV draws them with focus instead of taps;
  `NowPlayingScreen.kt` exports the panel pieces it already composes (backdrop, cover, queue list)
  as parameters-only composables; `HomeScreen` exports its row models. Nothing else moves.
- **Platform glue (new, Android-only):** `data/DeviceForm.kt` (pure function of two booleans,
  tested), `data/Storage.kt` gains `volumes: StateFlow<List<Volume>>` fed by
  `ACTION_MEDIA_MOUNTED/EJECT/UNMOUNTED` and `freeBytes(dir)`; `DownloadRepository` asks core's
  space rule before each job and pauses on "volume gone"; `LibraryRepository.libraryRoots` adds the
  removable volumes the user picked (`tv.libraryVolumes`) and keeps cache entries of a missing root
  flagged unavailable (core `library.rs`: a root that is not mounted is skipped, not purged).
- **Core (Rust, shared with desktop):** `downloads::space::check(dir, min_free_mb) ->
  Result<FreeSpace, SpaceError>` and `DownloadSettings.min_free_mb` (FFI record field, lenient
  default 500); `library.rs` "missing root keeps its cache". Nothing TV-specific in core.
- **Concurrency and memory:** TV composables read `StateFlow`s with `collectAsStateWithLifecycle`;
  the ambient and visualizer loops run only while the activity is `STARTED` (`repeatOnLifecycle`);
  `AudioTap` stays the single source for mascot and visualizer (no second tap); covers decoded at
  the card size (Coil `size`); Tidal stream cache default on TV 1 GB (`tv.streamCacheMb`, the
  Chromecast has ~4 GB free); `motion = AUTO` maps to `SUBTLE` on `isLowRamDevice`. The ride on a TV
  that runs it itself: `RideTvHost` serves on `127.0.0.1` and the WebView shows the core's own page
  (the one renderer), the position coming from the local player, exactly like `listening` today.
- **Keys:** media keys reach `PlaybackService` through the session (already wired, no activity
  code). In `TvNowPlaying`: center = play/pause when controls are hidden, left/right = previous /
  next (hold = seek by `tv.dpadSeekSeconds`), up = panel switch, down = controls, BACK = close.
  `KEYCODE_MEDIA_FAST_FORWARD/REWIND` seek by the same setting. Long press on center anywhere in a
  list = the row's context menu (play, play next, queue, download, heart, add to playlist, Sound
  for this…), the same actions the phone's row menus offer.
- **10-foot rules** (constants in `TvDimens`): safe margins 48 dp sides / 27 dp top-bottom
  (`tv.safeMargins` on/off for TVs without overscan), body ≥ 16 sp in the TV type scale, cards
  ≥ 160 dp, one focused thing per screen, no hover-only affordances, `tv.textScale` 0.9..1.3.

## Feature map (TV notes)

| Feature | On the TV | Notes |
| --- | --- | --- |
| Login | code + QR + countdown | device-code flow unchanged; terms notice first |
| Home | recents, continue, what's new | `HomeScreen` row models reused |
| Library | songs / albums / artists / folders / .m3u, USB volumes | own `TvFolderPicker` (SAF's document picker is often missing on Google TV, as on the A&K); "All files access" asked from Settings when the user picks a USB root |
| Search | TV keyboard + mic, links pasted from the phone not possible | `SearchViewModel`; results grouped as today |
| TIDAL | collection, artist / album / playlist pages, heart, stream | `TidalViewModel`, streaming cache 1 GB default |
| Downloads | target (internal / USB), space check, USB removed notice, per-row cancel / retry | foreground `DownloadService` unchanged |
| Playlists | HomeFi playlists, chat / party / concert with mic + quick replies, TIDAL save settings | sessions in core; the Gemini widget stays off on TV (WebView + D-pad) |
| Sound / EQ | sounds per target, output profile, D-pad curve editing (band left/right, ±`tv.eqStepDb` up/down), assistant with mic | HDMI output is `OutputKind.HDMI` already (`TYPE_HDMI/ARC/EARC`); USB DACs work as on the phone |
| Party / share | hub with QR + code (also in ambient), approvals, guests, party playback; join a room by code | LAN only; `ShareSettings` hidden radios |
| Exercise | watch a phone's ride (today) · build and run a ride on the TV | same `RideSession`; setup screen with chips + mic |
| DJ | only with a mouse | see Decisions |
| Player | full screen: cover · progress · panel (lyrics / karaoke / mascot / visualizer / queue) | the 8 phone layouts are not ported: one TV layout, skins' colours kept (`tv.playerPanel` default, `tv.playerLayoutColours` on/off) |
| Ambient | screensaver while playing | see Decisions |
| Settings | two-pane, every field | plus `AppSettings.tv` |
| Updates | in-app updater, same APK | see "Easy install on a TV" |
| Home-screen channels | "Continue listening" row | phase 6, nice-to-have; Google TV hides most third-party channels |

## Settings (`AppSettings.tv`, Android, both forms)

`uiForm` (auto / phone / tv, auto), `ambientSeconds` (0..=600 step 30, 60), `ambientStyle`
(cover / karaoke / mascot / clock, cover), `keepAwakeWhilePlaying` (true), `awakeAfterPauseMinutes`
(1..=30, 5), `controlsHideSeconds` (2..=15, 5), `dpadSeekSeconds` (5..=60, 10), `playerPanel`
(lyrics / mascot / visualizer / queue, lyrics), `playerLayoutColours` (true), `safeMargins` (true),
`textScale` (0.9..=1.3, 1.0), `djEntry` (auto / on / off, auto), `voiceInput` (true),
`downloadTarget` (internal / usb / ask, internal), `minFreeMb` (core, 100..=5000, 500),
`libraryVolumes` (list of volume roots), `streamCacheMb` (256..=4096, 1024), `partyAutoApprove`
(false), `channels` (phase 6). Defaults and ranges in one Kotlin object `TvDefaults` next to the
data class; `clamped()` and lenient JSON like `ShareSettings`. Existing fields apply as they are
(skins, mascot, lyrics mode / size / offset, crossfade, quality, template, AI keys…).

## Easy install on a TV

Sideloading is the pain; the APK and the updater already exist. Free options, decided:

1. **Stable URL, Downloader app, short code (phase 1, José and everyone).** The release
   workflow already publishes `HomeFi-<v>-android.apk` and copies it to the stable name
   `HomeFi-android.apk` ("Stable names" step in `.github/workflows/release.yml`), so
   `https://github.com/joseespana/homefi/releases/latest/download/HomeFi-android.apk` always
   serves the newest release (the SR35 build is never published; `AppUpdater` refuses any id
   other than `com.perseusoft.homefi`). On Google TV / Fire TV the free "Downloader" app
   (AFTVnews, on Play for TV) opens a URL and the system installer; it also takes a 5–6 digit
   **Downloader code** that anyone can register for free at aftvnews.com for a URL. We register
   one for the stable URL and print it in the README, the release notes and the phone helper. If
   José owns a domain, a free Cloudflare redirect (`<domain>/tv` → the stable URL) is the second
   short form; without a domain, the code is enough. Flow for a user: install Downloader → type
   the code → Install → allow "unknown apps" for Downloader once → open HomeFi → scan the QR to
   sign in. Nothing to build; one README section and the code.
2. **"Put HomeFi on your TV" helper in the phone app (phase 1, small):** Settings → About →
   "Put HomeFi on your TV": the steps above, the Downloader code and the stable URL big enough to
   read across the room, a QR of the release page for the phone. **Serving the APK from the phone
   on the LAN is rejected:** the TV user still types an address in Downloader (an IP:port is no
   easier than a code), the phone's `sourceDir` is only the universal APK on GitHub builds (Play
   split APKs and the SR35 build would be wrong), and it adds a server and an open port for no
   step saved. Pushing by network ADB is worse (pairing dialogs, a client to write). Revisit only
   if a TV cannot reach GitHub.
3. **After the first install the TV updates itself (phase 1, verify on the emulator).**
   `AppUpdater` downloads the APK, checks SHA-256 against `SHA256SUMS`, checks package name and
   version, then `ACTION_VIEW` with the `FileProvider` URI; `REQUEST_INSTALL_PACKAGES` and
   `ACTION_MANAGE_UNKNOWN_APP_SOURCES` work on Android TV 8+ and Google TV (Downloader relies on
   the same path; the setting is Apps → Security & restrictions → Unknown sources). The TV flow
   needs: a D-pad-navigable update card (the phone's `UpdateCard` state), the "allow HomeFi" step
   explained once, and the note that the music stops while Android installs. Same signing key
   (`ANDROID_KEYSTORE_*`) or Android refuses the update: unchanged.
4. **Play Store (when José pays, becomes the main path).** The TV listing needs: the
   `LEANBACK_LAUNCHER` activity (have), `android:banner` 320×180 (have `tv_banner.xml`, Play wants a
   raster at that size too), `touchscreen required=false` (have), no permission implying a required
   feature (Bluetooth features already `required=false`), ≥ 2 TV screenshots 1920×1080, the TV
   quality review (D-pad everywhere, BACK never dead-ends, no touch-only flow, audio focus). Policy
   points: `REQUEST_INSTALL_PACKAGES` (self-update) and `MANAGE_EXTERNAL_STORAGE` are not allowed
   for a music player on Play → product flavor `play` removes both, `AppUpdater.enabled` is false
   there and USB writes use the app folder on the drive. Play requires an AAB and Play App Signing:
   enrol with the **existing** release key as the app signing key so the GitHub APK and the Play
   build keep one signature and update each other. New personal developer accounts must run a
   closed test (12 testers, 14 days) before production: start it the day the account exists.

Recommendation: 1 + 2 + 3 now (two days of work, zero cost), 4 in December as the default path
for everyone else; the GitHub URL stays as the no-store path.

## Phases

0. **Prep** (rust-pro for core; kotlin-specialist for Android): `downloads::space` + `min_free_mb`
   + "missing root keeps its cache" with tests; `DeviceForm` + `uiForm`; `AppSettings.tv` +
   `TvDefaults` (defaults / clamp / lenient JSON tests); `tv-material` in the version catalog;
   the `LyricsView` / `NowPlayingScreen` / `HomeScreen` splits; `TvActivity` label → "HomeFi",
   `MainActivity` forwarding, service intents by form. Verify: `make check`, Gradle compile,
   phone screenshots unchanged (`astral-a35`).
1. **Shell, login, player, install path** (ui-designer: a one-page TV spec `docs/TV_UI.md` with
   the drawer, card sizes, focus look per skin and the player; then kotlin-specialist): `TvRoot`,
   `TvTheme`, `TvFocus`, `TvLogin`, `TvHome`, `TvNowPlaying` with lyrics/karaoke, mascot,
   visualizer and queue panels, `TvAmbient`, keep-awake, keys, `TvTrain` (watch) = today's
   `TvScreen`; the phone helper "Put HomeFi on your TV"; README section + Downloader code
   (mobile-developer registers it with José). Verify on `homefi_googletv`: D-pad scripts
   (`adb shell input keyevent KEYCODE_DPAD_*`), screenshots of every screen, login with the real
   account by QR (read-only), play a Tidal song and a library file over HDMI, ambient after 60 s,
   update card flow with "unknown sources".
2. **Library, search, TIDAL, downloads** (kotlin-specialist screens; mobile-developer for
   `Storage` volumes, USB mount / unmount, "All files access" on TV): `TvLibrary`, `TvFolderPicker`,
   `TvSearch` + mic, `TvTidal`, `TvDownloads` with target and space. Verify: emulator with a
   virtual SD/USB (`-sdcard`), pull the drive mid-download, library with 2 000 files scrolls at
   60 fps, no `StrictMode` disk reads on Main.
3. **Playlists, Sound, AI, Party** (kotlin-specialist): `TvPlaylists` (chat / party / concert
   with mic + chips, save to TIDAL), `TvSound` (sounds, D-pad curve, assistant), `TvParty` (hub,
   QR, approvals, guests; join). Verify: a phone guest joins the TV hub on the emulator's LAN,
   party playback, the AI chat with a real key (José's), karaoke in the player while the hub runs.
4. **Train on the TV, Settings, what's new** (kotlin-specialist; mobile-developer for
   localhost `RideTvHost` + WebView): ride setup and run on the TV, `TvSettings` two-pane with
   every field, release notes dialog, DJ-with-mouse detection. Verify: a 10-minute ride on the
   emulator, every setting reachable by D-pad, a mouse on the emulator shows DJ.
5. **Release** (mobile-developer): low-RAM defaults, TV screenshots in the README, `play` flavor,
   AAB build in `release.yml` (not uploaded until José asks), TV quality checklist pass on a real
   Chromecast. Verify: `make check`, `pnpm check`, Gradle compile, release APK with R8 on the
   emulator and the real device.
6. **Later:** Android TV home-screen "Continue listening" channel (`androidx.tvprovider`),
   `MEDIA_PLAY_FROM_SEARCH` for "play X on HomeFi" (needs the Play listing), a global search
   provider, Cast receiver (paid registration: no).

## Tests

- Rust: `space::check` with a fake `statvfs` (enough / not enough / unreadable), lenient
  `min_free_mb`, library cache keeps a missing root.
- Kotlin unit: `DeviceForm.of(tvMode, leanback, override)`, `TvSettings` defaults / clamp /
  old JSON, `Storage.volumes` parsing with fake `StorageVolume`s, the key router (which key does
  what in each player state).
- Compose UI tests (emulator, `androidTest`): focus order of the drawer and a list,
  `performKeyInput` for DPAD and media keys, long-press opens the context menu, BACK from the
  player returns to the section, ambient appears after the timeout and any key hides it.
- Manual on `homefi_googletv` (Android 14, 1080p, 1.5 GB) and a real Chromecast: everything in
  the phase lists, plus font scale 1.3 and the light skins' focus contrast.

## Risks

- **Low-end boxes (Chromecast: 2 GB RAM, four small cores):** the mascot at 60 fps plus the
  visualizer plus blurred backdrops can stutter; defaults on `isLowRamDevice` are `motion = SUBTLE`,
  visualizer off, blur radius low, all settings. Measure with `adb shell dumpsys gfxinfo`.
- **HDMI is never bit-perfect on Android TV:** the system mixes at the TV's rate (usually 48 kHz)
  and the Chromecast has no other output; a USB DAC behaves as on the phone. Say so in Settings →
  Sound (the "bit-perfect" badge reads "system mix" on HDMI).
- **USB writing policies:** `ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION` exists on Android TV
  11+, but some boxes hide the page; raw-path reads of media on a removable volume with
  `READ_MEDIA_AUDIO` must be verified on the emulator (Android 11+ re-allowed file-path access for
  media, but the core scans with `read_dir`). Fallback for both is the app folder on the volume.
- **WebView on the emulator is 113** (no Play to update it): `ride.html` must stay inside
  Chrome 113 CSS/JS (no features newer than `color-mix`), tested there; real devices update it.
- **Play policies** (restricted permissions, AAB, closed test for new accounts) can delay the
  store path by weeks: the GitHub + Downloader path stays the fallback.
- **tv-material and material3 together:** two `MaterialTheme`s; the `TvTheme` wrapper is the only
  place that maps a `Skin` to both, and every shared composable keeps reading `LocalSkin`.
- **Remote without BACK on some boxes** (Fire TV has it, some Android TV remotes map it to Home):
  every screen also closes from its own "Close" row.

## Open questions for José

1. Which TV do you have for real tests: a Chromecast with Google TV (4K or HD, with a USB-C hub?)
   or a TV with Android TV built in? A second one from a friend for the party test?
2. Downloads on the TV: internal until it drops under 500 MB free, then offer USB (proposed), or
   always ask where?
3. USB without "All files access": files in `Android/data/com.perseusoft.homefi` on the drive
   (gone if the app is uninstalled, invisible to other TV players) vs asking for the permission once
   from Settings. Proposed: ask once, fall back to the app folder.
4. DJ on TV: hidden unless a mouse is connected (proposed), or always shown with the notice?
5. The Play build drops the self-updater and "All files access" (policy). OK with two builds of the
   same app (GitHub full, Play store-compliant), same signing key?
6. Voice input through the Google TV mic (`RecognizerIntent`: audio goes to Google, free, no key):
   fine for search and the AI chats?
7. Party on the TV: approve guests automatically by default (it's your living room)?
8. Do you own a domain for a short `/tv` link, or is the Downloader code enough?
9. Ambient mode is TV-only for now; do you want a desktop screensaver too (the "both platforms"
   rule), or leave it?

### José's answers (2026-11-09)

- The HomeFi TV app targets Android TV / Google TV only. A Chromecast (José has one) is a
  casting target only — that waits for the paid Google account (end of Nov 2026).
- Downloads on the TV: always ask where (`downloadTarget` default `ASK`).
- USB: ask for "All files access" when it is needed, every time it is missing (no silent
  fallback without asking first; the app folder stays the fallback if he declines).
- Party on the TV: guests are approved automatically by default (`partyAutoApprove = true`).
- Voice input: not answered; stays on as a setting (`voiceInput`).
