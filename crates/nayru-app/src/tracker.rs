//! Sentence-to-chunk mapping for tracking which sentence is currently playing.
//!
//! The TTS engine splits text into ~200-char chunks that don't align 1:1 with
//! user-visible sentences. This module maps chunk indices back to sentence indices.

use nayru_core::text_prep::{split_sentences, split_text, DEFAULT_MAX_CHUNK_LEN};

#[derive(Debug)]
pub struct SentenceTracker {
    /// The sentences being spoken (from start_index onward).
    pub sentences: Vec<String>,
    /// Cumulative chunk count after each sentence.
    /// `chunk_offsets[i]` = total chunks for sentences[0..=i].
    pub chunk_offsets: Vec<usize>,
    /// Total chunks across all sentences.
    pub total_chunks: usize,
    /// Offset into the original text's sentence array.
    pub start_index: usize,
    /// Full text for re-speaking from a different index.
    pub full_text: String,
}

impl SentenceTracker {
    pub fn empty() -> Self {
        Self {
            sentences: Vec::new(),
            chunk_offsets: Vec::new(),
            total_chunks: 0,
            start_index: 0,
            full_text: String::new(),
        }
    }

    /// Build a tracker from full text, starting at `start_index`.
    pub fn new(full_text: &str, start_index: usize) -> Self {
        let all_sentences = split_sentences(full_text);
        let sentences: Vec<String> = all_sentences.into_iter().skip(start_index).collect();

        let max_chunk_len = DEFAULT_MAX_CHUNK_LEN;
        let mut chunk_offsets = Vec::with_capacity(sentences.len());
        let mut total = 0usize;

        for sentence in &sentences {
            let chunks = split_text(sentence, max_chunk_len);
            let batched_count = simulate_merge(&chunks, max_chunk_len);
            total += batched_count;
            chunk_offsets.push(total);
        }

        Self {
            sentences,
            chunk_offsets,
            total_chunks: total,
            start_index,
            full_text: full_text.to_string(),
        }
    }

    /// Given how many chunks have been completed, return the current sentence
    /// index (in the original text's sentence numbering).
    pub fn current_sentence(&self, chunks_completed: usize) -> Option<usize> {
        for (i, &offset) in self.chunk_offsets.iter().enumerate() {
            if chunks_completed < offset {
                return Some(self.start_index + i);
            }
        }
        None // all done
    }

    pub fn total_sentences_in_text(&self) -> usize {
        self.start_index + self.sentences.len()
    }
}

/// Simulate the text_processor_task's merge logic: merge adjacent chunks
/// if `merged.len() + 1 + next.len() <= max_chunk_len`.
/// Returns the number of batched chunks that will actually be sent to the fetcher.
fn simulate_merge(chunks: &[String], max_chunk_len: usize) -> usize {
    if chunks.is_empty() {
        return 0;
    }

    let mut batched_count = 0;
    let mut i = 0;

    while i < chunks.len() {
        let mut merged_len = chunks[i].len();
        i += 1;

        while i < chunks.len() && merged_len + 1 + chunks[i].len() <= max_chunk_len {
            merged_len += 1 + chunks[i].len();
            i += 1;
        }

        batched_count += 1;
    }

    batched_count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_tracker() {
        let t = SentenceTracker::empty();
        assert_eq!(t.total_chunks, 0);
        assert_eq!(t.current_sentence(0), None);
    }

    #[test]
    fn single_sentence() {
        let t = SentenceTracker::new("Hello world.", 0);
        assert_eq!(t.sentences.len(), 1);
        assert_eq!(t.total_chunks, 1);
        assert_eq!(t.current_sentence(0), Some(0));
        assert_eq!(t.current_sentence(1), None);
    }

    #[test]
    fn multiple_sentences() {
        let t = SentenceTracker::new("First sentence. Second sentence. Third sentence.", 0);
        assert_eq!(t.sentences.len(), 3);
        // All short sentences, each becomes 1 chunk, merge may combine them
        assert_eq!(t.current_sentence(0), Some(0));
    }

    #[test]
    fn start_from_middle() {
        let t = SentenceTracker::new("First. Second. Third.", 1);
        assert_eq!(t.sentences.len(), 2); // "Second." and "Third."
        assert_eq!(t.start_index, 1);
        assert_eq!(t.current_sentence(0), Some(1));
    }

    #[test]
    fn simulate_merge_basic() {
        let chunks = vec!["Hello.".to_string(), "World.".to_string()];
        // Both fit in 200, so they merge into 1
        assert_eq!(simulate_merge(&chunks, 200), 1);
    }

    #[test]
    fn simulate_merge_no_fit() {
        let long = "a".repeat(150);
        let chunks = vec![long.clone(), long];
        // 150 + 1 + 150 = 301 > 200, so they stay separate
        assert_eq!(simulate_merge(&chunks, 200), 2);
    }
}
