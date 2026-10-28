<script lang="ts" module>
  export type VisualMode = "bars" | "scope" | "halo";
  const MODES: VisualMode[] = ["bars", "scope", "halo"];
  const LABELS: Record<VisualMode, string> = { bars: "Spectrum", scope: "Oscilloscope", halo: "Halo" };
</script>

<script lang="ts">
  import { visual } from "../lib/visual.svelte";

  /** `cycle` lets a click switch modes (Winamp style); the choice is remembered. */
  let { mode = $bindable("bars"), cycle = false, height = 140 }: { mode?: VisualMode; cycle?: boolean; height?: number } = $props();

  let canvas: HTMLCanvasElement;

  $effect(() => {
    if (cycle) {
      try {
        const saved = localStorage.getItem("homefi.visual") as VisualMode | null;
        if (saved && MODES.includes(saved)) mode = saved;
      } catch {}
    }
  });

  function next() {
    if (!cycle) return;
    mode = MODES[(MODES.indexOf(mode) + 1) % MODES.length];
    try {
      localStorage.setItem("homefi.visual", mode);
    } catch {}
  }

  type Colors = { accent: string; glow: string; peak: string; grid: string };

  $effect(() => {
    const ctx = canvas.getContext("2d")!;
    let raf = 0;
    let colors = { accent: "#5fd4ea", glow: "#0891b2", peak: "#ffffff", grid: "rgb(255 255 255 / 0.08)" };
    let lastColors = -Infinity;

    const draw = (t: number) => {
      raf = requestAnimationFrame(draw);
      const dpr = window.devicePixelRatio || 1;
      const w = canvas.clientWidth;
      const h = canvas.clientHeight;
      if (canvas.width !== w * dpr || canvas.height !== h * dpr) {
        canvas.width = w * dpr;
        canvas.height = h * dpr;
      }
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, w, h);
      if (t - lastColors > 500) {
        const css = getComputedStyle(canvas);
        colors = {
          accent: css.getPropertyValue("--room-accent").trim() || colors.accent,
          glow: css.getPropertyValue("--room-glow").trim() || colors.glow,
          peak: css.getPropertyValue("--viz-peak").trim() || colors.peak,
          grid: css.getPropertyValue("--viz-grid").trim() || colors.grid,
        };
        lastColors = t;
      }
      if (mode === "bars") bars(ctx, w, h, colors);
      else if (mode === "scope") scope(ctx, w, h, colors);
      else halo(ctx, w, h, colors);
    };
    raf = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(raf);
  });

  /** LED-segment bars with falling peak caps. */
  function bars(ctx: CanvasRenderingContext2D, w: number, h: number, c: Colors) {
    const n = visual.spectrum.length;
    const gap = 3;
    const bw = (w - gap * (n - 1)) / n;
    const seg = 5;
    const grad = ctx.createLinearGradient(0, h, 0, 0);
    grad.addColorStop(0, c.glow);
    grad.addColorStop(0.65, c.accent);
    grad.addColorStop(1, c.peak);
    ctx.fillStyle = grad;
    for (let i = 0; i < n; i++) {
      const x = i * (bw + gap);
      const v = visual.spectrum[i];
      const segments = Math.round((v * h) / seg);
      for (let s = 0; s < segments; s++) ctx.fillRect(x, h - (s + 1) * seg, bw, seg - 1.5);
      const peak = visual.peaks[i] ?? 0;
      ctx.fillStyle = c.peak;
      ctx.fillRect(x, h - peak * h - 3, bw, 2);
      ctx.fillStyle = grad;
    }
  }

  /** Glowing oscilloscope trace. */
  function scope(ctx: CanvasRenderingContext2D, w: number, h: number, c: Colors) {
    ctx.strokeStyle = c.grid;
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(0, h / 2);
    ctx.lineTo(w, h / 2);
    ctx.stroke();
    const wave = visual.wave;
    ctx.shadowColor = c.accent;
    ctx.shadowBlur = 14;
    ctx.strokeStyle = c.accent;
    ctx.lineWidth = 2.5;
    ctx.beginPath();
    wave.forEach((v, i) => {
      const x = (i / (wave.length - 1)) * w;
      const y = h / 2 - v * h * 0.45;
      i ? ctx.lineTo(x, y) : ctx.moveTo(x, y);
    });
    ctx.stroke();
    ctx.shadowBlur = 0;
  }

  /** Circular spectrum (MilkDrop-ish halo). */
  function halo(ctx: CanvasRenderingContext2D, w: number, h: number, c: Colors) {
    const n = visual.spectrum.length;
    const cx = w / 2;
    const cy = h / 2;
    const base = Math.min(w, h) * 0.37;
    const pulse = base * (1 + visual.energy * 0.12);
    ctx.lineCap = "round";
    ctx.shadowColor = c.accent;
    ctx.shadowBlur = 18;
    for (let i = 0; i < n * 2; i++) {
      const v = visual.spectrum[i < n ? i : n * 2 - 1 - i];
      const a = (i / (n * 2)) * Math.PI * 2 - Math.PI / 2;
      const len = 3 + v * base * 0.32;
      ctx.strokeStyle = i % 2 ? c.accent : c.glow;
      ctx.lineWidth = Math.max(2, ((Math.PI * 2 * pulse) / (n * 2)) * 0.55);
      ctx.beginPath();
      ctx.moveTo(cx + Math.cos(a) * pulse, cy + Math.sin(a) * pulse);
      ctx.lineTo(cx + Math.cos(a) * (pulse + len), cy + Math.sin(a) * (pulse + len));
      ctx.stroke();
    }
    ctx.shadowBlur = 0;
  }
</script>

<button class="visualizer" class:cycle onclick={next} style:height="{height}px" title={cycle ? `${LABELS[mode]} · click to switch` : undefined} aria-label="Visualizer: {LABELS[mode]}">
  <canvas bind:this={canvas}></canvas>
  {#if cycle}<span class="mode">{LABELS[mode]}</span>{/if}
</button>

<style>
  .visualizer {
    position: relative;
    display: block;
    width: 100%;
    padding: 0;
    cursor: default;
  }
  .visualizer.cycle {
    cursor: pointer;
  }
  canvas {
    width: 100%;
    height: 100%;
    display: block;
  }
  .mode {
    position: absolute;
    top: 8px;
    right: 10px;
    font: 700 10px ui-monospace, SFMono-Regular, Menlo, monospace;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--muted);
    opacity: 0;
    transition: opacity 0.2s;
  }
  .visualizer:hover .mode {
    opacity: 1;
  }
</style>
