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
  "concert_build",
  "concert_rearrange",
  "party_start",
  "chat_open",
  "party_reply",
  "party_build",
  "home_playlists",
  "home_playlist_save_to_tidal",
  "disc_open",
  "disc_identify",
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
  /** The target that matched (with `taste_rule` / `taste_label`). */
  taste_target?: TasteScope | null;
  /** The sound playing is a Try (not saved). */
  auditioning?: boolean;
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
  /** Right-click on the mascot: its actions menu, the next style, or nothing. */
  mascot_hold: "menu" | "style" | "off";
  /** A dances button next to the mascot. */
  mascot_dance_button: boolean;
  mascot_temper: "off" | "patient" | "normal" | "short";
  /** Long hair: `auto` (in mosh mode), `long` or `short`. */
  mascot_hair?: "auto" | "long" | "short";
  /** Dance on the real beat of the music; off keeps the free tempo. */
  mascot_follow_beat?: boolean;
  /** How hard it headbangs; the windmill needs more than gentle. */
  mascot_headbang?: "gentle" | "normal" | "wild";
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
  /** How wide Settings reads: a centred column (two columns of groups on very wide windows) or one broader column. */
  settings_width: SettingsWidth;
  /** Lyrics, queue and mini player in a bar above the content, or in the player bar. */
  player_buttons: PlayerButtonsPlace;
  /** On a wide window the song playing sits in a column on the right (sized to the window) while you browse. */
  side_player?: boolean;
}

export type PlayerButtonsPlace = "top" | "bottom";

export type Motion = "auto" | "full" | "subtle" | "off";

export type SettingsWidth = "comfortable" | "wide";

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
  /** Sounds (camelCase inside, like `eq`); the master switch is `eq.enabled`. */
  taste: TasteSettings;
  /** Loudness normalization from ReplayGain; off keeps playback bit-perfect. */
  loudness: LoudnessSettings;
  sound_ui: SoundUiSettings;
  /** One card per album on Tidal lists, and which release it shows. */
  album_versions: AlbumVersionPrefs;
  /** "Get ready for a concert" and the playlists HomeFi makes. */
  playlists: PlaylistSettings;
  /** Reading and ripping discs (Settings → Discs); camelCase inside, like `eq`. */
  disc: DiscSettings;
  /** Crossfade between songs (off: gapless, bit-perfect); the same JSON as Android. */
  crossfade: CrossfadeSettings;
  /** Stereo on surround devices and the resampler's quality. */
  output: OutputOptions;
  /** Looking for and installing new versions (Settings → About). */
  updates: UpdateSettings;
  /** What the mini player adds to its bar (Settings → Listening room). */
  mini: MiniSettings;
  /** DJ mode (Settings → DJ); snake_case inside, the same JSON as Android. Older files lack it. */
  dj?: import("./dj-api").DjSettings;
  /** Exercise mode (Settings → Exercise); snake_case inside, the same JSON as Android. Older files lack it. */
  exercise?: import("./exercise-api").ExerciseSettings;
  /** The version that ran last: the backend owns it (`set_settings` keeps the stored value). */
  last_seen_version: string | null;
}

export interface MiniSettings {
  /** The synced lyrics: the line being sung, big, and the next one. */
  lyrics: boolean;
  /** HomeFito dances to the music. */
  mascot: boolean;
}

// ---------------------------------------------------------------- updates (src-tauri/src/updater.rs)

export interface UpdateSettings {
  /** On launch and every 6 hours. */
  auto_check: boolean;
  /** Beta versions too (GitHub pre-releases). */
  prereleases: boolean;
  /** `ask`: a banner offers it; `idle`: downloaded and installed when nothing plays, downloads or is shared. */
  install: "ask" | "idle";
  /** Show the notes of the new version the first time HomeFi opens after updating. */
  whats_new: boolean;
}

// ---------------------------------------------------------------- what's new (src-tauri/src/whats_new.rs)

export interface NoteSpan {
  text: string;
  bold: boolean;
  code: boolean;
  /** http(s) only. */
  link: string | null;
}
export interface NoteLine {
  spans: NoteSpan[];
}
export interface NoteSection {
  title: string;
  items: NoteLine[];
}
export interface ReleaseNotes {
  version: string;
  /** yyyy-mm-dd or "". */
  date: string;
  summary: NoteLine | null;
  sections: NoteSection[];
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
  /** Suggestion cards (a sound for what you listen to most). */
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
  /** Bass / treble / clarity sliders inside Edit curve. */
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

/** One thing a sound applies to, and when / by whom it was added. */
export interface TasteTarget {
  scope: TasteScope;
  /** Unix ms; 0 = unknown (migrated from 1.5.0). */
  addedMs: number;
  addedBy: TasteCreator;
}

/** A sound: a curve (tilt and / or up to 8 bands) for its targets, on its outputs (none = every output).
 * Precedence: song > album > artist > playlist > genre > everything else (the output's own curve). */
export interface TasteRule {
  id: string;
  label: string;
  tilt: Tilt;
  /** The parametric curve (absent on simple and old rules). */
  bands?: TasteBand[];
  /** Why, in the assistant's words. */
  explanation?: string;
  createdBy: TasteCreator;
  /** Kept for old files; no toggle in the UI. */
  enabled: boolean;
  targets: TasteTarget[];
  /** Output ids (desktop: device names); absent / empty = all outputs. */
  outputs?: string[];
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

/** How big a step the assistant and reactions take: subtle 1.5–3 dB, balanced 3–5 dB, bold 5–8 dB. */
export type TasteIntensity = "subtle" | "balanced" | "bold";

export interface TasteSettings {
  /** Compat only: playback ignores it. The master switch is `settings.eq.enabled`. */
  enabled: boolean;
  rules: TasteRule[];
  intensity: TasteIntensity;
  /** Settings format; always send back what you received. */
  version?: number;
}

/** What a turn or a reaction changed in the taste rules: merge it into the current ones (upsert by id or scope, remove by id). */
export interface TasteChange {
  upsert: TasteRule[];
  remove: string[];
  /** Taste layers switched on / off, or null when untouched. */
  enabled: boolean | null;
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

/** A target that moved from one sound to another (for Undo). */
export interface TargetMove {
  target: TasteScope;
  /** Id of the sound it came from. */
  from: string;
}
export interface SavedSound {
  rule: TasteRule;
  moves: TargetMove[];
}
export type TargetSource = "library" | "tidal";
export interface TargetHit {
  scope: TasteScope;
  /** "Song · Steely Dan", "Artist", "Playlist · 48 tracks". */
  subtitle: string;
  source: TargetSource;
  /** Id of the sound already holding it on the current output. */
  inSound: string | null;
}

/** The track playing and the sound it plays with. */
export interface NowSound {
  context: NowPlayingContext | null;
  /** Most specific first: track, album, artist, playlist, genre. */
  scopes: TasteScope[];
  /** The sound playing now (the Try while auditioning). */
  rule: TasteRule | null;
  /** Which of its targets matched. */
  target: TasteScope | null;
  /** The output id it resolves for. */
  output: string | null;
  auditioning: boolean;
}

export type SoundReaction = "needsBass" | "tooBoomy" | "tooBright" | "dull" | "vocalsBuried" | "tooQuiet" | "tooLoud" | "perfect";

/** What a reaction changed (already saved and applied by the backend). */
export interface ReactionResult {
  rule: TasteRule | null;
  taste: TasteSettings;
  tasteChange: TasteChange;
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

// ---------------------------------------------------------------- HomeFi playlists (concert)
// homefi-core's `playlist` and `concert` modules (serde shape, snake_case). Saved only in
// HomeFi (`~/.homefi/playlists`); nothing is written to the Tidal account.

export type EntryTag = "always" | "usually" | "sometimes" | "encore" | "plain";
export interface PlaylistEntry {
  title: string;
  artist: string;
  album: string;
  tidal_id: string | null;
  isrc: string | null;
  local_path: string | null;
  duration_ms: number;
  explicit: boolean;
  cover_url: string | null;
  tag: EntryTag;
  /** "Played in 7 of 8 shows · encore", already in the build language. */
  note: string;
}
export interface SongCandidate {
  title: string;
  artist: string;
  tag: EntryTag;
  note: string;
}
export interface SourceLink {
  url: string;
  title: string;
}
export interface ConcertSong {
  name: string;
  original_artist: string | null;
  tape: boolean;
  encore: boolean;
}
export interface ConcertShow {
  id: string;
  /** ISO yyyy-mm-dd. */
  date: string;
  venue: string;
  city: string;
  country: string;
  tour: string;
  url: string | null;
  songs: ConcertSong[];
}
export type ConcertMode = { kind: "tour" } | { kind: "show"; show_id: string };
export type SetlistSource = { kind: "ai"; provider_id: string; model: string } | { kind: "tidal_popular" };
export interface ConcertThresholds {
  always_pct: number;
  usually_pct: number;
  sometimes_pct: number;
  encore_pct: number;
}
export interface ConcertOrigin {
  artist: string;
  source: SetlistSource;
  mode: ConcertMode;
  shows_wanted: number;
  thresholds: ConcertThresholds;
  /** Every show found, most recent first (empty for Tidal's most played). */
  shows: ConcertShow[];
  fetched_ms: number;
}
// "Make a party playlist": the questions, and what the AI's session answers (homefi-core `party`).
export type PartyGuests = "friends" | "family" | "coworkers" | "mixed";
export type AgeGroup = "kids" | "teens" | "young_adults" | "adults" | "seniors";
export type PartyVibe = "background" | "chill" | "dance" | "singalong" | "throwback";
export type EnergyCurve = "arc" | "rising" | "steady";
/** The energy curves with their words (party questions, settings, chat summary). */
export const ENERGY_CURVES: { id: EnergyCurve; label: string }[] = [
  { id: "arc", label: "Warm up, peak, wind down" },
  { id: "rising", label: "Keep building" },
  { id: "steady", label: "Steady" },
];
export interface PartyBrief {
  guests: PartyGuests;
  /** Empty = any age. */
  ages: AgeGroup[];
  vibe: PartyVibe;
  /** 30..=480, rounded to 15. */
  minutes: number;
  curve: EnergyCurve;
  allow_explicit: boolean;
  notes: string;
}
export interface PartyOrigin {
  brief: PartyBrief;
  provider_id: string;
  model: string;
  built_ms: number;
}
// "Make a playlist with AI": the same session as the party, with what the user said in the chat as the brief.
export interface ChatBrief {
  /** The user's first message: the playlist's description. */
  wish: string;
  /** The AI's short title; empty until it sets one. */
  title: string;
  /** 30..=480, rounded to 15. */
  minutes: number;
  curve: EnergyCurve;
  allow_explicit: boolean;
  /** Songs the user named, as the AI corrected them: they open the playlist. */
  songs?: { title: string; artist: string }[];
}
/** The song “Playlist like this” starts from (what the app knows of it). */
export interface SongSeed {
  title: string;
  artist: string;
  album?: string;
  year?: number;
  genre?: string;
}
export interface ChatOrigin {
  brief: ChatBrief;
  provider_id: string;
  model: string;
  built_ms: number;
}
export type PlaylistOrigin = ({ kind: "concert" } & ConcertOrigin) | ({ kind: "party" } & PartyOrigin) | ({ kind: "chat" } & ChatOrigin);
export interface PartyTurn {
  text: string;
  quick_replies: string[];
  /** The brief as the AI understands it so far (chat mode only). */
  chat_brief?: ChatBrief | null;
  /** The conversation's title as the core sees it (the AI chats menu). */
  title?: string;
}
export interface PartyBuilt {
  playlist: HomePlaylist;
  note: string | null;
}
export type PartyProgress = { kind: "asking"; round: number } | { kind: "resolving"; done: number; total: number };
export interface HomePlaylist {
  version: number;
  id: string;
  name: string;
  description: string;
  created_ms: number;
  updated_ms: number;
  origin: PlaylistOrigin;
  entries: PlaylistEntry[];
  unmatched: SongCandidate[];
  sources: SourceLink[];
  /** Gemini only: Google's Search Suggestions HTML, shown with the grounded result. */
  search_widget_html: string | null;
  /** The copy saved in the user's TIDAL account; missing/null until it's saved there. */
  tidal_uuid?: string | null;
}
/** What saving a HomeFi playlist to TIDAL did (src-tauri/src/playlists.rs). */
export interface TidalSaveReport {
  uuid: string;
  /** Made now (first save, or the old copy was deleted on TIDAL). */
  created: boolean;
  saved: number;
  /** "Title · Artist" of the songs TIDAL doesn't have. */
  skipped: string[];
  /** "Saved 24 songs to TIDAL · 2 aren't on TIDAL". */
  summary: string;
}
export interface TidalSaved {
  /** As stored now, with its `tidal_uuid`. */
  playlist: HomePlaylist;
  report: TidalSaveReport;
}
/** How an automatic save to TIDAL went (event `home-playlist-tidal`). */
export type TidalSyncEvent = ({ kind: "saved" } & TidalSaved) | { kind: "failed"; id: string; message: string };
export interface ConcertRequest {
  artist: string;
  shows: number;
  thresholds: ConcertThresholds;
  mode: ConcertMode;
  /** Skip the AI: Tidal's most played. */
  popular_only: boolean;
}
export interface ConcertBuilt {
  playlist: HomePlaylist;
  fallback_note: string | null;
}
export type ConcertProgress =
  | { kind: "searching"; with_ai: boolean }
  | { kind: "shows_found"; count: number }
  | { kind: "resolving"; done: number; total: number };
export interface ConcertSources {
  /** null: no AI set up (or its key is missing). */
  ai: { provider_id: string; provider_name: string; model: string; web_search: boolean } | null;
  /** Signed in to Tidal. */
  tidal: boolean;
}
export interface PlaylistSettings {
  /** Recent shows the AI looks up (3..15). */
  concert_shows: number;
  concert_thresholds: ConcertThresholds;
  prefer_library: boolean;
  /** Web searches the AI may run per build (1..10). */
  search_budget: number;
  skip_tape: boolean;
  covers_original_artist: boolean;
  show_sources: boolean;
  home_section: boolean;
  /** Default answers on the party screen. */
  party_minutes: number;
  party_curve: EnergyCurve;
  party_allow_explicit: boolean;
  /** Songs by one artist, at most (1..10). */
  party_max_per_artist: number;
  /** Defaults of "Make a playlist with AI"; the chat can change them. */
  chat_minutes: number;
  chat_curve: EnergyCurve;
  chat_allow_explicit: boolean;
  /** Offer “Playlist like this” in song menus (default true). */
  chat_from_song: boolean;
  /** Most AI chats open at once, 1..10 (default 3). */
  chat_max_open: number;
  /** A notice when a chat's playlist finishes or fails while another screen is shown (default true). */
  chat_done_notice: boolean;
  /** Saving a HomeFi playlist also saves it to the TIDAL account (default false). */
  tidal_auto_save: boolean;
  /** Deleting a HomeFi playlist also deletes its TIDAL copy (default false). */
  tidal_delete_with: boolean;
}
/** Slider ranges (the core clamps again at use). */
export const CONCERT_SHOWS_RANGE = { min: 3, max: 15 } as const;
export const THRESHOLD_PCT_RANGE = { min: 5, max: 100, step: 5 } as const;
export const SEARCH_BUDGET_RANGE = { min: 1, max: 10 } as const;
export const PARTY_MINUTES_RANGE = { min: 30, max: 480, step: 15 } as const;
export const PER_ARTIST_RANGE = { min: 1, max: 10 } as const;
const MIN_PER_HOUR = 60;
/** A party's length as "2 h", "1 h 30 min", "45 min" (`long`: "2 hours", for screen readers). */
export function partyLength(minutes: number, long = false): string {
  const h = Math.floor(minutes / MIN_PER_HOUR);
  const m = minutes % MIN_PER_HOUR;
  const hours = long ? `${h} ${h === 1 ? "hour" : "hours"}` : `${h} h`;
  const mins = long ? `${m} ${m === 1 ? "minute" : "minutes"}` : `${m} min`;
  return !h ? mins : m ? `${hours} ${mins}` : hours;
}
/** Chats open at once (Settings → Playlists); the backend clamps the same range. */
export const CHAT_OPEN_RANGE = { min: 1, max: 10 } as const;

export const DEFAULT_PLAYLIST_SETTINGS: PlaylistSettings = {
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
  chat_max_open: 3,
  chat_done_notice: true,
  tidal_auto_save: false,
  tidal_delete_with: false,
};

// ---------------------------------------------------------------- discs (src-tauri/src/disc.rs, crates/homefi-disc)
// Audio CDs are read and ripped on this computer; the rules (secure reading, offsets, encoders,
// file names) live in homefi-disc. The UI draws state and sends intents.

export type DiscMedia = "none" | "audio" | "data" | "protected" | "unknown";
export type DiscProtection = "css" | "cppm" | "aacs";
export interface DriveView {
  id: string;
  name: string;
  media: DiscMedia;
  tracks?: number;
  lengthMs?: number;
  protection?: DiscProtection;
  /** Where a data disc is mounted (to open its folder). */
  mount?: string;
}
export type DiscSource = "cdText" | "tidal" | "musicbrainz" | "ai" | "manual";
export interface AlbumMeta {
  title: string;
  artist: string;
  year: number | null;
  genre: string | null;
  upc: string | null;
  discNumber: number;
  discTotal: number;
  tidalAlbumId: number | null;
  mbid: string | null;
}
export interface TrackMeta {
  number: number;
  title: string;
  artists: string[];
  isrc: string | null;
  tidalTrackId: number | null;
}
/** What was found about a disc (on the disc itself, or by a lookup the user turned on). */
export interface DiscMatch {
  source: DiscSource;
  confidence: number;
  album: AlbumMeta;
  tracks: TrackMeta[];
  cover: { kind: "tidal"; id: string } | { kind: "url"; url: string } | null;
  note: string | null;
}
export interface DiscView {
  drive: DriveView;
  toc: { tracks: { number: number; startLba: number; endLba: number; preEmphasis: boolean }[]; leadoutLba: number };
  ids: { musicbrainz: string; freedb: string; ctdb: string };
  mcn: string | null;
  /** The names on the disc (CD-Text), when it has them. */
  cdText: DiscMatch | null;
}
export type RipFormat = { kind: "flac"; level: number } | { kind: "alac" } | { kind: "wav" } | { kind: "mp3"; mp3: "cbr320" | "v0" } | { kind: "aac"; kbps: number } | { kind: "opus"; kbps: number };
export type RipFormatKind = RipFormat["kind"];
export interface TrackRead {
  samplesTotal: number;
  rereads: number;
  suspiciousSamples: number;
  unreadableSamples: number;
  c2Errors: number;
}
export type RipProgress =
  | { type: "started"; track: number; title: string }
  | { type: "reading"; track: number; doneSectors: number; totalSectors: number; rereads: number; suspiciousSamples: number }
  | { type: "encoding"; track: number; percent: number }
  | { type: "verifying"; track: number }
  | { type: "trackDone"; track: number; path: string; read: TrackRead; verified: string | null }
  | { type: "trackFailed"; track: number; message: string }
  | { type: "finished"; ok: number; failed: number; logPath: string | null };
/** What the user decided at rip time (blanks already filled in: "Disc 1", "Unknown artist", "Track 01"). */
export interface RipRequest {
  drive: string;
  tracks: number[];
  meta: DiscMatch;
  format: RipFormat;
  /** An image file chosen for the cover, or null. */
  coverFile: string | null;
}
export interface RipSummary {
  ok: number;
  failed: number;
  logPath: string | null;
  /** The folder the album was written to. */
  albumDir: string | null;
}
export interface IdentifySources {
  cdText: boolean;
  tidal: boolean;
  musicbrainz: boolean;
  ai: boolean;
}
export type DiscEvent = { type: "inserted"; drive: DriveView } | { type: "removed"; drive: DriveView } | { type: "isrcs"; drive: DriveView; isrcs: (string | null)[] };

export type DiscOpenOnInsert = "notice" | "open" | "off";
export type DiscRailItem = "whenInserted" | "always" | "never";
export type DiscReadMode = "secure" | "fast";
export type DiscC2 = "auto" | "on" | "off";
export type DiscSpeed = "max" | "auto" | number;
export interface DiscSettings {
  watch: boolean;
  watchSeconds: number;
  openOnInsert: DiscOpenOnInsert;
  railItem: DiscRailItem;
  format: RipFormat;
  flacVerify: boolean;
  mode: DiscReadMode;
  agree: number;
  maxRereads: number;
  c2: DiscC2;
  /** Read offset in samples, per drive id (people have more than one drive). */
  offsets: Record<string, number>;
  speed: DiscSpeed;
  onUnreadable: "keep" | "skip";
  sources: IdentifySources;
  cover: { embed: boolean; saveFile: boolean; source: string };
  /** Null: the downloads folder. */
  ripDir: string | null;
  /** Null: the downloads template. */
  template: string | null;
  writeLog: boolean;
  writeCue: boolean;
  ejectWhenDone: boolean;
  hideCdOnlyNote: boolean;
}
export const DISC_WATCH_SECONDS_RANGE = { min: 2, max: 30 } as const;
export const DISC_AGREE_RANGE = { min: 2, max: 4 } as const;
export const DISC_REREADS_RANGE = { min: 1, max: 50 } as const;
export const DISC_OFFSET_RANGE = { min: -2000, max: 2000 } as const;
export const DISC_FLAC_LEVEL_RANGE = { min: 0, max: 8 } as const;
export const DEFAULT_DISC_SETTINGS: DiscSettings = {
  watch: true,
  watchSeconds: 3,
  openOnInsert: "notice",
  railItem: "whenInserted",
  format: { kind: "flac", level: 5 },
  flacVerify: true,
  mode: "secure",
  agree: 2,
  maxRereads: 20,
  c2: "auto",
  offsets: {},
  speed: "auto",
  onUnreadable: "keep",
  sources: { cdText: true, tidal: false, musicbrainz: false, ai: false },
  cover: { embed: true, saveFile: false, source: "best" },
  ripDir: null,
  template: null,
  writeLog: true,
  writeCue: false,
  ejectWhenDone: true,
  hideCdOnlyNote: false,
};
/** The setting with defaults for whatever an older settings file doesn't have. */
export const discSettingsOf = (s: Settings | null | undefined): DiscSettings => ({ ...DEFAULT_DISC_SETTINGS, ...s?.disc, sources: { ...DEFAULT_DISC_SETTINGS.sources, ...s?.disc?.sources }, cover: { ...DEFAULT_DISC_SETTINGS.cover, ...s?.disc?.cover } });

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
  /** The provider can search the web from its API (needed by "Get ready for a concert"). */
  web_search: boolean;
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
  taste_change?: TasteChange | null;
  /** Loudness changed this turn (already saved and applied). */
  loudness?: LoudnessSettings | null;
  /** A sound proposed this turn (not saved yet): draw its curve and band notes; Try / Keep it. */
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
  /** The mini player's window takes this size (logical px), keeping its bottom-right corner. */
  miniResize: (width: number, height: number) => invoke<void>("mini_resize", { width, height }),
  moveTrack: (from: number, to: number) => invoke<void>("player_move", { from, to }),
  shuffle: (on: boolean) => invoke<void>("player_shuffle", { on }),
  repeat: (mode: Repeat) => invoke<void>("player_repeat", { mode }),
  volume: (volume: number) => invoke<void>("player_volume", { volume }),
  /** Silences the sound without touching the saved volume. */
  mute: (muted: boolean) => invoke<void>("player_mute", { muted }),
  isMuted: () => invoke<boolean>("player_muted"),
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
  concertSources: () => invoke<ConcertSources>("concert_sources"),
  concertBuild: (request: ConcertRequest, replaceId: string | null, language: "en" | "es", onProgress: (p: ConcertProgress) => void) => {
    const channel = new Channel<ConcertProgress>();
    channel.onmessage = onProgress;
    return invoke<ConcertBuilt>("concert_build", { request, replaceId, language, onProgress: channel });
  },
  concertRearrange: (playlist: HomePlaylist, request: ConcertRequest, language: "en" | "es", onProgress: (p: ConcertProgress) => void) => {
    const channel = new Channel<ConcertProgress>();
    channel.onmessage = onProgress;
    return invoke<HomePlaylist>("concert_rearrange", { playlist, request, language, onProgress: channel });
  },
  concertCancel: () => invoke<void>("concert_cancel"),
  partyDefaultBrief: () => invoke<PartyBrief>("party_default_brief"),
  chatDefaultBrief: () => invoke<ChatBrief>("chat_default_brief"),
  chatSongWish: (seed: SongSeed, language: "en" | "es") => invoke<string>("chat_song_wish", { seed, language }),
  // every AI chat is its own session in the backend, by the id the UI gave it (`conversation`)
  chatOpen: (conversation: string, brief: ChatBrief, language: "en" | "es") => invoke<string>("chat_open", { conversation, brief, language }),
  partyOpen: (conversation: string, brief: PartyBrief, language: "en" | "es") => invoke<string>("party_open", { conversation, brief, language }),
  partySetBrief: (conversation: string, brief: PartyBrief) => invoke<string>("party_set_brief", { conversation, brief }),
  partyStart: (conversation: string) => invoke<PartyTurn>("party_start", { conversation }),
  partyReply: (conversation: string, text: string) => invoke<PartyTurn>("party_reply", { conversation, text }),
  partyBuild: (conversation: string, replaceId: string | null, onProgress: (p: PartyProgress) => void) => {
    const channel = new Channel<PartyProgress>();
    channel.onmessage = onProgress;
    return invoke<PartyBuilt>("party_build", { conversation, replaceId, onProgress: channel });
  },
  partyCancel: (conversation: string) => invoke<void>("party_cancel", { conversation }),
  partyClose: (conversation: string) => invoke<void>("party_close", { conversation }),
  homePlaylists: () => invoke<HomePlaylist[]>("home_playlists"),
  // discs
  discDrives: () => invoke<DriveView[]>("disc_drives"),
  discOpen: (drive: string | null) => invoke<DiscView>("disc_open", { drive }),
  discIdentify: (drive: string, sources: IdentifySources) => invoke<DiscMatch[]>("disc_identify", { drive, sources }),
  discRip: (request: RipRequest, onProgress: (p: RipProgress) => void) => {
    const channel = new Channel<RipProgress>();
    channel.onmessage = onProgress;
    return invoke<RipSummary>("disc_rip", { request, onProgress: channel });
  },
  discCancel: () => invoke<void>("disc_cancel"),
  discEject: (drive: string) => invoke<void>("disc_eject", { drive }),
  /** Answers the playlist as stored (it keeps a TIDAL copy the given one didn't know about). */
  homePlaylistSave: (playlist: HomePlaylist) => invoke<HomePlaylist>("home_playlist_save", { playlist }),
  /** "Save to TIDAL" / "Update on TIDAL": creates or replaces its copy in the TIDAL account. */
  homePlaylistSaveToTidal: (id: string, language: "en" | "es") => invoke<TidalSaved>("home_playlist_save_to_tidal", { id, language }),
  homePlaylistDelete: (id: string) => invoke<boolean>("home_playlist_delete", { id }),
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
  tastePreview: (rule: TasteRule, output: string | null = null) => invoke<TastePreview>("taste_preview", { rule, output }),
  tasteAssign: (soundId: string, scope: TasteScope) => invoke<TargetMove[]>("taste_assign", { soundId, scope }),
  tasteUnassign: (soundId: string, scope: TasteScope) => invoke<void>("taste_unassign", { soundId, scope }),
  tasteUndoMoves: (moves: TargetMove[]) => invoke<void>("taste_undo_moves", { moves }),
  tasteSetOutputs: (soundId: string, outputs: string[]) => invoke<TargetMove[]>("taste_set_outputs", { soundId, outputs }),
  /** New sound, rename, curve edits, "New sound from this song". */
  tasteSave: (rule: TasteRule) => invoke<SavedSound>("taste_save", { rule }),
  /** Delete sound. */
  tasteRemove: (soundId: string) => invoke<boolean>("taste_remove", { soundId }),
  /** false: nothing plays or Sound is off. */
  tasteAudition: (rule: TasteRule | null) => invoke<boolean>("taste_audition", { rule }),
  tasteKeep: (rule: TasteRule, scope: TasteScope, onlyOn: string | null = null) => invoke<SavedSound>("taste_keep", { rule, scope, onlyOn }),
  /** At most 30, ranked. */
  soundSearchTargets: (query: string) => invoke<TargetHit[]>("sound_search_targets", { query }),
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

  // what's new after an update (src-tauri/src/whats_new.rs): the notes are embedded in the app
  /** The notes to show now: the first call per launch decides and records the version; later calls return []. */
  whatsNewCheck: () => invoke<ReleaseNotes[]>("whats_new_check"),
  /** The notes of the version that is running (Settings → About → What's new). */
  whatsNewLatest: () => invoke<ReleaseNotes | null>("whats_new_latest"),
};

export const on = {
  player: (f: (s: PlayerState) => void): Promise<UnlistenFn> => listen<PlayerState>("player", (e) => f(e.payload)),
  download: (f: (e: DownloadEvent) => void): Promise<UnlistenFn> => listen<DownloadEvent>("download", (e) => f(e.payload)),
  settings: (f: (s: Settings) => void): Promise<UnlistenFn> => listen<Settings>("settings", (e) => f(e.payload)),
  library: (f: () => void): Promise<UnlistenFn> => listen("library", () => f()),
  muted: (f: (muted: boolean) => void): Promise<UnlistenFn> => listen<boolean>("muted", (e) => f(e.payload)),
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
  disc: (f: (e: DiscEvent) => void): Promise<UnlistenFn> => listen<DiscEvent>("disc", (e) => f(e.payload)),
  homePlaylistTidal: (f: (e: TidalSyncEvent) => void): Promise<UnlistenFn> => listen<TidalSyncEvent>("home-playlist-tidal", (e) => f(e.payload)),
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
