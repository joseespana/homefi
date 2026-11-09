// Development-only backend for "Get ready for a concert" (mock.ts spreads `handlers`).
// The same shapes as the Rust commands (src-tauri/src/playlists.rs), with Korn's 2026 tour as data.
// Flags: ?concert opens the concert screen with a draft built; ?concertbuilding stays at
// "Finding the songs: 9 of 21"; ?concertpopular builds Tidal's most played (with its fallback
// note); ?concertgemini adds Google's Search Suggestions sample; ?noai / ?nowebsearch change
// `concert_sources`; ?homeplaylist opens a saved playlist.
// "Make a party playlist": ?party opens the screen, ?partychat starts the chat, ?partybuilt builds
// a ~2 h playlist, ?partybuilding stays at "Finding the songs: 9 of 40", ?partyshort builds one
// with a shortfall note, ?partynoai is the no-AI state (also makes `concert_sources` say so).
// "Make a playlist with AI": ?chat opens the screen (tap a suggestion to talk), ?chatbuilt sends a
// first message and builds, ?chatbuilding stays at "Finding the songs: 9 of 30", ?chatnoai is the
// no-AI state.
import type { AgeGroup, ChatBrief, ConcertBuilt, ConcertProgress, ConcertRequest, ConcertShow, ConcertSources, EntryTag, HomePlaylist, PartyBrief, PartyBuilt, PartyProgress, PartyTurn, PlaylistEntry, PlaylistSettings, SongCandidate, SongSeed } from "./api";

const url = new URLSearchParams(location.search);
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
const NOW_MS = Date.UTC(2026, 10, 2);
const CLAMP_MIN_PCT = 5;

const CHAT_FLAGS = ["chat", "chatbuilt", "chatbuilding", "chatnoai"];

// dev: the flags open their screen (App.svelte reads `#name` once the app is ready)
if (!location.hash) {
  if (["concert", "concertbuilding", "concertpopular", "concertgemini"].some((f) => url.has(f))) location.hash = "#concert";
  else if (["party", "partychat", "partybuilt", "partybuilding", "partyshort", "partynoai"].some((f) => url.has(f))) location.hash = "#party";
  else if (CHAT_FLAGS.some((f) => url.has(f))) location.hash = "#chat-playlist";
  else if (url.has("homeplaylist")) location.hash = "#home-playlist";
}

const defaults: PlaylistSettings = {
  concert_shows: 8,
  concert_thresholds: { always_pct: 80, usually_pct: 50, sometimes_pct: 25, encore_pct: 50 },
  prefer_library: true,
  search_budget: 5,
  skip_tape: true,
  covers_original_artist: true,
  show_sources: true,
  home_section: true,
  party_minutes: 120,
  party_curve: "arc",
  party_allow_explicit: true,
  party_max_per_artist: 3,
  chat_minutes: 60,
  chat_curve: "steady",
  chat_allow_explicit: true,
  chat_from_song: true,
  tidal_auto_save: false,
  tidal_delete_with: false,
};
let settings: PlaylistSettings = { ...defaults };

const roundMinutes = (minutes: number) => Math.min(480, Math.max(30, Math.round(minutes / 15) * 15));

/** Like the core's `clamped()`: usually <= always and sometimes <= usually. */
function clamp(s: PlaylistSettings): PlaylistSettings {
  const pct = (n: number) => Math.min(100, Math.max(CLAMP_MIN_PCT, Math.round(n)));
  const always = pct(s.concert_thresholds.always_pct);
  const usually = Math.min(always, pct(s.concert_thresholds.usually_pct));
  const sometimes = Math.min(usually, pct(s.concert_thresholds.sometimes_pct));
  return { ...s, concert_shows: Math.min(15, Math.max(3, s.concert_shows)), search_budget: Math.min(10, Math.max(1, s.search_budget)), party_minutes: roundMinutes(s.party_minutes), chat_minutes: roundMinutes(s.chat_minutes), party_max_per_artist: Math.min(10, Math.max(1, s.party_max_per_artist)), concert_thresholds: { always_pct: always, usually_pct: usually, sometimes_pct: sometimes, encore_pct: pct(s.concert_thresholds.encore_pct) } };
}

// ---- data
const CITIES: [string, string, string, string][] = [
  ["2026-10-24", "Arena CDMX", "Mexico City", "MX"],
  ["2026-10-22", "Arena Monterrey", "Monterrey", "MX"],
  ["2026-10-20", "Auditorio Telmex", "Guadalajara", "MX"],
  ["2026-10-17", "Movistar Arena", "Bogotá", "CO"],
  ["2026-10-14", "Movistar Arena", "Santiago", "CL"],
  ["2026-10-11", "Movistar Arena", "Buenos Aires", "AR"],
  ["2026-10-08", "Allianz Parque", "São Paulo", "BR"],
  ["2026-10-05", "Arena Lima", "Lima", "PE"],
];
const SET = ["Blind", "Here to Stay", "Twist", "Got the Life", "Freak on a Leash", "Falling Away from Me", "Coming Undone", "Y'all Want a Single", "Shoots and Ladders", "Somebody Someone"];
const shows: ConcertShow[] = CITIES.map(([date, venue, city, country], i) => ({
  id: `${date}-0`,
  date,
  venue,
  city,
  country,
  tour: "2026 Tour",
  url: `https://www.example.com/korn-${date}`,
  songs: [
    { name: "Intro", original_artist: null, tape: true, encore: false },
    ...SET.filter((_, j) => j < 5 || (i + j) % 3 !== 0).map((name, j, all) => ({ name, original_artist: null, tape: false, encore: j >= all.length - 2 })),
  ],
}));

const tint = (seed: number) => `data:image/svg+xml,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="hsl(${(seed * 47) % 360} 45% 32%)"/><circle cx="5" cy="5" r="2.4" fill="hsl(${(seed * 47 + 40) % 360} 60% 60%)"/></svg>`)}`;

const entry = (i: number, title: string, album: string, tag: EntryTag, note: string, extra: Partial<PlaylistEntry> = {}): PlaylistEntry => ({
  title,
  artist: "Korn",
  album,
  tidal_id: String(5000 + i),
  isrc: `USSM1${String(9400000 + i)}`,
  local_path: null,
  duration_ms: 200_000 + ((i * 37_000) % 120_000),
  explicit: i % 3 === 0,
  cover_url: tint(i),
  tag,
  note,
  ...extra,
});
const played = (p: number, suffix = "") => `Played in ${p} of 8 shows${suffix}`;
const tourEntries = (): PlaylistEntry[] => [
  entry(1, "Blind", "Korn", "always", played(8)),
  entry(2, "Here to Stay", "Untouchables", "always", played(8)),
  entry(3, "Twist", "Life Is Peachy", "always", played(7)),
  entry(4, "Got the Life", "Follow the Leader", "always", played(7), { local_path: "/Users/me/Music/Korn/Got the Life.flac" }),
  entry(5, "Freak on a Leash", "Follow the Leader", "usually", played(6)),
  entry(6, "Falling Away from Me", "Issues", "usually", played(6)),
  entry(7, "Coming Undone", "See You on the Other Side", "usually", played(5)),
  entry(8, "Y'all Want a Single", "Take a Look in the Mirror", "usually", played(5)),
  entry(9, "Shoots and Ladders", "Korn", "sometimes", played(3)),
  entry(10, "Somebody Someone", "Issues", "sometimes", played(3)),
  entry(11, "Make Me Bad", "Issues", "sometimes", played(2)),
  entry(12, "Clown", "Korn", "encore", played(8, " · encore")),
  entry(13, "Falling Away from Me (Live)", "Unplugged", "encore", played(4, " · encore")),
  entry(14, "Thoughtless", "Untouchables", "encore", played(4, " · encore · Korn cover")),
];
const popularEntries = (artist: string): PlaylistEntry[] =>
  ["Freak on a Leash", "Blind", "Coming Undone", "Falling Away from Me", "Got the Life", "Twist", "Here to Stay", "Make Me Bad", "Y'all Want a Single", "Thoughtless", "Clown", "Right Now", "Did My Time", "Alone I Break"].map((t, i) =>
    entry(i + 40, t, "Greatest Hits", "plain", "", { artist }),
  );
const unmatched = (): SongCandidate[] => [
  { title: "Some Jam", artist: "Korn", tag: "sometimes", note: played(2) },
  { title: "Kill Mercy Within", artist: "Korn", tag: "sometimes", note: played(2) },
];

const WIDGET_HTML = `<style>
  .container{font-family:Google Sans,Roboto,Arial,sans-serif;display:flex;align-items:center;gap:8px;padding:6px 0}
  .headline{display:none}.carousel{display:flex;flex-wrap:wrap;gap:8px}
  .chip{display:inline-block;padding:8px 14px;border-radius:18px;font-size:14px;text-decoration:none;background:#f1f3f4;color:#3c4043;border:1px solid #dadce0}
  @media (prefers-color-scheme: dark){.chip{background:#303134;color:#e8eaed;border-color:#5f6368}}
</style>
<div class="container"><div class="carousel">
  <a class="chip" href="https://www.google.com/search?q=korn+setlist+2026">korn setlist 2026</a>
  <a class="chip" href="https://www.google.com/search?q=korn+tour+dates">korn tour dates</a>
  <a class="chip" href="https://www.google.com/search?q=korn+live+mexico+city">korn live mexico city</a>
</div></div>`;

const sourceLinks = () => [
  { url: "https://www.example.com/korn-2026-10-24", title: "Korn live in Mexico City" },
  { url: "https://www.example.com/korn-2026-10-22", title: "Korn live in Monterrey" },
  { url: "https://www.example.com/korn-tour-2026", title: "Korn 2026 tour" },
];

let counter = 0;
const newId = () => `p${(NOW_MS + ++counter).toString(16)}`;

function sources(): ConcertSources {
  const ai = url.has("noai") || url.has("partynoai") || url.has("chatnoai")
    ? null
    : url.has("nowebsearch")
      ? { provider_id: "mistral", provider_name: "Mistral", model: "mistral-large", web_search: false }
      : url.has("concertgemini")
        ? { provider_id: "gemini", provider_name: "Google Gemini", model: "gemini-3.8-flash", web_search: true }
        : { provider_id: "anthropic", provider_name: "Anthropic (Claude)", model: "claude-sonnet-5-5", web_search: true };
  return { ai, tidal: !url.has("notidal") };
}

function playlist(request: ConcertRequest, popular: boolean, id = newId(), createdMs = NOW_MS): HomePlaylist {
  const ai = sources().ai;
  const artist = request.artist.trim() || "Korn";
  const single = request.mode.kind === "show" ? shows.find((s) => request.mode.kind === "show" && s.id === request.mode.show_id) : undefined;
  const entries = popular ? popularEntries(artist) : single ? tourEntries().slice(0, 9).map((e) => ({ ...e, tag: "plain" as const, note: "Played in 1 of 1 shows" })) : tourEntries();
  return {
    version: 1,
    id,
    name: popular ? `${artist} · Most played` : single ? `${artist} · ${single.city}, ${single.date}` : `${artist} · Tour setlist`,
    description: popular ? `Tidal's most played for ${artist}. Not the real setlist.` : single ? `One show: ${single.venue}, ${single.city}` : `From ${request.shows} recent shows`,
    created_ms: createdMs,
    updated_ms: NOW_MS,
    origin: {
      kind: "concert",
      artist,
      source: popular || !ai ? { kind: "tidal_popular" } : { kind: "ai", provider_id: ai.provider_id, model: ai.model },
      mode: request.mode,
      shows_wanted: request.shows,
      thresholds: request.thresholds,
      shows: popular ? [] : shows.slice(0, request.shows),
      fetched_ms: NOW_MS,
    },
    entries,
    unmatched: popular ? [] : unmatched(),
    sources: popular ? [] : sourceLinks(),
    search_widget_html: !popular && url.has("concertgemini") ? WIDGET_HTML : null,
  };
}


let abort: (() => void) | null = null;
/** Sends a progress event through the Tauri channel, like the Rust side. */
const sent = new Map<number, number>();
const emit = (channel: { id: number }, p: ConcertProgress | PartyProgress) => {
  // a Tauri channel delivers messages by their index
  const index = sent.get(channel.id) ?? 0;
  sent.set(channel.id, index + 1);
  (window as any)[`_${channel.id}`]?.({ index, message: p });
};

async function build(request: ConcertRequest, channel: { id: number }): Promise<ConcertBuilt> {
  const popular = request.popular_only || url.has("concertpopular") || !sources().ai?.web_search;
  const cancelled = new Promise<never>((_, reject) => (abort = () => reject("cancelled")));
  const run = async () => {
    emit(channel, { kind: "searching", with_ai: !popular });
    await wait(500);
    if (!popular) {
      emit(channel, { kind: "shows_found", count: request.shows });
      await wait(400);
    }
    const total = 21;
    for (let done = 1; done <= total; done++) {
      emit(channel, { kind: "resolving", done, total });
      if (url.has("concertbuilding") && done === 9) await new Promise(() => {});
      await wait(60);
    }
    const ai = sources().ai;
    const note = popular
      ? request.popular_only
        ? null
        : !ai
          ? `No AI is set up, so these are ${request.artist.trim() || "Korn"}'s most played songs on Tidal, not the real setlist.`
          : ai.web_search
            ? `Your AI didn't find enough recent setlists for ${request.artist.trim() || "Korn"}, so these are their most played songs on Tidal.`
            : `${ai.provider_name} can't search the web, so these are ${request.artist.trim() || "Korn"}'s most played songs on Tidal, not the real setlist.`
      : null;
    return { playlist: playlist(request, popular), fallback_note: note };
  };
  try {
    return await Promise.race([run(), cancelled]);
  } finally {
    abort = null;
  }
}


// ---- party playlists
const PARTY_SONGS: [string, string, string][] = [
  ["Dreams", "Fleetwood Mac", "Rumours"], ["September", "Earth, Wind & Fire", "The Best of"], ["Valerie", "Amy Winehouse", "Back to Black"], ["Put Your Records On", "Corinne Bailey Rae", "Corinne Bailey Rae"],
  ["Sunday Morning", "Maroon 5", "Songs About Jane"], ["Come Away With Me", "Norah Jones", "Come Away With Me"], ["Superstition", "Stevie Wonder", "Talking Book"], ["Don't Stop Me Now", "Queen", "Jazz"],
  ["Dancing Queen", "ABBA", "Arrival"], ["Uptown Funk", "Mark Ronson", "Uptown Special"], ["Get Lucky", "Daft Punk", "Random Access Memories"], ["Mr. Brightside", "The Killers", "Hot Fuss"],
  ["Pump It Up", "Elvis Costello", "This Year's Model"], ["Dance the Night", "Dua Lipa", "Barbie The Album"], ["Levitating", "Dua Lipa", "Future Nostalgia"], ["Shake It Off", "Taylor Swift", "1989"],
  ["Titi Me Preguntó", "Bad Bunny", "Un Verano Sin Ti"], ["Danza Kuduro", "Don Omar", "Meet the Orphans"], ["I Wanna Dance with Somebody", "Whitney Houston", "Whitney"], ["Billie Jean", "Michael Jackson", "Thriller"],
  ["Livin' on a Prayer", "Bon Jovi", "Slippery When Wet"], ["Africa", "Toto", "Toto IV"], ["Vienna", "Billy Joel", "The Stranger"], ["Take Me Home, Country Roads", "John Denver", "Poems, Prayers & Promises"], ["Here Comes the Sun", "The Beatles", "Abbey Road"],
];
const PARTY_MISSING: SongCandidate[] = [
  { title: "Bailando Juntos", artist: "Los Ejemplos", tag: "plain", note: "" },
  { title: "Fiesta en la Azotea", artist: "Banda Imaginaria", tag: "plain", note: "" },
];
const VIBE_NAME: Record<PartyBrief["vibe"], string> = { background: "Background", chill: "Chill", dance: "Dance", singalong: "Sing-along", throwback: "Throwback" };
const GUEST_NAME: Record<PartyBrief["guests"], string> = { friends: "friends", family: "family", coworkers: "coworkers", mixed: "a mix of people" };
const AGE_NAME: Record<AgeGroup, string> = { kids: "kids", teens: "teens", young_adults: "18–29", adults: "30–49", seniors: "50+" };
const AGE_ORDER = Object.keys(AGE_NAME) as AgeGroup[];
const MIN_PER_HOUR = 60;
const lengthLabel = (minutes: number) => (minutes < MIN_PER_HOUR ? `${minutes} min` : minutes % MIN_PER_HOUR ? `${Math.floor(minutes / MIN_PER_HOUR)} h ${minutes % MIN_PER_HOUR} min` : `${minutes / MIN_PER_HOUR} h`);
const WARM_UP_SHARE = 0.25;
const WIND_DOWN_SHARE = 0.2;
const PARTY_TURN_DELAY_MS = 600;
const PARTY_QUESTIONS: PartyTurn[] = [
  { text: "Any artists everyone loves?", quick_replies: ["Bad Bunny", "80s pop", "No idea"] },
  { text: "Should it stay in English, or mix in Spanish songs?", quick_replies: ["English only", "Mix both", "Mostly Spanish"] },
  { text: "Anything you'd rather not hear?", quick_replies: ["Nothing", "No slow songs", "No rap"] },
];
const CHAT_LIMIT_TURN: PartyTurn = { text: "That's plenty to go on. Build the playlist when you're ready.", quick_replies: [] };
const MAX_CHAT_TURNS = 6;
const MAX_FREE_CHAT_TURNS = 12;

const DEFAULT_BRIEF: PartyBrief = { guests: "friends", ages: [], vibe: "dance", minutes: 120, curve: "arc", allow_explicit: true, notes: "" };
let partyBrief: PartyBrief | null = null;
/** The open session's brief when it is a chat ("Make a playlist with AI"); null for a party. */
let chatBrief: ChatBrief | null = null;
let partyTurns = 0;

const CHAT_OPENER: PartyTurn = {
  text: "What kind of playlist do you want today? Tell me how you feel, a decade, a language, a genre, how long…",
  quick_replies: ["I'm feeling down", "Happy and upbeat", "90s in English", "Something to focus"],
};
const CHAT_FOLLOW_UPS: string[] = ["Anything you'd rather avoid?", "Calmer or more upbeat?", "Any artists to lean on?"];
const CHAT_SUGGESTIONS: string[][] = [
  ["Mostly guitars", "A bit calmer", "No explicit lyrics"],
  ["Make it 30 minutes", "Add some ballads"],
  ["Sounds good", "Make it 2 hours"],
];
const CHAT_TITLE = "Sad 90s in English";
const CHAT_SONGS: [string, string, string][] = [
  ["Creep", "Radiohead", "Pablo Honey"], ["Mad World", "Gary Jules", "Donnie Darko"], ["Nothing Compares 2 U", "Sinéad O'Connor", "I Do Not Want What I Haven't Got"], ["Black", "Pearl Jam", "Ten"],
  ["Tears in Heaven", "Eric Clapton", "Rush"], ["Fade Into You", "Mazzy Star", "So Tonight That I Might See"], ["Hurt", "Nine Inch Nails", "The Downward Spiral"], ["Zombie", "The Cranberries", "No Need to Argue"],
  ["Everybody Hurts", "R.E.M.", "Automatic for the People"], ["Wonderwall", "Oasis", "(What's the Story) Morning Glory?"], ["Hallelujah", "Jeff Buckley", "Grace"], ["Iris", "Goo Goo Dolls", "Dizzy Up the Girl"],
  ["Linger", "The Cranberries", "Everybody Else Is Doing It"], ["Boys Don't Cry", "The Cure", "Boys Don't Cry"], ["Disarm", "The Smashing Pumpkins", "Siamese Dream"],
];

function partyList(brief: PartyBrief, id: string, createdMs: number): HomePlaylist {
  const ai = sources().ai;
  const target = Math.max(1, Math.round((brief.minutes * MIN_PER_HOUR * 1000) / 288_000));
  const songs = Array.from({ length: Math.min(target, 40) }, (_, i) => PARTY_SONGS[i % PARTY_SONGS.length]);
  const entries = songs.map(([title, artist, album], i): PlaylistEntry => {
    const at = i / songs.length;
    const note = brief.curve !== "arc" ? "" : at < WARM_UP_SHARE ? "Warm-up" : at >= 1 - WIND_DOWN_SHARE ? "Wind-down" : "Peak";
    return entry(i + 100, title, album, "plain", note, { artist, duration_ms: 270_000 + ((i * 13_000) % 36_000), explicit: brief.allow_explicit && i % 7 === 3 });
  });
  const ages = brief.ages.length ? ` · ${AGE_ORDER.filter((a) => brief.ages.includes(a)).map((a) => AGE_NAME[a]).join(", ")}` : "";
  return {
    version: 1,
    id,
    name: `${VIBE_NAME[brief.vibe]} party · ${lengthLabel(brief.minutes)}`,
    description: `For ${GUEST_NAME[brief.guests]}${ages}`,
    created_ms: createdMs,
    updated_ms: NOW_MS,
    origin: { kind: "party", brief, provider_id: ai?.provider_id ?? "anthropic", model: ai?.model ?? "claude-sonnet-5-5", built_ms: NOW_MS },
    entries,
    unmatched: PARTY_MISSING,
    sources: [],
    search_widget_html: null,
  };
}

/** A saved or just built chat playlist (the title falls back to the wish, then to a plain name). */
function chatList(brief: ChatBrief, id: string, createdMs: number): HomePlaylist {
  const ai = sources().ai;
  const target = Math.max(1, Math.round((brief.minutes * MIN_PER_HOUR * 1000) / 240_000));
  const entries = Array.from({ length: Math.min(target, 30) }, (_, i) => {
    const [title, artist, album] = CHAT_SONGS[i % CHAT_SONGS.length];
    return entry(i + 200, title, album, "plain", "", { artist, duration_ms: 230_000 + ((i * 11_000) % 40_000), explicit: brief.allow_explicit && i % 9 === 4 });
  });
  const label = brief.title || brief.wish.slice(0, 40) || "Playlist with AI";
  return {
    version: 1,
    id,
    name: `${label} · ${lengthLabel(brief.minutes)}`,
    description: brief.wish,
    created_ms: createdMs,
    updated_ms: NOW_MS,
    origin: { kind: "chat", brief, provider_id: ai?.provider_id ?? "anthropic", model: ai?.model ?? "claude-sonnet-5-5", built_ms: NOW_MS },
    entries,
    unmatched: PARTY_MISSING,
    sources: [],
    search_widget_html: null,
  };
}

async function partyBuild({ replaceId, onProgress }: { replaceId: string | null; onProgress: { id: number } }): Promise<PartyBuilt> {
  const cancelled = new Promise<never>((_, reject) => (abort = () => reject("cancelled")));
  const run = async (): Promise<PartyBuilt> => {
    emit(onProgress, { kind: "asking", round: 0 });
    await wait(900);
    const total = chatBrief ? 30 : 40;
    for (let done = 1; done <= total; done++) {
      emit(onProgress, { kind: "resolving", done, total });
      if (url.has(chatBrief ? "chatbuilding" : "partybuilding") && done === 9) await new Promise(() => {});
      await wait(50);
    }
    const created = saved.find((p) => p.id === replaceId)?.created_ms ?? NOW_MS;
    if (chatBrief) return { playlist: chatList(chatBrief, replaceId ?? newId(), created), note: null };
    return { playlist: partyList(partyBrief ?? DEFAULT_BRIEF, replaceId ?? newId(), created), note: url.has("partyshort") ? "20 min short of 2 h: not enough of the suggested songs were found." : null };
  };
  try {
    return await Promise.race([run(), cancelled]);
  } finally {
    abort = null;
  }
}

const partyHandlers: Record<string, (args: any) => unknown> = {
  party_default_brief: (): PartyBrief => ({ guests: "friends", ages: [], vibe: "dance", minutes: settings.party_minutes, curve: settings.party_curve, allow_explicit: settings.party_allow_explicit, notes: "" }),
  party_open: ({ brief }: { brief: PartyBrief }) => {
    if (!sources().ai) throw "Party playlists are made by your AI. Set one up in Settings → AI.";
    partyBrief = brief;
    chatBrief = null;
    partyTurns = 0;
  },
  chat_default_brief: (): ChatBrief => ({ wish: "", title: "", minutes: settings.chat_minutes, curve: settings.chat_curve, allow_explicit: settings.chat_allow_explicit }),
  chat_song_wish: ({ seed }: { seed: SongSeed }) => {
    const detail = [seed.year, seed.genre].filter(Boolean).join(", ");
    return `A playlist like “${seed.title}” by ${seed.artist}${detail ? ` (${detail})` : ""}: same style, era and mood. Start with it.`;
  },
  chat_open: ({ brief }: { brief: ChatBrief }) => {
    if (!sources().ai) throw "Playlists with AI are made by your AI. Set one up in Settings → AI.";
    chatBrief = brief;
    partyBrief = null;
    partyTurns = 0;
  },
  party_set_brief: ({ brief }: { brief: PartyBrief }) => {
    partyBrief = brief;
  },
  party_start: async () => {
    partyTurns = 0;
    // the chat's first line is fixed words, no AI call: instant
    if (chatBrief) return CHAT_OPENER;
    await wait(PARTY_TURN_DELAY_MS);
    return PARTY_QUESTIONS[0];
  },
  party_reply: async ({ text }: { text: string }) => {
    await wait(PARTY_TURN_DELAY_MS);
    partyTurns++;
    if (chatBrief) {
      // the first message becomes the wish; the AI names the playlist and may change the facts
      chatBrief = { ...chatBrief, wish: chatBrief.wish || text, title: CHAT_TITLE, minutes: /30 min/i.test(text) ? 30 : /2 hours?/i.test(text) ? 120 : chatBrief.minutes, allow_explicit: /no explicit/i.test(text) ? false : chatBrief.allow_explicit };
      const done = partyTurns >= MAX_FREE_CHAT_TURNS;
      return {
        text: done ? CHAT_LIMIT_TURN.text : `Got it: ${CHAT_TITLE.toLowerCase()}. ${CHAT_FOLLOW_UPS[(partyTurns - 1) % CHAT_FOLLOW_UPS.length]}`,
        quick_replies: done ? [] : CHAT_SUGGESTIONS[(partyTurns - 1) % CHAT_SUGGESTIONS.length],
        chat_brief: chatBrief,
      };
    }
    return partyTurns >= MAX_CHAT_TURNS ? CHAT_LIMIT_TURN : PARTY_QUESTIONS[partyTurns % PARTY_QUESTIONS.length];
  },
  party_build: partyBuild,
  party_cancel: () => abort?.(),
  party_close: () => {
    partyBrief = null;
    chatBrief = null;
  },
};

let saved: HomePlaylist[] = [
  playlist({ artist: "Korn", shows: 8, thresholds: defaults.concert_thresholds, mode: { kind: "tour" }, popular_only: false }, false, "p1a2b3c"),
  partyList({ ...DEFAULT_BRIEF, ages: ["young_adults", "adults"] }, "p7a8b9c", NOW_MS - 2 * 86_400_000),
  chatList({ wish: "Something sad from the 90s, in English", title: CHAT_TITLE, minutes: 60, curve: "steady", allow_explicit: true }, "p9c8d7e", NOW_MS - 3_600_000),
  { ...playlist({ artist: "Deftones", shows: 8, thresholds: defaults.concert_thresholds, mode: { kind: "tour" }, popular_only: true }, true, "p4d5e6f", NOW_MS - 86_400_000), updated_ms: NOW_MS - 86_400_000 },
];

export const playlistsMock = {
  settings: () => ({ playlists: settings }),
  setSettings: (s: { playlists?: PlaylistSettings }) => {
    if (s.playlists) settings = clamp(s.playlists);
    return { playlists: settings };
  },
  handlers: {
    concert_sources: () => sources(),
    concert_build: ({ request, onProgress }: { request: ConcertRequest; onProgress: { id: number } }) => build(request, onProgress),
    concert_rearrange: async ({ playlist: p, request }: { playlist: HomePlaylist; request: ConcertRequest }) => {
      await wait(300);
      return { ...playlist(request, p.origin.kind === "concert" && p.origin.source.kind === "tidal_popular", p.id, p.created_ms), name: p.name };
    },
    concert_cancel: () => abort?.(),
    ...partyHandlers,
    home_playlists: () => [...saved],
    home_playlist_save: ({ playlist: p }: { playlist: HomePlaylist }) => {
      // like the core: a copy that doesn't know its TIDAL twin keeps the stored one
      const stored = { ...p, tidal_uuid: p.tidal_uuid ?? saved.find((x) => x.id === p.id)?.tidal_uuid ?? null };
      saved = [stored, ...saved.filter((x) => x.id !== p.id)];
      return stored;
    },
    home_playlist_save_to_tidal: async ({ id }: { id: string }) => {
      await wait(1200);
      const p = saved.find((x) => x.id === id);
      if (!p) throw "This playlist isn't in HomeFi anymore.";
      const created = !p.tidal_uuid;
      const stored = { ...p, tidal_uuid: p.tidal_uuid ?? `mock-${id}` };
      saved = saved.map((x) => (x.id === id ? stored : x));
      const skipped = p.unmatched.length ? [] : ["Some Jam · Korn"];
      const n = p.entries.length;
      const done = created ? `Saved ${n} songs to TIDAL` : `Updated on TIDAL: ${n} songs`;
      const summary = skipped.length ? `${done} · ${skipped.length} isn't on TIDAL` : done;
      return { playlist: stored, report: { uuid: stored.tidal_uuid, created, saved: n, skipped, summary } };
    },
    home_playlist_delete: ({ id }: { id: string }) => {
      const had = saved.some((x) => x.id === id);
      saved = saved.filter((x) => x.id !== id);
      return had;
    },
  } as Record<string, (args: any) => unknown>,
};

/** ?concert...: the screen builds this by itself (the flags are for screenshots). */
export const mockConcertAuto = ["concert", "concertbuilding", "concertpopular", "concertgemini"].some((f) => url.has(f));
