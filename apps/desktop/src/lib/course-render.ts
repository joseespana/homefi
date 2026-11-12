// The course mountain's renderer (docs/MOUNTAIN.md, reference: docs/mountain-prototype.html). Canvas 2D, the
// one drawing of the ride's profile on the desktop: setup, preview, ride, summary and the saved-rides minis.
// The TV page (crates/homefi-core/src/exercise/tv/ride.html) and the Compose mountain follow the same spec.
//
// Height and ground colour say how hard a part is; the zone is a thin saturated line (the ridge) and a capsule
// ribbon at the base. The three expensive bitmaps (backdrop, ridden, to-ride) are rebuilt only when the size,
// the course or the look changes; a frame is a few drawImage calls plus the overlay (rider, pin, trail).
//
// Pure geometry (`layoutCourse`, `tickStep`, `summitsOf`...) is exported separately from the canvas class so it
// can be tested without a DOM.

export type CourseLook = "ribbon" | "alpine" | "stage";

export interface CoursePart {
  kind: string;
  start_s: number;
  seconds: number;
}
export interface CoursePoint {
  t_s: number;
  elevation: number;
}
export interface CourseModel {
  profile: readonly CoursePoint[];
  parts: readonly CoursePart[];
  total_s: number;
}

type Rgb = readonly [number, number, number];
type Stop = readonly [number, string];

// ------------------------------------------------------------------ palette (the spec's tokens)

/** The zones, fixed: the mountain is a night scene in every theme, and a zone means the same thing everywhere. */
export const ZONE_HEX: Readonly<Record<string, string>> = {
  warm_up: "#5eb1ff",
  seated_flat: "#3ddc97",
  seated_climb: "#ffc233",
  standing_climb: "#ff8a3d",
  jumps: "#b983ff",
  sprint: "#ff4d70",
  recovery: "#2fd6c4",
  cool_down: "#9aa8ff",
};
/** A part of an unknown kind, or a course whose parts are not stored (a saved ride's thumbnail). */
const NEUTRAL_HEX = "#b9c6e6";

interface LookPreset {
  ground: readonly Stop[];
  /** Alpha of the zone wash at the ridge and at the foot. */
  wash: readonly [number, number];
  /** The ridge is the zone colour (`zone`) or a lit rim over a glow (`light`). */
  ridge: "zone" | "light";
  snow: number;
  sky: readonly Stop[];
  glow: string;
  /** Alpha of the current part's slope glow. */
  currentGlow: number;
  /** How much white the current part's ridge gets. */
  currentRim: number;
}

const LOOKS: Readonly<Record<CourseLook, LookPreset>> = {
  ribbon: {
    ground: [[0, "#0a2740"], [0.25, "#143f6e"], [0.5, "#35409a"], [0.75, "#74408f"], [0.92, "#b2527f"], [1, "#e58a66"]],
    wash: [0.16, 0],
    ridge: "zone",
    snow: 0.7,
    sky: [[0, "#060a1a"], [0.45, "#101847"], [0.78, "#38285f"], [1, "#7d4a78"]],
    glow: "#ffb48a",
    currentGlow: 0.34,
    currentRim: 0.12,
  },
  alpine: {
    ground: [[0, "#0a1322"], [0.5, "#1c2b47"], [1, "#4a6288"]],
    wash: [0.24, 0],
    ridge: "light",
    snow: 1,
    sky: [[0, "#06111d"], [0.5, "#0f2c47"], [0.82, "#265673"], [1, "#6c97a6"]],
    glow: "#bfe6ff",
    currentGlow: 0.2,
    currentRim: 0.5,
  },
  stage: {
    ground: [[0, "#080e22"], [1, "#1c2754"]],
    wash: [0.6, 0.1],
    ridge: "light",
    snow: 0,
    sky: [[0, "#060a1a"], [0.5, "#111a44"], [0.8, "#2c2660"], [1, "#6a4678"]],
    glow: "#ff9fb0",
    currentGlow: 0.2,
    currentRim: 0.5,
  },
};

// ------------------------------------------------------------------ sizes and tuning (all fractions of H, clamped)

const MINI_H = 72; // under this height: no ribbon, ticks, pin or rider chrome
const PAD_TALL_H = 200; // from this height the plot keeps a wide margin at both ends
// the rider glyph and the resistance-step tag (one spec with the Android and TV renderers)
const RIDER_SCALE = 2.4; // glyph height = this x the position dot's radius
const RIDER_BOX = 24;
const RIDER_ANCHOR: readonly [number, number] = [5, 24]; // the rear wheel's contact point
const RIDER_TILT = 0.6;
const RIDER_PAUSED_ALPHA = 0.6;
const INK: Rgb = [4, 6, 18];
const INK_OUTLINE_ALPHA = 0.55;
const HOT_TIP: readonly [number, number] = [8, 14];
const HOT_WIDTH = 16;
const HOT_LIFT = 3;
const HOT_SCALE: readonly [number, number] = [1.25, 1.7];
const HOT_ACTIVE_BOOST = 1.15;
const HOT_FILL_ALPHA = 0.92;
const HOT_PASSED_ALPHA = 0.8;
const HOT_SUMMIT_GAP = 22;
/** The rider silhouette, verbatim from the visual spec: filled, NON-ZERO, facing right in a 24 x 24 box. */
const RIDER_D =
  "M0 19a5 5 0 1 0 10 0a5 5 0 1 0 -10 0ZM1.1 19a3.9 3.9 0 1 1 7.8 0a3.9 3.9 0 1 1 -7.8 0ZM14 19a5 5 0 1 0 10 0a5 5 0 1 0 -10 0ZM15.1 19a3.9 3.9 0 1 1 7.8 0a3.9 3.9 0 1 1 -7.8 0ZM5.5 19.24L9.5 10.84L8.5 10.36L4.5 18.76ZM4.98 19.55L10.98 19.75L11.02 18.65L5.02 18.45ZM8.46 10.72L10.46 19.32L11.54 19.08L9.54 10.48ZM9.02 11.2L16.62 11L16.58 9.8L8.98 10ZM11.55 19.55L17.15 10.75L16.05 10.05L10.45 18.85ZM16.02 10.56L18.42 19.16L19.58 18.84L17.18 10.24ZM7.6 10.1L11.4 10.1L11.4 8.9L7.6 8.9ZM10.32 9.42L15.32 5.92L13.88 3.88L8.88 7.38ZM14.01 5.28L17.01 9.98L18.19 9.22L15.19 4.52ZM8.83 9.11L13.23 13.91L14.77 12.49L10.37 7.69ZM13.18 12.96L11.38 19.16L13.02 19.64L14.82 13.44ZM17.06 10.78L18.06 9.58L17.14 8.82L16.14 10.02ZM8.2 8.4a1.4 1.4 0 1 0 2.8 0a1.4 1.4 0 1 0 -2.8 0ZM13 13.2a1 1 0 1 0 2 0a1 1 0 1 0 -2 0ZM13.3 4.9a1.3 1.3 0 1 0 2.6 0a1.3 1.3 0 1 0 -2.6 0ZM16.7 9.3a0.9 0.9 0 1 0 1.8 0a0.9 0.9 0 1 0 -1.8 0ZM14.8 2.5a2.1 2.1 0 1 0 4.2 0a2.1 2.1 0 1 0 -4.2 0Z";
/** The step tag: a rounded label pointing down at the ridge, tip at (8, 14). */
const HOT_D = "M3.5 0H12.5A3.5 3.5 0 0 1 16 3.5V7.5A3.5 3.5 0 0 1 12.5 11H10L8 14L6 11H3.5A3.5 3.5 0 0 1 0 7.5V3.5A3.5 3.5 0 0 1 3.5 0Z";
// built once, lazily: Path2D does not exist where the pure geometry is tested without a DOM
let riderPath: Path2D | null = null;
let hotPath: Path2D | null = null;
const TICK_STEPS_S = [60, 120, 300, 600, 900, 1200, 1800] as const;
const TAU = Math.PI * 2;
const HALF_PI = Math.PI / 2;
const SLATE: Rgb = [52, 66, 118];
const WHITE: Rgb = [255, 255, 255];
const FONT = '-apple-system, "Segoe UI", Roboto, sans-serif';
const SUMMIT_MAX = 3;
const SUMMIT_PROMINENCE = 0.16;
const BEACON_HZ = 1; // beacon ring + dot blink cycles per second
const BLINK_MID = 0.775; // the dot blinks between 100 % and 55 % brightness
const BLINK_AMP = 0.225;
const BREATH_NEXT_RAD_S = 2.4;
const MAX_DPR = 2;

const clamp = (v: number, a: number, b: number) => (v < a ? a : v > b ? b : v);
const smooth01 = (t: number) => {
  const u = clamp(t, 0, 1);
  return u * u * (3 - 2 * u);
};
const hex = (h: string): Rgb => [parseInt(h.slice(1, 3), 16), parseInt(h.slice(3, 5), 16), parseInt(h.slice(5, 7), 16)];
const mix = (a: Rgb, b: Rgb, t: number): Rgb => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
const css = (c: Rgb, a = 1) => `rgba(${Math.round(c[0])},${Math.round(c[1])},${Math.round(c[2])},${a})`;
export const mmss = (s: number) => {
  const n = Math.max(0, Math.round(s));
  return `${Math.floor(n / 60)}:${String(n % 60).padStart(2, "0")}`;
};
/** Past (ridden) colours are derived: desaturated toward slate blue, not toward black or grey. */
export function dimmed(c: Rgb): Rgb {
  const l = 0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2];
  return mix(mix([l, l, l], c, 0.25), SLATE, 0.52);
}
const zoneRgb = (kind: string): Rgb => hex(ZONE_HEX[kind] ?? NEUTRAL_HEX);

function rng(seed: number) {
  let s = seed;
  return () => {
    s |= 0;
    s = (s + 0x6d2b79f5) | 0;
    let t = Math.imul(s ^ (s >>> 15), 1 | s);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const hnoise = (n: number, seed: number) => {
  const r = Math.sin(n * 127.1 + seed * 311.7) * 43758.5453;
  return r - Math.floor(r);
};
function vnoise(x: number, seed: number) {
  const i = Math.floor(x);
  const f = smooth01(x - i);
  return hnoise(i, seed) * (1 - f) + hnoise(i + 1, seed) * f;
}
const fbm = (x: number, seed: number) => vnoise(x, seed) * 0.62 + vnoise(x * 2.3 + 11, seed + 3) * 0.28 + vnoise(x * 5.1 + 29, seed + 7) * 0.1;

// ------------------------------------------------------------------ geometry

export interface Layout {
  W: number;
  H: number;
  mini: boolean;
  tick: number;
  band: number;
  floor: number;
  yLo: number;
  yHi: number;
  R: number;
  padX: number;
  /** Samples of the smooth elevation (not counting the two run-outs). */
  M: number;
  emin: number;
  emax: number;
  /** x, textured ridge y, smooth ridge y, elevation, zone colour, light: with a run-out sample at each end. */
  xs: number[];
  ys: number[];
  ys0: number[];
  eh: number[];
  zc: Rgb[];
  L: number[];
  /** Indices (in the extended arrays) of the labelled summits and of the highest point. */
  summits: number[];
  peak: number;
}

export function elevationAt(points: readonly CoursePoint[], t: number): number {
  for (let i = 1; i < points.length; i++) {
    const c = points[i]!;
    if (t <= c.t_s) {
      const a = points[i - 1]!;
      const span = c.t_s - a.t_s;
      return span <= 0 ? c.elevation : a.elevation + ((c.elevation - a.elevation) * (t - a.t_s)) / span;
    }
  }
  return points.length ? points[points.length - 1]!.elevation : 0;
}
export function partIndexAt(parts: readonly CoursePart[], t: number): number {
  for (let i = parts.length - 1; i >= 0; i--) if (t >= parts[i]!.start_s) return i;
  return 0;
}
/** The first later part of a different kind than the current one: split parts of one kind are not "next". */
export function nextDifferent(parts: readonly CoursePart[], current: number): number {
  const kind = parts[current]?.kind;
  for (let i = current + 1; i < parts.length; i++) if (parts[i]!.kind !== kind) return i;
  return -1;
}

function gaussSmooth(raw: readonly number[], sigma: number): number[] {
  const n = raw.length;
  const r = Math.max(1, Math.ceil(sigma * 3));
  const k: number[] = [];
  for (let i = -r; i <= r; i++) k.push(Math.exp(-(i * i) / (2 * sigma * sigma)));
  const out: number[] = new Array(n);
  for (let i = 0; i < n; i++) {
    let acc = 0;
    let wsum = 0;
    for (let j = -r; j <= r; j++) {
      const w = k[j + r]!;
      acc += raw[clamp(i + j, 0, n - 1)]! * w;
      wsum += w;
    }
    out[i] = acc / wsum;
  }
  return out;
}

/** The zone colour at `t`: each boundary is a short smoothstep blend, so washes and ridges never show seams. */
function zoneColourAt(parts: readonly CoursePart[], t: number, total: number): Rgb {
  if (!parts.length) return zoneRgb("");
  const p = partIndexAt(parts, t);
  const part = parts[p]!;
  let c = zoneRgb(part.kind);
  const bw = total * 0.006;
  const ds = t - part.start_s;
  const de = part.start_s + part.seconds - t;
  if (ds < bw && p > 0) c = mix(zoneRgb(parts[p - 1]!.kind), c, 0.5 + 0.5 * smooth01(ds / bw));
  else if (de < bw && p < parts.length - 1) c = mix(zoneRgb(parts[p + 1]!.kind), c, 0.5 + 0.5 * smooth01(de / bw));
  return c;
}

/** Local maxima over a window of 6 % of the ride with enough prominence: tallest first, at most three, 12 % apart. */
export function summitsOf(e: readonly number[]): number[] {
  const M = e.length;
  const w = Math.round(M * 0.06);
  const cand: number[] = [];
  for (let i = w; i < M - w; i++) {
    let isMax = true;
    for (let j = -w; j <= w; j++) {
      if (e[i + j]! > e[i]!) {
        isMax = false;
        break;
      }
    }
    if (!isMax) continue;
    let minL = 9;
    let minR = 9;
    for (let j = 0; j < M * 0.3 && i - j >= 0; j++) minL = Math.min(minL, e[i - j]!);
    for (let j = 0; j < M * 0.3 && i + j < M; j++) minR = Math.min(minR, e[i + j]!);
    if (e[i]! - Math.max(minL, minR) > SUMMIT_PROMINENCE) cand.push(i);
  }
  cand.sort((a, b) => e[b]! - e[a]!);
  const out: number[] = [];
  for (const c of cand) {
    if (out.length >= SUMMIT_MAX) break;
    if (out.every((s) => Math.abs(s - c) >= M * 0.12)) out.push(c);
  }
  return out;
}

/** Ticks: the smallest step that gives at most 9 labels (4 on a narrow card). */
export function tickStep(total: number, W: number): number {
  const most = W < 450 ? 4 : 9;
  return TICK_STEPS_S.find((s) => total / s <= most) ?? TICK_STEPS_S[TICK_STEPS_S.length - 1]!;
}

/** Where the plot starts on the left (and ends on the right) of a card: room for the rider at both ends. */
export function plotPad(W: number, H: number): number {
  return H >= PAD_TALL_H ? clamp(H * 0.12, 28, 40) : clamp(W * 0.012, 8, 16);
}

export function layoutCourse(model: CourseModel, W: number, H: number): Layout {
  const mini = H < MINI_H;
  const total = Math.max(1, model.total_s);
  const tick = mini ? 0 : clamp(H * 0.085, 11, 20);
  const band = mini ? 0 : clamp(H * 0.05, 4, 12);
  const floor = H - tick;
  const yLo = floor - band - H * 0.045;
  const yHi = H * (mini ? 0.12 : H < 130 ? 0.2 : 0.21);
  const R = yLo - yHi;
  const padX = plotPad(W, H);
  const M = clamp(Math.round(W / 2), 160, 700);

  const raw: number[] = [];
  for (let i = 0; i < M; i++) raw.push(clamp(elevationAt(model.profile, (total * i) / (M - 1)), 0, 1));
  const sm = gaussSmooth(raw, M * 0.0055);
  // the gaussian shaves the summits, and the peaks are the point: stretch back to the data's own range
  const rlo = Math.min(...raw);
  const rhi = Math.max(...raw);
  const slo = Math.min(...sm);
  const shi = Math.max(...sm);
  const e = sm.map((v) => (shi > slo ? rlo + ((v - slo) / (shi - slo)) * (rhi - rlo) : v));

  const amp = clamp(H * 0.011, 0, 3.6);
  const xs: number[] = [];
  const ys: number[] = [];
  const ys0: number[] = [];
  const eh: number[] = [];
  const zc: Rgb[] = [];
  for (let i = 0; i < M; i++) {
    const u = i / (M - 1);
    const n = (fbm(u * 60, 5) - 0.5) * 2 * amp * (0.35 + e[i]!);
    xs.push(padX + u * (W - 2 * padX));
    ys0.push(yLo - e[i]! * R);
    ys.push(yLo - e[i]! * R + n);
    eh.push(e[i]!);
    zc.push(zoneColourAt(model.parts, total * u, total));
  }
  // flat run-outs to the card edges, so the terrain is edge to edge
  xs.unshift(0);
  ys0.unshift(ys0[0]!);
  ys.unshift(ys[0]!);
  zc.unshift(zc[0]!);
  eh.unshift(eh[0]!);
  xs.push(W);
  ys0.push(ys0[ys0.length - 1]!);
  ys.push(ys[ys.length - 1]!);
  zc.push(zc[zc.length - 1]!);
  eh.push(eh[eh.length - 1]!);

  // light from the left: a ridge rising to the right faces the sun
  const L: number[] = [];
  for (let i = 0; i < xs.length; i++) {
    const a = Math.max(0, i - 2);
    const b = Math.min(xs.length - 1, i + 2);
    const sl = (ys0[b]! - ys0[a]!) / Math.max(1, xs[b]! - xs[a]!);
    L.push(clamp(-sl * 1.1, -1, 1));
  }
  let peak = 0;
  for (let i = 1; i < M; i++) if (e[i]! > e[peak]!) peak = i;
  return { W, H, mini, tick, band, floor, yLo, yHi, R, padX, M, emin: rlo, emax: rhi, xs, ys, ys0, eh, zc, L, summits: summitsOf(e).map((i) => i + 1), peak: peak + 1 };
}

export const xOfT = (g: Layout, t: number, total: number) => g.padX + clamp(t / total, 0, 1) * (g.W - 2 * g.padX);

/** The textured ridge's y at `x` (linear between samples). */
export function ridgeY(g: Layout, x: number): number {
  const xs = g.xs;
  let lo = 0;
  let hi = xs.length - 1;
  const cx = clamp(x, 0, g.W);
  while (hi - lo > 1) {
    const mid = (lo + hi) >> 1;
    if (xs[mid]! <= cx) lo = mid;
    else hi = mid;
  }
  const span = xs[hi]! - xs[lo]!;
  return span <= 0 ? g.ys[lo]! : g.ys[lo]! + ((g.ys[hi]! - g.ys[lo]!) * (cx - xs[lo]!)) / span;
}

/** The highest (smallest y) ridge point between two instants: where a tooltip or pin sits above the stretch. */
export function ridgeTop(g: Layout, x0: number, x1: number): number {
  let top = Math.min(ridgeY(g, x0), ridgeY(g, x1));
  for (let i = 0; i < g.xs.length; i++) if (g.xs[i]! >= x0 && g.xs[i]! <= x1) top = Math.min(top, g.ys[i]!);
  return top;
}

// ------------------------------------------------------------------ the canvas

/** What changes from frame to frame (the course, the size and the look are set separately). */
export interface Scene {
  /** Seconds ridden: before it the terrain is dimmed. `null` = nothing ridden (all lit); `>= total` = all dim. */
  ridden: number | null;
  /** Draw the rider at `ridden`. */
  rider: boolean;
  /** The part whose slope glows (the current one, or the one under the pointer). */
  glow: number | null;
  /** The next different part: a dashed ridge and a pin with this text. */
  pin: { index: number; text: string } | null;
  summits: boolean;
  /** Resistance steps to mark on the ridge (the plan's hot points); none = no tags. */
  hot?: readonly HotPointLike[];
  /** The rider is the bike silhouette (default) or just the dot. */
  riderGlyph?: "bike" | "dot";
  /** The ride is paused: the bike dims. */
  paused?: boolean;
  /** Animation clock in seconds: the halos breathe. `null` = static, halos at their middle value. */
  clock: number | null;
}

/** A resistance step on the course (homefi_core::exercise::HotPoint, what the renderer needs of it). */
export interface HotPointLike {
  t_s: number;
}

interface Bitmap {
  c: HTMLCanvasElement;
  x: CanvasRenderingContext2D;
}

function bitmap(W: number, H: number, dpr: number): Bitmap {
  const c = document.createElement("canvas");
  c.width = Math.max(1, Math.round(W * dpr));
  c.height = Math.max(1, Math.round(H * dpr));
  const x = c.getContext("2d")!;
  x.scale(dpr, dpr);
  return { c, x };
}

export class CourseCanvas {
  private ctx: CanvasRenderingContext2D;
  private key = "";
  private model: CourseModel | null = null;
  private look: CourseLook = "ribbon";
  private W = 0;
  private H = 0;
  private dpr = 1;
  private g: Layout | null = null;
  private terrain: Path2D | null = null;
  private back: Bitmap | null = null;
  private dim: Bitmap | null = null;
  private lit: Bitmap | null = null;
  private tmp: Bitmap | null = null;

  private readonly canvas: HTMLCanvasElement;

  constructor(canvas: HTMLCanvasElement) {
    this.canvas = canvas;
    this.ctx = canvas.getContext("2d")!;
  }

  /** The layout of the last frame (for hit areas and tooltips); null before the first. */
  get layout(): Layout | null {
    return this.g;
  }

  /** Sizes the canvas (CSS px) and rebuilds the cached bitmaps if the size, the course or the look changed. */
  prepare(model: CourseModel, look: CourseLook, W: number, H: number) {
    const dpr = Math.min(window.devicePixelRatio || 1, MAX_DPR);
    const key = `${W}|${H}|${dpr}|${look}`;
    if (key === this.key && model.profile === this.model?.profile && model.parts === this.model.parts && model.total_s === this.model.total_s) return;
    this.key = key;
    this.model = model;
    this.look = look;
    this.W = W;
    this.H = H;
    this.dpr = dpr;
    this.canvas.width = Math.max(1, Math.round(W * dpr));
    this.canvas.height = Math.max(1, Math.round(H * dpr));
    const g = layoutCourse(model, W, H);
    this.g = g;
    const P = new Path2D();
    P.moveTo(0, g.floor);
    for (let i = 0; i < g.xs.length; i++) P.lineTo(g.xs[i]!, g.ys[i]!);
    P.lineTo(W, g.floor);
    P.closePath();
    this.terrain = P;
    this.back = this.paintBackdrop(g);
    this.dim = this.paintTerrain(g, model, true);
    this.lit = this.paintTerrain(g, model, false);
    this.tmp = bitmap(W, H, dpr);
  }

  private paintBackdrop(g: Layout): Bitmap {
    const bm = bitmap(g.W, g.H, this.dpr);
    const x = bm.x;
    const { W, H } = g;
    const d = LOOKS[this.look];
    const sky = x.createLinearGradient(0, 0, 0, g.floor);
    for (const [o, c] of d.sky) sky.addColorStop(o, c);
    x.fillStyle = sky;
    x.fillRect(0, 0, W, H);
    if (g.mini) return bm;
    const rnd = rng(7);
    const stars = Math.round(W / 14);
    for (let i = 0; i < stars; i++) {
      const sx = rnd() * W;
      const sy = rnd() * g.floor * 0.5;
      const sr = 0.4 + rnd() * (H > 200 ? 0.9 : 0.5);
      x.fillStyle = `rgba(255,255,255,${0.18 + rnd() * 0.4})`;
      x.beginPath();
      x.arc(sx, sy, sr, 0, TAU);
      x.fill();
    }
    // the glow behind the highest summit
    const px = g.xs[g.peak]!;
    const py = g.ys[g.peak]! + g.R * 0.15;
    const gr = Math.max(g.R * 1.1, W * 0.12);
    const glowRgb = hex(d.glow);
    const glow = x.createRadialGradient(px, py, 0, px, py, gr);
    glow.addColorStop(0, css(glowRgb, 0.34));
    glow.addColorStop(0.45, css(glowRgb, 0.1));
    glow.addColorStop(1, css(glowRgb, 0));
    x.fillStyle = glow;
    x.fillRect(0, 0, W, H);
    // two far ranges, fainter and hazier with distance, shaped from the same course so they rhyme with it
    const layers = [
      { top: 0.04, seed: 21, shift: 0.31, k: 0.55, c: [86, 96, 160] as Rgb, a: 0.42 },
      { top: 0.2, seed: 33, shift: 0.62, k: 0.5, c: [52, 64, 130] as Rgb, a: 0.62 },
    ];
    const haze = hex(d.sky[d.sky.length - 1]![1]);
    const e = g.eh.slice(1, g.M + 1);
    layers.forEach((l, li) => {
      const topY = g.yHi + l.top * g.R;
      x.beginPath();
      x.moveTo(0, g.floor);
      const steps = Math.round(W / 3);
      for (let i = 0; i <= steps; i++) {
        const u = i / steps;
        // mirrored where it would wrap, so the range has no cliff
        const wv = (u * 0.85 + l.shift) % 2;
        const pe = e[clamp(Math.round((wv > 1 ? 2 - wv : wv) * (g.M - 1)), 0, g.M - 1)]!;
        const val = clamp(l.k * pe + (1 - l.k) * fbm(u * 6 + l.shift * 9, l.seed) * 1.25, 0, 1);
        x.lineTo(u * W, g.floor - (0.28 + 0.72 * val) * (g.floor - topY));
      }
      x.lineTo(W, g.floor);
      x.closePath();
      const fg = x.createLinearGradient(0, topY, 0, g.floor);
      fg.addColorStop(0, css(l.c, l.a));
      fg.addColorStop(1, css(mix(l.c, haze, 0.7), l.a * 0.6));
      x.fillStyle = fg;
      x.fill();
      x.strokeStyle = css(mix(l.c, WHITE, 0.45), 0.16 + li * 0.06);
      x.lineWidth = 1;
      x.stroke();
      const mist = x.createLinearGradient(0, g.floor - g.R * 0.6, 0, g.floor);
      mist.addColorStop(0, css(haze, 0));
      mist.addColorStop(1, css(haze, 0.32));
      x.fillStyle = mist;
      x.fillRect(0, g.floor - g.R * 0.6, W, g.R * 0.6);
    });
    return bm;
  }

  /** The near terrain; `dim` is the "already ridden" version. */
  private paintTerrain(g: Layout, model: CourseModel, dim: boolean): Bitmap {
    const bm = bitmap(g.W, g.H, this.dpr);
    const x = bm.x;
    const d = LOOKS[this.look];
    const n = g.xs.length;
    const { W } = g;
    const C = (c: Rgb): Rgb => (dim ? dimmed(c) : c);
    // strips snap to whole device pixels, so neighbours never overlap (no hairline seams) or leave gaps
    const dpr = this.dpr;
    const colX = (i: number) => Math.floor(g.xs[i]! * dpr) / dpr;
    x.save();
    x.clip(this.terrain!);
    // 1) ground, by elevation: cool at the foot, warm on the summits (ribbon) or a calm slate
    const span = g.floor - g.yHi;
    const gr = x.createLinearGradient(0, g.yHi, 0, g.floor);
    for (let k = d.ground.length - 1; k >= 0; k--) {
      const stop = d.ground[k]!;
      const off = clamp((g.yLo - stop[0] * g.R - g.yHi) / span, 0, 1);
      gr.addColorStop(off, css(C(hex(stop[1]))));
    }
    gr.addColorStop(1, css(C(hex(d.ground[0]![1]))));
    x.fillStyle = gr;
    x.fillRect(0, 0, W, g.floor);
    // 2) the zone washes the slope, strongest at the ridge and fading to the foot
    if (d.wash[0] > 0) {
      for (let i = 0; i < n - 1; i++) {
        const x0 = colX(i);
        const x1 = colX(i + 1);
        if (x1 <= x0) continue;
        const yy = g.ys0[i]!;
        const col = C(g.zc[i]!);
        const wg = x.createLinearGradient(0, yy, 0, g.floor);
        wg.addColorStop(0, css(col, d.wash[0] * (dim ? 0.8 : 1)));
        wg.addColorStop(1, css(col, d.wash[1]));
        x.fillStyle = wg;
        x.fillRect(x0, yy - 5, x1 - x0, g.floor - yy + 5);
      }
    }
    // 3) sun from the left: slopes that rise to the right catch light, the others sink
    for (let i = 0; i < n - 1; i++) {
      const x0 = colX(i);
      const x1 = colX(i + 1);
      if (x1 <= x0) continue;
      const Lv = g.L[i]!;
      const yy = g.ys0[i]!;
      const depth = (g.floor - yy) * 0.62;
      const lg = x.createLinearGradient(0, yy, 0, yy + depth);
      if (Lv >= 0) {
        lg.addColorStop(0, `rgba(255,244,230,${0.2 * Lv * (dim ? 0.45 : 1)})`);
        lg.addColorStop(1, "rgba(255,244,230,0)");
      } else {
        lg.addColorStop(0, `rgba(4,6,18,${0.34 * -Lv})`);
        lg.addColorStop(1, "rgba(4,6,18,0)");
      }
      x.fillStyle = lg;
      x.fillRect(x0, yy - 5, x1 - x0, depth + 5);
    }
    // 4) snow on the high ground (above 80 % of the ride's own elevation range)
    if (d.snow > 0 && g.emax > 0.7) {
      const th = g.emin + (g.emax - g.emin) * 0.8;
      for (let i = 0; i < n - 1; i++) {
        const eh = g.eh[i]!;
        if (eh <= th) continue;
        const dp = ((eh - th) / (g.emax - th)) * g.R * 0.3 + (fbm(i * 0.06, 9) - 0.4) * g.R * 0.08;
        const x0 = colX(i);
        const x1 = colX(i + 1);
        if (dp < 1 || x1 <= x0) continue;
        const sg = x.createLinearGradient(0, g.ys0[i]!, 0, g.ys0[i]! + dp);
        const sc = mix([232, 240, 255], [128, 150, 214], clamp(0.5 - g.L[i]! * 0.5, 0, 1));
        const sa = d.snow * (dim ? 0.3 : 1);
        sg.addColorStop(0, css(sc, 0.95 * sa));
        sg.addColorStop(0.6, css(sc, 0.5 * sa));
        sg.addColorStop(1, css(sc, 0));
        x.fillStyle = sg;
        x.fillRect(x0, g.ys0[i]! - 5, x1 - x0, dp + 5);
      }
    }
    x.restore();
    // 5) the ridge: a dark under-line for contrast, then the zone colour (ribbon) or a lit rim (alpine, stage)
    x.lineJoin = "round";
    x.lineCap = "round";
    const rw = clamp(g.H * 0.016, 2, 5);
    x.beginPath();
    x.moveTo(g.xs[0]!, g.ys[0]!);
    for (let q = 1; q < n; q++) x.lineTo(g.xs[q]!, g.ys[q]!);
    x.strokeStyle = d.ridge === "zone" ? "rgba(4,6,18,0.55)" : "rgba(4,6,18,0.35)";
    x.lineWidth = rw + (d.ridge === "zone" ? 2.6 : 3);
    x.stroke();
    for (let i = 0; i < n - 1; i++) {
      const zc = C(g.zc[i]!);
      x.beginPath();
      x.moveTo(g.xs[i]!, g.ys[i]!);
      x.lineTo(g.xs[i + 1]!, g.ys[i + 1]!);
      if (d.ridge === "zone") {
        x.strokeStyle = css(zc, dim ? 0.8 : 1);
        x.lineWidth = rw;
      } else {
        x.strokeStyle = css(mix(zc, WHITE, dim ? 0.25 : 0.55), dim ? 0.8 : 0.95);
        x.lineWidth = Math.max(1.2, rw * 0.55);
      }
      x.stroke();
    }
    if (g.mini) return bm;
    // the tick strip is a calm footer, not sky
    x.fillStyle = "rgba(7,10,20,0.9)";
    x.fillRect(0, g.floor, W, g.H - g.floor);
    // 6) the base ribbon: one capsule per part, hard-edged so the zones stay countable
    const by = g.floor - g.band;
    const bh = g.band - 1;
    x.fillStyle = "rgba(255,255,255,0.05)";
    x.fillRect(0, by - 1, W, g.band + 1);
    for (const p of model.parts) {
      const bx0 = xOfT(g, p.start_s, model.total_s) + 1;
      const bx1 = xOfT(g, p.start_s + p.seconds, model.total_s) - 1;
      if (bx1 - bx0 < 1) continue;
      const r = Math.min(bh / 2, (bx1 - bx0) / 2);
      x.fillStyle = css(C(zoneRgb(p.kind)), dim ? 0.85 : 1);
      x.beginPath();
      x.moveTo(bx0 + r, by);
      x.lineTo(bx1 - r, by);
      x.arc(bx1 - r, by + bh / 2, bh / 2, -HALF_PI, HALF_PI);
      x.lineTo(bx0 + r, by + bh);
      x.arc(bx0 + r, by + bh / 2, bh / 2, HALF_PI, HALF_PI * 3);
      x.closePath();
      x.fill();
    }
    return bm;
  }

  /** Strokes the ridge between two x positions. */
  private stroke(g: Layout, xa: number, xb: number, style: string | CanvasGradient, width: number) {
    const x = this.ctx;
    let started = false;
    x.beginPath();
    for (let i = 0; i < g.xs.length; i++) {
      if (g.xs[i]! < xa - 1 || g.xs[i]! > xb + 1) continue;
      if (!started) {
        x.moveTo(xa, ridgeY(g, xa));
        started = true;
      }
      x.lineTo(g.xs[i]!, g.ys[i]!);
    }
    if (!started) return;
    x.lineTo(xb, ridgeY(g, xb));
    x.lineJoin = "round";
    x.lineCap = "round";
    x.strokeStyle = style;
    x.lineWidth = width;
    x.stroke();
  }

  /** One frame. `prepare` must have been called with the same course. */
  draw(scene: Scene) {
    const { g, model, back, dim, lit, tmp, terrain } = this;
    if (!g || !model || !back || !dim || !lit || !tmp || !terrain) return;
    const x = this.ctx;
    const { W, H, dpr } = this;
    const d = LOOKS[this.look];
    const total = Math.max(1, model.total_s);
    const parts = model.parts;
    const t = scene.clock ?? 0;
    const animated = scene.clock !== null;
    x.setTransform(dpr, 0, 0, dpr, 0, 0);
    x.clearRect(0, 0, W, H);
    x.drawImage(back.c, 0, 0, W, H);

    const ridden = scene.ridden === null ? 0 : clamp(scene.ridden, 0, total);
    const xr = xOfT(g, ridden, total);
    const yr = ridgeY(g, xr);
    const allDim = scene.ridden !== null && scene.ridden >= total;
    if (allDim) {
      x.drawImage(dim.c, 0, 0, W, H);
    } else if (scene.ridden === null || scene.ridden <= 0) {
      x.drawImage(lit.c, 0, 0, W, H);
    } else {
      x.drawImage(dim.c, 0, 0, W, H);
      // the lit (still to ride) terrain, with a soft edge at the rider
      const tx = tmp.x;
      const fade = clamp(W * 0.02, 10, 34);
      tx.setTransform(1, 0, 0, 1, 0, 0);
      tx.clearRect(0, 0, tmp.c.width, tmp.c.height);
      tx.drawImage(lit.c, 0, 0);
      tx.globalCompositeOperation = "destination-in";
      const mg = tx.createLinearGradient((xr - fade * 0.25) * dpr, 0, (xr + fade) * dpr, 0);
      mg.addColorStop(0, "rgba(0,0,0,0)");
      mg.addColorStop(1, "rgba(0,0,0,1)");
      tx.fillStyle = mg;
      tx.fillRect(0, 0, tmp.c.width, tmp.c.height);
      tx.globalCompositeOperation = "source-over";
      x.drawImage(tmp.c, 0, 0, W, H);
    }

    const rw = clamp(H * 0.016, 2, 5);
    const curIx = scene.rider ? partIndexAt(parts, ridden) : -1;
    const glowIx = scene.glow ?? (curIx >= 0 ? curIx : null);
    const glowPart = glowIx !== null ? parts[glowIx] : undefined;
    // the current part: its slope glows in its own colour, and its stretch of ridge is thicker
    if (glowPart) {
      const zc = zoneRgb(glowPart.kind);
      const px0 = xOfT(g, glowPart.start_s, total);
      const px1 = xOfT(g, glowPart.start_s + glowPart.seconds, total);
      x.save();
      x.clip(terrain);
      const cg = x.createLinearGradient(0, g.yHi, 0, g.floor);
      cg.addColorStop(0, css(zc, d.currentGlow));
      cg.addColorStop(1, css(zc, 0));
      x.fillStyle = cg;
      x.fillRect(px0, 0, px1 - px0, g.floor);
      x.restore();
      this.stroke(g, px0, px1, css(zc, 0.28), rw * 3);
      this.stroke(g, px0, px1, css(mix(zc, WHITE, d.currentRim), 1), d.ridge === "zone" ? rw : Math.max(1.4, rw * 0.7));
    }

    // the trail: a bright thread along the ridge that fades out behind the rider
    const trailW = Math.min(xr, W * 0.28);
    if (scene.rider && trailW > 4) {
      const tg = x.createLinearGradient(xr - trailW, 0, xr, 0);
      tg.addColorStop(0, "rgba(255,255,255,0)");
      tg.addColorStop(1, "rgba(255,255,255,0.95)");
      this.stroke(g, xr - trailW, xr, tg, Math.max(1.4, H * 0.009));
    }

    // the next different part: dashed bright ridge over it, a pin with the name and the countdown
    let pinBox: [number, number, number, number] | null = null;
    const np = scene.pin ? parts[scene.pin.index] : undefined;
    if (scene.pin && np && !g.mini) {
      const nc = zoneRgb(np.kind);
      const nx0 = xOfT(g, np.start_s, total);
      const nx1 = xOfT(g, np.start_s + np.seconds, total);
      const pulse = animated ? 0.5 + 0.5 * Math.sin(t * BREATH_NEXT_RAD_S) : 0.6;
      x.setLineDash([clamp(H * 0.035, 5, 10), clamp(H * 0.025, 4, 7)]);
      this.stroke(g, nx0, nx1, css(nc, 0.25 + 0.2 * pulse), rw * 3);
      this.stroke(g, nx0, nx1, css(mix(nc, WHITE, 0.2), 1), Math.max(1.6, rw * 0.8));
      x.setLineDash([]);
      const fs = clamp(H * 0.058, 10, 19);
      const ph = fs * 1.7;
      const stem = clamp(H * 0.05, 5, 14);
      x.font = `700 ${fs}px ${FONT}`;
      x.textAlign = "left";
      x.textBaseline = "middle";
      const pw = x.measureText(scene.pin.text).width + fs * 1.5;
      const ry = ridgeY(g, nx0);
      const pxl = clamp(nx0 - fs * 0.75, 3, Math.max(3, W - pw - 3));
      // lift the pin above every ridge point under it, so it never hides the climb it names
      let hiY = ry;
      for (let qx = pxl; qx <= pxl + pw; qx += 4) hiY = Math.min(hiY, ridgeY(g, qx));
      const lifted = hiY - stem * 0.6 - ph;
      const py0 = lifted >= 2 ? lifted : 2;
      x.strokeStyle = css(nc, 0.9);
      x.lineWidth = 1.4;
      x.beginPath();
      x.moveTo(nx0, ry);
      x.lineTo(nx0, py0 + ph);
      x.stroke();
      x.shadowColor = css(nc, 0.55);
      x.shadowBlur = fs * 0.7;
      x.fillStyle = css(nc);
      x.beginPath();
      const rr = ph / 2;
      x.moveTo(pxl + rr, py0);
      x.lineTo(pxl + pw - rr, py0);
      x.arc(pxl + pw - rr, py0 + rr, rr, -HALF_PI, HALF_PI);
      x.lineTo(pxl + rr, py0 + ph);
      x.arc(pxl + rr, py0 + rr, rr, HALF_PI, HALF_PI * 3);
      x.closePath();
      x.fill();
      x.shadowBlur = 0;
      pinBox = [pxl, py0, pw, ph];
      x.fillStyle = "#06080f";
      x.fillText(scene.pin.text, pxl + fs * 0.75, py0 + ph / 2 + 0.5);
      x.fillStyle = css(nc);
      x.beginPath();
      x.arc(nx0, ry, Math.max(2.2, rw * 0.9), 0, TAU);
      x.fill();
    }

    // resistance steps: a tag on the ridge at each (the last one passed glows); summit labels keep clear of them
    const hot = !g.mini && scene.hot ? scene.hot : [];
    const hotX = hot.map((h) => xOfT(g, h.t_s, total));
    const hotActive = scene.rider ? hot.reduce((last, h, i) => (h.t_s <= ridden ? i : last), -1) : -1;

    // summit labels (a setting): a small triangle and the time, only when there is width
    if (scene.summits && W >= 500 && !g.mini) {
      const fs0 = clamp(H * 0.05, 10, 14);
      x.font = `600 ${fs0}px ${FONT}`;
      x.textAlign = "center";
      x.textBaseline = "alphabetic";
      for (const s of g.summits) {
        const sx = g.xs[s]!;
        const sy = g.ys[s]!;
        const st = ((s - 1) / (g.M - 1)) * total;
        const nearRider = scene.rider && Math.abs(sx - xr) < 34;
        const underPin = pinBox !== null && sx + 20 > pinBox[0] && sx - 20 < pinBox[0] + pinBox[2] && sy - 28 < pinBox[1] + pinBox[3];
        const nearHot = hotX.some((hx) => Math.abs(sx - hx) < HOT_SUMMIT_GAP);
        if (nearRider || underPin || nearHot) continue;
        x.fillStyle = scene.ridden !== null && sx < xr ? "rgba(190,200,225,0.5)" : "rgba(255,255,255,0.9)";
        x.beginPath();
        x.moveTo(sx, sy - 5);
        x.lineTo(sx - 3.5, sy - 11);
        x.lineTo(sx + 3.5, sy - 11);
        x.closePath();
        x.fill();
        x.fillText(mmss(st), sx, sy - 15);
      }
    }

    hot.forEach((h, i) => {
      const part = parts[partIndexAt(parts, h.t_s)];
      this.paintHot(g, hotX[i]!, zoneRgb(part?.kind ?? ""), i === hotActive ? "active" : i < hotActive ? "passed" : "next");
    });
    if (scene.rider) this.paintRider(g, xr, yr, parts[curIx], t, animated, scene.riderGlyph !== "dot", !!scene.paused);
    if (!g.mini) this.paintTicks(g, total);
  }

  /** The "you are here" marker: hairline to the ribbon, still halo, a dot on the ridge blinking inside a beacon ring. */
  private paintRider(g: Layout, xr: number, yr: number, part: CoursePart | undefined, t: number, animated: boolean, bike: boolean, paused: boolean) {
    const x = this.ctx;
    const { H } = g;
    const zc = zoneRgb(part?.kind ?? "");
    if (!g.mini) {
      x.strokeStyle = css(zc, 0.55);
      x.lineWidth = 1;
      x.beginPath();
      x.moveTo(xr, yr + 8);
      x.lineTo(xr, g.floor - g.band);
      x.stroke();
    }
    const hr = clamp(H * 0.13, 12, 36);
    const hg = x.createRadialGradient(xr, yr, 0, xr, yr, hr);
    hg.addColorStop(0, css(zc, 0.7));
    hg.addColorStop(0.4, css(zc, 0.22));
    hg.addColorStop(1, css(zc, 0));
    x.fillStyle = hg;
    x.beginPath();
    x.arc(xr, yr, hr, 0, TAU);
    x.fill();
    const dr = clamp(H * 0.045, 4, 13);
    const beaconR = clamp(H * 0.28, 14, 70);
    const phase = (t * BEACON_HZ) % 1;
    const blink = animated ? BLINK_MID + BLINK_AMP * Math.cos(TAU * phase) : 1;
    const ring = (r: number, a: number) => {
      x.strokeStyle = css(zc, a);
      x.beginPath();
      x.arc(xr, yr, r, 0, TAU);
      x.stroke();
    };
    x.lineWidth = Math.max(1.5, dr * 0.3);
    if (animated) ring(dr + (beaconR - dr) * phase, 0.85 * (1 - phase) ** 1.5);
    else {
      ring(dr * 2, 0.55);
      ring(dr * 3.2, 0.28);
    }
    if (bike && !g.mini) this.paintBike(g, xr, yr, dr, zc, paused);
    x.globalAlpha = blink;
    x.fillStyle = css(zc);
    x.strokeStyle = "rgba(4,6,18,0.6)";
    x.lineWidth = 1.5;
    x.beginPath();
    x.arc(xr, yr, dr, 0, TAU);
    x.fill();
    x.stroke();
    x.fillStyle = "#fff";
    x.beginPath();
    x.arc(xr, yr, dr * 0.5, 0, TAU);
    x.fill();
    x.globalAlpha = 1;
  }

  /** One resistance step: a tag with "+1" pointing at the ridge; `active` is the step now (glows), `passed` is dimmed. */
  private paintHot(g: Layout, px: number, zc: Rgb, state: "next" | "active" | "passed") {
    const x = this.ctx;
    hotPath ??= new Path2D(HOT_D);
    const k = clamp(g.H * 0.085, HOT_SCALE[0], HOT_SCALE[1]) * (state === "active" ? HOT_ACTIVE_BOOST : 1);
    x.save();
    x.translate(px, ridgeY(g, px) - HOT_LIFT);
    x.scale(k, k);
    x.translate(-HOT_TIP[0], -HOT_TIP[1]);
    if (state === "active") {
      const radius = HOT_WIDTH * 1.6;
      const glow = x.createRadialGradient(HOT_TIP[0], 6, 0, HOT_TIP[0], 6, radius);
      glow.addColorStop(0, css(zc, 0.7));
      glow.addColorStop(0.4, css(zc, 0.22));
      glow.addColorStop(1, css(zc, 0));
      x.fillStyle = glow;
      x.fillRect(HOT_TIP[0] - radius, 6 - radius, radius * 2, radius * 2);
    }
    x.globalAlpha = state === "passed" ? HOT_PASSED_ALPHA : HOT_FILL_ALPHA;
    x.fillStyle = css(state === "passed" ? dimmed(zc) : zc);
    x.fill(hotPath);
    x.globalAlpha = 1;
    x.lineWidth = 1 / k;
    x.strokeStyle = css(INK, INK_OUTLINE_ALPHA);
    x.stroke(hotPath);
    x.fillStyle = css(INK);
    x.font = `800 7px ${FONT}`;
    x.textAlign = "center";
    x.textBaseline = "middle";
    x.fillText("+1", HOT_TIP[0], 8.2);
    x.restore();
  }

  /** The bike silhouette: rear wheel on the ridge just ahead of the dot, tilted to the slope. No height threshold. */
  private paintBike(g: Layout, xr: number, yr: number, dr: number, zc: Rgb, paused: boolean) {
    const x = this.ctx;
    riderPath ??= new Path2D(RIDER_D);
    const tilt = clamp(Math.atan2(ridgeY(g, xr + 9) - ridgeY(g, xr - 9), 18), -RIDER_TILT, RIDER_TILT);
    const k = (RIDER_SCALE * dr) / RIDER_BOX;
    x.save();
    x.translate(xr, yr - dr);
    x.rotate(tilt);
    x.scale(k, k);
    x.translate(-RIDER_ANCHOR[0], -RIDER_ANCHOR[1]);
    x.globalAlpha = paused ? RIDER_PAUSED_ALPHA : 1;
    x.fillStyle = css(zc);
    x.fill(riderPath);
    x.lineJoin = "round";
    x.lineWidth = 1 / k;
    x.strokeStyle = css(INK, INK_OUTLINE_ALPHA);
    x.stroke(riderPath);
    x.restore();
  }

  private paintTicks(g: Layout, total: number) {
    const x = this.ctx;
    const { W, H } = g;
    const step = tickStep(total, W);
    const tf = clamp(H * 0.05, 9, 14);
    const y = H - (g.tick - tf) / 2 - 1;
    x.font = `600 ${tf}px ${FONT}`;
    x.textBaseline = "alphabetic";
    x.fillStyle = "rgba(205,213,230,0.55)";
    for (let tt = 0; tt < total; tt += step) {
      if (tt > 0 && total - tt < step * 0.5) continue;
      x.textAlign = tt === 0 ? "left" : "center";
      x.fillText(mmss(tt), tt === 0 ? g.padX : xOfT(g, tt, total), y);
    }
    x.textAlign = "right";
    x.fillText(mmss(total), W - g.padX, y);
  }
}
