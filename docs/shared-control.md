# Shared control ("car mode") and the hub

One phone is the **hub**: it plays the music (into the car, a speaker...) and
owns the queue. Everyone else is a **guest**: from their own phone they search,
see what is playing and what comes next, add songs and, if the hub allows it,
skip, pause or reorder. Audio never leaves the hub in this mode.

The same hub later powers **party mode** (guests also receive the audio and
play it in sync on their own speakers), so everything here is written to be
reused by it.

## Who runs what

| Piece | Where | Why |
| --- | --- | --- |
| Protocol, hub server, guest client, fair queue, permissions | `homefi-core::share` (Rust) | one implementation for Android, the SR35 and desktop |
| Hub screen: "Share control", QR, guests, permissions, discreet notices | Android (Kotlin) + desktop (Svelte) | native UI |
| Guest experience | **HomeFi Android app** (native, main path) | everyone in the car has Android |
| Guest fallback | small web page served by the hub | a passenger without HomeFi (or with an iPhone) |

## Connection (no router, no internet needed)

- **Wi-Fi Direct group**: the hub creates it and is the group owner. Each
  phone keeps its own mobile data for internet, so nobody loses theirs.
- **Finding the hub without typing anything**: the hub announces a Wi-Fi
  Direct service (`_homefi._tcp`, name "Música de José"). A guest opening
  HomeFi sees a banner, "José is sharing music · Join", and taps it.
- **QR code**: shows the session (name, address, port, join code). It's the
  fallback when discovery doesn't work, and the way into the web page (in that
  case the hub also opens a local-only hotspot without internet).
- Android permissions: `NEARBY_WIFI_DEVICES` (API 33+), fine location below
  33. minSdk stays 24 (SR35).

## Finding a session

Guests look for hubs while HomeFi is open (any screen; a card offers "Join"),
on every channel at once; the same hub heard twice is one entry:

| Channel | Hub needs | Guest needs |
| --- | --- | --- |
| Wi-Fi Direct DNS-SD (`_homefi._tcp`) | nothing extra | location services below API 33 |
| NSD on the home Wi-Fi (also for a Wi-Fi Direct hub, "announce on the home Wi-Fi") | being on that Wi-Fi | nothing |
| Bluetooth LE beacon + GATT record (`Ble.kt`) | Bluetooth on | location below API 31 |
| Gateway: `GET /hello` on the Wi-Fi gateway | nothing | being joined to the hub's group or hotspot |

The gateway path is for devices with no camera and no GPS (the SR35): join the
hub's `DIRECT-hf-…` network once from the system Wi-Fi settings (the name and
password stay the same between parties, set in Settings), open HomeFi and the
session appears; only the 4-digit code is typed. `/hello` answers
`{"app":"homefi","v":1,"name":…,"party":…}`, never the code. Creating the
group needs no location services.

## Protocol

WebSocket with JSON messages on the hub (default port 47800), plus plain HTTP
for covers (`GET /cover/<id>`) and the web page (`GET /`).

```
guest → hub   hello { name, color, code, app_version }
hub → guest   welcome { guest_id, session_name, permissions, state }
hub → all     state { now_playing, position_ms, playing, queue: [QueueItem], guests }
guest → hub   search { q }                    → results { tracks, albums, artists, playlists }
guest → hub   browse { kind, id }             → page { header, tracks }      (album / artist / playlist)
guest → hub   add { item, next: bool }
guest → hub   remove { queue_id } | move { queue_id, to }
guest → hub   skip | vote_skip | toggle | volume { value }
hub → guest   notice { text } | denied { reason }
```

`QueueItem` = what (`tidal:<id>`, `hub:<library id>`, later `guest:<upload id>`),
title, artist, album, duration, cover url, `added_by` (name + color).

Item sources:

1. **Hub's library and the hub's Tidal account**: the guest searches through
   the hub, so guests need no account.
2. **The guest's own Tidal** (when logged in on their phone): they browse their
   own favorites and playlists natively and send `tidal:<id>`. The hub plays it
   with its own account.
3. *(later)* **A song that only the guest has**: the guest app uploads the file
   to the hub over HTTP. The hub keeps it in a temporary cache and deletes it
   after playing.

## Party mode: playing along on every phone

The use cases: a beach with no Wi-Fi where everyone brought a different
Bluetooth speaker; a car (control only, no audio to guests); home, where a
computer or another device joins over the home Wi-Fi.

With "stream audio" on, the hub serves the song playing now and every guest
plays it on its own speaker, in sync:

- **The file, not raw audio.** `state.audio` carries
  `/audio/<queue_id>?k=<session key>` (HTTP with range requests): each guest's
  own player downloads and decodes it (FLAC, ALAC, AAC...). It works the same
  for local songs and Tidal (the hub has the cached file) and uses far less
  air time than PCM. Only the song playing now is served, and only with the
  key (guests get it in `state`, so only people who joined).
- **Local first.** `state.prefetch` also lists the **next** and the **previous**
  song (same URL scheme). Guests keep everything in a cache on the phone: the
  song playing now as it streams, next and previous downloaded ahead one after
  the other. A song change then starts from local storage at once, and a busy
  radio doesn't cut the music. The cache is deleted on leaving ("Download the
  songs ahead", on by default).
- **One clock.** Guests send `clock { t0 }`, the hub answers at once with
  `clock { t0, hub_ms }`; the guest keeps the sample with the shortest round
  trip of the last 8 (NTP style). A burst of 5 after joining, then one every
  3 s. On the same Wi-Fi Direct group the error is a few ms.
- **Where the song is.** `state` has `position_ms` and `clock_ms` (the hub
  clock when that position was true); the hub refreshes it on every
  play / pause / seek and every few seconds. A guest's target is
  `position_ms + (hub_now - clock_ms)` while playing, plus **its speaker's
  delay** (a setting per phone: Bluetooth speakers lag 100-300 ms, differently
  per brand). The hub applies its own delay setting when reporting.
- **Staying together.** The guest measures the gap 4 times a second and uses
  the median of the last 7 (one late clock sample or report doesn't move it).
  `sync_action(target, actual)`: within 15 ms do nothing; up to 350 ms play up
  to ±2 % faster / slower, in 0.5 % steps changed at most every 1.5 s (each
  change is a tiny glitch); further than that, and only when 4+ measurements
  agree, jump. After starting or jumping, the position settles for 700 ms
  before it counts, and the time a jump takes is learnt for the next one.
  "Smooth sync" off only jumps.
- **Starting together.** On a new song the hub holds its player at 0 and
  announces a start in the future (`position_ms` 0, `clock_ms` = start time;
  guests wait at 0 until then). Each guest sends `ready { queue_id }` once the
  song is loaded (guests that don't play along send it at once). When all are
  ready the hub moves the start to 350 ms ahead; after the wait in Settings
  (3 s by default, 0-10) it starts anyway and late guests catch up. Guests time
  the start to the millisecond, not to the next 250 ms tick.
- **A quiet radio.** A Wi-Fi Direct scan takes the radio off the group's
  channel for a moment, which is what made the music cut out. Guests stop
  every search once they are in; the hub stops scanning as soon as one guest
  is in (newcomers still find it by Bluetooth, NSD or the gateway) unless "Keep
  scanning over Wi-Fi Direct with guests in" is on.
- **Meters for everyone.** Guests that play along read their own audio (the
  party player has the same tap as the app's player). For the rest the hub
  sends `levels { l, r, b }` (0-255, 32 spectrum bands) ~15 times a second
  while playing, a few hundred bytes a second ("Send the visualizer to
  guests", on by default), so visualizer, VU meters and mascot move on every
  phone.

```
guest → hub   clock { t0 }                       hub → guest  clock { t0, hub_ms }
hub → all     state { ..., position_ms, clock_ms, audio: {...}, prefetch: [{...}] }
guest → hub   ready { queue_id }                 hub → all    levels { l, r, b: [..32] }
GET /audio/<queue_id>?k=<key>   (Range: bytes=...)
```

Networks: **Wi-Fi Direct** (the hub creates the group; guests join it with
the group name and passphrase from discovery or the QR code, keeping their
mobile data) or the **home Wi-Fi** (NSD / mDNS `_homefi._tcp`). The QR code is
`homefi://join?host=..&port=..&code=..&name=..[&net=..&pass=..&mode=..]`, so a
phone's camera app opens HomeFi straight into joining.

## Desktop (macOS, Windows, Linux)

The desktop app is a hub or a guest with the same protocol and core
(`apps/desktop/src-tauri/src/share/`), and every option of Android's "Shared
control & party" settings except the phone-only radios (Wi-Fi Direct group name
and password, Bluetooth, "keep scanning over Wi-Fi Direct").

- **Network: the one the computer is on** (home Wi-Fi, Ethernet, a phone's
  hotspot). No Wi-Fi Direct, no Bluetooth. The hub binds `0.0.0.0` on the port
  in Settings (47800) and announces itself with mDNS / DNS-SD (`mdns-sd`, pure
  Rust on all three systems): `_homefi._tcp`, instance = session name, TXT `v=1`,
  `name`, `port` (the same records as Android's NSD; never the code). Guests
  browse for it while "Look for music nearby" is on and they are neither hub
  nor guest; a typed address is checked with `GET /hello`. QR and links are the
  same `homefi://join?...` (no `net` / `pass`).
- **Hub, mirrored to the player.** The hub owns the shared queue; the player
  holds what of it is on this computer: library files, and Tidal songs fetched
  into the stream cache up to 3 ahead (`Player::set_upcoming` replaces what
  comes after the current song without touching it). The host's own edits
  (play, play next, add to queue, dragging in the queue panel) go through the
  hub; a list changed in the player itself (a new album while the queue was
  empty, a jump, previous) is pushed to the hub as the host's songs, like
  Android's reconcile. Shuffle is off while sharing. At most 100 shared songs
  sit in the player at once.
- **Party, as hub:** `set_audio` / `set_next_audio` with the local or cached
  file, the hold at 0 on each new song until every guest is ready (the wait in
  Settings), levels ~15 times a second from the engine's analyser, "play here"
  off mutes the output and keeps the clock (guests still see the real volume).
- **Party, as guest:** a second engine plays the hub's song (the user's own queue
  stays as it was and is there after leaving). Songs are downloaded whole into
  `<app dir>/party-cache` (range requests resume an interrupted download; next
  and previous ahead), deleted on leaving. **Sync is by seeking only**: the
  desktop engine can't change its playback rate yet, so "Smooth sync" is kept
  as a setting but only phones use it; the desktop jumps when the median of the
  last 7 gaps is over 80 ms (4+ agreeing, 700 ms settle, learnt seek lead).
  Scheduled starts load the song paused and silent at its start, send `ready`,
  and start on a timer at the hub's instant. Not playing along, the visualizer,
  VU meters and mascot follow the hub's `levels`.
- **UI:** a share button in the deck and the rail opens the panel (start in
  control or party mode, QR, big code, guests with remove + an unticked "Also
  block this device", approvals, stop; or the sessions found nearby and "Join
  with a code"). As a guest, the room view has Player (the normal room in the
  chosen layout, controls as the hub allows), Room (now playing, skip / vote,
  volume, play here + speaker delay, the queue with play now / play next / move
  / remove) and Search. The deck and the mini player show the room's song
  ("From <session>") until the user plays their own music.
- Dev aid: a debug build started with `HOMEFI_SHARE_AUTOSTART=<code>` (and
  `HOMEFI_SHARE_PARTY=1`) shares at launch; `cargo test -p homefi-desktop smoke
  -- --ignored --nocapture` then joins it like a guest.

## Permissions (all set by the hub)

| What | Options |
| --- | --- |
| Add songs | free · hub approves · max N per person |
| Queue order | first come · **turns** (round robin per guest) |
| Skip | nobody · everyone · **vote** (half of the guests) |
| Pause / volume | yes · no |
| Remove / move | own songs · **all** (the app's default) · none |
| Play a queued song now | with pause / volume, or when everyone can skip |
| Explicit content | allow · block |
| Guests' songs when they leave | stay · leave with them |

The hub's own "play next" really is next (it skips the turns); a guest's
"play next" only reorders their own songs. A guest's queue menu has play now,
play next, move up / down and remove (each only when the rules allow it).
The hub can remove a guest (asked first; blocking that device until the
session ends is a separate, unticked choice). The join code in the QR code or discovery reply
stops people outside the car from joining.

## Fair queue

With "turns", songs added by guests are interleaved: one from each person in
order, the hub's own songs counting as one more person. The order is
recomputed when someone adds or leaves; songs already started never move.

## Phases

1. **Core**: `homefi-core::share` with the hub server (WebSocket + HTTP), the
   guest client, the protocol types, permissions and fair queue. FFI for
   Android (start/stop hub, events to Kotlin; guest connect/send/state flow).
   Tests with two in-process clients.
2. **Android hub**: "Share control" in the player, QR, list of guests,
   permissions in Settings, notices that don't need the driver's attention.
   Wired to `PlayerController`.
3. **Android guest**: Wi-Fi Direct discovery banner, join, a "remote" mode
   where the queue sheet and the now-playing bar show the hub and the
   Search/Tidal/Library screens get an "Add to José's queue" action.
4. **Web fallback** page and the desktop hub (desktop hub and guest: done, LAN only).
5. Guest uploads; then party mode (audio to guests, clock sync), reusing all
   of the above.
