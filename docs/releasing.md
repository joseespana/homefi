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
