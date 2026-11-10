// Exercise mode (indoor cycling): the types and calls of src-tauri/src/exercise.rs, with the
// shapes of homefi_core::exercise (snake_case JSON, the same on Android). The state lives in
// lib/exercise.svelte.ts; the words and numbers every screen shares are here, once.
import { Channel, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { PlaylistEntry } from "./api";

// ------------------------------------------------------------------ the course

export type PresetId = "first_ride" | "easy" | "classic" | "intervals";
export const PRESET_IDS: readonly PresetId[] = ["first_ride", "easy", "classic", "intervals"];

export type SegmentKind = "warm_up" | "seated_flat" | "seated_climb" | "standing_climb" | "jumps" | "sprint" | "recovery" | "cool_down";

export type Position = { kind: "seated" } | { kind: "standing" } | { kind: "alternating"; beats: number };

export interface RpmRange {
  min_rpm: number;
  max_rpm: number;
}

export interface Interval {
  work_s: number;
  rest_s: number;
  rounds: number;
  rest_cadence: RpmRange;
}

export interface Segment {
  kind: SegmentKind;
  seconds: number;
  target: { kind: "cadence" } & RpmRange;
  effort: number;
  position: Position;
  interval: Interval | null;
  note: string;
}

export interface Workout {
  version: number;
  id: string;
  name: string;
  sport: string;
  segments: Segment[];
  origin: { kind: string };
}

/** How famous the songs are: the genre's big hits (the default), a mix, or music to discover. */
export type Popularity = "hits" | "mix" | "discover";
export const POPULARITIES: readonly { id: Popularity; label: string; hint: string }[] = [
  { id: "hits", label: "Big hits", hint: "Famous songs by the genre's best-known acts" },
  { id: "mix", label: "Mix", hint: "Mostly hits, some lesser-known songs" },
  { id: "discover", label: "Discover", hint: "Songs and artists you may not know yet" },
];
/** The decades the setup offers (homefi_core::exercise::DECADES); none picked is any era. */
export const EX_DECADES: readonly number[] = [1960, 1970, 1980, 1990, 2000, 2010, 2020];
/** Liked or avoided artists kept, and the longest name (the core's limits). */
export const EX_MAX_TASTE_ARTISTS = 10;
export const EX_MAX_ARTIST_CHARS = 60;

/** The rider's taste for a ride; a setting (the setup saves every answer). */
export interface RideTaste {
  eras: number[];
  popularity: Popularity;
  artists: string[];
  avoid: string[];
}
export const DEFAULT_TASTE: RideTaste = { eras: [], popularity: "hits", artists: [], avoid: [] };

export interface MusicWish {
  genres: string[];
  mix: boolean;
  sentence: string;
  /** The backend fills it from the settings when building. */
  taste?: RideTaste;
}

export interface RideRequest {
  /** 10 to 90, steps of 5. */
  minutes: number;
  style: PresetId;
  wish: MusicWish;
  workout: Workout | null;
}

export interface ProfilePoint {
  t_s: number;
  /** 0 to 1. */
  elevation: number;
}

export type BeatRatio = "whole" | "half" | "double";
export type BpmSource = "tag" | "tidal" | "analysis";

export interface Fit {
  ratio: BeatRatio;
  /** The cadence the song gives at that ratio. */
  rpm: number;
  /** How far outside the target range (0 inside). */
  error_rpm: number;
}

export interface PlannedSong {
  segment_index: number;
  entry: PlaylistEntry;
  bpm: number | null;
  source: BpmSource | null;
  fit: Fit | null;
  pulse: "song_beat" | "metronome";
}

export interface RidePlan {
  version: number;
  workout: Workout;
  songs: PlannedSong[];
  notes: string[];
  /** The AI was asked and failed (why): shown loudly. Absent when it answered or was not asked. */
  ai_problem?: string | null;
  cues: { countdown_seconds: number; notice_seconds: number };
  wish: MusicWish;
  profile: ProfilePoint[];
}

// ------------------------------------------------------------------ the ride, live

export interface IntervalView {
  phase: "work" | "rest";
  round: number;
  rounds: number;
  phase_remaining_s: number;
}

export interface NextView {
  kind: SegmentKind;
  position: Position;
  target_rpm: number;
  seconds: number;
}

export interface PulseView {
  source: "song_beat" | "metronome";
  first_beat_ms: number;
  beat_ms: number;
  /** Time between pedal strokes. */
  stroke_ms: number;
  ratio: BeatRatio;
  /** The song's position this view was made at. */
  song_position_ms: number;
}

export type CueKind = "countdown3" | "countdown2" | "countdown1" | "change" | "standing" | "seated" | "sprint_go" | "recover" | "finish" | "click";

export interface Cue {
  /** Stable across views: a cue seen twice plays once. */
  id: number;
  kind: CueKind;
  /** From the view's moment (negative: just passed). */
  in_ms: number;
}

export interface RideView {
  at_ms: number;
  finished: boolean;
  segment_index: number;
  segment_count: number;
  kind: SegmentKind;
  position: Position;
  effort: number;
  target_rpm: number;
  range: RpmRange;
  segment_seconds: number;
  segment_start_s: number;
  remaining_s: number;
  segment_progress: number;
  elapsed_s: number;
  total_s: number;
  progress: number;
  marker: ProfilePoint;
  interval: IntervalView | null;
  notice: boolean;
  next: NextView | null;
  pulse: PulseView;
  cues: Cue[];
}

// ------------------------------------------------------------------ the backend's state

export type Stage = "idle" | "building" | "ready" | "riding" | "finished";
export type TvAudio = "tv" | "phone";

export interface TvState {
  server: boolean;
  url: string | null;
  window: boolean;
  hub: boolean;
  audio: TvAudio;
}

export interface ExerciseState {
  stage: Stage;
  plan: RidePlan | null;
  view: RideView | null;
  playing: boolean;
  tv: TvState;
  notice: string | null;
  /** The saved ride the plan belongs to (every build is saved by itself). */
  saved_id: string | null;
}

/** The `ride` event. */
export interface RideTick {
  stage: Stage;
  playing: boolean;
  view: RideView;
}

export type RideProgress =
  | { kind: "searching"; with_ai: boolean }
  | { kind: "resolving"; done: number; total: number }
  | { kind: "analyzing"; done: number; total: number }
  | { kind: "matching" };

/** The build's steps as the screens name them, and where each sits on an overall bar (start %, span %). */
export const BUILD_STEPS: readonly { id: RideProgress["kind"]; label: string; span: [number, number] }[] = [
  { id: "searching", label: "Searching songs", span: [4, 14] },
  { id: "resolving", label: "Finding them on TIDAL and in your library", span: [18, 40] },
  { id: "analyzing", label: "Checking the tempo", span: [58, 30] },
  { id: "matching", label: "Matching songs to each part", span: [90, 8] },
];
/** The step a progress report is at (0 before the first) and the overall percent (never 100 before the end). */
export function buildStepOf(p: RideProgress | null): { at: number; percent: number } {
  const at = p ? Math.max(0, BUILD_STEPS.findIndex((s) => s.id === p.kind)) : 0;
  const fraction = p && "done" in p && p.total > 0 ? p.done / p.total : 0;
  const [start, span] = BUILD_STEPS[at].span;
  return { at, percent: Math.min(99, Math.round(start + span * fraction)) };
}

export interface RideBuilt {
  plan: RidePlan;
  playlist: { id: string; name: string };
  /** The automatic save of this build (null when it could not be written). */
  saved: SavedRide | null;
}

/** homefi_core::exercise::RideSummary: a saved ride as the list shows it. */
export interface SavedRide {
  id: string;
  name: string;
  minutes: number;
  style: PresetId;
  genres: string[];
  songs: number;
  created_ms: number;
  last_ridden_ms: number | null;
  times_ridden: number;
  favorite: boolean;
  /** Saved by itself (it may be pruned); false once the rider keeps it or imported it. */
  auto: boolean;
  profile: ProfilePoint[];
}

/** What `exercise_ride_load` answers: the request and plan, ready to start (no build, no AI). */
export interface RideLoaded {
  request: RideRequest;
  plan: RidePlan;
  playlist: { id: string; name: string };
  saved: SavedRide;
}

/** The file kind the ride export dialogs use. */
export const RIDE_FILE = { extension: "homefiride", label: "HomeFi ride" } as const;

export interface ExerciseTurn {
  text: string;
  quick_replies: string[];
  request: RideRequest;
  title: string;
}

export interface PresetView {
  id: PresetId;
  name: string;
}

export interface MonitorView {
  name: string | null;
  width: number;
  height: number;
  x: number;
  y: number;
  primary: boolean;
}

// ------------------------------------------------------------------ settings (homefi_core::exercise::ExerciseSettings)

export type PulseStyle = "ring" | "bar" | "flash";
/** How the course mountain is drawn (docs/MOUNTAIN.md). */
export type CourseLook = "ribbon" | "alpine" | "stage";
export type SourcesMode = "library_first" | "tidal_first" | "library_only";

export interface ExerciseSettings {
  show_entry: boolean;
  beginner_mode: boolean;
  default_preset: PresetId;
  ride_minutes: number;
  cadence_tolerance_rpm: number;
  allow_half_time: boolean;
  allow_double_time: boolean;
  sources: SourcesMode;
  analysis_budget: number;
  allow_explicit: boolean;
  countdown_seconds: number;
  segment_notice_seconds: number;
  audible_cues: boolean;
  cue_volume: number;
  metronome_click: boolean;
  pulse_style: PulseStyle;
  pulse_offset_ms: number;
  keep_screen_on: boolean;
  tv_enabled: boolean;
  tv_audio: TvAudio;
  tv_port: number;
  tv_window_fullscreen: boolean;
  /** "auto" or a screen's name. */
  tv_window_display: string;
  /** The TV shows the song's lyrics. */
  tv_lyrics: boolean;
  /** How the course mountain looks (setup, preview, ride, summary, the TV). */
  course_look: CourseLook;
  /** Label the tallest summits with their time. */
  course_summits: boolean;
  ride_always_dark: boolean;
  /** Android only; kept so the JSON round-trips. */
  haptic_cues: boolean;
  health_notice_seen: boolean;
  tempo_nudge_pct: number;
  /** How many automatic saved rides are kept (kept ones and favorites never count). */
  ride_history_max: number;
  /** The setup's taste answers, remembered for the next ride. */
  taste: RideTaste;
}

// Ranges and defaults: a copy of homefi_core::exercise (the core clamps whatever is sent).
export const EX_MINUTES = { min: 10, max: 90, step: 5, page: 15, chips: [20, 30, 45, 60] } as const;
export const EX_TOLERANCE_RPM = { min: 2, max: 12 } as const;
export const EX_ANALYSIS_BUDGET = { min: 0, max: 50 } as const;
export const EX_COUNTDOWN_S = { min: 0, max: 10 } as const;
export const EX_NOTICE_S = { min: 5, max: 30 } as const;
export const EX_PULSE_OFFSET_MS = { min: -500, max: 500, step: 10 } as const;
export const EX_HISTORY = { min: 5, max: 200, step: 5 } as const;
export const EX_TV_PORT = { min: 1024, max: 65535 } as const;
export const EX_TV_DISPLAY_AUTO = "auto";
export const EX_MAX_GENRES = 6;
export const EX_MAX_GENRE_CHARS = 40;
export const EX_MAX_SENTENCE_CHARS = 300;

export const DEFAULT_EXERCISE_SETTINGS: ExerciseSettings = {
  show_entry: true,
  beginner_mode: true,
  default_preset: "first_ride",
  ride_minutes: 30,
  cadence_tolerance_rpm: 4,
  allow_half_time: true,
  allow_double_time: false,
  sources: "library_first",
  analysis_budget: 12,
  allow_explicit: true,
  countdown_seconds: 3,
  segment_notice_seconds: 10,
  audible_cues: true,
  cue_volume: 0.6,
  metronome_click: false,
  pulse_style: "ring",
  pulse_offset_ms: 0,
  keep_screen_on: true,
  tv_enabled: false,
  tv_audio: "phone",
  tv_port: 47801,
  tv_window_fullscreen: true,
  tv_window_display: EX_TV_DISPLAY_AUTO,
  tv_lyrics: true,
  course_look: "ribbon",
  course_summits: false,
  ride_always_dark: true,
  haptic_cues: true,
  health_notice_seen: false,
  tempo_nudge_pct: 0,
  ride_history_max: 30,
  taste: DEFAULT_TASTE,
};
/** The setting with defaults for whatever an older settings file doesn't have. */
export const exerciseSettingsOf = (s: { exercise?: Partial<ExerciseSettings> } | null | undefined): ExerciseSettings => {
  const merged = { ...DEFAULT_EXERCISE_SETTINGS, ...s?.exercise };
  return { ...merged, taste: { ...DEFAULT_TASTE, ...merged.taste } };
};

// ------------------------------------------------------------------ words and numbers every screen shares

/** Language the backend builds its words in (the desktop UI is English). */
export const EX_LANGUAGE = "en";

export interface ZoneInfo {
  /** The CSS colour token (app.css). */
  token: string;
  label: string;
}
export const ZONES: Record<SegmentKind, ZoneInfo> = {
  warm_up: { token: "--z-warm", label: "Warm-up" },
  seated_flat: { token: "--z-flat", label: "Seated flat" },
  seated_climb: { token: "--z-climb", label: "Seated climb" },
  standing_climb: { token: "--z-stand", label: "Standing climb" },
  jumps: { token: "--z-jumps", label: "Jumps" },
  sprint: { token: "--z-sprint", label: "Sprints" },
  recovery: { token: "--z-recov", label: "Recovery" },
  cool_down: { token: "--z-cool", label: "Cool-down" },
};
export const zoneVar = (kind: SegmentKind) => `var(${ZONES[kind].token})`;
export const zoneName = (kind: SegmentKind) => ZONES[kind].label;
/** The legend order. */
export const ZONE_ORDER: readonly SegmentKind[] = ["warm_up", "seated_flat", "seated_climb", "standing_climb", "jumps", "sprint", "recovery", "cool_down"];

export const STYLE_INFO: Record<PresetId, { name: string; line: string }> = {
  first_ride: { name: "First ride", line: "Gentle, no sprints" },
  easy: { name: "Easy", line: "Climbs and jumps" },
  classic: { name: "Classic", line: "Two climbs, intervals" },
  intervals: { name: "Intervals", line: "Sprints and rests" },
};

export const GENRES = ["Rock", "Metal", "Pop", "Dance", "Hip-hop", "Latin", "Funk", "80s", "Indie", "Punk"] as const;

export const isStanding = (p: Position) => p.kind !== "seated";
export function positionLabel(p: Position): string {
  return p.kind === "seated" ? "Seated" : p.kind === "standing" ? "Standing" : "Seated, stand on the beat";
}
export const positionIcon = (p: Position) => (p.kind === "standing" ? "standing" : "seated");

const EFFORT_WORDS = ["Very easy", "Comfortable", "Steady", "Hard", "All out"] as const;
/** "Hard" for 7 and 8: five words for ten steps. */
export const effortWord = (effort: number) => EFFORT_WORDS[Math.min(EFFORT_WORDS.length - 1, Math.max(0, Math.ceil(effort / 2) - 1))];
export const effortHint = (effort: number) => `Turn the resistance until it feels ${effortWord(effort).toLowerCase()}.`;
export const EFFORT_STEPS = 10;

export function clock(seconds: number): string {
  const s = Math.max(0, Math.round(seconds));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}
/** "5 min", "2 min 30 s". */
export function spoken(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = Math.round(seconds % 60);
  return m && s ? `${m} min ${s} s` : m ? `${m} min` : `${s} s`;
}

export interface Part {
  kind: SegmentKind;
  start_s: number;
  seconds: number;
}
export function partsOf(w: Pick<Workout, "segments">): Part[] {
  let start_s = 0;
  return w.segments.map((s) => {
    const part = { kind: s.kind, start_s, seconds: s.seconds };
    start_s += s.seconds;
    return part;
  });
}

/** The elevation `t_s` seconds into the ride (homefi_core::exercise::profile::elevation_at). */
export function elevationAt(points: readonly ProfilePoint[], t_s: number): number {
  const first = points[0];
  const last = points[points.length - 1];
  if (!first || !last) return 0;
  if (t_s <= first.t_s) return first.elevation;
  if (t_s >= last.t_s) return last.elevation;
  for (let i = 1; i < points.length; i++) {
    const a = points[i - 1];
    const b = points[i];
    if (t_s <= b.t_s) {
      const span = b.t_s - a.t_s;
      return span <= 0 ? b.elevation : a.elevation + ((b.elevation - a.elevation) * (t_s - a.t_s)) / span;
    }
  }
  return last.elevation;
}

const RATIO_WORDS: Record<BeatRatio, string> = { whole: "1:1", half: "half-time", double: "double-time" };
export type FitTone = "ok" | "warn" | "metronome";
/** The tag of a song in the plan: "176 BPM · half-time -> 88 ok", "(off by 2)" when it is stretched, "Metronome" when no tempo fits. */
export function fitOf(song: PlannedSong, toleranceRpm: number): { tone: FitTone; text: string } {
  if (!song.fit || song.pulse === "metronome") return { tone: "metronome", text: "Metronome" };
  const bpm = song.bpm ? `${Math.round(song.bpm)} BPM · ` : "";
  const rpm = Math.round(song.fit.rpm);
  const off = Math.round(song.fit.error_rpm);
  const tone: FitTone = song.fit.error_rpm > toleranceRpm ? "warn" : "ok";
  const tail = off >= 1 ? ` (off by ${off})` : " ok";
  return { tone, text: `${bpm}${RATIO_WORDS[song.fit.ratio]} -> ${rpm}${tail}` };
}
export const FIT_TOOLTIP = "The song's beat, how it maps to your pedaling, and how close it is to the target.";

/** What a screen reader hears for the mountain. */
export function courseText(parts: readonly Part[], total_s: number, nowIndex: number | null, elapsed_s?: number): string {
  const list = parts.map((p) => `${zoneName(p.kind).toLowerCase()} ${spoken(p.seconds)}`).join(", ");
  const now = nowIndex !== null && parts[nowIndex] ? `, now in ${zoneName(parts[nowIndex].kind).toLowerCase()} at ${Math.floor((elapsed_s ?? 0) / 60)} minutes of ${Math.round(total_s / 60)}` : "";
  return `Course: ${list}${now}`;
}

/** Counts of what the ride holds, for "2 climbs, 6 jumps, 1 sprint block". */
export function describeWorkout(w: Pick<Workout, "segments">): string {
  const n = (f: (s: Segment) => boolean) => w.segments.filter(f).length;
  const climbs = n((s) => s.kind === "seated_climb" || s.kind === "standing_climb");
  const jumps = n((s) => s.kind === "jumps");
  const blocks = n((s) => !!s.interval);
  const plural = (c: number, one: string, many = `${one}s`) => `${c} ${c === 1 ? one : many}`;
  const parts = [plural(climbs, "climb"), jumps ? plural(jumps, "jump block") : "", blocks ? plural(blocks, "sprint block") : ""].filter(Boolean);
  const rpm = w.segments.flatMap((s) => [s.target.min_rpm, s.target.max_rpm]);
  const range = rpm.length ? `${Math.min(...rpm)} to ${Math.max(...rpm)} RPM` : "";
  return [...parts, range].filter(Boolean).join(" · ");
}

// ------------------------------------------------------------------ style sketches

/** One stretch of a style's drawing: a kind, its share of the ride and where the elevation ends. */
interface SketchPart {
  kind: SegmentKind;
  share: number;
  to: number;
}
/** A drawing of what each style looks like at any length. It only illustrates the picker; the plan the
 * core builds (its own profile) replaces it as soon as there is one. */
const SKETCHES: Record<PresetId, readonly SketchPart[]> = {
  first_ride: [
    { kind: "warm_up", share: 0.2, to: 0.08 },
    { kind: "seated_flat", share: 0.3, to: 0.2 },
    { kind: "seated_climb", share: 0.2, to: 0.55 },
    { kind: "recovery", share: 0.12, to: 0.3 },
    { kind: "cool_down", share: 0.18, to: 0 },
  ],
  easy: [
    { kind: "warm_up", share: 0.14, to: 0.1 },
    { kind: "seated_climb", share: 0.18, to: 0.5 },
    { kind: "jumps", share: 0.16, to: 0.7 },
    { kind: "recovery", share: 0.1, to: 0.4 },
    { kind: "seated_climb", share: 0.16, to: 0.8 },
    { kind: "sprint", share: 0.08, to: 1 },
    { kind: "cool_down", share: 0.18, to: 0 },
  ],
  classic: [
    { kind: "warm_up", share: 0.1, to: 0.08 },
    { kind: "seated_climb", share: 0.18, to: 0.55 },
    { kind: "recovery", share: 0.06, to: 0.35 },
    { kind: "standing_climb", share: 0.2, to: 0.95 },
    { kind: "recovery", share: 0.06, to: 0.5 },
    { kind: "sprint", share: 0.16, to: 0.9 },
    { kind: "cool_down", share: 0.24, to: 0 },
  ],
  intervals: [
    { kind: "warm_up", share: 0.12, to: 0.1 },
    { kind: "sprint", share: 0.1, to: 0.5 },
    { kind: "recovery", share: 0.08, to: 0.25 },
    { kind: "sprint", share: 0.1, to: 0.7 },
    { kind: "recovery", share: 0.08, to: 0.35 },
    { kind: "sprint", share: 0.12, to: 0.95 },
    { kind: "recovery", share: 0.08, to: 0.5 },
    { kind: "sprint", share: 0.1, to: 1 },
    { kind: "cool_down", share: 0.22, to: 0 },
  ],
};
export interface Sketch {
  parts: Part[];
  profile: ProfilePoint[];
}
export function sketchOf(style: PresetId, minutes: number): Sketch {
  const total = minutes * 60;
  const parts: Part[] = [];
  const profile: ProfilePoint[] = [{ t_s: 0, elevation: 0 }];
  // like the core's profile, the highest point is 1
  const top = Math.max(...SKETCHES[style].map((p) => p.to));
  let t = 0;
  for (const p of SKETCHES[style]) {
    const seconds = Math.round(total * p.share);
    parts.push({ kind: p.kind, start_s: t, seconds });
    t += seconds;
    profile.push({ t_s: t, elevation: p.to / top });
  }
  return { parts, profile };
}

// ------------------------------------------------------------------ the calls

export const exerciseApi = {
  state: () => invoke<ExerciseState>("exercise_state"),
  presets: (language: string) => invoke<PresetView[]>("exercise_presets", { language }),
  rideMinutes: () => invoke<number[]>("exercise_ride_minutes"),
  defaultRequest: () => invoke<RideRequest>("exercise_default_request"),
  healthNotice: (language: string) => invoke<string>("exercise_health_notice", { language }),
  /** Artists the library has the most songs of in these genres (for "artists you like"). */
  artistSuggestions: (genres: string[]) => invoke<string[]>("exercise_artist_suggestions", { genres }),
  /** The AI conversation: the error is "needs your AI key" when none is set up. */
  open: (request: RideRequest, language: string) => invoke<ExerciseTurn>("exercise_open", { request, language }),
  reply: (text: string) => invoke<ExerciseTurn>("exercise_reply", { text }),
  close: () => invoke<void>("exercise_close"),
  build: (request: RideRequest, language: string, onProgress: (p: RideProgress) => void) => {
    const channel = new Channel<RideProgress>();
    channel.onmessage = onProgress;
    return invoke<RideBuilt>("exercise_build", { request, language, onProgress: channel });
  },
  cancel: () => invoke<void>("exercise_cancel"),
  start: (plan: RidePlan) => invoke<ExerciseState>("exercise_start", { plan }),
  stop: () => invoke<ExerciseState>("exercise_stop"),
  /** Starts the TV's page (or stops it); answers the address a TV's browser opens. */
  tv: (on: boolean) => invoke<string | null>("exercise_tv", { on }),
  monitors: () => invoke<MonitorView[]>("exercise_monitors"),
  tvWindow: (on: boolean, monitor: string | null) => invoke<void>("exercise_tv_window", { on, monitor }),
  rides: () => invoke<SavedRide[]>("exercise_rides"),
  /** Puts a saved ride on the screen as ready: no AI, no build. */
  rideLoad: (id: string) => invoke<RideLoaded>("exercise_ride_load", { id }),
  rideKeep: (id: string, name?: string) => invoke<SavedRide>("exercise_ride_keep", { id, name: name ?? null }),
  rideRename: (id: string, name: string) => invoke<SavedRide>("exercise_ride_rename", { id, name }),
  rideFavorite: (id: string, favorite: boolean) => invoke<SavedRide>("exercise_ride_favorite", { id, favorite }),
  rideDelete: (id: string) => invoke<boolean>("exercise_ride_delete", { id }),
  /** Writes the `.homefiride` to a path the rider picked; answers the path written. */
  rideExport: (id: string, path: string) => invoke<string>("exercise_ride_export", { id, path }),
  rideImport: (path: string) => invoke<SavedRide>("exercise_ride_import", { path }),
  /** A cue as a mono 16-bit WAV. */
  cueWav: (kind: CueKind, sampleRate: number) => invoke<ArrayBuffer>("exercise_cue_wav", { kind, sampleRate }),
};

export const onExercise = {
  ride: (f: (t: RideTick) => void): Promise<UnlistenFn> => listen<RideTick>("ride", (e) => f(e.payload)),
  state: (f: (s: ExerciseState) => void): Promise<UnlistenFn> => listen<ExerciseState>("exercise-state", (e) => f(e.payload)),
  /** The list of saved rides changed (no payload). */
  rides: (f: () => void): Promise<UnlistenFn> => listen("exercise-rides", () => f()),
};

/** Where `position_ms` of the song falls inside a pedal stroke, 0 to 1 (0 = on the stroke): homefi_core's `PulseView::stroke_phase`. */
export function strokePhase(p: PulseView, position_ms: number): number {
  if (p.stroke_ms <= 0) return 0;
  const phase = ((position_ms - p.first_beat_ms) / p.stroke_ms) % 1;
  return phase < 0 ? phase + 1 : phase;
}
