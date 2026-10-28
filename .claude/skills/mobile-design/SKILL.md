---
name: mobile-design
description: Mobile design thinking for the HomeFi Android app (native Kotlin + Jetpack Compose, Material 3). Touch targets, thumb zone, loading/error states, navigation, typography, dark mode, accessibility (TalkBack), performance of Compose lists, plus a Compose audit script. Use before designing or reviewing any screen in apps/android.
allowed-tools: Read, Glob, Grep, Bash
---

# Mobile design for HomeFi (Android, Compose)

> Touch-first. Battery-conscious. Platform-respectful.
> Mobile is not a small desktop: design for one hand, bad network, interrupted attention.

## Decisions already made (don't ask again)

| Aspect | HomeFi |
| --- | --- |
| Platform | Android only, phone-first (minSdk 26, target 36). No iOS. |
| Framework | Native Kotlin + Jetpack Compose + Material 3. **Not** React Native / Flutter. |
| Navigation | Login gate → bottom `NavigationBar` with 3 tabs: Search, Downloads, Settings (`ui/HomefiRoot.kt`). Don't add more than 5 tabs; prefer sections inside a tab. |
| State | `StateFlow` in `AndroidViewModel`s / app-scoped repositories, read with `collectAsStateWithLifecycle()`. No Redux-style libraries. |
| Offline | Downloading needs network; downloaded files are plain audio files used by any music player. Show clear errors with retry. Don't build sync layers. |
| Brand | violet `#7C3AED` → indigo `#4F46E5` → sky `#0EA5E9` gradient, white house + equalizer mark (`ui/theme/Theme.kt`, `assets/brand/`). See the `ui-ux-pro-max` skill for the full design system. |
| Languages | English (default) + Spanish (`values-es`). |

Only ask the user when a request changes one of these.

## Audit script

```bash
python3 .claude/skills/mobile-design/scripts/compose_audit.py            # apps/android/app
```

Checks Kotlin/Compose sources and resources: icon buttons without content description,
hard-coded texts and colors, `fontSize` in dp, `items()` without keys, `collectAsState()`,
`runBlocking`/`GlobalScope`, credentials in logs, EN/ES string parity, backup/cleartext flags,
adaptive-icon monochrome layer. Errors exit with code 1. Run it before finishing UI work.
(`scripts/mobile_audit.py` is the original React Native/Flutter auditor and does not scan `.kt` files.)

## Reference files

They are framework-neutral in their principles. Where they show React Native or Flutter code,
apply the Compose equivalent from the table below.

| File | Read when |
| --- | --- |
| [mobile-design-thinking.md](mobile-design-thinking.md) | first, before any new screen |
| [touch-psychology.md](touch-psychology.md) | layouts, buttons, gestures |
| [platform-android.md](platform-android.md) | always: Material 3, type scale, components, TalkBack |
| [mobile-navigation.md](mobile-navigation.md) | tabs, back handling, deep links (Tidal share / VIEW intents) |
| [mobile-typography.md](mobile-typography.md) | text hierarchy, font scaling |
| [mobile-color-system.md](mobile-color-system.md) | dark mode, contrast, OLED |
| [mobile-performance.md](mobile-performance.md) | lists, recomposition, battery |
| [mobile-testing.md](mobile-testing.md) / [mobile-debugging.md](mobile-debugging.md) | emulator runs, logcat |
| `platform-ios.md`, `decision-trees.md` | not applicable (iOS / framework selection) |

| RN / Flutter in the docs | Compose in HomeFi |
| --- | --- |
| `FlatList` / `ListView.builder` | `LazyColumn` + `items(list, key = { it.id })` |
| `React.memo`, `const` widgets | stable/immutable params (`data class`, `List` from state), hoisted lambdas, `remember` |
| `useNativeDriver`, transform/opacity | `graphicsLayer { }`, `animate*AsState`; avoid animating layout size |
| `AsyncStorage` / `SecureStore` | app-private `filesDir` (Rust `auth.json`, 0600) / SharedPreferences for non-secrets |
| `NetInfo` | surface the engine error (`userMessage()`) with a retry action |
| haptics libs | `LocalHapticFeedback.current.performHapticFeedback(...)` for confirmations |
| `console.log` | no `Log.*` with user data; Rust side uses `tracing` |

## HomeFi anti-patterns

| Never | Instead |
| --- | --- |
| Icon-only button with `contentDescription = null` | describe the action (`stringResource(R.string.download)`) |
| Touch targets < 48 dp, or < 8 dp apart | `IconButton` (48 dp) / `ListItem` rows |
| Spinner forever, silent failures | determinate progress when bytes are known (`ActiveTask.total`), error rows in Downloads, retry |
| Hard-coded strings or colors in screens | `strings.xml` (EN + ES), `MaterialTheme.colorScheme` / `Brand` |
| Pure black backgrounds | dark surface `#0F0D1A` from the theme |
| Business logic in composables | ViewModel / `DownloadRepository` / Rust core |
| Long work in a ViewModel scope | `DownloadRepository` in `appScope` + `DownloadService` |
| Blocking the main thread (`runBlocking`, file IO) | `suspend` engine calls, `Dispatchers.IO` |
| Requesting permissions at launch without context | ask when the feature needs it (storage from Settings, notifications after login) |

## Checkpoint (fill before writing screen code)

```
Screen / flow:
Primary action & where the thumb reaches it:
Loading / empty / error states:
Strings added (EN + ES):
TalkBack labels for icon buttons:
Dark mode checked:
compose_audit.py result:
```

## Before finishing

- [ ] `python3 .claude/skills/mobile-design/scripts/compose_audit.py` → PASS
- [ ] `make android-debug` builds
- [ ] Checked on the emulator in light **and** dark mode, with font scale 1.3 (`adb shell settings put system font_scale 1.3`)
- [ ] Back gesture behaves (no dead ends, login gate not re-entered after logout)
