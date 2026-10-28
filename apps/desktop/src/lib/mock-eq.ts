// Development-only equalizer backend for mock.ts: the same RBJ math as
// homefi-core's `dsp` module (curves, auto preamp, taste) and a small fake
// AutoEq. ?eq starts with the equalizer on (HD 650 on "External Headphones");
// ?autoeqoffline makes AutoEq fail like it does without internet.
import type { EqBand, EqProfile, EqSettings, HeadphoneEntry, Tilt } from "./api";

const url = new URLSearchParams(location.search);
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

const hd650: EqProfile = {
  name: "Sennheiser HD 650",
  preampDb: -6.1,
  bands: [
    ["lowShelf", 105, 6.4, 0.7],
    ["peak", 8800, 5.1, 1.42],
    ["peak", 118, -3.1, 0.5],
    ["peak", 37, 0.7, 3.96],
    ["peak", 3169, -1.7, 3.89],
    ["highShelf", 10000, -2.1, 0.7],
    ["peak", 1227, -1.2, 2.53],
    ["peak", 2055, 1.2, 3.23],
    ["peak", 587, 0.4, 1.19],
    ["peak", 5332, -1.1, 5.75],
  ].map(([kind, freqHz, gainDb, q]) => ({ kind, freqHz, gainDb, q, enabled: true }) as EqBand),
  source: { kind: "autoEq", model: "Sennheiser HD 650", source: "oratory1990" },
};

const entries: HeadphoneEntry[] = [
  ["Sennheiser HD 650", "oratory1990", "over-ear", null],
  ["Sennheiser HD 650", "crinacle", "over-ear", "GRAS 43AG-7"],
  ["Sennheiser HD 600", "oratory1990", "over-ear", null],
  ["Sony WH-1000XM5", "oratory1990", "over-ear", null],
  ["Sony WH-1000XM5", "Rtings", "over-ear", "HMS II.3"],
  ["Apple AirPods Pro 2", "Rtings", "in-ear", "HMS II.3"],
  ["Moondrop Aria", "crinacle", "in-ear", "711"],
  ["Apple AirPods (3rd generation)", "Rtings", "earbud", "HMS II.3"],
].map(([name, source, kind, rig]) => ({ name: name!, source: source!, kind: kind!, rig, path: `${source}/${rig ? `${rig} ` : ""}${kind}/${name}` }));

let settings: EqSettings = url.has("eq")
  ? { enabled: true, autoPreamp: true, defaultProfile: null, perOutput: { "External Headphones": hd650 } }
  : { enabled: false, autoPreamp: true, defaultProfile: null, perOutput: {} };

// ------------------------------------------------------------------ math

const clamp = (v: number, lo: number, hi: number, nan: number) => (Number.isNaN(v) ? nan : Math.min(hi, Math.max(lo, v)));
const neutral = (b: EqBand) => !b.enabled || (["peak", "lowShelf", "highShelf"].includes(b.kind) && Math.abs(b.gainDb) < 0.05);

function coefficients(b: EqBand, fs: number): number[] {
  const f0 = Math.min(clamp(b.freqHz, 20, 20000, 1000), fs * 0.49);
  const q = clamp(b.q, 0.1, 10, 0.707);
  const a = 10 ** (clamp(b.gainDb, -24, 24, 0) / 40);
  const w0 = (2 * Math.PI * f0) / fs;
  const cos = Math.cos(w0);
  const alpha = Math.sin(w0) / (2 * q);
  const s = 2 * Math.sqrt(a) * alpha;
  let c: number[];
  switch (b.kind) {
    case "lowShelf":
      c = [a * (a + 1 - (a - 1) * cos + s), 2 * a * (a - 1 - (a + 1) * cos), a * (a + 1 - (a - 1) * cos - s), a + 1 + (a - 1) * cos + s, -2 * (a - 1 + (a + 1) * cos), a + 1 + (a - 1) * cos - s];
      break;
    case "highShelf":
      c = [a * (a + 1 + (a - 1) * cos + s), -2 * a * (a - 1 + (a + 1) * cos), a * (a + 1 + (a - 1) * cos - s), a + 1 - (a - 1) * cos + s, 2 * (a - 1 - (a + 1) * cos), a + 1 - (a - 1) * cos - s];
      break;
    case "lowPass":
      c = [(1 - cos) / 2, 1 - cos, (1 - cos) / 2, 1 + alpha, -2 * cos, 1 - alpha];
      break;
    case "highPass":
      c = [(1 + cos) / 2, -(1 + cos), (1 + cos) / 2, 1 + alpha, -2 * cos, 1 - alpha];
      break;
    default:
      c = [1 + alpha * a, -2 * cos, 1 - alpha * a, 1 + alpha / a, -2 * cos, 1 - alpha / a];
  }
  return [c[0] / c[3], c[1] / c[3], c[2] / c[3], c[4] / c[3], c[5] / c[3]];
}

function magnitudeDb([b0, b1, b2, a1, a2]: number[], f: number, fs: number): number {
  const w = (2 * Math.PI * Math.min(Math.max(f, 0), fs / 2)) / fs;
  const re = (x0: number, x1: number, x2: number) => x0 + x1 * Math.cos(w) + x2 * Math.cos(2 * w);
  const im = (x1: number, x2: number) => -(x1 * Math.sin(w) + x2 * Math.sin(2 * w));
  const num = re(b0, b1, b2) ** 2 + im(b1, b2) ** 2;
  const den = re(1, a1, a2) ** 2 + im(a1, a2) ** 2;
  return 10 * Math.log10(Math.max(num / Math.max(den, 1e-300), 1e-30));
}

function response(p: EqProfile, freqs: number[], fs = 48000): number[] {
  const stages = p.bands.slice(0, 10).filter((b) => !neutral(b)).map((b) => coefficients(b, fs || 48000));
  return freqs.map((f) => stages.reduce((sum, c) => sum + magnitudeDb(c, f, fs || 48000), 0));
}

function logFreqs(n: number): number[] {
  if (n <= 0) return [];
  if (n === 1) return [20];
  return Array.from({ length: n }, (_, i) => Math.exp(Math.log(20) + ((Math.log(20000) - Math.log(20)) * i) / (n - 1)));
}

function autoPreamp(p: EqProfile): number {
  if (p.bands.every(neutral)) return 0;
  const f = logFreqs(512);
  const peak = Math.max(0, ...response(p, f, 44100), ...response(p, f, 192000));
  return -Math.ceil(peak * 10) / 10;
}

function withTaste(p: EqProfile, t: Tilt): EqProfile {
  const out: EqProfile = { ...p, bands: p.bands.map((b) => ({ ...b })) };
  const wanted: [EqBand["kind"], number, number, number][] = [
    ["lowShelf", 105, t.bassDb, 0.7],
    ["highShelf", 10000, t.trebleDb, 0.7],
    ["peak", 3000, t.clarityDb ?? 0, 0.9],
  ];
  let changed = false;
  for (const [kind, freqHz, gainDb, q] of wanted) {
    if (!(Math.abs(gainDb) >= 0.05)) continue;
    changed = true;
    const near = out.bands.find((b) => b.enabled && b.kind === kind && Math.abs(Math.log2(b.freqHz / freqHz)) <= 1 && (kind !== "peak" || Math.abs(Math.log2(b.q / q)) <= 0.6));
    if (near) near.gainDb = clamp(near.gainDb + gainDb, -24, 24, 0);
    else if (out.bands.length < 10) out.bands.push({ kind, freqHz, gainDb, q, enabled: true });
  }
  if (changed) out.preampDb = Math.min(out.preampDb, autoPreamp(out));
  return out;
}

function profileFor(output: string): EqProfile | null {
  return settings.perOutput[output] ?? settings.defaultProfile ?? null;
}

// --------------------------------------------------------------- handlers

export const eqMock = {
  settings: () => settings,
  /** `eq_active` / `eq_profile` of the player for `device`. */
  playerFlags(device: string) {
    const p = profileFor(device);
    const active = settings.enabled && !!p && !(p.bands.every(neutral) && (settings.autoPreamp || Math.abs(p.preampDb) < 0.05));
    return { eq_active: active, eq_profile: active ? p!.name : null };
  },
  set(next: EqSettings): EqSettings {
    settings = next;
    return settings;
  },
  handlers: {
    eq_get: () => settings,
    eq_response: ({ profile, freqs, sampleRate }: { profile: EqProfile; freqs: number[]; sampleRate: number }) => response(profile, freqs, sampleRate),
    eq_log_freqs: ({ n }: { n: number }) => logFreqs(n),
    eq_auto_preamp: ({ profile }: { profile: EqProfile }) => autoPreamp(profile),
    eq_with_taste: ({ profile, tilt }: { profile: EqProfile; tilt: Tilt }) => withTaste(profile, tilt),
    autoeq_search: async ({ query }: { query: string }) => {
      await wait(300);
      if (url.has("autoeqoffline")) throw "Couldn't download the headphone list from AutoEq. Check your connection and try again.";
      const words = String(query).toLowerCase().split(/\s+/).filter(Boolean);
      const squash = (s: string) => s.toLowerCase().replace(/[^a-z0-9]/g, "");
      return entries.filter((e) => words.length > 0 && words.every((w) => squash(e.name).includes(squash(w)) || e.source.toLowerCase().includes(w)));
    },
    autoeq_profile: async ({ entry }: { entry: HeadphoneEntry }) => {
      await wait(400);
      if (url.has("autoeqoffline")) throw "Couldn't download this headphone's profile from AutoEq. Check your connection and try again.";
      return { ...hd650, name: entry.name, source: { kind: "autoEq", model: entry.name, source: entry.source } } as EqProfile;
    },
  },
};
