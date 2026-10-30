// Development-only taste layers / loudness / reactions for mock.ts: the same
// rules as homefi-core (most specific wins, ±2 dB per tap capped at ±6, undo).
// ?taste starts with a few rules; ?loudness with normalization on (Auto);
// ?nosuggest makes the suggestion fail quietly. "Ask the AI to refine" a rule
// answers with a 4-band curve on the table; "yes" saves it.
import type { AssistantTurn, LoudnessSettings, NowPlayingContext, NowSound, ReactionResult, SoundReaction, SoundUiSettings, TasteBand, TasteChange, TasteIntensity, TasteRule, TasteScope, TasteSettings, Tilt, Track } from "./api";

const url = new URLSearchParams(location.search);
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
const copy = <T>(v: T): T => JSON.parse(JSON.stringify(v));

const rule = (id: string, scope: TasteScope, tilt: Tilt, label: string, createdBy: TasteRule["createdBy"], enabled = true): TasteRule => ({ id, scope, tilt, label, createdBy, enabled });
const band = (kind: TasteBand["kind"], freqHz: number, gainDb: number, q: number, note = ""): TasteBand => ({ kind, freqHz, gainDb, q, enabled: true, note });

let taste: TasteSettings = url.has("taste")
  ? {
      enabled: true,
      intensity: "balanced",
      rules: [
        rule("r1", { kind: "genre", name: "Electronic" }, { bassDb: 4, trebleDb: 0, clarityDb: null }, "More punch · Electronic", "reaction"),
        rule("r2", { kind: "album", id: "", name: "Kind of Blue" }, { bassDb: 0, trebleDb: 1, clarityDb: 1.5 }, "A touch more detail · Kind of Blue", "manual"),
        rule("r3", { kind: "playlist", id: "1", name: "Every song, ever" }, { bassDb: 2, trebleDb: -1.5, clarityDb: null }, "Warmer · Every song, ever", "ai"),
        rule("r4", { kind: "artist", id: "", name: "Radiohead" }, { bassDb: 0, trebleDb: -2, clarityDb: 1 }, "Softer highs · Radiohead", "guided", false),
        {
          ...rule("r5", { kind: "artist", id: "", name: "David Guetta" }, { bassDb: 2, trebleDb: 0, clarityDb: null }, "More punch for David Guetta", "ai"),
          bands: [band("peak", 3200, -2, 3, "Softens the harsh edge of the cymbals"), band("peak", 65, 1.5, 1.2, "More kick from the drum"), band("peak", 320, -1.5, 1.4, "Clears the mud so the kick stays tight")],
          explanation: "A tighter, punchier low end and less bite on the cymbals at high volume.",
        },
        {
          ...rule("r6", { kind: "album", id: "", name: "Random Access Memories" }, { bassDb: 0, trebleDb: 0, clarityDb: null }, "Less harsh · Random Access Memories", "manual"),
          bands: [band("peak", 7000, -2.5, 4), band("highShelf", 11000, 1, 0.7)],
        },
      ],
    }
  : { enabled: false, rules: [], intensity: "balanced" };
let loudness: LoudnessSettings = { mode: url.has("loudness") ? "auto" : "off", targetLufs: -14, preventClipping: true };
let soundUi: SoundUiSettings = { react_deck: true, react_room: true, layer_chip: true, suggestions: !url.has("nosuggest") };
/** What undoes a reaction: the opposite taste change (merged on top of the rules as they are by then) and the loudness before. */
const undo = new Map<string, { inverse: TasteChange; loudness: LoudnessSettings }>();
let suggesting: "Electronic" | "Jazz" | null = null;
/** The rule a refine conversation works on, and the curve proposed for it. */
let refining: { id: string; proposal: TasteRule | null } | null = null;

const KIND_RANK: Record<string, number> = { track: 5, album: 4, playlist: 3, artist: 2, genre: 1 };
const same = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase() && a.trim() !== "";
const sameScope = (a: TasteScope, b: TasteScope) => a.kind === b.kind && (a.kind !== "genre" && b.kind !== "genre" && a.id && b.id ? a.id === b.id : same(a.name, b.name));

const noChange = (): TasteChange => ({ upsert: [], remove: [], enabled: null });

/** homefi-core's `TasteChange::apply`: upsert by id or scope, remove by id, never a whole replacement. */
function merge(current: TasteSettings, change: TasteChange): TasteSettings {
  let rules = current.rules.filter((r) => !change.remove.includes(r.id));
  for (const u of change.upsert) {
    const at = rules.findIndex((r) => r.id === u.id || sameScope(r.scope, u.scope));
    rules = at >= 0 ? rules.map((r, i) => (i === at ? u : r)) : [...rules, u];
  }
  return { ...current, enabled: change.enabled ?? (change.upsert.length ? true : current.enabled), rules };
}

/** Applies `change` to the live taste and returns what undoes it. */
function applyChange(change: TasteChange): TasteChange {
  const inverse: TasteChange = { upsert: [], remove: [], enabled: change.enabled === null ? null : taste.enabled };
  for (const u of change.upsert) {
    const prior = taste.rules.find((r) => r.id === u.id || sameScope(r.scope, u.scope));
    if (prior) inverse.upsert.push(copy(prior));
    else inverse.remove.push(u.id);
  }
  for (const id of change.remove) {
    const prior = taste.rules.find((r) => r.id === id);
    if (prior) inverse.upsert.push(copy(prior));
  }
  taste = merge(taste, change);
  return inverse;
}

export function contextOf(t: Track | null): NowPlayingContext | null {
  if (!t) return null;
  return {
    trackId: null,
    path: t.path,
    title: t.title,
    album: t.album ? { id: "", name: t.album } : null,
    artists: t.artist ? [{ id: "", name: t.artist }] : [],
    genres: t.genre.split(",").map((g) => g.trim()).filter(Boolean),
    source: null,
  };
}

function scopes(c: NowPlayingContext): TasteScope[] {
  const out: TasteScope[] = [];
  if (c.path) out.push({ kind: "track", id: c.path, name: c.title ?? "" });
  if (c.album) out.push({ kind: "album", id: c.album.id, name: c.album.name });
  if (c.artists[0]) out.push({ kind: "artist", id: c.artists[0].id, name: c.artists[0].name });
  if (c.genres[0]) out.push({ kind: "genre", name: c.genres[0] });
  return out;
}

function resolve(c: NowPlayingContext | null): TasteRule | null {
  if (!c || !taste.enabled) return null;
  let best: [number, number, TasteRule] | null = null;
  for (const r of taste.rules) {
    if (!r.enabled) continue;
    const s = r.scope;
    let pos = -1;
    if (s.kind === "genre") pos = c.genres.findIndex((g) => same(g, s.name));
    else if (s.kind === "artist") pos = c.artists.findIndex((a) => same(a.name, s.name));
    else if (s.kind === "album") pos = c.album && same(c.album.name, s.name) ? 0 : -1;
    else if (s.kind === "track") pos = c.path === s.id || (!s.id && same(c.title ?? "", s.name)) ? 0 : -1;
    if (pos < 0) continue;
    const score: [number, number, TasteRule] = [KIND_RANK[s.kind], -pos, r];
    if (!best || score[0] > best[0] || (score[0] === best[0] && score[1] >= best[1])) best = score;
  }
  return best?.[2] ?? null;
}

const words = (t: Tilt) =>
  [
    [t.bassDb, "bass"],
    [t.trebleDb, "treble"],
    [t.clarityDb ?? 0, "clarity"],
  ]
    .filter(([db]) => Math.abs(db as number) >= 0.05)
    .map(([db, w]) => `${(db as number) > 0 ? "+" : ""}${db} dB ${w}`)
    .join(", ");

/** dB of one tap for each change size (homefi-core: subtle 1.5–3, balanced 3–5, bold 5–8). */
const STEP_DB: Record<TasteIntensity, number> = { subtle: 1.5, balanced: 3, bold: 5 };
const SIGNS: Record<SoundReaction, Tilt> = {
  needsBass: { bassDb: 1, trebleDb: 0 },
  tooBoomy: { bassDb: -1, trebleDb: 0 },
  tooBright: { bassDb: 0, trebleDb: -1 },
  dull: { bassDb: 0, trebleDb: 1 },
  vocalsBuried: { bassDb: 0, trebleDb: 0, clarityDb: 1 },
  tooQuiet: { bassDb: 0, trebleDb: 0 },
  tooLoud: { bassDb: 0, trebleDb: 0 },
  perfect: { bassDb: 0, trebleDb: 0 },
};
const LABELS: Record<string, string> = { needsBass: "More bass", tooBoomy: "Less bass", tooBright: "Softer highs", dull: "Brighter", vocalsBuried: "Clearer vocals" };
const TILT_CAP_DB = 8;
const cap = (v: number) => Math.max(-TILT_CAP_DB, Math.min(TILT_CAP_DB, v));

function react(reaction: SoundReaction, c: NowPlayingContext | null, hint: TasteScope | null): ReactionResult {
  const out: ReactionResult = { rule: null, taste: copy(taste), tasteChange: noChange(), loudness: { ...loudness }, message: "", undoToken: "", byAi: false };
  if (reaction === "perfect") {
    const r = resolve(c);
    return { ...out, message: r ? `Great, it stays as it is (“${r.label}”).` : "Great, nothing to change." };
  }
  const token = `u${Date.now()}`;
  const loudnessBefore = { ...loudness };
  undo.set(token, { inverse: noChange(), loudness: loudnessBefore });
  if (reaction === "tooQuiet" || reaction === "tooLoud") {
    if (loudness.mode === "off") {
      loudness = { ...loudness, mode: "auto" };
      return { ...out, loudness, undoToken: token, message: "Turned on loudness normalization: songs play at an even level (no longer bit-perfect)." };
    }
    loudness = { ...loudness, targetLufs: Math.max(-24, Math.min(-8, loudness.targetLufs + (reaction === "tooQuiet" ? 2 : -2))) };
    return { ...out, loudness, undoToken: token, message: `Normalization ${reaction === "tooQuiet" ? "raised" : "lowered"} to ${loudness.targetLufs} LUFS.` };
  }
  const all = c ? scopes(c) : [];
  const scope = hint ?? all.find((s) => s.kind === "genre") ?? all.find((s) => s.kind === "artist") ?? all[0];
  if (!scope) return { ...out, message: "Nothing is playing; press play first." };
  const existing = taste.rules.find((r) => sameScope(r.scope, scope));
  const base = existing?.tilt ?? resolve(c)?.tilt ?? { bassDb: 0, trebleDb: 0, clarityDb: null };
  const sign = SIGNS[reaction];
  const db = STEP_DB[taste.intensity];
  const step: Tilt = { bassDb: sign.bassDb * db, trebleDb: sign.trebleDb * db, clarityDb: (sign.clarityDb ?? 0) * db };
  const tilt: Tilt = {
    bassDb: cap(base.bassDb + step.bassDb),
    trebleDb: cap(base.trebleDb + step.trebleDb),
    clarityDb: base.clarityDb == null && !step.clarityDb ? null : cap((base.clarityDb ?? 0) + (step.clarityDb ?? 0)),
  };
  const r: TasteRule = existing
    ? { ...existing, tilt, label: `${LABELS[reaction]} · ${scope.name}`, enabled: true }
    : rule(`r${Date.now()}`, scope, tilt, `${LABELS[reaction]} · ${scope.name}`, url.has("aikey") ? "ai" : "reaction");
  const tasteChange: TasteChange = { upsert: [r], remove: [], enabled: true };
  undo.set(token, { inverse: applyChange(tasteChange), loudness: loudnessBefore });
  const change: Tilt = { bassDb: tilt.bassDb - base.bassDb, trebleDb: tilt.trebleDb - base.trebleDb, clarityDb: (tilt.clarityDb ?? 0) - (base.clarityDb ?? 0) };
  const said = words(change);
  return {
    ...out,
    rule: r,
    taste: copy(taste),
    tasteChange,
    undoToken: token,
    byAi: url.has("aikey"),
    message: said ? (url.has("aikey") ? `Gave ${scope.name} a bit more of what was missing: ${said}.` : `${said[0].toUpperCase()}${said.slice(1)} for ${scope.name}.`) : `That's already the safe limit for ${scope.name}.`,
  };
}

export const soundMock = {
  settings: () => ({ taste, loudness, sound_ui: soundUi }),
  setSettings(s: { taste?: TasteSettings; loudness?: LoudnessSettings; sound_ui?: SoundUiSettings }) {
    if (s.taste) taste = { ...s.taste };
    if (s.loudness) loudness = s.loudness;
    if (s.sound_ui) soundUi = s.sound_ui;
  },
  /** `eq_active` and the layer fields of the player for `track`, on top of the equalizer's own. */
  playerFlags(track: Track | null, eq: { eq_active: boolean; eq_profile: string | null }) {
    const r = resolve(contextOf(track));
    const loud = loudness.mode === "off" ? 0 : track?.album === "Kind of Blue" ? 3.4 : -4.2 + (loudness.targetLufs + 14);
    return { ...eq, eq_active: eq.eq_active || !!r, taste_rule: r?.id ?? null, taste_label: r?.label ?? null, loudness_db: Math.round(loud * 10) / 10 };
  },
  /** "Ask the AI to refine": what the curve does now and a 4-band proposal. */
  refine(id: string, es: boolean): AssistantTurn | null {
    const r = taste.rules.find((x) => x.id === id);
    if (!r) return null;
    const say = (en: string, s: string) => (es ? s : en);
    const proposal: TasteRule = {
      ...copy(r),
      createdBy: "ai",
      bands: [
        band("peak", 3200, -2.5, 3, say("−2.5 dB at 3.2 kHz, narrow: softens the harsh edge of the cymbals", "−2,5 dB en 3,2 kHz, estrecho: suaviza el filo de los platillos")),
        band("peak", 7200, -2, 4.5, say("−2 dB at 7.2 kHz, very narrow: tames the hissing s's", "−2 dB en 7,2 kHz, muy estrecho: doma las eses que silban")),
        band("peak", 65, 2, 1.2, say("+2 dB at 65 Hz: more kick from the drum", "+2 dB en 65 Hz: más golpe del bombo")),
        band("peak", 320, -1.5, 1.4, say("−1.5 dB at 320 Hz: clears the mud so the kick stays tight", "−1,5 dB en 320 Hz: quita lo embarrado para que el bombo quede firme")),
      ],
      tilt: { bassDb: 0, trebleDb: 0, clarityDb: null },
      explanation: say("Narrow cuts where the harshness lives, a bit more kick and less mud.", "Cortes estrechos donde vive la aspereza, un poco más de bombo y menos barro."),
    };
    refining = { id, proposal };
    return {
      text: say(
        `Right now “${r.label}” lifts the bass and softens 3.2 kHz a little. You said the cymbals still bite: here's a cleaner curve with 4 bands. Apply it?`,
        `Ahora «${r.label}» sube el bajo y suaviza un poco los 3,2 kHz. Dijiste que los platillos aún pican: te propongo una curva más limpia con 4 bandas. ¿La aplico?`,
      ),
      quick_replies: [say("Less bass", "Menos bajo"), say("Keep it subtler", "Más sutil")],
      proposal: null,
      confirmed: false,
      done: false,
      taste_proposal: proposal,
    };
  },
  /** The mock assistant: an answer to the suggestion card or a refine (null when it isn't one). */
  reply(text: string): AssistantTurn | null {
    if (refining?.proposal) {
      const p = refining.proposal;
      refining = null;
      const base: AssistantTurn = { text: "", quick_replies: [], proposal: null, confirmed: false, done: true };
      if (!/^(s[ií]|yes|ok|dale|claro|apply|aplic)/i.test(text.trim())) return { ...base, text: "OK, nothing changed." };
      const taste_change: TasteChange = { upsert: [p], remove: [], enabled: null };
      applyChange(taste_change);
      return { ...base, text: `Done: “${p.label}” now has ${p.bands?.length ?? 0} bands.`, taste_change };
    }
    const genre = suggesting;
    if (!genre) return null;
    suggesting = null;
    const base: AssistantTurn = { text: "", quick_replies: [], proposal: null, confirmed: false, done: true };
    if (!/^(s[ií]|yes|ok|dale|claro)/i.test(text.trim())) return { ...base, text: "OK, nothing changed." };
    const tilt: Tilt = genre === "Jazz" ? { bassDb: 0, trebleDb: 1, clarityDb: 1.5 } : { bassDb: 3, trebleDb: 0, clarityDb: null };
    const label = genre === "Jazz" ? "A touch more detail · Jazz" : "A bit more bass · Electronic";
    const r = rule(`r${Date.now()}`, { kind: "genre", name: genre }, tilt, label, url.has("aikey") ? "ai" : "guided");
    const taste_change: TasteChange = { upsert: [r], remove: [], enabled: true };
    applyChange(taste_change);
    return { ...base, text: `Done: “${label}”. You'll find it in Taste rules, to change or remove it.`, taste_change };
  },
  handlers: (current: () => Track | null, changed: () => void) => ({
    taste_get: () => taste,
    taste_set: ({ taste: t }: { taste: TasteSettings }) => ((taste = t), changed(), taste),
    loudness_get: () => loudness,
    loudness_set: ({ loudness: l }: { loudness: LoudnessSettings }) => ((loudness = l), changed(), loudness),
    sound_now: (): NowSound => {
      const c = contextOf(current());
      return { context: c, scopes: c ? scopes(c) : [], rule: resolve(c) };
    },
    sound_react: async ({ reaction, scope }: { reaction: SoundReaction; scope: TasteScope | null }) => {
      await wait(url.has("aikey") ? 900 : 250);
      const r = react(reaction, contextOf(current()), scope);
      changed();
      return r;
    },
    sound_undo: ({ token }: { token: string }) => {
      const before = undo.get(token);
      if (!before) return false;
      undo.delete(token);
      taste = merge(taste, before.inverse);
      loudness = before.loudness;
      changed();
      return true;
    },
    taste_suggest: async () => {
      await wait(600);
      if (url.has("nosuggest")) throw "no suggestion";
      if (taste.rules.some((r) => r.scope.kind === "genre" && same(r.scope.name, "Electronic"))) {
        suggesting = "Jazz";
        return { text: "Jazz is your second most played genre. Want a touch more detail when it plays (+1 dB treble, +1.5 dB clarity)?", quick_replies: ["Yes, try it", "No, thanks"], proposal: null, confirmed: false, done: false };
      }
      suggesting = "Electronic";
      return { text: "You listen to a lot of Electronic. Want a bit more bass when it plays (+3 dB bass)? Only that music changes.", quick_replies: ["Yes, try it", "No, thanks"], proposal: null, confirmed: false, done: false };
    },
    listening_summary: () => ({ top_genres: ["Electronic", "Jazz", "Alternative"], top_artists: [{ id: "", name: "Daft Punk" }, { id: "", name: "Miles Davis" }], playlists: [{ id: "1", name: "Every song, ever" }] }),
  }),
};
