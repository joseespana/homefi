// Sounds, loudness and "How does it sound?": what plays now, the moves between
// sounds (with a toast and Undo), Try / Keep, the reactions and the words the
// screens use for sounds. The rules themselves belong to the backend: this
// store only asks for changes and refreshes from the `settings` event.
import {
  api,
  errorText,
  type EqProfile,
  type LoudnessMode,
  type LoudnessSettings,
  type NowSound,
  type PlayerState,
  type SavedSound,
  type SoundReaction,
  type SoundUiSettings,
  type TargetHit,
  type TargetMove,
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
export const TASTE_DEFAULTS: TasteSettings = { enabled: false, rules: [], intensity: "balanced", version: 1 };
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

/** Most specific first, like the backend's precedence (song > album > artist > playlist > genre). */
export const SCOPE_KINDS: { id: TasteScopeKind; label: string; plural: string; badge: string; icon: string }[] = [
  { id: "track", label: "Song", plural: "Songs", badge: "SO", icon: "music" },
  { id: "album", label: "Album", plural: "Albums", badge: "AL", icon: "disc" },
  { id: "artist", label: "Artist", plural: "Artists", badge: "AR", icon: "user" },
  { id: "playlist", label: "Playlist", plural: "Playlists", badge: "PL", icon: "queue" },
  { id: "genre", label: "Genre", plural: "Genres", badge: "GE", icon: "layers" },
];
/** The order a sound lists its targets in (artists first: what most sounds are about). */
const SUMMARY_ORDER: TasteScopeKind[] = ["artist", "album", "playlist", "track", "genre"];
/** Targets named in a sound's one-line summary before "+N". */
const SUMMARY_NAMES = 2;
/** How long "moves from…" notes keep their Undo (s). */
export const UNDO_SECONDS = 8;
/** Why a Try can't start, as the note says it. */
const TRY_REFUSED = { off: "Turn Sound on to try it", idle: "Play something to try it" } as const;

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

const kindInfo = (kind: TasteScopeKind) => SCOPE_KINDS.find((k) => k.id === kind) ?? SCOPE_KINDS[SCOPE_KINDS.length - 1];
export const kindLabel = (kind: TasteScopeKind) => kindInfo(kind).label;
export const kindBadge = (kind: TasteScopeKind) => kindInfo(kind).badge;
/** Where a kind goes in precedence order (0 = most specific). */
export const kindRank = (kind: TasteScopeKind) => SCOPE_KINDS.findIndex((k) => k.id === kind);

/** The same target (ids when both have one, else names), for showing what a sound covers. The backend decides what really matches. */
export function sameScope(a: TasteScope, b: TasteScope): boolean {
  if (a.kind !== b.kind) return false;
  if (a.kind !== "genre" && b.kind !== "genre" && a.id && b.id) return a.id === b.id;
  return a.name.trim().toLowerCase() === b.name.trim().toLowerCase();
}

export const holds = (rule: TasteRule, scope: TasteScope) => rule.targets.some((t) => sameScope(t.scope, scope));

/** Two sounds can play on the same output (an empty list means every output). */
export function sharesOutput(a: TasteRule, b: TasteRule): boolean {
  const x = a.outputs ?? [];
  const y = b.outputs ?? [];
  return !x.length || !y.length || x.some((o) => y.includes(o));
}

/** Plays on `output` (or on every output). */
export const playsOn = (rule: TasteRule, output: string | null) => !rule.outputs?.length || (!!output && rule.outputs.includes(output));

/** The targets of a sound as its row shows them, main kind first. */
function summaryTargets(rule: TasteRule): TasteScope[] {
  return rule.targets.map((t) => t.scope).sort((a, b) => SUMMARY_ORDER.indexOf(a.kind) - SUMMARY_ORDER.indexOf(b.kind));
}

/** The kind that stands for a sound in lists (badge). */
export function mainKind(rule: TasteRule): TasteScopeKind | null {
  return summaryTargets(rule)[0]?.kind ?? null;
}

/** "Eagles, Fleetwood Mac +3". */
export function targetsSummary(rule: TasteRule): string {
  const all = summaryTargets(rule);
  if (!all.length) return "Applies to nothing yet";
  const names = all.slice(0, SUMMARY_NAMES).map((s) => s.name || "Untitled");
  return all.length > SUMMARY_NAMES ? `${names.join(", ")} +${all.length - SUMMARY_NAMES}` : names.join(", ");
}

/** "All outputs", "WH-1000XM5 only", "2 outputs". */
export function outputsLabel(rule: TasteRule): string {
  const o = rule.outputs ?? [];
  return !o.length ? "All outputs" : o.length === 1 ? `${o[0]} only` : `${o.length} outputs`;
}

/** Whether the output's own curve changes anything on the device playing now (a flat one doesn't). */
export function outputCurveOn(p: Pick<PlayerState, "eq_profile" | "stream"> | null | undefined): boolean {
  const eq = app.settings?.eq;
  if (!p?.eq_profile || !eq || !p.stream) return false;
  const profile = eq.perOutput[p.stream.device] ?? eq.defaultProfile ?? null;
  return !!profile?.bands.some((b) => b.enabled);
}

/** What the deck and Settings say plays: the sound first, "Warm and airy · Eagles + HD 650";
 *  the output's curve only when it changes something. */
export function layersWords(p: Pick<PlayerState, "eq_profile" | "taste_label" | "taste_target" | "stream"> | null | undefined): string | null {
  if (!p) return null;
  const sound = p.taste_label ? (p.taste_target?.name ? `${p.taste_label} · ${p.taste_target.name}` : p.taste_label) : null;
  return [sound, sound && !outputCurveOn(p) ? null : p.eq_profile].filter(Boolean).join(" + ") || null;
}

/** Just the name of the sound playing ("Warm and airy"), for the small tag under the song. */
export function soundName(p: Pick<PlayerState, "eq_profile" | "taste_label"> | null | undefined): string {
  return p?.taste_label || p?.eq_profile || "Custom";
}

/** Who measured a correction ("oratory1990") or what it is called. */
export function correctionName(p: EqProfile): string {
  return p.source.kind === "autoEq" ? p.source.source : p.name;
}

/** A rule that changes nothing, to start a new sound from. */
export function neutralRule(label: string, extra: Partial<TasteRule> = {}): TasteRule {
  return { id: newId(), label, tilt: { bassDb: 0, trebleDb: 0, clarityDb: null }, bands: [], createdBy: "manual", enabled: true, targets: [], ...extra };
}

export const newTarget = (scope: TasteScope, addedBy: TasteCreator = "manual") => ({ scope, addedMs: Date.now(), addedBy });

const newId = () => `s${Date.now().toString(16)}${Math.floor(Math.random() * 65_536).toString(16)}`;

class SoundStore {
  /** The reactions popover is open (in the deck or the room). */
  reacting = $state<"deck" | "room" | "top" | null>(null);
  busy = $state<SoundReaction | null>(null);
  /** The sound playing now; replaced wholesale. */
  now = $state.raw<NowSound | null>(null);
  /** A proposal being tried (the TryBar's label while `now` catches up). */
  trying = $state.raw<TasteRule | null>(null);
  /** The Ask-the-AI panel on the Sound screen is open, and what to put in its field. */
  askOpen = $state(false);
  askSeed = $state<string | null>(null);
  /** "Only on this output" for the sound a Keep or "New sound from this song" creates. */
  onlyHere = $state(false);
  /** "Sound ▸" menu anchored at a point (the album / playlist buttons). */
  menu = $state<{ scope: TasteScope; label: string; x: number; y: number } | null>(null);
  /** Output devices by name (for the output chips); loaded on demand. */
  devices = $state.raw<string[]>([]);
  private nowSeq = 0;

  get ui(): SoundUiSettings {
    return { ...SOUND_UI_DEFAULTS, ...(app.settings?.sound_ui ?? {}) };
  }
  get taste(): TasteSettings {
    return { ...TASTE_DEFAULTS, ...(app.settings?.taste ?? {}) };
  }
  get loudness(): LoudnessSettings {
    return { ...LOUDNESS_DEFAULTS, ...(app.settings?.loudness ?? {}) };
  }
  get rules(): TasteRule[] {
    return this.taste.rules;
  }
  /** The master switch: off is bit-perfect. */
  get on(): boolean {
    return app.settings?.eq?.enabled ?? false;
  }
  async loadDevices() {
    try {
      this.devices = [...new Set((await api.outputDevices()).map((d) => d.name))];
    } catch {
      // the chips fall back to the outputs the sounds and profiles already name
    }
  }
  /** Outputs to choose from: devices, the one playing, those with a profile and those some sound names. */
  outputChoices(): string[] {
    const named = this.rules.flatMap((r) => r.outputs ?? []);
    return [...new Set([...(this.output ? [this.output] : []), ...this.devices, ...Object.keys(app.settings?.eq?.perOutput ?? {}), ...named])];
  }
  ruleById(id: string | null | undefined): TasteRule | null {
    return id ? (this.rules.find((r) => r.id === id) ?? null) : null;
  }
  /** The output the player is on (what "this output" means). */
  get output(): string | null {
    const p = app.player;
    return this.now?.output ?? p?.stream?.device ?? p?.preferred_device ?? null;
  }

  /** The correction an output plays with (its own profile, else the default one), or null. */
  profileFor(output: string | null): EqProfile | null {
    const eq = app.settings?.eq;
    return (output && eq?.perOutput?.[output]) || eq?.defaultProfile || null;
  }

  /** The sound that holds `scope` on the current output, if any. */
  holder(scope: TasteScope): TasteRule | null {
    const out = this.output;
    return this.rules.find((r) => holds(r, scope) && playsOn(r, out)) ?? this.rules.find((r) => holds(r, scope)) ?? null;
  }

  async refreshNow() {
    const my = ++this.nowSeq;
    try {
      const n = await api.soundNow();
      if (my !== this.nowSeq) return;
      this.now = n;
      if (!n.auditioning) this.trying = null;
    } catch {
      if (my === this.nowSeq) this.now = null;
    }
  }

  open(place: "deck" | "room" | "top") {
    this.reacting = this.reacting === place ? null : place;
    if (this.reacting) this.refreshNow();
  }

  /** One tap: the rule is saved and applied by the backend; a toast says what changed, with Undo. */
  async react(reaction: SoundReaction, scope: TasteScope | null = null, language: string = uiLanguage()) {
    if (this.busy) return;
    this.busy = reaction;
    try {
      const r = await api.soundReact(reaction, scope, language);
      this.reacting = null;
      const undo = r.undoToken ? { label: "Undo", run: () => this.undo(r.undoToken) } : undefined;
      app.note(r.message, r.undoToken ? "success" : "info", "sound-react", { seconds: undo ? UNDO_SECONDS : 4, action: undo });
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

  async saveUi(patch: Partial<SoundUiSettings>) {
    try {
      await app.saveSettings({ sound_ui: { ...this.ui, ...patch } });
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

  // ------------------------------------------------------------ moves between sounds

  /** "Korn moves from “System Of A Down” to “Lasgo”", with Undo for a few seconds. */
  private announce(moves: TargetMove[], to: TasteRule | null, scope: TasteScope | null, added: string) {
    const undo = moves.length ? { label: "Undo", run: () => this.undoMoves(moves) } : scope && to ? { label: "Undo", run: () => this.unassign(to.id, scope, true) } : undefined;
    let text = added;
    if (moves.length) {
      const first = moves[0];
      const from = this.ruleById(first.from)?.label ?? "another sound";
      text = moves.length === 1 ? `${first.target.name || "It"} moves from “${from}” to “${to?.label ?? "this sound"}”` : `${moves.length} items move to “${to?.label ?? "this sound"}”`;
    }
    app.note(text, "success", "sound-moves", { seconds: UNDO_SECONDS, action: undo });
  }

  async assign(soundId: string, scope: TasteScope) {
    try {
      const moves = await api.tasteAssign(soundId, scope);
      const to = this.ruleById(soundId);
      this.announce(moves, to, scope, `Added ${scope.name || "it"} to “${to?.label ?? "the sound"}”`);
      this.refreshNow();
    } catch (e) {
      app.note(errorText(e), "error", "sound-moves");
    }
  }

  async unassign(soundId: string, scope: TasteScope, quiet = false) {
    const from = this.ruleById(soundId);
    try {
      await api.tasteUnassign(soundId, scope);
      if (!quiet) app.note(`${scope.name || "It"} left “${from?.label ?? "the sound"}”`, "success", "sound-moves", { seconds: UNDO_SECONDS, action: { label: "Undo", run: () => this.assign(soundId, scope) } });
      this.refreshNow();
    } catch (e) {
      app.note(errorText(e), "error", "sound-moves");
    }
  }

  async undoMoves(moves: TargetMove[]) {
    try {
      await api.tasteUndoMoves(moves);
      app.note("Back to how it was", "info", "sound-moves");
      this.refreshNow();
    } catch (e) {
      app.note(errorText(e), "error", "sound-moves");
    }
  }

  async setOutputs(soundId: string, outputs: string[]) {
    try {
      const moves = await api.tasteSetOutputs(soundId, outputs);
      if (moves.length) this.announce(moves, this.ruleById(soundId), null, "");
      this.refreshNow();
    } catch (e) {
      app.note(errorText(e), "error", "sound-moves");
    }
  }

  /** New, rename, curve edits. Returns what the backend saved (null when it failed). */
  async save(rule: TasteRule, quiet = false): Promise<SavedSound | null> {
    try {
      const saved = await api.tasteSave(JSON.parse(JSON.stringify(rule)));
      if (saved.moves.length && !quiet) this.announce(saved.moves, saved.rule, null, "");
      this.refreshNow();
      return saved;
    } catch (e) {
      app.note(errorText(e), "error", "sound-moves");
      return null;
    }
  }

  /** "+ New": a neutral sound to shape in Edit sound. */
  newSound(label = "New sound", extra: Partial<TasteRule> = {}): Promise<SavedSound | null> {
    return this.save(neutralRule(label, extra));
  }

  /** A new sound for the song / album playing (its first target), optionally only on this output. */
  newSoundFor(scope: TasteScope, onlyOn: string | null): Promise<SavedSound | null> {
    return this.newSound(scope.name || "New sound", { targets: [newTarget(scope)], ...(onlyOn ? { outputs: [onlyOn] } : {}) });
  }

  async remove(rule: TasteRule): Promise<boolean> {
    const ok = await app.confirm({ title: `Delete “${rule.label}”?`, text: "Its music goes back to the output's own sound.", confirmLabel: "Delete sound", danger: true });
    if (!ok) return false;
    try {
      await api.tasteRemove(rule.id);
      this.refreshNow();
      return true;
    } catch (e) {
      app.note(errorText(e), "error", "sound-moves");
      return false;
    }
  }

  // ------------------------------------------------------------ Try / Keep

  /** Plays `rule` on this song at once, nothing saved. */
  async tryNow(rule: TasteRule): Promise<boolean> {
    try {
      const ok = await api.tasteAudition(JSON.parse(JSON.stringify(rule)));
      if (!ok) {
        app.note(this.on ? TRY_REFUSED.idle : TRY_REFUSED.off, "info", "sound-try");
        return false;
      }
      this.trying = rule;
      this.refreshNow();
      return true;
    } catch (e) {
      app.note(errorText(e), "error", "sound-try");
      return false;
    }
  }

  async stopTrying() {
    this.trying = null;
    try {
      await api.tasteAudition(null);
    } catch (e) {
      app.note(errorText(e), "error", "sound-try");
    }
    this.refreshNow();
  }

  /** The scopes "Keep" offers for the sound being tried: the kind it was made for first, then the others (never a genre). */
  keepScopes(rule: TasteRule | null): { primary: TasteScope | null; others: TasteScope[] } {
    const scopes = (this.now?.scopes ?? []).filter((s) => s.kind !== "genre");
    const want = rule?.targets[0]?.scope.kind ?? "artist";
    const primary = scopes.find((s) => s.kind === want) ?? scopes.find((s) => s.kind === "artist") ?? scopes[0] ?? null;
    return { primary, others: scopes.filter((s) => s !== primary) };
  }

  async keep(rule: TasteRule, scope: TasteScope, onlyOn: string | null = null) {
    try {
      const saved = await api.tasteKeep(JSON.parse(JSON.stringify(rule)), scope, onlyOn);
      this.trying = null;
      this.announce(saved.moves, saved.rule, scope, `Saved “${saved.rule.label}” for ${scope.name || `this ${scope.kind}`}`);
      this.refreshNow();
    } catch (e) {
      app.note(errorText(e), "error", "sound-try");
    }
  }

  searchTargets(query: string): Promise<TargetHit[]> {
    return api.soundSearchTargets(query);
  }

  // ------------------------------------------------------------ the Ask-the-AI panel and the Sound ▸ menu

  /** Opens Sound with the AI panel ready ("a sound for Steely Dan"). */
  openAsk(seed: string | null = null) {
    if (app.view.name !== "sound") app.view = { name: "sound" };
    this.askSeed = seed;
    this.askOpen = true;
  }

  openMenu(e: MouseEvent, scope: TasteScope, label: string) {
    e.preventDefault();
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    this.menu = { scope, label, x: e.type === "contextmenu" ? e.clientX : r.left, y: e.type === "contextmenu" ? e.clientY : r.bottom + 6 };
  }
}

export const sound = new SoundStore();
