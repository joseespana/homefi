// Homifito DJ: the pure part (no DOM, no stores). Where Homifito stands, what he does for what DJ mode
// is doing (bob on the master beat, hands on the decks, scratching, a hand to the crossfader, a
// reaction to a sampler pad, one headphone cup off the ear while the other deck is cued) and the
// damping that makes it smooth. The component draws it; `tests/homifito-dj.test.ts` pins it down.
import type { DeckId, DeckSnapshot, DjSnapshot, SfxKind } from "./dj-api";

/** What the user can pick: nothing, the small booth, or the big stage. */
export type HomifitoMode = "off" | "booth" | "big";
export const HOMIFITO_MODES: readonly HomifitoMode[] = ["off", "booth", "big"];
export type HomifitoPlace = "top" | "bottom";
export type MotionLevelName = "full" | "subtle" | "off";

export interface Vec {
  x: number;
  y: number;
}

// ------------------------------------------------------------------ the stage (viewBox units)
/** `top` is negative: the headphone band and a raised hand reach above the console's row. */
export const STAGE = { w: 1000, h: 400, top: -84 } as const;
/** Homifito's house is the logo's 1024 drawing scaled by this and hung from `TOP`. */
export const HOUSE_SCALE = 0.46;
export const HOUSE_TOP_Y = 26;
/** Where the house's bottom middle is on the stage, the pivot of his sway. */
export const PIVOT: Vec = { x: 500, y: HOUSE_TOP_Y + (806 - 288) * HOUSE_SCALE };
/** The shoulders as the logo draws them (1024 units), turned into stage units. */
const houseToStage = (x: number, y: number): Vec => ({ x: STAGE.w / 2 + (x - 512) * HOUSE_SCALE, y: HOUSE_TOP_Y + (y - 288) * HOUSE_SCALE });
export const SHOULDER_L = houseToStage(268, 730);
export const SHOULDER_R = houseToStage(756, 730);

/** The decks' platters (centre) and the crossfader's slot, on the console. */
export const PLATTER: [Vec, Vec] = [
  { x: 240, y: 322 },
  { x: 760, y: 322 },
];
export const PLATTER_RX = 98;
/** The platter is seen from above at an angle: its height over its width. */
export const PLATTER_SQUASH = 0.36;
export const XF_SLOT = { x1: 438, x2: 562, y: 354 } as const;
export const HAND_R = 15;

const REST_L: Vec = { x: 292, y: 304 };
const REST_R: Vec = { x: 708, y: 304 };
const UP_L: Vec = { x: 330, y: 52 };
const UP_R: Vec = { x: 670, y: 52 };
const JUMP_L: Vec = { x: 372, y: 140 };
const JUMP_R: Vec = { x: 628, y: 140 };
const SCRATCH_REACH_PX = 34;
const TAP_PX = 7;

// ------------------------------------------------------------------ beat
const FULL_CIRCLE = 1;
/** A track's degrees per second on a 33⅓ rpm record. */
export const DEG_PER_SECOND = 200;
/** How fast the body drops on the beat (fraction of a beat), then climbs back. */
const DIP_ATTACK = 0.12;
const BOB_PX = 14;
const SWAY_DEG = 3.2;
const CUE_TILT_DEG = 5;
/** Without a beat grid the stage still bounces, at this tempo. */
export const FREE_BPM = 112;
const SECONDS_PER_MINUTE = 60;

/** The beat phase of a deck (0 on the beat, up to 1), or `null` when its beat isn't known. */
export function beatPhase(deck: DeckSnapshot, positionMs: number): number | null {
  const t = deck.track;
  if (!t || !t.has_grid || t.beat_ms <= 0) return null;
  const p = ((positionMs - t.first_beat_ms) / t.beat_ms) % FULL_CIRCLE;
  return p < 0 ? p + FULL_CIRCLE : p;
}

/** Which beat of the track a position is on (for alternating the sway). */
export function beatNumber(deck: DeckSnapshot, positionMs: number): number {
  const t = deck.track;
  return t && t.beat_ms > 0 ? Math.floor((positionMs - t.first_beat_ms) / t.beat_ms) : 0;
}

/** The body's drop on a beat: 0 at rest, 1 right after the beat, easing back by the next. */
export function beatDip(phase: number): number {
  if (phase < DIP_ATTACK) return phase / DIP_ATTACK;
  const back = (phase - DIP_ATTACK) / (FULL_CIRCLE - DIP_ATTACK);
  return (1 - back) * (1 - back);
}

/** The deck the room hears most among the playing ones (the crossfader decides when both play). */
export function masterDeck(snap: DjSnapshot | null): DeckId | null {
  if (!snap) return null;
  const [a, b] = snap.decks;
  const aOn = a.playing && !!a.track;
  const bOn = b.playing && !!b.track;
  if (aOn && bOn) return snap.crossfader < 0.5 ? "a" : "b";
  return aOn ? "a" : bOn ? "b" : null;
}

/** A loaded deck that doesn't play while the other one does: the DJ is cueing it in his headphones. */
export function cueingDeck(snap: DjSnapshot | null): DeckId | null {
  if (!snap) return null;
  const [a, b] = snap.decks;
  if (a.playing && b.track && !b.playing && !b.loading) return "b";
  if (b.playing && a.track && !a.playing && !a.loading) return "a";
  return null;
}

// ------------------------------------------------------------------ reactions
/** What a sampler pad makes him do. */
export type Reaction = "hands_up" | "jump" | "scratch";
const REACTIONS: Record<SfxKind, Reaction> = { air_horn: "hands_up", siren: "hands_up", laser: "hands_up", riser: "hands_up", drop: "jump", hit: "jump", scratch: "scratch", rewind: "scratch" };
export const reactionOf = (sound: SfxKind): Reaction => REACTIONS[sound];
/** How long each reaction lasts (ms). */
export const REACTION_MS: Record<Reaction, number> = { hands_up: 1400, jump: 800, scratch: 1000 };
const JUMP_PX = 40;
const WAVE_HZ = 1.6;
const WAVE_PX = 22;
const SCRATCH_HZ = 2.6;

// ------------------------------------------------------------------ what to aim at
/** The inputs of one frame, already read from the DJ snapshot and the clock. */
export interface StageInput {
  level: MotionLevelName;
  /** Master beat phase 0..1 (free-running when the beat isn't known). */
  phase: number;
  beatIndex: number;
  /** A deck is playing. */
  playing: [boolean, boolean];
  /** A deck is being scratched or nudged. */
  scratching: [boolean, boolean];
  cueing: DeckId | null;
  crossfader: number;
  /** A hand is on the crossfader (it moved just now, or a mix runs), and which one. */
  xfHand: 0 | 1 | null;
  reaction: { kind: Reaction; progress: number } | null;
  seconds: number;
}

/** Where everything should be; the component eases the drawn pose towards it. */
export interface Target {
  bodyY: number;
  tilt: number;
  handL: Vec;
  handR: Vec;
  /** 0..1: how far the headphone cup opposite the cued deck is off the ear (0 on the ear). */
  cupLift: number;
  /** The cup that lifts: Homifito's left (as drawn) or right. */
  cupSide: "l" | "r" | null;
  happy: boolean;
}

const lerp = (a: number, b: number, t: number) => a + (b - a) * t;

export const crossfaderKnob = (xf: number): Vec => ({ x: lerp(XF_SLOT.x1, XF_SLOT.x2, Math.min(1, Math.max(0, xf))), y: XF_SLOT.y - 4 });

/** What Homifito aims for: calm on "subtle" (a gentle sway), still on "off", the whole show on "full". */
export function aim(i: StageInput): Target {
  const full = i.level === "full";
  const amp = full ? 1 : i.level === "subtle" ? 0.3 : 0;
  const lively = i.playing[0] || i.playing[1];
  const dip = lively ? beatDip(i.phase) : 0;
  const side = i.beatIndex % 2 === 0 ? -1 : 1;
  const cupSide = i.cueing === "a" ? "r" : i.cueing === "b" ? "l" : null;
  const cueSign = i.cueing === "a" ? -1 : i.cueing === "b" ? 1 : 0;

  let bodyY = dip * BOB_PX * amp;
  let tilt = (lively ? side * SWAY_DEG * amp : 0) + cueSign * CUE_TILT_DEG * (i.level === "off" ? 0 : 1);
  const handL: Vec = { x: REST_L.x, y: REST_L.y + dip * TAP_PX * amp };
  const handR: Vec = { x: REST_R.x, y: REST_R.y + dip * TAP_PX * amp * (side > 0 ? 0.4 : 1) };
  let happy = false;

  const scratch = (hand: Vec, deck: 0 | 1, t: number) => {
    hand.x = (deck === 0 ? REST_L.x : REST_R.x) + Math.sin(t * Math.PI * 2 * SCRATCH_HZ) * SCRATCH_REACH_PX;
    hand.y = (deck === 0 ? REST_L.y : REST_R.y) + 14;
  };
  if (full) {
    if (i.scratching[0]) scratch(handL, 0, i.seconds);
    if (i.scratching[1]) scratch(handR, 1, i.seconds + 0.3);
  }

  // a hand on the crossfader follows the knob
  if (i.xfHand !== null) {
    const knob = crossfaderKnob(i.crossfader);
    const hand = i.xfHand === 0 ? handL : handR;
    hand.x = knob.x + (i.xfHand === 0 ? -4 : 4);
    hand.y = knob.y + 6;
  }

  const r = i.reaction;
  if (r) {
    happy = true;
    if (full) {
      const wave = Math.sin(i.seconds * Math.PI * 2 * WAVE_HZ) * WAVE_PX;
      if (r.kind === "hands_up") {
        Object.assign(handL, { x: UP_L.x + wave, y: UP_L.y + Math.abs(wave) * 0.4 });
        Object.assign(handR, { x: UP_R.x + wave, y: UP_R.y + Math.abs(wave) * 0.4 });
      } else if (r.kind === "jump") {
        const arc = Math.sin(Math.PI * Math.min(1, r.progress));
        bodyY -= JUMP_PX * arc;
        Object.assign(handL, { x: JUMP_L.x, y: JUMP_L.y - 40 * arc });
        Object.assign(handR, { x: JUMP_R.x, y: JUMP_R.y - 40 * arc });
      } else {
        scratch(handL, 0, i.seconds);
        scratch(handR, 1, i.seconds + 0.25);
      }
    }
  }
  const cupLift = i.cueing ? 1 : 0;
  return { bodyY, tilt, handL, handR, cupLift, cupSide, happy };
}

// ------------------------------------------------------------------ smoothing
/** The pose as drawn. */
export interface Pose {
  bodyY: number;
  tilt: number;
  handL: Vec;
  handR: Vec;
  cupLift: number;
  happy: boolean;
  cupSide: "l" | "r" | null;
}

export const REST_POSE: Pose = { bodyY: 0, tilt: 0, handL: { ...REST_L }, handR: { ...REST_R }, cupLift: 0, happy: false, cupSide: null };
/** How fast each part follows (per second): the body is snappy so the beat reads, hands a little looser. */
const RATE_BODY = 26;
const RATE_HAND = 16;
const RATE_CUP = 9;
/** `current` moved towards `target` over `dt` seconds (frame-rate independent). */
export const damp = (current: number, target: number, rate: number, dt: number): number => target + (current - target) * Math.exp(-rate * dt);

/** One step towards `t`; `dt` of `Infinity` jumps there (the still pose of "off"). */
export function approach(p: Pose, t: Target, dt: number): Pose {
  const step = (a: number, b: number, rate: number) => (Number.isFinite(dt) ? damp(a, b, rate, dt) : b);
  return {
    bodyY: step(p.bodyY, t.bodyY, RATE_BODY),
    tilt: step(p.tilt, t.tilt, RATE_BODY * 0.4),
    handL: { x: step(p.handL.x, t.handL.x, RATE_HAND), y: step(p.handL.y, t.handL.y, RATE_HAND) },
    handR: { x: step(p.handR.x, t.handR.x, RATE_HAND), y: step(p.handR.y, t.handR.y, RATE_HAND) },
    cupLift: step(p.cupLift, t.cupLift, RATE_CUP),
    happy: t.happy,
    cupSide: t.cupSide ?? p.cupSide,
  };
}

/** The shoulder as it sits after the body's drop and sway (the arms start from there). */
export function shoulderAt(base: Vec, pose: Pose): Vec {
  const a = (pose.tilt * Math.PI) / 180;
  const dx = base.x - PIVOT.x;
  const dy = base.y - PIVOT.y;
  return { x: PIVOT.x + dx * Math.cos(a) - dy * Math.sin(a), y: PIVOT.y + dx * Math.sin(a) + dy * Math.cos(a) + pose.bodyY };
}

/** An arm from a shoulder to a hand: a soft curve whose elbow hangs down. */
export function armPath(from: Vec, to: Vec): string {
  const sag = 26 + Math.min(40, Math.hypot(to.x - from.x, to.y - from.y) * 0.12);
  const cx = (from.x + to.x) / 2;
  const cy = Math.max(from.y, to.y) + sag * 0.6 - Math.abs(to.y - from.y) * 0.1;
  return `M${from.x.toFixed(1)} ${from.y.toFixed(1)}Q${cx.toFixed(1)} ${cy.toFixed(1)} ${to.x.toFixed(1)} ${to.y.toFixed(1)}`;
}

/** Where a spot on a platter's label is for a record turned `degrees` (the platter is squashed by the view). */
export function platterDot(deck: 0 | 1, degrees: number, radius: number): Vec {
  const a = (degrees * Math.PI) / 180;
  const c = PLATTER[deck];
  return { x: c.x + Math.cos(a) * radius, y: c.y + Math.sin(a) * radius * PLATTER_SQUASH };
}

/** How far the record has turned at `positionMs` (the real playhead: a scratch turns it back). */
export const recordDegrees = (positionMs: number): number => (positionMs / 1000) * DEG_PER_SECOND;

/** A deck is "scratched" when it is paused but its playhead moves, or while it's being nudged. */
export function isScratching(deck: DeckSnapshot, movedMs: number): boolean {
  const dragged = Math.abs(movedMs) > SCRATCH_MOVE_MS && Math.abs(movedMs) < SEEK_MS;
  return !!deck.track && ((!deck.playing && dragged) || (deck.playing && deck.bend !== 0));
}
/** A paused playhead that moves more than this between two frames is being dragged by a hand... */
const SCRATCH_MOVE_MS = 4;
/** ...unless it jumped this far, which is a seek (a hot cue, a click on the overview). */
const SEEK_MS = 1500;
/** Free-running beat phase at `FREE_BPM`. */
export const freePhase = (seconds: number): number => (seconds * (FREE_BPM / SECONDS_PER_MINUTE)) % FULL_CIRCLE;
