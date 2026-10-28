//! File decoding with symphonia: FLAC, ALAC, WAV, AIFF, MP3, AAC, Vorbis, ...

use std::fs::File;
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::{Time, TimeBase};

use crate::error::{PlayerError, Result};

/// An open audio file producing interleaved `f32` samples.
pub struct Source {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    time_base: Option<TimeBase>,
    samples: Option<SampleBuffer<f32>>,
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: Option<u32>,
    pub duration_ms: Option<u64>,
    /// Short codec name: `flac`, `alac`, `mp3`, `aac`, `vorbis`, `pcm_s24le`, ...
    pub codec: String,
}

impl Source {
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path).map_err(|e| PlayerError::Open(format!("{}: {e}", path.display())))?;
        let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
        // symphonia can't decode FLAC inside MP4 (Tidal Hi-Res): remux it in memory first
        if matches!(ext.as_deref(), Some("m4a" | "mp4")) {
            let data = std::fs::read(path).map_err(|e| PlayerError::Open(format!("{}: {e}", path.display())))?;
            if let Ok(Some(flac)) = homefi_core::remux::flac_from_mp4_bytes(&data) {
                let cursor = std::io::Cursor::new(flac);
                return Self::open_stream(
                    MediaSourceStream::new(Box::new(cursor), Default::default()),
                    Some("flac"),
                    path,
                );
            }
        }
        Self::open_stream(MediaSourceStream::new(Box::new(file), Default::default()), ext.as_deref(), path)
    }

    fn open_stream(mss: MediaSourceStream, ext: Option<&str>, path: &Path) -> Result<Self> {
        let mut hint = Hint::new();
        if let Some(ext) = ext {
            hint.with_extension(ext);
        }

        let probed = symphonia::default::get_probe()
            .format(
                &hint,
                mss,
                &FormatOptions { enable_gapless: true, ..Default::default() },
                &MetadataOptions::default(),
            )
            .map_err(|e| PlayerError::Unsupported(format!("{}: {e}", path.display())))?;
        let format = probed.format;

        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or_else(|| PlayerError::Unsupported(format!("{}: no audio track", path.display())))?;
        let params = track.codec_params.clone();

        let decoder = symphonia::default::get_codecs()
            .make(&params, &DecoderOptions::default())
            .map_err(|e| PlayerError::Unsupported(format!("{}: {e}", path.display())))?;

        let codec = symphonia::default::get_codecs()
            .get_codec(params.codec)
            .map(|d| d.short_name.to_owned())
            .unwrap_or_else(|| "audio".into());

        let duration_ms = match (params.n_frames, params.time_base) {
            (Some(frames), Some(tb)) => {
                let t = tb.calc_time(frames);
                Some(t.seconds * 1000 + (t.frac * 1000.0) as u64)
            }
            (Some(frames), None) => params.sample_rate.map(|r| frames * 1000 / r as u64),
            _ => None,
        };

        Ok(Self {
            track_id: track.id,
            time_base: params.time_base,
            sample_rate: params.sample_rate.unwrap_or(44_100),
            channels: params.channels.map(|c| c.count() as u16).unwrap_or(2),
            bits_per_sample: params.bits_per_sample,
            duration_ms,
            codec,
            format,
            decoder,
            samples: None,
        })
    }

    /// Next chunk of interleaved samples, `None` at the end of the file.
    pub fn next_chunk(&mut self) -> Result<Option<&[f32]>> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
                Err(SymError::ResetRequired) => {
                    self.decoder.reset();
                    continue;
                }
                Err(e) => return Err(PlayerError::Decode(e.to_string())),
            };
            if packet.track_id() != self.track_id {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    let spec = *decoded.spec();
                    // the real spec is only known after the first packet for some formats
                    self.sample_rate = spec.rate;
                    self.channels = spec.channels.count() as u16;
                    let needed = decoded.capacity() as u64;
                    let buf = match &mut self.samples {
                        Some(b) if b.capacity() as u64 >= needed * spec.channels.count() as u64 => b,
                        slot => slot.insert(SampleBuffer::new(needed, spec)),
                    };
                    buf.copy_interleaved_ref(decoded);
                    return Ok(Some(self.samples.as_ref().expect("buffer set").samples()));
                }
                // a corrupt packet: skip it, keep playing
                Err(SymError::DecodeError(e)) => {
                    tracing::debug!("decode error (skipped): {e}");
                    continue;
                }
                Err(e) => return Err(PlayerError::Decode(e.to_string())),
            }
        }
    }

    /// Seeks to `ms`; returns the position actually reached.
    pub fn seek(&mut self, ms: u64) -> Result<u64> {
        let time = Time::new(ms / 1000, (ms % 1000) as f64 / 1000.0);
        let seeked = self
            .format
            .seek(SeekMode::Accurate, SeekTo::Time { time, track_id: Some(self.track_id) })
            .map_err(|e| PlayerError::Decode(format!("seek: {e}")))?;
        self.decoder.reset();
        let reached = match self.time_base {
            Some(tb) => {
                let t = tb.calc_time(seeked.actual_ts);
                t.seconds * 1000 + (t.frac * 1000.0) as u64
            }
            None => ms,
        };
        Ok(reached)
    }
}
