//! Streaming rodio `Source` backed by a channel of PCM chunks.
//!
//! The fetcher creates this source only after receiving the first PCM data
//! from Kokoro, pre-loading it into the channel. This ensures the sink
//! never starts consuming from an empty source (no ALSA underruns).
//!
//! Once playing, `next()` uses a 10ms recv timeout — if data doesn't arrive
//! in time it yields a silence sample to keep rodio alive. When `Done` is
//! received or the sender is dropped, iteration ends.

use std::collections::VecDeque;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use rodio::Source;

/// A chunk of PCM data sent from the fetcher to the streaming source.
pub enum PcmChunk {
    /// Raw interleaved i16 PCM samples.
    Data(Vec<i16>),
    /// Stream is complete — no more data will arrive.
    Done,
}

/// A rodio `Source` that yields samples from a channel on-demand.
pub struct StreamingSource {
    rx: Receiver<PcmChunk>,
    buffer: VecDeque<i16>,
    channels: u16,
    sample_rate: u32,
    finished: bool,
}

impl StreamingSource {
    /// Create a new streaming source.
    ///
    /// The first `PcmChunk::Data` should already be sent to the channel
    /// before this source is appended to the sink, ensuring `next()`
    /// returns real audio immediately.
    pub fn new(rx: Receiver<PcmChunk>, channels: u16, sample_rate: u32) -> Self {
        Self {
            rx,
            buffer: VecDeque::with_capacity(8192),
            channels,
            sample_rate,
            finished: false,
        }
    }

    /// Try to fill the buffer from the channel.
    fn fill_buffer(&mut self) {
        // Drain all immediately available chunks
        while let Ok(chunk) = self.rx.try_recv() {
            match chunk {
                PcmChunk::Data(samples) => self.buffer.extend(samples),
                PcmChunk::Done => {
                    self.finished = true;
                    return;
                }
            }
        }

        // If still empty, block briefly for new data
        if self.buffer.is_empty() && !self.finished {
            match self.rx.recv_timeout(Duration::from_millis(10)) {
                Ok(PcmChunk::Data(samples)) => self.buffer.extend(samples),
                Ok(PcmChunk::Done) => self.finished = true,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    self.finished = true;
                }
            }
        }
    }
}

impl Iterator for StreamingSource {
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        if let Some(sample) = self.buffer.pop_front() {
            return Some(sample);
        }

        if self.finished {
            return None;
        }

        self.fill_buffer();

        if let Some(sample) = self.buffer.pop_front() {
            Some(sample)
        } else if self.finished {
            None
        } else {
            // Timeout — yield silence to keep rodio alive
            Some(0)
        }
    }
}

impl Source for StreamingSource {
    fn current_frame_len(&self) -> Option<usize> {
        if self.finished && self.buffer.is_empty() {
            Some(0)
        } else if self.buffer.is_empty() {
            Some(1)
        } else {
            Some(self.buffer.len())
        }
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn streams_data_then_finishes() {
        let (tx, rx) = mpsc::channel();
        let mut source = StreamingSource::new(rx, 1, 24000);

        tx.send(PcmChunk::Data(vec![100, 200, 300])).unwrap();
        tx.send(PcmChunk::Data(vec![400, 500])).unwrap();
        tx.send(PcmChunk::Done).unwrap();

        let samples: Vec<i16> = source.by_ref().collect();
        assert_eq!(samples, vec![100, 200, 300, 400, 500]);
    }

    #[test]
    fn sender_drop_ends_stream() {
        let (tx, rx) = mpsc::channel();
        let mut source = StreamingSource::new(rx, 1, 16000);

        tx.send(PcmChunk::Data(vec![42])).unwrap();
        drop(tx);

        let samples: Vec<i16> = source.by_ref().collect();
        assert_eq!(samples, vec![42]);
    }

    #[test]
    fn reports_correct_format() {
        let (_tx, rx) = mpsc::channel();
        let source = StreamingSource::new(rx, 2, 48000);
        assert_eq!(source.channels(), 2);
        assert_eq!(source.sample_rate(), 48000);
        assert_eq!(source.total_duration(), None);
    }
}
