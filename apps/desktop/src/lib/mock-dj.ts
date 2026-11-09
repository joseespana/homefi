// Development-only DJ backend (mock.ts spreads `handlers`): the same shapes as src-tauri/src/dj.rs,
// with a small simulated mixer so the screen moves. Flags (all open the DJ screen):
//   ?dj            DJ mode on: A playing "Midnight City", B ready ("Dreams")
//   ?dj=mixing     the same, mixing A to B (35 %)
//   ?dj=loading    B finding the beat        ?dj=error   B could not be opened
//   ?dj=empty      both decks empty          ?dj=off     DJ mode not started yet
//   ?djfull        all controls (Simple view off)      ?djtips    the first-run card
//   ?djedit        the "Edit layout" panel open        ?djsetups  three saved setups (and the menu's data)
//   ?djsheet       the keyboard sheet open             ?djdrawer  (narrow) the crate drawer open
//   Homifito DJ:   ?hdj=off|booth|big  ?hdjplace=bottom  ?hdjlook (the setting, settings.dj.homifito)
//                  ?dj=mixlive  a mix to B that runs for real      ?djscratch  deck B dragged back and forth
//                  ?djpad=air_horn|drop|scratch|...  that pad fires every 4 s (Homifito reacts)
import type { DeckId, DeckSnapshot, DjControl, DjOverview, DjPick, DjSettings, DjSetupApplied, DjSetupSummary, DjSnapshot, DjState, DjTrackInfo } from "./dj-api";
import { DEFAULT_DJ_SETTINGS, SFX_KINDS, type SfxKind } from "./dj-api";

const url = new URLSearchParams(location.search);
const flag = url.get("dj");
const active = url.has("dj");
const TICK_MS = 50;
const SLICES_PER_SECOND = 50;
const PAD_MS = 600;
const autoPad = SFX_KINDS.indexOf(url.get("djpad") as SfxKind);
const AUTO_PAD_EVERY_MS = 4000;
const AUTO_PAD_FIRST_MS = 2500;
let nextAutoPad = AUTO_PAD_FIRST_MS;
const scratching = url.has("djscratch");

if (active && !location.hash) location.hash = "#dj";

let settings: DjSettings = { ...DEFAULT_DJ_SETTINGS, layout: structuredClone(DEFAULT_DJ_SETTINGS.layout), simple_view: !url.has("djfull"), show_tips: url.has("djtips") };
// dev screenshots: ?hdj=off|booth|big  ?hdjplace=bottom  ?hdjlook
const hdjMode = url.get("hdj");
if (hdjMode === "off" || hdjMode === "booth" || hdjMode === "big") settings.homifito = { ...settings.homifito, mode: hdjMode };
if (url.get("hdjplace") === "bottom") settings.homifito = { ...settings.homifito, place: "bottom" };
if (url.has("hdjlook")) settings.homifito = { ...settings.homifito, look: true };
let on = active && flag !== "off";

// ------------------------------------------------------------------ setups (src-tauri/src/dj_setups.rs)
const DAY_MS = 86_400_000;
interface MockSetup extends DjSetupSummary {
  dj: DjSettings;
}
let setupSeq = 0;
const makeSetup = (name: string, dj: DjSettings, ago_days = 0): MockSetup => ({ id: `setup-${++setupSeq}`, name, created_ms: Date.now() - ago_days * DAY_MS, updated_ms: Date.now() - ago_days * DAY_MS, dj: structuredClone(dj) });
const sections = (order: string[], hidden: string[] = []) => order.map((kind) => ({ kind, visible: !hidden.includes(kind) })) as DjSettings["layout"]["sections"];
let setups: MockSetup[] = url.has("djsetups")
  ? [
      makeSetup("Party night", { ...DEFAULT_DJ_SETTINGS, simple_view: true, layout: { sections: sections(["deck_a", "mixer", "deck_b", "sampler", "crate"], ["crate"]), pads: ["air_horn", "siren", "air_horn", "rewind", "drop", "drop", "riser", "hit"] } }, 12),
      makeSetup("Headphones practice", { ...DEFAULT_DJ_SETTINGS, simple_view: false, layout: { sections: sections(["deck_a", "deck_b", "mixer", "crate", "sampler"]), pads: [...DEFAULT_DJ_SETTINGS.layout.pads] } }, 3),
      makeSetup("Tablet layout", { ...DEFAULT_DJ_SETTINGS, layout: { sections: sections(["mixer", "deck_a", "deck_b", "sampler", "crate"], ["crate", "sampler"]), pads: [...DEFAULT_DJ_SETTINGS.layout.pads] } }, 0),
    ]
  : [];
const summary = (s: MockSetup): DjSetupSummary => ({ id: s.id, name: s.name, created_ms: s.created_ms, updated_ms: s.updated_ms });
const setupById = (id: string) => {
  const found = setups.find((s) => s.id === id);
  if (!found) throw "That setup was not found.";
  return found;
};
const setupsChanged = () => send("dj-setups", null);

const idx = (d: DeckId) => (d === "a" ? 0 : 1);
const cover = (hue: number) =>
  `data:image/svg+xml;utf8,${encodeURIComponent(`<svg xmlns='http://www.w3.org/2000/svg' width='200' height='200'><defs><linearGradient id='g' x1='0' y1='0' x2='1' y2='1'><stop offset='0' stop-color='hsl(${hue} 85% 58%)'/><stop offset='1' stop-color='hsl(${(hue + 60) % 360} 70% 22%)'/></linearGradient></defs><rect width='200' height='200' fill='url(#g)'/><circle cx='100' cy='100' r='60' fill='none' stroke='white' stroke-opacity='.35' stroke-width='9'/></svg>`)}`;

function info(title: string, bpm: number, seconds: number, firstBeat = 120): DjTrackInfo {
  return { title, duration_ms: seconds * 1000, sample_rate: 44100, codec: "flac", bpm, first_beat_ms: firstBeat, beat_ms: 60000 / bpm, has_grid: true, confidence: 0.8, loudness_db: -9, auto_gain_db: -2, bytes: seconds * 44100 * 8 };
}
function emptyDeck(): DeckSnapshot {
  return { track: null, loading: false, error: null, playing: false, position_ms: 0, cue_ms: 0, hot_cues: Array(8).fill(null), loop_ms: null, tempo_percent: 0, current_bpm: 0, synced: false, bend: 0, trim_db: 0, eq_db: [0, 0, 0], eq_kill: [false, false, false], filter: 0, volume: 1, peak: [0, 0] };
}
function loaded(t: DjTrackInfo): DeckSnapshot {
  return { ...emptyDeck(), track: t, current_bpm: t.bpm };
}

const picks: [DjPick | null, DjPick | null] = [null, null];
const seeds = [1, 4];
let snap: DjSnapshot = {
  running: true,
  output: { device: "External Headphones", sample_rate: 48000, channels: 2, buffer_frames: 512 },
  device_lost: null,
  decks: [emptyDeck(), emptyDeck()],
  crossfader: 0,
  crossfader_curve: "smooth",
  master: 0.8,
  master_peak: [0, 0],
  limiting: false,
  auto_mix: null,
  sampler_volume: 0.8,
  pads_active: Array(8).fill(false),
  memory_used_mb: 0,
  memory_limit_mb: 1024,
};
const padUntil = Array(8).fill(0);
let send: (event: string, payload: unknown) => void = () => {};

function put(deck: DeckId, pick: DjPick, t: DjTrackInfo, d: Partial<DeckSnapshot> = {}) {
  picks[idx(deck)] = { ...pick, deck };
  snap.decks[idx(deck)] = { ...loaded(t), ...d };
}

if (on && flag !== "empty") {
  const a = info("Midnight City", 105, 240);
  put("a", { source: { kind: "path", path: "/mock/M83/Hurry Up/1.flac#260" }, title: "Midnight City", artist: "M83 · Hurry Up, We're Dreaming", cover_url: cover(200), duration_ms: 240000, deck: "a" }, a, {
    playing: true,
    position_ms: 72_000,
    tempo_percent: 1.2,
    current_bpm: 106.3,
    synced: true,
    hot_cues: [8_000, 52_800, 146_400, null, null, null, null, null],
    loop_ms: url.has("djloop") ? [105_600, 107_885] : null,
    eq_db: [-30, 0, 2.5],
    eq_kill: [true, false, false],
    cue_ms: 8_000,
  });
  const dreams = { source: { kind: "path" as const, path: "/mock/Fleetwood Mac/Rumours/2.flac#30" }, title: "Dreams", artist: "Fleetwood Mac · Rumours", cover_url: cover(320), duration_ms: 257000, deck: "b" as const };
  if (flag === "loading") {
    picks[1] = dreams;
    snap.decks[1] = { ...emptyDeck(), loading: true };
  } else if (flag === "error") {
    picks[1] = dreams;
    snap.decks[1] = { ...emptyDeck(), error: "this song needs about 1400 MB of memory, and DJ mode may use 1024 MB. Raise the DJ memory limit in the settings or pick a shorter song" };
  } else {
    put("b", dreams, info("Dreams", 120, 257, 40), { tempo_percent: -0.8, current_bpm: 119.0, filter: 0.35 });
  }
  if (flag === "mixlive") {
    snap.auto_mix = { to: "b", beats: 16, armed: false, progress: 0 };
    snap.decks[1].playing = true;
    snap.decks[1].position_ms = 31_000;
  }
  if (flag === "mixing") {
    snap.auto_mix = { to: "b", beats: 16, armed: false, progress: 0.35 };
    snap.crossfader = 0.35;
    snap.decks[1].playing = true;
    snap.decks[1].position_ms = 31_000;
  }
}

function state(): DjState {
  return { on, snapshot: on ? snap : null, decks: [...picks] as DjState["decks"] };
}

/** A deterministic, song-like overview: verses, choruses, a break. */
function overview(deck: number): DjOverview | null {
  const t = snap.decks[deck].track;
  if (!t) return null;
  const n = Math.floor((t.duration_ms / 1000) * SLICES_PER_SECOND);
  const seed = seeds[deck];
  const bytes = new Uint8Array(n * 4);
  const rnd = (i: number) => {
    const x = Math.sin(i * 127.1 + seed * 311.7) * 43758.5453;
    return x - Math.floor(x);
  };
  const beatSlices = (t.beat_ms / 1000) * SLICES_PER_SECOND;
  for (let i = 0; i < n; i++) {
    const p = i / n;
    const section = 0.45 + 0.4 * Math.abs(Math.sin(p * 9 + seed)) * (p > 0.55 && p < 0.62 ? 0.3 : 1);
    const onBeat = (i % beatSlices) / beatSlices < 0.18 ? 1 : 0.55;
    const low = Math.min(1, section * onBeat * (0.8 + 0.2 * rnd(i)));
    const mid = Math.min(1, section * (0.45 + 0.35 * rnd(i + 3)));
    const high = Math.min(1, section * (0.2 + 0.3 * rnd(i + 9)));
    bytes.set([Math.max(low, mid, high) * 255, low * 255, mid * 255, high * 255], i * 4);
  }
  let raw = "";
  for (let i = 0; i < bytes.length; i++) raw += String.fromCharCode(bytes[i]);
  return { duration_ms: t.duration_ms, bpm: t.bpm, first_beat_ms: t.first_beat_ms, beat_ms: t.beat_ms, confidence: t.confidence, slices_per_second: SLICES_PER_SECOND, slices: btoa(raw) };
}

/** homefi_core::dj::keymap::default_bindings, for the mock. */
function keymap() {
  const deck: [string, string, string][] = [
    ["play", "q", "p"], ["cue", "a", "l"], ["sync", "g", ";"], ["loop_1", "w", "o"], ["loop_2", "e", "i"], ["loop_4", "r", "u"], ["loop_8", "t", "y"], ["loop_exit", "s", "k"], ["bend_back", "d", "j"],
  ];
  const b: { action: string; key: string }[] = [];
  for (const [name, a, bk] of deck) b.push({ action: `deck_a.${name}`, key: a }, { action: `deck_b.${name}`, key: bk });
  ["1", "2", "3", "4"].forEach((k, i) => b.push({ action: `deck_a.hot_cue_${i + 1}`, key: k }, { action: `deck_a.hot_cue_${i + 5}`, key: `Shift+${k}` }));
  ["7", "8", "9", "0"].forEach((k, i) => b.push({ action: `deck_b.hot_cue_${i + 1}`, key: k }, { action: `deck_b.hot_cue_${i + 5}`, key: `Shift+${k}` }));
  b.push({ action: "deck_a.bend_forward", key: "f" }, { action: "deck_b.bend_forward", key: "h" }, { action: "crossfader.left", key: "[" }, { action: "crossfader.right", key: "]" }, { action: "crossfader.center", key: "\\" }, { action: "auto_mix.to_a", key: "c" }, { action: "auto_mix.to_b", key: "v" });
  for (let p = 1; p <= 8; p++) b.push({ action: `sampler.${p}`, key: `Alt+${p}` });
  const bindings = b.map((x) => ({ ...x, key: x.action in settings.key_overrides ? settings.key_overrides[x.action] : x.key })).filter((x) => x.key);
  return { bindings, conflicts: [] };
}

function control(c: DjControl) {
  const d = "deck" in c ? snap.decks[idx(c.deck)] : null;
  const set = (patch: Partial<DeckSnapshot>) => d && Object.assign(d, patch);
  switch (c.type) {
    case "play":
      return set({ playing: !!d?.track });
    case "pause":
      return set({ playing: false });
    case "toggle_play":
      return set({ playing: !d?.playing && !!d?.track });
    case "cue":
      return c.pressed && set({ playing: false, position_ms: d?.cue_ms ?? 0 });
    case "seek":
      return set({ position_ms: Math.max(0, c.ms) });
    case "nudge":
      return set({ position_ms: Math.max(0, (d?.position_ms ?? 0) + c.ms) });
    case "bend":
      return set({ bend: c.amount });
    case "hot_cue":
      return d && (d.hot_cues[c.index] === null ? (d.hot_cues[c.index] = d.position_ms) : set({ position_ms: d.hot_cues[c.index]! }));
    case "set_hot_cue":
      return d && (d.hot_cues[c.index] = d.position_ms);
    case "clear_hot_cue":
      return d && (d.hot_cues[c.index] = null);
    case "loop_beats":
      return d?.track && set({ loop_ms: [d.position_ms, d.position_ms + c.beats * d.track.beat_ms] });
    case "loop_exit":
      return set({ loop_ms: null });
    case "tempo":
      return d?.track && set({ tempo_percent: c.percent, current_bpm: d.track.bpm * (1 + c.percent / 100), synced: false });
    case "sync": {
      const other = snap.decks[1 - idx(c.deck)];
      return d?.track && other.track && set({ synced: true, current_bpm: other.current_bpm, tempo_percent: (other.current_bpm / d.track.bpm - 1) * 100 });
    }
    case "sync_off":
      return set({ synced: false });
    case "trim":
      return set({ trim_db: c.db });
    case "eq":
      return d && (d.eq_db[{ low: 0, mid: 1, high: 2 }[c.band]] = c.db);
    case "eq_kill":
      return d && (d.eq_kill[{ low: 0, mid: 1, high: 2 }[c.band]] = c.on);
    case "filter":
      return set({ filter: c.value });
    case "volume":
      return set({ volume: c.value });
    case "eject":
      picks[idx(c.deck)] = null;
      snap.decks[idx(c.deck)] = emptyDeck();
      return;
    case "crossfader":
      snap.auto_mix = null;
      snap.crossfader = c.value;
      return;
    case "master":
      snap.master = c.value;
      return;
    case "sampler_volume":
      snap.sampler_volume = c.value;
      return;
    case "set_curve":
      snap.crossfader_curve = c.curve;
      return;
    case "auto_mix":
      snap.auto_mix = { to: c.to, beats: c.beats || settings.auto_mix_beats, armed: false, progress: 0 };
      snap.decks[idx(c.to)].playing = true;
      return;
    case "auto_mix_cancel":
      snap.auto_mix = null;
      return;
    case "sampler":
      padUntil[c.pad] = performance.now() + PAD_MS;
      return;
  }
}

function tick() {
  const now = performance.now();
  for (const d of snap.decks) {
    if (!d.track) continue;
    if (d.playing) {
      d.position_ms = Math.min(d.track.duration_ms, d.position_ms + TICK_MS * (d.current_bpm / d.track.bpm + d.bend * 0.04));
      if (d.loop_ms && d.position_ms >= d.loop_ms[1]) d.position_ms = d.loop_ms[0];
    }
    const level = d.playing ? (0.55 + 0.3 * Math.abs(Math.sin(now / 180))) * d.volume : 0;
    d.peak = [level, level * 0.95];
  }
  const a = snap.auto_mix;
  if (a && !flag?.startsWith("mixing")) {
    const beatMs = snap.decks[idx(a.to)].track?.beat_ms ?? 500;
    a.progress = Math.min(1, a.progress + TICK_MS / (a.beats * beatMs));
    snap.crossfader = a.to === "b" ? a.progress : 1 - a.progress;
    if (a.progress >= 1) {
      snap.decks[1 - idx(a.to)].playing = false;
      snap.auto_mix = null;
    }
  }
  if (autoPad >= 0 && now > nextAutoPad) {
    padUntil[autoPad] = now + PAD_MS * 1.5;
    nextAutoPad = now + AUTO_PAD_EVERY_MS;
  }
  if (scratching && snap.decks[1].track && !snap.decks[1].playing) snap.decks[1].position_ms = 31_000 + Math.sin(now / 120) * 70;
  snap.pads_active = padUntil.map((t) => t > now);
  const m = Math.max(...snap.decks.map((d) => d.peak[0])) * snap.master;
  snap.master_peak = [m, m];
  snap.memory_used_mb = Math.round(snap.decks.reduce((s, d) => s + (d.track?.bytes ?? 0), 0) / 1048576);
  snap = { ...snap, decks: [{ ...snap.decks[0] }, { ...snap.decks[1] }] };
}

export const djMock = {
  settings: () => ({ dj: settings }),
  setSettings: (s: { dj?: DjSettings }) => {
    if (s.dj) settings = { ...settings, ...s.dj, homifito: { ...settings.homifito, ...s.dj.homifito } };
    snap.crossfader_curve = settings.crossfader_curve;
    return { dj: settings };
  },
  /** The 20 Hz `dj` event while on. */
  start: (emit: (event: string, payload: unknown) => void) => {
    send = emit;
    setInterval(() => {
      if (!on) return;
      if (!url.has("djstill")) tick();
      emit("dj", state());
    }, TICK_MS);
  },
  handlers: {
    dj_enable: async ({ on: want }: { on: boolean }) => {
      await new Promise((r) => setTimeout(r, 300));
      on = want;
      if (!on) {
        picks[0] = picks[1] = null;
        snap.decks = [emptyDeck(), emptyDeck()];
      }
      return state();
    },
    dj_state: () => state(),
    dj_load: async ({ deck, pick }: { deck: DeckId; pick: DjPick }) => {
      const i = idx(deck);
      picks[i] = { ...pick, deck };
      snap.decks[i] = { ...emptyDeck(), loading: true };
      send("dj-load", { type: "loading", deck });
      setTimeout(() => {
        const seconds = Math.round((pick.duration_ms ?? 214_000) / 1000);
        const track = info(pick.title, 96 + ((pick.title.length * 7) % 40), seconds);
        snap.decks[i] = loaded(track);
        send("dj-load", { type: "loaded", deck, track });
      }, 900);
      return null;
    },
    dj_control: ({ control: c }: { control: DjControl }) => (control(c), null),
    dj_overview: ({ deck }: { deck: DeckId }) => overview(idx(deck)),
    dj_keymap: () => keymap(),
    dj_setups: () => [...setups].sort((a, b) => a.name.localeCompare(b.name)).map(summary),
    dj_setup_save: ({ name }: { name: string }) => {
      const made = makeSetup(name.trim() || "My setup", settings);
      setups.push(made);
      setupsChanged();
      return summary(made);
    },
    dj_setup_overwrite: ({ id }: { id: string }) => {
      const found = setupById(id);
      Object.assign(found, { dj: structuredClone(settings), updated_ms: Date.now() });
      setupsChanged();
      return summary(found);
    },
    dj_setup_rename: ({ id, name }: { id: string; name: string }) => {
      const found = setupById(id);
      if (name.trim()) found.name = name.trim();
      setupsChanged();
      return summary(found);
    },
    dj_setup_apply: ({ id }: { id: string }): DjSetupApplied => {
      const found = setupById(id);
      const restart_needed = on && JSON.stringify(settings.layout.pads) !== JSON.stringify(found.dj.layout.pads);
      settings = structuredClone(found.dj);
      return { settings, restart_needed };
    },
    dj_setup_delete: ({ id }: { id: string }) => {
      const had = setups.some((s) => s.id === id);
      setups = setups.filter((s) => s.id !== id);
      setupsChanged();
      return had;
    },
    dj_setup_export: ({ path }: { id: string; path: string }) => (path.endsWith(".homefidj") ? path : `${path}.homefidj`),
    dj_setup_import: () => {
      const made = makeSetup("From my tablet", { ...DEFAULT_DJ_SETTINGS, layout: { sections: sections(["deck_a", "mixer", "deck_b", "crate", "sampler"]), pads: [...DEFAULT_DJ_SETTINGS.layout.pads] } });
      setups.push(made);
      setupsChanged();
      return summary(made);
    },
    dj_cached_bpm: ({ paths }: { paths: string[] }) => paths.map((p, i) => (i % 3 === 2 ? null : 90 + ((p.length * 13) % 50))),
  },
};
