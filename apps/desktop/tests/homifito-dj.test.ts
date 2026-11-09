// Homifito DJ's choreography: beat phase from the playhead, who is master, who is cued, what a pad
// makes him do, and that a calm / still motion level never moves the body. Run: `pnpm test`.
import { describe, it } from "node:test";
import { strict } from "node:assert";
import { REST_POSE, aim, approach, beatDip, beatPhase, cueingDeck, isScratching, masterDeck, reactionOf, type StageInput } from "../src/lib/homifito-dj.ts";

const track = { title: "t", duration_ms: 200_000, sample_rate: 44_100, codec: "flac", bpm: 120, first_beat_ms: 100, beat_ms: 500, has_grid: true, confidence: 0.9, loudness_db: -9, auto_gain_db: 0, bytes: 1 };
const deck = (patch: Record<string, unknown> = {}) => ({ track, loading: false, error: null, playing: false, position_ms: 0, bend: 0, ...patch }) as never;
const snap = (a: Record<string, unknown>, b: Record<string, unknown>, crossfader = 0) => ({ decks: [deck(a), deck(b)], crossfader }) as never;

const input = (patch: Partial<StageInput> = {}): StageInput => ({ level: "full", phase: 0.5, beatIndex: 0, playing: [true, false], scratching: [false, false], cueing: null, crossfader: 0, xfHand: null, reaction: null, seconds: 1, ...patch });

describe("beat", () => {
  it("phase is 0 on a beat and climbs to 1, also before the first beat", () => {
    strict.equal(beatPhase(deck(), 100), 0);
    strict.ok(Math.abs((beatPhase(deck(), 350) ?? 0) - 0.5) < 1e-9);
    strict.ok(Math.abs((beatPhase(deck(), 0) ?? 0) - 0.8) < 1e-9);
  });
  it("is unknown without a grid", () => {
    strict.equal(beatPhase(deck({ track: { ...track, has_grid: false } }), 1000), null);
  });
  it("the body drops fast on the beat and eases back", () => {
    strict.equal(beatDip(0), 0);
    strict.ok(beatDip(0.12) === 1);
    strict.ok(beatDip(0.5) < beatDip(0.2));
    strict.ok(beatDip(0.999) < 0.01);
  });
});

describe("decks", () => {
  it("master is the playing deck, or the crossfader's side when both play", () => {
    strict.equal(masterDeck(snap({ playing: true }, {})), "a");
    strict.equal(masterDeck(snap({}, { playing: true })), "b");
    strict.equal(masterDeck(snap({ playing: true }, { playing: true }, 0.8)), "b");
    strict.equal(masterDeck(snap({}, {})), null);
  });
  it("cues the loaded deck that waits while the other plays", () => {
    strict.equal(cueingDeck(snap({ playing: true }, {})), "b");
    strict.equal(cueingDeck(snap({ playing: true }, { track: null })), null);
    strict.equal(cueingDeck(snap({ playing: true }, { playing: true })), null);
  });
  it("scratching is a paused deck dragged, not a seek", () => {
    strict.equal(isScratching(deck(), 30), true);
    strict.equal(isScratching(deck(), 0), false);
    strict.equal(isScratching(deck(), 40_000), false);
    strict.equal(isScratching(deck({ playing: true, bend: 1 }), 0), true);
  });
});

describe("what he does", () => {
  it("pads map to a reaction", () => {
    strict.equal(reactionOf("air_horn"), "hands_up");
    strict.equal(reactionOf("drop"), "jump");
    strict.equal(reactionOf("scratch"), "scratch");
  });
  it("hands up on an air horn, a jump leaves the ground", () => {
    const up = aim(input({ reaction: { kind: "hands_up", progress: 0.3 } }));
    strict.ok(up.handL.y < 100 && up.happy);
    const jump = aim(input({ reaction: { kind: "jump", progress: 0.5 } }));
    strict.ok(jump.bodyY < -20);
  });
  it("a hand follows the crossfader", () => {
    const left = aim(input({ crossfader: 0, xfHand: 0 })).handL.x;
    const right = aim(input({ crossfader: 1, xfHand: 0 })).handL.x;
    strict.ok(right > left + 100);
  });
  it("subtle only sways a little and off stays put", () => {
    const full = aim(input({ phase: 0.12 })).bodyY;
    const subtle = aim(input({ level: "subtle", phase: 0.12 })).bodyY;
    const off = aim(input({ level: "off", phase: 0.12, reaction: { kind: "jump", progress: 0.5 } }));
    strict.ok(subtle > 0 && subtle < full * 0.5);
    strict.equal(off.bodyY, 0);
    strict.equal(off.handL.y, REST_POSE.handL.y);
  });
  it("lifts the cup opposite the cued deck", () => {
    strict.equal(aim(input({ cueing: "b" })).cupSide, "l");
    strict.equal(aim(input({ cueing: "a" })).cupSide, "r");
  });
  it("eases towards a target without overshooting, or jumps when asked", () => {
    const t = aim(input({ phase: 0.12 }));
    const next = approach(REST_POSE, t, 1 / 60);
    strict.ok(next.bodyY > 0 && next.bodyY < t.bodyY);
    strict.equal(approach(REST_POSE, t, Infinity).bodyY, t.bodyY);
  });
});
