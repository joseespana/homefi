//! Throwaway library-scan benchmark (synthetic tiny WAV files, no real audio, no network).
//!
//!   cargo run --release -p homefi-core --example scan_bench -- gen  <dir> <count>
//!   cargo run --release -p homefi-core --example scan_bench -- scan <dir>
//!
//! `scan` uses `<dir>/index.json` as the cache and prints time, tracks and index bytes.
//! Run it twice for cold (no index) then warm (unchanged). Wrap with `/usr/bin/time -l` for peak RSS.

use std::path::PathBuf;
use std::time::Instant;

fn wav() -> Vec<u8> {
    let data = [0u8; 64];
    let mut v = Vec::new();
    v.extend(b"RIFF");
    v.extend((36 + data.len() as u32).to_le_bytes());
    v.extend(b"WAVEfmt ");
    v.extend(16u32.to_le_bytes());
    v.extend(1u16.to_le_bytes());
    v.extend(2u16.to_le_bytes());
    v.extend(44_100u32.to_le_bytes());
    v.extend((44_100u32 * 4).to_le_bytes());
    v.extend(4u16.to_le_bytes());
    v.extend(16u16.to_le_bytes());
    v.extend(b"data");
    v.extend((data.len() as u32).to_le_bytes());
    v.extend(data);
    v
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = PathBuf::from(&args[2]);
    match args[1].as_str() {
        "gen" => {
            let n: usize = args[3].parse().unwrap();
            let bytes = wav();
            for i in 0..n {
                let d =
                    dir.join("music").join(format!("artist{:04}", i / 100)).join(format!("album{:02}", (i / 10) % 10));
                std::fs::create_dir_all(&d).unwrap();
                std::fs::write(d.join(format!("track{i:06}.wav")), &bytes).unwrap();
            }
        }
        _ => {
            let index = dir.join("index.json");
            let t = Instant::now();
            let tracks = homefi_core::library::scan(&[dir.join("music")], Some(&index));
            let bytes = std::fs::metadata(&index).map(|m| m.len()).unwrap_or(0);
            println!("tracks={} ms={} index_bytes={bytes}", tracks.len(), t.elapsed().as_millis());
        }
    }
}
