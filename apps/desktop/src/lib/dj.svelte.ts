// DJ mode's state on the screen: on / off, the mixer's last snapshot (20 a second from the
// `dj` event), what is on each deck, the crate, the optional keyboard shortcuts. Components only
// draw from here and send intents through `dj.send`.
import { coverUrl, errorText, type Hit, type Track } from "./api";
import { DECKS, defaultDjLayout, deckIndex, djApi, djSettingsOf, onDj, otherDeck, overviewBytes, type DeckId, type DeckSnapshot, type DjBinding, type DjControl, type DjEvent, type DjLayout, type DjOverview, type DjPick, type DjSectionKind, type DjSettings, type DjSnapshot, type DjSource, type DjState, type SfxKind } from "./dj-api";
import { moveSection, normalizeLayout, samePads, setPad, setSectionVisible } from "./dj-layout";
import { app } from "./state.svelte";

/** What the screen shows of a deck's song (the engine only knows its title). */
export interface DeckMeta {
  title: string;
  artist: string;
  /** Cover image URL. */
  cover: string | null;
  /** What was loaded, for "Try again". */
  source?: DjSource;
}

/** A song in the crate (the songs set aside for the set), or one being dragged to a deck. */
export interface CrateItem extends DeckMeta {
  key: string;
  source: DjSource;
  durationMs: number | null;
}

export interface LoadedOverview {
  meta: DjOverview;
  bytes: Uint8Array;
}

/** Positions are extrapolated from the last snapshot at most this far (ms): a stalled engine
 * must not look like it plays on. */
const MAX_EXTRAPOLATION_MS = 250;
/** Crossfader position past which deck B is the one the room hears. */
const CROSSFADER_CENTER = 0.5;
/** One key press on the crossfader moves it this much. */
const CROSSFADER_KEY_STEP = 0.1;
const PERCENT = 100;

export type DeckPhase = "empty" | "fetching" | "loading" | "error" | "ready";

const sourceKey = (s: DjSource) => (s.kind === "path" ? `path:${s.path}` : `tidal:${s.id}`);

class DjStore {
  on = $state(false);
  starting = $state(false);
  /** Why DJ mode could not start. */
  startError = $state<string | null>(null);
  /** The mixer's state; replaced whole 20 times a second (raw: one signal, not one per field). */
  snap = $state.raw<DjSnapshot | null>(null);
  /** When `snap` arrived (performance.now()), for the positions between snapshots. */
  private snapAt = 0;
  /** What each deck holds, as the screen names it (from the backend, so any window knows). */
  meta = $state.raw<[DeckMeta | null, DeckMeta | null]>([null, null]);
  /** A TIDAL song being fetched for the deck (before the engine starts decoding it). */
  fetching = $state<[boolean, boolean]>([false, false]);
  /** Big arrays the canvases read; never deep-proxied. */
  overview = $state.raw<[LoadedOverview | null, LoadedOverview | null]>([null, null]);
  crate = $state<CrateItem[]>([]);
  /** A song picked elsewhere while "Ask which deck" is the setting. */
  asking = $state.raw<DjPick | null>(null);
  keymap = $state.raw<DjBinding[]>([]);
  sheetOpen = $state(false);
  /** The crate next to the sampler (wide windows). */
  crateOpen = $state(true);
  /** The crate as a drawer over the screen (narrow windows). */
  drawerOpen = $state(false);
  /** The song being dragged onto a deck. */
  dragging = $state.raw<CrateItem | null>(null);
  /** Said by the screen reader (polite). */
  live = $state("");
  /** Mix length picked on the screen (starts at the setting). */
  mixBeats = $state<number | null>(null);
  /** Beats the zoomed waveforms show, changed with the wheel (starts at the setting). */
  zoomBeats = $state<number | null>(null);

  /** The "Edit layout" mode: the screen shows its editing panel and the sections can be arranged. */
  editing = $state(false);
  /** The pad sounds the engine rendered when DJ mode started (they are rendered once, at the start). */
  private renderedPads = $state.raw<SfxKind[] | null>(null);

  private started = false;
  private lastAutoMix: DjSnapshot["auto_mix"] = null;

  settings = $derived<DjSettings>(djSettingsOf(app.settings));
  /** The sections in screen order and the sound of each pad. */
  layout = $derived<DjLayout>(normalizeLayout(this.settings.layout));
  /** The pad sounds changed while DJ mode is on: they play after "Restart DJ mode". */
  restartNeeded = $derived(this.on && !!this.renderedPads && !samePads(this.layout.pads, this.renderedPads));
  /** The crate has a place on the screen (the layout may hide it). */
  crateVisible = $derived(this.layout.sections.some((s) => s.kind === "crate" && s.visible));

  async init() {
    if (this.started) return;
    this.started = true;
    onDj.state((s) => this.apply(s));
    onDj.load((e) => this.onLoad(e));
    onDj.picked((p) => {
      if (!p.deck) return;
      this.setMeta(p.deck, metaOfPick(p));
      app.note(`Loading “${p.title}” on deck ${p.deck.toUpperCase()}`, "info", "dj");
    });
    onDj.ask((p) => (this.asking = p));
    onDj.crate((p) => this.addToCrate(p));
    try {
      this.apply(await djApi.state());
    } catch {}
    this.refreshKeys();
  }

  private apply(s: DjState) {
    this.on = s.on;
    if (!s.on) {
      this.renderedPads = null;
      this.editing = false;
    } else if (!this.renderedPads) this.renderedPads = [...this.layout.pads];
    this.snap = s.snapshot;
    this.snapAt = performance.now();
    const meta = (s.decks ?? [null, null]).map((p) => (p ? metaOfPick(p) : null)) as [DeckMeta | null, DeckMeta | null];
    if (meta.some((m, i) => !sameMeta(m, this.meta[i]))) this.meta = meta;
    if (!s.snapshot) return;
    this.watchMix(s.snapshot);
    // a deck emptied by the engine (eject, a failed load) forgets what the screen knew of it
    for (const d of DECKS) {
      const i = deckIndex(d);
      const deck = s.snapshot.decks[i];
      if (!deck.track && !deck.loading && !this.fetching[i] && this.overview[i]) this.setOverview(i, null);
      // a song already on the deck (another window loaded it, the screen reloaded): its waveform
      if (deck.track && !this.overview[i] && !this.overviewAsked[i]) this.fetchOverview(d);
    }
  }

  private watchMix(s: DjSnapshot) {
    const before = this.lastAutoMix;
    this.lastAutoMix = s.auto_mix;
    if (!before && s.auto_mix) this.say(`Mix started, ${s.auto_mix.beats} beats`);
    if (before && !s.auto_mix) this.say("Mix finished");
  }

  private onLoad(e: DjEvent) {
    switch (e.type) {
      case "loading":
        this.setFetching(deckIndex(e.deck), false);
        break;
      case "loaded": {
        this.say(`Deck ${e.deck.toUpperCase()} loaded: ${e.track.title}`);
        this.fetchOverview(e.deck);
        if (!e.track.has_grid) app.note("Could not find the beat. You can still play it; Sync is off for this song.", "info", `dj-grid-${e.deck}`);
        break;
      }
      case "load_failed":
        this.setFetching(deckIndex(e.deck), false);
        app.note(e.message, "error", `dj-load-${e.deck}`);
        break;
      case "device_lost":
        app.note(`${e.message}.`, "error", "dj");
        break;
    }
  }

  /** An overview request in flight, per deck (one at a time). */
  private overviewAsked = [false, false];
  private async fetchOverview(deck: DeckId) {
    const i = deckIndex(deck);
    this.overviewAsked[i] = true;
    try {
      const o = await djApi.overview(deck).catch(() => null);
      this.setOverview(i, o ? { meta: o, bytes: overviewBytes(o) } : null);
    } finally {
      this.overviewAsked[i] = false;
    }
  }

  /** Shows the crate: next to the sampler, or as the drawer on a narrow window. */
  showCrate(drawer: boolean) {
    if (drawer) this.drawerOpen = true;
    else this.crateOpen = true;
  }

  private setOverview(i: number, o: LoadedOverview | null) {
    const next = [...this.overview] as [LoadedOverview | null, LoadedOverview | null];
    next[i] = o;
    this.overview = next;
  }

  private setFetching(i: number, on: boolean) {
    this.fetching[i] = on;
  }

  private setMeta(deck: DeckId, m: DeckMeta | null) {
    const next = [...this.meta] as [DeckMeta | null, DeckMeta | null];
    next[deckIndex(deck)] = m;
    this.meta = next;
  }

  private say(text: string) {
    this.live = text;
  }

  async refreshKeys() {
    try {
      this.keymap = (await djApi.keymap()).bindings;
    } catch {}
  }

  // ------------------------------------------------------------ on / off

  async start() {
    if (this.starting || this.on) return;
    this.starting = true;
    this.startError = null;
    try {
      this.apply(await djApi.enable(true));
    } catch (e) {
      this.startError = errorText(e);
    } finally {
      this.starting = false;
    }
  }

  async stop() {
    try {
      this.apply(await djApi.enable(false));
    } catch (e) {
      app.note(errorText(e), "error", "dj");
    }
    this.meta = [null, null];
    this.overview = [null, null];
    this.fetching = [false, false];
  }

  // ------------------------------------------------------------ intents

  send(control: DjControl) {
    if (!this.on) return;
    djApi.control(control).catch(() => {});
  }

  deck(d: DeckId): DeckSnapshot | null {
    return this.snap?.decks[deckIndex(d)] ?? null;
  }

  phase(d: DeckId): DeckPhase {
    const i = deckIndex(d);
    const s = this.deck(d);
    if (this.fetching[i]) return "fetching";
    if (s?.loading) return "loading";
    if (s?.track) return "ready";
    if (s?.error) return "error";
    return "empty";
  }

  /** Where the playhead is now (ms), between snapshots too. Plain number: canvases read it every frame. */
  positionMs(d: DeckId): number {
    const s = this.deck(d);
    if (!s?.track) return 0;
    if (!s.playing) return s.position_ms;
    const elapsed = Math.min(MAX_EXTRAPOLATION_MS, performance.now() - this.snapAt);
    const speed = (s.track.bpm > 0 && s.current_bpm > 0 ? s.current_bpm / s.track.bpm : 1 + s.tempo_percent / PERCENT) + (s.bend * this.settings.bend_pct) / PERCENT;
    let ms = s.position_ms + elapsed * speed;
    if (s.loop_ms && s.loop_ms[1] > s.loop_ms[0] && ms >= s.loop_ms[1]) ms = s.loop_ms[0] + ((ms - s.loop_ms[0]) % (s.loop_ms[1] - s.loop_ms[0]));
    return Math.min(ms, s.track.duration_ms);
  }

  /** "Try again" on a deck whose song could not be opened. */
  retry(deck: DeckId) {
    const m = this.meta[deckIndex(deck)];
    if (m?.source) this.loadSource(deck, m.source, m);
  }

  /** A crate row as something to drag or load. */
  itemOfTrack(t: Track): CrateItem {
    return { key: `path:${t.path}`, source: { kind: "path", path: t.path, title: t.title }, title: t.title, artist: t.artist, cover: coverUrl(t.path), durationMs: t.durationMs };
  }

  itemOfHit(h: Hit): CrateItem {
    const local = (h.isrc && app.localIsrc.get(h.isrc)?.path) || null;
    return { key: `tidal:${h.id}`, source: { kind: "tidal", id: h.id, title: h.title, local }, title: h.title, artist: h.artist ?? h.subtitle, cover: h.cover_url, durationMs: h.duration_ms };
  }

  loadCrate(deck: DeckId, item: CrateItem) {
    this.loadSource(deck, item.source, item, item.durationMs);
  }

  async loadSource(deck: DeckId, source: DjSource, meta: DeckMeta, durationMs: number | null = null) {
    const i = deckIndex(deck);
    this.setMeta(deck, { title: meta.title, artist: meta.artist, cover: meta.cover, source });
    this.setOverview(i, null);
    if (source.kind === "tidal" && !source.local) this.setFetching(i, true);
    try {
      await djApi.load(deck, { source, title: meta.title, artist: meta.artist, cover_url: meta.cover, duration_ms: durationMs, deck });
    } catch (e) {
      this.setFetching(i, false);
      app.note(errorText(e), "error", `dj-load-${deck}`);
    }
  }

  eject(deck: DeckId) {
    this.send({ type: "eject", deck });
    this.setMeta(deck, null);
    this.setOverview(deckIndex(deck), null);
  }

  /** A song picked elsewhere ("Ask which deck") goes to `deck`, or to the crate. */
  answerAsk(deck: DeckId | "crate" | null) {
    const p = this.asking;
    this.asking = null;
    if (!p || !deck) return;
    if (deck === "crate") this.addToCrate(p);
    else this.loadSource(deck, p.source, metaOfPick(p), p.duration_ms);
  }

  addToCrate(p: DjPick) {
    const key = sourceKey(p.source);
    if (this.crate.some((c) => c.key === key)) return;
    this.crate.push({ key, source: p.source, durationMs: p.duration_ms, ...metaOfPick(p) });
    app.note(`“${p.title}” is in the crate`, "success", "dj");
  }

  removeFromCrate(key: string) {
    this.crate = this.crate.filter((c) => c.key !== key);
  }

  // ------------------------------------------------------------ the pair

  /** Sync couples the decks: either one following the other means both show it. */
  get linked(): boolean {
    const s = this.snap;
    return !!s && (s.decks[0].synced || s.decks[1].synced);
  }

  toggleSync(d: DeckId) {
    if (this.linked) {
      this.send({ type: "sync_off", deck: "a" });
      this.send({ type: "sync_off", deck: "b" });
      this.say("Sync off");
    } else {
      this.send({ type: "sync", deck: d });
      this.say(`Synced to deck ${otherDeck(d).toUpperCase()}`);
    }
  }

  /** The deck the room hears less: the one "Mix" brings in. */
  get mixTarget(): DeckId {
    return (this.snap?.crossfader ?? 0) < CROSSFADER_CENTER ? "b" : "a";
  }

  get canMix(): boolean {
    return this.phase("a") === "ready" && this.phase("b") === "ready";
  }

  get zoom(): number {
    return this.zoomBeats ?? this.settings.zoom_beats;
  }

  get beats(): number {
    return this.mixBeats ?? this.settings.auto_mix_beats;
  }

  mix() {
    if (this.snap?.auto_mix) return this.send({ type: "auto_mix_cancel" });
    if (!this.canMix) return app.note("Load a song on both decks to mix.", "info", "dj");
    this.send({ type: "auto_mix", to: this.mixTarget, beats: this.beats });
  }

  /** Loops are toggles: the same length again leaves the loop. */
  loop(d: DeckId, beats: number) {
    const s = this.deck(d);
    if (s?.loop_ms && s.track && Math.abs((s.loop_ms[1] - s.loop_ms[0]) / s.track.beat_ms - beats) < 0.01) this.send({ type: "loop_exit", deck: d });
    else this.send({ type: "loop_beats", deck: d, beats });
  }

  async saveSettings(patch: Partial<DjSettings>) {
    await app.saveSettings({ dj: { ...this.settings, ...patch } });
    if ("key_overrides" in patch || "keys_enabled" in patch) this.refreshKeys();
  }

  // ------------------------------------------------------------ the layout

  private saveLayout(layout: DjLayout) {
    return this.saveSettings({ layout }).catch((e) => app.note(errorText(e), "error", "dj-layout"));
  }
  moveSection = (from: number, to: number) => this.saveLayout(moveSection(this.layout, from, to));
  setSectionVisible = (kind: DjSectionKind, visible: boolean) => this.saveLayout(setSectionVisible(this.layout, kind, visible));
  setPadSound = (pad: number, sound: SfxKind) => this.saveLayout(setPad(this.layout, pad, sound));
  /** Every section back in its place and visible, every pad on its own sound. */
  resetLayout = () => this.saveLayout(defaultDjLayout());
  /** The pad sounds are rendered when DJ mode starts: turn it off and on again (the decks are emptied). */
  async restart() {
    const editing = this.editing;
    await this.stop();
    await this.start();
    this.editing = editing && this.on;
  }

  // ------------------------------------------------------------ keyboard (optional extras)

  /** The key bound to `action`, as the screen prints it ("Q", "Shift+1"). */
  keyFor(action: string): string | null {
    const key = this.keymap.find((b) => b.action === action)?.key;
    return key ? printKey(key) : null;
  }

  /** Window keydown / keyup while the DJ screen shows. `true` when the key was used. */
  onKey(e: KeyboardEvent, down: boolean): boolean {
    if (!this.on || !this.settings.keys_enabled) return false;
    const target = e.target as HTMLElement | null;
    if (target?.closest("input, textarea, select, [contenteditable], [role=dialog]")) return false;
    if (down && e.key === "?") {
      this.sheetOpen = !this.sheetOpen;
      return true;
    }
    const key = keyOf(e);
    const action = this.keymap.find((b) => b.key === key)?.action;
    if (!action) return false;
    if (down && e.repeat) return true;
    this.run(action, down);
    return true;
  }

  private run(action: string, down: boolean) {
    const [scope, name] = action.split(".");
    if (scope === "deck_a" || scope === "deck_b") {
      const d: DeckId = scope === "deck_a" ? "a" : "b";
      if (name === "cue") return this.send({ type: "cue", deck: d, pressed: down });
      if (name === "bend_back" || name === "bend_forward") return this.send({ type: "bend", deck: d, amount: down ? (name === "bend_back" ? -1 : 1) : 0 });
      if (!down) return;
      if (name === "play") return this.send({ type: "toggle_play", deck: d });
      if (name === "sync") return this.toggleSync(d);
      if (name === "loop_exit") return this.send({ type: "loop_exit", deck: d });
      const loop = /^loop_(\d+)$/.exec(name);
      if (loop) return this.loop(d, Number(loop[1]));
      const cue = /^hot_cue_(\d+)$/.exec(name);
      if (cue) return this.send({ type: "hot_cue", deck: d, index: Number(cue[1]) - 1 });
      return;
    }
    if (!down) return;
    if (scope === "sampler") return this.send({ type: "sampler", pad: Number(name) - 1 });
    const xf = this.snap?.crossfader ?? CROSSFADER_CENTER;
    if (action === "crossfader.left") return this.send({ type: "crossfader", value: Math.max(0, xf - CROSSFADER_KEY_STEP) });
    if (action === "crossfader.right") return this.send({ type: "crossfader", value: Math.min(1, xf + CROSSFADER_KEY_STEP) });
    if (action === "crossfader.center") return this.send({ type: "crossfader", value: CROSSFADER_CENTER });
    if (action === "auto_mix.to_a" || action === "auto_mix.to_b") {
      if (this.snap?.auto_mix) return this.send({ type: "auto_mix_cancel" });
      if (this.canMix) this.send({ type: "auto_mix", to: action.endsWith("_a") ? "a" : "b", beats: this.beats });
    }
  }
}

function metaOfPick(p: DjPick): DeckMeta {
  const cover = p.cover_url ?? (p.source.kind === "path" ? coverUrl(p.source.path) : p.source.local ? coverUrl(p.source.local) : null);
  return { title: p.title, artist: p.artist, cover, source: p.source };
}

const sameMeta = (a: DeckMeta | null, b: DeckMeta | null) =>
  a === b || (!!a && !!b && a.title === b.title && a.artist === b.artist && a.cover === b.cover && (a.source && sourceKey(a.source)) === (b.source && sourceKey(b.source)));

/** Physical keys whose `KeyboardEvent.key` changes with Shift / Alt, by `code`. */
const CODE_KEYS: Record<string, string> = { BracketLeft: "[", BracketRight: "]", Backslash: "\\", Semicolon: ";", Quote: "'", Comma: ",", Period: ".", Slash: "/", Minus: "-", Equal: "=", Backquote: "`" };

/** The key of an event written like homefi_core::dj::keymap (`Alt+Ctrl+Shift+x`). */
export function keyOf(e: KeyboardEvent): string {
  let base: string;
  if (/^Key[A-Z]$/.test(e.code)) base = e.code.slice(3).toLowerCase();
  else if (/^Digit\d$/.test(e.code)) base = e.code.slice(5);
  else base = CODE_KEYS[e.code] ?? (e.key.length === 1 ? e.key.toLowerCase() : e.key);
  return `${e.altKey ? "Alt+" : ""}${e.ctrlKey || e.metaKey ? "Ctrl+" : ""}${e.shiftKey ? "Shift+" : ""}${base}`;
}

/** "Alt+Shift+q" → "Alt+Shift+Q". */
export function printKey(key: string): string {
  const m = /^((?:(?:Alt|Ctrl|Shift)\+)*)(.+)$/.exec(key);
  if (!m) return key;
  return m[1] + (m[2].length === 1 ? m[2].toUpperCase() : m[2]);
}

const DECK_NAMES: Record<string, string> = {
  play: "Play / pause",
  cue: "Cue (hold to listen)",
  sync: "Sync",
  loop_1: "Loop 1 beat",
  loop_2: "Loop 2 beats",
  loop_4: "Loop 4 beats",
  loop_8: "Loop 8 beats",
  loop_exit: "Leave the loop",
  bend_back: "Slow down (hold)",
  bend_forward: "Speed up (hold)",
};
const OTHER_NAMES: Record<string, string> = {
  "crossfader.left": "Crossfader to the left",
  "crossfader.right": "Crossfader to the right",
  "crossfader.center": "Crossfader to the centre",
  "auto_mix.to_a": "Mix to A",
  "auto_mix.to_b": "Mix to B",
};

/** A shortcut's action in words ("Hot cue 3", "Mix to B"). */
export function actionLabel(action: string): string {
  const [scope, what] = action.split(".");
  if (scope === "sampler") return `Pad ${what}`;
  const cue = /^hot_cue_(\d+)$/.exec(what ?? "");
  if (cue) return `Hot cue ${cue[1]}`;
  return DECK_NAMES[what] ?? OTHER_NAMES[action] ?? action;
}

export const dj = new DjStore();
