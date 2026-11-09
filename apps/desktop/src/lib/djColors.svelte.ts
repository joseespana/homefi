// The colours the DJ canvases draw with, read from the CSS tokens (--dj-*), only when the theme,
// light / dark or skin changes (not per frame).

export interface WaveColors {
  deck: string;
  low: string;
  high: string;
  bg: string;
  beat: string;
  bar: string;
  playhead: string;
}

class ThemeWatch {
  /** Bumped by every theme / skin / light-dark change. */
  version = $state(0);
  constructor() {
    if (typeof document === "undefined") return;
    new MutationObserver(() => this.version++).observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme", "data-skin", "data-mode"] });
  }
}

export const themeWatch = new ThemeWatch();

/** The tokens as seen from `el` (a deck sets `--c` to its colour). */
export function readColors(el: Element): WaveColors {
  const css = getComputedStyle(el);
  const v = (name: string, fallback: string) => css.getPropertyValue(name).trim() || fallback;
  return {
    deck: v("--c", "#38bdf8"),
    low: v("--dj-band-low", "#f97316"),
    high: v("--dj-band-high", "#ffffff"),
    bg: v("--dj-wave-bg", "#0b1522"),
    beat: v("--dj-beat", "rgb(255 255 255 / .14)"),
    bar: v("--dj-bar", "rgb(255 255 255 / .3)"),
    playhead: v("--dj-playhead", "#ffffff"),
  };
}

/** Hot cue colours, by pad (the numbers carry the meaning too). */
export const HOT_CUE_COLORS = ["#f87171", "#fbbf24", "#4ade80", "#22d3ee", "#a78bfa", "#f472b6", "#fb923c", "#a3e635"] as const;

/** A canvas sized to its box in device pixels; returns the 2D context scaled to CSS pixels. */
export function fitCanvas(canvas: HTMLCanvasElement): { g: CanvasRenderingContext2D; w: number; h: number } | null {
  const r = canvas.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  const w = Math.max(1, Math.round(r.width));
  const h = Math.max(1, Math.round(r.height));
  if (canvas.width !== w * dpr || canvas.height !== h * dpr) {
    canvas.width = w * dpr;
    canvas.height = h * dpr;
  }
  const g = canvas.getContext("2d");
  if (!g) return null;
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  return { g, w, h };
}

const BYTES_PER_SLICE = 4;
const FULL = 255;

/** Draws slices `[from, to)` of an overview (4 bytes each: peak, low, mid, high) across
 * `x0..x0 + width`, centred on `h / 2`. `colored` off draws one band in the deck colour. */
export function drawSlices(g: CanvasRenderingContext2D, bytes: Uint8Array, from: number, to: number, x0: number, width: number, h: number, c: WaveColors, colored: boolean) {
  const count = to - from;
  if (count <= 0 || width <= 0) return;
  const total = bytes.length / BYTES_PER_SLICE;
  const columns = Math.max(1, Math.floor(width));
  const perColumn = count / columns;
  const step = width / columns;
  const mid = h / 2;
  const layers: [number, string, number][] = colored
    ? [
        [1, c.low, 0.8],
        [2, c.deck, 0.9],
        [3, c.high, 0.75],
      ]
    : [[0, c.deck, 0.9]];
  for (const [band, color, alpha] of layers) {
    g.fillStyle = color;
    g.globalAlpha = alpha;
    for (let col = 0; col < columns; col++) {
      const a = Math.floor(from + col * perColumn);
      const b = Math.max(a + 1, Math.floor(from + (col + 1) * perColumn));
      let top = 0;
      for (let i = Math.max(0, a); i < Math.min(total, b); i++) top = Math.max(top, bytes[i * BYTES_PER_SLICE + band]);
      if (!top) continue;
      const bar = (top / FULL) * h * (colored && band === 3 ? 0.7 : 1);
      g.fillRect(x0 + col * step, mid - bar / 2, Math.max(1, step - (step > 2 ? 1 : 0)), bar);
    }
  }
  g.globalAlpha = 1;
}

// One animation frame loop for every DJ canvas: it runs only while something is registered
// (a playing deck) and the window is visible (the browser pauses rAF when hidden).
const frameCallbacks = new Set<() => void>();
let frameId = 0;
function frame() {
  for (const f of frameCallbacks) f();
  frameId = frameCallbacks.size ? requestAnimationFrame(frame) : 0;
}
/** Calls `f` every frame until the returned function is called. */
export function onFrame(f: () => void): () => void {
  frameCallbacks.add(f);
  if (!frameId) frameId = requestAnimationFrame(frame);
  return () => {
    frameCallbacks.delete(f);
  };
}

/** "1 minute 12 of 4 minutes" (screen readers). */
export function spokenTime(ms: number, totalMs: number): string {
  const part = (v: number) => {
    const s = Math.max(0, Math.floor(v / 1000));
    const m = Math.floor(s / 60);
    return m ? `${m} minute${m === 1 ? "" : "s"} ${s % 60}` : `${s} seconds`;
  };
  return `${part(ms)} of ${part(totalMs)}`;
}
