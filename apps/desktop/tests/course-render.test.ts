// The course mountain's pure geometry (docs/MOUNTAIN.md): resampling, summits, ticks, past colour.
// Run: `pnpm test` (node's own runner).
import { describe, it } from "node:test";
import { strict as assert } from "node:assert";
import { dimmed, layoutCourse, nextDifferent, partIndexAt, plotPad, ridgeY, summitsOf, tickStep, xOfT, type CourseModel } from "../src/lib/course-render.ts";

const parts = [
  { kind: "warm_up", start_s: 0, seconds: 300 },
  { kind: "seated_climb", start_s: 300, seconds: 300 },
  { kind: "seated_climb", start_s: 600, seconds: 300 },
  { kind: "recovery", start_s: 900, seconds: 300 },
];
const profile = [
  { t_s: 0, elevation: 0 },
  { t_s: 300, elevation: 0.2 },
  { t_s: 600, elevation: 0.7 },
  { t_s: 900, elevation: 1 },
  { t_s: 1200, elevation: 0.3 },
];
const model: CourseModel = { profile, parts, total_s: 1200 };

describe("course geometry", () => {
  it("samples about every two pixels and keeps the data's own range", () => {
    const g = layoutCourse(model, 700, 160);
    assert.equal(g.M, 350);
    assert.equal(g.xs.length, g.M + 2, "a run-out at each end");
    assert.equal(g.xs[0], 0);
    assert.equal(g.xs[g.xs.length - 1], 700);
    assert.ok(g.emax > 0.99 && g.emin < 0.01, "the summits are not shaved");
    assert.ok(g.ys0.every((y) => y >= g.yHi - 1e-6 && y <= g.yLo + 1e-6));
  });

  it("maps time to the padded plot and the ridge sits on the textured samples", () => {
    const g = layoutCourse(model, 700, 160);
    assert.equal(g.padX, plotPad(700, 160));
    assert.equal(xOfT(g, 0, 1200), g.padX);
    assert.equal(xOfT(g, 1200, 1200), 700 - g.padX);
    assert.ok(Math.abs(ridgeY(g, g.xs[10]!) - g.ys[10]!) < 1e-6);
  });

  it("minis drop the ribbon and the tick strip", () => {
    const g = layoutCourse(model, 112, 44);
    assert.ok(g.mini && g.tick === 0 && g.band === 0 && g.floor === 44);
  });

  it("finds the next part of a different kind", () => {
    assert.equal(nextDifferent(parts, 1), 3, "split parts of one kind are not next");
    assert.equal(nextDifferent(parts, 3), -1);
    assert.equal(partIndexAt(parts, 650), 2);
  });

  it("labels at most three summits, apart and prominent", () => {
    const e = Array.from({ length: 300 }, (_, i) => 0.5 + 0.5 * Math.sin((i / 300) * Math.PI * 6));
    const s = summitsOf(e);
    assert.ok(s.length >= 1 && s.length <= 3);
    for (let i = 1; i < s.length; i++) assert.ok(Math.abs(s[i]! - s[i - 1]!) >= 300 * 0.12);
    assert.deepEqual(summitsOf(Array.from({ length: 300 }, () => 0.4)), [], "a flat ride has none");
  });

  it("picks the tick step that gives at most nine labels (four when narrow)", () => {
    assert.equal(tickStep(1800, 700), 300);
    assert.equal(tickStep(2700, 700), 300);
    assert.equal(tickStep(1200, 300), 300);
    assert.equal(tickStep(5400, 700), 600);
  });

  it("dims toward slate, not toward black", () => {
    const [r, g, b] = dimmed([255, 138, 61]);
    assert.ok(b > r * 0.4, "stays bluish");
    assert.ok(r < 200 && g < 150);
  });
});
