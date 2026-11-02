// Development-only sounds / loudness / reactions for mock.ts: the same rules as
// homefi-core (most specific wins: song > album > artist > playlist > genre, a
// sound can be limited to some outputs, a target belongs to one sound per
// overlapping outputs, moves can be undone). The master switch is the
// equalizer's `enabled`.
// ?taste starts with the mockup's sounds playing "Hotel California" on the
// Sony WH-1000XM5; ?audition a Try on this song; ?soundoff the master off;
// ?nomusic nothing playing; ?loudness normalization on; ?nosuggest makes the
// suggestion fail quietly. "Ask the AI" answers with a proposal (Try / Keep).
import type {
  AssistantTurn,
  LoudnessSettings,
  NowPlayingContext,
  NowSound,
  ReactionResult,
  SavedSound,
  SoundReaction,
  SoundUiSettings,
  TargetHit,
  TargetMove,
  TasteBand,
  TasteChange,
  TasteCreator,
  TasteIntensity,
  TasteRule,
  TasteScope,
  TasteSettings,
  Tilt,
  Track,
} from "./api";

const url = new URLSearchParams(location.search);
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
const copy = <T>(v: T): T => JSON.parse(JSON.stringify(v));

const DAY_MS = 86_400_000;
const NOW_MS = Date.UTC(2026, 10, 2);
const ago = (days: number) => NOW_MS - days * DAY_MS;

const band = (kind: TasteBand["kind"], freqHz: number, gainDb: number, q: number, note = ""): TasteBand => ({ kind, freqHz, gainDb, q, enabled: true, note });
const artist = (name: string): TasteScope => ({ kind: "artist", id: "", name });
const album = (name: string): TasteScope => ({ kind: "album", id: "", name });
const song = (name: string): TasteScope => ({ kind: "track", id: "", name });
const playlist = (id: string, name: string): TasteScope => ({ kind: "playlist", id, name });
const genre = (name: string): TasteScope => ({ kind: "genre", name });
const target = (scope: TasteScope, addedMs: number, addedBy: TasteCreator = "ai") => ({ scope, addedMs, addedBy });

const sound = (id: string, label: string, createdBy: TasteCreator, tilt: Tilt, targets: TasteRule["targets"], extra: Partial<TasteRule> = {}): TasteRule => ({
  id,
  label,
  tilt,
  bands: [],
  createdBy,
  enabled: true,
  targets,
  ...extra,
});

const SONY = "Sony WH-1000XM5";

let taste: TasteSettings = url.has("taste")
  ? {
      enabled: true,
      intensity: "balanced",
      version: 1,
      rules: [
        sound("bad-omens", "Bad Omens", "ai", { bassDb: 4, trebleDb: 1.5, clarityDb: 1 }, [target(artist("Bad Omens"), ago(4)), target(artist("Architects"), ago(4)), target(artist("Spiritbox"), ago(1))], { explanation: "Bold bass, crisp vocals." }),
        sound("warm", "Warm and airy", "ai", { bassDb: 0, trebleDb: 0, clarityDb: null }, [
          target(artist("Eagles"), ago(5)),
          target(artist("Fleetwood Mac"), ago(5)),
          target(artist("Steely Dan"), ago(2)),
          target(song("Hotel California (Live)"), ago(1), "manual"),
          target(playlist("classic-rock", "Classic Rock Drive"), 0),
        ], {
          outputs: [SONY],
          bands: [band("lowShelf", 120, 2, 0.7, "+2 dB at 120 Hz: lifts the low warmth"), band("highShelf", 9000, 1.5, 0.7, "+1.5 dB above 9 kHz: opens the top end"), band("peak", 3000, -1.2, 2, "−1.2 dB at 3 kHz: trims the glare")],
          explanation: "Lifts the low warmth +2 dB, opens the top end +1.5 dB, trims the 3 kHz glare.",
        }),
        sound("lasgo", "Lasgo", "ai", { bassDb: 3, trebleDb: 1, clarityDb: null }, [target(artist("Lasgo"), ago(9)), target(artist("Alice DJ"), ago(9))], { explanation: "Tight low end, sparkle." }),
        sound("sting", "Sting", "manual", { bassDb: 0.5, trebleDb: -1.5, clarityDb: 1 }, [target(artist("Sting"), ago(12), "manual"), target(artist("The Police"), ago(12), "manual"), target(album("Synchronicity"), ago(11), "manual"), target(song("Roxanne"), ago(3), "manual")], { explanation: "Soft, intimate." }),
        sound("soad", "System Of A Down", "ai", { bassDb: 1, trebleDb: -1, clarityDb: 2 }, [target(artist("System Of A Down"), ago(20)), target(artist("Korn"), ago(8))], { explanation: "Tamed mids, fuller kick." }),
      ],
    }
  : { enabled: false, rules: [], intensity: "balanced", version: 1 };
let loudness: LoudnessSettings = { mode: url.has("loudness") ? "auto" : "off", targetLufs: -14, preventClipping: true };
let soundUi: SoundUiSettings = { react_deck: true, react_room: true, layer_chip: true, suggestions: !url.has("nosuggest"), assistant_deck: true };
/** What undoes a reaction: the opposite taste change (merged on top of the rules as they are by then) and the loudness before. */
const undo = new Map<string, { inverse: TasteChange; loudness: LoudnessSettings }>();
let suggesting: "Electronic" | "Jazz" | null = null;
/** The sound a refine conversation works on, and the curve proposed for it. */
let refining: { id: string; proposal: TasteRule | null } | null = null;
/** A Try: the rule and the song it plays on (it ends when the song changes). */
let audition: { rule: TasteRule; path: string } | null = null;
/** The inline "Ask the AI" conversation is on (its answers are proposals). */
let inline = false;

const KIND_RANK: Record<string, number> = { track: 5, album: 4, artist: 3, playlist: 2, genre: 1 };
const KIND_ORDER = ["artist", "track", "album", "playlist", "genre"];
const same = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase() && a.trim() !== "";
const sameScope = (a: TasteScope, b: TasteScope) => a.kind === b.kind && (a.kind !== "genre" && b.kind !== "genre" && a.id && b.id ? a.id === b.id : same(a.name, b.name));
const sharesOutput = (a: TasteRule, b: TasteRule) => !a.outputs?.length || !b.outputs?.length || a.outputs.some((o) => b.outputs!.includes(o));
const playsOn = (r: TasteRule, output: string) => !r.outputs?.length || r.outputs.includes(output);

const noChange = (): TasteChange => ({ upsert: [], remove: [], enabled: null });

/** homefi-core's `TasteChange::apply`: upsert by id, remove by id, never a whole replacement. */
function merge(current: TasteSettings, change: TasteChange): TasteSettings {
  let rules = current.rules.filter((r) => !change.remove.includes(r.id));
  for (const u of change.upsert) {
    const at = rules.findIndex((r) => r.id === u.id);
    rules = at >= 0 ? rules.map((r, i) => (i === at ? u : r)) : [...rules, u];
  }
  return { ...current, rules };
}

/** Applies `change` to the live sounds and returns what undoes it. */
function applyChange(change: TasteChange): TasteChange {
  const inverse: TasteChange = { upsert: [], remove: [], enabled: null };
  for (const u of change.upsert) {
    const prior = taste.rules.find((r) => r.id === u.id);
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

/** Takes the targets of `s` out of any other sound on overlapping outputs. */
function steal(s: TasteRule): TargetMove[] {
  const moves: TargetMove[] = [];
  for (const other of taste.rules) {
    if (other.id === s.id || !sharesOutput(other, s)) continue;
    for (const t of s.targets) {
      const at = other.targets.findIndex((x) => sameScope(x.scope, t.scope));
      if (at >= 0) {
        other.targets.splice(at, 1);
        moves.push({ target: t.scope, from: other.id });
      }
    }
  }
  return moves;
}

function upsert(r: TasteRule): SavedSound {
  const clean = copy(r);
  clean.enabled = true;
  clean.targets = clean.targets.filter((t, i, all) => all.findIndex((x) => sameScope(x.scope, t.scope)) === i);
  const at = taste.rules.findIndex((x) => x.id === clean.id);
  taste = { ...taste, rules: at >= 0 ? taste.rules.map((x, i) => (i === at ? clean : x)) : [...taste.rules, clean] };
  const moves = steal(taste.rules.find((x) => x.id === clean.id)!);
  return { rule: copy(taste.rules.find((x) => x.id === clean.id)!), moves };
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
    // ?taste plays everything from the playlist "Classic Rock Drive"
    source: url.has("taste") ? { kind: "playlist", id: "classic-rock", name: "Classic Rock Drive" } : null,
  };
}

function scopes(c: NowPlayingContext): TasteScope[] {
  const out: TasteScope[] = [];
  if (c.path) out.push({ kind: "track", id: c.path, name: c.title ?? "" });
  if (c.album) out.push({ kind: "album", id: c.album.id, name: c.album.name });
  if (c.artists[0]) out.push({ kind: "artist", id: c.artists[0].id, name: c.artists[0].name });
  if (c.source?.kind === "playlist") out.push({ kind: "playlist", id: c.source.id, name: c.source.name });
  if (c.genres[0]) out.push({ kind: "genre", name: c.genres[0] });
  return out;
}

/** How early in the song's own lists `s` matches (-1: it doesn't). */
function position(s: TasteScope, c: NowPlayingContext): number {
  if (s.kind === "genre") return c.genres.findIndex((g) => same(g, s.name));
  if (s.kind === "artist") return c.artists.findIndex((a) => same(a.name, s.name));
  if (s.kind === "album") return c.album && same(c.album.name, s.name) ? 0 : -1;
  if (s.kind === "playlist") return c.source?.kind === "playlist" && (c.source.id === s.id || same(c.source.name, s.name)) ? 0 : -1;
  return c.path === s.id || (!s.id && same(c.title ?? "", s.name)) ? 0 : -1;
}

interface Resolved {
  rule: TasteRule;
  target: TasteScope;
}
function resolve(c: NowPlayingContext | null, output: string, master: boolean): Resolved | null {
  if (!c || !master) return null;
  if (audition && audition.path === c.path) return { rule: audition.rule, target: { kind: "track", id: c.path ?? "", name: c.title ?? "" } };
  let best: { rank: number; pos: number; rule: TasteRule; target: TasteScope } | null = null;
  for (const r of taste.rules) {
    if (!playsOn(r, output)) continue;
    for (const t of r.targets) {
      const pos = position(t.scope, c);
      if (pos < 0) continue;
      const rank = KIND_RANK[t.scope.kind];
      if (!best || rank > best.rank || (rank === best.rank && -pos >= -best.pos)) best = { rank, pos, rule: r, target: t.scope };
    }
  }
  return best && { rule: best.rule, target: best.target };
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

function react(reaction: SoundReaction, c: NowPlayingContext | null, hint: TasteScope | null, output: string, master: boolean): ReactionResult {
  const out: ReactionResult = { rule: null, taste: copy(taste), tasteChange: noChange(), loudness: { ...loudness }, message: "", undoToken: "", byAi: false };
  if (reaction === "perfect") {
    const r = resolve(c, output, master);
    return { ...out, message: r ? `Great, it stays as it is (“${r.rule.label}”).` : "Great, nothing to change." };
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
  const existing = taste.rules.find((r) => playsOn(r, output) && r.targets.some((t) => sameScope(t.scope, scope)));
  const base = existing?.tilt ?? resolve(c, output, true)?.rule.tilt ?? { bassDb: 0, trebleDb: 0, clarityDb: null };
  const sign = SIGNS[reaction];
  const db = STEP_DB[taste.intensity];
  const step: Tilt = { bassDb: sign.bassDb * db, trebleDb: sign.trebleDb * db, clarityDb: (sign.clarityDb ?? 0) * db };
  const tilt: Tilt = {
    bassDb: cap(base.bassDb + step.bassDb),
    trebleDb: cap(base.trebleDb + step.trebleDb),
    clarityDb: base.clarityDb == null && !step.clarityDb ? null : cap((base.clarityDb ?? 0) + (step.clarityDb ?? 0)),
  };
  const by: TasteCreator = url.has("aikey") ? "ai" : "reaction";
  const r: TasteRule = existing
    ? { ...copy(existing), tilt, label: `${LABELS[reaction]} · ${scope.name}` }
    : sound(`r${Date.now()}`, `${LABELS[reaction]} · ${scope.name}`, by, tilt, [target(scope, Date.now(), by)], { outputs: [output] });
  const tasteChange: TasteChange = { upsert: [r], remove: [], enabled: null };
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

// ------------------------------------------------------------------ search

/** Library playlists the search knows (the real app lists the user's own). */
const LIBRARY_PLAYLISTS: { scope: TasteScope; tracks: number }[] = [
  { scope: playlist("classic-rock", "Classic Rock Drive"), tracks: 48 },
  { scope: playlist("every-song", "Every song, ever"), tracks: 437 },
];
/** What TIDAL answers (a few fixed hits; the real search is the catalog's). */
const TIDAL_HITS: { scope: TasteScope; subtitle: string }[] = [
  { scope: { kind: "album", id: "tidal-aja", name: "Aja" }, subtitle: "Album · Steely Dan" },
  { scope: playlist("tidal-sd-essentials", "Steely Dan Essentials"), subtitle: "Playlist · 52 tracks" },
  { scope: { kind: "track", id: "tidal-peg", name: "Peg" }, subtitle: "Song · Steely Dan" },
];

/** 0 exact, 1 starts with it, 2 a word starts with it, 3 contains it. */
function matchQuality(name: string, q: string): number {
  const n = name.toLowerCase();
  if (n === q) return 0;
  if (n.startsWith(q)) return 1;
  if (n.split(/\s+/).some((w) => w.startsWith(q))) return 2;
  return n.includes(q) ? 3 : -1;
}

function search(query: string, library: Track[], output: string): TargetHit[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  const pool: { scope: TasteScope; subtitle: string; source: TargetHit["source"] }[] = [];
  const seen = new Set<string>();
  const add = (scope: TasteScope, subtitle: string, source: TargetHit["source"]) => {
    const key = `${scope.kind}:${scope.name}`;
    if (!seen.has(key)) (seen.add(key), pool.push({ scope, subtitle, source }));
  };
  for (const t of library) {
    if (t.artist) add(artist(t.artist), "Artist", "library");
    if (t.album) add(album(t.album), `Album · ${t.artist}`, "library");
    add({ kind: "track", id: t.path, name: t.title }, `Song · ${t.artist}`, "library");
    for (const g of t.genre.split(",")) if (g.trim()) add(genre(g.trim()), "Genre", "library");
  }
  for (const p of LIBRARY_PLAYLISTS) add(p.scope, `Playlist · ${p.tracks} tracks`, "library");
  for (const h of TIDAL_HITS) add(h.scope, h.subtitle, "tidal");
  return pool
    // a name that matches ranks first; a song or album by a matching artist comes after
    .map((p) => ({ ...p, quality: matchQuality(p.scope.name, q) >= 0 ? matchQuality(p.scope.name, q) : p.subtitle.toLowerCase().includes(q) ? 4 : -1 }))
    .filter((p) => p.quality >= 0)
    .sort((a, b) => a.quality - b.quality || KIND_ORDER.indexOf(a.scope.kind) - KIND_ORDER.indexOf(b.scope.kind) || (a.source === b.source ? 0 : a.source === "library" ? -1 : 1) || a.scope.name.localeCompare(b.scope.name))
    .slice(0, 30)
    .map((p) => ({ scope: p.scope, subtitle: p.subtitle, source: p.source, inSound: taste.rules.find((r) => playsOn(r, output) && r.targets.some((t) => sameScope(t.scope, p.scope)))?.id ?? null }));
}

// ------------------------------------------------------------------ the inline AI

/** A proposal for what the user asked, on the artist playing (the real AI reads the whole request). */
function propose(text: string, c: NowPlayingContext | null, es: boolean): AssistantTurn {
  const say = (en: string, s: string) => (es ? s : en);
  const who = c?.artists[0]?.name ?? "this song";
  const t = text.toLowerCase();
  let label = say("A softer touch", "Un toque más suave");
  let tilt: Tilt = { bassDb: 0, trebleDb: -1, clarityDb: null };
  let bands: TasteBand[] = [];
  let why = say("A small, gentle change.", "Un cambio pequeño y suave.");
  if (/warm|c[aá]lid/.test(t)) {
    label = say("Warmer", "Más cálido");
    tilt = { bassDb: 2.5, trebleDb: -1, clarityDb: null };
    why = say("More low warmth and a little less treble.", "Más calidez en graves y un poco menos de agudos.");
  } else if (/bass|boom|bajo/.test(t)) {
    label = /less|too|menos/.test(t) ? say("Less boomy", "Menos retumbo") : say("More bass", "Más bajo");
    bands = [band("peak", 180, /less|too|menos/.test(t) ? -3 : 3, 1.1, /less|too|menos/.test(t) ? "Cut 180 Hz −3 dB: tighter bass" : "+3 dB at 180 Hz: more body")];
    why = say("Tighter bass, vocals stay put.", "Bajo más firme, las voces se quedan donde están.");
  } else if (/vocal|clear|voz|voces/.test(t)) {
    label = say("Vocals clearer", "Voces más claras");
    tilt = { bassDb: 0, trebleDb: 0, clarityDb: 2 };
    why = say("Lifts the range where voices live.", "Sube la zona donde viven las voces.");
  } else if (/harsh|[aá]spero|sibil/.test(t)) {
    label = say("Less harsh", "Menos áspero");
    bands = [band("peak", 3200, -2.5, 3, "−2.5 dB at 3.2 kHz: softens the harsh edge"), band("peak", 7200, -2, 4.5, "−2 dB at 7.2 kHz: tames hissing s's")];
    why = say("Narrow cuts where the harshness lives.", "Cortes estrechos donde vive la aspereza.");
  } else if (/high|treble|agudo/.test(t)) {
    label = say("Quieter highs", "Agudos más suaves");
    tilt = { bassDb: 0, trebleDb: -2.5, clarityDb: null };
    why = say("Brings the top end down a notch.", "Baja un poco la parte alta.");
  }
  const proposal: TasteRule = sound(`ai-${Date.now()}`, `${label} for ${who}`, "ai", tilt, c?.artists[0] ? [target(artist(who), Date.now())] : [], { bands, explanation: why });
  return {
    text: say(`Here's “${proposal.label}”: ${why} Try it, and keep it if you like it.`, `Aquí va «${proposal.label}»: ${why} Pruébala y quédate con ella si te gusta.`),
    quick_replies: [say("Less", "Menos"), say("A bit more", "Un poco más"), say("Vocals clearer", "Voces más claras")],
    proposal: null,
    confirmed: false,
    done: false,
    taste_proposal: proposal,
  };
}

export const soundMock = {
  settings: () => ({ taste: copy(taste), loudness: { ...loudness }, sound_ui: { ...soundUi } }),
  setSettings(s: { taste?: TasteSettings; loudness?: LoudnessSettings; sound_ui?: SoundUiSettings }) {
    if (s.taste) taste = copy(s.taste);
    if (s.loudness) loudness = s.loudness;
    if (s.sound_ui) soundUi = s.sound_ui;
  },
  /** `?audition`: a Try of "Warmer for Eagles" on the song playing at the start. */
  startAudition(track: Track | null) {
    if (!url.has("audition") || !track) return;
    audition = {
      path: track.path,
      rule: sound("try-warmer", "Warmer for Eagles", "ai", { bassDb: 2.5, trebleDb: -1, clarityDb: null }, [target(artist("Eagles"), Date.now())], { explanation: "More low warmth and a little less treble." }),
    };
  },
  /** `eq_active` and the sound fields of the player for `track` on `device`, on top of the equalizer's own. */
  playerFlags(track: Track | null, eq: { eq_active: boolean; eq_profile: string | null }, device: string, master: boolean) {
    if (audition && audition.path !== track?.path) audition = null;
    const r = resolve(contextOf(track), device, master);
    const loud = loudness.mode === "off" || !master ? 0 : track?.album === "Kind of Blue" ? 3.4 : -4.2 + (loudness.targetLufs + 14);
    return { ...eq, eq_active: eq.eq_active || !!r, taste_rule: r?.rule.id ?? null, taste_label: r?.rule.label ?? null, taste_target: r?.target ?? null, auditioning: !!r && !!audition && audition.rule.id === r.rule.id, loudness_db: Math.round(loud * 10) / 10 };
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
  /** The inline panel starts a session with its first words as notes: answer with a proposal. */
  inlineStart(notes: string | null, current: Track | null, es: boolean): AssistantTurn | null {
    inline = !!notes;
    return notes ? propose(notes, contextOf(current), es) : null;
  },
  /** The mock assistant: an answer to the inline panel, the suggestion card or a refine (null when it isn't one). */
  reply(text: string, current: Track | null, es: boolean): AssistantTurn | null {
    if (inline) return /^(s[ií]|yes|ok|dale|claro|apply|aplic)/i.test(text.trim()) ? { text: "Done.", quick_replies: [], proposal: null, confirmed: false, done: true } : propose(text, contextOf(current), es);
    if (refining?.proposal) {
      const p = refining.proposal;
      refining = null;
      const base: AssistantTurn = { text: "", quick_replies: [], proposal: null, confirmed: false, done: true };
      if (!/^(s[ií]|yes|ok|dale|claro|apply|aplic)/i.test(text.trim())) return { ...base, text: "OK, nothing changed." };
      const taste_change: TasteChange = { upsert: [p], remove: [], enabled: null };
      applyChange(taste_change);
      return { ...base, text: `Done: “${p.label}” now has ${p.bands?.length ?? 0} bands.`, taste_change };
    }
    const g = suggesting;
    if (!g) return null;
    suggesting = null;
    const base: AssistantTurn = { text: "", quick_replies: [], proposal: null, confirmed: false, done: true };
    if (!/^(s[ií]|yes|ok|dale|claro)/i.test(text.trim())) return { ...base, text: "OK, nothing changed." };
    const tilt: Tilt = g === "Jazz" ? { bassDb: 0, trebleDb: 1, clarityDb: 1.5 } : { bassDb: 3, trebleDb: 0, clarityDb: null };
    const label = g === "Jazz" ? "A touch more detail · Jazz" : "A bit more bass · Electronic";
    const r = sound(`r${Date.now()}`, label, url.has("aikey") ? "ai" : "guided", tilt, [target(genre(g), Date.now(), "guided")]);
    const taste_change: TasteChange = { upsert: [r], remove: [], enabled: null };
    applyChange(taste_change);
    return { ...base, text: `Done: “${label}”. You'll find it in Sound, to change or remove it.`, taste_change };
  },
  handlers: (current: () => Track | null, changed: () => void, output: () => string, master: () => boolean, library: () => Track[]) => ({
    taste_get: () => copy(taste),
    taste_set: ({ taste: t }: { taste: TasteSettings }) => ((taste = copy(t)), changed(), copy(taste)),
    loudness_get: () => loudness,
    loudness_set: ({ loudness: l }: { loudness: LoudnessSettings }) => ((loudness = l), changed(), loudness),
    sound_now: (): NowSound => {
      const c = contextOf(current());
      const r = resolve(c, output(), master());
      return { context: c, scopes: c ? scopes(c) : [], rule: r ? copy(r.rule) : null, target: r?.target ?? null, output: output(), auditioning: !!r && !!audition && audition.rule.id === r.rule.id };
    },
    taste_assign: ({ soundId, scope }: { soundId: string; scope: TasteScope }): TargetMove[] => {
      const s = taste.rules.find((r) => r.id === soundId);
      if (!s) throw "That sound doesn't exist anymore.";
      const moves = s.targets.some((t) => sameScope(t.scope, scope)) ? [] : steal({ ...s, targets: [target(scope, Date.now(), "manual")] });
      if (!s.targets.some((t) => sameScope(t.scope, scope))) s.targets.push(target(scope, Date.now(), "manual"));
      changed();
      return moves;
    },
    taste_unassign: ({ soundId, scope }: { soundId: string; scope: TasteScope }) => {
      const s = taste.rules.find((r) => r.id === soundId);
      if (s) s.targets = s.targets.filter((t) => !sameScope(t.scope, scope));
      changed();
    },
    taste_undo_moves: ({ moves }: { moves: TargetMove[] }) => {
      for (const m of moves) {
        const home = taste.rules.find((r) => r.id === m.from);
        if (!home) continue;
        for (const other of taste.rules) if (other.id !== home.id && sharesOutput(other, home)) other.targets = other.targets.filter((t) => !sameScope(t.scope, m.target));
        if (!home.targets.some((t) => sameScope(t.scope, m.target))) home.targets.push(target(m.target, 0, "manual"));
      }
      changed();
    },
    taste_set_outputs: ({ soundId, outputs }: { soundId: string; outputs: string[] }): TargetMove[] => {
      const s = taste.rules.find((r) => r.id === soundId);
      if (!s) throw "That sound doesn't exist anymore.";
      s.outputs = outputs;
      const moves = steal(s);
      changed();
      return moves;
    },
    taste_save: ({ rule }: { rule: TasteRule }): SavedSound => {
      const saved = upsert(rule);
      changed();
      return saved;
    },
    taste_remove: ({ soundId }: { soundId: string }) => {
      const had = taste.rules.some((r) => r.id === soundId);
      taste = { ...taste, rules: taste.rules.filter((r) => r.id !== soundId) };
      changed();
      return had;
    },
    taste_audition: ({ rule }: { rule: TasteRule | null }) => {
      const t = current();
      if (!rule) {
        audition = null;
        changed();
        return true;
      }
      if (!t || !master()) return false;
      audition = { rule: copy(rule), path: t.path };
      changed();
      return true;
    },
    taste_keep: ({ rule, scope, onlyOn }: { rule: TasteRule; scope: TasteScope; onlyOn: string | null }): SavedSound => {
      audition = null;
      const existing = taste.rules.find((r) => r.id === rule.id);
      const base = existing ? copy(existing) : { ...copy(rule), targets: [] as TasteRule["targets"], outputs: onlyOn ? [onlyOn] : (rule.outputs ?? []) };
      if (!base.targets.some((t) => sameScope(t.scope, scope))) base.targets.push(target(scope, Date.now(), "ai"));
      const saved = upsert(base);
      changed();
      return saved;
    },
    sound_search_targets: async ({ query }: { query: string }) => {
      await wait(120);
      return search(query, library(), output());
    },
    sound_react: async ({ reaction, scope }: { reaction: SoundReaction; scope: TasteScope | null }) => {
      await wait(url.has("aikey") ? 900 : 250);
      const r = react(reaction, contextOf(current()), scope, output(), master());
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
      if (taste.rules.some((r) => r.targets.some((t) => t.scope.kind === "genre" && same(t.scope.name, "Electronic")))) {
        suggesting = "Jazz";
        return { text: "Jazz is your second most played genre. Want a touch more detail when it plays (+1 dB treble, +1.5 dB clarity)?", quick_replies: ["Yes, try it", "No, thanks"], proposal: null, confirmed: false, done: false };
      }
      suggesting = "Electronic";
      return { text: "You listen to a lot of Electronic. Want a bit more bass when it plays (+3 dB bass)? Only that music changes.", quick_replies: ["Yes, try it", "No, thanks"], proposal: null, confirmed: false, done: false };
    },
    listening_summary: () => ({ top_genres: ["Electronic", "Jazz", "Alternative"], top_artists: [{ id: "", name: "Daft Punk" }, { id: "", name: "Miles Davis" }], playlists: [{ id: "1", name: "Every song, ever" }] }),
  }),
};
