---
name: rust-pro
description: HomeFi Rust playbook - conventions and step-by-step recipes for the homefi-core / homefi-cli / homefi-ffi workspace (new Tidal endpoint, model field, template field, config key, CLI flag, download event, FFI export, tests with wiremock, Android cross-compile). Use whenever writing or reviewing Rust in this repo.
---

# HomeFi Rust playbook

Recipes for this workspace. For the role and invariants see `.claude/agents/rust-pro.md`;
the short version: tiddl compatibility, core never prints (events), no panics, lenient serde,
pure-Rust Android-safe deps, one workspace version.

## Conventions

- `homefi_core::Result<T>` / `Error` (thiserror). Add a variant only when callers need to match
  on it (e.g. `is_authorization_pending`); otherwise reuse `Decode`, `Stream`, `Template`, ...
- Async everywhere on tokio; blocking file/tag work in `spawn_blocking`
  (see `download/engine.rs::spawn_blocking`). Never hold a `std::sync::MutexGuard` across `.await`
  (FFI futures must be `Send`).
- `rustfmt.toml`: `max_width = 120`, `use_small_heuristics = "Max"`. Clippy with `-D warnings`.
- `rust-version = 1.85` in `Cargo.toml`: don't use newer std APIs without bumping it.
- Dependencies are declared in the root `[workspace.dependencies]` and referenced with
  `.workspace = true`. Before adding one, check it builds for `aarch64-linux-android` (no C deps / OpenSSL).
- Comments explain *why*; doc comments on public items. Match the density of the file.

## Recipes

### New Tidal endpoint
1. Response type in `models.rs` (`#[serde(rename_all = "camelCase")]`, `default`/`Option` for
   non-essential fields, `de_null_default` for nullable arrays). Add a parse test with real-looking JSON.
2. Method on `TidalApi` (`api/mod.rs`) with a `CachePolicy`: `HOUR` for catalog data,
   `NoStore` for user/stream data, `WEEK` for near-static data. Paginated → `Page<T>` + `next_offset()`.
3. Test the HTTP flow with `wiremock` and `TidalClient::with_base_urls(...)`.

### New template field
1. Add it to `ItemFields` / `AlbumFields` / `PlaylistFields` in `template.rs`, fill it in the
   builder (`item()`, `album()`, `playlist()`), expose it in `lookup()`.
2. Pick the `Value` kind (`Int`, `Str`, `Date`, `Explicit`, `Flag`) so Python-style specs work.
3. Test in `template::tests::format_specs`, document in `docs/templating.md`.

### New config key / CLI flag
1. Field in the right `*Config` struct in `config.rs` (all have `#[serde(default, deny_unknown_fields)]`),
   default in its `Default` impl, copy into `DownloadOptions::from_config` if the engine needs it.
2. Document it in `docs/config.example.toml` (the `example_config_is_valid` test parses this file).
3. CLI: field in `DownloadArgs` (`crates/homefi-cli/src/main.rs`), keep tiddl's flag name and add
   `visible_alias` for its old short form; apply it in `download()`.
4. If the app should expose it: FFI `DownloadSettings` + Android settings (fullstack-developer).

### New download outcome
Add an `Event` variant (`download/mod.rs`), count it in `Downloader::emit` if it affects
`Summary`, render it in `crates/homefi-cli/src/progress.rs`, map it in
`crates/homefi-ffi/src/lib.rs` (`ListenerObserver`) and `DownloadRepository.kt`.

### New FFI export
- `#[uniffi::export]` free fn, or a method in the `#[uniffi::export(async_runtime = "tokio")] impl HomefiEngine`.
- Only FFI-owned `uniffi::Record` / `uniffi::Enum` types cross the boundary; convert from core types.
- Errors: return `Result<T, HomefiError>` (flat error: the `Display` text is the Kotlin message).
- Then `make android-debug` regenerates the Kotlin bindings and compiles the app.

## Testing patterns

| Need | Where / how |
| --- | --- |
| Pure logic | `#[cfg(test)] mod tests` next to the code |
| HTTP flows | `crates/homefi-core/tests/*.rs` with `wiremock::MockServer` |
| Real audio files | generate with ffmpeg in the test and skip gracefully when missing (`tests/tagging.rs`) |
| Filesystem | `tempfile::tempdir()` |

## Commands

```bash
make check                                   # fmt --check + clippy -D warnings + tests
cargo test -p homefi-core --test download_e2e
cargo run -p homefi-cli -- --help            # HOMEFI_PATH=<tmp dir> to avoid touching ~/.homefi
cargo ndk -t arm64-v8a build -p homefi-ffi --release   # needs ANDROID_NDK_HOME
```

The UniFFI bindgen reads metadata from the **host** debug library (`target/debug/libhomefi_ffi.*`):
release `.so` files are stripped and produce no bindings.
