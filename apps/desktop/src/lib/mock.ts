// Development-only fake backend so the UI can be designed in a normal browser
// (`pnpm dev` and open http://localhost:1420). Never bundled in Tauri builds.
import type { GuestView, HubView, Member, QueueItem, ShareSettings, ShareState, ShareTrack, PlayerState, Track } from "./api";
import type { AiProviderId, AiSettings, AssistantTurn, EqProfile, EqUiSettings } from "./api";
import { eqMock } from "./mock-eq";
import { soundMock } from "./mock-sound";
import { METALLICA_ID, findVersion, metallicaCards } from "./mock-versions";
import type { AlbumVersionPrefs, Hit } from "./api";
import type { CrossfadeSettings, OutputOptions } from "./api";

const url = new URLSearchParams(location.search);

let account: { userId: string; countryCode: string } | null = url.has("signedout") ? null : { userId: "12345", countryCode: "US" };
let polls = 0;
// ?terms: the personal-use notice not accepted yet (signed in: the upgrade dialog; ?signedout too: the login gate)
let albumVersions: AlbumVersionPrefs = { group: !url.has("ungrouped"), explicit: "explicit", spatial: "stereo", edition: "original" };
// playback: crossfade (?crossfade turns it on) and surround output
let crossfadeMock: CrossfadeSettings = { mode: url.has("crossfade") ? "on" : "off", seconds: 6, curve: "equalPower", smart: true, skipAlbumRuns: true, onManualSkip: true, manualSeconds: 1.5, minTrackSeconds: 0, lowPowerFallback: true };
let outputMock: OutputOptions = { surround: "frontOnly", resampler: "high" };
let updateMock: import("./api").UpdateView = {
  current: "0.1.0",
  status: url.has("update") ? "available" : "upToDate",
  version: url.has("update") ? "1.2.0" : null,
  notes: "### Added\n\n- **Auto-update**: HomeFi offers new versions on every platform.\n- A `Check now` button in Settings → About.\n\n### Fixed\n\n- Guests on Windows get the reason when the hub stops.",
  date: Date.now() - 3_600_000,
  prerelease: false,
  pageUrl: "https://github.com/joseespana/homefi/releases/tag/v1.2.0",
  inPlace: url.get("update") !== "manual",
  installKind: url.get("update") === "manual" ? "manual" : "app",
  manualReason: url.get("update") === "manual" ? "translocated" : null,
  downloaded: 0,
  total: null,
  error: null,
  lastChecked: Date.now() - 1_800_000,
  afterSong: false,
  dismissed: false,
};
let terms = url.has("terms") ? { tidal_terms_accepted: 0, tidal_terms_accepted_at: null as number | null } : { tidal_terms_accepted: 1, tidal_terms_accepted_at: Date.now() - 86_400_000 as number | null };

const albums = [
  { album: "Discovery", artist: "Daft Punk", year: 2001, hue: 18, n: 6, q: "FLAC · 24-bit / 96 kHz", rate: 96000, bits: 24, genre: "Electronic, House" },
  { album: "Random Access Memories", artist: "Daft Punk", year: 2013, hue: 40, n: 4, q: "FLAC · 24-bit / 88.2 kHz", rate: 88200, bits: 24, genre: "Electronic, Disco" },
  { album: "In Rainbows", artist: "Radiohead", year: 2007, hue: 330, n: 5, q: "FLAC · 16-bit / 44.1 kHz", rate: 44100, bits: 16, genre: "Alternative, Rock" },
  { album: "Currents", artist: "Tame Impala", year: 2015, hue: 280, n: 4, q: "ALAC · 24-bit / 48 kHz", rate: 48000, bits: 24, genre: "Psychedelic, Pop" },
  { album: "Blonde", artist: "Frank Ocean", year: 2016, hue: 90, n: 3, q: "MP3 · 320 kbps", rate: 44100, bits: null, genre: "R&B" },
  { album: "Kind of Blue", artist: "Miles Davis", year: 1959, hue: 205, n: 5, q: "FLAC · 24-bit / 192 kHz", rate: 192000, bits: 24, genre: "Jazz" },
];
const titles = ["One More Time", "Aerodynamic", "Digital Love", "Harder, Better", "Something About Us", "Veridis Quo", "Get Lucky", "Instant Crush", "Touch", "Contact", "Reckoner", "Nude", "Weird Fishes", "Videotape", "15 Step", "Let It Happen", "The Less I Know", "Eventually", "Yes I'm Changing", "Nikes", "Ivy", "Pink + White", "So What", "Freddie Freeloader", "Blue in Green", "All Blues", "Flamenco Sketches"];

let k = 0;
const tracks: Track[] = albums.flatMap((a) =>
  Array.from({ length: a.n }, (_, i) => ({
    path: `/mock/${a.artist}/${a.album}/${i + 1}.flac#${a.hue}`,
    title: titles[k++ % titles.length],
    artist: a.artist,
    album: a.album,
    albumArtist: a.artist,
    trackNumber: i + 1,
    discNumber: 1,
    year: a.year,
    genre: a.genre,
    durationMs: 180_000 + ((k * 37_000) % 160_000),
    format: a.q.split(" ")[0],
    sampleRate: a.rate,
    bitDepth: a.bits,
    lossless: a.bits !== null,
    quality: a.q,
    isrc: null,
  })),
);
// ?big: a large library (10k songs, 500 albums) and long Tidal lists, to check the app stays fast
const big = url.has("big");
if (big) {
  const seed = tracks.slice();
  for (let i = tracks.length; i < 10_000; i++) {
    const t = seed[i % seed.length];
    const a = Math.floor(i / 20);
    tracks.push({ ...t, path: `/mock/big/${a}/${i}.flac#${(a * 37) % 360}`, title: `${t.title} ${i}`, album: `Album ${a}`, albumArtist: `Artist ${a % 300}`, artist: `Artist ${a % 300}`, isrc: `MOCK${i}` });
  }
}

function coverSvg(path: string): string {
  const hue = Number(path.split("#")[1] ?? 250);
  const svg = `<svg xmlns='http://www.w3.org/2000/svg' width='400' height='400'><defs><linearGradient id='g' x1='0' y1='0' x2='1' y2='1'><stop offset='0' stop-color='hsl(${hue} 85% 58%)'/><stop offset='1' stop-color='hsl(${(hue + 60) % 360} 70% 22%)'/></linearGradient></defs><rect width='400' height='400' fill='url(#g)'/><circle cx='200' cy='200' r='120' fill='none' stroke='white' stroke-opacity='.35' stroke-width='18'/><circle cx='200' cy='200' r='30' fill='white' fill-opacity='.5'/></svg>`;
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
}

let state: PlayerState = {
  status: url.has("guest") ? "paused" : "playing",
  queue: tracks.slice(0, big ? 700 : tracks.length).map((t) => t.path),
  current: 0,
  position_ms: 62_000,
  duration_ms: tracks[0].durationMs,
  volume: 0.72,
  shuffle: false,
  repeat: "off",
  preferred_device: "External Headphones",
  stream: { codec: "flac", source_rate: 96000, bits_per_sample: 24, source_channels: 2, output_rate: 96000, output_channels: 2, resampled: false, device: "External Headphones" },
  error: null,
  track: tracks[0],
  ...soundMock.playerFlags(tracks[0], eqMock.playerFlags("External Headphones")),
};
/** Equalizer and taste-layer fields of the player on `device` for the current track. */
const flagsFor = (device: string) => soundMock.playerFlags(state.track, eqMock.playerFlags(device));
// ?chain=: the signal chain's cases (bitperfect, resampled, usb, bluetooth, fading)
{
  const chain = url.get("chain");
  const s = state.stream!;
  if (chain === "bitperfect") state = { ...state, stream: { ...s, codec: "flac", bits_per_sample: 16, source_rate: 44100, output_rate: 44100, device: "AK USB DAC" }, eq_active: false, eq_profile: null, taste_rule: null, taste_label: null, loudness_db: 0 };
  if (chain === "resampled") state = { ...state, stream: { ...s, source_rate: 96000, output_rate: 48000, resampled: true, device: "MacBook Pro Speakers" } };
  if (chain === "bluetooth") state = { ...state, stream: { ...s, codec: "aac", bits_per_sample: null, source_rate: 44100, output_rate: 48000, resampled: true, device: "WH-1000XM5" } };
  if (chain === "fading") (state as PlayerState & { fading?: unknown }).fading = { from: tracks[0].path, to: tracks[1].path, progress: 0.42, seconds: 6 };
}
// ?surround: a stereo song on a 7.1 device (WASAPI shared mode at 48 kHz)
if (url.has("surround")) state = { ...state, preferred_device: "Speakers (7.1 Surround Sound)", stream: { ...state.stream!, source_rate: 44100, output_rate: 48000, output_channels: 8, resampled: true, surround: "frontOnly", resampler: "high", device: "Speakers (7.1 Surround Sound)" } };

function setCurrent(i: number) {
  const t = tracks[(i + tracks.length) % tracks.length];
  const rate = t.sampleRate ?? 44100;
  state = {
    ...state,
    current: tracks.indexOf(t),
    track: t,
    position_ms: 0,
    duration_ms: t.durationMs,
    stream: { ...state.stream!, codec: t.format.toLowerCase(), source_rate: rate, bits_per_sample: t.bitDepth, output_rate: rate > 96000 ? 96000 : rate, resampled: rate > 96000 },
  };
  state = { ...state, ...flagsFor(state.stream?.device ?? "WH-1000XM5") };
}

const t0 = performance.now();
function visual(bands: number) {
  const t = (performance.now() - t0) / 1000;
  // in someone's room the backend answers with the hub's levels
  const playing = state.status === "playing" || (guest.status === "connected" && room.playing);
  const beat = Math.pow(Math.max(0, Math.sin(t * Math.PI * 2 * 2)), 8);
  const spectrum = Array.from({ length: bands }, (_, i) => {
    if (!playing) return 0;
    const x = i / bands;
    const tilt = 0.85 - x * 0.45;
    const wob = 0.12 * Math.sin(t * 3 + i * 0.7) + 0.08 * Math.sin(t * 7.3 + i * 1.9);
    return Math.max(0, Math.min(1, tilt + wob + (i < 6 ? beat * 0.25 : 0) - Math.random() * 0.08));
  });
  const wave = Array.from({ length: 256 }, (_, i) =>
    playing ? 0.45 * Math.sin(i / 9 + t * 20) * (0.6 + 0.4 * Math.sin(i / 40 + t)) + 0.15 * Math.sin(i / 2.3 + t * 33) : 0,
  );
  const lvl = playing ? 0.45 + 0.3 * beat + 0.1 * Math.random() : 0;
  return { left: lvl, right: lvl * (0.9 + 0.1 * Math.random()), spectrum, wave };
}

// ---------------------------------------------------------------- shared control
// ?hub: this computer is already sharing; ?guest: already in someone's room
// (?guest&party: playing along here); ?nearby: sessions found on the network.
const shareSettings: ShareSettings = {
  session_name: "", name: "", color: "#0891B2", fixed_code: false, code: "", port: 47800, party_default: false,
  add: "free", max_per_guest: 3, order: "turns", skip: "vote", playback: url.has("playback"), edit: "all", allow_explicit: true,
  drop_songs_on_leave: false, notice_joined: true, notice_left: true, notice_added: true, notice_approval: true, notice_seconds: 4,
  discover: true, discover_everywhere: true, announce_lan: true, play_along: true, speaker_delay_ms: 120, sync_by_speed: true,
  preload_songs: true, guest_opens_player: true, party_start_wait_sec: 3, send_levels: true,
};
const people: Member[] = [
  { guest_id: "hub", name: "Ana", color: "#EC4899" },
  { guest_id: "g1", name: "Luis", color: "#F59E0B" },
  { guest_id: "g2", name: "You", color: "#0891B2" },
  { guest_id: "g3", name: "Marta", color: "#10B981" },
];
let nextId = 1;
const song = (title: string, artist: string, album: string, hue: number, by: Member, ms = 215_000): QueueItem => ({
  queue_id: nextId++, source: `tidal:${1000 + nextId}`, title, artist, album, duration_ms: ms,
  cover_url: coverSvg(`x#${hue}`), explicit: false, added_by: by,
});
const permissions = { add: "free", max_per_guest: 3, order: "turns", skip: "vote", playback: url.has("playback"), edit: "own", allow_explicit: true, drop_songs_on_leave: false } as const;
let room: ShareState = {
  now_playing: song("Midnight City", "M83", "Hurry Up, We're Dreaming", 260, people[0], 243_000),
  position_ms: 71_000, clock_ms: 0, playing: true, audio: null, prefetch: [],
  queue: [
    song("Electric Feel", "MGMT", "Oracular Spectacular", 120, people[1]),
    song("Dreams", "Fleetwood Mac", "Rumours", 30, people[2], 257_000),
    song("Breathe", "Télépopmusik", "Genetic World", 190, people[3]),
    song("Kids", "MGMT", "Oracular Spectacular", 140, people[0]),
  ],
  guests: people.slice(1), permissions: { ...permissions }, skip_votes: 1, skip_votes_needed: 2, volume: 0.7,
};
if (url.has("party")) room.audio = { queue_id: room.now_playing!.queue_id, url: "/audio/1?k=x", mime: "audio/flac", size: 1 };
const idleGuest: GuestView = { status: "idle", session_name: "", guest_id: "", host: "", port: 47800, state: null, position_ms: 0, error: null, play_along: true, playing_here: false, round_trip_ms: null, stream: null };
let guest: GuestView = url.has("guest") ? inRoom() : idleGuest;
function inRoom(): GuestView {
  const party = !!room.audio;
  return { status: "connected", session_name: "Ana’s music", guest_id: "g2", host: "192.168.1.24", port: 47800, state: room, position_ms: room.position_ms, error: null, play_along: true, playing_here: party, round_trip_ms: party ? 6 : 8, stream: party ? state.stream : null };
}
const idleHub: HubView = { running: false, starting: false, party: false, play_here: true, info: null, guests: [], pending: [], state: null, error: null };
function runningHub(party: boolean): HubView {
  return {
    running: true, starting: false, party, play_here: true,
    info: { host: "192.168.1.31", port: 47800, code: "4821", session_name: "José’s music", join_uri: "homefi://join?host=192.168.1.31&port=47800&code=4821&name=Jos%C3%A9%E2%80%99s+music" },
    guests: [people[1], people[3], { guest_id: "g4", name: "Pixel 8", color: "#8B5CF6" }],
    pending: [song("Bohemian Rhapsody", "Queen", "A Night at the Opera", 0, people[3], 354_000)],
    state: room, error: null,
  };
}
let hub: HubView = url.has("hub") ? runningHub(url.has("party")) : idleHub;
const nearby = url.has("nearby") ? [{ name: "Ana’s music", host: "192.168.1.24", port: 47800, party: true }, { name: "Car", host: "192.168.1.40", port: 47800, party: false }] : [];
const tidalTrack = (title: string, artist: string, album: string, hue: number): ShareTrack => ({ source: `tidal:${hue}`, title, artist, album, duration_ms: 200_000 + hue * 100, cover_url: coverSvg(`x#${hue}`), explicit: hue % 3 === 0 });
function emit(event: string, payload: unknown) {
  for (const id of listeners.get(event) ?? []) w[`_${id}`]?.({ event, id: 0, payload });
}
const roomChanged = () => {
  if (guest.status === "connected") emit("share-guest", (guest = inRoom()));
};

// ---------------------------------------------------------------- AI
// ?aikey: Anthropic already has a key; ?noai: no provider chosen (guided wizard);
// ?aierror: the assistant's provider fails (rate limit); a key "bad..." fails the test.
let aiSettings: AiSettings = url.has("noai") ? { provider: "", model: "", base_url: "" } : { provider: "anthropic", model: "", base_url: "" };
const aiKeys = new Set<AiProviderId>(url.has("aikey") ? ["anthropic"] : []);
const aiProviders = [
  ["anthropic", "Anthropic (Claude)", "https://platform.claude.com/settings/keys", true, "https://api.anthropic.com/v1", false, "claude-sonnet-5-5", "recommended", "Most reliable; default Claude Sonnet 5.5"],
  ["gemini", "Google Gemini", "https://aistudio.google.com/apikey", true, "https://generativelanguage.googleapis.com/v1beta", false, null, "cheapest", "Very cheap and fast — pick a Flash model"],
  ["openai", "OpenAI (ChatGPT, Codex)", "https://platform.openai.com/api-keys", true, "https://api.openai.com/v1", false, null, null, "Good and cheap — a mini model"],
  ["openrouter", "OpenRouter", "https://openrouter.ai/settings/keys", true, "https://openrouter.ai/api/v1", false, null, "one_key_for_all", "One key for everyone's models"],
  ["ollama", "Ollama", "https://ollama.com/download", false, "http://localhost:11434/v1", true, null, "private_local", "Free and private, but small models make more mistakes — advanced"],
  ["mistral", "Mistral", "https://console.mistral.ai/api-keys", true, "https://api.mistral.ai/v1", false, null, null, "European provider"],
  ["groq", "Groq", "https://console.groq.com/keys", true, "https://api.groq.com/openai/v1", false, null, null, "Open models, very fast"],
  ["xai", "xAI (Grok)", "https://console.x.ai", true, "https://api.x.ai/v1", false, null, null, "The Grok models"],
  ["deepseek", "DeepSeek", "https://platform.deepseek.com/api_keys", true, "https://api.deepseek.com", false, null, null, "Low cost"],
  ["openai_compatible", "OpenAI-compatible", "", false, "", true, null, null, "Your own server (LM Studio, vLLM...) — advanced"],
].map(([id, name, key_url, needs_key, default_base_url, custom_base_url, default_model, badge, hint], i) => ({ id, name, key_url, needs_key, default_base_url, custom_base_url, default_model, rank: i + 1, badge, hint }));
// [id, recommended, small, economical]
const aiModels: Record<string, [string, boolean, boolean, boolean][]> = {
  anthropic: [["claude-sonnet-5-5", true, false, false], ["claude-opus-5-5", false, false, false], ["claude-haiku-5-5", false, false, true]],
  openai: [["gpt-5.6-mini", true, false, false], ["gpt-5.6", false, false, false], ["gpt-5.6-nano", false, true, false]],
  gemini: [["gemini-3.8-flash", true, false, false], ["gemini-3.8-flash-lite", false, true, false], ["gemini-3.8-pro", false, false, false]],
  ollama: [["llama3.3:70b", true, false, false], ["qwen3:8b", false, true, false]],
};
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
const mockProfile = (bass: number): EqProfile => ({
  name: "Sennheiser HD 650",
  preampDb: -(6.4 + bass),
  bands: [
    { kind: "lowShelf", freqHz: 105, gainDb: 6.4 + bass, q: 0.7, enabled: true },
    { kind: "peak", freqHz: 118, gainDb: -3.1, q: 0.5, enabled: true },
    { kind: "peak", freqHz: 3169, gainDb: -1.7, q: 3.89, enabled: true },
    { kind: "peak", freqHz: 8800, gainDb: 5.1, q: 1.42, enabled: true },
    { kind: "highShelf", freqHz: 10000, gainDb: -2.1, q: 0.7, enabled: true },
  ],
  source: { kind: "assistant" },
});
// a scripted conversation, the same for the AI and the guided wizard
let eqPresets: EqProfile[] = [];
let eqUi: EqUiSettings = { deck_button: true, ab_mode: "hold", band_labels: true, taste_sliders: true };
let assistantStep = 0;
let assistantEs = true;
function assistantTurn(text: string): AssistantTurn {
  const say = (en: string, es: string) => (assistantEs ? es : en);
  const yes = /^(s[ií]|yes|ok|aplicar|apply|dale|claro)/i.test(text.trim());
  const step = assistantStep++;
  const turn: AssistantTurn = { text: "", quick_replies: [], proposal: null, confirmed: false, done: false };
  switch (step) {
    case 0:
      return { ...turn, text: say("Hi! I'll help you tune the sound. What will you listen with?", "¡Hola! Te ayudo a ajustar el sonido. ¿Con qué vas a escuchar?"), quick_replies: ["External Headphones", "WH-1000XM5", "MacBook Air Speakers"] };
    case 1:
      return { ...turn, text: say("Which headphones are they? Brand and model is enough.", "¿Qué audífonos son? Con la marca y el modelo basta."), quick_replies: [say("I don't know", "No sé")] };
    case 2:
      return { ...turn, text: say("Found them. How do you like the sound?", "Los encontré. ¿Cómo te gusta el sonido?"), quick_replies: [say("Neutral", "Neutral"), say("More bass", "Más bajos"), say("More clarity", "Más claridad"), say("Less harsh", "Menos áspero")] };
    case 3: {
      const explanation = say(
        "It corrects your HD 650 from their measurements and adds a little bass. With the equalizer on, the sound is processed (no longer bit-perfect).",
        "Corrige tus HD 650 con sus mediciones y suma un poco de bajo. Con el ecualizador encendido el sonido se procesa (ya no es bit-perfect).",
      );
      return { ...turn, text: `${explanation}\n\n${say("Apply it?", "¿Lo aplico?")}`, quick_replies: [say("Apply", "Aplicar"), say("Cancel", "Cancelar")], proposal: { output_id: "External Headphones", profile: mockProfile(3), explanation } };
    }
    default:
      return yes
        ? { ...turn, text: say("Done. Your equalizer is on.", "Listo. Tu ecualizador está activado."), proposal: { output_id: "External Headphones", profile: mockProfile(3), explanation: "" }, confirmed: true, done: true }
        : { ...turn, text: say("OK, nothing changed.", "Va, no cambié nada."), done: true };
  }
}

const handlers: Record<string, (args: any) => unknown> = {
  ...eqMock.handlers,
  ...soundMock.handlers(
    () => state.track,
    () => {
      state = { ...state, ...flagsFor(state.stream?.device ?? "WH-1000XM5") };
      emit("player", state);
      emit("settings", handlers.get_settings({}));
    },
  ),
  eq_bypass: ({ on }) => {
    const eq = eqMock.settings();
    const flags = flagsFor(state.stream?.device ?? "WH-1000XM5");
    state = { ...state, ...(on && (eq.enabled || flags.eq_active) ? { eq_active: false, eq_profile: null, taste_rule: null, taste_label: null } : flags) };
    emit("player", state);
  },
  eq_set: ({ settings }) => {
    const saved = eqMock.set(settings);
    state = { ...state, ...flagsFor(state.stream?.device ?? "WH-1000XM5") };
    emit("settings", handlers.get_settings({}));
    return saved;
  },
  ai_providers: () => aiProviders,
  ai_key_status: () => Object.fromEntries(aiProviders.map((p) => [p.id, aiKeys.has(p.id as AiProviderId)])),
  ai_key_set: ({ provider }) => (aiKeys.add(provider), null),
  ai_key_clear: ({ provider }) => (aiKeys.delete(provider), null),
  ai_list_models: async ({ provider }) => {
    await wait(500);
    if (provider !== "ollama" && provider !== "openai_compatible" && !aiKeys.has(provider)) throw `There's no ${provider} key yet. Add it in Settings.`;
    const list = aiModels[provider] ?? [["model-a", true, false, false], ["model-b", false, false, false]];
    return list.map(([id, recommended, small, economical]) => ({ id, name: id, recommended, small, economical }));
  },
  ai_test_key: async ({ provider, key }) => {
    await wait(600);
    if (typeof key === "string" && key.startsWith("bad")) throw `The ${provider} key isn't valid. Check it in Settings.`;
  },
  ai_default_model: ({ provider }) => (aiModels[provider]?.find((m) => m[1]) ?? ["model-a"])[0],
  eq_assistant_start: async ({ language, focusRule, guided }) => {
    assistantStep = 0;
    assistantEs = String(language).toLowerCase().startsWith("es");
    await wait(700);
    const refine = focusRule && !guided ? soundMock.refine(focusRule, assistantEs) : null;
    return refine ?? assistantTurn("");
  },
  eq_assistant_reply: async ({ text }) => {
    await wait(900);
    if (url.has("aierror")) throw assistantEs ? "Anthropic pidió ir más despacio. Espera un minuto y vuelve a intentarlo." : "Anthropic asked us to slow down. Wait a minute and try again.";
    const suggested = soundMock.reply(text);
    if (suggested) {
      state = { ...state, ...flagsFor(state.stream?.device ?? "WH-1000XM5") };
      emit("settings", handlers.get_settings({}));
      return suggested;
    }
    const turn = assistantTurn(text);
    // like ai.rs: a confirmed proposal is saved and the equalizer turned on
    if (turn.confirmed && turn.proposal) handlers.eq_set({ settings: { ...eqMock.settings(), enabled: true, autoPreamp: true, perOutput: { ...eqMock.settings().perOutput, [turn.proposal.output_id]: turn.proposal.profile } } });
    return turn;
  },
  eq_assistant_close: () => null,
  share_status: () => ({ hub, guest, nearby, name: "José’s MacBook", session_name: "José’s MacBook’s music" }),
  share_start: ({ party }) => (emit("share-hub", (hub = runningHub(party))), hub),
  share_stop: () => emit("share-hub", (hub = idleHub)),
  share_set_party: ({ on }) => emit("share-hub", (hub = { ...hub, party: on })),
  share_set_play_here: ({ on }) => emit("share-hub", (hub = { ...hub, play_here: on })),
  share_kick: ({ guestId }) => (emit("share-hub", (hub = { ...hub, guests: hub.guests.filter((g) => g.guest_id !== guestId) })), true),
  share_approve: ({ queueId }) => (emit("share-hub", (hub = { ...hub, pending: hub.pending.filter((p) => p.queue_id !== queueId) })), true),
  share_reject: ({ queueId }) => (emit("share-hub", (hub = { ...hub, pending: hub.pending.filter((p) => p.queue_id !== queueId) })), true),
  share_join: async ({ code }) => {
    await new Promise((r) => setTimeout(r, 300));
    if (code && code !== "4821" && code !== "0000") throw "Wrong join code";
    guest = inRoom();
    emit("share-guest", guest);
    return guest;
  },
  share_leave: () => emit("share-guest", (guest = idleGuest)),
  share_play_along: ({ on }) => emit("share-guest", (guest = { ...guest, play_along: on, playing_here: on && !!room.audio })),
  share_probe: ({ host, port }) => ({ name: "Ana’s music", host, port, party: true }),
  share_parse_link: ({ link }) => {
    const u = new URL(link.replace("homefi://", "http://"));
    return { host: u.searchParams.get("host") ?? "", port: Number(u.searchParams.get("port") ?? 47800), code: u.searchParams.get("code") ?? "", session_name: u.searchParams.get("name") ?? "" };
  },
  share_guest: ({ message }) => {
    const q = room.queue;
    switch (message.type) {
      case "search":
        setTimeout(() => emit("share-results", {
          q: message.q,
          tracks: [tidalTrack("Something About Us", "Daft Punk", "Discovery", 18), tidalTrack("Digital Love", "Daft Punk", "Discovery", 30), tidalTrack("Get Lucky", "Daft Punk", "Random Access Memories", 40)],
          albums: [{ kind: "album", id: "1", title: "Discovery", subtitle: "Daft Punk · 2001", cover_url: coverSvg("x#18") }, { kind: "album", id: "2", title: "Random Access Memories", subtitle: "Daft Punk · 2013", cover_url: coverSvg("x#40") }],
          artists: [{ kind: "artist", id: "3", title: "Daft Punk", subtitle: "Artist", cover_url: coverSvg("x#300") }],
          playlists: [],
        }), 250);
        break;
      case "browse":
        setTimeout(() => emit("share-page", { header: { kind: message.kind, id: message.id, title: "Discovery", subtitle: "Daft Punk · 2001", cover_url: coverSvg("x#18") }, tracks: titles.slice(0, 8).map((t, i) => tidalTrack(t, "Daft Punk", "Discovery", 18 + i)) }), 250);
        break;
      case "add":
        room = { ...room, queue: [...q, { ...message.item, queue_id: nextId++, added_by: people[2] }] };
        break;
      case "remove":
        room = { ...room, queue: q.filter((i) => i.queue_id !== message.queue_id) };
        break;
      case "move": {
        const list = [...q];
        const from = list.findIndex((i) => i.queue_id === message.queue_id);
        list.splice(message.to, 0, ...list.splice(from, 1));
        room = { ...room, queue: list };
        break;
      }
      case "play":
      case "skip":
      case "vote_skip": {
        const pick = message.type === "play" ? q.find((i) => i.queue_id === message.queue_id) : q[0];
        if (message.type === "vote_skip" && room.skip_votes + 1 < room.skip_votes_needed) {
          room = { ...room, skip_votes: room.skip_votes + 1 };
          break;
        }
        if (pick) room = { ...room, now_playing: pick, position_ms: 0, skip_votes: 0, queue: q.filter((i) => i !== pick) };
        break;
      }
      case "toggle":
        room = { ...room, playing: !room.playing };
        break;
      case "volume":
        room = { ...room, volume: message.value };
        break;
    }
    roomChanged();
    return null;
  },
  app_info: () => ({ version: "0.1.0", appDir: "~/.homefi", ffmpeg: false, platform: "macos" }),
  // ?update: a new version is out (?update=manual: one that can't install itself)
  update_status: () => updateMock,
  update_check: async () => {
    await new Promise((r) => setTimeout(r, 600));
    updateMock = { ...updateMock, status: url.has("update") ? "available" : "upToDate", dismissed: false, lastChecked: Date.now() };
    emit("update", updateMock);
    return updateMock;
  },
  update_download: async () => {
    const total = 18_400_000;
    for (let got = 0; got <= total; got += 2_300_000) {
      updateMock = { ...updateMock, status: "downloading", downloaded: got, total };
      emit("update", updateMock);
      await new Promise((r) => setTimeout(r, 250));
    }
    updateMock = { ...updateMock, status: "ready", downloaded: total, total };
    emit("update", updateMock);
  },
  update_restart: ({ when }: { when: string }) => {
    updateMock = when === "song_end" && state.status === "playing" ? { ...updateMock, afterSong: true } : { ...updateMock, status: "installing" };
    emit("update", updateMock);
    return null;
  },
  update_dismiss: () => ((updateMock = { ...updateMock, dismissed: true, afterSong: false }), emit("update", updateMock), null),
  // ?signedout starts without a Tidal account; the fake sign-in approves on the second poll
  account: () => account,
  start_login: () => ((polls = 0), { deviceCode: "mock", userCode: "HFI-42X", url: "https://link.tidal.com/HFI42X", expiresIn: 300, interval: 1 }),
  poll_login: () => (++polls >= 2 ? ((account = { userId: "12345", countryCode: "US" }), "success") : "pending"),
  logout: () => ((account = null), null),
  library: () => tracks,
  queue_add: () => null,
  queue_tidal: ({ items }) => items.length,
  local_playlists: () => [
    { name: "Road trip", path: "~/Music/m3u/Road trip.m3u", tracks: tracks.slice(0, 6).map((t) => t.path).concat("~/Music/gone.flac") },
    { name: "Late night", path: "~/Music/m3u/Late night.m3u", tracks: tracks.slice(8, 14).map((t) => t.path) },
  ],
  rescan_library: () => tracks,
  player_state: () => state,
  play_tracks: ({ index }) => (setCurrent(index), (state = { ...state, status: "playing" })),
  player_toggle: () => (state = { ...state, status: state.status === "playing" ? "paused" : "playing" }),
  player_next: () => setCurrent((state.current ?? 0) + 1),
  player_previous: () => setCurrent((state.current ?? 0) - 1),
  player_jump: ({ index }) => setCurrent(index),
  player_seek: ({ ms }) => (state = { ...state, position_ms: ms }),
  player_shuffle: ({ on }) => (state = { ...state, shuffle: on }),
  player_repeat: ({ mode }) => (state = { ...state, repeat: mode }),
  player_volume: ({ volume }) => (state = { ...state, volume }),
  visual: ({ bands }) => visual(bands),
  output_devices: () => [
    { name: "External Headphones", is_default: false, sample_rates: [44100, 48000, 88200, 96000], default_rate: 48000, max_channels: 2 },
    { name: "WH-1000XM5", is_default: true, sample_rates: [44100], default_rate: 44100, max_channels: 2 },
    { name: "Razer Seiren V2 X", is_default: false, sample_rates: [44100, 48000], default_rate: 48000, max_channels: 2 },
    { name: "MacBook Air Speakers", is_default: false, sample_rates: [44100, 48000, 88200, 96000], default_rate: 48000, max_channels: 2 },
    { name: "Speakers (7.1 Surround Sound)", is_default: false, sample_rates: [48000], default_rate: 48000, max_channels: 8 },
  ],
  set_output: ({ name }) => (state = { ...state, preferred_device: name, stream: state.stream && { ...state.stream, device: name ?? "WH-1000XM5" }, ...flagsFor(name ?? "WH-1000XM5") }),
  // ?layout=<player layout> picks the room's look
  // ?style=<mood>, ?dance=<famous dance> and ?turns=off|dances|all pick the mascot's moves
  crossfade_get: () => crossfadeMock,
  crossfade_set: ({ crossfade }: { crossfade: CrossfadeSettings }) => ((crossfadeMock = { ...crossfade, seconds: Math.min(12, Math.max(1, crossfade.seconds)) }), crossfadeMock),
  output_options_get: () => outputMock,
  output_options_set: ({ options }: { options: OutputOptions }) => ((outputMock = options), outputMock),
  get_settings: () => ({ download_dir: "~/Music/homefi", library_dirs: ["~/Music"], quality: "max", embed_cover: true, embed_lyrics: false, skip_existing: true, volume: 0.72, output_device: null, room: { signal_chain: url.get("signal") !== "off", signal_style: (url.get("signal") === "detailed" ? "detailed" : "compact") as "compact", signal_animate: !url.has("still"), vu_meters: true, visualizer: true, halo: true, mascot: true, mascot_style: url.get("style") ?? "auto", mascot_dance: url.get("dance") ?? "auto", mascot_specials: "sometimes", mascot_limbs: "white", mascot_turns: (url.get("turns") ?? "all") as "all", mascot_size: Number(url.get("size") ?? 0), mascot_free: new URLSearchParams(location.search).has("float"), mascot_x: 0.85, mascot_y: 0.75, mascot_everywhere: false, mascot_right_click: true, mascot_temper: "normal", album_colors: true, layout: (url.get("layout") ?? "classic") as "classic", layout_everywhere: !url.has("roomonly"), immersive_shade: (url.get("shade") ?? "auto") as "auto", lyrics_mode: (url.get("lyrics") === "karaoke" ? "karaoke" : "normal") as "normal", lyrics_size: (url.get("lyricsize") ?? "medium") as "medium", lyrics_auto_open: url.has("lyrics"), lyrics_mascot: !url.has("nosinger"), lyrics_offset_ms: 0, motion: (url.get("motion") ?? "auto") as "auto" }, theme: (new URLSearchParams(location.search).get("theme") ?? "system") as "system", recent_folders: [], progress_style: "wavy", progress_thickness: "medium", progress_animated: true, lazy_lists: !url.has("whole"), share: shareSettings, ai: aiSettings, eq: eqMock.settings(), eq_presets: eqPresets, eq_ui: eqUi, album_versions: albumVersions, crossfade: crossfadeMock, output: outputMock, updates: { auto_check: true, prereleases: false, install: "ask" as const }, ...soundMock.settings(), ...terms }),
  set_settings: ({ settings }) => (settings.eq && eqMock.set(settings.eq), soundMock.setSettings(settings), (state = { ...state, ...flagsFor(state.stream?.device ?? "WH-1000XM5") }), (aiSettings = settings.ai ?? aiSettings), (eqPresets = settings.eq_presets ?? eqPresets), (eqUi = settings.eq_ui ?? eqUi), (albumVersions = settings.album_versions ?? albumVersions), (crossfadeMock = settings.crossfade ?? crossfadeMock), (outputMock = settings.output ?? outputMock), (terms = { tidal_terms_accepted: settings.tidal_terms_accepted, tidal_terms_accepted_at: settings.tidal_terms_accepted_at }), emit("settings", settings), settings),
  // ?nolyrics: none (an instrumental); ?plainlyrics: not synced; ?lyricserror: Tidal fails; ?lyricswait: never answers
  lyrics: async () => {
    if (url.has("lyricswait")) await new Promise(() => {});
    await new Promise((r) => setTimeout(r, 250));
    if (url.has("lyricserror")) throw "Tidal isn't answering right now. Try again in a moment.";
    if (url.has("nolyrics")) return null;
    return mockLyrics(url.has("plainlyrics"));
  },
  "plugin:window|set_theme": () => null,
  mini_player: () => null,
  mini_ready: () => null,
  prune_library: () => [],
  tidal_favorite_ids: () => ({ track: ["2"], album: [], artist: [], playlist: ["1"] }),
  tidal_favorite: () => null,
  tidal_my_playlists: () => [],
  tidal_tracks_by_isrc: () => ["99"],

  player_move: ({ from, to }) => {
    const q = [...state.queue];
    q.splice(to, 0, ...q.splice(from, 1));
    state = { ...state, queue: q };
  },
  tracks_for: ({ paths }) => paths.map(() => null),
  play_tidal: () => null,
  // ?upcoming: a Tidal stream with songs still to fetch
  stream_upcoming: () => (url.has("upcoming") ? ["Sahardnieh", "L'Aberek", "Fabienk", "Mata Zyklek"].map((title, i) => ({ id: String(900 + i), title, artist: "Angine de Poitrine", duration_ms: 240000 })) : []),
  // Tidal lists come in pages like the real ones (?failpage makes the third page fail once)
  tidal_library_page: ({ kind, offset, rest }) => mockPage(libraryItems(kind), offset, rest, 50),
  tidal_detail: ({ kind, id, lazy }) => {
    // Metallica: an artist page with albums Tidal lists two to five times
    if (kind === "artist" && id === METALLICA_ID) {
      const header = { kind, id, title: "Metallica", subtitle: "Artist", artist: "Metallica", album: "", cover_url: coverSvg("x#4"), explicit: false, quality: null, isrc: null, duration_ms: null };
      return { header, owned: false, description: null, albums: mockCards(), tracks: [], total: 0, duration_ms: null, next_offset: null };
    }
    const found = findVersion(id);
    const all = detailTracks(id);
    const page = mockPage(all, 0, !lazy, 100);
    const header = found
      ? { ...found.card, id, title: found.version?.title ?? found.card.title, explicit: found.version?.explicit ?? found.card.explicit, quality: found.version?.quality ?? found.card.quality, cover_url: mockCover(found.card), versions: undefined }
      : { kind, id, title: id === "1" ? "Every song, ever" : `Mock ${kind} ${id}`, subtitle: `${all.length} tracks`, artist: "", album: "", cover_url: coverSvg(`x#${(Number(id) * 47) % 360}`), explicit: false, quality: null, isrc: null, duration_ms: null };
    return page.then((p) => ({ header, owned: kind === "playlist" && id === "1", description: "A long mocked list to try loading as you scroll.", albums: [], tracks: p.items, total: p.total, duration_ms: all.length * 215_000, next_offset: p.next_offset }));
  },
  tidal_detail_page: ({ id, offset, rest }) => mockPage(detailTracks(id), offset, rest, 100),
  tidal_library_stream: ({ kind, offset, load, onPage }) => mockStream(libraryItems(kind), offset, 50, load, onPage),
  tidal_detail_stream: ({ id, offset, load, onPage }) => mockStream(detailTracks(id), offset, 100, load, onPage),
  tidal_cancel: ({ load }) => void cancelled.add(load),
  // any search finds Metallica (the artist and their albums, one card each)
  search: () => ({
    top_hit: null,
    tracks: [],
    albums: mockCards().slice(0, 8),
    artists: [{ kind: "artist", id: METALLICA_ID, title: "Metallica", subtitle: "Artist", artist: "Metallica", album: "", cover_url: coverSvg("x#4"), explicit: false, quality: null, isrc: null, duration_ms: null }],
    playlists: [],
    videos: [],
  }),
  tidal_album_versions: async ({ id }) => {
    await new Promise((r) => setTimeout(r, 300));
    const found = findVersion(id);
    return found?.card.versions?.map((v) => ({ ...v, cover_url: mockCover(found.card) })) ?? [];
  },
};

// ---------------------------------------------------------------- album versions
// every release of an album shares its cover (as on Tidal)
const mockCover = (card: Hit) => coverSvg(`x#${(metallicaCards.indexOf(card) * 53) % 360}`);
/** Metallica's cards as the settings say: grouped, or every release on its own. */
function mockCards(): Hit[] {
  const withCovers = metallicaCards.map((c) => ({ ...c, cover_url: mockCover(c), versions: c.versions?.map((v) => ({ ...v, cover_url: mockCover(c) })) }));
  if (albumVersions.group) return withCovers;
  return withCovers.flatMap((c) =>
    c.versions?.length ? c.versions.map((v) => ({ ...c, id: v.id, title: v.title, explicit: v.explicit, quality: v.quality, versions: undefined })) : [c],
  );
}

// ---------------------------------------------------------------- lyrics
// made-up words for the mock's songs, synced like Musixmatch's LRC (with an instrumental break)
const verses = [
  "Turn the lights down low tonight",
  "The record spins, the room is bright",
  "Every speaker hums our tune",
  "We dance beneath a paper moon",
  "",
  "One more time, we feel the sound",
  "One more time, our feet leave the ground",
  "Hold the note and don't let go",
  "Let the needle steal the show",
  "",
  "Static whispers in the dark",
  "A melody, a tiny spark",
  "Count the beats from one to four",
  "Then we sing it all once more",
];
function mockLyrics(plain: boolean) {
  const lines: { time_ms: number | null; text: string }[] = [];
  let t = 9_000;
  for (let round = 0; round < 3; round++) {
    for (const text of verses) {
      lines.push({ time_ms: plain ? null : t, text });
      t += text ? 3_600 : 2_000;
    }
    // an instrumental break between the rounds
    t += round === 0 ? 9_000 : 1_500;
  }
  return { lines, synced: !plain, provider: "MUSIXMATCH" };
}

// ---------------------------------------------------------------- Tidal lists
const mockHit = (kind: string, i: number, extra: Record<string, unknown> = {}) => ({
  kind,
  id: String(i + 1),
  title: kind === "track" ? `${titles[i % titles.length]} ${Math.floor(i / titles.length) || ""}`.trim() : `${albums[i % albums.length].album}${i >= albums.length ? ` ${Math.floor(i / albums.length) + 1}` : ""}`,
  subtitle: albums[i % albums.length].artist,
  artist: albums[i % albums.length].artist,
  album: albums[i % albums.length].album,
  cover_url: coverSvg(`x#${(i * 37) % 360}`),
  explicit: i % 7 === 0,
  quality: i % 2 ? "Lossless" : "Hi-Res",
  isrc: null,
  duration_ms: kind === "track" ? 150_000 + ((i * 13_000) % 180_000) : null,
  ...extra,
});
const libraryCounts: Record<string, number> = big ? { playlists: 300, albums: 500, artists: 400, tracks: 2000 } : { playlists: 64, albums: 140, artists: 37, tracks: 230 };
function libraryItems(kind: string) {
  const one = kind === "playlists" ? "playlist" : kind.slice(0, -1);
  // a few favorite albums Tidal lists several times
  const metallica = kind === "albums" ? mockCards().filter((c) => ["186460967", "441771365", "186460992"].includes(c.id)) : [];
  return (metallica as ReturnType<typeof mockHit>[]).concat(Array.from({ length: libraryCounts[kind] ?? 0 }, (_, i) =>
    mockHit(one, i, one === "playlist" ? { title: i === 0 ? "Every song, ever" : `Playlist ${i + 1}`, subtitle: `${i % 2 ? "Following" : "Yours"} · ${i === 0 ? 437 : 12 + i} tracks` } : {}),
  ));
}
// playlist "1" is long (437 tracks), the rest are album-sized
const detailTracks = (id: string) =>
  Array.from({ length: id === "1" ? (big ? 2000 : 437) : 9 + (Number(id) % 5) }, (_, i) => mockHit("track", i + Number(id) * 1000, big && i % 3 === 0 ? { isrc: `MOCK${(i * 7) % 10_000}` } : {}));
const cancelled = new Set<number>();
/** Pages to a Tauri channel, one at a time like the Rust side; returns how many were sent. */
async function mockStream<T>(all: T[], offset: number, limit: number, load: number, channel: { id: number }) {
  let sent = 0;
  for (let at = offset; at < all.length && !cancelled.has(load); at += limit) {
    await new Promise((r) => setTimeout(r, 120));
    if (cancelled.has(load)) break;
    const end = Math.min(all.length, at + limit);
    (window as any)[`_${channel.id}`]?.({ index: sent++, message: { items: all.slice(at, end), total: all.length, next_offset: end < all.length ? end : null } });
  }
  return sent;
}
let pageFailed = false;
async function mockPage<T>(all: T[], offset: number, rest: boolean, limit: number) {
  await new Promise((r) => setTimeout(r, rest ? 700 : 350));
  if (url.has("failpage") && offset === limit * 2 && !pageFailed) {
    pageFailed = true;
    throw "mock: the page didn't come";
  }
  const end = rest ? all.length : Math.min(all.length, offset + limit);
  return { items: all.slice(offset, end), total: all.length, next_offset: end < all.length ? end : null };
}

let callbacks = 0;
const listeners = new Map<string, number[]>();
const w = window as any;
w.__TAURI_INTERNALS__ = {
  transformCallback: (cb: (v: unknown) => void) => {
    const id = ++callbacks;
    w[`_${id}`] = cb;
    return id;
  },
  invoke: async (cmd: string, args: any) => {
    if (cmd === "plugin:event|listen") {
      listeners.set(args.event, [...(listeners.get(args.event) ?? []), args.handler]);
      return args.handler;
    }
    if (cmd.startsWith("plugin:")) return null;
    const h = handlers[cmd];
    if (!h) throw `mock: no handler for ${cmd}`;
    // ?slow keeps catalog / library calls pending, to see the loading states
    if (location.search.includes("slow") && /search|tidal_|library|playlists|scan/.test(cmd)) await new Promise(() => {});
    return h(args ?? {});
  },
};
// ?whitecover: a nearly white album (black-and-white artwork), to check text over light covers
w.__HOMEFI_MOCK_COVER__ = url.has("whitecover")
  ? () =>
      "data:image/svg+xml," +
      encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><rect width="100" height="100" fill="#f7f7f5"/><g fill="#111">' + Array.from({ length: 40 }, (_, i) => `<circle cx="${(i * 37) % 100}" cy="${(i * 53) % 100}" r="3"/>`).join("") + "</g></svg>")
  : coverSvg;

// advance the fake position and push "player" events like the Rust side does
setInterval(() => {
  if (state.status === "playing") state = { ...state, position_ms: Math.min(state.duration_ms ?? 0, state.position_ms + 250) };
  if (room.playing) room = { ...room, position_ms: Math.min(room.now_playing?.duration_ms ?? 0, room.position_ms + 250) };
  if (guest.status === "connected" && room.position_ms % 2000 === 0) roomChanged();
  for (const id of listeners.get("player") ?? []) w[`_${id}`]?.({ event: "player", id: 0, payload: state });
}, 250);

// headless screenshots freeze CSS transitions: ?static disables them
if (location.search.includes("static")) {
  const style = document.createElement("style");
  style.textContent = ":root{transition:none!important}";
  document.head.append(style);
}
