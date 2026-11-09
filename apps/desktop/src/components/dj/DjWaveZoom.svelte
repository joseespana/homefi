<script lang="ts">
  // The zoomed, scrolling waveform around the playhead (centre line), with the beat grid (thin =
  // beat, thick = bar; hidden when the beat isn't trusted), the loop and the hot cues.
  // Drag: scrub while stopped, nudge while playing (the deck speeds up / slows down while you
  // pull). Wheel: zoom (2 to 32 beats). Double-click: back to the setting's zoom.
  import { dj } from "../../lib/dj.svelte";
  import { deckIndex, DJ_MIN_GRID_CONFIDENCE, DJ_ZOOM_RANGE, type DeckId } from "../../lib/dj-api";
  import { drawSlices, fitCanvas, HOT_CUE_COLORS, onFrame, readColors, themeWatch, type WaveColors } from "../../lib/djColors.svelte";

  let { deck }: { deck: DeckId } = $props();

  /** Beat length assumed without a grid (120 BPM), so the zoom still means something. */
  const FALLBACK_BEAT_MS = 500;
  const BEATS_PER_BAR = 4;
  /** A nudge stops this long after the pointer stops moving. */
  const NUDGE_IDLE_MS = 120;

  const i = $derived(deckIndex(deck));
  const s = $derived(dj.deck(deck));
  const ov = $derived(dj.overview[i]);

  let canvas = $state<HTMLCanvasElement>();
  let width = $state(0);
  let colors: WaveColors | null = null;
  let drag: { x: number; nudging: boolean } | null = null;
  let idle: ReturnType<typeof setTimeout> | undefined;

  const beatMs = () => {
    const t = s?.track;
    return t && t.beat_ms > 0 ? t.beat_ms : FALLBACK_BEAT_MS;
  };
  const windowMs = () => dj.zoom * beatMs();

  function draw() {
    const c = canvas;
    if (!c) return;
    const fit = fitCanvas(c);
    if (!fit) return;
    const { g, w, h } = fit;
    g.clearRect(0, 0, w, h);
    const o = ov;
    const d = s;
    if (!o || !d?.track) return;
    const col = (colors ??= readColors(c));
    const win = windowMs();
    const pos = dj.positionMs(deck);
    const start = pos - win / 2;
    const xOf = (ms: number) => ((ms - start) / win) * w;
    const sps = o.meta.slices_per_second;
    const total = o.bytes.length / 4;
    const from = (start / 1000) * sps;
    const to = ((start + win) / 1000) * sps;
    const a = Math.max(0, from);
    const b = Math.min(total, to);
    if (b > a) {
      const x0 = ((a - from) / (to - from)) * w;
      const x1 = ((b - from) / (to - from)) * w;
      drawSlices(g, o.bytes, a, b, x0, x1 - x0, h, col, dj.settings.waveform_colors);
    }
    // the loop
    if (d.loop_ms) {
      g.fillStyle = col.beat;
      g.fillRect(xOf(d.loop_ms[0]), 0, xOf(d.loop_ms[1]) - xOf(d.loop_ms[0]), h);
    }
    // the beat grid, only when it is trusted
    const t = d.track;
    if (dj.settings.show_beat_grid && t.has_grid && t.confidence >= DJ_MIN_GRID_CONFIDENCE && t.beat_ms > 0) {
      const first = Math.ceil((start - t.first_beat_ms) / t.beat_ms);
      const last = Math.floor((start + win - t.first_beat_ms) / t.beat_ms);
      for (let k = first; k <= last; k++) {
        const bar = ((k % BEATS_PER_BAR) + BEATS_PER_BAR) % BEATS_PER_BAR === 0;
        g.fillStyle = bar ? col.bar : col.beat;
        g.fillRect(Math.round(xOf(t.first_beat_ms + k * t.beat_ms)), 0, bar ? 2 : 1, h);
      }
    }
    // hot cue flags
    d.hot_cues.forEach((cue, n) => {
      if (cue === null || cue < start || cue > start + win) return;
      const x = xOf(cue);
      g.fillStyle = HOT_CUE_COLORS[n];
      g.fillRect(x, 0, 2, h);
      g.beginPath();
      g.moveTo(x, 0);
      g.lineTo(x + 9, 0);
      g.lineTo(x, 9);
      g.fill();
    });
  }

  // a new song, size, zoom, theme or setting: read the colours again and draw
  $effect(() => {
    themeWatch.version;
    width;
    ov;
    s;
    dj.zoom;
    dj.settings.waveform_colors;
    dj.settings.show_beat_grid;
    colors = null;
    draw();
  });
  // scrolling: every frame while playing
  $effect(() => {
    if (!s?.playing) return;
    return onFrame(draw);
  });

  function down(e: PointerEvent) {
    if (!s?.track || e.button !== 0) return;
    canvas!.setPointerCapture(e.pointerId);
    drag = { x: e.clientX, nudging: s.playing };
  }
  function move(e: PointerEvent) {
    if (!drag || !s?.track) return;
    const dx = e.clientX - drag.x;
    drag.x = e.clientX;
    if (!dx) return;
    if (drag.nudging) {
      // pulling the music left brings it sooner: faster
      dj.send({ type: "bend", deck, amount: dx < 0 ? 1 : -1 });
      clearTimeout(idle);
      idle = setTimeout(() => dj.send({ type: "bend", deck, amount: 0 }), NUDGE_IDLE_MS);
    } else {
      dj.send({ type: "nudge", deck, ms: (-dx / Math.max(1, width)) * windowMs() });
    }
  }
  function up() {
    if (drag?.nudging) {
      clearTimeout(idle);
      dj.send({ type: "bend", deck, amount: 0 });
    }
    drag = null;
  }
  function wheel(e: WheelEvent) {
    if (!s?.track) return;
    e.preventDefault();
    const next = e.deltaY > 0 ? dj.zoom * 2 : dj.zoom / 2;
    dj.zoomBeats = Math.min(DJ_ZOOM_RANGE.max, Math.max(DJ_ZOOM_RANGE.min, Math.round(next)));
  }
</script>

<div class="wz" bind:clientWidth={width} role="img" aria-label="Zoomed waveform, deck {deck.toUpperCase()}" title="Drag to nudge or scrub. Wheel zooms ({dj.zoom} beats).">
  <canvas
    bind:this={canvas}
    aria-hidden="true"
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
    onpointercancel={up}
    onwheel={wheel}
    ondblclick={() => (dj.zoomBeats = null)}
  ></canvas>
  <span class="ph" aria-hidden="true"></span>
  <span class="hint" aria-hidden="true">drag = scrub · wheel = zoom</span>
</div>

<style>
  .wz {
    position: relative;
    height: var(--dj-deck-h);
    background: var(--dj-wave-bg);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    overflow: hidden;
  }
  canvas {
    display: block;
    width: 100%;
    height: 100%;
    cursor: grab;
    touch-action: none;
  }
  canvas:active {
    cursor: grabbing;
  }
  .ph {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 50%;
    width: 2px;
    margin-left: -1px;
    background: var(--dj-playhead);
    box-shadow: 0 0 8px var(--c);
    pointer-events: none;
  }
  .hint {
    position: absolute;
    right: 8px;
    top: 6px;
    font-size: 11px;
    color: var(--faint);
    pointer-events: none;
  }
  @container deck (max-width: 439px) {
    .hint {
      display: none;
    }
  }
</style>
