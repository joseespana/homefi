---
name: homefi-engineering
description: HomeFi's engineering principles, required for every code change in this repo (Rust core/player/FFI, Tauri + Svelte desktop, Kotlin/Compose Android, SwiftUI iOS) - SOLID, DRY, YAGNI, layers, composition and when inheritance is right, memory and allocation, how variables are created, stability, and design quality. Use before writing, reviewing or refactoring any code.
---

# HomeFi engineering principles

The owner's rule (José): every change follows SOLID, DRY, YAGNI, clear layers, the right use of
inheritance, careful memory management and variable creation, and whatever is most stable, with
good design. This file says what that means in each part of HomeFi. Read it with CLAUDE.md, the
`homefi-rust` playbook and `docs/HANDOFF.md`.

## 1. Layers (and which way they depend)

```
apps (desktop Svelte · Android Compose · iOS SwiftUI)   UI: draws state, sends intents
  └─ app shells (Tauri commands · Kotlin ViewModels/repositories · Swift stores)
       └─ homefi-ffi (UniFFI)   thin, typed bridge; no logic of its own
            └─ homefi-core       domain: Tidal API, library, dsp, ai, share, downloads
homefi-player (desktop audio engine) depends on homefi-core::dsp, never on the app
```

- Dependencies point **down only**. The core never knows Tauri, Android or a screen; it reports
  through events / return values ("core never prints").
- Each layer translates at its edge: core types → FFI records → Kotlin/Swift/TS types. Don't leak
  a lower layer's details (HTTP status, file paths, reqwest errors) into the UI: map them to
  `humane` messages at the boundary.
- One home per rule: a product rule (caps on EQ boosts, taste precedence, album grouping, update
  checks) lives in **homefi-core** once and every app calls it. Duplicating it in Kotlin and TS is
  a bug waiting to happen.

## 2. SOLID, applied

- **S — one reason to change.** A module/struct/component does one job: `dsp::autoeq` fetches and
  parses profiles, it doesn't apply them; `EqProcessor` runs biquads, it doesn't decide which.
  Svelte components draw; state and IPC live in `lib/*.svelte.ts`. Split a file when it has two
  jobs, not when it's merely long.
- **O — extend without editing.** New providers, output kinds, skins, reactions are new enum
  variants / trait impls / table rows, not new `if` chains scattered across files. Exhaustive
  `match` (Rust) and `when` (Kotlin) make the compiler list every place to update.
- **L — substitutable.** Every `PlayerSource` (own player, guest, felt wrapper) must behave like
  the contract says (capabilities say what's allowed; no surprise panics or no-ops).
- **I — small interfaces.** Pass what a function needs (`&[EqBand]`, a `ReactionContext`), not
  the whole `AppState`/`Settings`. Kotlin: inject the narrow flow or repository, not the app.
- **D — depend on abstractions at seams.** Network, clock, filesystem and the AI provider sit
  behind traits/interfaces so tests use wiremock / fakes (see `ai::chat`, `api::client`).

## 3. DRY and YAGNI (together)

- **DRY:** extract the second time you'd copy *logic* (not the third). Shared constants
  (limits, ranges, keys) are defined once (`dsp` limits, `EQ_LIMITS`, `TIDAL_TERMS_VERSION`).
  Strings that must match across platforms (settings keys, JSON field names) come from one place
  or a test checks them.
- **YAGNI:** build what was asked, plus what the "everything configurable" rule requires — no
  speculative frameworks, plugin systems, generic abstractions with one user, or settings nobody
  asked for. Prefer deleting code to keeping "maybe later" paths. A new dependency needs a clear
  payoff (see the async audit: `arc-swap` yes, Rx frameworks no).
- Duplication is better than the wrong abstraction: if two things only look alike, keep them apart.

## 4. Inheritance vs composition (per language)

- **Rust:** there is no class inheritance — use traits for behaviour, composition for data,
  enums for closed sets (`AiProvider`, `TasteScope`, `OutputKind`). Default trait methods for
  shared behaviour; never emulate inheritance with `Deref` tricks.
- **Kotlin:** prefer composition and interfaces; use `sealed interface/class` for closed state
  hierarchies (UI state, events, results) so `when` is exhaustive. Open class inheritance only
  for framework types (Activity, Service, ViewModel, Media3 `ForwardingPlayer`/`BaseAudioProcessor`)
  and keep those subclasses thin.
- **Svelte/TS:** components compose (props + snippets), no class hierarchies for UI; plain
  functions and small classes with `$state` for stores. Discriminated unions for view/state types.
- **Swift:** protocols + value types (`struct`), classes only for shared mutable objects
  (`@Observable` stores).

## 5. Memory management and allocation

- **Audio callbacks (cpal, Media3 processors, AVAudio):** zero allocation, no locks that can
  block (only `try_lock`/atomics), no syscalls, no logging. Pre-size buffers before the stream
  starts; fixed-size `Copy` designs (`EqDesign`). Move whole frames only.
- **Rust:** borrow before you clone; clone `Arc`s, not big `Vec`s; publish snapshots behind
  `Arc`/`ArcSwap` instead of cloning queues on every tick; `Vec::with_capacity` when the size is
  known; avoid `to_lowercase()` in comparators (`sort_by_cached_key`). Never hold a
  `std::sync::MutexGuard` across `.await` or I/O. Drop large buffers as soon as they're done.
- **Kotlin:** no `Context`/View held in long-lived objects (leaks); `remember`/`derivedStateOf`
  for computed UI values; `collectAsStateWithLifecycle`; cancel coroutines with their scope;
  close streams/cursors with `use {}`; bitmaps decoded at the size shown.
- **Svelte/TS:** `$state.raw` for big lists replaced wholesale; derive small per-row state, never
  per-row subscriptions to the whole player; virtualize long lists; clean up listeners, intervals
  and `unlisten` handles in effect teardowns; cancel in-flight loads on navigation.

## 6. Creating variables

- Immutable by default: `let` (Rust) without `mut`, `val` (Kotlin), `const` (TS), `let` (Swift).
  Make something mutable only where it changes.
- Smallest scope, declared where first used, one meaning per name. Names say what the value is
  in the domain (`gain_db`, `output_id`, `album_in_order`), units in the name when ambiguous
  (`_ms`, `_hz`, `_db`).
- No magic numbers: a named `const` with a doc comment saying why (`CLOSE_GRACE`, `MAX_BOOST_DB`).
- Types over strings/bools: enums instead of string modes, newtypes/records instead of tuples
  that cross layers, `Option`/nullable instead of sentinel values.

## 7. Stability first

- No `unwrap`/`expect`/`!!` on anything that can fail at runtime (files, network, user data,
  locks — recover from poison). Errors are typed and turned into humane messages at the edge.
- Every network call has a timeout; GETs retry 429/5xx with backoff; long work is cancellable.
- Writes are atomic (temp + rename); settings and caches tolerate corrupt/old JSON (lenient serde,
  defaults per field) without wiping everything.
- The UI thread never waits: Tauri commands that can block are async/spawn_blocking; Android
  calls the core off Main; heavy JS work is chunked.
- Fallbacks over failures: if the AI, a stream, the second crossfade deck or an update isn't
  ready, degrade to the simple path and say so.
- Tests for every rule that matters (caps, precedence, parsing, retries, channel mapping), with
  fixtures/wiremock, no real network in CI.

## 8. Design

- Every new look or behaviour is a setting on desktop **and** Android (CLAUDE.md).
- Respect skins/themes (tokens, `LocalSkin`, `data-skin`), the motion setting and haptics.
- Interactive things look interactive and do something: no decorative play icons.
- Accessible: keyboard focus, labels/aria, contrast in light and dark, touch targets ≥ 48 dp.
- Verify visually (mock + headless screenshots, emulator) before saying a UI change is done.

## Checklist before finishing a change

1. Which layer owns this? Is the rule in core once?
2. SOLID/DRY/YAGNI: anything duplicated, over-built or doing two jobs?
3. Memory: allocations or clones in hot paths, leaks, unbounded growth?
4. Variables: immutable, scoped, named with units, no magic numbers?
5. Stability: timeouts, cancellation, no panics, atomic writes, fallbacks, tests?
6. Design: setting on both platforms, skins/motion, accessibility, screenshots?
7. `make check`, `pnpm check` (apps/desktop), `./gradlew -q compileDebugKotlin -Phomefi.skipRust=true` (apps/android).
