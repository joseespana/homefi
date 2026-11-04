//! The secure reader against disc images: every rule of `read_track` without a drive.

use homefi_disc::source::{FRAMES_PER_SECTOR, SAMPLES_PER_SECTOR, SECTOR_BYTES};
use homefi_disc::{
    read_track, C2Use, DiscError, FlakySource, ImageSource, ReadMode, ReadPlan, SectorSource, Toc, TrackRead,
};

const TRACKS: [u32; 3] = [60, 100, 45];

/// Every sample of the synthetic disc, built once.
fn disc() -> &'static [i16] {
    static DISC: std::sync::OnceLock<Vec<i16>> = std::sync::OnceLock::new();
    DISC.get_or_init(|| {
        let sectors: u32 = TRACKS.iter().sum();
        (0..sectors)
            .flat_map(ImageSource::synthetic_sector)
            .collect::<Vec<u8>>()
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect()
    })
}

/// What the reader must deliver for `track` of the synthetic disc with a read `offset`.
fn expected(toc: &Toc, track: u8, offset: i32) -> Vec<i16> {
    let entry = toc.track(track).unwrap();
    let frames = i64::from(entry.sectors()) * FRAMES_PER_SECTOR as i64;
    let first = i64::from(entry.start_lba) * FRAMES_PER_SECTOR as i64 + i64::from(offset);
    let disc = disc();
    let mut out = Vec::with_capacity(frames as usize * 2);
    for frame in first..first + frames {
        match usize::try_from(frame).ok().filter(|f| f * 2 < disc.len()) {
            Some(f) => out.extend_from_slice(&disc[f * 2..f * 2 + 2]),
            None => out.extend([0, 0]),
        }
    }
    out
}

fn run(src: &mut dyn SectorSource, track: u8, plan: &ReadPlan) -> (Vec<i16>, TrackRead) {
    let toc = src.toc().unwrap();
    let mut out = Vec::new();
    let read = read_track(
        src,
        &toc,
        track,
        plan,
        &mut |s| {
            out.extend_from_slice(s);
            true
        },
        &|_| {},
    )
    .unwrap();
    (out, read)
}

fn toc() -> Toc {
    ImageSource::synthetic(&TRACKS).toc().unwrap()
}

fn fast() -> ReadPlan {
    ReadPlan { mode: ReadMode::Fast, ..ReadPlan::default() }
}

#[test]
fn secure_mode_reproduces_the_image_bit_exactly() {
    let mut image = ImageSource::synthetic(&TRACKS);
    for track in 1..=3 {
        let (samples, read) = run(&mut image, track, &ReadPlan::default());
        assert_eq!(samples, expected(&toc(), track, 0), "track {track}");
        assert_eq!(read.rereads, 0);
        assert_eq!((read.unreadable_samples, read.suspicious_samples, read.c2_errors), (0, 0, 0));
        assert_eq!(read.samples_total, u64::from(TRACKS[track as usize - 1]) * FRAMES_PER_SECTOR as u64);
    }
}

#[test]
fn secure_mode_reads_twice_and_fast_mode_once() {
    let sectors: u32 = TRACKS[1];
    let mut secure = FlakySource::new(ImageSource::synthetic(&TRACKS));
    run(&mut secure, 2, &ReadPlan::default());
    assert_eq!(secure.total_reads(), 2 * sectors);
    let mut three = FlakySource::new(ImageSource::synthetic(&TRACKS));
    run(&mut three, 2, &ReadPlan { agree: 3, ..ReadPlan::default() });
    assert_eq!(three.total_reads(), 3 * sectors);
    let mut once = FlakySource::new(ImageSource::synthetic(&TRACKS));
    run(&mut once, 2, &fast());
    assert_eq!(once.total_reads(), sectors);
}

#[test]
fn a_bad_first_read_is_re_read_and_corrected() {
    let bad = 60 + 30; // inside track 2
    let mut flaky = FlakySource::new(ImageSource::synthetic(&TRACKS)).garbage_on(bad, 0);
    let (samples, read) = run(&mut flaky, 2, &ReadPlan::default());
    assert_eq!(samples, expected(&toc(), 2, 0));
    assert_eq!(read.rereads, 1);
    assert_eq!(read.unreadable_samples, 0);
}

#[test]
fn a_sector_that_never_agrees_is_counted_unreadable() {
    let bad = 60 + 10;
    let mut flaky = FlakySource::new(ImageSource::synthetic(&TRACKS)).always_garbage(bad);
    let plan = ReadPlan { max_rereads: 6, ..ReadPlan::default() };
    let (samples, read) = run(&mut flaky, 2, &plan);
    assert_eq!(read.unreadable_samples, FRAMES_PER_SECTOR as u64);
    assert_eq!(read.rereads, 6, "stops at the budget");
    // Everything else is exact and the track keeps its length.
    let good = expected(&toc(), 2, 0);
    assert_eq!(samples.len(), good.len());
    let hole = 10 * SAMPLES_PER_SECTOR..11 * SAMPLES_PER_SECTOR;
    assert_eq!(samples[..hole.start], good[..hole.start]);
    assert_eq!(samples[hole.end..], good[hole.end..]);
}

#[test]
fn a_sector_that_cannot_be_read_is_silence() {
    let bad = 60 + 5;
    for plan in [ReadPlan::default(), fast()] {
        let mut flaky = FlakySource::new(ImageSource::synthetic(&TRACKS)).always_error(bad);
        let (samples, read) = run(&mut flaky, 2, &ReadPlan { max_rereads: 3, ..plan });
        assert_eq!(read.unreadable_samples, FRAMES_PER_SECTOR as u64);
        let hole = 5 * SAMPLES_PER_SECTOR..6 * SAMPLES_PER_SECTOR;
        assert!(samples[hole].iter().all(|s| *s == 0));
        assert_eq!(samples.len(), expected(&toc(), 2, 0).len());
    }
}

#[test]
fn a_transient_read_error_is_retried() {
    let mut flaky = FlakySource::new(ImageSource::synthetic(&TRACKS)).error_on(60 + 7, 0);
    let (samples, read) = run(&mut flaky, 2, &ReadPlan::default());
    assert_eq!(samples, expected(&toc(), 2, 0));
    assert_eq!(read.unreadable_samples, 0);
    assert!(read.rereads >= 1);
}

#[test]
fn c2_bits_force_a_re_read_even_in_fast_mode() {
    let mut flaky = FlakySource::new(ImageSource::synthetic(&TRACKS).with_c2(true)).c2_on(60 + 12, 0);
    let (samples, read) = run(&mut flaky, 2, &fast());
    assert_eq!(samples, expected(&toc(), 2, 0));
    assert_eq!(read.rereads, 1);
    assert_eq!(read.c2_errors, 1);
    assert_eq!(read.suspicious_samples, 0);
    assert_eq!(flaky.reads_of(60 + 12), 2);
}

#[test]
fn c2_is_ignored_when_switched_off() {
    let mut flaky = FlakySource::new(ImageSource::synthetic(&TRACKS).with_c2(true)).always_c2(60 + 12);
    let (_, read) = run(&mut flaky, 2, &ReadPlan { c2: C2Use::Off, ..fast() });
    assert_eq!((read.rereads, read.c2_errors), (0, 0));
}

#[test]
fn persistent_c2_marks_the_sector_suspicious() {
    let mut flaky = FlakySource::new(ImageSource::synthetic(&TRACKS).with_c2(true)).always_c2(60 + 12);
    let plan = ReadPlan { max_rereads: 4, ..fast() };
    let (samples, read) = run(&mut flaky, 2, &plan);
    assert_eq!(samples, expected(&toc(), 2, 0), "the audio was right, only the flags were not");
    assert_eq!(read.suspicious_samples, FRAMES_PER_SECTOR as u64);
    assert_eq!(read.unreadable_samples, 0);
    assert_eq!(read.rereads, 4);
}

#[test]
fn offsets_shift_samples_across_sector_boundaries_and_zero_fill_the_edges() {
    let mut image = ImageSource::synthetic(&TRACKS);
    for offset in [30, -30, 588, -1176, 1, 2000, -2000] {
        for track in 1..=3 {
            let plan = ReadPlan { offset_samples: offset, ..ReadPlan::default() };
            let (samples, read) = run(&mut image, track, &plan);
            assert_eq!(samples, expected(&toc(), track, offset), "track {track} offset {offset}");
            assert_eq!(read.unreadable_samples, 0);
        }
    }
    // The first 30 samples of track 1 with offset -30 are silence; the last 30 of track 3 with +30.
    let (first, _) = run(&mut image, 1, &ReadPlan { offset_samples: -30, ..ReadPlan::default() });
    assert!(first[..60].iter().all(|s| *s == 0));
    let (last, _) = run(&mut image, 3, &ReadPlan { offset_samples: 30, ..ReadPlan::default() });
    assert!(last[last.len() - 60..].iter().all(|s| *s == 0));
}

#[test]
fn cancel_stops_within_one_chunk() {
    let mut flaky = FlakySource::new(ImageSource::synthetic(&TRACKS));
    let toc = flaky.toc().unwrap();
    let mut chunks = 0;
    let result = read_track(
        &mut flaky,
        &toc,
        2,
        &ReadPlan::default(),
        &mut |_| {
            chunks += 1;
            false
        },
        &|_| {},
    );
    assert!(matches!(result, Err(DiscError::Cancelled)));
    assert_eq!(chunks, 1);
    // One window, read twice (secure), then stop: nothing past the first 27 sectors.
    assert!(flaky.requests().iter().all(|(lba, count)| *lba + u32::from(*count) <= 60 + 27));
}

#[test]
fn fatal_errors_stop_the_read() {
    struct Gone(ImageSource);
    impl SectorSource for Gone {
        fn toc(&mut self) -> homefi_disc::Result<Toc> {
            self.0.toc()
        }
        fn read_audio(
            &mut self,
            _: u32,
            _: u16,
            _: &mut [u8],
            _: Option<&mut [u8]>,
        ) -> homefi_disc::Result<homefi_disc::ReadFlags> {
            Err(DiscError::Removed)
        }
        fn cd_text(&mut self) -> homefi_disc::Result<Option<homefi_disc::CdText>> {
            Ok(None)
        }
        fn isrc(&mut self, _: u8) -> homefi_disc::Result<Option<String>> {
            Ok(None)
        }
        fn mcn(&mut self) -> homefi_disc::Result<Option<String>> {
            Ok(None)
        }
        fn set_speed(&mut self, _: homefi_disc::ReadSpeed) -> homefi_disc::Result<()> {
            Ok(())
        }
        fn capabilities(&self) -> homefi_disc::Capabilities {
            homefi_disc::Capabilities::default()
        }
    }
    let mut gone = Gone(ImageSource::synthetic(&TRACKS));
    let toc = gone.toc().unwrap();
    let result = read_track(&mut gone, &toc, 1, &ReadPlan::default(), &mut |_| true, &|_| {});
    assert!(matches!(result, Err(DiscError::Removed)));
}

#[test]
fn bad_track_numbers_are_refused() {
    let mut image = ImageSource::synthetic(&TRACKS);
    let toc = image.toc().unwrap();
    let plan = ReadPlan::default();
    let result = read_track(&mut image, &toc, 9, &plan, &mut |_| true, &|_| {});
    assert!(matches!(result, Err(DiscError::NoSuchTrack(9))));
    let entries = [
        homefi_disc::RawTocEntry { number: 1, control: 0, start_lba: 0 },
        homefi_disc::RawTocEntry { number: 2, control: 4, start_lba: 60 },
    ];
    let mixed = Toc::from_entries(&entries, 160);
    let result = read_track(&mut image, &mixed, 2, &plan, &mut |_| true, &|_| {});
    assert!(matches!(result, Err(DiscError::NotAudioTrack(2))));
}

#[test]
fn progress_reaches_the_total() {
    let mut image = ImageSource::synthetic(&TRACKS);
    let toc = image.toc().unwrap();
    let last = std::cell::Cell::new(None);
    read_track(&mut image, &toc, 2, &ReadPlan::default(), &mut |_| true, &|p| last.set(Some(p))).unwrap();
    let p = last.get().unwrap();
    assert_eq!((p.track, p.done_sectors, p.total_sectors), (2, 100, 100));
}

#[test]
fn the_pipe_carries_a_whole_track() {
    use std::sync::atomic::AtomicBool;
    let (mut tx, rx) = homefi_disc::chunk_channel(4);
    let reader = std::thread::spawn(move || {
        let mut image = ImageSource::synthetic(&TRACKS);
        let toc = image.toc().unwrap();
        let cancel = AtomicBool::new(false);
        let mut sink = tx.sink(&cancel);
        read_track(&mut image, &toc, 2, &ReadPlan::default(), &mut sink, &|_| {})
    });
    let mut samples = Vec::new();
    while let Some(chunk) = rx.recv() {
        samples.extend_from_slice(&chunk.samples);
        rx.recycle(chunk);
    }
    reader.join().unwrap().unwrap();
    assert_eq!(samples, expected(&toc(), 2, 0));
    let _ = SECTOR_BYTES;
}
