// The mascot's long hair: strands of points on springs (verlet), pinned to the
// scalp, pulled down by gravity and back to the shape they grow in, so the hair
// swings and lags a little behind the head. Pure math with no DOM; the Android
// app keeps the same numbers (`ui/player/Hair.kt`).
//
// Everything lives in the mascot's own units (the SVG is 1000 wide), in the
// picture's coordinates (not the body's), so the head's motion shows up as
// real movement of the roots and the hair carries on moving after it stops.

/** Where a strand grows from the head and how it hangs. */
export interface Strand {
  /** Root on the head, in the head's own units. */
  x: number;
  y: number;
  /** How far from straight down it grows, degrees (+ towards the right of the picture). */
  fan: number;
  /** Points on the strand (6). */
  points: number;
  /** Drawn behind the head (otherwise over the shoulder). */
  back: boolean;
  /** Darker underside strand, drawn first, for depth. */
  dark: boolean;
}

export const HAIR = {
  /** Fixed time step, seconds; at most `maxSteps` per frame. */
  step: 1 / 120,
  maxSteps: 4,
  gravity: 2600,
  /** Velocity kept each step: 0.86 per 1/60 s, so per 1/120 s step its square root. */
  damping: Math.pow(0.86, 1 / 2),
  /** Share of the way a point goes towards its resting shape each step, from the root end to the tip. */
  pullRoot: 0.055,
  pullTip: 0.018,
  segment: 58,
  /** Share of its sideways speed hair keeps when it lies on the shoulder. */
  friction: 0.5,
  /** Hair can not go below this (the shoulders). */
  shoulder: 850,
  /** Width of a strand at its root, and at its tip, in mascot units. */
  rootWidth: 90,
  tipWidth: 18,
  /** Passes that keep every segment its length. */
  iterations: 3,
} as const;

/** Eight strands: four dark ones deep behind, two behind the head, one each side over the shoulders. */
export const STRANDS: readonly Strand[] = [
  { x: 300, y: 440, fan: -52, points: 6, back: true, dark: true },
  { x: 724, y: 440, fan: 52, points: 6, back: true, dark: true },
  { x: 390, y: 372, fan: -30, points: 6, back: true, dark: true },
  { x: 634, y: 372, fan: 30, points: 6, back: true, dark: true },
  { x: 270, y: 470, fan: -34, points: 6, back: true, dark: false },
  { x: 754, y: 470, fan: 34, points: 6, back: true, dark: false },
  { x: 258, y: 500, fan: -14, points: 6, back: false, dark: false },
  { x: 766, y: 500, fan: 14, points: 6, back: false, dark: false },
];

const MAX_POINTS = 7;
const RAD = Math.PI / 180;

export class Hair {
  /** Positions, previous positions and resting offsets, `MAX_POINTS` slots per strand. */
  private readonly x = new Float64Array(STRANDS.length * MAX_POINTS);
  private readonly y = new Float64Array(STRANDS.length * MAX_POINTS);
  private readonly px = new Float64Array(STRANDS.length * MAX_POINTS);
  private readonly py = new Float64Array(STRANDS.length * MAX_POINTS);
  /** The shape each strand grows in, relative to its root (head not tilted). */
  private readonly restX = new Float64Array(STRANDS.length * MAX_POINTS);
  private readonly restY = new Float64Array(STRANDS.length * MAX_POINTS);
  private readonly outline = new Float64Array(4 * MAX_POINTS);
  private acc = 0;
  private placed = false;

  constructor() {
    STRANDS.forEach((s, i) => {
      let rx = 0;
      let ry = 0;
      for (let k = 0; k < s.points; k++) {
        // hair grows outwards from the scalp and curves back to hang
        const angle = s.fan * RAD * (1 - (0.7 * k) / s.points);
        if (k) {
          rx += Math.sin(angle) * HAIR.segment;
          ry += Math.cos(angle) * HAIR.segment;
        }
        this.restX[i * MAX_POINTS + k] = rx;
        this.restY[i * MAX_POINTS + k] = ry;
      }
    });
  }

  /** Lets it hang from wherever the roots are now (a new look, a new track). */
  settle() {
    this.placed = false;
  }

  /**
   * Moves the hair on by `dt` seconds. `roots` holds the x, y of every strand's
   * root in the picture, `tilt` is how many radians the head is turned from upright.
   */
  step(dt: number, roots: ArrayLike<number>, tilt: number) {
    if (!this.placed) this.place(roots, tilt);
    this.acc = Math.min(this.acc + dt, HAIR.step * HAIR.maxSteps);
    const cos = Math.cos(tilt);
    const sin = Math.sin(tilt);
    while (this.acc >= HAIR.step) {
      this.acc -= HAIR.step;
      this.substep(roots, cos, sin);
    }
  }

  private place(roots: ArrayLike<number>, tilt: number) {
    const cos = Math.cos(tilt);
    const sin = Math.sin(tilt);
    STRANDS.forEach((s, i) => {
      for (let k = 0; k < s.points; k++) {
        const j = i * MAX_POINTS + k;
        this.x[j] = this.px[j] = roots[2 * i] + this.restX[j] * cos - this.restY[j] * sin;
        this.y[j] = this.py[j] = roots[2 * i + 1] + this.restX[j] * sin + this.restY[j] * cos;
      }
    });
    this.placed = true;
  }

  private substep(roots: ArrayLike<number>, cos: number, sin: number) {
    const dt2 = HAIR.step * HAIR.step;
    for (let i = 0; i < STRANDS.length; i++) {
      const n = STRANDS[i].points;
      const base = i * MAX_POINTS;
      const rootX = roots[2 * i];
      const rootY = roots[2 * i + 1];
      // pinned to the scalp
      this.px[base] = this.x[base] = rootX;
      this.py[base] = this.y[base] = rootY;
      for (let k = 1; k < n; k++) {
        const j = base + k;
        const vx = (this.x[j] - this.px[j]) * HAIR.damping;
        const vy = (this.y[j] - this.py[j]) * HAIR.damping;
        this.px[j] = this.x[j];
        this.py[j] = this.y[j];
        this.x[j] += vx;
        this.y[j] += vy + HAIR.gravity * dt2;
        // towards the shape it grows in, stiffer near the root
        const stiff = HAIR.pullRoot + (HAIR.pullTip - HAIR.pullRoot) * (k / (n - 1));
        const tx = rootX + this.restX[j] * cos - this.restY[j] * sin;
        const ty = rootY + this.restX[j] * sin + this.restY[j] * cos;
        this.x[j] += (tx - this.x[j]) * stiff;
        this.y[j] += (ty - this.y[j]) * stiff;
      }
      for (let pass = 0; pass < HAIR.iterations; pass++) {
        for (let k = 1; k < n; k++) {
          const a = base + k - 1;
          const b = base + k;
          const dx = this.x[b] - this.x[a];
          const dy = this.y[b] - this.y[a];
          const len = Math.hypot(dx, dy) || 1;
          const fix = (len - HAIR.segment) / len;
          // the root does not move; the others share the correction
          const wa = k === 1 ? 0 : 0.5;
          this.x[a] += dx * fix * wa;
          this.y[a] += dy * fix * wa;
          this.x[b] -= dx * fix * (1 - wa);
          this.y[b] -= dy * fix * (1 - wa);
        }
      }
      // the shoulders: hair lies on them instead of going through
      for (let k = 1; k < n; k++) {
        const j = base + k;
        if (this.y[j] > HAIR.shoulder) {
          this.y[j] = HAIR.shoulder;
          // and drags: it keeps only some of its sideways speed
          this.px[j] = this.x[j] - (this.x[j] - this.px[j]) * HAIR.friction;
        }
      }
    }
  }

  /**
   * The outline of strand `i` as a smooth tapered shape (an SVG path): wide at
   * the root, thin at the tip, curves through the points.
   */
  path(i: number): string {
    const s = STRANDS[i];
    const n = s.points;
    const base = i * MAX_POINTS;
    const o = this.outline;
    // left and right edge of every point, from the direction of the strand there
    for (let k = 0; k < n; k++) {
      const a = base + Math.max(0, k - 1);
      const b = base + Math.min(n - 1, k + 1);
      const dx = this.x[b] - this.x[a];
      const dy = this.y[b] - this.y[a];
      const len = Math.hypot(dx, dy) || 1;
      // narrow where it leaves the scalp (so the cut end does not show), fullest a little below, then tapering
      const taper = HAIR.tipWidth + (HAIR.rootWidth - HAIR.tipWidth) * Math.pow(1 - k / (n - 1), 0.8);
      const w = (k === 0 ? 0.5 * taper : taper) / 2;
      const j = base + k;
      o[4 * k] = this.x[j] - (dy / len) * w;
      o[4 * k + 1] = this.y[j] + (dx / len) * w;
      o[4 * k + 2] = this.x[j] + (dy / len) * w;
      o[4 * k + 3] = this.y[j] - (dx / len) * w;
    }
    const f = (v: number) => v.toFixed(1);
    let d = `M${f(o[0])} ${f(o[1])}`;
    // down one edge, through the midpoints (quadratic curves), round the tip, up the other
    for (let k = 1; k < n - 1; k++) d += `Q${f(o[4 * k])} ${f(o[4 * k + 1])} ${f((o[4 * k] + o[4 * k + 4]) / 2)} ${f((o[4 * k + 1] + o[4 * k + 5]) / 2)}`;
    d += `L${f(o[4 * (n - 1)])} ${f(o[4 * (n - 1) + 1])}L${f(o[4 * (n - 1) + 2])} ${f(o[4 * (n - 1) + 3])}`;
    for (let k = n - 2; k >= 1; k--) d += `Q${f(o[4 * k + 2])} ${f(o[4 * k + 3])} ${f((o[4 * k + 2] + o[4 * k - 2]) / 2)} ${f((o[4 * k + 3] + o[4 * k - 1]) / 2)}`;
    return `${d}L${f(o[2])} ${f(o[3])}Z`;
  }
}
