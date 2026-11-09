# The course mountain (spec)

Reference implementation: `docs/mountain-prototype.html` (canvas 2D, ES5, runs in Chrome 113). The TV page
(`crates/homefi-core/src/exercise/tv/ride.html`), the Svelte `Mountain.svelte` and the Compose mountain follow this
document. Chosen direction: **A, "Ribbon"** (B "Alpine" and C "Stage" stay in the prototype as alternatives and as
the proposed values of the `Course look` setting, see "Settings").

## Why

The old chart painted each part as a block, so colour answered "which part" but nothing answered "how hard". Now
two questions are answered by two different channels:

- **Height and ground colour = effort.** One continuous terrain, cool at the foot (deep teal-blue), violet in the
  middle, rose and peach on the summits. Peaks read as peaks without any legend.
- **Zone = a thin, saturated line.** The ridge itself and a capsule ribbon at the base carry the part colour. Thin
  coloured lines on a calm ground stay vivid and never go muddy, which wide fills of olive, teal and green did.
- A faint wash of the zone colour (16 %) falls from the ridge down the slope so the eye links ridge and ribbon.

## Palette tokens

Zone colours are the app's (`COLORS` in `ride.html`, EXERCISE_UI section 6); do not change them here.

| Token | Value | Use |
|---|---|---|
| `zone.warm_up` | `#5eb1ff` | ridge, ribbon, wash |
| `zone.seated_flat` | `#3ddc97` | " |
| `zone.seated_climb` | `#ffc233` | " |
| `zone.standing_climb` | `#ff8a3d` | " |
| `zone.jumps` | `#b983ff` | " |
| `zone.sprint` | `#ff4d70` | " |
| `zone.recovery` | `#2fd6c4` | " |
| `zone.cool_down` | `#9aa8ff` | " |
| `ground.ramp` (by elevation e) | e0 `#0a2740`, .25 `#143f6e`, .5 `#35409a`, .75 `#74408f`, .92 `#b2527f`, 1 `#e58a66` | terrain fill, vertical gradient anchored to elevation, not to the card |
| `sky` (top to horizon) | `#060a1a`, .45 `#101847`, .78 `#38285f`, 1 `#7d4a78` | backdrop |
| `summit.glow` | `#ffb48a`, 34 % at centre | radial glow behind the highest summit |
| `far.1` / `far.2` | `rgb(86,96,160)` 42 % / `rgb(52,64,130)` 62 %, hazing to the horizon colour | far ranges |
| `snow` | `rgb(232,240,255)` lit side, `rgb(128,150,214)` shaded side, 70 % | above 80 % of the ride's elevation range |
| `ink` | `rgba(4,6,18,.55)` | 2.6 px under-line of the ridge (contrast on any ground) |
| `footer` | `rgba(7,10,20,.9)` | the tick strip |
| `tick.text` | `rgba(205,213,230,.55)` | time ticks |

Past (ridden) colours are derived, never separate tokens:
`dim(c) = mix(mix(luma(c), c, 0.25), [52,66,118], 0.52)`. It desaturates **toward slate blue**, not toward black or
grey, so warm zones do not turn brown.

## Layers (bottom to top)

1. Sky gradient, a few seeded stars (`W / 14`, radius 0.4 to 1.3 px, alpha .18 to .58).
2. Glow behind the highest summit.
3. Two far ranges: shaped from the same elevation data (`0.55 * e(shifted u) + 0.45 * fbm`) so they rhyme with the
   course; each has a lit 1 px edge; mist gradient (32 %) at the foot of each.
4. Near terrain, drawn **twice** into cached bitmaps: `dim` (past) and `lit` (to come):
   ground ramp, zone wash (alpha .16 at the ridge, 0 at the foot), slope lighting, snow, ridge, base ribbon, footer.
5. Per frame, on top: soft lit/dim edge at the rider, current-part glow, trail, summit labels, next pin, rider guide,
   rider, ticks.

Slope lighting: sun from the left. Per column `L = clamp(-slope * 1.1, -1, 1)` from the **smooth** ridge (not the
textured one, or stripes appear). `L > 0` paints warm white `rgba(255,244,230, .2 * L)` fading over 62 % of the column
height; `L < 0` paints `rgba(4,6,18, .34 * |L|)` the same way.

## Geometry and smoothing

- Inputs: `profile [{t_s, elevation 0..1}]`, `parts [{kind, start_s, seconds}]`, `elapsed_s`, `total_s`.
- Resample elevation to `M = clamp(round(W / 2), 160, 700)` samples (about 2 px apart).
- Smooth with a gaussian, `sigma = M * 0.0055` samples (clamped at the ends), then **stretch back to the data's own
  min and max** (the gaussian shaves the summits, and the peaks are the point).
- Rock texture: `+ (fbm(u * 60) - 0.5) * 2 * amp * (0.35 + e)` px with `amp = clamp(H * 0.011, 0, 3.6)`. The textured
  ridge is the real one: the rider and the pin sit on it.
- Vertical map: `y = yLo - e * (yLo - yHi)`; `yHi = 0.21 H` (0.2 H below 130 px), `yLo = floor - band - 0.045 H`,
  `floor = H - tick`, `tick = clamp(0.085 H, 11, 20)`, `band = clamp(0.05 H, 4, 12)`.
- Horizontal map: `x = padX + (t / total) * (W - 2 padX)`, flat run-outs to both card edges.
  `padX = clamp(0.12 H, 28, 40)` when the cyclist is shown (H >= 200), else `clamp(0.012 W, 8, 16)`.
- Far ranges sample the course mirrored (not wrapped) where `u * 0.85 + shift` passes 1, so they have no cliff.
- Zone colour per sample: the part's colour, blended with its neighbour by smoothstep within `0.6 %` of the ride
  on each side of a boundary (no seams in the ridge or the wash). The base ribbon uses hard edges.
- Columns snap to whole pixels (`floor(x)`), so strips never overlap or leave hairlines.
- Summits (optional labels): local maxima over a window of 6 % of the ride, prominence > 0.16, tallest first, at
  most three, at least 12 % of the ride apart.

## States

| State | Look |
|---|---|
| Past | `dim` bitmap (slate, 25 % of the zone colour left), ridge at 80 % alpha, ribbon capsules dimmed |
| Edge at the rider | the `lit` bitmap fades in over `clamp(0.02 W, 10, 34)` px, starting 25 % of that before the rider |
| Current part | slope glow in its colour (A 34 %, B/C 20 %, vertical fade), a 3x wide 28 % halo under its ridge stretch, the ridge brighter |
| Trail | white thread on the ridge behind the rider, 95 % at the rider fading to 0 over `min(x, 28 % W)` |
| Next different part | dashed ridge (`0.035 H` dash, `0.025 H` gap), halo that breathes between 25 and 45 %, a pin |
| Next pin | capsule in the next zone colour, text `#06080f` 700 weight, `fs = clamp(0.058 H, 10, 19)`, label "Standing climb  in 0:54" (phone, H < 130: without "in"). Stem to the ridge start, lifted above the highest ridge point under the label, never below 2 px from the top. No next part: no pin. |
| Rider (position marker, every size, live mode) | Still halo `clamp(0.13 H, 12, 36)` px radius (zone colour 70 % to 0). Hairline "you are here", 1 px, zone colour 55 %, from the ridge down to the ribbon. Dot ON the ridge at the elapsed position: radius `dr = clamp(0.045 H, 4, 13)`, zone colour fill, white core of `0.5 dr`, 1.5 px `ink` outline. **Beacon**: a ring in the zone colour expands from `dr` to `clamp(0.28 H, 14, 70)` once per second, alpha `0.85 (1 - p)^1.5`, stroke `max(1.5, 0.3 dr)`; the dot blinks once per second between 100 % and 55 % (`0.775 + 0.225 cos(2 pi p)`). H >= 200 (TV): the cyclist glyph (scale `clamp(0.064 H, 8, 21)`, tilted with the ridge, clamped 0.6 rad) sits just above the dot (base at `y - dr - 3`). Calm (motion off, reduced motion, paused): no blinking, steady full-brightness dot with a static double ring (radius `2 dr` at 55 %, `3.2 dr` at 28 %). |
| Summit labels (setting) | small white triangle and `m:ss` above the summit, past ones at 50 %; hidden under the pin or within 34 px of the rider, and when W < 500 |
| Empty / no profile | the card is not drawn (as today) |
| Done | `elapsed = total`: everything dim, no pin, rider at the end |

Ticks: `[60,120,300,600,900,1200,1800]` s, the smallest step giving at most 9 labels (4 when W < 450); first label
left at `padX`, last (total) right at `W - padX`; skip a tick that is within half a step of the end.

## Sizes

| Size | Canvas | Differences |
|---|---|---|
| TV | 1700 x 300 (drawn at a fixed 1700 design width, scaled to the real pixels) | position dot 13 px + cyclist above it, stars, summit labels allowed, pin 17 px; every text (pin, ticks, summit times) x 1.6 and the tick strip with it, because 14 px ticks are unreadable across a room |
| Desktop card | 700 x 160 | position dot 7 px + beacon, pin 10 px, summit labels allowed |
| Phone | 360 x 96 dp | position dot 4 dp + beacon, pin without "in", no summit labels, ticks at most 4 |

Draw at `devicePixelRatio` up to 2; the same code path scales (all sizes are fractions of H, with clamps).

## Motion

Only two things move: the rider beacon (ring + dot blink, 1 Hz, drawn in the overlay only, never in the cached bitmaps)
and the next-part halo, which breathes (25 to 45 %). Static redraw when the motion setting is "off",
`prefers-reduced-motion` or the ride is paused: steady dot with the static double ring, next-part halo at its middle value.
Cost: the three expensive bitmaps (backdrop, dim, lit) are rebuilt only when size, ride or look changes; a frame is
four `drawImage` calls plus the overlay, capped at about 30 fps and only while motion is on.
Compose: build the bitmaps (or `ImageBitmap`) in `remember(size, course, look)`; the overlay in `drawWithContent`.

## Settings (every new look is a setting, desktop and Android)

- `Course look`: Ribbon (default), Alpine, Stage. The three presets are three rows of one table (`ground`, `wash`,
  `ridge`, `snow`, `sky`, `glow`), so the code is one renderer.
- `Course summit labels`: off by default.
- Motion: follows the existing motion setting (`lib/motion.ts`, `ui/theme/Motion.kt`).
- Accessibility: the canvas has `role="img"` and an `aria-label` that says the ride, the current part, and elapsed over
  total; in Compose a `semantics { contentDescription }`. The info is also on the ribbon and the pin, never colour alone
  (the pin names the next part, the ticks give time). Known near pair: seated flat (green) and recovery (teal) are
  distinguishable by ridge position (flat vs descending) and by the pin text; if adjacent, keep the 2 px gap in the ribbon.

## Per-skin notes

The mountain is a night scene on purpose: it sits on the dark glass of the TV page, and on desktop and Android the
card keeps its dark surface in light mode too (as the exercise screens do). Skins may tint only `sky` and
`summit.glow`; zone colours and the ground ramp stay fixed so a zone means the same thing everywhere.
