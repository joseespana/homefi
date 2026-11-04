// Synthetic onset streams against the beat follower. Run: `pnpm test`
// (node's own runner, no extra dependency).
import { describe, it } from "node:test";
import { strict } from "node:assert";
import { BEAT, BeatFollower, OnsetDetector } from "../src/lib/beat.ts";

const FRAME = 1 / 60;

/** A tiny seeded random, so the jitter is the same on every run. */
function rng(seed: number) {
  let s = seed;
  return () => ((s = (s * 1664525 + 1013904223) >>> 0) / 2 ** 32);
}

interface Hit {
  at: number;
  strength: number;
}

/** Plays hits into a follower frame by frame; `probe` sees it after each frame. */
function play(f: BeatFollower, hits: Hit[], seconds: number, from = 0, probe?: (t: number) => void) {
  let i = hits.findIndex((h) => h.at >= from);
  for (let t = from; t < from + seconds; t += FRAME) {
    f.advance(FRAME);
    while (i >= 0 && i < hits.length && hits[i].at <= t) f.onset(hits[i++].strength);
    probe?.(t);
  }
}

/** Evenly spaced hits at `bpm`, each moved by up to ±`jitter` seconds. */
function steady(bpm: number, from: number, to: number, jitter: number, random: () => number, strength = 0.6): Hit[] {
  const gap = 60 / bpm;
  const hits: Hit[] = [];
  for (let t = from; t < to; t += gap) hits.push({ at: t + (random() * 2 - 1) * jitter, strength });
  return hits;
}

/** How far the follower's beat is from the true beat grid, in beats (−0.5..0.5). */
function phaseError(f: BeatFollower, bpm: number, t: number, origin: number) {
  const truth = ((t - origin) * bpm) / 60;
  const e = f.beat - truth;
  return e - Math.round(e);
}

describe("BeatFollower", () => {
  it("locks onto a steady 120 bpm with jitter", () => {
    const f = new BeatFollower();
    const hits = steady(120, 0.1, 30, 0.02, rng(1));
    play(f, hits, 30);
    strict.ok(f.confidence > 0.7, `confidence ${f.confidence}`);
    strict.ok(Math.abs(f.bpm - 120) < 3, `bpm ${f.bpm}`);
    strict.ok(Math.abs(f.estimate - 120) <= 7, `estimate ${f.estimate}`);
    // the beat sits on the hits (within a tenth of a beat)
    const e = phaseError(f, 120, 30, 0.1);
    strict.ok(Math.abs(e) < 0.1, `phase error ${e}`);
  });

  it("keeps the phase from drifting after the first seconds", () => {
    const f = new BeatFollower();
    const hits = steady(120, 0.1, 60, 0.015, rng(2));
    let worst = 0;
    play(f, hits, 60, 0, (t) => {
      if (t > 8) worst = Math.max(worst, Math.abs(phaseError(f, 120, t, 0.1)));
    });
    strict.ok(worst < 0.15, `worst phase error ${worst}`);
  });

  it("follows a tempo change from 120 to 140", () => {
    const f = new BeatFollower();
    const random = rng(3);
    const hits = [...steady(120, 0.1, 20, 0.015, random), ...steady(140, 20.1, 60, 0.015, random)];
    play(f, hits, 60);
    strict.ok(f.confidence > 0.6, `confidence ${f.confidence}`);
    strict.ok(Math.abs(f.bpm - 140) < 5, `bpm ${f.bpm}`);
    strict.ok(Math.abs(f.estimate - 140) <= 7, `estimate ${f.estimate}`);
    const e = phaseError(f, 140, 60, 20.1);
    strict.ok(Math.abs(e) < 0.12, `phase error ${e}`);
  });

  it("ignores hits far from the grid and loses its confidence on pure noise", () => {
    const f = new BeatFollower();
    const random = rng(4);
    play(f, steady(120, 0.1, 15, 0.01, random), 15);
    const locked = f.confidence;
    strict.ok(locked > 0.7);
    // off-beat hits (the hi-hat in between) do not move the grid
    const before = f.beat;
    f.onset(0.5); // lands on a beat boundary
    const offGrid = Math.round(f.beat) + 0.5;
    f.beat = offGrid;
    const bpm = f.bpm;
    f.onset(0.5);
    strict.equal(f.beat, offGrid);
    strict.equal(f.bpm, bpm);
    void before;

    // random hits, six a second, for a while: the mascot must let go
    const noise: Hit[] = [];
    for (let t = 15; t < 40; t += 0.12 + random() * 0.3) noise.push({ at: t, strength: 0.5 });
    play(f, noise, 25, 15);
    strict.ok(f.confidence < BEAT.minConfidence, `confidence ${f.confidence}`);
  });

  it("fades out in silence", () => {
    const f = new BeatFollower();
    play(f, steady(120, 0.1, 10, 0.01, rng(5)), 10);
    strict.ok(f.confidence > 0.7);
    play(f, [], 6, 10);
    strict.ok(f.confidence < BEAT.minConfidence, `confidence ${f.confidence}`);
    // and it keeps ticking, so a new beat can be picked up again
    const before = f.beat;
    play(f, [], 1, 16);
    strict.ok(f.beat > before);
  });

  it("finds the downbeat from the strongest hit in the bar", () => {
    const f = new BeatFollower();
    const hits: Hit[] = [];
    const gap = 0.5;
    for (let n = 0; n < 80; n++) hits.push({ at: 0.1 + n * gap, strength: n % 4 === 2 ? 0.95 : 0.5 });
    play(f, hits, 40);
    // the loud hit is every fourth, so beats ≡ hit index mod 4 start the bar
    const loud = Math.round(f.beat - ((40 - (0.1 + 78 * gap)) * f.bpm) / 60) & 3;
    strict.equal(f.downbeat, ((loud % 4) + 4) % 4);
    strict.ok(f.accents > 10, `accents ${f.accents}`);
  });

  it("measures intensity against the recent past and names the section", () => {
    const f = new BeatFollower();
    for (let i = 0; i < 60 * 20; i++) f.loudness(0.5, FRAME);
    strict.ok(Math.abs(f.intensity - 0.4) < 0.05, `steady ${f.intensity}`);
    strict.equal(f.section, "groove");
    for (let i = 0; i < 60; i++) f.loudness(0.1, FRAME);
    strict.equal(f.section, "quiet");
    for (let i = 0; i < 60 * 3; i++) f.loudness(1, FRAME);
    strict.equal(f.section, "loud");
  });

  it("fires a drop after a quiet stretch, at most one per eight bars", () => {
    const f = new BeatFollower();
    const hits = steady(120, 0.1, 80, 0.01, rng(6));
    // quiet for 12 s (6 bars), a jump, quiet again, a second jump 6 s later, then a long quiet and a third
    const loud: [number, number][] = [[24, 26], [32, 34], [60, 70]];
    const dropsAt: number[] = [];
    let seen = 0;
    play(f, hits, 80, 0, (now) => {
      f.loudness(loud.some(([a, b]) => now >= a && now < b) ? 0.9 : 0.12, FRAME);
      if (f.drops !== seen) dropsAt.push(Math.round(now)), (seen = f.drops);
    });
    // the second jump comes too soon after the first: only two drops, not three
    strict.equal(f.drops, 2, `drops at ${dropsAt}`);
    strict.ok(dropsAt[0] >= 24 && dropsAt[0] <= 26, `first at ${dropsAt[0]}`);
    strict.ok(dropsAt[1] >= 60 && dropsAt[1] <= 63, `second at ${dropsAt[1]}`);
  });

  it("does not drop on a steady loud song", () => {
    const f = new BeatFollower();
    play(f, steady(120, 0.1, 40, 0.01, rng(7)), 40, 0, () => f.loudness(0.8, FRAME));
    strict.equal(f.drops, 0);
  });

  it("starts again from nothing after reset", () => {
    const f = new BeatFollower();
    play(f, steady(140, 0.1, 10, 0.01, rng(8)), 10);
    f.reset();
    strict.equal(f.confidence, 0);
    strict.equal(f.bpm, 120);
    strict.equal(f.drops, 0);
  });
});

describe("OnsetDetector", () => {
  const spectrum = (low: number, mid = 0.1) => Array.from({ length: 48 }, (_, i) => (i < 6 ? low : i < 20 ? mid : 0.05));

  it("hears kicks on a pulsing low band and nothing in a steady tone", () => {
    const d = new OnsetDetector();
    let hits = 0;
    // 2 s of a steady tone, then 4 s of a kick every 0.5 s (30 fps)
    for (let i = 0; i < 60; i++) hits += d.push(spectrum(0.4), 1 / 30) ? 1 : 0;
    strict.equal(hits, 0);
    for (let i = 0; i < 120; i++) hits += d.push(spectrum(i % 15 === 0 ? 0.9 : 0.4), 1 / 30) ? 1 : 0;
    strict.ok(hits >= 7 && hits <= 9, `hits ${hits}`);
  });

  it("rests between hits", () => {
    const d = new OnsetDetector();
    for (let i = 0; i < 30; i++) d.push(spectrum(0.2), 1 / 30);
    strict.ok(d.push(spectrum(0.9), 1 / 30) > 0);
    // a second jump 33 ms later is the same hit
    strict.equal(d.push(spectrum(0.2), 1 / 30), 0);
    strict.equal(d.push(spectrum(0.9), 1 / 30), 0);
  });
});
