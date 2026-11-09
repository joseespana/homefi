<script lang="ts">
  // The ride's course, the one drawing of it (setup, preview, ride, summary): a night mountain on a canvas
  // (docs/MOUNTAIN.md). Height and ground colour say how hard each part is, the zone is a thin line on the
  // ridge and a ribbon at the base, and in the live ride a rider and a pin on the next part. The pixels come
  // from lib/course-render.ts; this component sizes the canvas, feeds it the scene and keeps the part
  // picking (hover, focus, keys) that the preview's table follows. It only draws: the numbers come in as props.
  import { app } from "../../lib/state.svelte";
  import { motionWatch } from "../../lib/motionLevel.svelte";
  import { exerciseSettingsOf, zoneName, type CourseLook, type Part, type ProfilePoint } from "../../lib/exercise-api";
  import { CourseCanvas, mmss, nextDifferent, plotPad, ridgeTop, type CourseModel, type Scene } from "../../lib/course-render";

  type Mode = "ghost" | "preview" | "live" | "summary";
  let {
    parts,
    profile,
    mode,
    height = 120,
    current = null,
    marker = null,
    label,
    describe,
    active = $bindable(null),
    onpick,
    look,
    summits,
  }: {
    parts: readonly Part[];
    profile: readonly ProfilePoint[];
    /** ghost: all dim (a sketch while building); preview/summary: all lit; live: a rider, a pin on the next part. */
    mode: Mode;
    height?: number | string;
    /** The part being ridden (live). */
    current?: number | null;
    /** Where the rider is (live): its time is the ride's elapsed time. */
    marker?: ProfilePoint | null;
    /** A text alternative for the whole drawing. */
    label: string;
    /** One line for a part: its tooltip and its name for assistive tech. Makes the parts hoverable. */
    describe?: (index: number) => string;
    /** The part under the pointer or the focus (the table's row follows it). */
    active?: number | null;
    onpick?: (index: number) => void;
    /** Overrides the rider's Course look setting. */
    look?: CourseLook;
    /** Overrides the rider's Course summit labels setting. */
    summits?: boolean;
  } = $props();

  const total = $derived(profile.length ? profile[profile.length - 1].t_s : parts.reduce((s, p) => s + p.seconds, 0));
  const flat = $derived(profile.length < 2 || total <= 0);
  const settings = $derived(exerciseSettingsOf(app.settings));
  const lookNow = $derived(look ?? settings.course_look);
  const summitsNow = $derived(summits ?? settings.course_summits);
  const model = $derived<CourseModel>({ profile, parts, total_s: total });

  let host: HTMLDivElement | undefined = $state();
  let canvas: HTMLCanvasElement | undefined = $state();
  let box = $state({ w: 0, h: 0 });
  $effect(() => {
    if (!host) return;
    const ro = new ResizeObserver(([e]) => {
      if (e) box = { w: Math.round(e.contentRect.width), h: Math.round(e.contentRect.height) };
    });
    ro.observe(host);
    return () => ro.disconnect();
  });

  let painter: CourseCanvas | null = null;
  $effect(() => {
    painter = canvas ? new CourseCanvas(canvas) : null;
    return () => (painter = null);
  });

  // the halos breathe only in full motion, and only in the live ride
  const animated = $derived(mode === "live" && motionWatch.level === "full" && !flat);

  function scene(clock: number | null): Scene {
    const base = { summits: summitsNow, clock, glow: active, pin: null, rider: false };
    if (mode === "ghost") return { ...base, ridden: total, glow: null };
    if (mode !== "live") return { ...base, ridden: null };
    const elapsed = marker?.t_s ?? 0;
    const now = current ?? 0;
    const ix = nextDifferent(parts, now);
    const next = ix >= 0 ? parts[ix] : undefined;
    const pin = next ? { index: ix, text: `${zoneName(next.kind)}  in ${mmss(next.start_s - elapsed)}` } : null;
    return { ...base, ridden: elapsed, rider: true, glow: now, pin };
  }

  function render(clock: number | null) {
    if (!painter || flat || box.w < 2 || box.h < 2) return;
    painter.prepare(model, lookNow, box.w, box.h);
    painter.draw(scene(clock));
  }

  // every prop that changes the picture lands here (the reads are the dependencies)
  $effect(() => {
    render(animated ? performance.now() / 1000 : null);
  });

  // the live ride's breathing: about 30 frames a second, only while it is on screen
  const FRAME_MS = 33;
  $effect(() => {
    if (!animated) return;
    let id = 0;
    let last = 0;
    const loop = (now: number) => {
      if (now - last >= FRAME_MS) {
        last = now;
        render(now / 1000);
      }
      id = requestAnimationFrame(loop);
    };
    id = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(id);
  });

  // hit areas and tooltips follow the plot's own horizontal map (run-outs to the card edges belong to the end parts)
  const edgeX = (t: number) => {
    const pad = plotPad(box.w, box.h);
    return pad + (total > 0 ? Math.min(1, Math.max(0, t / total)) : 0) * (box.w - 2 * pad);
  };
  const hits = $derived(
    parts.map((p, i) => {
      const left = i === 0 ? 0 : edgeX(p.start_s);
      const right = i === parts.length - 1 ? box.w : edgeX(p.start_s + p.seconds);
      return { left, width: Math.max(1, right - left), mid: (edgeX(p.start_s) + edgeX(p.start_s + p.seconds)) / 2 };
    }),
  );

  /** A tooltip this close to a side opens inward, so it stays inside the card. */
  const EDGE_PX = 90;
  let tip = $state<{ text: string; left: number; top: number } | null>(null);
  function setHover(i: number | null) {
    active = i;
    const p = i === null ? undefined : parts[i];
    const g = painter?.layout;
    if (i === null || !p || !g || !describe) {
      tip = null;
      return;
    }
    const x0 = edgeX(p.start_s);
    const x1 = edgeX(p.start_s + p.seconds);
    tip = { text: describe(i), left: (x0 + x1) / 2, top: ridgeTop(g, x0, x1) };
  }
  function key(e: KeyboardEvent, i: number) {
    const to = e.key === "ArrowRight" ? i + 1 : e.key === "ArrowLeft" ? i - 1 : e.key === "Home" ? 0 : e.key === "End" ? parts.length - 1 : null;
    if (to === null || to < 0 || to >= parts.length) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).parentElement?.querySelectorAll<HTMLElement>(".hit")[to]?.focus();
  }
</script>

<div class="mtn" class:mini={box.h > 0 && box.h < 72} bind:this={host} style:height={typeof height === "number" ? `${height}px` : height}>
  <div class="sky" role="img" aria-label={label}>
    <canvas bind:this={canvas} aria-hidden="true" hidden={flat}></canvas>
  </div>
  {#if flat}<span class="empty">Profile not available</span>{/if}

  {#if describe && !flat}
    <div class="hits" role="group" aria-label="Parts of the ride">
      {#each parts as _, i}
        <button
          type="button"
          class="hit"
          style:left="{hits[i]?.left ?? 0}px"
          style:width="{hits[i]?.width ?? 0}px"
          tabindex={i === (active ?? 0) ? 0 : -1}
          aria-label={describe(i)}
          onmouseenter={() => setHover(i)}
          onmouseleave={() => setHover(null)}
          onfocus={() => setHover(i)}
          onblur={() => setHover(null)}
          onclick={() => onpick?.(i)}
          onkeydown={(e) => key(e, i)}
        ></button>
      {/each}
    </div>
  {/if}

  {#if tip}<div class="tip" class:start={tip.left < EDGE_PX} class:end={tip.left > box.w - EDGE_PX} style:left="{tip.left}px" style:top="{tip.top}px">{tip.text}</div>{/if}
</div>

<style>
  .mtn {
    position: relative;
    width: 100%;
    min-height: 0;
  }
  /* a night scene in every theme (docs/MOUNTAIN.md): it keeps its dark glass in light mode too */
  .sky {
    position: absolute;
    inset: 0;
    overflow: hidden;
    border-radius: var(--r-md);
    background: #060a1a;
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.08);
  }
  .mini .sky {
    border-radius: var(--r-sm);
  }
  canvas {
    display: block;
    width: 100%;
    height: 100%;
  }
  .empty {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: var(--muted);
    font-size: 13px;
  }
  .hits {
    position: absolute;
    inset: 0;
    /* skins dress role=group boxes; this one is only a layer of hit areas */
    background: transparent !important;
    border: 0 !important;
    box-shadow: none !important;
  }
  .hit {
    position: absolute;
    top: 0;
    bottom: 0;
    border-radius: var(--r-xs);
    /* invisible hit areas: a skin that dresses every button must not paint them */
    background: transparent !important;
    border: 0 !important;
    box-shadow: none !important;
  }
  .hit:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: -2px;
  }
  .tip {
    position: absolute;
    transform: translate(-50%, calc(-100% - 8px));
    background: var(--surface-3);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
    border-radius: var(--r-sm);
    padding: 5px 10px;
    font-size: 12px;
    font-weight: 600;
    white-space: nowrap;
    pointer-events: none;
    z-index: 3;
  }
  .tip.start {
    transform: translate(-30%, calc(-100% - 8px));
  }
  .tip.end {
    transform: translate(-70%, calc(-100% - 8px));
  }
</style>
