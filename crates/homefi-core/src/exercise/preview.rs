//! A quick listen to a song of the plan before riding it ("do I like it?"): a short window from the middle of the
//! song, where the chorus usually is, rather than its intro. One rule for every app.

/// How long a preview plays.
pub const PREVIEW_MS: u64 = 30_000;
/// Where it starts, as a share of the song: past the intro, near the first chorus.
const PREVIEW_AT: f64 = 0.35;

/// The part of a song a preview plays: from `from_ms`, for `length_ms`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewWindow {
    pub from_ms: u64,
    pub length_ms: u64,
}

/// The preview of a song `duration_ms` long: [`PREVIEW_MS`] from about a third in, moved back so it ends with
/// the song at the latest; a short song plays whole, and one of unknown length (0) from its start.
pub fn preview_window(duration_ms: u64) -> PreviewWindow {
    if duration_ms == 0 {
        return PreviewWindow { from_ms: 0, length_ms: PREVIEW_MS };
    }
    let at_ms = (duration_ms as f64 * PREVIEW_AT) as u64;
    let from_ms = at_ms.min(duration_ms.saturating_sub(PREVIEW_MS));
    PreviewWindow { from_ms, length_ms: PREVIEW_MS.min(duration_ms - from_ms) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_preview_starts_past_the_intro_and_ends_with_the_song_at_the_latest() {
        assert_eq!(preview_window(200_000), PreviewWindow { from_ms: 70_000, length_ms: PREVIEW_MS });
        // near the end: moved back so the whole window fits
        assert_eq!(preview_window(40_000), PreviewWindow { from_ms: 10_000, length_ms: PREVIEW_MS });
        // shorter than a preview: the whole song
        assert_eq!(preview_window(20_000), PreviewWindow { from_ms: 0, length_ms: 20_000 });
        // unknown length: from the start
        assert_eq!(preview_window(0), PreviewWindow { from_ms: 0, length_ms: PREVIEW_MS });
    }
}
