// HomeFito on a bike: the mascot (a white house with headphones) riding the course, pedalling with the part. One
// spec with the Android (`CourseHomefito.kt`) and TV (`ride.html`) renderers: keep the numbers in sync.
//
// Units: the rear wheel touches the ground at (REAR_X, 0), x forward, y down (so up is negative); the drawing is
// HEIGHT units tall. The legs follow the crank (two-bone reach to the pedals), the body leans and bobs with the
// effort, and on hard parts the eyes squeeze and sweat flies back; in easy ones he smiles upright.

type Rgb = readonly [number, number, number];

/** How the rider feels a part: easy (warm-up, recovery, cool-down), normal (flat), hard (climbs, sprints, jumps). */
export type Strain = "easy" | "normal" | "hard";

export const HOMEFITO_HEIGHT = 30;
export const HOMEFITO_REAR_X = 4.5;

const WHEEL_R = 4.5;
const REAR: Pt = [4.5, -4.5];
const FRONT: Pt = [19.5, -4.5];
const CRANK: Pt = [11, -4.5];
const CRANK_R = 2.2;
const SEAT: Pt = [9.2, -11.6];
const HEAD: Pt = [16.6, -10.6];
const BAR: Pt = [17.6, -12.8];
const HIP: Pt = [9.8, -12.9];
const THIGH = 5;
const SHIN = 5;
const SHOULDER: Pt = [12.6, -15.4];
// the house: walls, then the roof's eaves and ridge
const WALL_L = 4.6;
const WALL_R = 14.6;
const WALL_TOP = -20.5;
const EAVE = 1;
const RIDGE_Y = -25.5;
const CUP_RX = 1.3;
const CUP_RY = 2;
const CUP_Y = -17.6;
const BAND_TOP = -30;
const EYE_Y = -16.6;
const EYE_L = 9.4;
const EYE_R = 12.4;
const EYE_SIZE = 0.95;
const SWEAT = "#7fd3ff";
const WHITE = "#ffffff";
/** Pedal turns per second by strain (a still picture: the crank at 45°). */
const CADENCE_HZ: Record<Strain, number> = { easy: 1.1, normal: 1.45, hard: 1.2 };
/** Forward lean (radians) by strain: upright when easy, over the bars when hard. */
const LEAN: Record<Strain, number> = { easy: 0, normal: 0.1, hard: 0.24 };
const STILL_CRANK = Math.PI / 4;
const BOB = 0.35;
const FAR_ALPHA = 0.55;

type Pt = readonly [number, number];

export function strainOf(kind: string | undefined): Strain {
  switch (kind) {
    case "seated_climb":
    case "standing_climb":
    case "sprint":
    case "jumps":
      return "hard";
    case "warm_up":
    case "recovery":
    case "cool_down":
      return "easy";
    default:
      return "normal";
  }
}

const css = (c: Rgb, a = 1) => `rgba(${c[0]},${c[1]},${c[2]},${a})`;

/** The knee of a leg from [hip] to [foot]: the two-bone solution that bends forward (towards +x). */
function knee(hip: Pt, foot: Pt): Pt {
  const dx = foot[0] - hip[0];
  const dy = foot[1] - hip[1];
  const d = Math.min(Math.hypot(dx, dy), THIGH + SHIN - 0.01);
  const a = Math.atan2(dy, dx);
  const b = Math.acos(Math.max(-1, Math.min(1, (THIGH * THIGH + d * d - SHIN * SHIN) / (2 * THIGH * d))));
  const k1: Pt = [hip[0] + THIGH * Math.cos(a - b), hip[1] + THIGH * Math.sin(a - b)];
  const k2: Pt = [hip[0] + THIGH * Math.cos(a + b), hip[1] + THIGH * Math.sin(a + b)];
  return k1[0] >= k2[0] ? k1 : k2;
}

/** [p] turned by [ang] around [o]. */
function turn(p: Pt, o: Pt, ang: number): Pt {
  const c = Math.cos(ang);
  const s = Math.sin(ang);
  const dx = p[0] - o[0];
  const dy = p[1] - o[1];
  return [o[0] + dx * c - dy * s, o[1] + dx * s + dy * c];
}

/**
 * Draws HomeFito riding with the context already placed: (0, 0) is where the rear wheel meets the ridge, scaled to
 * units and turned to the slope. [t] is the animation clock in seconds (null: a still picture), [ink] the outline.
 */
export function paintHomefito(x: CanvasRenderingContext2D, zone: Rgb, ink: Rgb, strain: Strain, t: number | null, paused: boolean) {
  const moving = t !== null && !paused;
  const crank = moving ? t * CADENCE_HZ[strain] * Math.PI * 2 : STILL_CRANK;
  const lean = LEAN[strain];
  const bob = moving ? BOB * Math.sin(crank * 2) : 0;
  const line = (a: Pt, b: Pt, w: number, color: string) => {
    x.strokeStyle = color;
    x.lineWidth = w;
    x.beginPath();
    x.moveTo(a[0], a[1]);
    x.lineTo(b[0], b[1]);
    x.stroke();
  };
  // a limb: white with an ink edge, like the mascot's arms and legs
  const limb = (a: Pt, b: Pt, c: Pt | null) => {
    for (const [w, color] of [
      [1.7, css(ink, 0.85)],
      [1.05, WHITE],
    ] as const) {
      x.strokeStyle = color;
      x.lineWidth = w;
      x.beginPath();
      x.moveTo(a[0], a[1]);
      x.lineTo(b[0], b[1]);
      if (c) x.lineTo(c[0], c[1]);
      x.stroke();
    }
  };
  const dot = (p: Pt, r: number, color: string) => {
    x.fillStyle = color;
    x.beginPath();
    x.arc(p[0], p[1], r, 0, Math.PI * 2);
    x.fill();
  };
  const pedal = (phase: number): Pt => [CRANK[0] + CRANK_R * Math.cos(crank + phase), CRANK[1] + CRANK_R * Math.sin(crank + phase)];
  const hip: Pt = [HIP[0], HIP[1] + bob];

  x.save();
  x.lineCap = "round";
  x.lineJoin = "round";
  if (paused) x.globalAlpha *= 0.6;
  const base = x.globalAlpha;

  // the far leg, behind the bike
  const farFoot = pedal(Math.PI);
  x.globalAlpha = base * FAR_ALPHA;
  limb(hip, knee(hip, farFoot), farFoot);
  dot(farFoot, 0.8, css(ink));
  x.globalAlpha = base;

  // wheels and frame, in the zone's colour
  for (const hub of [REAR, FRONT]) {
    x.strokeStyle = css(ink, 0.9);
    x.lineWidth = 1.3;
    x.beginPath();
    x.arc(hub[0], hub[1], WHEEL_R - 0.4, 0, Math.PI * 2);
    x.stroke();
    x.strokeStyle = css(zone);
    x.lineWidth = 0.6;
    x.stroke();
    dot(hub, 0.6, css(zone));
  }
  const frame = css(zone);
  line(REAR, CRANK, 1.1, frame);
  line(CRANK, SEAT, 1.1, frame);
  line(SEAT, REAR, 1, frame);
  line(SEAT, HEAD, 1.1, frame);
  line(CRANK, HEAD, 1.1, frame);
  line(HEAD, FRONT, 1, frame);
  line(HEAD, BAR, 1, frame);
  line([SEAT[0] - 1.4, SEAT[1] - 0.3], [SEAT[0] + 1.4, SEAT[1] - 0.3], 1.2, css(ink));

  // the body: the house leaning from the hip, bobbing with the stroke
  x.save();
  x.translate(0, bob);
  x.translate(HIP[0], HIP[1]);
  x.rotate(lean);
  x.translate(-HIP[0], -HIP[1]);
  const house = new Path2D();
  house.moveTo(WALL_L, HIP[1] + 0.4);
  house.lineTo(WALL_L, WALL_TOP);
  house.lineTo(WALL_L - EAVE, WALL_TOP);
  house.lineTo((WALL_L + WALL_R) / 2, RIDGE_Y);
  house.lineTo(WALL_R + EAVE, WALL_TOP);
  house.lineTo(WALL_R, WALL_TOP);
  house.lineTo(WALL_R, HIP[1] + 0.4);
  house.closePath();
  x.fillStyle = WHITE;
  x.fill(house);
  x.strokeStyle = css(ink, 0.85);
  x.lineWidth = 0.6;
  x.stroke(house);
  // headphones: the band over the roof, a cup on each side in the zone's colour
  x.strokeStyle = css(ink, 0.9);
  x.lineWidth = 0.9;
  x.beginPath();
  x.moveTo(WALL_L - 0.3, CUP_Y - CUP_RY);
  x.quadraticCurveTo((WALL_L + WALL_R) / 2, BAND_TOP, WALL_R + 0.3, CUP_Y - CUP_RY);
  x.stroke();
  for (const cx of [WALL_L - 0.3, WALL_R + 0.3]) {
    x.fillStyle = css(zone);
    x.beginPath();
    x.ellipse(cx, CUP_Y, CUP_RX, CUP_RY, 0, 0, Math.PI * 2);
    x.fill();
    x.lineWidth = 0.5;
    x.stroke();
  }
  // the face: round eyes and a smile; on a hard part squeezed eyes and a tight mouth
  const inkCss = css(ink);
  if (strain === "hard") {
    for (const ex of [EYE_L, EYE_R]) {
      line([ex - 0.8, EYE_Y - 0.5], [ex + 0.8, EYE_Y], 0.55, inkCss);
      line([ex + 0.8, EYE_Y], [ex - 0.8, EYE_Y + 0.5], 0.55, inkCss);
    }
    line([EYE_L + 0.1, EYE_Y + 2.4], [EYE_R - 0.1, EYE_Y + 2.4], 0.55, inkCss);
  } else {
    dot([EYE_L, EYE_Y], EYE_SIZE, inkCss);
    dot([EYE_R, EYE_Y], EYE_SIZE, inkCss);
    x.strokeStyle = inkCss;
    x.lineWidth = 0.55;
    x.beginPath();
    x.arc((EYE_L + EYE_R) / 2, EYE_Y + 1.3, strain === "easy" ? 1.2 : 0.9, 0.25 * Math.PI, 0.75 * Math.PI);
    x.stroke();
  }
  x.restore();

  // the near arm to the bars (the shoulder moves with the lean and the bob), then the near leg over the frame
  const shoulder = turn([SHOULDER[0], SHOULDER[1] + bob], [HIP[0], HIP[1] + bob], lean);
  limb(shoulder, BAR, null);
  dot(BAR, 0.75, WHITE);
  const foot = pedal(0);
  line(CRANK, foot, 0.8, css(ink));
  limb(hip, knee(hip, foot), foot);
  dot(foot, 0.8, css(ink));

  // sweat flying back off the roof on a hard part
  if (strain === "hard" && moving) {
    for (let i = 0; i < 2; i++) {
      const p = (t * 1.3 + i * 0.5) % 1;
      x.globalAlpha = base * (1 - p);
      dot([WALL_L - 1 - 4 * p, WALL_TOP - 1 + 3 * p * p], 0.55, SWEAT);
    }
    x.globalAlpha = base;
  }
  x.restore();
}
