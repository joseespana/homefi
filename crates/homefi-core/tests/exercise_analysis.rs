//! Exercise mode with the DJ's real analysis on audio files (a kick on every beat): a library with no
//! `BPM` tag must still give every part the song's own beat, whatever the song and segment lengths.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use homefi_core::dj::{analyze_file, AnalysisCache, DjAnalysis};
use homefi_core::exercise::{
    build, cues::wav_16bit_mono, Analyzer, BpmSource, BuildContext, ExerciseSettings, PresetId, PulseSource,
    RideRequest, SourcesMode,
};
use homefi_core::humane::Lang;
use homefi_core::playlist::{LibraryIndex, LibraryRef, PlaylistEntry};

const RATE_HZ: u32 = 11_025;
const SONG_SECONDS: usize = 200;
const KICK_SAMPLES: usize = 400;
const MEMORY_MB: u32 = 256;

/// `SONG_SECONDS` of silence with a decaying low thump on every beat of `bpm`.
fn kick_track(bpm: f64) -> Vec<u8> {
    let mut samples = vec![0f32; RATE_HZ as usize * SONG_SECONDS];
    let beat = RATE_HZ as f64 * 60.0 / bpm;
    let mut at = 0.0;
    while (at as usize) < samples.len() {
        for k in 0..KICK_SAMPLES {
            if let Some(s) = samples.get_mut(at as usize + k) {
                let decay = 1.0 - k as f32 / KICK_SAMPLES as f32;
                *s = 0.9 * decay * (k as f32 * 0.05).sin();
            }
        }
        at += beat;
    }
    wav_16bit_mono(&samples, RATE_HZ)
}

/// What the apps do: decode the file and analyze it, with the on-disk cache.
struct FileAnalyzer {
    cache: AnalysisCache,
}

impl Analyzer for FileAnalyzer {
    fn analyze<'a>(
        &'a self,
        entry: &'a PlaylistEntry,
    ) -> Pin<Box<dyn Future<Output = Option<DjAnalysis>> + Send + 'a>> {
        Box::pin(async move {
            let path = entry.local_path.as_deref()?;
            analyze_file(std::path::Path::new(path), &self.cache, MEMORY_MB).ok()
        })
    }
}

#[tokio::test]
async fn untagged_songs_longer_than_their_parts_still_give_the_beat() {
    let dir = tempfile::tempdir().unwrap();
    let bpms = [72.0, 80.0, 88.0, 95.0, 100.0, 110.0, 120.0, 128.0];
    let songs: Vec<LibraryRef> = bpms
        .iter()
        .enumerate()
        .map(|(n, bpm)| {
            let path: PathBuf = dir.path().join(format!("{bpm}.wav"));
            std::fs::write(&path, kick_track(*bpm)).unwrap();
            LibraryRef {
                path: path.to_string_lossy().into_owned(),
                title: format!("Kick {bpm}"),
                artist: format!("Artist {n}"),
                album: "RideTest".into(),
                duration_ms: SONG_SECONDS as u64 * 1000,
                genre: "Rock".into(),
                ..LibraryRef::default()
            }
        })
        .collect();
    let settings = ExerciseSettings { sources: SourcesMode::LibraryOnly, ..ExerciseSettings::default() };
    let library = LibraryIndex::new(songs);
    let analyzer = FileAnalyzer { cache: AnalysisCache::new(dir.path().join("cache")) };
    let ctx = BuildContext {
        settings: &settings,
        ai: None,
        api: None,
        library: &library,
        analyzer: Some(&analyzer),
        lang: Lang::En,
    };
    let request = RideRequest {
        minutes: 10,
        style: PresetId::FirstRide,
        wish: homefi_core::exercise::MusicWish { genres: vec!["rock".into()], ..Default::default() },
        ..RideRequest::default()
    };
    let built = build(&request, &ctx, &|_| {}).await.unwrap();

    assert!(!built.plan.songs.is_empty());
    for song in &built.plan.songs {
        assert_eq!(song.source, Some(BpmSource::Analysis), "{:?} {:?}", song.entry.title, built.plan.notes);
        assert!(song.grid.is_some(), "{}: no beat grid; notes {:?}", song.entry.title, built.plan.notes);
        assert_eq!(song.pulse, PulseSource::SongBeat);
    }
    assert!(!built.plan.notes.iter().any(|n| n.contains("metronome")), "{:?}", built.plan.notes);
    let asked = 10 * 60;
    let got = built.plan.workout.total_seconds();
    assert!(got.abs_diff(asked) * 5 <= asked, "{got} s vs {asked} s");
}
