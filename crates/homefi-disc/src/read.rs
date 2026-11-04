//! The secure reader: turns a [`SectorSource`] into the exact samples of a track.
//!
//! * `Secure` reads every window `agree` times and settles any sector whose reads differ (or
//!   whose C2 bits are set) by re-reading that sector until `agree` identical, C2-clean reads
//!   exist, up to `max_rereads` extra reads.
//! * `Fast` reads once and re-reads only sectors the drive flags with C2.
//! * The read offset shifts the stream by whole samples; sectors outside the audio area of the
//!   disc are zero-filled instead of read.
//!
//! Memory is bounded: a few 27-sector windows, allocated once per track. Nothing allocates
//! while reading.

use serde::{Deserialize, Serialize};

use crate::error::{DiscError, Result};
use crate::source::{
    ReadFlags, ReadSpeed, SectorSource, C2_BYTES, CHUNK_SECTORS, FRAMES_PER_SECTOR, SAMPLES_PER_SECTOR, SECTOR_BYTES,
};
use crate::toc::Toc;

/// Sectors between a re-read and the throw-away read that flushes the drive's cache. Caches
/// hold a few MB (about 1 700 sectors), so this is well past them.
const CACHE_BUST_DISTANCE: u32 = 4096;
/// Distinct versions of one sector kept while voting.
const MAX_CANDIDATES: usize = 4;
const WINDOW: usize = CHUNK_SECTORS as usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReadMode {
    /// Every sector read until `agree` identical reads agree, C2-clean.
    #[default]
    Secure,
    /// One read; C2-flagged sectors are re-read.
    Fast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum C2Use {
    /// Use C2 when the drive says it reports it.
    #[default]
    Auto,
    /// Ask for C2 even if the drive did not announce it; used only if the drive answers.
    On,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadPlan {
    pub mode: ReadMode,
    /// Drive read offset in samples (stereo frames). Positive = the drive reads ahead: the
    /// reader drops that many samples at the start and reads that many past the end.
    pub offset_samples: i32,
    pub c2: C2Use,
    /// Extra reads allowed per sector.
    pub max_rereads: u8,
    pub speed: ReadSpeed,
    /// Identical reads needed in secure mode (2..=4). Added to the contract, see
    /// contract-deviations.md.
    pub agree: u8,
}

impl Default for ReadPlan {
    fn default() -> Self {
        ReadPlan {
            mode: ReadMode::Secure,
            offset_samples: 0,
            c2: C2Use::Auto,
            max_rereads: 20,
            speed: ReadSpeed::Auto,
            agree: 2,
        }
    }
}

/// What reading a track found. Samples are stereo frames (588 per sector).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackRead {
    pub samples_total: u64,
    /// Extra reads beyond the baseline passes.
    pub rereads: u32,
    /// Samples delivered but still C2-flagged after every re-read.
    pub suspicious_samples: u64,
    /// Samples whose reads never agreed (or could not be read at all): the best candidate, or
    /// silence, was delivered in their place.
    pub unreadable_samples: u64,
    /// Sectors the drive flagged with C2 at least once.
    pub c2_errors: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadProgress {
    pub track: u8,
    pub done_sectors: u32,
    pub total_sectors: u32,
    pub rereads: u32,
    pub suspicious_samples: u64,
}

struct Candidate {
    data: Vec<u8>,
    votes: u8,
    /// At least one of its reads had clean C2 bits.
    clean: bool,
    used: bool,
}

struct Reader<'a> {
    src: &'a mut dyn SectorSource,
    use_c2: bool,
    /// Identical reads a sector needs.
    need: u8,
    max_rereads: u8,
    secure: bool,
    audio_end: u32,
    window: Vec<u8>,
    verify: Vec<u8>,
    c2_a: Vec<u8>,
    c2_b: Vec<u8>,
    scratch: Vec<u8>,
    candidates: Vec<Candidate>,
    stats: TrackRead,
}

fn c2_flagged(c2: &[u8], index: usize) -> bool {
    c2[index * C2_BYTES..(index + 1) * C2_BYTES].iter().any(|b| *b != 0)
}

fn vote(candidates: &mut [Candidate], data: &[u8], clean: bool) {
    if let Some(found) = candidates.iter_mut().find(|c| c.used && c.data == data) {
        found.votes = found.votes.saturating_add(1);
        found.clean |= clean;
        return;
    }
    // A free slot, else the weakest candidate gives way.
    let slot = candidates
        .iter()
        .position(|c| !c.used)
        .or_else(|| candidates.iter().enumerate().min_by_key(|(_, c)| c.votes).map(|(i, _)| i))
        .unwrap_or(0);
    let candidate = &mut candidates[slot];
    candidate.data.copy_from_slice(data);
    candidate.votes = 1;
    candidate.clean = clean;
    candidate.used = true;
}

fn best(candidates: &[Candidate]) -> Option<usize> {
    candidates
        .iter()
        .enumerate()
        .filter(|(_, c)| c.used)
        .max_by_key(|(i, c)| (c.votes, c.clean, std::cmp::Reverse(*i)))
        .map(|(i, _)| i)
}

impl Reader<'_> {
    /// Fills `window[dst..]` with `count` verified sectors starting at `lba`.
    fn read_window(&mut self, lba: u32, count: u16, dst: usize) -> Result<()> {
        let sectors = usize::from(count);
        let (pcm_len, c2_len) = (sectors * SECTOR_BYTES, sectors * C2_BYTES);
        let mut dirty = [false; WINDOW];
        let mut c2_a = [false; WINDOW];
        let mut c2_b = [false; WINDOW];

        let primary = self.src.read_audio(
            lba,
            count,
            &mut self.window[dst..dst + pcm_len],
            self.use_c2.then(|| &mut self.c2_a[..c2_len]),
        );
        let primary_ok = match primary {
            Ok(flags) => {
                if flags.c2 {
                    for i in 0..sectors {
                        c2_a[i] = c2_flagged(&self.c2_a, i);
                        dirty[i] |= c2_a[i];
                    }
                }
                true
            }
            Err(e) if e.is_fatal() => return Err(e),
            Err(_) => {
                dirty[..sectors].fill(true);
                false
            }
        };

        let mut verify_ok = false;
        if self.secure && primary_ok {
            verify_ok = true;
            for _ in 1..self.need {
                match self.src.read_audio(
                    lba,
                    count,
                    &mut self.verify[..pcm_len],
                    self.use_c2.then(|| &mut self.c2_b[..c2_len]),
                ) {
                    Ok(ReadFlags { c2 }) => {
                        for i in 0..sectors {
                            if c2 {
                                c2_b[i] |= c2_flagged(&self.c2_b, i);
                            }
                            let range = i * SECTOR_BYTES..(i + 1) * SECTOR_BYTES;
                            dirty[i] |=
                                c2_b[i] || self.window[dst + range.start..dst + range.end] != self.verify[range];
                        }
                    }
                    Err(e) if e.is_fatal() => return Err(e),
                    Err(_) => {
                        verify_ok = false;
                        dirty[..sectors].fill(true);
                        break;
                    }
                }
            }
        }

        for i in 0..sectors {
            if c2_a[i] || c2_b[i] {
                self.stats.c2_errors += 1;
            }
            if !dirty[i] {
                continue;
            }
            for c in &mut self.candidates {
                c.used = false;
            }
            let sector = dst + i * SECTOR_BYTES..dst + (i + 1) * SECTOR_BYTES;
            if primary_ok {
                vote(&mut self.candidates, &self.window[sector.clone()], !c2_a[i]);
            }
            if verify_ok {
                vote(&mut self.candidates, &self.verify[i * SECTOR_BYTES..(i + 1) * SECTOR_BYTES], !c2_b[i]);
            }
            self.settle(lba + i as u32, sector)?;
        }
        Ok(())
    }

    /// Re-reads one sector until the candidates agree, then writes the winner into the window.
    fn settle(&mut self, lba: u32, destination: std::ops::Range<usize>) -> Result<()> {
        let mut extra = 0u8;
        loop {
            if let Some(b) = best(&self.candidates) {
                let candidate = &self.candidates[b];
                if candidate.votes >= self.need && (candidate.clean || !self.use_c2) {
                    break;
                }
            }
            if extra >= self.max_rereads {
                break;
            }
            extra += 1;
            self.stats.rereads += 1;
            self.bust_cache(lba);
            let c2 = self.use_c2.then(|| &mut self.c2_a[..C2_BYTES]);
            match self.src.read_audio(lba, 1, &mut self.scratch, c2) {
                Ok(flags) => {
                    let clean = !(flags.c2 && c2_flagged(&self.c2_a, 0));
                    vote(&mut self.candidates, &self.scratch, clean);
                }
                Err(e) if e.is_fatal() => return Err(e),
                Err(_) => {}
            }
        }
        let frames = FRAMES_PER_SECTOR as u64;
        match best(&self.candidates) {
            None => {
                self.window[destination].fill(0);
                self.stats.unreadable_samples += frames;
            }
            Some(b) => {
                let candidate = &self.candidates[b];
                self.window[destination].copy_from_slice(&candidate.data);
                if candidate.votes < self.need {
                    self.stats.unreadable_samples += frames;
                } else if self.use_c2 && !candidate.clean {
                    self.stats.suspicious_samples += frames;
                }
            }
        }
        Ok(())
    }

    /// Drives answer repeat reads from their cache; reading somewhere far away first forces a
    /// real read. Errors are irrelevant here.
    fn bust_cache(&mut self, lba: u32) {
        let other = if lba + CACHE_BUST_DISTANCE < self.audio_end {
            lba + CACHE_BUST_DISTANCE
        } else if lba >= CACHE_BUST_DISTANCE {
            lba - CACHE_BUST_DISTANCE
        } else {
            return;
        };
        let _ = self.src.read_audio(other, 1, &mut self.scratch, None);
    }
}

/// Reads `track` and hands its samples (interleaved stereo `i16`) to `sink`, in pieces of at
/// most one window. `sink` returns `false` to stop: the read ends with [`DiscError::Cancelled`].
pub fn read_track(
    src: &mut dyn SectorSource,
    toc: &Toc,
    track: u8,
    plan: &ReadPlan,
    sink: &mut dyn FnMut(&[i16]) -> bool,
    progress: &dyn Fn(ReadProgress),
) -> Result<TrackRead> {
    let entry = toc.track(track).ok_or(DiscError::NoSuchTrack(track))?;
    if !entry.audio {
        return Err(DiscError::NotAudioTrack(track));
    }
    let _ = src.set_speed(plan.speed);
    let capabilities = src.capabilities();
    let use_c2 = match plan.c2 {
        C2Use::Off => false,
        C2Use::Auto => capabilities.c2,
        C2Use::On => true,
    };
    let secure = plan.mode == ReadMode::Secure;
    let frames_per_sector = FRAMES_PER_SECTOR as i64;
    let total_frames = u64::from(entry.sectors()) * FRAMES_PER_SECTOR as u64;
    let first_frame = i64::from(entry.start_lba) * frames_per_sector + i64::from(plan.offset_samples);
    let first_sector = first_frame.div_euclid(frames_per_sector);
    let mut skip = first_frame.rem_euclid(frames_per_sector) as usize;
    let sectors_needed = (skip as u64 + total_frames).div_ceil(FRAMES_PER_SECTOR as u64) as u32;

    let audio_end = i64::from(toc.audio_end_lba());
    let mut reader = Reader {
        src,
        use_c2,
        need: if secure { plan.agree.clamp(2, 4) } else { 1 },
        max_rereads: plan.max_rereads,
        secure,
        audio_end: audio_end as u32,
        window: vec![0; WINDOW * SECTOR_BYTES],
        verify: vec![0; if secure { WINDOW * SECTOR_BYTES } else { 0 }],
        c2_a: vec![0; if use_c2 { WINDOW * C2_BYTES } else { 0 }],
        c2_b: vec![0; if use_c2 && secure { WINDOW * C2_BYTES } else { 0 }],
        scratch: vec![0; SECTOR_BYTES],
        candidates: (0..MAX_CANDIDATES)
            .map(|_| Candidate { data: vec![0; SECTOR_BYTES], votes: 0, clean: false, used: false })
            .collect(),
        stats: TrackRead { samples_total: total_frames, ..TrackRead::default() },
    };
    let mut samples = vec![0i16; WINDOW * SAMPLES_PER_SECTOR];
    let mut remaining = total_frames;
    let mut done = 0u32;

    while done < sectors_needed {
        let count = (sectors_needed - done).min(u32::from(CHUNK_SECTORS)) as u16;
        let start = first_sector + i64::from(done);
        // The readable part of this window; the rest lies outside the disc's audio.
        let lo = start.max(0);
        let hi = (start + i64::from(count)).min(audio_end);
        let window_bytes = usize::from(count) * SECTOR_BYTES;
        reader.window[..window_bytes].fill(0);
        if hi > lo {
            let dst = (lo - start) as usize * SECTOR_BYTES;
            reader.read_window(lo as u32, (hi - lo) as u16, dst)?;
        }

        for (pair, sample) in reader.window[..window_bytes].chunks_exact(2).zip(samples.iter_mut()) {
            *sample = i16::from_le_bytes([pair[0], pair[1]]);
        }
        let frames_here = usize::from(count) * FRAMES_PER_SECTOR;
        let take = ((frames_here - skip) as u64).min(remaining) as usize;
        if take > 0 && !sink(&samples[skip * 2..(skip + take) * 2]) {
            return Err(DiscError::Cancelled);
        }
        remaining -= take as u64;
        skip = 0;
        done += u32::from(count);
        progress(ReadProgress {
            track,
            done_sectors: done,
            total_sectors: sectors_needed,
            rereads: reader.stats.rereads,
            suspicious_samples: reader.stats.suspicious_samples,
        });
    }
    Ok(reader.stats)
}
