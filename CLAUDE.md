# HomeFi

Read [`docs/HANDOFF.md`](docs/HANDOFF.md) first: the idea, the state of every
part, what comes next and how to set up a new machine.

Rules from the owner (José):

- Talk to him in Spanish.
- Everything is configurable, on Android and desktop alike ("que todo sea
  configurable en phone y en desktop todo porque es nuestra filosofia"). A new
  look or behaviour is a setting on both platforms.
- Public since v1, carefully: the only things published are the installers the
  release workflow builds for a `vX.Y.Z` tag, and only when José asks. Never
  publish the SR35 APK (it uses Spotify's app id), keys, sessions or personal
  data. Users accept the personal, non-commercial use notice before connecting
  Tidal.
- Never print tokens (`~/.homefi/auth.json`). Only read-only calls against the
  real Tidal account.
- Deleting moves files to the Trash, and only library files.
- The Android APK is built and signed in CI with the `ANDROID_*` secrets; the
  release key lives outside the repo in `~/.homefi-release/`.
- Before saying something is done: `make check`, `pnpm check` in
  `apps/desktop`, and `./gradlew -q compileDebugKotlin -Phomefi.skipRust=true`
  in `apps/android`.
