# Releasing HomeFi

## 1. Bump the version

```bash
cargo xtask bump 1.4.0          # explicit
cargo xtask bump patch          # 1.3.9 -> 1.3.10, 1.4.0-beta.2 -> 1.4.0
cargo xtask bump 1.5.0-beta.1   # pre-release (published as a GitHub pre-release)
```

This updates `Cargo.toml` and `Cargo.lock`, and moves the `[Unreleased]` notes in
`CHANGELOG.md` under the new version. The Android app reads the same version at build time:

| Version | `versionName` | `versionCode` |
| --- | --- | --- |
| `1.4.0` | `1.4.0` | `1004000` |
| `1.5.0-beta.1` | `1.5.0-beta.1` | `1005000` |

Pass `-Phomefi.versionCode=<n>` to Gradle if you ever need to override the code.

## 2. Tag and push

```bash
git commit -am "release: v1.4.0"
git tag v1.4.0
git push --follow-tags
```

The release workflow checks that the tag matches `Cargo.toml`, builds every artifact and
publishes a GitHub Release with them.

## Stable download links

Besides `HomeFi-<v>-…`, every build also carries the same files without the version
(`HomeFi-macos-arm64.dmg`, `HomeFi-android.apk`, `homefi-cli-<target>.tar.gz`, …). The README's
download table points at `releases/latest/download/<name>`, which always serves the newest
stable release (pre-releases don't count as latest).

## Builds from `main`

Every push to `main` runs the same workflow (`.github/workflows/release.yml`) without
publishing anything: the installers are kept as workflow artifacts for 30 days
(Actions → the run → **Artifacts**; `HomeFi-<version>-main.<run>` has everything plus `SHA256SUMS`).

| Platform | Architectures | Files |
| --- | --- | --- |
| macOS 11+ | Apple Silicon (`arm64`), Intel (`x64`) | `HomeFi-<v>-macos-<arch>.dmg` |
| Windows 10/11 | `x64`, `arm64` | `HomeFi-<v>-windows-<arch>.setup.exe` (+ `.msi` on x64) |
| Linux | `x86_64`, `arm64` | `.deb` (Debian/Ubuntu), `.rpm` (Fedora/openSUSE), `.AppImage` (any) |
| Android 8+ | arm64-v8a, armeabi-v7a, x86_64 (universal) | `HomeFi-<v>-android.apk` |
| CLI | the six desktop targets | `homefi-cli-<v>-<target>.tar.gz` / `.zip` |

There is no iOS build: the app is about your own files and output devices, which iOS doesn't open up.

Linux arm64 builds use GitHub's `ubuntu-22.04-arm` runners. If they aren't available for
the repository's plan, those two jobs stay queued; the rest of the release is not blocked.

## In-app updates

Every app offers new versions by itself (see the README's *Updates*). The only calls are to
GitHub: `api.github.com/repos/joseespana/homefi/releases[/latest]` and the release files.

### Desktop: signing key and `latest.json`

The desktop app updates with [tauri-plugin-updater](https://v2.tauri.app/plugin/updater/): it
reads `latest.json` and only installs a file whose [minisign](https://jedisct1.github.io/minisign/)
signature matches the public key in `apps/desktop/src-tauri/tauri.conf.json`
(`plugins.updater.pubkey`). The private key never goes in the repository. Add it as secrets:

```bash
gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.homefi-release/updater.key
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD < ~/.homefi-release/updater-password.txt
```

| Secret | Value |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | contents of the private key (`pnpm tauri signer generate -w ~/.homefi-release/updater.key`) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | its password |

A tag fails without them (like the Android key): a release whose files aren't signed can't be
installed by the apps already out. Builds of `main` without them skip the updater files. **Losing
the key** means the installed apps can't update in place any more: a new key needs a release that
users install by hand once (it carries the new public key).

With the key, the desktop jobs also produce the updater's files and their `.sig`:

| Platform | Updater file (in the release) | `latest.json` targets |
| --- | --- | --- |
| macOS | `HomeFi-<v>-macos-<arch>.app.tar.gz` (from the `app` bundle) | `darwin-aarch64`, `darwin-x86_64` (+ `-app`) |
| Windows | the setup `.exe` and the `.msi` themselves | `windows-x86_64` / `windows-aarch64` (+ `-nsis`), `windows-x86_64-msi` |
| Linux | the `.AppImage`, `.deb` and `.rpm` themselves | `linux-x86_64` / `linux-aarch64` (+ `-appimage`, `-deb`, `-rpm`) |

The app looks for `<os>-<arch>-<install type>` first (the bundler writes the install type into
each package's binary), then `<os>-<arch>`, so a `.deb` install updates with the `.deb`. The
`.deb` / `.rpm` are installed with `pkexec` (a password prompt); without `pkexec`, and for copies
that can't replace themselves (a dev build, run from the `.dmg`, macOS's translocated copy,
an AppImage without `$APPIMAGE`), the app only offers **Download** (the release page).

The `collect` job writes `latest.json` for every tag (`version`, `notes` = the CHANGELOG section,
`pub_date`, `platforms` with the signature and the URL of this tag's versioned file) and
publishes it with the release. `releases/latest/download/latest.json` (the app's endpoint) is
always the newest **stable** release, because GitHub's "latest" skips pre-releases; with
"Include pre-releases" on, the app reads the newest release from the API and uses that
pre-release's own `latest.json` asset. The `.sig` files go into `latest.json`, not the release.

macOS: the app is ad-hoc signed. The updater downloads with its own HTTP client, which doesn't
quarantine the file, so the new `HomeFi.app` it swaps into `/Applications` opens without the
"Open Anyway" step (when the folder isn't writable by the user, macOS asks for an administrator
password). Each ad-hoc build has a new signature, so macOS may ask once more for access to the
keychain item of the AI keys ("Always Allow").

Windows: the setup runs in passive mode (a progress window, no questions) and opens HomeFi
again; the app saves its settings and closes first.

### Testing an update locally

The manifest's `version` must be the version the file was signed for (the signature records it,
and `requireSignedVersion` is on), so the test installs a real upgrade: an app built as an older
version updates to the current build.

```bash
cd apps/desktop
export TAURI_SIGNING_PRIVATE_KEY=~/.homefi-release/updater.key
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat ~/.homefi-release/updater-password.txt)"
HTTP='"plugins":{"updater":{"dangerousInsecureTransportProtocol":true}}'   # test builds only

# 1. the update: this version, signed (target/release/bundle/macos/HomeFi.app.tar.gz + .sig)
pnpm tauri build --bundles app --config "{$HTTP}"
mkdir -p /tmp/upd/srv && cp ../../target/release/bundle/macos/HomeFi.app.tar.gz* /tmp/upd/srv/

# 2. the app to update: the same code calling itself 1.0.9
pnpm tauri build --bundles app --config "{\"version\":\"1.0.9\",$HTTP}"
cp -R ../../target/release/bundle/macos/HomeFi.app /tmp/upd/   # anywhere writable

# 3. the manifest, served over plain HTTP
python3 - "$(cargo xtask version)" <<'PY'
import json, sys
sig = open("/tmp/upd/srv/HomeFi.app.tar.gz.sig").read()
entry = {"url": "http://127.0.0.1:8765/HomeFi.app.tar.gz", "signature": sig}
json.dump({"version": sys.argv[1], "notes": "- Local test", "pub_date": "2026-01-01T00:00:00Z",
           "platforms": {"darwin-aarch64": entry, "darwin-x86_64": entry}}, open("/tmp/upd/srv/latest.json", "w"))
PY
python3 -m http.server 8765 --bind 127.0.0.1 --directory /tmp/upd/srv &

# 4. run it with its own data folder, pointed at the local manifest
HOMEFI_PATH=/tmp/upd/home HOMEFI_UPDATE_ENDPOINT=http://127.0.0.1:8765/latest.json \
  /tmp/upd/HomeFi.app/Contents/MacOS/homefi-desktop
```

About 20 s after launch the card says "HomeFi <version> is available"; **Update now** downloads
and verifies it, **Restart now** swaps the app and opens the new one (Settings → About shows the
new version). With "Install updates: When HomeFi is idle" (Settings → About) it does all of
that by itself after 5 idle minutes. `HOMEFI_UPDATE_ENDPOINT` replaces the manifest URL; release
builds only accept HTTPS ones. A tampered file, another key's signature or a manifest whose
version differs from the signed one is refused ("The update's signature didn't match").

### Android and CLI

Android (the official `com.perseusoft.homefi` build) reads the same releases through the core
(`homefi_core::update`), downloads `HomeFi-android.apk`, checks it against the release's
`SHA256SUMS`, and opens Android's installer. It only installs over the existing app when the APK
is signed with the same key (`ANDROID_KEYSTORE_*`). The SR35 and debug builds have another
package name and never update themselves. The CLI prints a notice once a day and never installs.

## Cloudflare R2 (optional)

With these set, every build is also uploaded to an R2 bucket (the step is skipped otherwise):

| Kind | Name | Value |
| --- | --- | --- |
| Secret | `R2_ACCOUNT_ID` | Cloudflare account id (R2 overview page) |
| Secret | `R2_ACCESS_KEY_ID` | R2 API token → Access Key ID |
| Secret | `R2_SECRET_ACCESS_KEY` | R2 API token → Secret Access Key |
| Variable | `R2_BUCKET` | bucket name, e.g. `homefi-releases` |
| Variable | `R2_PUBLIC_URL` | optional: public / custom domain of the bucket, used for the URLs in `manifest.json` |

Create the token in Cloudflare → R2 → **Manage API tokens** with *Object Read & Write* on that bucket only.

| Path in the bucket | Contents |
| --- | --- |
| `releases/<version>/` | every tag |
| `latest/` | the last stable tag (pre-releases don't replace it) |
| `main/<version>-main.<run>/` | every push to `main` |
| `main/latest/` | the last build of `main` |

Each folder has a `manifest.json` with the files, their sizes, SHA-256 and (with `R2_PUBLIC_URL`)
download URLs. Builds of `main` pile up: add a lifecycle rule on the `main/` prefix
(R2 → bucket → Settings → Object lifecycle rules), e.g. delete after 30 days.

## Desktop signing (optional)

Unsigned installers work, but the OS warns the first time:

- **macOS**: the app is *ad-hoc* signed (no Apple Developer account needed; a build with no
  signature at all would be reported as "damaged" on Apple Silicon). The first time:
  open it, close the "HomeFi can't be opened" dialog, then System Settings → Privacy & Security →
  **Open Anyway** (on macOS 14 and older, right-click the app → **Open** also works).
  Or, in a terminal: `xattr -dr com.apple.quarantine /Applications/HomeFi.app`.
- **Windows**: SmartScreen → **More info** → **Run anyway**.

To sign and notarize the macOS app with a Developer ID instead (needs the paid Apple Developer
Program), add these secrets:

| Secret | Value |
| --- | --- |
| `APPLE_CERTIFICATE` | `base64 -i DeveloperID.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | password of the `.p12` |
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: Name (TEAMID)` |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | for notarization (`APPLE_PASSWORD` is an app-specific password) |

## Android

The `android` job of the release workflow builds the universal APK and signs it with the
secrets below. A tag fails without them: a published APK signed with a throwaway key could
never be updated. To build the same APK locally:

```bash
export HOMEFI_KEYSTORE_FILE=~/keys/homefi-release.jks HOMEFI_KEYSTORE_PASSWORD=… \
       HOMEFI_KEY_ALIAS=homefi HOMEFI_KEY_PASSWORD=…
make android                     # signed release APK, all ABIs
# apps/android/app/build/outputs/apk/release/homefi-<version>-release.apk
adb install -r apps/android/app/build/outputs/apk/release/homefi-*-release.apk
```

### Astell&Kern players (SR35 and similar)

A&K players have no `adb` and only install APKs placed in the slot of an app they know, in
`OpenService/Download/` (Spotify.apk, Tidal.apk, ...). An APK with HomeFi's own package name
fails there with "There was a problem parsing the package". Build one that takes the id of a
slot app that is **not** installed on the player (Spotify here), and copy it as that file:

```bash
cd apps/android
./gradlew assembleRelease -Phomefi.abis=arm64-v8a,armeabi-v7a \
  -Phomefi.applicationId=com.spotify.music -Phomefi.minSdk=24 -Phomefi.compressLibs=true \
  -Phomefi.versionCode=1000000000
# copy app/build/outputs/apk/release/homefi-*-release.apk to OpenService/Download/Spotify.apk
# (MTP: macOS needs OpenMTP or libmtp), then on the player: A logo → Services → Spotify → Install
```

Updates install over it only with a higher `versionCode` and the same signing key. Don't
update "Spotify" from the A&K menu: it would replace HomeFi.

This build is for your own player only and is never published (its id is another app's).
Keep signing it with the debug key it was first installed with: don't put the release key in
`~/.gradle/gradle.properties`, or the player refuses the update.

## Android signing

Create a keystore once and keep it safe. Losing it means users have to uninstall before
they can update:

```bash
keytool -genkeypair -v -keystore homefi-release.jks -alias homefi \
  -keyalg RSA -keysize 4096 -validity 10000
```

Add these repository secrets (Settings → Secrets and variables → Actions):

| Secret | Value |
| --- | --- |
| `ANDROID_KEYSTORE_BASE64` | `base64 -i homefi-release.jks` |
| `ANDROID_KEYSTORE_PASSWORD` | keystore password |
| `ANDROID_KEY_ALIAS` | `homefi` |
| `ANDROID_KEY_PASSWORD` | key password |

For local release builds, export `HOMEFI_KEYSTORE_FILE`, `HOMEFI_KEYSTORE_PASSWORD`,
`HOMEFI_KEY_ALIAS` and `HOMEFI_KEY_PASSWORD`, or put `homefi.keystore.file`,
`homefi.keystore.password`, `homefi.key.alias` and `homefi.key.password` in
`~/.gradle/gradle.properties`. Without them the release APK is signed with the debug key.
It installs fine, but it can't be updated by a properly signed build later.

## Local builds

```bash
make dist   # CLI for this machine + release APK + SHA256SUMS in dist/
```
