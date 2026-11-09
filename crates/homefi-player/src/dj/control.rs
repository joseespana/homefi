//! The vocabulary between the app and the DJ engine: what the user can do ([`DjControl`]) and
//! what the screen shows ([`DjSnapshot`]). Plain `serde` types (snake_case, tagged) so the Tauri
//! layer passes them through untouched.

use serde::{Deserialize, Serialize};

use homefi_core::dj::{CrossfaderCurve, HOT_CUES, PAD_COUNT};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeckId {
    A,
    B,
}

impl DeckId {
    pub const BOTH: [DeckId; 2] = [DeckId::A, DeckId::B];

    pub fn index(self) -> usize {
        match self {
            Self::A => 0,
            Self::B => 1,
        }
    }

    pub fn from_index(i: usize) -> Self {
        if i == 0 {
            Self::A
        } else {
            Self::B
        }
    }

    pub fn other(self) -> Self {
        match self {
            Self::A => Self::B,
            Self::B => Self::A,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DjEqBand {
    Low,
    Mid,
    High,
}

impl DjEqBand {
    pub fn index(self) -> usize {
        match self {
            Self::Low => 0,
            Self::Mid => 1,
            Self::High => 2,
        }
    }
}

/// Everything the user can do. `Copy`, so it travels through the lock-free command ring as is.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DjControl {
    Play {
        deck: DeckId,
    },
    Pause {
        deck: DeckId,
    },
    TogglePlay {
        deck: DeckId,
    },
    /// CDJ-style cue: while playing it returns to the cue point and stops; when stopped it sets the
    /// cue point (or, still on the cue point, previews while `pressed`).
    Cue {
        deck: DeckId,
        pressed: bool,
    },
    Seek {
        deck: DeckId,
        ms: f64,
    },
    /// Moves the playhead by `ms` (negative = back); a drag on the zoomed waveform.
    Nudge {
        deck: DeckId,
        ms: f64,
    },
    /// Pitch bend while held: -1 slows down, +1 speeds up, 0 releases.
    Bend {
        deck: DeckId,
        amount: f32,
    },
    /// Jumps to the hot cue, or sets it on the playhead when it is empty.
    HotCue {
        deck: DeckId,
        index: u8,
    },
    SetHotCue {
        deck: DeckId,
        index: u8,
    },
    ClearHotCue {
        deck: DeckId,
        index: u8,
    },
    /// Starts a loop of 1, 2, 4 or 8 beats at the playhead (needs a beat grid).
    LoopBeats {
        deck: DeckId,
        beats: u32,
    },
    LoopIn {
        deck: DeckId,
    },
    LoopOut {
        deck: DeckId,
    },
    LoopExit {
        deck: DeckId,
    },
    /// Tempo fader, percent (+3.5 plays 3.5 % faster); limited to the tempo range.
    Tempo {
        deck: DeckId,
        percent: f32,
    },
    /// Matches this deck's tempo and beats to the other deck and keeps following it.
    Sync {
        deck: DeckId,
    },
    SyncOff {
        deck: DeckId,
    },
    Trim {
        deck: DeckId,
        db: f32,
    },
    Eq {
        deck: DeckId,
        band: DjEqBand,
        db: f32,
    },
    EqKill {
        deck: DeckId,
        band: DjEqBand,
        on: bool,
    },
    /// Filter knob, -1 (low-pass) .. 1 (high-pass), 0 off.
    Filter {
        deck: DeckId,
        value: f32,
    },
    /// Channel fader, 0..=1.
    Volume {
        deck: DeckId,
        value: f32,
    },
    /// Unloads the deck.
    Eject {
        deck: DeckId,
    },
    /// Crossfader, 0 (deck A) ..= 1 (deck B). Touching it cancels an automatic transition.
    Crossfader {
        value: f32,
    },
    /// Master volume, 0..=1.5.
    Master {
        value: f32,
    },
    SetCurve {
        curve: CrossfaderCurve,
    },
    /// Brings `to` in over `beats` (8, 16 or 32; 0 = the setting): starts it on the beat in sync,
    /// then fades the crossfader and swaps the bass.
    AutoMix {
        to: DeckId,
        beats: u32,
    },
    AutoMixCancel,
    /// Plays a sampler pad (0..8).
    Sampler {
        pad: u8,
    },
    SamplerVolume {
        value: f32,
    },
}

/// A loaded song, as the screen needs to know it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DjTrackInfo {
    pub title: String,
    pub duration_ms: f64,
    pub sample_rate: u32,
    pub codec: String,
    /// 0 when no tempo was found.
    pub bpm: f64,
    pub first_beat_ms: f64,
    pub beat_ms: f64,
    pub has_grid: bool,
    pub confidence: f32,
    pub loudness_db: f32,
    pub auto_gain_db: f32,
    /// RAM the decoded song takes.
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeckSnapshot {
    pub track: Option<DjTrackInfo>,
    pub loading: bool,
    /// Why the last load failed, in words for the user.
    pub error: Option<String>,
    pub playing: bool,
    pub position_ms: f64,
    pub cue_ms: f64,
    pub hot_cues: [Option<f64>; HOT_CUES],
    /// Active loop `(start_ms, end_ms)`.
    pub loop_ms: Option<(f64, f64)>,
    pub tempo_percent: f32,
    /// Tempo of the song as it plays now (0 without a grid).
    pub current_bpm: f64,
    pub synced: bool,
    pub bend: f32,
    pub trim_db: f32,
    /// Knob positions (dB); a killed band keeps its knob.
    pub eq_db: [f32; 3],
    pub eq_kill: [bool; 3],
    pub filter: f32,
    pub volume: f32,
    /// Peak since the last snapshot, 0..=1 (left, right).
    pub peak: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct AutoMixSnapshot {
    pub to: DeckId,
    pub beats: u32,
    /// Waiting for the next beat of the outgoing song.
    pub armed: bool,
    /// 0..=1 through the transition.
    pub progress: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DjOutputInfo {
    pub device: String,
    pub sample_rate: u32,
    pub channels: u16,
    /// The fixed buffer the device accepted; `None` when it picked its own.
    pub buffer_frames: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DjSnapshot {
    pub running: bool,
    pub output: DjOutputInfo,
    /// The audio stream broke (device unplugged ...): DJ mode must be started again.
    pub device_lost: Option<String>,
    pub decks: [DeckSnapshot; 2],
    pub crossfader: f32,
    pub crossfader_curve: CrossfaderCurve,
    pub master: f32,
    pub master_peak: [f32; 2],
    /// The soft limiter touched the output since the last snapshot.
    pub limiting: bool,
    pub auto_mix: Option<AutoMixSnapshot>,
    pub sampler_volume: f32,
    /// Pads sounding now.
    pub pads_active: [bool; PAD_COUNT],
    pub memory_used_mb: u32,
    pub memory_limit_mb: u32,
}

/// Things the engine tells the app without being asked (loads finish on their own threads).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DjEvent {
    Loading { deck: DeckId },
    Loaded { deck: DeckId, track: DjTrackInfo },
    LoadFailed { deck: DeckId, message: String },
    DeviceLost { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_serialize_tagged_in_snake_case() {
        let c = DjControl::AutoMix { to: DeckId::B, beats: 16 };
        assert_eq!(serde_json::to_string(&c).unwrap(), r#"{"type":"auto_mix","to":"b","beats":16}"#);
        let back: DjControl = serde_json::from_str(r#"{"type":"eq_kill","deck":"a","band":"low","on":true}"#).unwrap();
        assert_eq!(back, DjControl::EqKill { deck: DeckId::A, band: DjEqBand::Low, on: true });
        let c: DjControl = serde_json::from_str(r#"{"type":"set_curve","curve":"cut"}"#).unwrap();
        assert_eq!(c, DjControl::SetCurve { curve: CrossfaderCurve::Cut });
        assert!(serde_json::from_str::<DjControl>(r#"{"type":"nope"}"#).is_err());
        assert_eq!(serde_json::to_string(&DjControl::AutoMixCancel).unwrap(), r#"{"type":"auto_mix_cancel"}"#);
    }

    #[test]
    fn deck_ids() {
        assert_eq!(DeckId::A.other(), DeckId::B);
        assert_eq!(DeckId::from_index(DeckId::B.index()), DeckId::B);
        assert_eq!(DjEqBand::High.index(), 2);
    }
}
