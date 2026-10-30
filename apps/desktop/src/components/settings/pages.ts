// The Settings pages: what each is called, its icon, the words the search matches
// besides the title, and how the page list groups them.

export type PageId = "look" | "room" | "lyrics" | "mascot" | "sound" | "playback" | "ai" | "share" | "downloads" | "library" | "tidal" | "about";

export interface PageDef {
  id: PageId;
  title: string;
  icon: string;
  /** What the search box matches, besides the title. */
  words: string;
}

export const PAGES: PageDef[] = [
  { id: "look", title: "Look", icon: "palette", words: "appearance theme light dark system skin layout everywhere whole app progress bar seek wavy neon led thickness animate motion animation animations transitions effects movement reduce reduced still calm settings width wide comfortable" },
  { id: "room", title: "Listening room", icon: "disc", words: "layout player skin theme everywhere whole app look classic immersive studio aluminum cover flow retro amp winamp luna xp web 2000 vinyl signal chain path pipeline connectors flow flowing animate animation compact detailed source codec resample resampling output dac bit-perfect processed vu meters visualizer halo album colors shade shadow gradient readable title" },
  { id: "lyrics", title: "Lyrics", icon: "lyrics", words: "lyrics karaoke words sing singing song text lines synced lrc musixmatch size timing offset delay bluetooth late early open automatically mascot homefito microphone" },
  { id: "mascot", title: "Mascot", icon: "smile", words: "mascot dance style mood turns arms legs angry temper click right-click" },
  { id: "sound", title: "Sound", icon: "eq", words: "sound equalizer eq parametric bands preamp clipping autoeq headphones correction bass treble clarity taste curve parametric bands editor simple advanced bit-perfect bypass compare a/b presets player bar button set up my sound assistant guided taste layers rules genre playlist artist album how does it sound reactions intensity strength bold subtle balanced change size loudness normalization replaygain lufs volume suggestions" },
  { id: "playback", title: "Playback", icon: "soundwave", words: "playback crossfade cross fade transition transitions mix overlap gapless dj fade out fade in skip silence album surround 7.1 5.1 channels speakers upmix center subwoofer lfe rear resampler resampling quality cpu" },
  { id: "ai", title: "AI", icon: "ai", words: "ai key api openai chatgpt anthropic claude gemini google openrouter mistral groq xai grok deepseek ollama local model assistant keychain privacy provider server" },
  { id: "share", title: "Shared control & party", icon: "cast", words: "share shared control party hub guest join code qr port rules permissions add approve limit skip vote turns queue order explicit remove move notices speaker delay bluetooth sync play along nearby discover announce network mdns bonjour visualizer levels name colour color" },
  { id: "downloads", title: "Downloads", icon: "download", words: "quality lossless max hi-res flac aac cover lyrics skip existing folder" },
  { id: "library", title: "Music folders", icon: "folder", words: "library folders music add remove scan" },
  { id: "tidal", title: "Tidal account", icon: "user", words: "tidal account login log out connect lists scroll lazy load pages playlists favorites infinite album versions duplicates same album group explicit clean edited dolby atmos spatial stereo remaster remastered original deluxe box set edition hi-res lossless default pick E badge" },
  { id: "about", title: "About", icon: "info", words: "about version ffmpeg data personal use terms notice disclaimer copyright accepted update updates updater upgrade new version check now automatically auto pre-release prerelease beta install idle restart download release notes what's new github privacy" },
];

/** The page list's captions, in order. */
export const NAV_GROUPS: { label: string; pages: PageId[] }[] = [
  { label: "Appearance", pages: ["look", "room", "lyrics", "mascot"] },
  { label: "Sound", pages: ["sound", "playback", "ai"] },
  { label: "Connections", pages: ["share", "downloads", "library", "tidal"] },
  { label: "App", pages: ["about"] },
];

export const isPageId = (id: unknown): id is PageId => PAGES.some((p) => p.id === id);
