<script lang="ts">
  // The whole song at a glance: low (orange), mid (deck colour) and high bands, hot cues, the loop,
  // the cue point and the playhead. Click jumps, drag scrubs, hover shows a ghost line and the time.
  // Keys: arrows = 5 s, Shift + arrows = one beat.
  import { formatTime } from "../../lib/api";
  import { dj } from "../../lib/dj.svelte";
  import { deckIndex, type DeckId } from "../../lib/dj-api";
  import { drawSlices, fitCanvas, HOT_CUE_COLORS, onFrame, readColors, spokenTime, themeWatch } from "../../lib/djColors.svelte";

  let { deck }: { deck: DeckId } = $props();

  const KEY_STEP_MS = 5000;
  const i = $derived(deckIndex(deck));
  const s = $derived(dj.deck(deck));
  const track = $derived(s?.track ?? null);
  const duration = $derived(track?.duration_ms ?? 0);
  const ov = $derived(dj.overview[i]);
  const colored = $derived(dj.settings.waveform_colors);

  let canvas = $state<HTMLCanvasElement>();
  let box = $state<HTMLElement>();
  let head = $state<HTMLElement>();
  let width = $state(0);
  let ghost = $state<{ x: number; ms: number } | null>(null);
  let scrubbing = false;

  // the song itself: redrawn only when it, the size or the theme changes
  $effect(() => {
    themeWatch.version;
    width;
    const o = ov;
    const c = canvas;
    if (!c) return;
    const fit = fitCanvas(c);
    if (!fit) return;
    const { g, w, h } = fit;
    g.clearRect(0, 0, w, h);
    if (!o) return;
    const n = o.bytes.length / 4;
    drawSlices(g, o.bytes, 0, n, 0, w, h, readColors(c), colored);
  });

  const pct = (ms: number) => (duration > 0 ? Math.min(100, Math.max(0, (ms / duration) * 100)) : 0);
  function place() {
    if (head) head.style.left = `${pct(dj.positionMs(deck))}%`;
  }
  // the playhead moves every frame while playing, from a plain number (no reactive state per frame)
  $effect(() => {
    place();
    if (!s?.playing) return;
    return onFrame(place);
  });

  function msAt(e: PointerEvent): number {
    const r = box!.getBoundingClientRect();
    return Math.min(duration, Math.max(0, ((e.clientX - r.left) / r.width) * duration));
  }
  function down(e: PointerEvent) {
    if (!track || e.button !== 0) return;
    box!.setPointerCapture(e.pointerId);
    scrubbing = true;
    dj.send({ type: "seek", deck, ms: msAt(e) });
  }
  function move(e: PointerEvent) {
    if (!track) return;
    const r = box!.getBoundingClientRect();
    ghost = { x: e.clientX - r.left, ms: msAt(e) };
    if (scrubbing) dj.send({ type: "seek", deck, ms: ghost.ms });
  }
  function key(e: KeyboardEvent) {
    if (!track) return;
    const beat = track.beat_ms > 0 ? track.beat_ms : 500;
    const step = e.shiftKey ? beat : KEY_STEP_MS;
    const now = dj.positionMs(deck);
    const by: Record<string, number> = { ArrowRight: step, ArrowUp: step, ArrowLeft: -step, ArrowDown: -step };
    if (e.key in by) dj.send({ type: "seek", deck, ms: Math.min(duration, Math.max(0, now + by[e.key])) });
    else if (e.key === "Home") dj.send({ type: "seek", deck, ms: 0 });
    else return;
    e.preventDefault();
    e.stopPropagation();
  }
</script>

<div
  class="wo"
  bind:this={box}
  bind:clientWidth={width}
  role="slider"
  tabindex={track ? 0 : -1}
  aria-label="Waveform, deck {deck.toUpperCase()}"
  aria-valuemin={0}
  aria-valuemax={Math.round(duration)}
  aria-valuenow={Math.round(s?.position_ms ?? 0)}
  aria-valuetext={spokenTime(s?.position_ms ?? 0, duration)}
  title="Click to jump, drag to scrub"
  onpointerdown={down}
  onpointermove={move}
  onpointerup={() => (scrubbing = false)}
  onpointercancel={() => (scrubbing = false)}
  onpointerleave={() => (ghost = null)}
  onkeydown={key}
>
  <canvas bind:this={canvas} aria-hidden="true"></canvas>
  {#if track && s}
    {#if s.loop_ms}<span class="loop" style:left="{pct(s.loop_ms[0])}%" style:width="{pct(s.loop_ms[1]) - pct(s.loop_ms[0])}%"></span>{/if}
    {#each s.hot_cues as cue, n (n)}
      {#if cue !== null}<span class="tick" style:left="{pct(cue)}%" style:background={HOT_CUE_COLORS[n]}></span>{/if}
    {/each}
    <span class="cue" style:left="{pct(s.cue_ms)}%" aria-hidden="true"></span>
    <span class="ph" bind:this={head} aria-hidden="true"></span>
    {#if ghost}
      <span class="ghost" style:left="{ghost.x}px" aria-hidden="true"></span>
      <span class="when" style:left="{ghost.x}px" aria-hidden="true">{formatTime(ghost.ms)}</span>
    {/if}
  {/if}
</div>

<style>
  .wo {
    position: relative;
    height: 48px;
    background: var(--dj-wave-bg);
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    overflow: hidden;
    cursor: pointer;
    touch-action: none;
  }
  .wo:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 2px;
  }
  canvas {
    display: block;
    width: 100%;
    height: 100%;
  }
  .ph,
  .ghost,
  .tick,
  .cue,
  .loop,
  .when {
    position: absolute;
    top: 0;
    bottom: 0;
    pointer-events: none;
  }
  .ph {
    width: 2px;
    margin-left: -1px;
    background: var(--dj-playhead);
    box-shadow: 0 0 8px var(--c);
  }
  .ghost {
    width: 1px;
    background: color-mix(in srgb, var(--dj-playhead) 55%, transparent);
  }
  .when {
    top: 2px;
    bottom: auto;
    transform: translateX(-50%);
    padding: 0 5px;
    border-radius: var(--r-xs);
    background: var(--surface);
    color: var(--text);
    font: 600 10px var(--font-mono);
  }
  .tick {
    width: 3px;
    border-radius: var(--r-xs);
  }
  .cue {
    top: auto;
    bottom: 0;
    width: 0;
    height: 0;
    margin-left: -5px;
    border: 5px solid transparent;
    border-bottom-color: var(--warn);
  }
  .loop {
    background: color-mix(in srgb, var(--dj-playhead) 16%, transparent);
    border-inline: 2px solid var(--c);
  }
</style>
