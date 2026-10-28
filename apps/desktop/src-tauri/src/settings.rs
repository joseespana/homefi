//! Desktop preferences, stored in `<app dir>/desktop.json` (the app dir and
//! Tidal credentials are shared with the `homefi` CLI).

use std::path::{Path, PathBuf};

use homefi_core::quality::TrackQuality;
use homefi_core::share::{AddMode, EditMode, Permissions, QueueOrder, SkipMode};
use serde::{Deserialize, Serialize};

/// What the listening room shows. Everything can be turned off.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoomSettings {
    pub signal_chain: bool,
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
    /// Right-click on the mascot switches to the next style.
    pub mascot_right_click: bool,
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
}

impl Default for RoomSettings {
    fn default() -> Self {
        Self {
            signal_chain: true,
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
            mascot_right_click: true,
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
        }
    }
}

impl DesktopSettings {
    pub fn load(path: &Path) -> Self {
        std::fs::read(path).ok().and_then(|raw| serde_json::from_slice(&raw).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        std::fs::write(path, serde_json::to_vec_pretty(self)?)
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
    }
}
