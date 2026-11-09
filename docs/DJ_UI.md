# DJ mode: UI spec (desktop + Android simple)

Status: design for phase 4 (desktop) and phase 5 (Android), from `docs/DJ_PLAN.md`.
Visual reference: `docs/dj-mockup.html` (dark, classic, static; open it in Chrome at 1440 x 1000).

## 1. Rationale

- **For a casual party and a non-DJ.** The owner is not a DJ and uses only the mouse. So the screen has two
  layers: the **easy layer** (always in view, big) and the **pro layer** (all there, quieter, grouped).
  - Easy: load a song (drag it, or "A"/"B"), Play, Sync (on by default), **Mix to B / Mix to A** (one click, a
    beat-aligned automatic transition), sampler pads, master volume.
  - Pro: EQ + kill, filter, trim, hot cues, loops, tempo fader, pitch bend, crossfader curve.
- **Mouse first.** Every control is at least 40 x 40 px (transport and Mix 56 px high, sampler pads 60 px). Gestures are
  consistent: vertical drag moves a fader or knob, Shift = fine, double-click resets, wheel nudges. Nothing needs a
  keyboard; shortcuts show up only in hover hints.
- **Fun, not a cramped clone.** Rounded tokens (`--r-*`), one colour per deck (A cyan, B pink) used on everything of
  that deck (edge, letter, waveform, fader, VU), big friendly Mix button with the A to B gradient, pads that pulse.
- **Plain words.** Every control has a tooltip that says what it does in everyday language ("Bass", "Treble"), not
  jargon first. Labels keep the DJ names (High / Mid / Low) with the plain word in the tooltip.
- **Safe by default.** Auto-sync on, tempo range 8 %, mix length 16 beats, crossfader curve Smooth, load target "free deck".

## 2. Layout and breakpoints

View `dj` in the rail. Container `.dj` = CSS grid, rows `auto 1fr auto` (header, stage, lower), padding and gap
`--dj-gap` (12 px; 8 px under 1100 px).

The board lays itself out by **its own width** (CSS container queries on `.dj`, then on each deck, the mixer and the sampler),
not the window's. The content width is the window minus the sidebar and the padding (about 160 px).

| Board width | Stage | Lower row | Notes |
| --- | --- | --- | --- |
| >= 1030 | `1fr / clamp(250px,22vw,320px) / 1fr` | sampler `1fr` + crate (side by side on a wide window, a drawer under 1180 px of window) | zoom 96 to 112 px |
| 680 to 1029 | the two decks side by side, the **mixer under them** as a band (EQ, faders, Mix + crossfader in three columns) | sampler full width, 8 pads in a row (4 x 2 when the sampler is under 720 px) | the deck compacts under 440 px (labels hidden, 44 px cover) |
| < 680 (a 760 px window and up) | decks **stacked**, the mixer between them in its band | sampler wraps to 4 x 2 (2 x 4 under 360 px) | the board scrolls; every hit target keeps its size |

There is no "make the window wider" gate any more: DJ mode works down to Tauri's minimum window (800 px; 760 px works too).

Vertical: the stage fills the height; under 820 px tall the page scrolls (do not shrink hit targets). The two decks
and the mixer stretch to the same height. At 1440 x 1000 nothing scrolls.

Deck inner layout (left to right for A, mirrored for B so the tempo fader sits on the outer edge):
`main (1fr) | tempo strip (64 px)`. Main, top to bottom: tempo readout, zoomed waveform, overview, transport
(CUE / PLAY / SYNC, 56 px), hot cues (4 x 2, 48 px), loops row.

Mixer (middle column): title, **band rows** (Trim, High, Mid, Low, Filter; knob A | label | knob B, kill pills next to
the knobs), channel faders with VUs (A faders left, B right, VUs in the middle), **Mix to B** (56 px, pill, gradient)
+ length 8/16/32 + progress, crossfader (48 px hit height), curve segmented control. Order top to bottom in the
final build: Mix block first (easy layer), then the faders, EQ, crossfader (see open question 1).

Header: title, "DJ mode on: not bit-perfect" badge, spacer, master knob + master VU, Shortcuts, Crate toggle, Exit DJ.

## 3. Components (props and states)

All under `components/dj/`. Canvas components only draw; state comes from `lib/dj.svelte.ts`.

- `DjScreen` (layout, crate drawer, keyboard sheet host).
- `DjDeck { deck: "a"|"b" }`: states empty, loading, analysing, ready, playing, error.
- `DjWaveZoom`: canvas, beat lines (thin = beat, thick = bar), centre playhead, loop shading, hot-cue flags. Drag
  horizontally = scrub (while playing: nudge, resumes on release); wheel = zoom 2 s / 4 s / 8 s / 16 s (setting
  `wave_zoom`); double-click = re-centre.
- `DjWaveOverview`: canvas, bands coloured low = orange, mid = deck colour, high = white; markers: hot cues (3 px
  bars in their colour), loop region, playhead, cue point. Click = jump (hover shows a time tooltip and a ghost line).
- `DjTransport`: Cue (hold to preview, release returns), Play, Sync (toggle, ring when on).
- `DjHotCues`: 8 pads. Empty = dashed "+": click sets a cue here. Set = coloured, click jumps, Shift-click or right-click clears
  (tooltip says so), name editable by double-click. Group heading "Hot cues" in the pro layer.
- `DjLoops`: 1 / 2 / 4 / 8 beats (toggle; active one filled), In, Out, Exit. Disabled until the deck is ready.
- `DjTempo`: vertical fader (`--dj-hit` wide 44 px), centre line, readout `+1.2%`, `-`/`+` pitch bend (hold: bends while
  held, returns on release), `0` reset, range button cycles 8 / 16 / 50 %. Dragging while Sync is on turns Sync off
  (with a small "Sync off" toast; or keep it, see question 4).
- `DjKnob` (new, derived from `Knob.svelte`): `value -1..1` or `0..1`, `bipolar`, `label`, `onchange`, `detent` (reset
  value). Same gestures as `Knob.svelte`; add Shift = fine (/ 8), double-click = detent, aria-valuetext.
- `DjFader` (vertical or horizontal): same gestures; `fill` from the zero point; VU optional.
- `DjKill`: pill toggle, `aria-pressed`, red when on. A killed band also dims its knob.
- `DjVu`: segmented bar (green to 60 %, amber to 85 %, red), updated from the 20 Hz event, rendered by CSS height (no
  per-frame state churn).
- `DjMix`: **Mix to B / Mix to A** button (label follows which deck is audible), length segmented 8/16/32, progress
  bar while mixing; the button becomes **Stop mix** during the mix (click to freeze where it is). See 5.
- `DjCrossfader`: horizontal fader + `DjCurve` segmented Smooth / Linear / Cut (small curve glyph next to it).
- `DjSampler`: 8 `DjPad`s (icon, name, small key hint). Playing state: ring pulse 0.6 s (one animation; off when motion = off).
- `DjCrate`: search field, tabs Library / TIDAL / Queue, rows `cover, title/artist, BPM (from cache), A, B`. Rows are
  draggable: drop on a deck (deck shows a dashed highlight and "Drop to load on deck A"). "A"/"B" load on click. If
  the BPM is unknown the cell shows a dash and loads normally.
- `DjKeyboardSheet`: dialog (existing `.dialog` style) listing shortcuts, opened with the header button or `?`.

## 4. States (every deck component)

| State | What it shows |
| --- | --- |
| Empty (no song) | Both waveforms replaced by a dashed box "Drag a song here" and a secondary line "or press A / B on a song in the crate"; transport, loops, hot cues, Sync, Mix disabled (opacity .45, `aria-disabled`, tooltip "Load a song first"). Tempo fader and EQ stay live. |
| Loading (decode) | Cover skeleton + title skeleton (use `Skeleton.svelte`), progress line "Getting the song ready..." |
| Analysing | Waveform shows the overview dimmed 78 % with "Finding the beat..." and a sweeping 3 px line in the deck colour (static line when motion off). BPM shows `--.-`. Play works (plays without grid), Sync, loops, Mix disabled until done; tooltip "Waiting for the beat". |
| Ready | as the mock-up |
| Playing | Play button shows Pause, waveform scrolls, cover has a 2 px deck-colour ring |
| Active toggles | Sync ring; loop active = filled; Kill red |
| Error | Deck body replaced by an inline note (danger colour, icon, text, button "Try again" / "Choose another"): "This song could not be opened.", "Not enough memory to load this song. Try a shorter one or raise the limit in Settings.", "Could not find the beat. You can still play it; Sync is off for this song." |
| Hover | border becomes deck colour on buttons and fader caps; tooltip after 500 ms with the plain explanation and key |
| Focus | `outline: 2px solid var(--primary); outline-offset: 2px` (never removed); knobs/faders are `role="slider"` with arrows (Shift+arrows = big step), Home = reset, PageUp/PageDown |
| Dragging a song over | dashed deck-colour border, bg tint 8 % |

Mix block states: disabled when a deck is empty / analysing (tooltip "Load a song on both decks"); Mixing (progress, Stop mix);
error (a deck ended mid-mix: finishes the fade, toast "Deck B stopped, mix finished").

## 5. Beginner features

- **Sync on by default** (setting `auto_sync`, default on): when a song loads on the second deck and the first is
  playing, its tempo and beat are matched to it automatically. The deck shows "SYNCED" next to the tempo. Sync stays
  within the tempo range; if the songs are too far apart (more than the range, or a half/double BPM fits better) it
  uses half/double time and shows a note "Matched at half speed"; if impossible: "These songs are too different to
  match. They will play at their own speed." and Sync stays off.
- **Mix to B (A)**: one click. Starts deck B (if stopped) from its first beat or its hot cue 1, aligns beats, then over N
  beats (8 / 16 / 32, default 16) moves the crossfader from A to B along the curve, **swaps the bass** (A low goes to kill
  at the middle of the mix, B low comes in at the same moment, the "bass swap"), and at the end stops A and rewinds it
  ready for the next song. If a crate row is selected it loads first. Cancel by clicking **Stop mix** or touching the
  crossfader (manual takes over). Live progress bar; the button label flips to "Mix to A" when done.
- **Explained controls**: tooltips (below) and the "?" in the Shortcuts sheet. First time DJ opens, a one-time card
  (dismissable, setting `dj_tips`, on by default) of 3 steps: "1 Drag a song to A. 2 Drag another to B, press Play on A.
  3 Press Mix to B." with a "Got it" button.
- **Grouping**: Hot cues and Loops have a small heading and sit lower on the deck; the mixer shows Trim last and in
  muted colour. Option "Simple view" (setting `dj_simple`, default off; on = hides hot cues, loops, trim and the
  pitch-bend buttons; Mix, Sync, EQ, filter and crossfader stay).

## 6. Mouse gestures (precise)

| Target | Gesture | Result |
| --- | --- | --- |
| Knob / fader | pointer down + vertical drag (160 px = full travel; horizontal faders use horizontal drag) | change; pointer capture, cursor `ns-resize` / `ew-resize` |
| same | Shift + drag | x 0.1 (fine) |
| same | wheel | step 1/40 of range, Shift = 1/200; `preventDefault` only over the control |
| same | double-click | reset (knob to centre/0.8, tempo to 0, crossfader to centre, trim 0 dB) |
| same | click on a fader track | jump there (animated 120 ms; instant when motion = off) |
| Overview | click | jump; drag = scrub; hover = ghost line + time |
| Zoomed wave | drag left/right | scrub (paused) or nudge (playing, +/- 8 % bend while dragging); wheel = zoom; double-click re-centre |
| Pitch bend `-` / `+` | hold | temporary +/- 4 % while held |
| Cue | press and hold | plays from cue; release returns |
| Hot cue | click / Shift-click / right-click | jump or set / clear |
| Loop buttons | click | start loop of N beats at the playhead (snapped to the beat); click again = exit |
| Pad | click (press) | plays once; overlapping presses mix |
| Crate row | drag to a deck, or A / B buttons | load; double-click a row = load on the free deck |
| Mix | click | start mix; click again = stop |
| Master / Channel | wheel | volume |

Hover hints (tooltip text, with the optional key shown in brackets): see 9. Keys are rebindable
(`keymap.rs`); the sheet reads them from there. Default keys in the mock-up: Q/W/E and I/O/P transport, 1 to 8 pads,
Z / M crossfader nudge, C crate, `?` sheet.

## 7. Tokens and per-skin notes

New tokens (add to `app.css` `:root`, overridden in light and skins):

```
--dj-a: #38bdf8;  --dj-a-ink: #042b3f;      /* deck A colour, text on it  */
--dj-b: #f472b6;  --dj-b-ink: #3b0620;
--dj-wave-bg: #0b1522;  --dj-playhead: #fff;
--dj-beat: rgb(255 255 255 / .14);  --dj-bar: rgb(255 255 255 / .3);
--dj-hit: 48px;  --dj-gap: 12px;  --dj-deck-h: 96px;     /* zoom wave height */
--dj-band-low: #f97316;  --dj-band-high: #ffffff;       /* mid = deck colour */
```

- Light mode: `--dj-a: #0369a1`, `--dj-b: #be185d` (on-colour text white; contrast >= 4.5:1), wave bg `#e8f1f4`,
  playhead `#0b1e3a`, beat lines `rgb(11 30 58 / .14)`, band high `#0b1e3a`. Waveform stays readable because the canvas
  reads these through `getComputedStyle` on theme/skin change only.
- Surfaces use existing tokens only (`--surface`, `--surface-2/3`, `--border`, `--hw-top/bottom`, `--glass*`,
  `--primary`, `--warn`, `--danger`, `--ok`) and corners only `--r-xs/sm/md/lg/pill`; `--dj-*` never hardcode a corner.
- Skins (all from tokens, no per-skin component code except where noted):
  - **classic**: as the mock-up.
  - **immersive**: deck and mixer panels `--glass` with `--glass-border`; deck colours stay; waveform bg 40 % transparent.
  - **vinyl**: deck colours amber `#d97706` (A) and brick `#b45309`-> use `#be3a2d` (B); serif headings; paper grain from
    `.room-bg`; faders look like wood caps (`--hw-*`).
  - **studio**: always dark; `--r-*` capped at 4 px by the skin; mono numbers; LED green VUs, amber peak; A `#34d399`, B `#fbbf24`.
  - **retro_amp**: square, bevel borders from the skin's tokens, LCD green waveform (single colour, no band colours),
    A `#35c235`, B `#e8c000`, steps easing.
  - **luna / aluminum / web2000** (always light): A `#2c6fd6` / `#2f7fd9` / `#f08a00`, B `#2a9d3f` / `#8a8f98` / `#7ec11f`; glossy
    caps from `--hw-*`; Mix button uses the skin's primary-button look (gloss) instead of the gradient.
- The Mix button's gradient is `linear-gradient(90deg, var(--dj-a), var(--dj-b))`; skins with `--gradient: none`
  use `--primary`.
- Densities: if the skin sets a smaller `--r-max` it is respected; hit targets do not shrink.

## 8. Motion

Everything respects `data-motion` (full / subtle / off) and `--motion-speed`, `--ease-out`, `--ease-pop`.

- Waveform scrolling is time-driven from the 20 Hz event, interpolated with `requestAnimationFrame` only while the deck
  plays and the window is visible; no state per frame (the canvas reads a plain number).
- Pad playing ring: 0.6 s once per press (`motion: full`); in subtle a 120 ms colour flash; off none (colour stays while the sound plays).
- Analysing sweep: 1.4 s loop in full; static bar in subtle/off.
- Fader click-to-jump 120 ms; Mix progress is the real state, not an animation.
- Drop highlight 120 ms fade. No blur transitions anywhere; glass skins use their existing static blur.
- Reduced motion (system) maps to subtle through `lib/motion.ts`.

## 9. Accessibility

- Every control has an `aria-label` including the deck ("Tempo, deck A"). Sliders: `role="slider"`, `aria-valuemin/max/now`,
  `aria-valuetext` (+1.2 %, "-6 dB", "Left 30 %"), `aria-orientation` for horizontal faders.
- Toggles use `aria-pressed`; segmented groups `role="radiogroup"` / `radio`; waveforms `role="img"` with label
  "Waveform, deck A, 1 minute 12 of 4 minutes" and clicking works through a hidden `role="slider"` seek (keys arrows = 5 s, Shift = 1 beat).
- Focus order: header, deck A (header, waves, transport, hot cues, loops, tempo), mixer (Mix, bands top to bottom, faders, crossfader),
  deck B, sampler, crate. Tab never traps; Esc closes the drawer and the sheet.
- Contrast: deck-colour text on `--surface` >= 4.5:1 in dark and light (use `--dj-*-ink` on filled backgrounds);
  disabled controls still show a readable label (opacity .45 is only for fills, the text uses `--faint`).
- Colour is never the only cue: Sync, kill, loop active and pads set have shape / text changes; hot-cue pads carry numbers.
- Live regions (polite): "Deck A loaded: Midnight City", "Synced to deck A", "Mix started, 16 beats", "Mix finished".
- Hit targets: >= 40 px, main ones >= 48 px; no drag-only action: faders also take keys and the buttons `-` / `+`.

Tooltips (EN, plain words), plus ES in the Android section:

| Control | Tooltip |
| --- | --- |
| Play | Play or pause this deck [W] |
| Cue | Go back to the start point. Hold it to listen from there. [Q] |
| Sync | Match this song's speed and beat to the other deck |
| Mix to B | Fade from A to B smoothly, with the beats lined up |
| Mix length | How many beats the fade lasts. Longer is smoother. |
| Tempo | Speed of this song. Up = faster and higher pitch. Double-click to reset. |
| Pitch bend -/+ | Hold to speed up or slow down a little, to line up beats |
| Trim | Input volume of this song. Use it if one is louder than the other. |
| High / Mid / Low | Treble / voice / bass. Turn down what you don't want. |
| Kill | Silence this band completely |
| Filter | Left cuts the high end (muffled), right cuts the bass (thin). Centre is off. |
| Channel fader | Volume of this deck |
| Crossfader | Blends between deck A (left) and deck B (right) |
| Curve | How the crossfader blends: Smooth, Linear or Cut (sharp, for scratching) |
| Loop 1/2/4/8 | Repeat the next N beats |
| In / Out / Exit | Set the loop start, set the loop end, leave the loop |
| Hot cue | Jump to a saved spot. Empty: click to save the current spot. Shift-click to delete. |
| Pad | Plays a sound effect [key] |
| Crate A / B | Load this song on deck A / B |
| Master | Overall volume |

## 10. Strings (desktop, English)

Header: "DJ", "DJ mode on: not bit-perfect", badge tooltip "DJ mode mixes two songs, so audio is not bit-perfect. Turn it off to listen bit-perfect.", "Shortcuts", "Crate", "Exit DJ".
Deck: "Deck A", "Drag a song here", "or press A / B on a song in the crate", "Getting the song ready...", "Finding the beat...", "SYNCED", "Matched at half speed", "BPM", "Tempo", "Hot cues", "Loops", "Loop", "In", "Out", "Exit", "Cue", "Play", "Pause", "Sync".
Mixer: "Mixer", "Mix to A", "Mix to B", "Stop mix", "Mix length", "Trim", "High", "Mid", "Low", "Filter", "Kill", "Crossfader", "Curve", "Smooth", "Linear", "Cut", "Master".
Sampler: "Sampler", "Air horn", "Siren", "Laser", "Rewind", "Drop", "Scratch", "Riser", "Hit".
Crate: "Search your library or TIDAL", "Library", "TIDAL", "Queue", "Load on deck A", "Load on deck B", "Drop to load on deck A".
Errors: "This song could not be opened.", "Not enough memory to load this song. Try a shorter one or raise the limit in Settings.", "Could not find the beat. You can still play it; Sync is off for this song.", "These songs are too different to match. They will play at their own speed.", "Load a song on both decks to mix.", "Make the window wider to use DJ mode."
First-run card: "Your first mix", "1. Drag a song to deck A and press Play.", "2. Drag another to deck B.", "3. Press Mix to B.", "Got it".
Keyboard sheet: "Keyboard shortcuts", "All optional. You can do everything with the mouse.", "Change them in Settings".

Settings (new page "DJ", all with defaults from `DjSettings`): auto sync, mix length (8/16/32), bass swap on mix, tempo range
(8/16/50), crossfader curve, wave zoom, load target, latency (low/safe), memory limit, simple view, show tips, motion
follows the app. Every one has Android equivalent where the feature exists.

## 11. Android simple DJ (portrait, 360 to 412 dp wide; 720 x 1280 SR35 first)

Screen `ui/dj/DjScreen.kt`, Material 3, `LocalSkin`, motion and haptics settings (a light tick on pad press, confirm
on Mix start, none when haptics off). One scrolling column, safe areas respected, all targets >= 48 dp.

```
[ top bar: back / "DJ mode" / (i) not bit-perfect ]
[ Deck A card ]  cover 56dp | title, artist | BPM    waveform 56dp (tap overview = jump)
                 [ Cue ] [ Play ] [ Sync ]   (56dp)   Tempo slider (horizontal, -8..+8, reset 0) + key lock switch
                 Hot cues 4 pads | Loop [4] [8]
[ Mix to B   (56dp, full width, gradient) ]  length chips 8 | 16 | 32
[ Crossfader (horizontal, 56dp track)  ]   [ EQ & filter ] button -> bottom sheet
[ Deck B card ]  same as A
[ Sampler: 4 pads (2 x 2 or 4 x 1) ]
[ Volume A | Volume B (vertical-ish sliders next to each card, or in the EQ sheet) | Master ]
[ Crate: bottom sheet "Choose a song" (search, A / B buttons) ]
```

- EQ sheet: three rows High / Mid / Low per deck as vertical sliders with kill switches + Filter slider + Trim (in "More").
- Empty deck card: dashed area "Choose a song" (tap = crate sheet, preselected for that deck). Analysing: "Finding the beat..." with a linear indeterminate progress.
- Landscape: the two decks side by side with the mixer between (>= 600 dp wide); same components.
- Scroll never hides Mix: the Mix row and crossfader are in a sticky bottom bar (`bottomBar`) on phones shorter than 700 dp.
- Skins: deck A / B colours from the skin's tokens (see section 7); retro_amp and luna use their bevel faces on the buttons.

Android strings (EN / ES):

| key | EN | ES |
| --- | --- | --- |
| dj_title | DJ mode | Modo DJ |
| dj_badge | Not bit-perfect | No es bit-perfect |
| dj_choose | Choose a song | Elige una canción |
| dj_finding_beat | Finding the beat... | Buscando el ritmo... |
| dj_cue | Cue | Cue |
| dj_play / dj_pause | Play / Pause | Reproducir / Pausa |
| dj_sync | Sync | Sincronizar |
| dj_synced | Synced | Sincronizado |
| dj_mix_to_b | Mix to B | Mezclar a B |
| dj_mix_to_a | Mix to A | Mezclar a A |
| dj_stop_mix | Stop mix | Detener mezcla |
| dj_mix_length | Mix length (beats) | Duración de la mezcla (tiempos) |
| dj_tempo | Tempo | Tempo |
| dj_key_lock | Keep the pitch | Mantener el tono |
| dj_hot_cue | Hot cue | Punto guardado |
| dj_loop | Loop | Bucle |
| dj_eq | EQ and filter | Ecualizador y filtro |
| dj_high / dj_mid / dj_low | Treble / Voice / Bass | Agudos / Voces / Graves |
| dj_kill | Mute band | Silenciar banda |
| dj_filter | Filter | Filtro |
| dj_crossfader | Crossfader | Crossfader |
| dj_master | Master volume | Volumen general |
| dj_volume_deck | Volume, deck %1$s | Volumen, deck %1$s |
| dj_sampler | Sampler | Sampler |
| dj_pad_airhorn..hit | Air horn, Siren, Laser, Rewind, Drop, Scratch, Riser, Hit | Bocina, Sirena, Láser, Rebobinar, Caída, Scratch, Subida, Golpe |
| dj_both_decks | Load a song on both decks to mix. | Carga una canción en ambos decks para mezclar. |
| dj_err_open | This song could not be opened. | No se pudo abrir esta canción. |
| dj_err_memory | Not enough memory for this song. | No hay memoria suficiente para esta canción. |
| dj_err_beat | Could not find the beat. Sync is off. | No se encontró el ritmo. Sincronizar está desactivado. |
| dj_first_run | Drag... / Pick a song for A, another for B, then tap Mix to B. | Elige una canción para A, otra para B y toca Mezclar a B. |

## 12. Acceptance screenshots

Vite mock `?dj` in headless Chrome (add a fixture with both decks loaded, one analysing, one empty variant):
1. 1440 x 900 dark classic: both ready, A playing, mixing in progress.
2. 1440 x 900 light classic.
3. 1100 x 800 dark: crate as drawer closed and open.
4. 1000 x 700: minimum, no overlap, scrolls vertically.
5. Empty decks ("Drag a song here"), then drag-over highlight.
6. Analysing deck ("Finding the beat...") and error deck.
7. One screenshot per skin (immersive, vinyl, studio, retro_amp, luna, aluminum, web2000), dark where applicable.
8. Keyboard sheet; first-run card; focus ring on a knob and on Mix.
9. `data-motion="off"`: pad playing and analysing without animation.
10. Android 720 x 1280: both decks, empty deck, EQ sheet, crate sheet, ES strings, light and dark.
`docs/dj-mockup.html` is the dark classic reference for 1.

## 13. Open design questions

1. Mix block position: top of the mixer (easy layer first, as in section 2) or only between the decks under the crossfader (as
   in the mock-up, where it sits under the faders for the 1000 px height)?
2. Tempo range default 8 % with Shift for 16 %: fine for beginners, or a plain "Auto" with no visible fader in Simple view?
3. Should Simple view be the default the first time (hides hot cues, loops, trim), with "Show all controls" in the header?
4. Moving the tempo fader while Sync is on: turn Sync off (DJ behaviour) or ignore until Sync is turned off (safer for a beginner)?
5. Bass swap on Mix: always, or a setting only (default on)? Spec assumes a setting.
6. Sampler volume: one shared slider in the sampler strip, or ducking the decks while a pad plays (not in the MVP engine)?
7. Android: 4 pads fixed or user-pickable from the 8?
8. Colours for A / B in the vinyl, luna and aluminum skins need a quick look with a real waveform for contrast.

## Decisions on the open questions (2026-11-06)

José isn't a DJ and left these to us:

1. The Mix block sits under the channel faders, above the crossfader (as in the mock-up).
2. Simple view is the default on first run; "Show all controls" switches to the full view (a setting).
3. Moving the tempo of a synced deck keeps Sync on: the other deck follows (the pair keeps one tempo).
4. Bass swap during Mix is a setting, on by default.
5. Pads don't duck the decks in the MVP.
6. Android shows 4 pads; which 4 is a setting (picked from the 8).
7. A/B colours get a contrast check per skin during implementation (screenshots).
8. Simple view hides the tempo fader and pitch bend and relies on Sync; tempo range stays ±8 % by default.

## Homifito DJ (optional, a frontend setting)

Homifito, HomeFi's house with headphones, can DJ on the screen behind his own booth: a strip above (or below) the decks drawn in
SVG, with two turntables whose records turn with the real playheads, a mixer whose crossfader and VUs follow the snapshot, two
speakers, equalizer walls and a spotlight per deck. `DjHomifito.svelte` draws it; `lib/homifito-dj.ts` holds the maths
(tested in `tests/homifito-dj.test.ts`).

- **Setting** `homifito_dj`: `off` / `booth` / `big` (default `booth`), plus where he stands (`top` / `bottom`) and "Homifito
  colours" (the whole board in mint / pink / navy; light and dark variants). Set in Settings → DJ, in the layout editor, and on / off
  with the headphones button in the DJ top bar. It lives in `localStorage` (`homefi.dj.homifito`), because `DjSettings` in Rust has
  no field for it yet: to make it travel with setups, add `homifito_dj` (+ place, look) to `homefi_core::dj::DjSettings` and the
  Android settings. It is a strip, not a `DjSection`, so a Rust section kind is not needed.
- **Moves**: bobs and sways on the master beat (beat phase from the playing deck's playhead and its grid; free-running at 112 BPM
  without one), a hand on each deck, scratching when a paused deck's playhead is dragged, a deck is nudged, or the Scratch / Rewind
  pad fires; a hand follows the crossfader whenever it moves and through "Mix to A / B"; air horn, siren, laser, riser raise both
  hands; drop and hit make him jump; one headphone cup lifts off the ear while the other deck is cued.
- **Motion**: Look → Motion. Full is the show; Subtle is a gentle sway and no gestures (the crossfader hand still follows); Off is a
  still picture that redraws when a snapshot changes. Reduced motion resolves to Subtle through `data-motion`.
- **Mock**: `?hdj=off|booth|big`, `?hdjplace=bottom`, `?hdjlook`, `?dj=mixlive`, `?djscratch`, `?djpad=air_horn|drop|scratch|...`.

### Homifito DJ on Android

`ui/dj/HomifitoStage.kt` draws the same booth on a Compose Canvas with the app's mascot paths (`Art` in `Mascot.kt`);
`ui/dj/HomifitoChoreography.kt` holds the maths (a port of `lib/homifito-dj.ts`, tested in `HomifitoChoreographyTest`).
The setting is the core's `DjSettings.homifitoMode / homifitoPlace / homifitoLook` (Settings → DJ, the layout sheet and the
headphones-house button in the DJ top bar; the bar remembers the size it turns back on). "Homifito colours" is `LocalDjLook`
(deck colours, waveform well, playhead). Pad reactions come from `DjSession.padEvents`; a scratch is a paused playhead that moves.
Motion: Full / Subtle / Off follow the app's motion setting. Fit: hidden under 480 dp of height (phone on its side), Big shrinks to
the booth under 700 dp, and the strip stays pinned above or below the scrolling board from 560 dp of height.
Responsive board: under 700 dp high the Mix row is one line; from 600 dp wide the decks sit side by side with the mixer between;
from 840 dp wide the margins grow and the board stops at 1200 dp.
