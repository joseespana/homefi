# HomeFi for iPhone

A native SwiftUI app (`apps/ios`) on the same Rust core as the CLI, the
desktop and Android: `homefi-ffi` is built as a static library for iPhone and
the simulator, packed into `HomefiCore.xcframework`, and UniFFI generates the
Swift bindings (`HomefiCore.swift`). Private project: it is only installed by
hand from a Mac, never published.

## Build

Requirements: Xcode (iOS 17+ SDK), XcodeGen (`brew install xcodegen`), Rust with
`rustup target add aarch64-apple-ios aarch64-apple-ios-sim`.

```bash
make ios          # core + Xcode project, opens Xcode
make ios-sim      # command-line simulator build (no signing)
```

- `apps/ios/project.yml` is the source of truth; `HomeFi.xcodeproj` and
  `apps/ios/Core/` are generated and not committed.
- Xcode runs `scripts/build-core.sh` before every build (debug core for Debug,
  release for Release), in a clean environment so Xcode's SDK variables do not
  reach cargo.
- Only arm64 (iPhones and Apple-silicon simulators).
- Debug builds accept `-HomeFiDemo` (plays the first song and opens the
  player) and any setting as `-<key> <value>` (e.g. `-mascot_dance RUNNING`)
  for simulator screenshots.

## Installing on an iPhone (by hand)

1. Xcode → Settings → Accounts: add the Apple ID.
2. Open the project, target HomeFi → Signing & Capabilities → Team: the
   Apple ID's team.
3. Connect the iPhone by cable, trust the Mac, and turn on
   Settings → Privacy & Security → **Developer Mode** (it restarts).
4. Pick the iPhone as the destination and Run (⌘R). The first time, the
   iPhone asks to trust the developer: Settings → General → VPN & Device
   Management.

With a free Apple ID the app stops opening after **7 days** (run it from
Xcode again to renew it) and the account can sign up to 3 apps per device.
A paid Apple Developer account makes it last a year.

## Music on the iPhone

The app's Documents folder is shared: Finder → the iPhone → Files → HomeFi
(drag folders there), or the Files app → On My iPhone → HomeFi. Pull down in
the Library to rescan. Tidal streams through the core into the caches folder
(2 GB cap).

## State

Done: library (songs / albums / artists, embedded covers, play next / add to
queue), Tidal sign-in (device code), favorites and search, player with
background audio and lock-screen controls, the mascot (1:1 port of Android,
with sideways turns) as the player's stage, settings (theme and every mascot
option, same keys as Android), English and Spanish.

Pending (to match Android): downloads, Tidal artist / album / playlist pages,
.m3u playlists, Home with recents, the 8 player layouts and retro skins,
progress bar styles, output picker, audio analysis for the mascot (it uses
120 BPM and medium energy for now), shared control (car mode).
