//! Desktop preferences, stored in `<app dir>/desktop.json` (the app dir and
//! Tidal credentials are shared with the `homefi` CLI).

use std::path::{Path, PathBuf};

use homefi_core::dsp::{CrossfadeSettings, EqProfile, EqSettings, LoudnessSettings, TasteSettings};
use homefi_core::quality::TrackQuality;
use homefi_core::share::{AddMode, EditMode, Permissions, QueueOrder, SkipMode};
use homefi_player::OutputOptions;
use serde::{Deserialize, Serialize};

/// What the listening room shows. Everything can be turned off.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoomSettings {
    pub signal_chain: bool,
    /// How the signal chain looks: `compact` (small stages joined by a
    /// flowing signal, details on hover) or `detailed` (the big boxes).
    pub signal_style: String,
    /// The links of the signal chain flow while the music plays.
    pub signal_animate: bool,
    pub vu_meters: bool,
    pub visualizer: bool,
    pub halo: bool,
    pub mascot: bool,
    /// `auto` (from the genre) or a fixed style: `mosh`, `pogo`, `cute`, `party`, `salsa`, `groove`, `chill`.
    pub mascot_style: String,
    /// `auto` (the genre's groove, famous dances now and then) or one famous dance all the time.
    pub mascot_dance: String,
    /// How often famous dances come up: `off`, `sometimes` or `often`.
    pub mascot_specials: String,
    /// Arms and legs: `white`, `navy`, `accent` or `off`.
    pub mascot_limbs: String,
    /// Turning sideways to show the moves in profile: `off`, `dances` (only
    /// the famous dances) or `all` (the everyday grooves too).
    pub mascot_turns: String,
    /// Width in pixels; 0 sizes it to fit the room.
    pub mascot_size: u32,
    /// Dragged out of the room: it floats at `mascot_x` / `mascot_y` (fractions of the window).
    pub mascot_free: bool,
    pub mascot_x: f32,
    pub mascot_y: f32,
    /// While floating, keep it on every screen, not only the room.
    pub mascot_everywhere: bool,
    /// Right-click on the mascot: `menu` (its dances, wave, next style),
    /// `style` (the next style) or `off`.
    pub mascot_hold: String,
    /// A dances button next to the mascot.
    pub mascot_dance_button: bool,
    /// Clicking it a lot makes it angry: `off`, `patient` (10 quick clicks),
    /// `normal` (6) or `short` (4).
    pub mascot_temper: String,
    /// Tint the interface with the colors of the album art.
    pub album_colors: bool,
    /// Look of the now-playing room, as on Android: `classic`, `immersive`,
    /// `vinyl`, `studio`, `retro_amp`, `luna`, `aluminum` or `web2000`.
    pub layout: String,
    /// The layout is a skin for the whole app (rail, deck, lists, settings),
    /// not only the now-playing room.
    pub layout_everywhere: bool,
    /// Immersive: the shade under the title over the album, so it reads on
    /// any cover: `auto` (darker on light covers), `strong`, `soft` or `off`.
    pub immersive_shade: String,
    /// Lyrics view: `normal` (the whole text, the current line lit) or
    /// `karaoke` (the line being sung, big, filling as it goes).
    pub lyrics_mode: String,
    /// Text size of the lyrics: `small`, `medium` or `large`.
    pub lyrics_size: String,
    /// Open the lyrics on their own when a song that has them starts.
    pub lyrics_auto_open: bool,
    /// The mascot sings into a microphone in karaoke.
    pub lyrics_mascot: bool,
    /// Shift of the lyrics against the music in ms (Bluetooth delay);
    /// positive shows them later. −2000..2000.
    pub lyrics_offset_ms: i32,
    /// How much the app moves: `auto` (full, or subtle when the system asks
    /// for reduced motion), `full`, `subtle` (short fades only) or `off`.
    pub motion: String,
    /// How wide the Settings screen reads: `comfortable` (a centred column,
    /// two columns of groups on very wide windows) or `wide` (one broader column).
    pub settings_width: String,
}

impl Default for RoomSettings {
    fn default() -> Self {
        Self {
            signal_chain: true,
            signal_style: "compact".into(),
            signal_animate: true,
            vu_meters: true,
            visualizer: true,
            halo: true,
            mascot: true,
            mascot_style: "auto".into(),
            mascot_dance: "auto".into(),
            mascot_specials: "sometimes".into(),
            mascot_limbs: "white".into(),
            mascot_turns: "all".into(),
            mascot_size: 0,
            mascot_free: false,
            mascot_x: 0.85,
            mascot_y: 0.75,
            mascot_everywhere: false,
            mascot_hold: "menu".into(),
            mascot_dance_button: true,
            mascot_temper: "normal".into(),
            album_colors: true,
            layout: "classic".into(),
            layout_everywhere: true,
            immersive_shade: "auto".into(),
            lyrics_mode: "normal".into(),
            lyrics_size: "medium".into(),
            lyrics_auto_open: false,
            lyrics_mascot: true,
            lyrics_offset_ms: 0,
            motion: "auto".into(),
            settings_width: "comfortable".into(),
        }
    }
}

/// Shared control and party mode (the same options as Android's, minus the
/// phone-only radios: Wi-Fi Direct group, Bluetooth). The desktop always shares
/// over the local network.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShareSettings {
    /// Blank uses "<name>'s music".
    pub session_name: String,
    /// How others see you; blank uses the computer's name.
    pub name: String,
    /// `#RRGGBB`
    pub color: String,
    /// Keep `code` instead of a random one each session.
    pub fixed_code: bool,
    pub code: String,
    pub port: u16,
    /// The share panel starts on party mode (guests play the music too).
    pub party_default: bool,
    pub add: AddMode,
    pub max_per_guest: u32,
    pub order: QueueOrder,
    pub skip: SkipMode,
    /// Guests may pause / resume and change the volume.
    pub playback: bool,
    /// Which queued songs guests may remove or move.
    pub edit: EditMode,
    pub allow_explicit: bool,
    pub drop_songs_on_leave: bool,
    pub notice_joined: bool,
    pub notice_left: bool,
    pub notice_added: bool,
    pub notice_approval: bool,
    /// Seconds a notice stays on screen.
    pub notice_seconds: u32,
    /// Look for sessions on the network while the app is open.
    pub discover: bool,
    /// Offer a session found nearby on any screen, not only in the listening room.
    pub discover_everywhere: bool,
    /// Announce the session on the network (mDNS / Bonjour), so others find it without typing.
    pub announce_lan: bool,
    /// As a guest, play the hub's music on this computer when it streams.
    pub play_along: bool,
    /// This computer's speaker latency (Bluetooth...), as hub and as guest; may be negative.
    pub speaker_delay_ms: i32,
    /// Playing along: close small gaps by playing a touch faster or slower. The desktop
    /// player can't change speed yet, so here it only jumps; kept for parity with Android.
    pub sync_by_speed: bool,
    /// Playing along: download the next and previous songs ahead and play them from here.
    pub preload_songs: bool,
    /// As a guest, open on the player with the hub's song; off, on the room (queue and controls).
    pub guest_opens_player: bool,
    /// Party mode, as hub: at most this long (seconds) a new song waits for every guest to
    /// have it loaded, so all start together. 0 starts at once.
    pub party_start_wait_sec: u32,
    /// As hub: send the music's levels so guests' visualizers and VU meters move.
    pub send_levels: bool,
}

impl Default for ShareSettings {
    fn default() -> Self {
        Self {
            session_name: String::new(),
            name: String::new(),
            color: "#0891B2".into(),
            fixed_code: false,
            code: String::new(),
            port: homefi_core::share::DEFAULT_PORT,
            party_default: false,
            add: AddMode::Free,
            max_per_guest: 3,
            order: QueueOrder::Turns,
            skip: SkipMode::Vote,
            playback: false,
            edit: EditMode::All,
            allow_explicit: true,
            drop_songs_on_leave: false,
            notice_joined: true,
            notice_left: true,
            notice_added: true,
            notice_approval: true,
            notice_seconds: 4,
            discover: true,
            discover_everywhere: true,
            announce_lan: true,
            play_along: true,
            speaker_delay_ms: 0,
            sync_by_speed: true,
            preload_songs: true,
            guest_opens_player: true,
            party_start_wait_sec: 3,
            send_levels: true,
        }
    }
}

impl ShareSettings {
    pub fn permissions(&self) -> Permissions {
        Permissions {
            add: self.add,
            max_per_guest: self.max_per_guest.max(1),
            order: self.order,
            skip: self.skip,
            playback: self.playback,
            edit: self.edit,
            allow_explicit: self.allow_explicit,
            drop_songs_on_leave: self.drop_songs_on_leave,
        }
    }
}

/// Which AI the assistants use. Never the key: that one lives in the OS
/// keychain (see `ai.rs`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AiSettings {
    /// Provider id (`anthropic`, `openai`, `gemini`, `openrouter`, `mistral`,
    /// `groq`, `xai`, `deepseek`, `ollama`, `openai_compatible`); empty = no
    /// AI, the EQ assistant runs as the guided wizard.
    pub provider: String,
    /// Empty: the provider's default, or the best one it lists.
    pub model: String,
    /// Address of an Ollama or custom server; empty = the provider's own.
    pub base_url: String,
}

/// How the equalizer screen looks and behaves (desktop only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EqUiSettings {
    /// An "EQ" button next to the output in the player bar.
    pub deck_button: bool,
    /// "Hear without EQ": `hold` (while pressed) or `toggle` (click on, click off).
    pub ab_mode: String,
    /// Band numbers on the handles of the curve.
    pub band_labels: bool,
    /// Bass / treble / clarity sliders on the equalizer screen.
    pub taste_sliders: bool,
}

impl Default for EqUiSettings {
    fn default() -> Self {
        Self { deck_button: true, ab_mode: "hold".into(), band_labels: true, taste_sliders: true }
    }
}

/// "How does it sound?" and taste layers in the interface (desktop only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SoundUiSettings {
    /// The "How does it sound?" button in the player bar.
    pub react_deck: bool,
    /// ...and in the listening room.
    pub react_room: bool,
    /// "HD 650 + Electronic" next to the EQ button: which layers play.
    pub layer_chip: bool,
    /// Suggestion cards (a taste layer for what you listen to most).
    pub suggestions: bool,
    /// How a taste rule's editor opens: `auto` (the curve editor when the rule
    /// has bands, else the three sliders), `simple` or `advanced`.
    pub taste_editor: String,
}

impl Default for SoundUiSettings {
    fn default() -> Self {
        Self { react_deck: true, react_room: true, layer_chip: true, suggestions: true, taste_editor: "auto".into() }
    }
}

/// How updates are installed once downloaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum UpdateInstall {
    /// A banner offers it; the restart is the user's call.
    #[default]
    Ask,
    /// Downloaded by itself and installed after a few minutes with nothing
    /// playing, downloading or shared (HomeFi restarts).
    Idle,
}

/// Settings → About → Updates (the same options as Android's).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateSettings {
    /// Look for a new version on launch and every 6 hours.
    pub auto_check: bool,
    /// Offer beta versions too (GitHub pre-releases).
    pub prereleases: bool,
    pub install: UpdateInstall,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self { auto_check: true, prereleases: false, install: UpdateInstall::Ask }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DesktopSettings {
    pub download_dir: PathBuf,
    /// Folders scanned for the library (the download folder is always included).
    pub library_dirs: Vec<PathBuf>,
    pub quality: TrackQuality,
    pub embed_cover: bool,
    pub embed_lyrics: bool,
    pub skip_existing: bool,
    pub volume: f32,
    /// Output device name; `None` follows the system default.
    pub output_device: Option<String>,
    pub room: RoomSettings,
    /// `system`, `light` or `dark`.
    pub theme: String,
    /// Folders opened with "Open folder", newest first.
    pub recent_folders: Vec<PathBuf>,
    /// Seek bar look: `classic`, `wavy`, `waveform`, `neon`, `led` or `minimal`.
    pub progress_style: String,
    /// `thin`, `medium` or `thick`.
    pub progress_thickness: String,
    /// Animate the seek bar while music plays.
    pub progress_animated: bool,
    /// Tidal lists show their first page right away and load more while
    /// scrolling; off loads each list whole before showing it.
    pub lazy_lists: bool,
    pub share: ShareSettings,
    /// Version of the personal-use notice the user accepted before connecting
    /// Tidal; 0 = never. Asked again when the notice's version goes up.
    pub tidal_terms_accepted: u32,
    /// When it was accepted (Unix epoch, milliseconds).
    pub tidal_terms_accepted_at: Option<u64>,
    /// Equalizer (off by default): a profile per output device, AutoEq
    /// headphone corrections. camelCase inside, like on Android; an unreadable
    /// section falls back to the defaults instead of resetting every setting.
    #[serde(deserialize_with = "homefi_core::dsp::lenient_settings")]
    pub eq: EqSettings,
    /// Bring-your-own-key AI (the EQ assistant); the key is in the OS keychain.
    pub ai: AiSettings,
    /// The user's saved equalizer presets (camelCase inside, like `eq`).
    pub eq_presets: Vec<EqProfile>,
    pub eq_ui: EqUiSettings,
    /// Taste layers: tilts for a genre, playlist, artist, album or track on top
    /// of the output's profile (camelCase inside, like `eq`; off until the first rule).
    #[serde(deserialize_with = "homefi_core::dsp::lenient_taste")]
    pub taste: TasteSettings,
    /// Loudness normalization from ReplayGain (off: bit-perfect).
    #[serde(deserialize_with = "homefi_core::dsp::lenient_loudness")]
    pub loudness: LoudnessSettings,
    pub sound_ui: SoundUiSettings,
    /// One card per album on Tidal lists, and which release it shows.
    pub album_versions: homefi_core::album_versions::VersionPrefs,
    /// Crossfade between songs (off: gapless, bit-perfect). camelCase inside,
    /// the same JSON as Android's `crossfade_settings`.
    #[serde(deserialize_with = "homefi_core::dsp::lenient_crossfade")]
    pub crossfade: CrossfadeSettings,
    /// Surround devices (stereo on 5.1 / 7.1 speakers) and resampler quality.
    #[serde(deserialize_with = "lenient")]
    pub output: OutputOptions,
    /// Looking for and installing new versions.
    #[serde(deserialize_with = "lenient")]
    pub updates: UpdateSettings,
}

/// A section written by a newer version (unknown values) reads as its
/// defaults instead of resetting the whole file.
fn lenient<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Any<T> {
        Known(T),
        Other(serde::de::IgnoredAny),
    }
    Ok(match Any::<T>::deserialize(d)? {
        Any::Known(v) => v,
        Any::Other(_) => T::default(),
    })
}

impl Default for DesktopSettings {
    fn default() -> Self {
        let music = dirs::audio_dir().unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join("Music"));
        Self {
            download_dir: homefi_core::paths::default_download_dir(),
            library_dirs: vec![music],
            quality: TrackQuality::High,
            embed_cover: true,
            embed_lyrics: false,
            skip_existing: true,
            volume: 0.8,
            output_device: None,
            room: RoomSettings::default(),
            theme: "system".into(),
            recent_folders: Vec::new(),
            progress_style: "wavy".into(),
            progress_thickness: "medium".into(),
            progress_animated: true,
            lazy_lists: true,
            share: ShareSettings::default(),
            tidal_terms_accepted: 0,
            tidal_terms_accepted_at: None,
            eq: EqSettings::default(),
            ai: AiSettings::default(),
            eq_presets: Vec::new(),
            eq_ui: EqUiSettings::default(),
            taste: TasteSettings::default(),
            loudness: LoudnessSettings::default(),
            sound_ui: SoundUiSettings::default(),
            album_versions: Default::default(),
            crossfade: CrossfadeSettings::default(),
            output: OutputOptions::default(),
            updates: UpdateSettings::default(),
        }
    }
}

impl DesktopSettings {
    pub fn load(path: &Path) -> Self {
        std::fs::read(path).ok().and_then(|raw| serde_json::from_slice(&raw).ok()).unwrap_or_default()
    }

    /// Writes the file whole or not at all (a temp file renamed over it): a
    /// crash mid-write can't leave a broken file that would reset every setting.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&tmp, path)
    }

    pub fn library_roots(&self) -> Vec<PathBuf> {
        let mut roots = vec![self.download_dir.clone()];
        roots.extend(self.library_dirs.iter().cloned());
        roots
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_settings_files_get_the_share_defaults() {
        let old: DesktopSettings = serde_json::from_str(r#"{"theme":"dark","volume":0.5}"#).unwrap();
        assert_eq!(old.theme, "dark");
        assert_eq!(old.share, ShareSettings::default());
        assert!(old.lazy_lists);
        assert!(old.album_versions.group);
        // nobody has accepted the personal-use notice until they see it
        assert_eq!(old.tidal_terms_accepted, 0);
        assert_eq!(old.tidal_terms_accepted_at, None);
        // a partial share section keeps the rest at their defaults
        let some: DesktopSettings = serde_json::from_str(r#"{"share":{"skip":"everyone","port":0}}"#).unwrap();
        assert_eq!(some.share.skip, SkipMode::Everyone);
        assert_eq!(some.share.port, 0);
        assert_eq!(some.share.edit, EditMode::All);
        assert_eq!(some.share.party_start_wait_sec, 3);
    }

    #[test]
    fn old_room_settings_get_the_lyrics_defaults() {
        let old: DesktopSettings = serde_json::from_str(r#"{"room":{"layout":"luna"}}"#).unwrap();
        assert_eq!(old.room.layout, "luna");
        assert_eq!(old.room.lyrics_mode, "normal");
        assert_eq!(old.room.lyrics_size, "medium");
        assert!(!old.room.lyrics_auto_open);
        assert!(old.room.lyrics_mascot);
        assert_eq!(old.room.lyrics_offset_ms, 0);
        assert_eq!(old.room.motion, "auto");
        assert_eq!(old.room.settings_width, "comfortable");
    }

    #[test]
    fn the_equalizer_starts_off_and_survives_bad_data() {
        let old: DesktopSettings = serde_json::from_str(r#"{"theme":"dark"}"#).unwrap();
        assert_eq!(old.eq, EqSettings::default());
        assert!(!old.eq.enabled);
        let eq: DesktopSettings = serde_json::from_str(
            r#"{"eq":{"enabled":true,"perOutput":{"USB DAC":{"name":"HD 650","bands":[{"freqHz":105,"gainDb":6}]}}}}"#,
        )
        .unwrap();
        assert!(eq.eq.enabled && eq.eq.auto_preamp);
        assert_eq!(eq.eq.profile_for("USB DAC").map(|p| p.bands.len()), Some(1));
        // a broken eq section keeps the rest of the file
        let bad: DesktopSettings = serde_json::from_str(r#"{"theme":"dark","eq":{"enabled":"yes"}}"#).unwrap();
        assert_eq!(bad.theme, "dark");
        assert_eq!(bad.eq, EqSettings::default());
        assert!(bad.eq_presets.is_empty() && bad.eq_ui.deck_button && bad.eq_ui.ab_mode == "hold");
    }

    #[test]
    fn crossfade_and_surround_start_off_and_survive_bad_data() {
        use homefi_player::{CrossfadeMode, ResamplerQuality, SurroundMode};
        let old: DesktopSettings = serde_json::from_str(r#"{"theme":"dark"}"#).unwrap();
        assert!(!old.crossfade.is_on());
        assert_eq!(old.output, OutputOptions::default());
        assert_eq!(old.output.surround, SurroundMode::FrontOnly);
        let set: DesktopSettings = serde_json::from_str(
            r#"{"crossfade":{"mode":"on","seconds":40},"output":{"surround":"upmix","resampler":"fast"}}"#,
        )
        .unwrap();
        assert_eq!((set.crossfade.mode, set.crossfade.seconds), (CrossfadeMode::On, 12.0));
        assert_eq!((set.output.surround, set.output.resampler), (SurroundMode::Upmix, ResamplerQuality::Fast));
        let bad: DesktopSettings =
            serde_json::from_str(r#"{"theme":"dark","crossfade":7,"output":{"surround":"dolby atmos"}}"#).unwrap();
        assert_eq!(bad.theme, "dark");
        assert!(!bad.crossfade.is_on());
        assert_eq!(bad.output, OutputOptions::default());
    }

    #[test]
    fn updates_are_checked_and_asked_by_default() {
        let old: DesktopSettings = serde_json::from_str(r#"{"theme":"dark"}"#).unwrap();
        assert_eq!(old.updates, UpdateSettings { auto_check: true, prereleases: false, install: UpdateInstall::Ask });
        let set: DesktopSettings =
            serde_json::from_str(r#"{"updates":{"auto_check":false,"prereleases":true,"install":"idle"}}"#).unwrap();
        assert_eq!(set.updates, UpdateSettings { auto_check: false, prereleases: true, install: UpdateInstall::Idle });
        let bad: DesktopSettings = serde_json::from_str(r#"{"theme":"dark","updates":{"install":"always"}}"#).unwrap();
        assert_eq!((bad.theme.as_str(), bad.updates), ("dark", UpdateSettings::default()));
    }

    #[test]
    fn taste_and_loudness_start_off_and_survive_bad_data() {
        let old: DesktopSettings = serde_json::from_str(r#"{"theme":"dark"}"#).unwrap();
        assert!(!old.taste.enabled && old.taste.rules.is_empty());
        assert_eq!(old.loudness, LoudnessSettings::default());
        assert!(old.sound_ui.react_deck && old.sound_ui.layer_chip);
        let bad: DesktopSettings =
            serde_json::from_str(r#"{"theme":"dark","taste":{"rules":7},"loudness":{"mode":42}}"#).unwrap();
        assert_eq!(bad.theme, "dark");
        assert!(bad.taste.rules.is_empty());
        assert_eq!(bad.loudness.mode, homefi_core::dsp::LoudnessMode::Off);
    }
}
