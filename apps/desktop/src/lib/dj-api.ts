// DJ mode: the types and calls of src-tauri/src/dj.rs (snake_case JSON, as homefi_player::dj
// and homefi_core::dj serialize them). The state lives in lib/dj.svelte.ts.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type DeckId = "a" | "b";
export const DECKS: readonly DeckId[] = ["a", "b"];
export const deckIndex = (d: DeckId): 0 | 1 => (d === "a" ? 0 : 1);
export const otherDeck = (d: DeckId): DeckId => (d === "a" ? "b" : "a");

export type DjEqBand = "low" | "mid" | "high";
/** Index of a band in `DeckSnapshot.eq_db` / `eq_kill`. */
export const EQ_BAND_INDEX: Record<DjEqBand, number> = { low: 0, mid: 1, high: 2 };
export type CrossfaderCurve = "smooth" | "linear" | "cut";
export type DjLatency = "low" | "balanced" | "safe";
export type LoadTarget = "free_deck" | "ask" | "crate";

/** homefi_core::dj::HomifitoDj: Homifito, the mascot at his booth (the same JSON on Android). */
export interface HomifitoDjSettings {
  mode: "off" | "booth" | "big";
  place: "top" | "bottom";
  /** The board in Homifito's colours. */
  look: boolean;
}

/** homefi_core::dj::DjSectionKind: the blocks of the DJ screen. */
export type DjSectionKind = "deck_a" | "mixer" | "deck_b" | "sampler" | "crate";
/** Today's order on screen (DjSectionKind::ALL). */
export const DJ_SECTION_KINDS: readonly DjSectionKind[] = ["deck_a", "mixer", "deck_b", "sampler", "crate"];
export interface DjSection {
  kind: DjSectionKind;
  visible: boolean;
}

/** homefi_core::dj::SfxKind: the sound a sampler pad plays. */
export type SfxKind = "air_horn" | "siren" | "laser" | "rewind" | "drop" | "scratch" | "riser" | "hit";
/** Pad order by default (SfxKind::ALL): pad `i` plays `SFX_KINDS[i]`. */
export const SFX_KINDS: readonly SfxKind[] = ["air_horn", "siren", "laser", "rewind", "drop", "scratch", "riser", "hit"];

/** homefi_core::dj::DjLayout: the order and visibility of the sections and the sound of each pad. */
export interface DjLayout {
  sections: DjSection[];
  pads: SfxKind[];
}

/** homefi_core::dj::DjSettings (the same JSON on Android). */
export interface DjSettings {
  latency: DjLatency;
  tempo_range_pct: number;
  bend_pct: number;
  quantize: boolean;
  crossfader_curve: CrossfaderCurve;
  master_limiter: boolean;
  auto_gain: boolean;
  eq_max_boost_db: number;
  sampler_volume: number;
  sync_on_load: boolean;
  auto_mix_beats: number;
  load_target: LoadTarget;
  memory_mb: number;
  keys_enabled: boolean;
  key_overrides: Record<string, string>;
  waveform_colors: boolean;
  zoom_beats: number;
  show_beat_grid: boolean;
  bass_swap: boolean;
  simple_view: boolean;
  show_tips: boolean;
  show_entry: boolean;
  layout: DjLayout;
  homifito: HomifitoDjSettings;
}

// Ranges and defaults: a copy of homefi_core::dj::settings (the core clamps whatever is sent).
export const DJ_TEMPO_RANGES = [8, 16, 50] as const;
export const DJ_TEMPO_RANGE = { min: 4, max: 50 } as const;
export const DJ_BEND_RANGE = { min: 1, max: 10 } as const;
export const DJ_MEMORY_RANGE = { min: 128, max: 4096, step: 128 } as const;
export const DJ_EQ_BOOST_RANGE = { min: 0, max: 12 } as const;
export const DJ_ZOOM_RANGE = { min: 2, max: 32 } as const;
export const DJ_MIX_BEATS = [8, 16, 32] as const;
/** homefi_core::dj::mixer_math: a killed band, the trim's travel. */
export const DJ_EQ_KILL_DB = -48;
export const DJ_TRIM_DB = { min: -12, max: 12 } as const;
/** homefi_player::dj: master volume travel. */
export const DJ_MASTER_MAX = 1.5;
/** homefi_core::dj::analysis: below this the beat grid is not trusted (and not drawn). */
export const DJ_MIN_GRID_CONFIDENCE = 0.25;
export const DJ_HOT_CUES = 8;
/** Narrower windows (px) show the crate as a drawer (docs/DJ_UI.md §2). */
export const DJ_DRAWER_WIDTH_PX = 1180;
export const DJ_PADS = 8;

export const defaultDjLayout = (): DjLayout => ({ sections: DJ_SECTION_KINDS.map((kind) => ({ kind, visible: true })), pads: [...SFX_KINDS] });

export const DEFAULT_DJ_SETTINGS: DjSettings = {
  latency: "balanced",
  tempo_range_pct: 8,
  bend_pct: 4,
  quantize: true,
  crossfader_curve: "smooth",
  master_limiter: true,
  auto_gain: true,
  eq_max_boost_db: 6,
  sampler_volume: 0.8,
  sync_on_load: true,
  auto_mix_beats: 16,
  load_target: "free_deck",
  memory_mb: 1024,
  keys_enabled: true,
  key_overrides: {},
  waveform_colors: true,
  zoom_beats: 8,
  show_beat_grid: true,
  bass_swap: true,
  simple_view: true,
  show_tips: true,
  show_entry: true,
  layout: defaultDjLayout(),
  homifito: { mode: "booth", place: "top", look: false },
};
/** The setting with defaults for whatever an older settings file doesn't have. */
export const djSettingsOf = (s: { dj?: Partial<DjSettings> } | null | undefined): DjSettings => ({
  ...DEFAULT_DJ_SETTINGS,
  ...s?.dj,
  homifito: { ...DEFAULT_DJ_SETTINGS.homifito, ...s?.dj?.homifito },
});

export interface DjTrackInfo {
  title: string;
  duration_ms: number;
  sample_rate: number;
  codec: string;
  /** 0 when no tempo was found. */
  bpm: number;
  first_beat_ms: number;
  beat_ms: number;
  has_grid: boolean;
  confidence: number;
  loudness_db: number;
  auto_gain_db: number;
  bytes: number;
}

export interface DeckSnapshot {
  track: DjTrackInfo | null;
  loading: boolean;
  error: string | null;
  playing: boolean;
  position_ms: number;
  cue_ms: number;
  hot_cues: (number | null)[];
  loop_ms: [number, number] | null;
  tempo_percent: number;
  current_bpm: number;
  synced: boolean;
  bend: number;
  trim_db: number;
  /** low, mid, high (dB). */
  eq_db: [number, number, number];
  eq_kill: [boolean, boolean, boolean];
  filter: number;
  volume: number;
  peak: [number, number];
}

export interface DjSnapshot {
  running: boolean;
  output: { device: string; sample_rate: number; channels: number; buffer_frames: number | null };
  device_lost: string | null;
  decks: [DeckSnapshot, DeckSnapshot];
  crossfader: number;
  crossfader_curve: CrossfaderCurve;
  master: number;
  master_peak: [number, number];
  limiting: boolean;
  auto_mix: { to: DeckId; beats: number; armed: boolean; progress: number } | null;
  sampler_volume: number;
  pads_active: boolean[];
  memory_used_mb: number;
  memory_limit_mb: number;
}

export interface DjState {
  on: boolean;
  snapshot: DjSnapshot | null;
  /** What each deck holds, as the screen named it. */
  decks: [DjPick | null, DjPick | null];
}

/** Everything the user can do (homefi_player::dj::DjControl). */
export type DjControl =
  | { type: "play" | "pause" | "toggle_play" | "loop_in" | "loop_out" | "loop_exit" | "sync" | "sync_off" | "eject"; deck: DeckId }
  | { type: "cue"; deck: DeckId; pressed: boolean }
  | { type: "seek" | "nudge"; deck: DeckId; ms: number }
  | { type: "bend"; deck: DeckId; amount: number }
  | { type: "hot_cue" | "set_hot_cue" | "clear_hot_cue"; deck: DeckId; index: number }
  | { type: "loop_beats"; deck: DeckId; beats: number }
  | { type: "tempo"; deck: DeckId; percent: number }
  | { type: "trim"; deck: DeckId; db: number }
  | { type: "eq"; deck: DeckId; band: DjEqBand; db: number }
  | { type: "eq_kill"; deck: DeckId; band: DjEqBand; on: boolean }
  | { type: "filter" | "volume"; deck: DeckId; value: number }
  | { type: "crossfader" | "master" | "sampler_volume"; value: number }
  | { type: "set_curve"; curve: CrossfaderCurve }
  | { type: "auto_mix"; to: DeckId; beats: number }
  | { type: "auto_mix_cancel" }
  | { type: "sampler"; pad: number };

export type DjEvent =
  | { type: "loading"; deck: DeckId }
  | { type: "loaded"; deck: DeckId; track: DjTrackInfo }
  | { type: "load_failed"; deck: DeckId; message: string }
  | { type: "device_lost"; message: string };

export type DjSource = { kind: "path"; path: string; title?: string } | { kind: "tidal"; id: string; title?: string; local?: string | null };

/** A song picked elsewhere in the app while DJ mode is on. */
export interface DjPick {
  source: DjSource;
  title: string;
  artist: string;
  cover_url: string | null;
  duration_ms: number | null;
  deck: DeckId | null;
}

export interface DjOverview {
  duration_ms: number;
  bpm: number;
  first_beat_ms: number;
  beat_ms: number;
  confidence: number;
  slices_per_second: number;
  /** base64 of [peak, low, mid, high] bytes per slice. */
  slices: string;
}

export interface DjBinding {
  action: string;
  key: string;
}
export interface DjKeymap {
  bindings: DjBinding[];
  conflicts: [string, string[]][];
}

export const djApi = {
  enable: (on: boolean) => invoke<DjState>("dj_enable", { on }),
  state: () => invoke<DjState>("dj_state"),
  load: (deck: DeckId, pick: DjPick) => invoke<void>("dj_load", { deck, pick }),
  control: (control: DjControl) => invoke<void>("dj_control", { control }),
  overview: (deck: DeckId) => invoke<DjOverview | null>("dj_overview", { deck }),
  keymap: () => invoke<DjKeymap>("dj_keymap"),
  cachedBpm: (paths: string[]) => invoke<(number | null)[]>("dj_cached_bpm", { paths }),
};

export const onDj = {
  state: (f: (s: DjState) => void): Promise<UnlistenFn> => listen<DjState>("dj", (e) => f(e.payload)),
  load: (f: (e: DjEvent) => void): Promise<UnlistenFn> => listen<DjEvent>("dj-load", (e) => f(e.payload)),
  picked: (f: (p: DjPick) => void): Promise<UnlistenFn> => listen<DjPick>("dj-picked", (e) => f(e.payload)),
  ask: (f: (p: DjPick) => void): Promise<UnlistenFn> => listen<DjPick>("dj-ask", (e) => f(e.payload)),
  crate: (f: (p: DjPick) => void): Promise<UnlistenFn> => listen<DjPick>("dj-crate", (e) => f(e.payload)),
  /** The list of setups changed (no payload). */
  setups: (f: () => void): Promise<UnlistenFn> => listen("dj-setups", () => f()),
};

/** The overview's slices as bytes (4 per slice: peak, low, mid, high). */
export function overviewBytes(o: DjOverview): Uint8Array {
  const raw = atob(o.slices);
  const out = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i++) out[i] = raw.charCodeAt(i);
  return out;
}

/** The words and the icon of each pad sound (the icons are Icon.svelte's `sfx*`). */
export const SFX: Record<SfxKind, { name: string; icon: string }> = {
  air_horn: { name: "Air horn", icon: "sfxHorn" },
  siren: { name: "Siren", icon: "sfxSiren" },
  laser: { name: "Laser", icon: "sfxLaser" },
  rewind: { name: "Rewind", icon: "sfxRewind" },
  drop: { name: "Drop", icon: "sfxDrop" },
  scratch: { name: "Scratch", icon: "sfxScratch" },
  riser: { name: "Riser", icon: "sfxRiser" },
  hit: { name: "Hit", icon: "sfxHit" },
};

/** The screen's sections in words. */
export const DJ_SECTION_INFO: Record<DjSectionKind, { name: string; icon: string; hint: string }> = {
  deck_a: { name: "Deck A", icon: "disc", hint: "The first deck: the song, its waveform and its buttons" },
  mixer: { name: "Mixer", icon: "eq", hint: "Crossfader, EQ, the Mix button" },
  deck_b: { name: "Deck B", icon: "disc", hint: "The second deck" },
  sampler: { name: "Sampler", icon: "sfxHorn", hint: "The sound-effect pads" },
  crate: { name: "Crate", icon: "queue", hint: "Songs set aside for the set" },
};

// ------------------------------------------------------------------ setups (src-tauri/src/dj_setups.rs)

/** A named setup as a list shows it. */
export interface DjSetupSummary {
  id: string;
  name: string;
  created_ms: number;
  updated_ms: number;
}

export interface DjSetupApplied {
  settings: DjSettings;
  /** DJ mode is on and the pad sounds changed: they play from the next start of DJ mode. */
  restart_needed: boolean;
}

/** The file kind the setup export dialogs use. */
export const DJ_SETUP_FILE = { extension: "homefidj", label: "HomeFi DJ setup" } as const;

export const djSetupApi = {
  list: () => invoke<DjSetupSummary[]>("dj_setups"),
  save: (name: string) => invoke<DjSetupSummary>("dj_setup_save", { name }),
  overwrite: (id: string) => invoke<DjSetupSummary>("dj_setup_overwrite", { id }),
  rename: (id: string, name: string) => invoke<DjSetupSummary>("dj_setup_rename", { id, name }),
  apply: (id: string) => invoke<DjSetupApplied>("dj_setup_apply", { id }),
  remove: (id: string) => invoke<boolean>("dj_setup_delete", { id }),
  export: (id: string, path: string) => invoke<string>("dj_setup_export", { id, path }),
  import: (path: string) => invoke<DjSetupSummary>("dj_setup_import", { path }),
};
