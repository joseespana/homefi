---
name: homefi-design
description: "UI/UX design intelligence for HomeFi. Holds the HomeFi design system (brand gradient, Material 3 color roles, typography, icons, logo usage) and a searchable guideline database with a Jetpack Compose stack plus UX, color, typography, style and chart domains. Use when designing, building, reviewing or polishing any HomeFi UI: Android screens (Compose), the README/landing page, release notes graphics, or brand assets in assets/brand."
---

# UI/UX Pro Max for HomeFi

Two parts: **(1) the HomeFi design system**, which is the source of truth and is never overridden by search
results, and **(2) a BM25 search tool** over guideline CSVs for details and inspiration.

## 1. HomeFi design system

### Brand
- Mascot: a happy white house wearing navy headphones (mint `#34D399` cushions) and sound waves from the cups, round eyes with sparkles,
  pink cheeks, leaning −6° as if dancing, with yellow `#FDE68A` music notes escaping top-right.
  App icon = mascot on the gradient, corner radius ≈ 22.7% (`logo.svg`); single-color version with
  the face cut out (`mark.svg`, also the Android monochrome/notification icons). Wordmark: "Home" in
  `#0B1E3A` + "Fi" in `#0891B2` (`wordmark.svg`). The previous equalizer-house mark lives in `assets/brand/legacy/`.
- **Source of truth is `assets/brand/build_icon.py`**: it generates every SVG and the Android vector
  drawables from one geometry. Edit the script, run it, then `cargo xtask icons`. Don't hand-edit the outputs.
- Gradient (135°): blue `#1D4ED8` → teal `#0891B2` (55%) → green `#10B981`, with a soft white glow top-left. Compose: `Brand.gradient`.
- `cargo xtask icons` renders the PNG sizes, `homefi.ico` and the Play Store icon. Keep the evenodd face
  cut-outs so the monochrome icon works, and check the sheet down to 32 px.
- In the app, `HomefiLogo()` draws the colored adaptive layer on `Brand.gradient`. Use it for brand moments
  (top bar, login, empty states). Don't recolor the mascot, change its expression per screen, or put it on busy imagery.

### Color roles (`apps/android/.../ui/theme/Theme.kt`)

| Role | Light | Dark |
| --- | --- | --- |
| primary / onPrimary | `#0E7490` / white | `#67E3F5` / `#042B33` |
| primaryContainer | `#CFF4FA` | `#0E5566` |
| secondary | `#047857` | `#6EE7B7` |
| secondaryContainer (quality badges) | `#D1FAE5` | `#065F46` |
| tertiary | `#1D4ED8` | `#93C5FD` |
| background / surface | `#F7FBFC` | `#0A121D` (never pure black) |
| surfaceContainer / High | `#EDF4F6` / `#E3EDF0` | `#131E2E` / `#1B283B` |
| error | Material default | Material default |

Semantic usage in HomeFi: primary = main actions and "done" states, secondary = quality info
(Lossless / Hi-Res badges, "exists"), error = failures, onSurfaceVariant = metadata lines.
Dynamic color (Material You) is intentionally **off** to keep the brand.

### Typography
Material 3 default type scale with the system font (Roboto). Titles `titleMedium` + Bold for
section headers, `bodySmall` for metadata, monospace only for the login device code. Text in sp; let
it scale. No custom web fonts in the app. For the website/README a geometric sans (e.g. Inter)
is fine.

### Iconography
Material Icons **Rounded** (`Icons.Rounded.*`, `Icons.AutoMirrored.Rounded.*` for directional).
Kinds: Track `MusicNote`, Album `Album`, Artist `Person` (circle crop), Playlist/Mix `QueueMusic`,
Video `Videocam`; actions `Download`, `Search`, `Settings`, `Link`. No emoji icons.

### Components & tone
- Rows are `ListItem`s with 52 dp artwork (8 dp radius, circle for artists), title single-line ellipsized,
  quality/explicit as `QualityBadge`.
- Primary CTAs are full-width 52 dp buttons on onboarding/login; elsewhere tonal or icon buttons.
- Copy is short, friendly, second person; Spanish and English always both updated.
- The disclaimer "personal use, not affiliated with Tidal" stays visible in Settings and README.

## 2. Search tool

Python 3 only, no network. Scripts in `.claude/skills/homefi-design/scripts/`.

```bash
# Compose implementation guidelines (HomeFi's stack)
python3 .claude/skills/homefi-design/scripts/search.py "list keys performance" --stack jetpack-compose
python3 .claude/skills/homefi-design/scripts/search.py "accessibility touch target" --stack jetpack-compose -n 5

# Cross-platform UX rules, colors, typography, styles, charts
python3 .claude/skills/homefi-design/scripts/search.py "loading error empty state" --domain ux
python3 .claude/skills/homefi-design/scripts/search.py "dark mode contrast" --domain ux
python3 .claude/skills/homefi-design/scripts/search.py "music entertainment" --domain typography

# Inspiration for a NEW surface (landing page, store listing); then map it onto the HomeFi system
python3 .claude/skills/homefi-design/scripts/search.py "music app landing page" --design-system -p HomeFi -f markdown
```

Domains: `product`, `style`, `typography`, `color`, `landing`, `chart`, `ux`, `icons`, `react`, `web`, `prompt`.
Stacks: `jetpack-compose` (HomeFi app), `html-tailwind` (a future website), and the others shipped with the
skill (react, nextjs, vue, svelte, swiftui, react-native, flutter, shadcn, nuxtjs, nuxt-ui), which don't apply here.

**Conflicts:** when `--design-system` or a palette search suggests other colors or fonts, keep the
HomeFi tokens above. Use results for layout patterns, UX rules and anti-patterns only.
Web-only rules (`cursor-pointer`, hover states, viewport meta) don't apply to the Android app;
translate them (hover → pressed ripple, focus rings → TalkBack focus order).

## Workflow

1. Identify the surface: Android screen, README/landing, or brand asset.
2. Apply the design system above; search `--stack jetpack-compose` and `--domain ux` for the specific problem.
3. Implement (kotlin-specialist conventions for Compose code).
4. Verify:
   - `python3 .claude/skills/mobile-design/scripts/compose_audit.py` → PASS
   - light + dark screenshots on the emulator, font scale 1.3
   - contrast ≥ 4.5:1 for body text (onSurfaceVariant on surface included)

## Pre-delivery checklist (Android)

- [ ] Colors only from `MaterialTheme.colorScheme` / `Brand`; dark mode checked
- [ ] Icon buttons labelled for TalkBack; touch targets ≥ 48 dp
- [ ] Strings in `values/` and `values-es/`
- [ ] Loading, empty and error states present
- [ ] Lists keyed; images sized (thumbnails 320 px from the core)
- [ ] Logo/mark used unmodified
