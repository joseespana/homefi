//! Read-only probe for a real drive: `cargo run -p homefi-disc --example probe [driveId]`.
//! Lists drives, then for the chosen one prints media, TOC, ids, CD-Text, MCN, ISRC of track 1,
//! whether C2 works, and reads a few sectors twice to show that reads are stable.

use homefi_disc::source::{C2_BYTES, SECTOR_BYTES};
use homefi_disc::{DriveId, Media, SectorSource};

fn main() {
    let drives = match homefi_disc::list_drives() {
        Ok(drives) => drives,
        Err(error) => return println!("listing failed: {error}"),
    };
    if drives.is_empty() {
        return println!("no optical drives found");
    }
    for drive in &drives {
        println!("{:<16} {:<32} {:?}", drive.id.0, drive.name, drive.media);
    }
    let wanted = std::env::args().nth(1).map(DriveId);
    let Some(info) = drives.iter().find(|d| wanted.as_ref().is_none_or(|w| *w == d.id)) else {
        return println!("no such drive");
    };
    if !matches!(info.media, Media::AudioCd { .. }) {
        return println!("nothing more to probe: not an audio CD");
    }
    let mut drive = match homefi_disc::open(&info.id) {
        Ok(drive) => drive,
        Err(error) => return println!("open failed: {error}"),
    };
    let toc = drive.toc().expect("toc");
    println!("capabilities: {:?}", drive.capabilities());
    println!("musicbrainz {}  freedb {}  ctdb {}", toc.musicbrainz_id(), toc.freedb_id(), toc.ctdb_toc());
    for track in &toc.tracks {
        println!("  track {:>2}  lba {:>6}..{:<6} audio {}", track.number, track.start_lba, track.end_lba, track.audio);
    }
    println!("cd-text: {:?}", drive.cd_text());
    println!("mcn: {:?}  isrc(1): {:?}", drive.mcn(), drive.isrc(1));
    let first = toc.audio_tracks().next().expect("audio track");
    let lba = first.start_lba + 10;
    let (mut a, mut b) = (vec![0u8; 10 * SECTOR_BYTES], vec![0u8; 10 * SECTOR_BYTES]);
    let mut c2 = vec![0u8; 10 * C2_BYTES];
    let started = std::time::Instant::now();
    let flags = drive.read_audio(lba, 10, &mut a, Some(&mut c2));
    println!(
        "read 10 sectors at {lba}: {flags:?} in {:?}, c2 bits set: {}",
        started.elapsed(),
        c2.iter().filter(|b| **b != 0).count()
    );
    let again = drive.read_audio(lba, 10, &mut b, None);
    println!("second read: {again:?}, identical: {}", a == b);
}
