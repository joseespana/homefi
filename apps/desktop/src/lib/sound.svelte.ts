// Taste layers, loudness and "How does it sound?": the reactions (with a toast
// and Undo), the "Sound for this…" dialog, the handoff of a suggestion card to
// the assistant and the words the screens use for rules.
import {
  api,
  errorText,
  type AssistantTurn,
  type LoudnessMode,
  type LoudnessSettings,
  type NowSound,
  type SoundReaction,
  type SoundUiSettings,
  type TasteBand,
  type TasteCreator,
  type TasteIntensity,
  type TasteRule,
  type TasteScope,
  type TasteScopeKind,
  type TasteSettings,
  type Tilt,
} from "./api";
import { uiLanguage } from "./eq.svelte";
import { app } from "./state.svelte";

export const SOUND_UI_DEFAULTS: SoundUiSettings = { react_deck: true, react_room: true, layer_chip: true, suggestions: true, taste_editor: "auto" };
export const LOUDNESS_DEFAULTS: LoudnessSettings = { mode: "off", targetLufs: -14, preventClipping: true };
export const TASTE_DEFAULTS: TasteSettings = { enabled: false, rules: [], intensity: "balanced" };
/** The size of the steps the assistant and reactions take (homefi-core `TasteIntensity`). */
export const INTENSITIES: readonly { id: TasteIntensity; label: string; hint: string }[] = [
  { id: "subtle", label: "Subtle", hint: "small, 1.5–3 dB" },
  { id: "balanced", label: "Balanced", hint: "clearly audible, 3–5 dB" },
  { id: "bold", label: "Bold", hint: "strong, 5–8 dB" },
];
/** Each taste value stays within ± this (dB), like homefi-core. */
export const MAX_TILT_DB = 6;

/** The reaction chips, in the order they are shown. */
export const REACTIONS: { id: SoundReaction; label: string; hint: string }[] = [
  { id: "needsBass", label: "Needs more bass", hint: "Thin, no punch" },
  { id: "tooBoomy", label: "Too boomy", hint: "Bass covers everything" },
  { id: "tooBright", label: "Too bright", hint: "Harsh, tiring highs" },
  { id: "dull", label: "Dull", hint: "Muffled, no sparkle" },
  { id: "vocalsBuried", label: "Can't hear the vocals", hint: "Voices far back" },
  { id: "tooQuiet", label: "Quieter than the rest", hint: "Evens out the volume" },
  { id: "tooLoud", label: "Louder than the rest", hint: "Evens out the volume" },
  { id: "perfect", label: "Sounds perfect", hint: "Keep it like this" },
];

export const SCOPE_KINDS: { id: TasteScopeKind; label: string; icon: string }[] = [
  { id: "track", label: "Track", icon: "music" },
  { id: "album", label: "Album", icon: "disc" },
  { id: "playlist", label: "Playlist", icon: "queue" },
  { id: "artist", label: "Artist", icon: "user" },
  { id: "genre", label: "Genre", icon: "layers" },
];

export const LOUDNESS_MODES: { id: LoudnessMode; label: string; hint: string }[] = [
  { id: "off", label: "Off", hint: "Bit-perfect: every song as it was mastered" },
  { id: "auto", label: "Auto", hint: "Album gain when an album plays in order, else per song" },
  { id: "track", label: "Per song", hint: "Every song at the same loudness" },
  { id: "album", label: "Per album", hint: "Albums keep their own dynamics" },
];

export function scopeIcon(kind: TasteScopeKind): string {
  return SCOPE_KINDS.find((k) => k.id === kind)?.icon ?? "layers";
}

/** "the album Discovery", "Electronic music". */
export function scopePhrase(s: TasteScope): string {
  return s.kind === "genre" ? `${s.name} music` : `the ${s.kind} ${s.name || "(untitled)"}`;
}

/** "this album", "Electronic". */
export function scopeShort(s: TasteScope): string {
  return s.kind === "genre" ? s.name : `this ${s.kind}`;
}

export function creatorLabel(c: TasteCreator): string {
  return { manual: "Made by you", ai: "Made with AI", guided: "Guided setup", reaction: "From a reaction" }[c] ?? "Made by you";
}

/** `+2 dB bass · −1 dB treble`. */
export function tiltWords(t: Tilt): string {
  const parts: string[] = [];
  const add = (db: number | null | undefined, what: string) => {
    if (db != null && Math.abs(db) >= 0.05) parts.push(`${db > 0 ? "+" : "−"}${Math.abs(Math.round(db * 10) / 10)} dB ${what}`);
  };
  add(t.bassDb, "bass");
  add(t.trebleDb, "treble");
  add(t.clarityDb, "clarity");
  return parts.length ? parts.join(" · ") : "No change";
}

const kHz = (f: number) => (f >= 1000 ? `${(f / 1000).toFixed(f >= 10_000 ? 0 : 1).replace(/\.0$/, "")}k` : `${Math.round(f)} Hz`);
const dbWords = (db: number) => `${db > 0 ? "+" : "−"}${Math.abs(Math.round(db * 10) / 10)} dB`;
const hasGain = (b: TasteBand) => b.kind === "peak" || b.kind === "lowShelf" || b.kind === "highShelf";
const audible = (b: TasteBand) => b.enabled && (!hasGain(b) || Math.abs(b.gainDb) >= 0.05);

/** `−2 dB 3.2k`, `HP 80 Hz`. */
export function bandWords(b: TasteBand): string {
  if (b.kind === "highPass") return `HP ${kHz(b.freqHz)}`;
  if (b.kind === "lowPass") return `LP ${kHz(b.freqHz)}`;
  return `${dbWords(b.gainDb)} ${kHz(b.freqHz)}`;
}

/** A rule's curve in a line, like homefi-core's `curve_summary`: `3 bands · −2 dB 3.2k, +1.5 dB 120 Hz, …` then the tilt. */
export function ruleWords(r: { tilt: Tilt; bands?: TasteBand[] }): string {
  const bands = (r.bands ?? []).filter(audible);
  const parts: string[] = [];
  if (bands.length) {
    const weight = (b: TasteBand) => (hasGain(b) ? Math.abs(b.gainDb) : 0.5);
    const top = [...bands].sort((a, b) => weight(b) - weight(a));
    const list = top.slice(0, 3).map(bandWords);
    if (top.length > 3) list.push("…");
    parts.push(`${bands.length} band${bands.length === 1 ? "" : "s"} · ${list.join(", ")}`);
  }
  const t = tiltWords(r.tilt);
  if (t !== "No change") parts.push(t);
  return parts.length ? parts.join(" · ") : "No change";
}

/** The rule changes nothing (flat tilt, no audible band). */
export function ruleNeutral(r: { tilt: Tilt; bands?: TasteBand[] }): boolean {
  return tiltWords(r.tilt) === "No change" && !(r.bands ?? []).some(audible);
}

/** A short default label for a manual rule. */
export function defaultLabel(scope: TasteScope, t: Tilt): string {
  const c = t.clarityDb ?? 0;
  const top = [
    [Math.abs(t.bassDb), t.bassDb >= 0 ? "More bass" : "Less bass"],
    [Math.abs(t.trebleDb), t.trebleDb >= 0 ? "Brighter" : "Softer highs"],
    [Math.abs(c), c >= 0 ? "Clearer vocals" : "Smoother mids"],
  ].sort((a, b) => (b[0] as number) - (a[0] as number))[0][1];
  return `${top} · ${scope.name}`;
}

export const sameScope = (a: TasteScope, b: TasteScope) => {
  if (a.kind !== b.kind) return false;
  if (a.kind !== "genre" && b.kind !== "genre" && a.id && b.id) return a.id === b.id;
  return a.name.trim().toLowerCase() === b.name.trim().toLowerCase();
};

const newId = () => `t${Date.now().toString(16)}${Math.floor(Math.random() * 4096).toString(16)}`;
const copy = <T>(v: T): T => JSON.parse(JSON.stringify(v));

class SoundStore {
  /** The reactions popover is open (in the deck or the room). */
  reacting = $state<"deck" | "room" | null>(null);
  busy = $state<SoundReaction | null>(null);
  now = $state<NowSound | null>(null);
  /** "Sound for this…" dialog. */
  forScope = $state<{ scope: TasteScope; label: string } | null>(null);
  /** A suggestion card's turn and the answer tapped, for the assistant to go on with. */
  handoff = $state<{ turn: AssistantTurn; reply: string | null } | null>(null);
  /** Notes to start the assistant with ("Sound for this album → Ask the AI"). */
  assistantNotes = $state<string | null>(null);
  /** The rule "Ask the AI to refine" opened the assistant for. */
  assistantFocus = $state<string | null>(null);

  get ui(): SoundUiSettings {
    return { ...SOUND_UI_DEFAULTS, ...(app.settings?.sound_ui ?? {}) };
  }
  get taste(): TasteSettings {
    return { ...TASTE_DEFAULTS, ...(app.settings?.taste ?? {}) };
  }
  get loudness(): LoudnessSettings {
    return { ...LOUDNESS_DEFAULTS, ...(app.settings?.loudness ?? {}) };
  }

  async refreshNow() {
    try {
      this.now = await api.soundNow();
    } catch {
      this.now = null;
    }
  }

  open(place: "deck" | "room") {
    this.reacting = this.reacting === place ? null : place;
    if (this.reacting) this.refreshNow();
  }

  /** One tap: the rule is saved and applied by the backend; a toast says what changed, with Undo. */
  async react(reaction: SoundReaction, scope: TasteScope | null = null) {
    if (this.busy) return;
    this.busy = reaction;
    try {
      const r = await api.soundReact(reaction, scope, uiLanguage());
      this.reacting = null;
      const undo = r.undoToken ? { label: "Undo", run: () => this.undo(r.undoToken) } : undefined;
      app.note(r.message, r.undoToken ? "success" : "info", "sound-react", { seconds: undo ? 8 : 4, action: undo });
      this.refreshNow();
    } catch (e) {
      app.note(errorText(e), "error", "sound-react");
    } finally {
      this.busy = null;
    }
  }

  async undo(token: string) {
    try {
      const ok = await api.soundUndo(token);
      app.note(ok ? "Back to how it was" : "Already undone", "info", "sound-react");
      this.refreshNow();
    } catch (e) {
      app.note(errorText(e), "error", "sound-react");
    }
  }

  async saveTaste(next: TasteSettings) {
    try {
      const saved = await api.tasteSet(next);
      if (app.settings) app.settings = { ...app.settings, taste: saved };
    } catch (e) {
      app.notify(errorText(e), true);
    }
  }

  async saveLoudness(next: LoudnessSettings) {
    try {
      const saved = await api.loudnessSet(next);
      if (app.settings) app.settings = { ...app.settings, loudness: saved };
    } catch (e) {
      app.notify(errorText(e), true);
    }
  }

  /** Adds or replaces the rule for `scope` (the first rule turns taste layers on). */
  async setRule(scope: TasteScope, tilt: Tilt, label?: string, createdBy: TasteCreator = "manual", bands?: TasteBand[]) {
    const t = copy(this.taste);
    const at = t.rules.findIndex((r) => sameScope(r.scope, scope));
    const old = at >= 0 ? t.rules[at] : null;
    const rule: TasteRule = {
      ...(old ?? {}),
      id: old?.id ?? newId(),
      scope,
      tilt,
      bands: bands ?? old?.bands ?? [],
      label: label?.trim() || defaultLabel(scope, tilt),
      createdBy: old?.createdBy ?? createdBy,
      enabled: true,
    };
    if (!t.rules.length) t.enabled = true;
    if (at >= 0) t.rules[at] = rule;
    else t.rules.push(rule);
    await this.saveTaste(t);
  }

  async patchRule(id: string, patch: Partial<TasteRule>) {
    const t = copy(this.taste);
    t.rules = t.rules.map((r) => (r.id === id ? { ...r, ...patch } : r));
    await this.saveTaste(t);
  }

  async removeRule(id: string) {
    const t = copy(this.taste);
    t.rules = t.rules.filter((r) => r.id !== id);
    await this.saveTaste(t);
  }

  ruleFor(scope: TasteScope): TasteRule | null {
    return this.taste.rules.find((r) => sameScope(r.scope, scope)) ?? null;
  }

  /** Opens the assistant about one scope. */
  askAi(scope: TasteScope) {
    this.forScope = null;
    this.assistantNotes = `I'd like a taste layer for ${scopePhrase(scope)}.`;
    app.view = { name: "assistant" };
  }

  /** "Ask the AI to refine": the assistant starts from this rule's curve (save the edits first). */
  refine(rule: TasteRule) {
    this.forScope = null;
    this.assistantFocus = rule.id;
    this.assistantNotes = `Refine “${rule.label}” (${ruleWords(rule)}).`;
    app.view = { name: "assistant" };
  }
}

export const sound = new SoundStore();
