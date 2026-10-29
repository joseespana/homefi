<script lang="ts" module>
  import { api } from "../../lib/api";

  // one set of log-spaced points for every graph
  const POINTS = 240;
  let freqsPromise: Promise<number[]> | null = null;
  const logFreqs = () => (freqsPromise ??= api.eqLogFreqs(POINTS).catch(() => ((freqsPromise = null), [] as number[])));
</script>

<script lang="ts">
  // Frequency response of a profile: log 20 Hz – 20 kHz, ±range dB. Editable:
  // drag a handle for frequency / gain, wheel or pinch over it for Q (width),
  // double-click empty space for a new band, arrows / Delete on a focused handle.
  import { EQ_LIMITS, type EqProfile } from "../../lib/api";
  import { clamp, formatDb, formatHz, hasGain } from "../../lib/eq.svelte";

  let {
    profile,
    compare = null,
    curve = null,
    background = null,
    notes = null,
    gainLimits = EQ_LIMITS.gainDb,
    editable = false,
    selected = $bindable(null),
    labels = true,
    height = 280,
    range = 24,
    dim = false,
    onedit,
    onadd,
    onremove,
  }: {
    profile: EqProfile | null;
    /** Drawn dashed underneath ("before", or what plays combined). */
    compare?: EqProfile | null;
    /** Draws the main curve from this instead of `profile` (handles stay on `profile`'s bands). */
    curve?: EqProfile | null;
    /** A faint curve behind everything (the output's own profile under a taste layer). */
    background?: EqProfile | null;
    /** One line per band (the assistant's), shown on hover and with the selected band. */
    notes?: (string | undefined)[] | null;
    /** Gain range a drag stays in. */
    gainLimits?: readonly [number, number];
    editable?: boolean;
    selected?: number | null;
    /** Band numbers on the handles. */
    labels?: boolean;
    height?: number;
    range?: number;
    /** The equalizer is off or bypassed: the curve is drawn faded. */
    dim?: boolean;
    onedit?: (index: number, patch: { freqHz?: number; gainDb?: number; q?: number }) => void;
    onadd?: (freqHz: number, gainDb: number) => void;
    onremove?: (index: number) => void;
  } = $props();

  let width = $state(600);
  let svg = $state<SVGSVGElement | null>(null);
  let freqs = $state<number[]>([]);
  let main = $state<number[]>([]);
  let before = $state<number[]>([]);
  let back = $state<number[]>([]);
  let band = $state<number[]>([]);
  let dragging = $state<number | null>(null);

  $effect(() => {
    logFreqs().then((f) => (freqs = f));
  });

  // the curves come from the core's own math (one call per change; the newest wins)
  function latest(set: (v: number[]) => void) {
    let n = 0;
    return async (p: EqProfile | null) => {
      const my = ++n;
      if (!p || !freqs.length) return set([]);
      try {
        const v = await api.eqResponse(p, freqs);
        if (my === n) set(v);
      } catch {
        if (my === n) set([]);
      }
    };
  }
  const drawMain = latest((v) => (main = v));
  const drawBefore = latest((v) => (before = v));
  const drawBack = latest((v) => (back = v));
  const drawBand = latest((v) => (band = v));
  function plain<T>(v: T): T {
    return JSON.parse(JSON.stringify(v));
  }
  $effect(() => {
    const p = curve ? plain(curve) : profile ? plain(profile) : null;
    freqs;
    drawMain(p);
  });
  $effect(() => {
    const b = background ? plain(background) : null;
    freqs;
    drawBack(b);
  });
  $effect(() => {
    const c = compare ? plain(compare) : null;
    freqs;
    drawBefore(c);
  });
  $effect(() => {
    const b = selected !== null && profile ? profile.bands[selected] : null;
    const one = b && b.enabled && profile ? { ...plain(profile), bands: [plain(b)] } : null;
    freqs;
    drawBand(one);
  });

  $effect(() => {
    if (!svg) return;
    const ro = new ResizeObserver(([e]) => (width = Math.max(240, e.contentRect.width)));
    ro.observe(svg);
    return () => ro.disconnect();
  });

  const PAD = { l: 38, r: 10, t: 10, b: 22 };
  const LOG_SPAN = Math.log(20_000 / 20);
  const plotW = $derived(width - PAD.l - PAD.r);
  const plotH = $derived(height - PAD.t - PAD.b);
  const x = (f: number) => PAD.l + (Math.log(Math.max(20, f) / 20) / LOG_SPAN) * plotW;
  const y = (db: number) => PAD.t + ((range - Math.max(-range, Math.min(range, db))) / (2 * range)) * plotH;
  const fAt = (px: number) => 20 * Math.exp(((px - PAD.l) / plotW) * LOG_SPAN);
  const dbAt = (py: number) => range - ((py - PAD.t) / plotH) * 2 * range;

  const path = (values: number[]) => (values.length === freqs.length && values.length ? values.map((v, i) => `${i ? "L" : "M"}${x(freqs[i]).toFixed(1)},${y(v).toFixed(1)}`).join("") : "");
  const mainPath = $derived(path(main));
  const area = $derived(mainPath ? `${mainPath}L${x(20000)},${y(0)}L${x(20)},${y(0)}Z` : "");
  const beforePath = $derived(path(before));
  const backPath = $derived(path(back));
  const bandPath = $derived(path(band));
  const bandArea = $derived(bandPath ? `${bandPath}L${x(20000)},${y(0)}L${x(20)},${y(0)}Z` : "");

  const GRID_F = [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000];
  const MINOR_F = [30, 40, 60, 70, 80, 90, 300, 400, 600, 700, 800, 900, 3000, 4000, 6000, 7000, 8000, 9000];
  const gridDb = $derived(range > 12 ? [-24, -18, -12, -6, 0, 6, 12, 18, 24].filter((d) => Math.abs(d) <= range) : [-12, -9, -6, -3, 0, 3, 6, 9, 12].filter((d) => Math.abs(d) <= range));
  const labelDb = (d: number) => (range > 12 ? d % 12 === 0 : d % 6 === 0);
  const shortHz = (f: number) => (f >= 1000 ? `${f / 1000}k` : `${f}`);

  // where a band's handle sits: its gain, or on the curve for pass filters
  function handleY(i: number): number {
    const b = profile!.bands[i];
    if (hasGain(b.kind)) return y(b.gainDb);
    if (!main.length || !freqs.length) return y(0);
    let k = 0;
    while (k < freqs.length - 1 && freqs[k] < b.freqHz) k++;
    return y(main[k]);
  }

  function point(e: PointerEvent | WheelEvent | MouseEvent) {
    const r = svg!.getBoundingClientRect();
    return { px: ((e.clientX - r.left) / r.width) * width, py: ((e.clientY - r.top) / r.height) * height };
  }

  let frame = 0;
  let pendingMove: { i: number; px: number; py: number } | null = null;
  function down(e: PointerEvent, i: number) {
    if (!editable || e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    selected = i;
    dragging = i;
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    (e.currentTarget as SVGElement).focus();
  }
  function move(e: PointerEvent) {
    if (dragging === null) return;
    const { px, py } = point(e);
    pendingMove = { i: dragging, px, py };
    // one change per frame, however fast the pointer moves
    frame ||= requestAnimationFrame(() => {
      frame = 0;
      const m = pendingMove;
      if (!m || !profile) return;
      const b = profile.bands[m.i];
      const freqHz = Math.round(clamp(fAt(m.px), EQ_LIMITS.freqHz));
      const gainDb = Math.round(clamp(dbAt(m.py), [Math.max(-range, gainLimits[0]), Math.min(range, gainLimits[1])]) * 10) / 10;
      onedit?.(m.i, hasGain(b.kind) ? { freqHz, gainDb } : { freqHz });
    });
  }
  function up() {
    dragging = null;
  }

  const widen = (q: number, delta: number) => Math.round(clamp(q * Math.exp(delta), EQ_LIMITS.q) * 100) / 100;

  // wheel (and the trackpad pinch, which arrives as ctrl + wheel) changes Q
  $effect(() => {
    if (!svg || !editable) return;
    const el = svg;
    const wheel = (e: WheelEvent) => {
      const i = (e.target as Element).closest?.("[data-band]") ? Number((e.target as Element).closest("[data-band]")!.getAttribute("data-band")) : selected;
      if (i === null || !profile?.bands[i]) return;
      e.preventDefault();
      selected = i;
      const step = e.ctrlKey ? -e.deltaY * 0.02 : -e.deltaY * 0.0015;
      onedit?.(i, { q: widen(profile.bands[i].q, step) });
    };
    // Safari / WKWebView pinch
    let startQ = 1;
    const gStart = (e: Event) => {
      if (selected !== null && profile?.bands[selected]) {
        e.preventDefault();
        startQ = profile.bands[selected].q;
      }
    };
    const gChange = (e: Event) => {
      if (selected === null || !profile?.bands[selected]) return;
      e.preventDefault();
      onedit?.(selected, { q: widen(startQ, Math.log((e as Event & { scale: number }).scale || 1)) });
    };
    el.addEventListener("wheel", wheel, { passive: false });
    el.addEventListener("gesturestart", gStart);
    el.addEventListener("gesturechange", gChange);
    return () => {
      el.removeEventListener("wheel", wheel);
      el.removeEventListener("gesturestart", gStart);
      el.removeEventListener("gesturechange", gChange);
    };
  });

  function dbl(e: MouseEvent) {
    if (!editable || (e.target as Element).closest("[data-band]")) return;
    const { px, py } = point(e);
    if (px < PAD.l || px > width - PAD.r) return;
    onadd?.(Math.round(clamp(fAt(px), EQ_LIMITS.freqHz)), Math.round(clamp(dbAt(py), [-range, range]) * 10) / 10);
  }

  function key(e: KeyboardEvent, i: number) {
    const b = profile?.bands[i];
    if (!b) return;
    const big = e.shiftKey;
    const octave = big ? 1 / 3 : 1 / 24;
    let patch: { freqHz?: number; gainDb?: number; q?: number } | null = null;
    if (e.key === "ArrowRight") patch = { freqHz: Math.round(clamp(b.freqHz * 2 ** octave, EQ_LIMITS.freqHz)) };
    else if (e.key === "ArrowLeft") patch = { freqHz: Math.round(clamp(b.freqHz / 2 ** octave, EQ_LIMITS.freqHz)) };
    else if (e.key === "ArrowUp" && hasGain(b.kind)) patch = { gainDb: Math.round(clamp(b.gainDb + (big ? 3 : 0.5), EQ_LIMITS.gainDb) * 10) / 10 };
    else if (e.key === "ArrowDown" && hasGain(b.kind)) patch = { gainDb: Math.round(clamp(b.gainDb - (big ? 3 : 0.5), EQ_LIMITS.gainDb) * 10) / 10 };
    else if (e.key === "+" || e.key === "=") patch = { q: widen(b.q, 0.1) };
    else if (e.key === "-") patch = { q: widen(b.q, -0.1) };
    else if (e.key === "Delete" || e.key === "Backspace") {
      e.preventDefault();
      e.stopPropagation();
      onremove?.(i);
      return;
    }
    if (patch) {
      // the arrows belong to the handle here, not to seeking
      e.preventDefault();
      e.stopPropagation();
      onedit?.(i, patch);
    }
  }

  const tip = $derived.by(() => {
    const i = dragging ?? selected;
    const b = i !== null ? profile?.bands[i] : null;
    if (!b || i === null) return null;
    const hx = x(b.freqHz);
    return { i, hx, hy: handleY(i), text: `${formatHz(b.freqHz)}${hasGain(b.kind) ? ` · ${formatDb(b.gainDb)}` : ""} · Q ${b.q.toFixed(2)}`, note: notes?.[i] || "" };
  });
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<svg
  bind:this={svg}
  class="graph"
  class:editable
  class:dim
  viewBox="0 0 {width} {height}"
  style:height="{height}px"
  role="application"
  aria-label={editable ? "Frequency response. Drag a handle to change its frequency and gain, scroll over it to change its width. Double-click to add a band." : "Frequency response"}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
  ondblclick={dbl}
>
  <defs>
    <linearGradient id="eq-fill" x1="0" x2="0" y1="0" y2="1">
      <stop offset="0" stop-color="var(--eq-curve)" stop-opacity="0.32" />
      <stop offset="0.5" stop-color="var(--eq-curve)" stop-opacity="0.04" />
      <stop offset="1" stop-color="var(--eq-curve)" stop-opacity="0.32" />
    </linearGradient>
  </defs>
  <rect class="plot" x={PAD.l} y={PAD.t} width={plotW} height={plotH} rx="6" />
  {#each MINOR_F as f}<line class="minor" x1={x(f)} x2={x(f)} y1={PAD.t} y2={PAD.t + plotH} />{/each}
  {#each GRID_F as f}
    <line class="grid" x1={x(f)} x2={x(f)} y1={PAD.t} y2={PAD.t + plotH} />
    <text class="axis" x={x(f)} y={height - 6} text-anchor={f === 20 ? "start" : f === 20000 ? "end" : "middle"}>{shortHz(f)}</text>
  {/each}
  {#each gridDb as d}
    <line class="grid" class:zero={d === 0} x1={PAD.l} x2={PAD.l + plotW} y1={y(d)} y2={y(d)} />
    {#if labelDb(d)}<text class="axis" x={PAD.l - 6} y={y(d) + 3.5} text-anchor="end">{d > 0 ? `+${d}` : d}</text>{/if}
  {/each}

  {#if backPath}<path class="back" d={backPath} />{/if}
  {#if beforePath}<path class="before" d={beforePath} />{/if}
  {#if bandArea}<path class="band-area" d={bandArea} />{/if}
  {#if area}<path class="area" d={area} fill="url(#eq-fill)" />{/if}
  {#if mainPath}<path class="curve" d={mainPath} />{/if}

  {#if profile && editable}
    {#each profile.bands as b, i}
      <g
        class="handle"
        class:off={!b.enabled}
        class:selected={selected === i}
        class:pass={!hasGain(b.kind)}
        data-band={i}
        transform="translate({x(b.freqHz)},{handleY(i)})"
        tabindex="0"
        role="slider"
        aria-label="Band {i + 1}: {formatHz(b.freqHz)}{hasGain(b.kind) ? `, ${formatDb(b.gainDb)}` : ''}, Q {b.q.toFixed(2)}"
        aria-valuenow={hasGain(b.kind) ? b.gainDb : b.freqHz}
        onpointerdown={(e) => down(e, i)}
        onfocus={() => (selected = i)}
        onkeydown={(e) => key(e, i)}
      >
        {#if notes?.[i]}<title>{notes[i]}</title>{/if}
        <circle class="hit" r="16" />
        {#if hasGain(b.kind)}<circle class="dot" r={labels ? 9 : 7} />{:else}<rect class="dot" x={labels ? -8 : -6} y={labels ? -8 : -6} width={labels ? 16 : 12} height={labels ? 16 : 12} rx="3" />{/if}
        {#if labels}<text class="num" y="3.5" text-anchor="middle">{i + 1}</text>{/if}
      </g>
    {/each}
  {/if}

  {#if tip && editable}
    {@const right = tip.hx > width - (tip.note ? 330 : 150)}
    <g class="tip" transform="translate({right ? tip.hx - 14 : tip.hx + 14},{Math.max(PAD.t + 12, Math.min(PAD.t + plotH - 12, tip.hy - 18))})">
      <text text-anchor={right ? "end" : "start"}>{tip.text}</text>
      {#if tip.note}<text class="note" y="14" text-anchor={right ? "end" : "start"}>{tip.note.length > 64 ? `${tip.note.slice(0, 62)}…` : tip.note}</text>{/if}
    </g>
  {/if}
</svg>

<style>
  .graph {
    --eq-curve: var(--accent-text);
    display: block;
    width: 100%;
    touch-action: none;
    overflow: visible;
    font-family: var(--font-mono);
  }
  .plot {
    fill: color-mix(in srgb, var(--surface) 55%, transparent);
    stroke: var(--glass-border);
  }
  .grid {
    stroke: var(--viz-grid);
    stroke-width: 1;
  }
  .grid.zero {
    stroke: color-mix(in srgb, var(--text) 28%, transparent);
  }
  .minor {
    stroke: var(--viz-grid);
    stroke-opacity: 0.45;
  }
  .axis {
    font-size: 10px;
    fill: var(--faint);
  }
  .curve {
    fill: none;
    stroke: var(--eq-curve);
    stroke-width: 2.5;
    stroke-linejoin: round;
    filter: drop-shadow(0 0 6px color-mix(in srgb, var(--eq-curve) 45%, transparent));
    transition: opacity 0.2s;
  }
  .before {
    fill: none;
    stroke: var(--muted);
    stroke-width: 1.6;
    stroke-dasharray: 5 4;
    opacity: 0.8;
  }
  .back {
    fill: none;
    stroke: var(--muted);
    stroke-width: 1.4;
    opacity: 0.35;
  }
  .tip text.note {
    font-family: var(--font-sans, inherit);
    font-weight: 500;
    font-size: 10.5px;
    fill: var(--muted);
  }
  .band-area {
    fill: color-mix(in srgb, var(--primary) 16%, transparent);
    stroke: color-mix(in srgb, var(--primary) 50%, transparent);
    stroke-width: 1;
  }
  .dim .curve,
  .dim .area {
    opacity: 0.35;
  }
  .handle {
    cursor: grab;
    outline: none;
  }
  .handle:active {
    cursor: grabbing;
  }
  .hit {
    fill: transparent;
  }
  .dot {
    fill: var(--surface);
    stroke: var(--eq-curve);
    stroke-width: 2;
    transition: r 0.15s;
  }
  .num {
    font-size: 9.5px;
    font-weight: 700;
    fill: var(--text);
    pointer-events: none;
  }
  .handle.selected .dot,
  .handle:focus-visible .dot {
    fill: var(--eq-curve);
    stroke: var(--text);
  }
  .handle.selected .num,
  .handle:focus-visible .num {
    fill: var(--surface);
  }
  .handle.off .dot {
    stroke-dasharray: 3 2;
    opacity: 0.55;
  }
  .tip text {
    font-size: 11px;
    font-weight: 600;
    fill: var(--text);
    paint-order: stroke;
    stroke: var(--surface);
    stroke-width: 4px;
    stroke-linejoin: round;
    pointer-events: none;
  }
</style>
