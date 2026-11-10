// Development-only Exercise backend (mock.ts spreads `handlers`): the shapes of src-tauri/src/exercise.rs
// and homefi_core::exercise, a plan of 30 minutes with eight songs, and a simulated ride that advances,
// so every screen can be seen without the app. Flags (all open the Exercise screen):
//   ?exercise                 the setup            ?exercise=building   Build stays at "Checking the tempo, 7 of 12"
//   ?exercise=preview         a built plan         ?exercise=notice     the health notice opens on Build
//   ?exerror                  Build fails          ?exgenres            3 genres + a sentence, style Easy
//   ?ride                     a live ride (seated climb, time moves; ?exspeed=10 runs faster)
//   ?ride=next|count|sprint|rest|paused|done   frozen scenes (next part in 10 s, the 3-2-1, GO!, Rest, Paused, finished)
//   ?exunseen  the health notice not seen yet     ?exdarkoff  ride screen follows the theme
//   ?exrides  five saved rides in "My rides" (opens that tab; ?exrides=empty opens it with none)
//   ?expulse=bar|flash  the beat pulse style      ?aikey  AI on (otherwise the setup shows "No AI")
import type {
  BeatRatio,
  CueKind,
  ExerciseSettings,
  ExerciseState,
  ExerciseTurn,
  PlannedSong,
  PresetView,
  ProfilePoint,
  RideBuilt,
  RidePlan,
  RideProgress,
  RideRequest,
  RideTick,
  RideView,
  SavedRide,
  Segment,
  SegmentKind,
} from "./exercise-api";
import { DEFAULT_EXERCISE_SETTINGS } from "./exercise-api";

const url = new URLSearchParams(location.search);
const flag = url.get("exercise");
const rideFlag = url.get("ride");
const active = url.has("exercise") || url.has("ride") || url.has("exrides");
if (active && !location.hash) location.hash = "#exercise";

const TICK_MS = 100;
const MS_PER_S = 1000;
const NOTICE_S = 10;
const COUNTDOWN_S = 3;
const CUE_HORIZON_MS = 15_000;
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

let settings: ExerciseSettings = {
  ...DEFAULT_EXERCISE_SETTINGS,
  health_notice_seen: !(url.has("exunseen") || flag === "notice"),
  ride_always_dark: !url.has("exdarkoff"),
  pulse_style: url.get("expulse") === "bar" ? "bar" : url.get("expulse") === "flash" ? "flash" : "ring",
  tv_audio: "phone",
  course_look: url.get("exlook") === "alpine" ? "alpine" : url.get("exlook") === "stage" ? "stage" : "ribbon",
  course_summits: url.has("exsummits"),
};

// ------------------------------------------------------------------ the plan

const cover = (hue: number) =>
  `data:image/svg+xml;utf8,${encodeURIComponent(`<svg xmlns='http://www.w3.org/2000/svg' width='120' height='120'><defs><linearGradient id='g' x1='0' y1='0' x2='1' y2='1'><stop offset='0' stop-color='hsl(${hue} 80% 55%)'/><stop offset='1' stop-color='hsl(${(hue + 70) % 360} 65% 20%)'/></linearGradient></defs><rect width='120' height='120' fill='url(#g)'/><circle cx='60' cy='60' r='34' fill='none' stroke='white' stroke-opacity='.35' stroke-width='7'/></svg>`)}`;

const seg = (kind: SegmentKind, seconds: number, min: number, max: number, effort: number, position: Segment["position"] = { kind: "seated" }, interval: Segment["interval"] = null): Segment => ({
  kind,
  seconds,
  target: { kind: "cadence", min_rpm: min, max_rpm: max },
  effort,
  position,
  interval,
  note: "",
});

const SEGMENTS: Segment[] = [
  seg("warm_up", 300, 60, 70, 3),
  seg("seated_climb", 300, 65, 75, 6),
  seg("jumps", 180, 80, 88, 6, { kind: "alternating", beats: 8 }),
  seg("recovery", 120, 65, 75, 2),
  seg("standing_climb", 300, 60, 66, 8, { kind: "standing" }),
  seg("recovery", 90, 70, 80, 2),
  seg("sprint", 360, 100, 110, 8, { kind: "seated" }, { work_s: 20, rest_s: 70, rounds: 4, rest_cadence: { min_rpm: 80, max_rpm: 90 } }),
  seg("cool_down", 150, 60, 70, 2),
];

/** homefi_core::exercise::profile::grade, for the mock's drawing. */
const GRADE: Record<SegmentKind, (effort: number) => number> = {
  warm_up: (e) => 0.15 * Math.max(0, (e - 3) / 7),
  seated_flat: (e) => 0.3 * Math.max(0, (e - 3) / 7),
  seated_climb: (e) => Math.max(0, (e - 3) / 7),
  standing_climb: (e) => 1.2 * Math.max(0, (e - 3) / 7),
  jumps: (e) => 0.6 * Math.max(0, (e - 3) / 7),
  sprint: (e) => 0.9 * Math.max(0, (e - 3) / 7),
  recovery: () => -0.5,
  cool_down: () => -0.6,
};
function profileOf(segments: Segment[]): ProfilePoint[] {
  const raw: [number, number][] = [[0, 0]];
  let t = 0;
  let e = 0;
  for (const s of segments) {
    t += s.seconds;
    e += GRADE[s.kind](s.effort) * s.seconds;
    raw.push([t, e]);
  }
  const lo = Math.min(...raw.map((r) => r[1]));
  const hi = Math.max(...raw.map((r) => r[1]));
  return raw.map(([t_s, v]) => ({ t_s, elevation: hi - lo > 1e-6 ? (v - lo) / (hi - lo) : 0.5 }));
}

const STROKES_PER_BEAT: Record<BeatRatio, number> = { whole: 1, half: 0.5, double: 2 };
const song = (index: number, title: string, artist: string, bpm: number | null, ratio: BeatRatio, hue: number, source: PlannedSong["source"] = "tag"): PlannedSong => {
  const s = SEGMENTS[index];
  const rpm = bpm === null ? 0 : bpm * STROKES_PER_BEAT[ratio];
  const range = s.target;
  const error = Math.max(0, range.min_rpm - rpm, rpm - range.max_rpm);
  return {
    segment_index: index,
    entry: { title, artist, album: "", tidal_id: null, isrc: null, local_path: null, duration_ms: s.seconds * MS_PER_S, explicit: false, cover_url: cover(hue), tag: "plain", note: "" },
    bpm,
    source: bpm === null ? null : source,
    fit: bpm === null ? null : { ratio, rpm, error_rpm: error },
    pulse: bpm === null ? "metronome" : "song_beat",
  };
};
const SONGS: PlannedSong[] = [
  song(0, "Heart of Glass", "Blondie", 136, "half", 210),
  song(1, "Smells Like Teen Spirit", "Nirvana", 142, "half", 20),
  song(2, "Blitzkrieg Bop", "Ramones", 177, "half", 350, "analysis"),
  song(3, "Nothing Else Matters", "Metallica", null, "whole", 280),
  song(4, "Back in Black", "AC/DC", 110, "half", 0, "tidal"),
  song(5, "Fade to Black", "Metallica", 144, "half", 160),
  song(6, "Painkiller", "Judas Priest", 207, "half", 330),
  song(7, "Wish You Were Here", "Pink Floyd", 61, "whole", 240),
];

function planFor(request: RideRequest): RidePlan {
  return {
    version: 1,
    workout: { version: 1, id: "ride-mock", name: `Easy ${request.minutes}`, sport: "indoor_cycling", segments: SEGMENTS, origin: { kind: "preset" } },
    songs: SONGS,
    notes: ["No song fit the Recovery part, so it uses the metronome.", "Beginner mode kept the standing climb at 60 RPM or more."],
    cues: { countdown_seconds: settings.countdown_seconds, notice_seconds: settings.segment_notice_seconds },
    wish: request.wish,
    profile: profileOf(SEGMENTS),
  };
}

// ------------------------------------------------------------------ the clock (homefi_core::exercise::RideClock, simplified)

const TOTAL_S = SEGMENTS.reduce((s, x) => s + x.seconds, 0);

function cuesOf(plan: RidePlan, index: number, posMs: number): RideView["cues"] {
  const s = plan.workout.segments[index];
  const endIn = s.seconds * MS_PER_S - posMs;
  const out: RideView["cues"] = [];
  const kinds: [number, CueKind][] = [[3, "countdown3"], [2, "countdown2"], [1, "countdown1"]];
  for (const [k, kind] of kinds) {
    const inMs = endIn - k * MS_PER_S;
    if (inMs > -500 && inMs < CUE_HORIZON_MS) out.push({ id: (index + 1) * 1000 + 100 + k, kind, in_ms: inMs });
  }
  if (endIn > -500 && endIn < CUE_HORIZON_MS) out.push({ id: (index + 1) * 1000, kind: plan.workout.segments[index + 1]?.position.kind === "standing" ? "standing" : "change", in_ms: endIn });
  return out;
}

function viewAt(plan: RidePlan, index: number, posMs: number, finished = false): RideView {
  const segments = plan.workout.segments;
  const s = segments[Math.min(index, segments.length - 1)];
  const i = Math.min(index, segments.length - 1);
  const posS = finished ? s.seconds : Math.floor(posMs / MS_PER_S);
  const before = segments.slice(0, i).reduce((a, x) => a + x.seconds, 0);
  const remaining = Math.max(0, s.seconds - posS);
  let range = { min_rpm: s.target.min_rpm, max_rpm: s.target.max_rpm };
  let interval: RideView["interval"] = null;
  if (s.interval) {
    const iv = s.interval;
    const cycle = iv.work_s + iv.rest_s;
    const round = Math.floor(posS / cycle);
    const offset = posS % cycle;
    const work = round < iv.rounds && offset < iv.work_s;
    interval = { phase: work ? "work" : "rest", round: Math.min(iv.rounds, round + 1), rounds: iv.rounds, phase_remaining_s: work ? iv.work_s - offset : round >= iv.rounds ? remaining : cycle - offset };
    if (!work) range = { min_rpm: iv.rest_cadence.min_rpm, max_rpm: iv.rest_cadence.max_rpm };
  }
  const next = segments[i + 1];
  const planned = plan.songs.find((x) => x.segment_index === i);
  const center = Math.round((range.min_rpm + range.max_rpm) / 2);
  const beatMs = planned?.bpm ? 60_000 / planned.bpm : 60_000 / center;
  const ratio = planned?.fit?.ratio ?? "whole";
  const t = before + posMs / MS_PER_S;
  const marker = elevationAt(plan.profile, finished ? TOTAL_S : t);
  const elapsed = Math.min(TOTAL_S, before + Math.min(posS, s.seconds));
  return {
    at_ms: Date.now(),
    finished,
    segment_index: i,
    segment_count: segments.length,
    kind: s.kind,
    position: s.position,
    effort: s.effort,
    target_rpm: center,
    range,
    segment_seconds: s.seconds,
    segment_start_s: before,
    remaining_s: remaining,
    segment_progress: s.seconds ? Math.min(1, posS / s.seconds) : 0,
    elapsed_s: elapsed,
    total_s: TOTAL_S,
    progress: elapsed / TOTAL_S,
    marker: { t_s: elapsed, elevation: marker },
    interval,
    notice: !finished && !!next && remaining <= plan.cues.notice_seconds,
    next: next ? { kind: next.kind, position: next.position, target_rpm: Math.round((next.target.min_rpm + next.target.max_rpm) / 2), seconds: next.seconds } : null,
    pulse: { source: planned?.pulse ?? "metronome", first_beat_ms: 180, beat_ms: beatMs, stroke_ms: beatMs / STROKES_PER_BEAT[ratio], ratio, song_position_ms: posMs },
    cues: finished ? [] : cuesOf(plan, i, posMs),
  };
}

function elevationAt(points: ProfilePoint[], t: number): number {
  for (let i = 1; i < points.length; i++) {
    if (t <= points[i].t_s) {
      const a = points[i - 1];
      const b = points[i];
      return a.elevation + ((b.elevation - a.elevation) * (t - a.t_s)) / Math.max(1, b.t_s - a.t_s);
    }
  }
  return points[points.length - 1].elevation;
}

// ------------------------------------------------------------------ the simulated session

const SCENES: Record<string, { index: number; pos: number; playing?: boolean; finished?: boolean }> = {
  next: { index: 1, pos: 290_000 },
  count: { index: 1, pos: 297_000 },
  sprint: { index: 6, pos: 5_000 },
  rest: { index: 6, pos: 40_000 },
  paused: { index: 1, pos: 120_000, playing: false },
  done: { index: 8, pos: 0, finished: true },
};
const scene = rideFlag ? SCENES[rideFlag] : undefined;
const frozen = !!scene || url.has("exstill");
const speed = Number(url.get("exspeed") ?? 1) || 1;

let stage: ExerciseState["stage"] = "idle";
let plan: RidePlan | null = null;
let ride = { index: 1, pos: 120_000, playing: true, finished: false };
let tvServer = false;
let tvWindow = false;
let abort: (() => void) | null = null;
let send: (event: string, payload: unknown) => void = () => {};
let request: RideRequest = {
  minutes: 30,
  style: url.has("exgenres") ? "easy" : "first_ride",
  wish: url.has("exgenres") ? { genres: ["Rock", "Metal", "Punk"], mix: true, sentence: "Classic rock for the climbs, something heavy for the sprints" } : { genres: [], mix: true, sentence: "" },
  workout: null,
};

function begin(from: { index: number; pos: number; playing?: boolean; finished?: boolean }) {
  plan = plan ?? planFor(request);
  ride = { index: from.index, pos: from.pos, playing: from.playing ?? true, finished: !!from.finished };
  stage = from.finished ? "finished" : "riding";
}
if (rideFlag !== null) begin(scene ?? { index: 1, pos: 120_000 });
else if (flag === "preview") {
  plan = planFor(request);
  stage = "ready";
}

const currentView = () => (plan && (stage === "riding" || stage === "finished") ? viewAt(plan, ride.index, ride.pos, ride.finished) : null);
// ------------------------------------------------------------------ saved rides (homefi_core::exercise::RideStore)

const DAY_MS = 86_400_000;
const NOW_MS = Date.now();
const STYLE_NAMES: Record<string, string> = { first_ride: "First ride", easy: "Easy", classic: "Classic", intervals: "Intervals" };
const autoName = (r: RideRequest) => `${r.minutes} min · ${STYLE_NAMES[r.style]} · ${r.wish.genres.length ? r.wish.genres.join(" + ") : "Any music"}`;
let rideSeq = 0;
function savedRide(r: RideRequest, patch: Partial<SavedRide> = {}): SavedRide {
  const segs = SEGMENTS;
  return { id: `ride-${++rideSeq}`, name: autoName(r), minutes: r.minutes, style: r.style, genres: r.wish.genres, songs: SONGS.length, created_ms: Date.now(), last_ridden_ms: null, times_ridden: 0, favorite: false, auto: true, profile: profileOf(segs), ...patch };
}
const req = (minutes: number, style: RideRequest["style"], genres: string[]): RideRequest => ({ minutes, style, wish: { genres, mix: true, sentence: "" }, workout: null });
let saved: SavedRide[] = [];
const seeded = url.has("exrides") && url.get("exrides") !== "empty";
if (seeded) {
  saved = [
    savedRide(req(45, "classic", ["Rock", "Metal"]), { name: "Friday climb", auto: false, favorite: true, times_ridden: 6, last_ridden_ms: NOW_MS - 2 * DAY_MS, created_ms: NOW_MS - 40 * DAY_MS }),
    savedRide(req(30, "intervals", ["Punk", "Pop"]), { auto: false, times_ridden: 2, last_ridden_ms: NOW_MS - 9 * DAY_MS, created_ms: NOW_MS - 20 * DAY_MS, name: "Sprint day with Ana" }),
    savedRide(req(30, "first_ride", []), { times_ridden: 1, last_ridden_ms: NOW_MS - 1 * DAY_MS, created_ms: NOW_MS - 3 * DAY_MS }),
    savedRide(req(20, "easy", ["Latin", "Dance"]), { created_ms: NOW_MS - 2 * 3600_000 }),
    savedRide(req(60, "classic", ["80s"]), { times_ridden: 3, last_ridden_ms: NOW_MS - 21 * DAY_MS, created_ms: NOW_MS - 50 * DAY_MS }),
  ];
}
const sortedRides = () => [...saved].sort((a, b) => Number(b.favorite) - Number(a.favorite) || (b.last_ridden_ms ?? b.created_ms) - (a.last_ridden_ms ?? a.created_ms));
const ridesChanged = () => send("exercise-rides", null);
const rideById = (id: string) => {
  const found = saved.find((r) => r.id === id);
  if (!found) throw "That saved ride was not found.";
  return found;
};
const patchRide = (id: string, patch: Partial<SavedRide>) => {
  const ride = rideById(id);
  Object.assign(ride, patch);
  ridesChanged();
  return ride;
};
let savedId: string | null = null;
if (flag === "preview") {
  const ride = savedRide(request);
  saved.push(ride);
  savedId = ride.id;
}

function state(): ExerciseState {
  return {
    stage,
    plan: stage === "idle" || stage === "building" ? null : plan,
    view: currentView(),
    playing: stage === "riding" && ride.playing,
    tv: { server: tvServer, url: tvServer ? "http://192.168.1.24:47801/" : null, window: tvWindow, hub: false, audio: settings.tv_audio },
    notice: null,
    saved_id: stage === "idle" || stage === "building" ? null : savedId,
  };
}
const publish = () => send("exercise-state", state());

function tick() {
  if (stage !== "riding" || !plan) return;
  if (ride.playing && !frozen) {
    ride.pos += TICK_MS * speed;
    const s = plan.workout.segments[ride.index];
    if (ride.pos >= s.seconds * MS_PER_S) {
      ride.index++;
      ride.pos = 0;
      if (ride.index >= plan.workout.segments.length) {
        ride.finished = true;
        stage = "finished";
        publish();
      }
    }
  }
  const tickEvent: RideTick = { stage, playing: ride.playing, view: viewAt(plan, ride.index, ride.pos, ride.finished) };
  send("ride", tickEvent);
}

// ------------------------------------------------------------------ build and chat

async function build(r: RideRequest, channel: { id: number }): Promise<RideBuilt> {
  request = r;
  stage = "building";
  publish();
  const cancelled = new Promise<never>((_, reject) => (abort = () => reject("cancelled")));
  const post = (() => {
    let n = 0;
    return (p: RideProgress) => (window as any)[`_${channel.id}`]?.({ index: n++, message: p });
  })();
  const run = async (): Promise<RideBuilt> => {
    post({ kind: "searching", with_ai: true });
    await wait(700);
    for (let done = 1; done <= 12; done++) {
      post({ kind: "resolving", done: done * 3, total: 36 });
      await wait(60);
    }
    for (let done = 0; done < 12; done++) {
      post({ kind: "analyzing", done, total: 12 });
      if ((flag === "building" || url.has("exrebuild")) && done === 6) await new Promise(() => {});
      await wait(120);
    }
    post({ kind: "matching" });
    await wait(500);
    if (url.has("exerror")) throw "TIDAL is not answering. Check your connection and try again.";
    plan = planFor(r);
    const ride = savedRide(r);
    saved.push(ride);
    savedId = ride.id;
    ridesChanged();
    return { plan, playlist: { id: "ride-mock", name: `Easy ${r.minutes} · ${r.wish.genres.join(" + ") || "Any music"}` }, saved: ride };
  };
  try {
    const built = await Promise.race([run(), cancelled]);
    stage = "ready";
    return built;
  } catch (e) {
    stage = plan ? "ready" : "idle";
    throw e;
  } finally {
    abort = null;
    publish();
  }
}

const turns: ExerciseTurn[] = [
  { text: "What kind of ride do you feel like today, and what do you want to hear?", quick_replies: ["Easy, with rock", "Something heavy", "Surprise me"], request, title: "" },
  { text: "Got it: an easy ride with rock and metal. I'll keep the sprints for the heaviest songs. Anything to avoid?", quick_replies: ["Nothing", "No slow songs"], request: { ...request, style: "easy", wish: { genres: ["Rock", "Metal"], mix: true, sentence: "" } }, title: "Easy rock and metal" },
];
let turnAt = 0;

// ------------------------------------------------------------------ cues

/** A short sine as a mono 16-bit WAV, like exercise_cue_wav. */
function wav(rate: number, hz: number): ArrayBuffer {
  const n = Math.floor(rate * 0.12);
  const buf = new ArrayBuffer(44 + n * 2);
  const v = new DataView(buf);
  const str = (o: number, t: string) => [...t].forEach((c, i) => v.setUint8(o + i, c.charCodeAt(0)));
  str(0, "RIFF");
  v.setUint32(4, 36 + n * 2, true);
  str(8, "WAVEfmt ");
  v.setUint32(16, 16, true);
  v.setUint16(20, 1, true);
  v.setUint16(22, 1, true);
  v.setUint32(24, rate, true);
  v.setUint32(28, rate * 2, true);
  v.setUint16(32, 2, true);
  v.setUint16(34, 16, true);
  str(36, "data");
  v.setUint32(40, n * 2, true);
  for (let i = 0; i < n; i++) v.setInt16(44 + i * 2, Math.sin((2 * Math.PI * hz * i) / rate) * 9000 * Math.sin((Math.PI * i) / n), true);
  return buf;
}

const PRESETS: PresetView[] = [
  { id: "first_ride", name: "First ride" },
  { id: "easy", name: "Easy" },
  { id: "classic", name: "Classic" },
  { id: "intervals", name: "Intervals" },
];

export const exerciseMock = {
  settings: () => ({ exercise: settings }),
  setSettings: (s: { exercise?: ExerciseSettings }) => {
    if (s.exercise) settings = { ...settings, ...s.exercise };
    return { exercise: settings };
  },
  /** The player's pause and next while a ride runs; true when the ride took it. */
  control: (what: "toggle" | "next"): boolean => {
    if (stage !== "riding" || !plan) return false;
    if (what === "toggle") ride.playing = !ride.playing;
    else if (ride.index < plan.workout.segments.length - 1) {
      ride.index++;
      ride.pos = 0;
    }
    publish();
    return true;
  },
  start: (emit: (event: string, payload: unknown) => void) => {
    send = emit;
    setInterval(tick, TICK_MS);
  },
  handlers: {
    exercise_state: () => state(),
    exercise_presets: () => PRESETS,
    exercise_ride_minutes: () => Array.from({ length: 17 }, (_, i) => 10 + i * 5),
    exercise_default_request: () => ({ ...request, minutes: settings.ride_minutes }),
    exercise_artist_suggestions: ({ genres }: { genres: string[] }) =>
      genres.some((g) => g.toLowerCase() === "metal") ? ["Metallica", "Iron Maiden", "Megadeth"] : ["Foo Fighters", "Queen"],
    exercise_health_notice: () =>
      "HomeFi suggests a pace and an effort, it does not measure your body or give medical advice. Listen to yourself, and stop if you feel pain, dizziness or shortness of breath. If in doubt, check with a doctor.",
    exercise_open: async () => {
      if (!url.has("aikey")) throw "Needs your AI key. Genres still work.";
      turnAt = 0;
      return turns[0];
    },
    exercise_reply: async () => {
      await wait(700);
      turnAt = Math.min(turns.length - 1, turnAt + 1);
      return turns[turnAt];
    },
    exercise_close: () => null,
    exercise_build: ({ request: r, onProgress }: { request: RideRequest; onProgress: { id: number } }) => build(r, onProgress),
    exercise_cancel: () => (abort?.(), null),
    exercise_rides: () => sortedRides(),
    exercise_ride_load: ({ id }: { id: string }) => {
      const ride = rideById(id);
      if (stage === "riding" || stage === "building") throw "Finish the ride in progress before loading another.";
      request = req(ride.minutes, ride.style, ride.genres);
      plan = planFor(request);
      stage = "ready";
      savedId = id;
      publish();
      return { request, plan, playlist: { id: "ride-mock", name: ride.name }, saved: ride };
    },
    exercise_ride_keep: ({ id, name }: { id: string; name: string | null }) => patchRide(id, { auto: false, ...(name?.trim() ? { name: name.trim() } : {}) }),
    exercise_ride_rename: ({ id, name }: { id: string; name: string }) => patchRide(id, { auto: false, ...(name.trim() ? { name: name.trim() } : {}) }),
    exercise_ride_favorite: ({ id, favorite }: { id: string; favorite: boolean }) => patchRide(id, { favorite }),
    exercise_ride_delete: ({ id }: { id: string }) => {
      const had = saved.some((r) => r.id === id);
      saved = saved.filter((r) => r.id !== id);
      if (savedId === id) savedId = null;
      ridesChanged();
      publish();
      return had;
    },
    exercise_ride_export: ({ path }: { id: string; path: string }) => (path.endsWith(".homefiride") ? path : `${path}.homefiride`),
    exercise_ride_import: () => {
      const ride = savedRide(req(40, "classic", ["Indie"]), { name: "Shared by Marco", auto: false, created_ms: Date.now() });
      saved.push(ride);
      ridesChanged();
      return ride;
    },
    exercise_start: ({ plan: p }: { plan: RidePlan }) => {
      plan = p;
      if (savedId) {
        const ride = saved.find((r) => r.id === savedId);
        if (ride) Object.assign(ride, { times_ridden: ride.times_ridden + 1, last_ridden_ms: Date.now() });
        ridesChanged();
      }
      begin({ index: 0, pos: 0 });
      if (settings.tv_enabled) tvServer = true;
      publish();
      return state();
    },
    exercise_stop: () => {
      stage = "idle";
      tvWindow = false;
      publish();
      return state();
    },
    exercise_tv: ({ on }: { on: boolean }) => {
      tvServer = on;
      publish();
      return on ? "http://192.168.1.24:47801/" : null;
    },
    exercise_monitors: () => [
      { name: "Built-in Retina Display", width: 3024, height: 1964, x: 0, y: 0, primary: true },
      { name: "LG TV", width: 3840, height: 2160, x: 3024, y: 0, primary: false },
    ],
    exercise_tv_window: ({ on }: { on: boolean }) => {
      tvWindow = on;
      if (on) tvServer = true;
      publish();
      return null;
    },
    exercise_cue_wav: ({ kind, sampleRate }: { kind: CueKind; sampleRate: number }) => wav(sampleRate, kind === "sprint_go" ? 880 : kind.startsWith("countdown") ? 660 : 523),
  },
};
