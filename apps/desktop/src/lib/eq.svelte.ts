// The equalizer as the screens edit it: the settings (a profile per output
// and a default for the rest), saved and applied live a moment after each
// change, plus small helpers for drawing and wording.
import { api, EQ_LIMITS, errorText, type EqBand, type EqBandKind, type EqProfile, type EqSettings, type OutputDevice } from "./api";
import { app } from "./state.svelte";

/** The target that edits `defaultProfile` ("other outputs"). */
export const OTHER_OUTPUTS = "";

/** Edits are saved and reach the player at most this often (and once more after the last one). */
const SAVE_DELAY_MS = 140;

export const BAND_KINDS: { id: EqBandKind; label: string; short: string }[] = [
  { id: "peak", label: "Peak", short: "PK" },
  { id: "lowShelf", label: "Low shelf", short: "LS" },
  { id: "highShelf", label: "High shelf", short: "HS" },
  { id: "lowPass", label: "Low pass", short: "LP" },
  { id: "highPass", label: "High pass", short: "HP" },
];

/** Bands that boost or cut (pass filters only have a frequency and a slope). */
export const hasGain = (kind: EqBandKind) => kind === "peak" || kind === "lowShelf" || kind === "highShelf";

export const clamp = (v: number, [lo, hi]: readonly [number, number]) => Math.min(hi, Math.max(lo, Number.isFinite(v) ? v : lo));

export function flatProfile(name = "Flat"): EqProfile {
  return { name, preampDb: 0, bands: [], source: { kind: "manual" } };
}

export function copyProfile(p: EqProfile): EqProfile {
  return JSON.parse(JSON.stringify(p));
}

export function formatHz(f: number): string {
  if (f >= 1000) return `${(f / 1000).toFixed(f >= 10_000 ? 1 : 2).replace(/\.?0+$/, "")} kHz`;
  return `${Math.round(f)} Hz`;
}

export const formatDb = (db: number) => `${db > 0 ? "+" : db < 0 ? "−" : ""}${Math.abs(db).toFixed(1)} dB`;

/** Where a profile came from, in a few words. */
export function sourceLabel(p: EqProfile): string {
  switch (p.source.kind) {
    case "autoEq":
      return `AutoEq · measured by ${p.source.source}`;
    case "assistant":
      return "Made with the assistant";
    case "guided":
      return "Made with the guided setup";
    default:
      return "Manual";
  }
}

/** A new band where there is room for one (an octave away from the others when possible). */
export function newBand(bands: EqBand[], freqHz?: number, gainDb = 0): EqBand {
  if (freqHz === undefined) {
    const spots = [1000, 125, 4000, 250, 8000, 60, 500, 2000, 16000, 32];
    freqHz = spots.find((f) => bands.every((b) => Math.abs(Math.log2(b.freqHz / f)) > 0.7)) ?? 1000;
  }
  return { kind: "peak", freqHz: clamp(freqHz, EQ_LIMITS.freqHz), gainDb: clamp(gainDb, EQ_LIMITS.gainDb), q: 1, enabled: true };
}

/** "es" when the system speaks Spanish, else "en" (the assistant talks in it). */
export function uiLanguage(): "es" | "en" {
  const langs = typeof navigator === "undefined" ? [] : [navigator.language, ...(navigator.languages ?? [])];
  return langs.some((l) => l?.toLowerCase().startsWith("es")) ? "es" : "en";
}

class EqStore {
  settings = $state<EqSettings | null>(null);
  /** Output whose profile is edited, or OTHER_OUTPUTS. */
  target = $state<string>(OTHER_OUTPUTS);
  devices = $state<OutputDevice[]>([]);
  error = $state<string | null>(null);
  /** Edits not saved yet: the settings coming back from Rust don't overwrite them. */
  private edits = 0;
  private saved = 0;
  private timer: ReturnType<typeof setTimeout> | null = null;

  /** The output playing now (what "this output" means). */
  get playingOn(): string | null {
    const p = app.player;
    return p?.stream?.device ?? p?.preferred_device ?? null;
  }

  async load(output?: string) {
    this.settings = copy(app.settings?.eq ?? (await api.eqGet()));
    this.target = output ?? this.playingOn ?? OTHER_OUTPUTS;
    api.outputDevices().then((d) => (this.devices = d)).catch(() => {});
  }

  /** Settings saved elsewhere (Settings → Sound, the assistant) show up here. */
  sync(eq: EqSettings | undefined) {
    if (eq && this.edits === this.saved && !this.timer) this.settings = copy(eq);
  }

  /** Outputs to choose from: the devices, plus those with a profile that aren't plugged in. */
  get outputs(): { name: string; connected: boolean }[] {
    // one entry per name: twin devices (two monitors of the same model) are one choice
    const names = [...new Set(this.devices.map((d) => d.name))].map((name) => ({ name, connected: true }));
    const playing = this.playingOn;
    if (playing && !names.some((n) => n.name === playing)) names.unshift({ name: playing, connected: true });
    for (const name of Object.keys(this.settings?.perOutput ?? {})) {
      if (!names.some((n) => n.name === name)) names.push({ name, connected: false });
    }
    return names;
  }

  /** The target's own profile (null: it follows the default / is flat). */
  own(target = this.target): EqProfile | null {
    const s = this.settings;
    if (!s) return null;
    return target === OTHER_OUTPUTS ? s.defaultProfile : (s.perOutput[target] ?? null);
  }

  /** What the target plays with: its own profile, or the default one. */
  effective(target = this.target): EqProfile | null {
    const s = this.settings;
    if (!s) return null;
    return target === OTHER_OUTPUTS ? s.defaultProfile : (s.perOutput[target] ?? s.defaultProfile);
  }

  /** Changes the settings now and saves them a moment later. */
  update(change: (s: EqSettings) => void) {
    if (!this.settings) return;
    const next = copy(this.settings);
    change(next);
    this.settings = next;
    this.edits++;
    // at most one save per SAVE_DELAY_MS, with the newest settings: a long drag is heard as it goes
    this.timer ??= setTimeout(() => this.flush(), SAVE_DELAY_MS);
  }

  /** Sets (or with null removes) the target's own profile. */
  setProfile(profile: EqProfile | null, target = this.target) {
    this.update((s) => {
      if (target === OTHER_OUTPUTS) s.defaultProfile = profile && copy(profile);
      else if (profile) s.perOutput[target] = copy(profile);
      else delete s.perOutput[target];
    });
  }

  /** Edits the target's profile; the first edit gives the target its own (from what it played). */
  edit(change: (p: EqProfile) => void) {
    const base = copy(this.own() ?? this.effective() ?? flatProfile());
    change(base);
    this.setProfile(base);
  }

  async flush() {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    if (!this.settings) return;
    const upTo = this.edits;
    try {
      await api.eqSet(copy(this.settings));
      this.error = null;
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.saved = Math.max(this.saved, upTo);
    }
  }
}

function copy<T>(v: T): T {
  return JSON.parse(JSON.stringify(v));
}

export const eq = new EqStore();
