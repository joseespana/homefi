//! The reader-to-encoder pipe: a bounded channel of recycled chunks. A fixed pool of buffers
//! circulates (producer fills, consumer drains and hands the buffer back), so a rip allocates
//! nothing in steady state and never holds more than `capacity` chunks of audio.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, sync_channel, Receiver, RecvTimeoutError, Sender, SyncSender};
use std::time::Duration;

use crate::source::{CHUNK_SECTORS, SAMPLES_PER_SECTOR};

/// Chunks in flight: 32 x 63 KiB is about 2 MB.
pub const DEFAULT_CHUNKS: usize = 32;
/// How often a blocked producer looks at the cancel flag.
const CANCEL_POLL: Duration = Duration::from_millis(50);

/// Interleaved stereo samples of up to one read window.
#[derive(Debug, Default)]
pub struct Chunk {
    pub samples: Vec<i16>,
}

pub struct ChunkProducer {
    data: SyncSender<Chunk>,
    free: Receiver<Chunk>,
}

pub struct ChunkConsumer {
    data: Receiver<Chunk>,
    free: Sender<Chunk>,
}

/// A pipe with `capacity` pre-allocated chunks (at least 1).
pub fn chunk_channel(capacity: usize) -> (ChunkProducer, ChunkConsumer) {
    let capacity = capacity.max(1);
    let (data_tx, data_rx) = sync_channel(capacity);
    let (free_tx, free_rx) = channel();
    for _ in 0..capacity {
        let chunk = Chunk { samples: Vec::with_capacity(usize::from(CHUNK_SECTORS) * SAMPLES_PER_SECTOR) };
        free_tx.send(chunk).expect("receiver is alive");
    }
    (ChunkProducer { data: data_tx, free: free_rx }, ChunkConsumer { data: data_rx, free: free_tx })
}

impl ChunkProducer {
    /// Copies `samples` into a free chunk and sends it. Blocks while every chunk is in flight
    /// (backpressure). `false` = stop: cancelled, or the consumer is gone.
    pub fn send(&mut self, samples: &[i16], cancel: &AtomicBool) -> bool {
        let mut chunk = loop {
            if cancel.load(Ordering::Relaxed) {
                return false;
            }
            match self.free.recv_timeout(CANCEL_POLL) {
                Ok(chunk) => break chunk,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return false,
            }
        };
        chunk.samples.clear();
        chunk.samples.extend_from_slice(samples);
        self.data.send(chunk).is_ok()
    }

    /// A sink for [`crate::read_track`] that feeds this pipe and stops when `cancel` is set.
    pub fn sink<'a>(&'a mut self, cancel: &'a AtomicBool) -> impl FnMut(&[i16]) -> bool + 'a {
        move |samples| self.send(samples, cancel)
    }
}

impl ChunkConsumer {
    /// The next chunk, or `None` once the producer has finished and dropped.
    pub fn recv(&self) -> Option<Chunk> {
        self.data.recv().ok()
    }

    /// Gives a drained chunk back to the pool.
    pub fn recycle(&self, chunk: Chunk) {
        let _ = self.free.send(chunk);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn chunks_circulate_without_growing() {
        let (mut tx, rx) = chunk_channel(2);
        let cancel = Arc::new(AtomicBool::new(false));
        let producer_cancel = cancel.clone();
        let producer = std::thread::spawn(move || {
            for n in 0..50i16 {
                assert!(tx.send(&[n; 8], &producer_cancel));
            }
        });
        let mut seen = 0;
        while let Some(chunk) = rx.recv() {
            assert_eq!(chunk.samples, vec![seen; 8]);
            seen += 1;
            rx.recycle(chunk);
        }
        producer.join().unwrap();
        assert_eq!(seen, 50);
    }

    #[test]
    fn cancel_unblocks_a_full_pipe() {
        let (mut tx, _rx) = chunk_channel(1);
        let cancel = AtomicBool::new(false);
        assert!(tx.send(&[1], &cancel));
        cancel.store(true, Ordering::Relaxed);
        assert!(!tx.send(&[2], &cancel));
    }

    #[test]
    fn a_gone_consumer_stops_the_producer() {
        let (mut tx, rx) = chunk_channel(1);
        drop(rx);
        assert!(!tx.send(&[1], &AtomicBool::new(false)));
    }
}
