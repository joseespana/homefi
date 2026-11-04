//! A rip from start to finish against disc images: no drive, real files in a temp folder, tags
//! read back with lofty, audio compared byte for byte with the image.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use homefi_disc::source::SECTOR_BYTES;
use homefi_disc::{
    rip, AlbumMeta, DiscError, DiscMatch, FlakySource, ImageSource, Mp3Kind, OnUnreadable, ReadPlan, RipFormat,
    RipOptions, RipOutput, RipProgress, RipRequest, SectorSource, Source, TrackMeta, Verified,
};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::prelude::*;
use lofty::tag::TagType;

const TRACKS: [u32; 3] = [60, 100, 45];
const TEMPLATE: &str = "{album.artist}/{album.title}/{item.number:02d} {item.title}";
const COVER: [u8; 6] = [0xFF, 0xD8, 0xFF, 0xE0, 0xFF, 0xD9];

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap()
}

fn meta() -> DiscMatch {
    DiscMatch {
        source: Source::Manual,
        confidence: 1.0,
        album: AlbumMeta {
            title: "Korn".into(),
            artist: "Korn".into(),
            year: Some(1994),
            genre: Some("Nu Metal".into()),
            ..AlbumMeta::default()
        },
        tracks: ["Blind", "Ball Tongue", "Need To"]
            .iter()
            .enumerate()
            .map(|(i, title)| TrackMeta {
                number: i as u8 + 1,
                title: (*title).into(),
                artists: vec!["Korn".into()],
                isrc: Some(format!("USSM1940000{}", i + 1)),
                ..TrackMeta::default()
            })
            .collect(),
        cover: None,
        note: None,
    }
}

fn options() -> RipOptions {
    RipOptions {
        flac_verify: true,
        on_unreadable: OnUnreadable::Keep,
        write_log: true,
        write_cue: true,
        save_cover_file: true,
    }
}

fn request(dir: &Path, format: RipFormat) -> RipRequest {
    RipRequest {
        drive_name: "Test drive".into(),
        tracks: vec![1, 2, 3],
        meta: meta(),
        format,
        plan: ReadPlan::default(),
        options: options(),
        out: RipOutput {
            dir: dir.to_path_buf(),
            template: TEMPLATE.into(),
            embed_cover: true,
            cover: Some(COVER.to_vec()),
        },
    }
}

struct Run {
    result: Result<homefi_disc::RipSummary, DiscError>,
    events: Vec<RipProgress>,
}

fn run(src: &mut dyn SectorSource, req: &RipRequest) -> Run {
    run_with(src, req, &AtomicBool::new(false), &|_| {})
}

fn run_with(
    src: &mut dyn SectorSource,
    req: &RipRequest,
    cancel: &AtomicBool,
    watch: &(dyn Fn(&RipProgress) + Sync),
) -> Run {
    let rt = runtime();
    let events = Mutex::new(Vec::new());
    let result = rip(src, req, cancel, rt.handle(), &|p| {
        watch(&p);
        events.lock().unwrap().push(p);
    });
    Run { result, events: events.into_inner().unwrap() }
}

/// Every byte of a track of the synthetic disc.
fn image_bytes(first_sector: u32, sectors: u32) -> Vec<u8> {
    (first_sector..first_sector + sectors).flat_map(ImageSource::synthetic_sector).collect()
}

fn track_start(n: usize) -> u32 {
    TRACKS[..n].iter().sum()
}

fn leftovers(dir: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.file_name().unwrap().to_string_lossy().starts_with("homefi-rip-") {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, &mut out);
    out
}

#[test]
fn wav_rip_is_bit_exact_named_by_the_template_and_tagged() {
    let dir = tempfile::tempdir().unwrap();
    let mut src = ImageSource::synthetic(&TRACKS);
    let outcome = run(&mut src, &request(dir.path(), RipFormat::Wav));
    let summary = outcome.result.unwrap();
    assert_eq!((summary.ok, summary.failed), (3, 0));
    let album = dir.path().join("Korn/Korn");
    assert_eq!(summary.album_dir.as_deref(), Some(album.as_path()));

    for (i, title) in ["Blind", "Ball Tongue", "Need To"].iter().enumerate() {
        let path = album.join(format!("{:02} {title}.wav", i + 1));
        let bytes = std::fs::read(&path).unwrap();
        let sectors = TRACKS[i];
        let data_start = bytes.windows(4).position(|w| w == b"data").unwrap() + 8;
        // lofty adds ID3/INFO chunks after the audio; the audio bytes are the first ones
        let audio = &bytes[data_start..data_start + sectors as usize * SECTOR_BYTES];
        assert_eq!(audio, image_bytes(track_start(i), sectors).as_slice(), "track {} audio", i + 1);

        let file = lofty::read_from_path(&path).unwrap();
        let tag = file.tag(TagType::Id3v2).expect("id3 tag in the wav");
        assert_eq!(tag.title().as_deref(), Some(*title));
        assert_eq!(tag.artist().as_deref(), Some("Korn"));
        assert_eq!(tag.album().as_deref(), Some("Korn"));
        assert_eq!(tag.track(), Some(i as u32 + 1));
        assert_eq!(tag.genre().as_deref(), Some("Nu Metal"));
        assert_eq!(tag.get_string(&lofty::tag::ItemKey::Isrc), Some(format!("USSM1940000{}", i + 1).as_str()));
        assert_eq!(tag.pictures().len(), 1, "cover embedded");
    }
    assert!(leftovers(dir.path()).is_empty(), "no work files left: {:?}", leftovers(dir.path()));
}

#[test]
fn flac_rip_is_verified_tagged_and_decodes_to_the_disc() {
    let dir = tempfile::tempdir().unwrap();
    let mut src = ImageSource::synthetic(&TRACKS);
    let outcome = run(&mut src, &request(dir.path(), RipFormat::Flac { level: 5 }));
    let summary = outcome.result.unwrap();
    assert_eq!((summary.ok, summary.failed), (3, 0));

    let album = dir.path().join("Korn/Korn");
    let path = album.join("02 Ball Tongue.flac");
    let file = lofty::read_from_path(&path).unwrap();
    let tag = file.primary_tag().expect("vorbis comments");
    assert_eq!(tag.title().as_deref(), Some("Ball Tongue"));
    assert_eq!(tag.track(), Some(2));
    assert_eq!(tag.get_string(&lofty::tag::ItemKey::AlbumArtist), Some("Korn"));
    assert_eq!(tag.get_string(&lofty::tag::ItemKey::Isrc), Some("USSM19400002"));
    assert_eq!(tag.get_string(&lofty::tag::ItemKey::RecordingDate), Some("1994-01-01"));
    assert_eq!(tag.pictures().len(), 1);
    assert_eq!(file.properties().sample_rate(), Some(44_100));
    assert_eq!(file.properties().channels(), Some(2));
    assert_eq!(file.properties().bit_depth(), Some(16));
    // 100 sectors x 588 frames
    let ms = file.properties().duration().as_millis();
    assert_eq!(ms, 100 * 588 * 1000 / 44_100);

    let done: Vec<_> = outcome
        .events
        .iter()
        .filter_map(|e| match e {
            RipProgress::TrackDone { track, verified, .. } => Some((*track, *verified)),
            _ => None,
        })
        .collect();
    assert_eq!(
        done,
        [(1, Some(Verified::FlacDecoded)), (2, Some(Verified::FlacDecoded)), (3, Some(Verified::FlacDecoded))]
    );

    // the log, the cue sheet and the cover file, as the settings ask
    let log = std::fs::read_to_string(album.join("Korn.homefi-rip.log")).unwrap();
    assert!(log.contains("Test drive") && log.contains("3 of 3 tracks saved") && log.contains("Ball Tongue"));
    assert_eq!(summary.log_path.as_deref(), Some(album.join("Korn.homefi-rip.log").as_path()));
    let cue = std::fs::read_to_string(album.join("Korn.cue")).unwrap();
    assert!(cue.contains("FILE \"03 Need To.flac\" WAVE") && cue.contains("ISRC USSM19400003"));
    assert_eq!(std::fs::read(album.join("cover.jpg")).unwrap(), COVER);
    assert!(leftovers(dir.path()).is_empty());
}

#[test]
fn flac_decoded_by_another_decoder_matches_the_wav_of_the_same_rip() {
    let dir = tempfile::tempdir().unwrap();
    let mut req = request(dir.path(), RipFormat::Flac { level: 8 });
    req.options.write_log = false;
    req.options.write_cue = false;
    req.tracks = vec![3];
    let mut src = ImageSource::synthetic(&TRACKS);
    run(&mut src, &req).result.unwrap();
    let path = dir.path().join("Korn/Korn/03 Need To.flac");

    // decode with symphonia, compare with the disc image
    let stream =
        symphonia::core::io::MediaSourceStream::new(Box::new(std::fs::File::open(&path).unwrap()), Default::default());
    let mut hint = symphonia::core::probe::Hint::new();
    hint.with_extension("flac");
    let probed =
        symphonia::default::get_probe().format(&hint, stream, &Default::default(), &Default::default()).unwrap();
    let mut format = probed.format;
    let mut decoder = symphonia::default::get_codecs()
        .make(&format.default_track().unwrap().codec_params, &Default::default())
        .unwrap();
    let mut pcm: Vec<i16> = Vec::new();
    while let Ok(packet) = format.next_packet() {
        let decoded = decoder.decode(&packet).unwrap();
        let mut buffer = symphonia::core::audio::SampleBuffer::<i16>::new(decoded.capacity() as u64, *decoded.spec());
        buffer.copy_interleaved_ref(decoded);
        pcm.extend_from_slice(buffer.samples());
    }
    let expected: Vec<i16> =
        image_bytes(track_start(2), TRACKS[2]).chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
    assert_eq!(pcm, expected);
}

#[test]
fn a_sector_that_lies_once_is_read_again_and_the_file_is_still_exact() {
    let dir = tempfile::tempdir().unwrap();
    let mut req = request(dir.path(), RipFormat::Wav);
    req.tracks = vec![2];
    let bad = track_start(1) + 30;
    let mut src = FlakySource::new(ImageSource::synthetic(&TRACKS)).garbage_on(bad, 0);
    let outcome = run(&mut src, &req);
    outcome.result.unwrap();
    let bytes = std::fs::read(dir.path().join("Korn/Korn/02 Ball Tongue.wav")).unwrap();
    let audio = &bytes[44..44 + TRACKS[1] as usize * SECTOR_BYTES];
    assert_eq!(audio, image_bytes(track_start(1), TRACKS[1]).as_slice());
    let read = outcome
        .events
        .iter()
        .find_map(|e| match e {
            RipProgress::TrackDone { read, .. } => Some(*read),
            _ => None,
        })
        .unwrap();
    assert!(read.rereads >= 1);
    assert_eq!(read.unreadable_samples, 0);
}

#[test]
fn a_damaged_track_is_kept_or_skipped_as_the_setting_says() {
    let bad = track_start(1) + 10;
    for (policy, saved) in [(OnUnreadable::Keep, true), (OnUnreadable::Skip, false)] {
        let dir = tempfile::tempdir().unwrap();
        let mut req = request(dir.path(), RipFormat::Flac { level: 5 });
        req.options.on_unreadable = policy;
        let mut src = FlakySource::new(ImageSource::synthetic(&TRACKS)).always_garbage(bad);
        let outcome = run(&mut src, &req);
        let summary = outcome.result.unwrap();
        let file = dir.path().join("Korn/Korn/02 Ball Tongue.flac");
        assert_eq!(file.exists(), saved, "{policy:?}");
        assert_eq!((summary.ok, summary.failed), if saved { (3, 0) } else { (2, 1) });
        assert!(
            dir.path().join("Korn/Korn/01 Blind.flac").exists()
                && dir.path().join("Korn/Korn/03 Need To.flac").exists()
        );
        let kept = outcome.events.iter().find_map(|e| match e {
            RipProgress::TrackDone { track: 2, read, .. } => Some(read.unreadable_samples),
            _ => None,
        });
        let failed = outcome.events.iter().any(|e| matches!(e, RipProgress::TrackFailed { track: 2, .. }));
        assert_eq!(kept.is_some(), saved);
        assert_eq!(failed, !saved);
        if saved {
            assert!(kept.unwrap() > 0);
            let log = std::fs::read_to_string(dir.path().join("Korn/Korn/Korn.homefi-rip.log")).unwrap();
            assert!(log.contains("damaged"));
        }
        assert!(leftovers(dir.path()).is_empty());
    }
}

#[test]
fn cancelling_keeps_the_finished_tracks_and_leaves_no_work_files() {
    let dir = tempfile::tempdir().unwrap();
    let req = request(dir.path(), RipFormat::Flac { level: 5 });
    let cancel = AtomicBool::new(false);
    let mut src = ImageSource::synthetic(&TRACKS);
    let outcome = run_with(&mut src, &req, &cancel, &|p| {
        if matches!(p, RipProgress::Started { track: 2, .. }) {
            cancel.store(true, Ordering::Relaxed);
        }
    });
    assert!(matches!(outcome.result, Err(DiscError::Cancelled)));
    assert!(dir.path().join("Korn/Korn/01 Blind.flac").exists());
    assert!(!dir.path().join("Korn/Korn/02 Ball Tongue.flac").exists());
    assert!(!dir.path().join("Korn/Korn/03 Need To.flac").exists());
    assert!(leftovers(dir.path()).is_empty(), "{:?}", leftovers(dir.path()));
    assert!(matches!(outcome.events.last(), Some(RipProgress::Finished { ok: 1, failed: 0, .. })));
}

/// A disc that is taken out of the drive after some reads.
struct Ejected<S> {
    inner: S,
    reads_left: u32,
}

impl<S: SectorSource> SectorSource for Ejected<S> {
    fn toc(&mut self) -> homefi_disc::Result<homefi_disc::Toc> {
        self.inner.toc()
    }
    fn read_audio(
        &mut self,
        lba: u32,
        count: u16,
        pcm: &mut [u8],
        c2: Option<&mut [u8]>,
    ) -> homefi_disc::Result<homefi_disc::ReadFlags> {
        if self.reads_left == 0 {
            return Err(DiscError::Removed);
        }
        self.reads_left -= 1;
        self.inner.read_audio(lba, count, pcm, c2)
    }
    fn cd_text(&mut self) -> homefi_disc::Result<Option<homefi_disc::CdText>> {
        self.inner.cd_text()
    }
    fn isrc(&mut self, track: u8) -> homefi_disc::Result<Option<String>> {
        self.inner.isrc(track)
    }
    fn mcn(&mut self) -> homefi_disc::Result<Option<String>> {
        self.inner.mcn()
    }
    fn set_speed(&mut self, speed: homefi_disc::ReadSpeed) -> homefi_disc::Result<()> {
        self.inner.set_speed(speed)
    }
    fn capabilities(&self) -> homefi_disc::Capabilities {
        self.inner.capabilities()
    }
}

#[test]
fn a_removed_disc_stops_the_rip_but_keeps_what_was_saved() {
    let dir = tempfile::tempdir().unwrap();
    let req = request(dir.path(), RipFormat::Wav);
    // track 1 (60 sectors, 27-sector windows, read twice in secure mode) is done well before this
    let mut src = Ejected { inner: ImageSource::synthetic(&TRACKS), reads_left: 12 };
    let outcome = run(&mut src, &req);
    assert!(matches!(outcome.result, Err(DiscError::Removed)), "{:?}", outcome.result);
    assert!(dir.path().join("Korn/Korn/01 Blind.wav").exists());
    assert!(!dir.path().join("Korn/Korn/02 Ball Tongue.wav").exists());
    assert!(leftovers(dir.path()).is_empty());
    assert!(outcome.events.iter().any(|e| matches!(e, RipProgress::TrackFailed { track: 2, .. })));
    assert!(matches!(outcome.events.last(), Some(RipProgress::Finished { ok: 1, failed: 1, .. })));
}

#[test]
fn tracks_with_the_same_name_do_not_overwrite_each_other() {
    let dir = tempfile::tempdir().unwrap();
    let mut req = request(dir.path(), RipFormat::Wav);
    req.out.template = "{album.title}/{item.title}".into();
    for track in &mut req.meta.tracks {
        track.title = "Untitled".into();
    }
    req.options.write_log = false;
    req.options.write_cue = false;
    let mut src = ImageSource::synthetic(&TRACKS);
    run(&mut src, &req).result.unwrap();
    let album = dir.path().join("Korn");
    assert!(
        album.join("Untitled.wav").exists()
            && album.join("Untitled (2).wav").exists()
            && album.join("Untitled (3).wav").exists()
    );
}

#[test]
fn a_bad_template_fails_before_anything_is_read() {
    let dir = tempfile::tempdir().unwrap();
    let mut req = request(dir.path(), RipFormat::Wav);
    req.out.template = "{nonsense.field}".into();
    let mut src = FlakySource::new(ImageSource::synthetic(&TRACKS));
    assert!(run(&mut src, &req).result.is_err());
    assert_eq!(src.total_reads(), 0);
    assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
}

#[test]
fn progress_is_ordered_and_reading_is_throttled() {
    let dir = tempfile::tempdir().unwrap();
    let mut req = request(dir.path(), RipFormat::Flac { level: 0 });
    req.tracks = vec![2];
    let mut src = ImageSource::synthetic(&TRACKS);
    let events = run(&mut src, &req).events;
    assert!(matches!(events.first(), Some(RipProgress::Started { track: 2, .. })));
    assert!(matches!(events.last(), Some(RipProgress::Finished { ok: 1, failed: 0, .. })));
    let reading = events.iter().filter(|e| matches!(e, RipProgress::Reading { .. })).count();
    assert!((1..=6).contains(&reading), "100 sectors in a blink: a handful of reports, got {reading}");
    let order: Vec<&str> = events
        .iter()
        .map(|e| match e {
            RipProgress::Started { .. } => "started",
            RipProgress::Reading { .. } => "reading",
            RipProgress::Encoding { .. } => "encoding",
            RipProgress::Verifying { .. } => "verifying",
            RipProgress::TrackDone { .. } => "done",
            RipProgress::TrackFailed { .. } => "failed",
            RipProgress::Finished { .. } => "finished",
        })
        .collect();
    let last_reading = order.iter().rposition(|e| *e == "reading").unwrap();
    assert!(order.iter().position(|e| *e == "encoding").unwrap() > 0);
    assert!(
        order.iter().position(|e| *e == "verifying").unwrap() > order.iter().position(|e| *e == "encoding").unwrap()
    );
    assert!(last_reading < order.iter().position(|e| *e == "done").unwrap());
}

#[test]
fn progress_serializes_the_way_the_screen_expects() {
    let reading =
        RipProgress::Reading { track: 4, done_sectors: 10, total_sectors: 20, rereads: 1, suspicious_samples: 2 };
    assert_eq!(
        serde_json::to_value(&reading).unwrap(),
        serde_json::json!({"type":"reading","track":4,"doneSectors":10,"totalSectors":20,"rereads":1,"suspiciousSamples":2})
    );
    let finished = RipProgress::Finished { ok: 1, failed: 0, log_path: None };
    assert_eq!(
        serde_json::to_value(&finished).unwrap(),
        serde_json::json!({"type":"finished","ok":1,"failed":0,"logPath":null})
    );
    let done = RipProgress::TrackDone {
        track: 1,
        path: "/a/b.flac".into(),
        read: Default::default(),
        verified: Some(Verified::FlacDecoded),
    };
    let json = serde_json::to_value(&done).unwrap();
    assert_eq!(json["type"], "trackDone");
    assert_eq!(json["verified"], "flacDecoded");
    assert_eq!(json["read"]["samplesTotal"], 0);
}

// ---- formats that go through the user's ffmpeg: only where ffmpeg can make them ----------------

fn ffmpeg_can(encoder: &str) -> bool {
    std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-encoders"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains(encoder))
        .unwrap_or(false)
}

#[test]
fn ffmpeg_formats_come_out_tagged_with_their_own_extension() {
    let cases = [
        (RipFormat::Alac, "alac", "m4a"),
        (RipFormat::Mp3 { kind: Mp3Kind::V0 }, "libmp3lame", "mp3"),
        (RipFormat::Aac { kbps: 192 }, " aac ", "m4a"),
        (RipFormat::Opus { kbps: 128 }, "libopus", "opus"),
    ];
    for (format, encoder, ext) in cases {
        if !ffmpeg_can(encoder) {
            eprintln!("skipping {format:?}: no ffmpeg encoder {encoder}");
            continue;
        }
        let dir = tempfile::tempdir().unwrap();
        let mut req = request(dir.path(), format);
        req.tracks = vec![3];
        let mut src = ImageSource::synthetic(&TRACKS);
        let summary = run(&mut src, &req).result.unwrap();
        assert_eq!((summary.ok, summary.failed), (1, 0), "{format:?}");
        let path = dir.path().join(format!("Korn/Korn/03 Need To.{ext}"));
        let file = lofty::read_from_path(&path).unwrap_or_else(|e| panic!("{format:?}: {e}"));
        let tag = file.primary_tag().unwrap_or_else(|| panic!("{format:?}: no tag"));
        assert_eq!(tag.title().as_deref(), Some("Need To"), "{format:?}");
        assert_eq!(tag.album().as_deref(), Some("Korn"), "{format:?}");
        assert_eq!(tag.track(), Some(3), "{format:?}");
        assert!(file.properties().duration().as_millis() >= 500, "{format:?}");
        assert!(leftovers(dir.path()).is_empty(), "{format:?}: {:?}", leftovers(dir.path()));
        assert!(!dir
            .path()
            .join("Korn/Korn")
            .read_dir()
            .unwrap()
            .flatten()
            .any(|e| e.path().extension().is_some_and(|x| x == "wav")));
    }
}
