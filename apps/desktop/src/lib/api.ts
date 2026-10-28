// Typed wrappers around the Rust commands (apps/desktop/src-tauri/src/commands.rs).
import { invoke as call } from "@tauri-apps/api/core";
import { busy } from "./busy.svelte";

/** Calls that make the user wait: they light the loading bar while they run. */
const SLOW = new Set([
  "search",
  "tidal_library_page",
  "tidal_detail",
  "tidal_detail_page",
  "tidal_tracks_by_isrc",
  "tidal_my_playlists",
  "tidal_favorite_ids",
  "scan_folder",
  "library",
  "local_playlists",
  "rescan_library",
  "play_tidal",
  "tracks_for",
  "queue_tidal",
]);

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!SLOW.has(cmd)) return call<T>(cmd, args);
  busy.count++;
  try {
    return await call<T>(cmd, args);
  } finally {
    busy.count--;
  }
}
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Kind = "track" | "video" | "album" | "playlist" | "artist" | "mix";
export type Quality = "low" | "normal" | "high" | "max";
export type Repeat = "off" | "all" | "one";

export interface Track {
  path: string;
  title: string;
  artist: string;
  album: string;
  albumArtist: string;
  trackNumber: number;
  discNumber: number;
  year: number | null;
  genre: string;
  durationMs: number;
  format: string;
  sampleRate: number | null;
  bitDepth: number | null;
  lossless: boolean;
  quality: string;
  isrc: string | null;
}

export interface StreamInfo {
  codec: string;
  source_rate: number;
  bits_per_sample: number | null;
  source_channels: number;
  output_rate: number;
  output_channels: number;
  resampled: boolean;
  device: string;
}

export interface PlayerState {
  status: "stopped" | "playing" | "paused";
  queue: string[];
  current: number | null;
  position_ms: number;
  duration_ms: number | null;
  volume: number;
  shuffle: boolean;
  repeat: Repeat;
  preferred_device: string | null;
  stream: StreamInfo | null;
  error: string | null;
  track: Track | null;
}

export interface OutputDevice {
  name: string;
  is_default: boolean;
  sample_rates: number[];
  default_rate: number | null;
  max_channels: number;
}

export interface Hit {
  kind: Kind;
  id: string;
  title: string;
  subtitle: string;
  /** Main artist (tracks, albums); album title (tracks). */
  artist?: string;
  album?: string;
  cover_url: string | null;
  explicit: boolean;
  quality: string | null;
  isrc: string | null;
  duration_ms: number | null;
}

/** An .m3u / .m3u8 file in the music folders; `tracks` are absolute paths (some may be missing). */
export interface LocalPlaylist {
  name: string;
  path: string;
  tracks: string[];
}

export interface TidalDetail {
  header: Hit;
  /** A playlist the user created (tracks can be added / removed). */
  owned: boolean;
  description: string | null;
  /** The first page (lazy) or every track. */
  tracks: Hit[];
  albums: Hit[];
  /** Tracks in the whole list (Tidal's count). */
  total: number;
  duration_ms: number | null;
  /** Offset of the next page for `tidalDetailPage`; null when `tracks` is the whole list. */
  next_offset: number | null;
}

/** A page of a Tidal list; `rest` asks for everything from `offset` to the end. */
export interface HitPage {
  items: Hit[];
  total: number;
  next_offset: number | null;
}

export type LibraryKind = "playlists" | "albums" | "artists" | "tracks";

export interface ArtworkProgress {
  done: number;
  total: number;
  fixed: number;
  current: string;
}

export interface RoomSettings {
  signal_chain: boolean;
  vu_meters: boolean;
  visualizer: boolean;
  halo: boolean;
  mascot: boolean;
  mascot_style: string;
  mascot_dance: string;
  mascot_specials: "off" | "sometimes" | "often";
  mascot_limbs: "white" | "navy" | "accent" | "off";
  mascot_turns: "off" | "dances" | "all";
  mascot_size: number;
  mascot_free: boolean;
  mascot_x: number;
  mascot_y: number;
  mascot_everywhere: boolean;
  mascot_right_click: boolean;
  mascot_temper: "off" | "patient" | "normal" | "short";
  album_colors: boolean;
  /** Look of the now-playing room (the same layouts as Android). */
  layout: PlayerLayout;
  /** The layout skins the whole app (rail, deck, lists, settings), not only the room. */
  layout_everywhere: boolean;
  immersive_shade: "auto" | "strong" | "soft" | "off";
  /** Lyrics view: the whole text, or karaoke (the line being sung, big). */
  lyrics_mode: LyricsMode;
  lyrics_size: "small" | "medium" | "large";
  /** Open the lyrics on their own when a song that has them starts. */
  lyrics_auto_open: boolean;
  /** The mascot sings into a microphone in karaoke. */
  lyrics_mascot: boolean;
  /** Lyrics shifted against the music (Bluetooth delay); positive = later. −2000..2000. */
  lyrics_offset_ms: number;
  /** How much the app moves (transitions, lists cascading in); `auto` is full, or subtle when the system asks for less motion. */
  motion: Motion;
}

export type Motion = "auto" | "full" | "subtle" | "off";

export type LyricsMode = "normal" | "karaoke";

/** One line of a song; `time_ms` is when it starts (null when not synced). */
export interface LyricLine {
  time_ms: number | null;
  text: string;
}

/** homefi_core::lyrics::Lyrics */
export interface Lyrics {
  lines: LyricLine[];
  /** Every line has a start time. */
  synced: boolean;
  /** Who provided them (e.g. `MUSIXMATCH`); null for local files. */
  provider: string | null;
}

export type PlayerLayout = "classic" | "immersive" | "vinyl" | "studio" | "retro_amp" | "luna" | "aluminum" | "web2000";

export interface SearchResults {
  top_hit: Hit | null;
  tracks: Hit[];
  albums: Hit[];
  artists: Hit[];
  playlists: Hit[];
  videos: Hit[];
}

export interface Visual {
  left: number;
  right: number;
  spectrum: number[];
  wave: number[];
}

export type Theme = "system" | "light" | "dark";

export interface Settings {
  download_dir: string;
  library_dirs: string[];
  quality: Quality;
  embed_cover: boolean;
  embed_lyrics: boolean;
  skip_existing: boolean;
  volume: number;
  output_device: string | null;
  room: RoomSettings;
  theme: Theme;
  recent_folders: string[];
  progress_style: ProgressStyle;
  progress_thickness: "thin" | "medium" | "thick";
  progress_animated: boolean;
  /** Tidal lists load page by page while scrolling; off loads them whole. */
  lazy_lists: boolean;
  share: ShareSettings;
  /** Version of the personal-use notice accepted before connecting Tidal; 0 = never. */
  tidal_terms_accepted: number;
  /** When it was accepted (epoch ms). */
  tidal_terms_accepted_at: number | null;
}

// ---------------------------------------------------------------- shared control
// Types of homefi-core's share protocol and the desktop's share module
// (src-tauri/src/share); snake_case like the wire format.

export type AddMode = "free" | "approve" | "limit";
export type QueueOrder = "first_come" | "turns";
export type SkipMode = "nobody" | "everyone" | "vote";
export type EditMode = "own" | "all" | "none";

export interface ShareSettings {
  session_name: string;
  name: string;
  color: string;
  fixed_code: boolean;
  code: string;
  port: number;
  party_default: boolean;
  add: AddMode;
  max_per_guest: number;
  order: QueueOrder;
  skip: SkipMode;
  playback: boolean;
  edit: EditMode;
  allow_explicit: boolean;
  drop_songs_on_leave: boolean;
  notice_joined: boolean;
  notice_left: boolean;
  notice_added: boolean;
  notice_approval: boolean;
  notice_seconds: number;
  discover: boolean;
  discover_everywhere: boolean;
  announce_lan: boolean;
  play_along: boolean;
  speaker_delay_ms: number;
  sync_by_speed: boolean;
  preload_songs: boolean;
  guest_opens_player: boolean;
  party_start_wait_sec: number;
  send_levels: boolean;
}

export interface Permissions {
  add: AddMode;
  max_per_guest: number;
  order: QueueOrder;
  skip: SkipMode;
  playback: boolean;
  edit: EditMode;
  allow_explicit: boolean;
  drop_songs_on_leave: boolean;
}

/** A song as guests see it; `source` is `tidal:<id>` or `hub:<path>`. */
export interface ShareTrack {
  source: string;
  title: string;
  artist: string;
  album: string;
  duration_ms: number | null;
  cover_url: string | null;
  explicit: boolean;
}

export interface Member {
  guest_id: string;
  name: string;
  color: string;
}

export interface QueueItem extends ShareTrack {
  queue_id: number;
  added_by: Member;
}

export interface AudioStream {
  queue_id: number;
  url: string;
  mime: string;
  size: number;
}

export interface ShareState {
  now_playing: QueueItem | null;
  position_ms: number;
  clock_ms: number;
  playing: boolean;
  audio: AudioStream | null;
  prefetch: AudioStream[];
  queue: QueueItem[];
  guests: Member[];
  permissions: Permissions;
  skip_votes: number;
  skip_votes_needed: number;
  volume: number | null;
}

export type BrowseKind = "album" | "artist" | "playlist" | "unknown";

export interface Collection {
  kind: BrowseKind;
  id: string;
  title: string;
  subtitle: string;
  cover_url: string | null;
}

export interface ShareResults {
  q: string;
  tracks: ShareTrack[];
  albums: Collection[];
  artists: Collection[];
  playlists: Collection[];
}

export interface SharePage {
  header: Collection;
  tracks: ShareTrack[];
}

export interface HubInfo {
  host: string;
  port: number;
  code: string;
  session_name: string;
  join_uri: string;
}

/** This computer as the hub. */
export interface HubView {
  running: boolean;
  starting: boolean;
  party: boolean;
  play_here: boolean;
  info: HubInfo | null;
  guests: Member[];
  pending: QueueItem[];
  state: ShareState | null;
  error: string | null;
}

/** This computer as a guest of someone else's hub. */
export interface GuestView {
  status: "idle" | "connecting" | "connected" | "disconnected";
  session_name: string;
  guest_id: string;
  host: string;
  port: number;
  state: ShareState | null;
  position_ms: number;
  error: string | null;
  play_along: boolean;
  playing_here: boolean;
  round_trip_ms: number | null;
  stream: StreamInfo | null;
}

/** A hub found on the network. */
export interface Nearby {
  name: string;
  host: string;
  port: number;
  party: boolean | null;
}

export interface ShareStatus {
  hub: HubView;
  guest: GuestView;
  nearby: Nearby[];
  name: string;
  session_name: string;
}

export interface ShareNotice {
  text: string;
  color: string | null;
  kind: "hub" | "guest" | "error";
  seconds: number;
}

export interface JoinLink {
  host: string;
  port: number;
  code: string;
  session_name: string;
}

/** What a guest can ask the hub. */
export type GuestRequest =
  | { type: "search"; q: string }
  | { type: "browse"; kind: BrowseKind; id: string }
  | { type: "add"; item: ShareTrack; next: boolean }
  | { type: "remove"; queue_id: number }
  | { type: "move"; queue_id: number; to: number }
  | { type: "skip" }
  | { type: "vote_skip" }
  | { type: "toggle" }
  | { type: "play"; queue_id: number }
  | { type: "volume"; value: number };

export type ProgressStyle = "classic" | "wavy" | "waveform" | "neon" | "led" | "minimal";

export interface AppInfo {
  version: string;
  appDir: string;
  ffmpeg: boolean;
  platform: string;
}

export interface Account {
  userId: string;
  countryCode: string;
}

export interface DeviceLogin {
  deviceCode: string;
  userCode: string;
  url: string;
  interval: number;
  expiresIn: number;
}

export type DownloadEvent =
  | { type: "jobStarted"; label: string; pending: number }
  | { type: "jobFinished"; label: string; downloaded: number; failed: number }
  | { type: "queued"; id: string; title: string }
  | { type: "started"; task: number; id: string; title: string; quality: string }
  | { type: "progress"; task: number; downloaded: number; total: number | null }
  | { type: "finished"; task: number; id: string; title: string; quality: string; path: string }
  | { type: "exists"; id: string; title: string; path: string }
  | { type: "skipped"; id: string; title: string; reason: string }
  | { type: "failed"; id: string | null; context: string; message: string };

/** A Tidal track to play; `local` when the library already has it. The rest is what guests see while sharing. */
/** A Tidal track of the stream not fetched yet: it joins the queue just before it plays. */
export interface Upcoming {
  id: string;
  title: string;
  artist: string;
  duration_ms: number | null;
}

export interface StreamItem {
  id: string;
  title: string;
  local: string | null;
  artist?: string;
  album?: string;
  cover_url?: string | null;
  duration_ms?: number | null;
  explicit?: boolean;
}

export type StreamEvent = { status: "loading"; title: string } | { status: "ready" } | { status: "failed"; title: string; message: string };

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  account: () => invoke<Account | null>("account"),
  startLogin: () => invoke<DeviceLogin>("start_login"),
  pollLogin: (deviceCode: string) => invoke<"pending" | "success" | "expired">("poll_login", { deviceCode }),
  logout: () => invoke<void>("logout"),
  search: (query: string) => invoke<SearchResults>("search", { query }),
  tidalLibraryPage: (kind: LibraryKind, offset = 0, rest = false) => invoke<HitPage>("tidal_library_page", { kind, offset, rest }),
  /** The whole collection, every page. */
  tidalLibrary: (kind: LibraryKind) => invoke<HitPage>("tidal_library_page", { kind, offset: 0, rest: true }).then((p) => p.items),
  tidalDetail: (kind: Kind, id: string, lazy = false) => invoke<TidalDetail>("tidal_detail", { kind, id, lazy }),
  tidalDetailPage: (kind: Kind, id: string, offset: number, rest = false) => invoke<HitPage>("tidal_detail_page", { kind, id, offset, rest }),
  tracksByIsrc: (isrc: string) => invoke<string[]>("tidal_tracks_by_isrc", { isrc }),
  favoriteIds: () => invoke<Record<"track" | "album" | "artist" | "playlist", string[]>>("tidal_favorite_ids"),
  favorite: (kind: Kind, id: string, on: boolean) => invoke<void>("tidal_favorite", { kind, id, on }),
  myPlaylists: () => invoke<Hit[]>("tidal_my_playlists"),
  createPlaylist: (title: string, description: string, trackIds: string[]) =>
    invoke<Hit>("tidal_create_playlist", { title, description, trackIds }),
  playlistAdd: (uuid: string, trackIds: string[]) => invoke<void>("tidal_playlist_add", { uuid, trackIds }),
  playlistRemove: (uuid: string, index: number) => invoke<void>("tidal_playlist_remove", { uuid, index }),
  playlistDelete: (uuid: string) => invoke<void>("tidal_playlist_delete", { uuid }),
  fixArtwork: () => invoke<ArtworkProgress>("fix_artwork"),
  scanFolder: (path: string) => invoke<Track[]>("scan_folder", { path }),
  download: (label: string, links: string[]) => invoke<void>("download", { label, links }),
  cancelDownloads: () => invoke<void>("cancel_downloads"),
  library: () => invoke<Track[]>("library"),
  localPlaylists: () => invoke<LocalPlaylist[]>("local_playlists"),
  rescanLibrary: () => invoke<Track[]>("rescan_library"),
  deleteTracks: (paths: string[]) => invoke<string[]>("delete_tracks", { paths }),
  pruneLibrary: () => invoke<string[]>("prune_library"),
  playerState: () => invoke<PlayerState>("player_state"),
  play: (paths: string[], index = 0, shuffle = false) => invoke<void>("play_tracks", { paths, index, shuffle }),
  streamUpcoming: () => invoke<Upcoming[]>("stream_upcoming"),
  playTidal: (items: StreamItem[], index = 0, shuffle = false) => invoke<void>("play_tidal", { items, index, shuffle }),
  queueAdd: (paths: string[], next: boolean) => invoke<void>("queue_add", { paths, next }),
  queueTidal: (items: StreamItem[], next: boolean) => invoke<number>("queue_tidal", { items, next }),
  tracksFor: (paths: string[]) => invoke<(Track | null)[]>("tracks_for", { paths }),
  toggle: () => invoke<void>("player_toggle"),
  next: () => invoke<void>("player_next"),
  previous: () => invoke<void>("player_previous"),
  seek: (ms: number) => invoke<void>("player_seek", { ms: Math.max(0, Math.round(ms)) }),
  jump: (index: number) => invoke<void>("player_jump", { index }),
  miniPlayer: (open: boolean) => invoke<void>("mini_player", { open }),
  moveTrack: (from: number, to: number) => invoke<void>("player_move", { from, to }),
  shuffle: (on: boolean) => invoke<void>("player_shuffle", { on }),
  repeat: (mode: Repeat) => invoke<void>("player_repeat", { mode }),
  volume: (volume: number) => invoke<void>("player_volume", { volume }),
  visual: (bands: number) => invoke<Visual>("visual", { bands }),
  outputDevices: () => invoke<OutputDevice[]>("output_devices"),
  setOutput: (name: string | null) => invoke<void>("set_output", { name }),
  /** Lyrics of a song: the local file (`.lrc` / tag) first, then Tidal; null when there are none. */
  lyrics: (trackId: string | null, path: string | null) => invoke<Lyrics | null>("lyrics", { trackId, path }),
  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (settings: Settings) => invoke<Settings>("set_settings", { settings }),

  shareStatus: () => invoke<ShareStatus>("share_status"),
  shareStart: (party: boolean) => invoke<HubView>("share_start", { party }),
  shareStop: () => invoke<void>("share_stop"),
  shareSetParty: (on: boolean) => invoke<void>("share_set_party", { on }),
  shareSetPlayHere: (on: boolean) => invoke<void>("share_set_play_here", { on }),
  shareKick: (guestId: string, ban: boolean) => invoke<boolean>("share_kick", { guestId, ban }),
  shareApprove: (queueId: number) => invoke<boolean>("share_approve", { queueId }),
  shareReject: (queueId: number) => invoke<boolean>("share_reject", { queueId }),
  shareJoin: (host: string, port: number, code: string, name?: string, color?: string, sessionName?: string) =>
    invoke<GuestView>("share_join", { host, port, code, name: name ?? null, color: color ?? null, sessionName: sessionName ?? null }),
  shareLeave: () => invoke<void>("share_leave"),
  shareGuest: (message: GuestRequest) => invoke<void>("share_guest", { message }),
  sharePlayAlong: (on: boolean) => invoke<void>("share_play_along", { on }),
  shareProbe: (host: string, port: number) => invoke<Nearby | null>("share_probe", { host, port }),
  shareParseLink: (link: string) => invoke<JoinLink | null>("share_parse_link", { link }),
};

export const on = {
  player: (f: (s: PlayerState) => void): Promise<UnlistenFn> => listen<PlayerState>("player", (e) => f(e.payload)),
  download: (f: (e: DownloadEvent) => void): Promise<UnlistenFn> => listen<DownloadEvent>("download", (e) => f(e.payload)),
  settings: (f: (s: Settings) => void): Promise<UnlistenFn> => listen<Settings>("settings", (e) => f(e.payload)),
  library: (f: () => void): Promise<UnlistenFn> => listen("library", () => f()),
  artwork: (f: (p: ArtworkProgress) => void): Promise<UnlistenFn> => listen<ArtworkProgress>("artwork", (e) => f(e.payload)),
  stream: (f: (e: StreamEvent) => void): Promise<UnlistenFn> => listen<StreamEvent>("stream", (e) => f(e.payload)),
  upcoming: (f: (list: Upcoming[]) => void): Promise<UnlistenFn> => listen<Upcoming[]>("stream-upcoming", (e) => f(e.payload)),
  shareHub: (f: (v: HubView) => void): Promise<UnlistenFn> => listen<HubView>("share-hub", (e) => f(e.payload)),
  shareGuest: (f: (v: GuestView) => void): Promise<UnlistenFn> => listen<GuestView>("share-guest", (e) => f(e.payload)),
  shareResults: (f: (r: ShareResults) => void): Promise<UnlistenFn> => listen<ShareResults>("share-results", (e) => f(e.payload)),
  sharePage: (f: (p: SharePage) => void): Promise<UnlistenFn> => listen<SharePage>("share-page", (e) => f(e.payload)),
  shareDenied: (f: (reason: string) => void): Promise<UnlistenFn> => listen<string>("share-denied", (e) => f(e.payload)),
  shareNotice: (f: (n: ShareNotice) => void): Promise<UnlistenFn> => listen<ShareNotice>("share-notice", (e) => f(e.payload)),
  shareNearby: (f: (list: Nearby[]) => void): Promise<UnlistenFn> => listen<Nearby[]>("share-nearby", (e) => f(e.payload)),
};

const isWindows = navigator.userAgent.includes("Windows");

/** URL of the cover art embedded in (or next to) a local file. */
let coverVersion = 0;
/** Call after covers were embedded so images reload. */
export function bumpCovers() {
  coverVersion++;
}

/** Covers of songs in someone else's room (`room:<queue id>` paths), by path. */
export const remoteCovers = new Map<string, string>();

export function coverUrl(path: string): string {
  if (path.startsWith("room:")) return remoteCovers.get(path) ?? "";
  const mock = (window as unknown as { __HOMEFI_MOCK_COVER__?: (p: string) => string }).__HOMEFI_MOCK_COVER__;
  if (mock) return mock(path);
  const encoded = encodeURIComponent(path);
  const v = coverVersion ? `?v=${coverVersion}` : "";
  return isWindows ? `http://cover.localhost/${encoded}${v}` : `cover://localhost/${encoded}${v}`;
}

export function formatTime(ms: number | null | undefined): string {
  const total = Math.max(0, Math.floor((ms ?? 0) / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = String(total % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
}

export function khz(rate: number): string {
  return rate % 1000 === 0 ? `${rate / 1000}` : (rate / 1000).toFixed(1);
}

/** An error in plain words for the screen. The Rust side already words its own
 * (homefi_core::humane); this covers the webview's (fetch, images, sockets). */
export function errorText(e: unknown): string {
  const text = typeof e === "string" ? e : e instanceof Error ? e.message : "";
  if (/failed to fetch|networkerror|load failed|network connection was lost|internet connection appears to be offline/i.test(text))
    return "No internet connection. Check your Wi-Fi and try again.";
  if (/timed? ?out/i.test(text)) return "It's taking too long to answer. Try again in a moment.";
  if (!text || /:\/\/|Error:|TypeError|undefined|null|\{/.test(text)) return "Something went wrong. Try again.";
  return text;
}

/** Icon for an output device, guessed from its name (cpal does not expose the transport). */
export function outputIcon(name: string): string {
  const n = name.toLowerCase();
  if (/(airpods|wh-|wf-|bose|buds|bluetooth|beats|jbl|sony)/.test(n)) return "bluetooth";
  if (/(headphone|auriculares|jack)/.test(n)) return "headphones";
  if (/(usb|dac|fiio|topping|schiit|ifi|chord|dragonfly|razer|focusrite|scarlett|motu|apogee|rme)/.test(n)) return "usb";
  if (/(display|hdmi|pa\d|monitor|tv\b|lg |dell|samsung)/.test(n)) return "monitor";
  return "speaker";
}
