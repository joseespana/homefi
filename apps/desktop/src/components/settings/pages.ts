// The Settings pages: what each is called, its icon, the words the search matches
// besides the title, and how the page list groups them.

export type PageId = "look" | "room" | "lyrics" | "mascot" | "sound" | "playback" | "dj" | "exercise" | "playlists" | "ai" | "share" | "downloads" | "discs" | "library" | "tidal" | "about";

export interface PageDef {
  id: PageId;
  title: string;
  icon: string;
  /** What the search box matches, besides the title. */
  words: string;
}

export const PAGES: PageDef[] = [
  { id: "look", title: "Look", icon: "palette", words: "appearance theme light dark system skin layout everywhere whole app progress bar seek wavy neon led thickness animate motion animation animations transitions effects movement reduce reduced still calm settings width wide comfortable" },
  { id: "room", title: "Listening room", icon: "disc", words: "layout player skin theme everywhere whole app look classic immersive studio aluminum cover flow retro amp winamp luna xp web 2000 vinyl signal chain path pipeline connectors flow flowing animate animation compact detailed source codec resample resampling output dac bit-perfect processed vu meters visualizer halo album colors shade shadow gradient readable title mini player pip floating window lyrics karaoke mascot homefito dance" },
  { id: "lyrics", title: "Lyrics", icon: "lyrics", words: "lyrics karaoke words sing singing song text lines synced lrc musixmatch size timing offset delay bluetooth late early open automatically mascot homefito microphone" },
  { id: "mascot", title: "Mascot", icon: "smile", words: "mascot dance style mood turns arms legs angry temper click right-click" },
  { id: "sound", title: "Sound", icon: "eq", words: "sound equalizer eq parametric bands preamp clipping autoeq headphones correction bass treble clarity taste curve parametric bands editor simple advanced bit-perfect bypass compare a/b presets player bar button set up my sound assistant guided taste layers rules genre playlist artist album how does it sound reactions intensity strength bold subtle balanced change size loudness normalization replaygain lufs volume suggestions" },
  { id: "playback", title: "Playback", icon: "soundwave", words: "playback crossfade cross fade transition transitions mix overlap gapless dj fade out fade in skip silence album surround 7.1 5.1 channels speakers upmix center subwoofer lfe rear resampler resampling quality cpu" },
  { id: "dj", title: "DJ", icon: "dj", words: "dj mode mix mixing decks deck turntable party beginner simple view show all controls sync tempo bpm beat grid match auto mix length beats bass swap crossfader curve smooth linear cut eq kill filter trim boost limiter clipping distortion loudness auto gain sampler pads effects air horn siren volume waveform colours colors zoom hot cues loops quantize snap nudge pitch bend keyboard shortcuts keys rebind latency buffer response memory crate free deck ask sidebar tips first mix layout arrange order sections hide pads sounds sampler setups save export import share google drive homefidj tablet phone" },
  { id: "exercise", title: "Exercise", icon: "bike", words: "exercise bike cycling indoor ride rpm cadence pedal spinning class workout training beginner mountain climb sprint interval style length genres tempo bpm half time double time tolerance cues countdown notice click beat pulse ring bar flash offset dark screen on tv window display port qr health notice medical sidebar saved rides my rides history favorites export import share google drive homefiride replay again" },
  { id: "playlists", title: "Playlists", icon: "queue", words: "playlists concert setlist tour live shows encore always usually sometimes thresholds percent web search sources google made by homefi library copies tapes intros covers chat chats open at once background notice ready mood ai playlist with ai party fiesta guests ages vibe energy explicit length per artist save to tidal account automatically sync copy every device delete tidal copy" },
  { id: "ai", title: "AI", icon: "ai", words: "ai key api openai chatgpt anthropic claude gemini google openrouter mistral groq xai grok deepseek ollama local model assistant keychain privacy provider server" },
  { id: "share", title: "Shared control & party", icon: "cast", words: "share shared control party hub guest join code qr port rules permissions add approve limit skip vote turns queue order explicit remove move notices speaker delay bluetooth sync play along nearby discover announce network mdns bonjour visualizer levels name colour color" },
  { id: "downloads", title: "Downloads", icon: "download", words: "quality lossless max hi-res flac aac cover lyrics skip existing folder" },
  { id: "discs", title: "Discs", icon: "cd", words: "disc cd rip ripper audio cd flac alac wav mp3 aac opus secure paranoid c2 offset speed eject cd-text musicbrainz verify cover drive dvd blu-ray copy protection sidebar insert notice log cue" },
  { id: "library", title: "Music folders", icon: "folder", words: "library folders music add remove scan" },
  { id: "tidal", title: "Tidal account", icon: "user", words: "tidal account login log out connect lists scroll lazy load pages playlists favorites infinite album versions duplicates same album group explicit clean edited dolby atmos spatial stereo remaster remastered original deluxe box set edition hi-res lossless default pick E badge" },
  { id: "about", title: "About", icon: "info", words: "about version ffmpeg data personal use terms notice disclaimer copyright accepted update updates updater upgrade new version check now automatically auto pre-release prerelease beta install idle restart download release notes what's new github privacy" },
];

/** The page list's captions, in order. */
export const NAV_GROUPS: { label: string; pages: PageId[] }[] = [
  { label: "Appearance", pages: ["look", "room", "lyrics", "mascot"] },
  { label: "Sound", pages: ["sound", "playback", "dj", "exercise", "playlists", "ai"] },
  { label: "Connections", pages: ["share", "downloads", "discs", "library", "tidal"] },
  { label: "App", pages: ["about"] },
];

export const isPageId = (id: unknown): id is PageId => PAGES.some((p) => p.id === id);
