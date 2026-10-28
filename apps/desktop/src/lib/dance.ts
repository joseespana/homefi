// The mascot's choreography: a pose for every moment of the beat. Each genre
// mood has its own groove, and every few bars it throws in a famous dance.
// Pure math with no DOM, so the Android app can share the same steps
// (keep `ui/player/Dance.kt` in sync).

import type { Mood } from "./mood";

export type DanceId =
  | "jagger"
  | "moonwalk"
  | "twist"
  | "robot"
  | "disco"
  | "ymca"
  | "floss"
  | "macarena"
  | "running"
  | "horse"
  | "guitar";

export const DANCES: { id: DanceId; label: string; hint: string }[] = [
  { id: "jagger", label: "Like Jagger", hint: "Rooster strut, hips and a point to the sky" },
  { id: "moonwalk", label: "Moonwalk", hint: "Gliding backwards" },
  { id: "twist", label: "The Twist", hint: "Hips twisting low" },
  { id: "robot", label: "Robot", hint: "Stiff moves, one per beat" },
  { id: "disco", label: "Disco fever", hint: "Point up, point down" },
  { id: "ymca", label: "YMCA", hint: "Spelling it with the arms" },
  { id: "floss", label: "Floss", hint: "Arms swinging past the hips" },
  { id: "macarena", label: "Macarena", hint: "Arms, head, hips, turn" },
  { id: "running", label: "Running man", hint: "Running on the spot" },
  { id: "horse", label: "Horse ride", hint: "Galloping with the reins" },
  { id: "guitar", label: "Air guitar", hint: "Strumming a guitar that isn't there" },
];

/** The famous dances each genre picks from. */
export const SPECIALS: Record<Mood, DanceId[]> = {
  bop: ["jagger", "twist", "moonwalk", "disco"],
  mosh: ["guitar", "running"],
  pogo: ["running", "twist", "guitar"],
  cute: ["floss", "macarena", "ymca"],
  party: ["robot", "disco", "ymca", "floss"],
  salsa: ["macarena", "twist", "jagger"],
  groove: ["running", "moonwalk", "horse", "robot"],
  chill: ["jagger", "moonwalk"],
};

/** How often the famous dances come up. */
export type Specials = "off" | "sometimes" | "often";

/** When it may turn sideways: never, only in the famous dances, or also in the grooves. */
export type Turns = "off" | "dances" | "all";

/**
 * Where everything is, in the mascot's own units (the SVG is 1000 wide).
 * Limb angles are how far the limb is raised away from the body: 0 hangs
 * straight down, 90 points sideways, 180 straight up, below 0 crosses in front.
 */
export interface Pose {
  x: number;
  y: number;
  /** Body tilt in degrees, around the feet. */
  rot: number;
  sx: number;
  sy: number;
  armL: number;
  armR: number;
  legL: number;
  legR: number;
  /** How high each foot is lifted. */
  liftL: number;
  liftR: number;
  /** Facing the other way (for turns when it may not turn sideways). */
  flip: boolean;
  /**
   * How far it has turned to show its side: 0 faces you, 1 is a profile
   * facing right, -1 facing left. In profile the limb angles swing forward
   * (positive) and back (negative) instead of out to the sides.
   */
  turn: number;
}

export const REST: Pose = { x: 0, y: 0, rot: 0, sx: 1, sy: 1, armL: 30, armR: 30, legL: 0, legR: 0, liftL: 0, liftR: 0, flip: false, turn: 0 };

const PI = Math.PI;
const sin = Math.sin;
const cos = Math.cos;
const pos = (v: number) => Math.max(0, v);
const frac = (v: number) => v - Math.floor(v);
/** 1 on the beat, 0 half way between beats. */
const onBeat = (b: number) => 0.5 + 0.5 * cos(2 * PI * b);
/** A snappy move into a new position at the start of each beat. */
const snap = (b: number, speed = 5) => Math.min(1, frac(b) * speed);
const ease = (t: number) => t * t * (3 - 2 * t);
const mix = (a: number, b: number, t: number) => a + (b - a) * t;

function pose(p: Partial<Pose>): Pose {
  return { ...REST, ...p };
}

/** Steps through a table of [left arm, right arm], one row per beat. */
function arms(table: [number, number][], b: number, speed = 4): [number, number] {
  const i = Math.floor(b) % table.length;
  const prev = table[(i + table.length - 1) % table.length];
  const t = ease(snap(b, speed));
  return [mix(prev[0], table[i][0], t), mix(prev[1], table[i][1], t)];
}

/** Steps through one value per beat, moving snappily into each. */
function steps(table: number[], b: number, speed = 4): number {
  const i = Math.floor(b) % table.length;
  const prev = table[(i + table.length - 1) % table.length];
  return mix(prev, table[i], ease(snap(b, speed)));
}

/**
 * The everyday groove for a mood. `b` counts beats, `k` (0.5..1.3) is how
 * loud it is; `side` lets it turn sideways.
 */
export function moodPose(mood: Mood, b: number, k: number, side = false): Pose {
  const p = groove(mood, b, k);
  if (!side) return p;
  const s = sin(PI * b);
  const slow = sin((PI * b) / 4); // one look each way every eight beats
  const turn: Record<Mood, number> = {
    mosh: 0,
    pogo: 0,
    cute: 0.25 * sin((2 * PI * b) / 3),
    party: 0.3 * s,
    salsa: 0.45 * s,
    groove: 0.35 * slow,
    chill: 0.3 * sin((PI * b) / 2),
    bop: 0.2 * slow,
  };
  return { ...p, turn: turn[mood] };
}

function groove(mood: Mood, b: number, k: number): Pose {
  const s = sin(PI * b); // one swing every two beats
  const d = onBeat(b);
  switch (mood) {
    case "mosh": // headbanging with the horns up
      return pose({ y: 10 * k * d, rot: k * (-6 + 24 * d), armL: 150 + 12 * k * d, armR: 150 + 12 * k * d, legL: 16, legR: 16 });
    case "pogo": {
      const j = sin(PI * frac(b)); // in the air between beats
      const land = Math.pow(1 - j, 4);
      return pose({ y: -70 * k * j, sx: 1 + 0.06 * land, sy: 1 - 0.08 * land, armL: 140 + 30 * sin(4 * PI * b), armR: 140 - 30 * sin(4 * PI * b), legL: -4, legR: -4 });
    }
    case "cute": {
      const w = sin((2 * PI * b) / 3);
      return pose({ y: -14 * k * pos(w), rot: 7 * k * w, armL: 125 + 30 * sin(2 * PI * b), armR: 125 + 30 * sin(2 * PI * b + PI), liftL: 10 * pos(w), liftR: 10 * pos(-w) });
    }
    case "party": // fist pump on every beat
      return pose({ x: 22 * k * s, y: -8 * k * d, rot: 10 * k * s, armL: 40 + 10 * s, armR: 95 + 75 * k * d, liftL: 12 * pos(s), liftR: 12 * pos(-s) });
    case "salsa": // the basic step: hips one way, a step the other
      return pose({ x: -22 * k * s, y: -6 * pos(-cos(2 * PI * b)), rot: 9 * k * s, armL: 80 + 18 * s, armR: 80 - 18 * s, legL: 6 * s, legR: -6 * s, liftL: 16 * pos(s), liftR: 16 * pos(-s) });
    case "groove": // knees bounce down on the beat
      return pose({ y: 12 * k * d, rot: k * (-3 + 9 * d), armL: 20 + 28 * k * s, armR: 20 - 28 * k * s, legL: 8 * d, legR: 8 * d });
    case "chill": {
      const w = sin((PI * b) / 2);
      return pose({ x: 6 * w, rot: 5 * k * w, armL: 22 + 8 * w, armR: 22 - 8 * w, liftR: 8 * d });
    }
    default: // bop: a nod on the beat, a little lean and a foot tap
      return pose({ y: 14 * k * d, rot: 4 * k * s, armL: 35 + 20 * k * s, armR: 35 - 20 * k * s, liftL: 14 * k * pos(s), liftR: 14 * k * pos(-s) });
  }
}

/**
 * A famous dance. `b` counts beats from the start of the dance; with `side`
 * the dances that look better from the side turn to show it.
 */
export function dancePose(id: DanceId, b: number, k: number, side = false): Pose {
  const s = sin(PI * b);
  const d = onBeat(b);
  switch (id) {
    case "jagger": {
      // the strut: hips across the floor, flapping elbows, a point up every bar
      const point = ease(Math.min(1, pos(frac(b / 4) * 4 - 3) * 3));
      // turned three quarters towards where the strut is going
      return pose({ x: 40 * k * sin((PI * b) / 2), y: -6 * d, rot: -6 + 8 * s, armL: 60 + 80 * pos(s), armR: mix(45 + 30 * pos(-s), 165, point), legL: 14 * pos(s), legR: 14 * pos(-s), liftL: 24 * pos(s), liftR: 24 * pos(-s), turn: side ? 0.6 * cos((PI * b) / 2) : 0 });
    }
    case "moonwalk": {
      // glide one way for four beats, turn, glide back; one heel up at a time
      const leg = Math.floor(b / 4) % 2 === 1;
      const t = frac(b / 4);
      const heel = Math.floor(b) % 2 === 0;
      if (side) {
        // in profile, facing away from the glide, spinning round at each end
        const dir = leg ? -1 : 1;
        const turn = mix(-dir, dir, ease(Math.min(1, (t * 4) / 0.6)));
        return pose({ x: (leg ? -1 : 1) * (70 - 140 * t), rot: -3, armL: 18 + 14 * s, armR: 18 - 14 * s, legL: heel ? 12 : -16, legR: heel ? -16 : 12, liftL: heel ? 20 : 0, liftR: heel ? 0 : 20, turn });
      }
      return pose({ x: (leg ? -1 : 1) * (70 - 140 * t), rot: -4, armL: 28 + 8 * s, armR: 28 - 8 * s, legL: heel ? -8 : 4, legR: heel ? 4 : -8, liftL: heel ? 18 : 0, liftR: heel ? 0 : 18, flip: leg });
    }
    case "twist": {
      const w = sin(2 * PI * b);
      return pose({ y: 14 + 8 * Math.abs(w), rot: 14 * k * w, sx: 1 + 0.03 * Math.abs(w), armL: 60 + 32 * w, armR: 60 - 32 * w, legL: 12 * w, legR: 12 * w, turn: side ? 0.35 * w : 0 });
    }
    case "robot": {
      const [l, r] = arms([[90, 10], [90, 90], [170, 90], [10, 170]], b, 8);
      const i = Math.floor(b) % 4;
      // a stiff quarter turn every couple of beats
      const turn = side ? steps([0, 0, 1, 1, 0, 0, -1, -1], b, 8) : 0;
      return pose({ x: [0, 10, 0, -10][i], rot: [0, -6, 0, 6][i], armL: l, armR: r, legL: 2, legR: 2, turn });
    }
    case "disco": {
      // the famous point: up to the sky, down across, hand on the hip
      const up = Math.floor(b) % 2 === 0;
      const t = ease(snap(b, 4));
      const turn = side ? (up ? mix(-0.15, 0.45, t) : mix(0.45, -0.15, t)) : 0;
      return pose({ x: 14 * s, rot: 6 * s, armL: 28, armR: up ? mix(-20, 165, t) : mix(165, -20, t), liftL: up ? 0 : 14, legL: up ? 0 : 10, turn });
    }
    case "ymca": {
      // Y, M, C, A, then four beats of clapping overhead
      const [l, r] = arms([[150, 150], [195, 195], [160, 70], [175, 175], [170, 170], [178, 178], [170, 170], [178, 178]], b, 5);
      return pose({ y: Math.floor(b) % 8 >= 4 ? -10 * d : 0, armL: l, armR: r });
    }
    case "floss": {
      const w = sin(2 * PI * b);
      return pose({ x: -14 * k * w, rot: -4 * w, armL: 30 + 45 * w, armR: 30 - 45 * w, turn: side ? 0.2 * w : 0 });
    }
    case "macarena": {
      // arms out, palms, cross, head, hips... then a hip wiggle and a quarter turn
      const [l, r] = arms([[35, 90], [90, 90], [90, -30], [-30, -30], [-30, 175], [175, 175], [175, 20], [20, 20]], b, 5);
      const wiggle = Math.floor(b) % 8 === 7 ? sin(4 * PI * b) : 0;
      const turned = Math.floor(b / 8) % 2 === 1;
      if (side) {
        // the quarter turn at the end of each round, shown as a real turn
        const into = ease(Math.min(1, frac(b / 8) * 16));
        const turn = turned ? into : b < 8 ? 0 : 1 - into;
        return pose({ x: 18 * wiggle, y: -4 * d, rot: 6 * wiggle, armL: l, armR: r, turn });
      }
      return pose({ x: 18 * wiggle, y: -4 * d, rot: 6 * wiggle, armL: l, armR: r, flip: turned });
    }
    case "running": {
      if (side) {
        // in profile: knee up in front, the other foot sliding back, arms pumping
        return pose({ y: -8 * d, rot: 7 + 2 * s, armL: 75 * s, armR: -75 * s, legL: 42 * s, legR: -42 * s, liftL: 34 * pos(s), liftR: 34 * pos(-s), turn: 1 });
      }
      return pose({ y: -8 * d, rot: 3 * s, armL: 40 + 34 * s, armR: 40 - 34 * s, legL: -14 * pos(s), legR: -14 * pos(-s), liftL: 32 * pos(s), liftR: 32 * pos(-s) });
    }
    case "horse": {
      // holding the reins, galloping; every other bar the lasso goes round
      const lasso = Math.floor(b / 4) % 2 === 1;
      const hop = Math.abs(s);
      if (side) {
        // riding sideways: reins held out in front, legs galloping
        return pose({ y: -26 * k * hop, rot: 5 + 3 * s, armL: 70 + 6 * d, armR: lasso ? 165 + 15 * sin(4 * PI * b) : 70 + 6 * d, legL: 24 * s, legR: 24 * sin(PI * b + 1), liftL: 14 * pos(s), liftR: 14 * pos(-s), turn: 1 });
      }
      return pose({ y: -26 * k * hop, rot: 3 * s, armL: -25 + 6 * d, armR: lasso ? 165 + 15 * sin(4 * PI * b) : -25 + 6 * d, liftL: 14 * pos(s), liftR: 14 * pos(-s) });
    }
    case "guitar": {
      // fretting hand out, picking hand strumming twice a beat, a jump at the end of the bar
      const jump = Math.floor(b) % 4 === 3 ? sin(PI * frac(b)) : 0;
      return pose({ y: 8 * d - 50 * k * jump, rot: -8 + 4 * d, armL: 70, armR: -10 + 26 * Math.abs(sin(4 * PI * b)), legL: 16, legR: 16, turn: side ? 0.45 : 0 });
    }
  }
}

function blend(a: Pose, b: Pose, t: number): Pose {
  return {
    x: mix(a.x, b.x, t),
    y: mix(a.y, b.y, t),
    rot: mix(a.rot, b.rot, t),
    sx: mix(a.sx, b.sx, t),
    sy: mix(a.sy, b.sy, t),
    armL: mix(a.armL, b.armL, t),
    armR: mix(a.armR, b.armR, t),
    legL: mix(a.legL, b.legL, t),
    legR: mix(a.legR, b.legR, t),
    liftL: mix(a.liftL, b.liftL, t),
    liftR: mix(a.liftR, b.liftR, t),
    flip: t < 0.5 ? a.flip : b.flip,
    turn: mix(a.turn, b.turn, t),
  };
}

const DANCE_BEATS = 16;

/**
 * Keeps the beat (tempo changes don't make it jump), decides when a famous
 * dance starts and blends smoothly from one move into the next.
 */
export class Choreographer {
  beat = 0;
  /** The famous dance going on, if any. */
  dance: DanceId | null = null;
  private danceStart = 0;
  private nextAt = 8;
  private from: Pose = REST;
  private fromAt = -10;
  private last: Pose = REST;
  private wave = 0;

  /** Starts a famous dance now (or a random one that suits the mood). */
  trigger(mood: Mood, id?: DanceId) {
    const pool = SPECIALS[mood];
    this.start(id ?? pool[Math.floor(Math.random() * pool.length)]);
  }

  /** Waves hello for a couple of seconds (when clicked while paused). */
  greet() {
    this.wave = 2.2;
  }

  private start(id: DanceId | null) {
    this.from = this.last;
    this.fromAt = this.beat;
    this.dance = id;
    this.danceStart = Math.ceil(this.beat);
  }

  /**
   * Advances by `dt` seconds and returns the pose to draw. `pinned` keeps one
   * famous dance going all the time.
   */
  step(dt: number, o: { playing: boolean; bpm: number; energy: number; mood: Mood; specials: Specials; pinned: DanceId | null; turns?: Turns }): Pose {
    const k = o.playing ? 0.5 + Math.min(1, o.energy * 1.6) * 0.8 : 0;
    this.wave = Math.max(0, this.wave - dt);
    if (!o.playing) {
      // asleep: arms down, breathing; or waving when poked
      const breathe = sin(performance.now() / 900);
      const p = pose({ y: 4 + 3 * breathe, sy: 1 + 0.012 * breathe, armL: 12, armR: this.wave > 0 ? 150 + 25 * sin(this.wave * 14) : 12 });
      return (this.last = this.ease(p, dt));
    }
    this.beat += (dt * o.bpm) / 60;

    if (o.pinned && this.dance !== o.pinned) this.start(o.pinned);
    if (!o.pinned) {
      if (this.dance && this.beat - this.danceStart >= DANCE_BEATS) {
        this.start(null);
        this.nextAt = this.beat + (o.specials === "often" ? 16 : 32) + Math.floor(Math.random() * 8);
      } else if (!this.dance && o.specials !== "off" && this.beat >= this.nextAt) {
        this.trigger(o.mood);
      }
    }

    const b = this.beat - this.danceStart;
    const turns = o.turns ?? "all";
    let p = this.dance && b >= 0 ? dancePose(this.dance, b, k, turns !== "off") : moodPose(o.mood, this.beat, k, turns === "all");
    // half a beat to blend into a new move
    const t = Math.min(1, (this.beat - this.fromAt) / 0.5);
    if (t < 1) p = blend(this.from, p, ease(t));
    if (this.wave > 0) p = { ...p, armR: 150 + 25 * sin(this.wave * 14) };
    this.last = p;
    return p;
  }

  /** Smooths changes when nothing drives the beat (e.g. falling asleep). */
  private ease(target: Pose, dt: number): Pose {
    return blend(this.last, target, Math.min(1, dt * 6));
  }
}

// Clicked too many times, too fast: it stomps, shakes its fists and fumes.

/** Quick clicks (within `TEMPER_WINDOW` seconds) that make it angry; `off` never. */
export const TEMPER_CLICKS = { off: 0, patient: 10, normal: 6, short: 4 } as const;
export type Temper = keyof typeof TEMPER_CLICKS;
export const TEMPER_WINDOW = 3;
/** How long a tantrum lasts, in seconds (more clicks while angry make it longer). */
export const ANGER_TIME = 3;

/** The tantrum, `t` seconds in: trembling, stomping in turns, fists shaking. */
export function angryPose(t: number): Pose {
  const shake = Math.sin(2 * Math.PI * 9 * t);
  const fists = Math.sin(2 * Math.PI * 4 * t);
  const stomp = Math.abs(Math.sin(2 * Math.PI * 2 * t));
  const left = Math.floor(t * 4) % 2 === 0;
  return pose({
    x: 9 * shake,
    rot: 3 * shake,
    sx: 1.03,
    sy: 0.97,
    armL: 118 + 26 * fists,
    armR: 118 - 26 * fists,
    liftL: left ? 26 * stomp : 0,
    liftR: left ? 0 : 26 * stomp,
  });
}
