// Typed wrappers around the Rust commands (apps/desktop/src-tauri/src/commands.rs).
import { Channel, invoke as call } from "@tauri-apps/api/core";
import { busy } from "./busy.svelte";

/** Calls that make the user wait: they light the loading bar while they run. */
const SLOW = new Set([
  "search",
  "tidal_library_page",
  "tidal_detail",
  "tidal_detail_page",
  "tidal_library_stream",
  "tidal_detail_stream",
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
  "autoeq_search",
  "autoeq_profile",
  "ai_list_models",
  "ai_test_key",
  "taste_suggest",
]);

/** A command that sends its pages through a channel as they arrive. */
async function streamPages(cmd: string, args: Record<string, unknown>, onPage: (page: HitPage) => void): Promise<void> {
  // the command answers with how many pages it sent; big pages can reach the
  // channel after that answer, so the stream ends once all of them are in
  let got = 0;
  let want = Infinity;
  let done: () => void = () => {};
  const all = new Promise<void>((r) => (done = r));
  const channel = new Channel<HitPage>();
  channel.onmessage = (page) => {
    onPage(page);
    if (++got >= want) done();
  };
  want = await invoke<number>(cmd, { ...args, onPage: channel });
  if (got >= want) done();
  // never wait forever on a page that can't come (the window reloaded under it...)
  else setTimeout(done, 15_000);
  await all;
}

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
  /** How a stereo / mono track is spread over a surround device (more
   * speakers than the track's channels); null otherwise. */
  surround?: SurroundMode | null;
  /** The resampler's quality, when `resampled`. */
  resampler?: ResamplerQuality | null;
}

/** A crossfade being heard (homefi-player's `Fading`): not bit-perfect while it lasts. */
export interface Fading {
  /** Paths of the outgoing and incoming tracks. */
  from: string;
  to: string;
  /** 0..1 */
  progress: number;
  seconds: number;
  /** The short fade of a skip or a pick. */
  manual?: boolean;
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
  /** The equalizer changes what reaches the device (not bit-perfect). Sent by
   * the engine; absent in views built by the UI (a shared room). */
  eq_active?: boolean;
  /** Name of the equalizer profile in use, when `eq_active`. */
  eq_profile?: string | null;
  /** Id and label of the taste rule playing (when `eq_active`). */
  taste_rule?: string | null;
  taste_label?: string | null;
  /** Loudness normalization gain on this track (dB; 0 = none, bit-perfect). */
  loudness_db?: number;
  /** A crossfade is playing (null otherwise; absent in views built by the UI). */
  fading?: Fading | null;
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
  /** Albums shown as one card: every release of it (this one is `primary`). Absent otherwise. */
  versions?: VersionHit[];
}

/** One release of an album (homefi_core::catalog::VersionHit), for the version picker. */
export interface VersionHit {
  id: string;
  title: string;
  cover_url: string | null;
  explicit: boolean;
  quality: string | null;
  year: number | null;
  tracks: number;
  /** Deluxe / box set / expanded: tracks over the standard edition. */
  extra_tracks: number | null;
  /** Dolby Atmos: needs an Atmos device. */
  atmos: boolean;
  /** The one shown by default (Settings → Tidal → Album versions). */
  primary: boolean;
  /** `explicit`, `clean`, `dolby_atmos`, `hi_res`, `lossless`, `remastered`, `deluxe`, `box_set`, `expanded`, `anniversary`, `live`, `original`. */
  label_codes: string[];
  /** Chip texts, same order as `label_codes`. */
  labels: string[];
  /** `Explicit · Hi-Res 24-bit · 12 tracks · 2023`. */
  description: string;
}

/** Which release of an album a card shows (homefi_core::album_versions::VersionPrefs). */
export interface AlbumVersionPrefs {
  /** One card per album; off lists every release as Tidal does. */
  group: boolean;
  explicit: "explicit" | "clean";
  spatial: "stereo" | "atmos";
  edition: "original" | "remaster";
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
  /** `compact`: small stages joined by a flowing signal, details on hover; `detailed`: the big boxes. */
  signal_style?: "compact" | "detailed";
  /** The links of the signal chain flow while the music plays. */
  signal_animate?: boolean;
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
  /** Equalizer (off by default); camelCase inside, like homefi-core's JSON. */
  eq: EqSettings;
  /** Which AI the assistants use (never the key: that is in the OS keychain). */
  ai: AiSettings;
  /** The user's saved equalizer presets ("Save as preset"). */
  eq_presets: EqProfile[];
  eq_ui: EqUiSettings;
  /** Taste layers (camelCase inside, like `eq`); off until the first rule. */
  taste: TasteSettings;
  /** Loudness normalization from ReplayGain; off keeps playback bit-perfect. */
  loudness: LoudnessSettings;
  sound_ui: SoundUiSettings;
  /** One card per album on Tidal lists, and which release it shows. */
  album_versions: AlbumVersionPrefs;
  /** Crossfade between songs (off: gapless, bit-perfect); the same JSON as Android. */
  crossfade: CrossfadeSettings;
  /** Stereo on surround devices and the resampler's quality. */
  output: OutputOptions;
  /** Looking for and installing new versions (Settings → About). */
  updates: UpdateSettings;
}

// ---------------------------------------------------------------- updates (src-tauri/src/updater.rs)

export interface UpdateSettings {
  /** On launch and every 6 hours. */
  auto_check: boolean;
  /** Beta versions too (GitHub pre-releases). */
  prereleases: boolean;
  /** `ask`: a banner offers it; `idle`: downloaded and installed when nothing plays, downloads or is shared. */
  install: "ask" | "idle";
}

export type UpdateStatus = "idle" | "checking" | "upToDate" | "available" | "downloading" | "ready" | "installing" | "error";

export interface UpdateView {
  current: string;
  status: UpdateStatus;
  version: string | null;
  /** Release notes (Markdown). */
  notes: string | null;
  /** Epoch ms. */
  date: number | null;
  prerelease: boolean;
  /** The release page ("What's new" in the browser; "Download" when it can't update itself). */
  pageUrl: string;
  /** It can install itself; otherwise only "Download". */
  inPlace: boolean;
  installKind: "app" | "nsis" | "msi" | "appimage" | "deb" | "rpm" | "manual";
  manualReason: "dev" | "dmg" | "translocated" | "not-app" | "no-appimage" | "no-pkexec" | "unknown" | "no-package" | null;
  downloaded: number;
  total: number | null;
  error: string | null;
  /** Epoch ms of the last check. */
  lastChecked: number | null;
  /** The restart waits for the song playing to end. */
  afterSong: boolean;
  /** "Later" was picked for this version. */
  dismissed: boolean;
}

// ---------------------------------------------------------------- playback
// homefi-core's dsp::crossfade and homefi-player's OutputOptions; camelCase like their JSON.

export type CrossfadeMode = "off" | "on";
export type FadeCurve = "equalPower" | "linear" | "sCurve";

export interface CrossfadeSettings {
  mode: CrossfadeMode;
  /** 1..12 */
  seconds: number;
  curve: FadeCurve;
  /** Skip the silence at the end of a song and the start of the next. */
  smart: boolean;
  /** Consecutive songs of one album, in order, stay gapless. */
  skipAlbumRuns: boolean;
  /** A short fade on next / previous / picking a song. */
  onManualSkip: boolean;
  /** 0.5..3 */
  manualSeconds: number;
  /** Shorter songs never fade; 0 = twice the fade. */
  minTrackSeconds: number;
  /** Android only: off on phones that can't keep up. */
  lowPowerFallback: boolean;
}

export type SurroundMode = "frontOnly" | "allSpeakers" | "upmix";
export type ResamplerQuality = "high" | "fast";

export interface OutputOptions {
  surround: SurroundMode;
  resampler: ResamplerQuality;
}

/** "How does it sound?" and taste layers in the interface (Settings → Sound). */
export interface SoundUiSettings {
  /** The "How does it sound?" button in the player bar... */
  react_deck: boolean;
  /** ...and in the listening room. */
  react_room: boolean;
  /** "HD 650 + Electronic" next to the EQ button. */
  layer_chip: boolean;
  /** Suggestion cards (a taste layer for what you listen to most). */
  suggestions: boolean;
  /** How a taste rule's editor opens: the curve when it has bands (`auto`), the three sliders, or the curve. */
  taste_editor?: "auto" | "simple" | "advanced";
}

/** How the equalizer screen looks and behaves (Settings → Sound). */
export interface EqUiSettings {
  /** An "EQ" button next to the output in the player bar. */
  deck_button: boolean;
  /** "Hear without EQ": while pressed, or click on / click off. */
  ab_mode: "hold" | "toggle";
  /** Band numbers on the handles of the curve. */
  band_labels: boolean;
  /** Bass / treble / clarity sliders. */
  taste_sliders: boolean;
}

// ---------------------------------------------------------------- equalizer
// Types of homefi-core's `dsp` module (src-tauri/src/eq.rs); camelCase like
// its JSON, the same format Android stores.

export type EqBandKind = "peak" | "lowShelf" | "highShelf" | "lowPass" | "highPass";

/** One filter. `gainDb` is unused by `lowPass` / `highPass`. */
export interface EqBand {
  kind: EqBandKind;
  freqHz: number;
  gainDb: number;
  /** Width: higher is narrower (0.707 is a gentle, standard slope). */
  q: number;
  enabled: boolean;
}

export type EqSource =
  | { kind: "manual" }
  /** A headphone correction: `model` as AutoEq lists it, `source` = who measured it. */
  | { kind: "autoEq"; model: string; source: string }
  | { kind: "assistant" }
  | { kind: "guided" };

export interface EqProfile {
  name: string;
  /** Gain before the filters (dB), usually negative to leave room for boosts. */
  preampDb: number;
  bands: EqBand[];
  source: EqSource;
}

export interface EqSettings {
  enabled: boolean;
  /** Compute the preamp from the curve instead of each profile's own. */
  autoPreamp: boolean;
  /** For outputs without a profile of their own. */
  defaultProfile: EqProfile | null;
  /** Profile per output device name (`OutputDevice.name`). */
  perOutput: Record<string, EqProfile>;
}

/** Taste adjustments applied on top of a profile (dB). */
export interface Tilt {
  bassDb: number;
  trebleDb: number;
  clarityDb?: number | null;
}

// ---------------------------------------------------------------- taste layers, loudness
// homefi-core's dsp::taste / dsp::loudness (src-tauri/src/sound.rs); camelCase like their JSON.

/** What a taste rule applies to. `id` is Tidal's (a playlist's uuid, a local track's path) or empty: then names are compared. */
export type TasteScope =
  | { kind: "genre"; name: string }
  | { kind: "playlist" | "artist" | "album" | "track"; id: string; name: string };
export type TasteScopeKind = TasteScope["kind"];
export type TasteCreator = "manual" | "ai" | "guided" | "reaction";

/** One band of a taste rule's curve: an EqBand plus, from the assistant, what it does. */
export interface TasteBand extends EqBand {
  note?: string;
}

/** A taste layer for some music, on top of the output's profile: a tilt (three knobs) and / or a curve of up to 8 bands. The most specific rule plays: track > album > playlist > artist > genre. */
export interface TasteRule {
  id: string;
  scope: TasteScope;
  tilt: Tilt;
  /** The parametric curve (absent on simple and old rules). */
  bands?: TasteBand[];
  label: string;
  /** Why, in the assistant's words. */
  explanation?: string;
  createdBy: TasteCreator;
  enabled: boolean;
}

/** What a rule's editor draws (src-tauri/src/sound.rs `taste_preview`). */
export interface TastePreview {
  /** The output it plays on. */
  output: string | null;
  /** That output's own profile (when the equalizer is on). */
  device: EqProfile | null;
  /** The rule's curve alone (tilt shelves and bands). */
  taste: EqProfile;
  /** Both, as the player runs them, with the preamp. */
  combined: EqProfile;
  /** The bands as they will play (in range, safe for the output). */
  bands: TasteBand[];
  /** What the taste caps changed (English). */
  notes: string[];
  small_speaker: boolean;
}

export interface TasteSettings {
  enabled: boolean;
  rules: TasteRule[];
}

export type LoudnessMode = "off" | "track" | "album" | "auto";

export interface LoudnessSettings {
  mode: LoudnessMode;
  /** -24..-8 LUFS (default -14). */
  targetLufs: number;
  /** Never boost a song past its peak. */
  preventClipping: boolean;
}

export interface ItemRef {
  id: string;
  name: string;
}

/** Where the queue came from (taste layers for playlists, album loudness). */
export interface SourceContext {
  kind: "album" | "playlist" | "artist" | "mix" | "library" | "folder" | string;
  id: string;
  name: string;
}

export interface NowPlayingContext {
  trackId: string | null;
  path: string | null;
  title: string | null;
  album: ItemRef | null;
  artists: ItemRef[];
  genres: string[];
  source: SourceContext | null;
}

/** The track playing and what taste layers can say about it. */
export interface NowSound {
  context: NowPlayingContext | null;
  /** Scopes to offer, most specific first. */
  scopes: TasteScope[];
  /** The rule that plays now. */
  rule: TasteRule | null;
}

export type SoundReaction = "needsBass" | "tooBoomy" | "tooBright" | "dull" | "vocalsBuried" | "tooQuiet" | "tooLoud" | "perfect";

/** What a reaction changed (already saved and applied by the backend). */
export interface ReactionResult {
  rule: TasteRule | null;
  taste: TasteSettings;
  loudness: LoudnessSettings;
  /** One line for the toast. */
  message: string;
  /** For `soundUndo`; empty when nothing changed. */
  undoToken: string;
  byAi: boolean;
}

export interface ListeningSummary {
  top_genres: string[];
  top_artists: ItemRef[];
  playlists: ItemRef[];
}

/** One headphone measurement in AutoEq. */
export interface HeadphoneEntry {
  name: string;
  /** Who measured it: `oratory1990`, `crinacle`, `Rtings`, ... */
  source: string;
  /** `over-ear`, `in-ear` or `earbud`. */
  kind: string;
  /** Measurement rig when the source used several (`711`, `GRAS 43AG-7`). */
  rig: string | null;
  /** Folder in AutoEq's results (`oratory1990/over-ear/Sennheiser HD 650`). */
  path: string;
}

/** Ranges the core clamps to (values outside are brought in). */
export const EQ_LIMITS = {
  maxBands: 10,
  freqHz: [20, 20_000],
  gainDb: [-24, 24],
  q: [0.1, 10],
  preampDb: [-60, 24],
} as const;

/** Caps of a taste rule's curve (homefi-core `dsp::taste`): each band ±9 dB, the whole layer ≤ +12 dB. */
export const TASTE_LIMITS = {
  maxBands: 8,
  freqHz: [20, 20_000],
  gainDb: [-9, 9],
  q: [0.3, 8],
  maxBoostDb: 12,
} as const;

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
  /** Where it was played from (album, playlist...), for taste layers. */
  source?: SourceContext | null;
  artist?: string;
  album?: string;
  cover_url?: string | null;
  duration_ms?: number | null;
  explicit?: boolean;
}

// ---------------------------------------------------------------- AI (bring your own key)
// homefi-core's `ai` module and src-tauri/src/ai.rs. Keys go to the backend
// (save / test) but never come back: only "set / not set".

export type AiProviderId = "anthropic" | "openai" | "gemini" | "openrouter" | "mistral" | "groq" | "xai" | "deepseek" | "ollama" | "openai_compatible";

export interface AiProviderInfo {
  id: AiProviderId;
  name: string;
  /** Where to create a key; empty for the custom server. */
  key_url: string;
  needs_key: boolean;
  default_base_url: string;
  /** The user may (Ollama) or must (custom) type the server's address. */
  custom_base_url: boolean;
  /** Only Anthropic has one; the rest pick from the model list. */
  default_model: string | null;
  /** 1 = the best pick; the list comes sorted by it. */
  rank: number;
  badge: AiProviderBadge | null;
  /** One line on why to pick it. */
  hint: string;
}

export type AiProviderBadge = "recommended" | "cheapest" | "one_key_for_all" | "private_local";

export interface AiModel {
  id: string;
  name: string;
  /** What "Automatic" picks. */
  recommended: boolean;
  /** Small by its id (8b, nano, lite…): cheap, but makes more mistakes. */
  small: boolean;
  /** The provider's cheaper tier that still works well (Claude Haiku). */
  economical: boolean;
}

export interface AiSettings {
  /** Empty: no AI, the EQ assistant runs as the guided wizard. */
  provider: AiProviderId | "";
  /** Empty: the provider's default or the best one it lists. */
  model: string;
  /** Ollama / custom server address; empty = the provider's. */
  base_url: string;
}

/** @deprecated the same as `EqBand`. */
export type AssistantEqBand = EqBand;
/** @deprecated the same as `EqProfile`. */
export type AssistantEqProfile = EqProfile;

/** A profile the assistant proposes for one output (validated, boosts capped, automatic preamp). */
export interface EqProposal {
  /** Output device name (the key of the EQ settings' perOutput). */
  output_id: string;
  profile: EqProfile;
  /** Why, in plain words and the user's language. */
  explanation: string;
}

/** One answer of the EQ assistant (AI) or the guided wizard: the same screen draws both. */
export interface AssistantTurn {
  text: string;
  /** Short answers to offer as buttons (the user may still type). */
  quick_replies: string[];
  /** The profile on the table, to preview. */
  proposal: EqProposal | null;
  /** The user said yes: the backend already saved and applied `proposal`. */
  confirmed: boolean;
  /** The conversation is over. */
  done: boolean;
  /** Taste rules changed this turn (already saved and applied). */
  taste?: TasteSettings | null;
  /** Loudness changed this turn (already saved and applied). */
  loudness?: LoudnessSettings | null;
  /** A taste layer proposed this turn (not saved yet): draw its curve and band notes. */
  taste_proposal?: TasteRule | null;
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
  /** Every release of an album (itself included), for "Other versions". */
  tidalAlbumVersions: (id: string) => invoke<VersionHit[]>("tidal_album_versions", { id }),
  /** Every page of a collection from `offset`, each sent to `onPage` as it arrives (cancel with `tidalCancel(load)`). */
  tidalLibraryStream: (kind: LibraryKind, offset: number, onPage: (page: HitPage) => void, load: number) =>
    streamPages("tidal_library_stream", { kind, offset, load }, onPage),
  /** Every track of an album / playlist / mix from `offset`, page by page (see `tidalLibraryStream`). */
  tidalDetailStream: (kind: Kind, id: string, offset: number, onPage: (page: HitPage) => void, load: number) =>
    streamPages("tidal_detail_stream", { kind, id, offset, load }, onPage),
  /** Stops a page stream (the user went elsewhere). */
  tidalCancel: (load: number) => invoke<void>("tidal_cancel", { load }),
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
  /** `source`: where the songs come from (an album, a playlist...), for taste layers and album loudness. */
  play: (paths: string[], index = 0, shuffle = false, source: SourceContext | null = null) => invoke<void>("play_tracks", { paths, index, shuffle, source }),
  streamUpcoming: () => invoke<Upcoming[]>("stream_upcoming"),
  playTidal: (items: StreamItem[], index = 0, shuffle = false, source: SourceContext | null = null) => invoke<void>("play_tidal", { items, index, shuffle, source }),
  queueAdd: (paths: string[], next: boolean) => invoke<void>("queue_add", { paths, next }),
  queueTidal: (items: StreamItem[], next: boolean) => invoke<number>("queue_tidal", { items, next }),
  tracksFor: (paths: string[]) => invoke<(Track | null)[]>("tracks_for", { paths }),
  toggle: () => invoke<void>("player_toggle"),
  next: () => invoke<void>("player_next"),
  previous: () => invoke<void>("player_previous"),
  seek: (ms: number) => invoke<void>("player_seek", { ms: Math.max(0, Math.round(ms)) }),
  jump: (index: number) => invoke<void>("player_jump", { index }),
  miniPlayer: (open: boolean) => invoke<void>("mini_player", { open }),
  /** The mini player's page has drawn: show its window and minimize the main one. */
  miniReady: () => invoke<void>("mini_ready"),
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

  /** Equalizer settings (also in `Settings.eq`). */
  eqGet: () => invoke<EqSettings>("eq_get"),
  /** Saves and applies them live to the player; returns them clamped. */
  eqSet: (settings: EqSettings) => invoke<EqSettings>("eq_set", { settings }),
  /** "Hear without EQ": the player runs flat while `on` (nothing is saved). */
  eqBypass: (on: boolean) => invoke<void>("eq_bypass", { on }),
  /** Response of the profile's bands in dB at `freqs` (preamp not included). */
  eqResponse: (profile: EqProfile, freqs: number[], sampleRate = 48_000) => invoke<number[]>("eq_response", { profile, freqs, sampleRate }),
  /** `n` log-spaced frequencies, 20 Hz to 20 kHz, for drawing curves. */
  eqLogFreqs: (n: number) => invoke<number[]>("eq_log_freqs", { n }),
  /** Preamp (dB, ≤ 0) that keeps the profile's largest boost from clipping. */
  eqAutoPreamp: (profile: EqProfile) => invoke<number>("eq_auto_preamp", { profile }),
  /** The profile with bass / treble / clarity adjustments applied. */
  eqWithTaste: (profile: EqProfile, tilt: Tilt) => invoke<EqProfile>("eq_with_taste", { profile, tilt }),
  /** Headphones in AutoEq matching `query`, best first (offline from the cache). */
  autoeqSearch: (query: string) => invoke<HeadphoneEntry[]>("autoeq_search", { query }),
  /** The AutoEq correction profile of a headphone (downloaded once, then cached). */
  autoeqProfile: (entry: HeadphoneEntry) => invoke<EqProfile>("autoeq_profile", { entry }),

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
  // AI: the key is read from the OS keychain by the backend
  aiProviders: () => invoke<AiProviderInfo[]>("ai_providers"),
  /** Which providers have a key saved. */
  aiKeyStatus: () => invoke<Record<AiProviderId, boolean>>("ai_key_status"),
  aiKeySet: (provider: AiProviderId, key: string) => invoke<void>("ai_key_set", { provider, key }),
  aiKeyClear: (provider: AiProviderId) => invoke<void>("ai_key_clear", { provider }),
  aiListModels: (provider: AiProviderId, baseUrl: string | null = null) => invoke<AiModel[]>("ai_list_models", { provider, baseUrl }),
  /** Checks `key` (before saving it) or, without it, the saved one; rejects with a plain message. */
  aiTestKey: (provider: AiProviderId, baseUrl: string | null = null, key: string | null = null) => invoke<void>("ai_test_key", { provider, baseUrl, key }),
  /** The model used when the settings name none. */
  aiDefaultModel: (provider: AiProviderId, baseUrl: string | null = null) => invoke<string>("ai_default_model", { provider, baseUrl }),
  /** Opens the EQ assistant with the AI of the settings, or the guided wizard (`guided`, or no provider). */
  eqAssistantStart: (guided: boolean, language: string, notes: string | null = null, focusRule: string | null = null) =>
    invoke<AssistantTurn>("eq_assistant_start", { guided, language, notes, focusRule }),
  eqAssistantReply: (text: string) => invoke<AssistantTurn>("eq_assistant_reply", { text }),
  eqAssistantClose: () => invoke<void>("eq_assistant_close"),

  // taste layers, loudness and "How does it sound?" (src-tauri/src/sound.rs)
  tasteGet: () => invoke<TasteSettings>("taste_get"),
  /** Saves and applies the rules (tilts brought in range). */
  tasteSet: (taste: TasteSettings) => invoke<TasteSettings>("taste_set", { taste }),
  loudnessGet: () => invoke<LoudnessSettings>("loudness_get"),
  loudnessSet: (loudness: LoudnessSettings) => invoke<LoudnessSettings>("loudness_set", { loudness }),
  crossfadeGet: () => invoke<CrossfadeSettings>("crossfade_get"),
  /** Saves and applies it from the next transition; returns it in range. */
  crossfadeSet: (crossfade: CrossfadeSettings) => invoke<CrossfadeSettings>("crossfade_set", { crossfade }),
  outputOptionsGet: () => invoke<OutputOptions>("output_options_get"),
  /** Saves the surround / resampler options; the song playing restarts where it is. */
  outputOptionsSet: (options: OutputOptions) => invoke<OutputOptions>("output_options_set", { options }),
  /** The track playing, the scopes it offers and the rule that plays. */
  soundNow: () => invoke<NowSound>("sound_now"),
  /** A rule's curve, the output's profile and both together, for its editor. */
  tastePreview: (rule: TasteRule) => invoke<TastePreview>("taste_preview", { rule }),
  /** A reaction to what plays (AI when set up, else fixed safe steps); saved and applied. */
  soundReact: (reaction: SoundReaction, scope: TasteScope | null = null, language = "en") => invoke<ReactionResult>("sound_react", { reaction, scope, language }),
  /** Puts back what a reaction changed; false when already undone. */
  soundUndo: (token: string) => invoke<boolean>("sound_undo", { token }),
  /** A proactive suggestion (a card); answers go to `eqAssistantReply`. */
  tasteSuggest: (language: string) => invoke<AssistantTurn>("taste_suggest", { language }),
  listeningSummary: () => invoke<ListeningSummary>("listening_summary"),

  // new versions (src-tauri/src/updater.rs): only GitHub is asked, nothing about the user is sent
  updateStatus: () => invoke<UpdateView>("update_status"),
  /** "Check now". */
  updateCheck: () => invoke<UpdateView>("update_check"),
  /** "Update now": downloads and verifies it (progress in `on.update`). */
  updateDownload: () => invoke<void>("update_download"),
  /** Installs the downloaded update and restarts, now or when the song playing ends. */
  updateRestart: (when: "now" | "song_end") => invoke<void>("update_restart", { when }),
  /** "Later": hides the banner for this version until HomeFi starts again. */
  updateDismiss: () => invoke<void>("update_dismiss"),
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
  update: (f: (v: UpdateView) => void): Promise<UnlistenFn> => listen<UpdateView>("update", (e) => f(e.payload)),
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
