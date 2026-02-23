//! Text preparation for TTS — markdown cleaning and sentence splitting.
//!
//! Pure functions, no I/O. Ported from `raia-app/lib/voice.ts`.

use regex::Regex;
use std::sync::LazyLock;

// Compiled regexes — allocated once, reused across calls.
static RE_TABLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)(?:^|\n)(\|[^\n]+\|(?:\n\|[^\n]+\|)*)").unwrap());
static RE_FENCED_CODE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)```.*?```").unwrap());
static RE_INLINE_CODE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"`[^`]+`").unwrap());
static RE_HR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^[\s]*[-*_]{3,}[\s]*$").unwrap());
static RE_BOLD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\*\*([^*]+)\*\*").unwrap());
static RE_ITALIC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\*([^*]+)\*").unwrap());
static RE_HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"#{1,6}\s*").unwrap());
static RE_LINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\([^)]+\)").unwrap());
static RE_BULLET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^[\s]*[-*]\s+").unwrap());
static RE_NUMBERED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^[\s]*\d+\.\s+").unwrap());
static RE_LEADING_DOT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\.\s*").unwrap());
static RE_DOUBLE_DOT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\.\s*\.").unwrap());
static RE_MULTI_SPACE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s{2,}").unwrap());

/// Strip markdown formatting so text reads naturally when spoken.
///
/// Handles: fenced code blocks, tables, inline code, bold/italic,
/// headings, links, bullets/numbered lists, horizontal rules.
pub fn clean_text_for_tts(text: &str) -> String {
    let mut c = text.to_string();

    // Tables → placeholder (before code blocks, since tables can appear inside fences)
    c = RE_TABLE.replace_all(&c, "\nSee the table in our conversation.\n").into_owned();
    // Fenced code blocks → placeholder
    c = RE_FENCED_CODE.replace_all(&c, " See the code in our conversation. ").into_owned();
    // Inline code → removed
    c = RE_INLINE_CODE.replace_all(&c, "").into_owned();
    // Horizontal rules → removed
    c = RE_HR.replace_all(&c, "").into_owned();
    // Bold → plain
    c = RE_BOLD.replace_all(&c, "$1").into_owned();
    // Italic → plain
    c = RE_ITALIC.replace_all(&c, "$1").into_owned();
    // Headings → pound signs removed
    c = RE_HEADING.replace_all(&c, "").into_owned();
    // Links → text only
    c = RE_LINK.replace_all(&c, "$1").into_owned();
    // Bullets / numbered lists → ". " prefix
    c = RE_BULLET.replace_all(&c, ". ").into_owned();
    c = RE_NUMBERED.replace_all(&c, ". ").into_owned();
    // Clean up leading dot at start of string
    c = RE_LEADING_DOT.replace(&c, "").into_owned();
    // Double periods → single
    c = RE_DOUBLE_DOT.replace_all(&c, ".").into_owned();
    // Collapse whitespace
    c = RE_MULTI_SPACE.replace_all(&c, " ").into_owned();

    c.trim().to_string()
}

/// Default maximum chunk length for [`split_text`].
pub const DEFAULT_MAX_CHUNK_LEN: usize = 200;

/// Split text into chunks of roughly `max_len` chars.
///
/// Prefers sentence boundaries (`. `), then word boundaries, then hard-splits.
/// Chunks shorter than 2 chars are discarded.
pub fn split_text(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }

    let mut result = Vec::new();
    let mut remaining = text;

    while remaining.len() > max_len {
        let window = &remaining[..max_len];

        // Prefer sentence boundary (". ")
        let split_at = if let Some(pos) = window.rfind(". ") {
            if pos >= max_len / 2 {
                pos + 1 // include the period
            } else {
                // Fall back to word boundary
                word_boundary_or_hard(window, max_len)
            }
        } else {
            word_boundary_or_hard(window, max_len)
        };

        let chunk = remaining[..split_at].trim_end();
        if !chunk.is_empty() {
            result.push(chunk.to_string());
        }
        remaining = remaining[split_at..].trim_start();
    }

    if remaining.len() >= 2 {
        result.push(remaining.to_string());
    }

    result
}

/// Split text into sentences at sentence-ending punctuation (`. `, `! `, `? `)
/// or paragraph breaks (double newlines).
///
/// Returns non-empty, trimmed strings. Used by the reader app to render
/// clickable sentence spans and by the backend `SentenceTracker` to build
/// chunk-to-sentence mappings.
///
/// The TypeScript mirror is: `text.split(/(?<=[.!?])\s+|\n\n+/)` — JS regex
/// supports lookbehind but Rust's `regex` crate does not, so we split manually.
pub fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Paragraph break: split before \n\n
        if bytes[i] == b'\n' && i + 1 < len && bytes[i + 1] == b'\n' {
            let chunk = text[start..i].trim();
            if !chunk.is_empty() {
                sentences.push(chunk.to_string());
            }
            // Skip all consecutive newlines
            while i < len && bytes[i] == b'\n' {
                i += 1;
            }
            start = i;
            continue;
        }

        // Sentence-ending punctuation followed by whitespace
        if (bytes[i] == b'.' || bytes[i] == b'!' || bytes[i] == b'?')
            && i + 1 < len
            && bytes[i + 1].is_ascii_whitespace()
            && bytes[i + 1] != b'\n' || (bytes[i] == b'.' || bytes[i] == b'!' || bytes[i] == b'?')
                && i + 1 < len
                && bytes[i + 1] == b' '
        {
            let chunk = text[start..=i].trim();
            if !chunk.is_empty() {
                sentences.push(chunk.to_string());
            }
            i += 1;
            // Skip whitespace after punctuation
            while i < len && bytes[i].is_ascii_whitespace() && bytes[i] != b'\n' {
                i += 1;
            }
            start = i;
            continue;
        }

        i += 1;
    }

    // Remaining text
    if start < len {
        let chunk = text[start..].trim();
        if !chunk.is_empty() {
            sentences.push(chunk.to_string());
        }
    }

    sentences
}

/// Find a word boundary, or fall back to a hard split.
fn word_boundary_or_hard(window: &str, max_len: usize) -> usize {
    if let Some(pos) = window.rfind(' ') {
        if pos >= max_len / 3 {
            return pos;
        }
    }
    max_len
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── clean_text_for_tts ──────────────────────────────────────────

    #[test]
    fn strips_fenced_code_blocks() {
        let input = "before ```rust\nfn main() {}\n``` after";
        assert_eq!(
            clean_text_for_tts(input),
            "before See the code in our conversation. after"
        );
    }

    #[test]
    fn strips_tables() {
        let input = "intro\n| a | b |\n| 1 | 2 |\nafter";
        let result = clean_text_for_tts(input);
        assert!(result.contains("See the table in our conversation."));
        assert!(result.contains("after"));
    }

    #[test]
    fn strips_inline_code() {
        assert_eq!(clean_text_for_tts("use `println!` here"), "use here");
    }

    #[test]
    fn strips_bold() {
        assert_eq!(clean_text_for_tts("this is **bold** text"), "this is bold text");
    }

    #[test]
    fn strips_italic() {
        assert_eq!(clean_text_for_tts("this is *italic* text"), "this is italic text");
    }

    #[test]
    fn strips_headings() {
        assert_eq!(clean_text_for_tts("## Hello World"), "Hello World");
        assert_eq!(clean_text_for_tts("# H1\n## H2"), "H1\nH2");
    }

    #[test]
    fn strips_links() {
        assert_eq!(
            clean_text_for_tts("click [here](https://example.com) now"),
            "click here now"
        );
    }

    #[test]
    fn strips_bullet_lists() {
        let input = "items:\n- first\n- second";
        let result = clean_text_for_tts(input);
        assert!(result.contains(". first"));
        assert!(result.contains(". second"));
    }

    #[test]
    fn strips_numbered_lists() {
        let input = "steps:\n1. first\n2. second";
        let result = clean_text_for_tts(input);
        assert!(result.contains(". first"));
        assert!(result.contains(". second"));
    }

    #[test]
    fn strips_horizontal_rules() {
        let result = clean_text_for_tts("above\n---\nbelow");
        assert!(!result.contains("---"));
        assert!(result.contains("above"));
        assert!(result.contains("below"));
    }

    #[test]
    fn collapses_whitespace() {
        assert_eq!(clean_text_for_tts("hello    world"), "hello world");
    }

    #[test]
    fn cleans_double_periods() {
        assert_eq!(clean_text_for_tts("end.. start"), "end. start");
    }

    #[test]
    fn combined_markdown() {
        let input = "# Title\n\nThis is **bold** and *italic*.\n\n```js\nconsole.log('hi');\n```\n\n- bullet one\n- [link](http://x.com)";
        let result = clean_text_for_tts(input);
        assert!(!result.contains('#'));
        assert!(!result.contains('*'));
        assert!(!result.contains("```"));
        assert!(!result.contains("http"));
        assert!(result.contains("See the code in our conversation."));
    }

    #[test]
    fn empty_input() {
        assert_eq!(clean_text_for_tts(""), "");
    }

    #[test]
    fn plain_text_unchanged() {
        assert_eq!(
            clean_text_for_tts("Hello, how are you today?"),
            "Hello, how are you today?"
        );
    }

    // ── split_text ──────────────────────────────────────────────────

    #[test]
    fn short_text_not_split() {
        let chunks = split_text("Hello world.", 200);
        assert_eq!(chunks, vec!["Hello world."]);
    }

    #[test]
    fn splits_at_sentence_boundary() {
        let text = "First sentence. Second sentence. Third sentence that is long enough to push past the limit.";
        // max_len = 40 so it must split
        let chunks = split_text(text, 40);
        assert!(chunks.len() >= 2);
        // First chunk should end at a sentence boundary
        assert!(chunks[0].ends_with('.'));
    }

    #[test]
    fn splits_at_word_boundary() {
        // No periods — must fall back to word boundary
        let text = "word ".repeat(50);
        let chunks = split_text(text.trim(), 30);
        assert!(chunks.len() > 1);
        for chunk in &chunks {
            assert!(chunk.len() <= 30, "chunk too long: {}", chunk.len());
        }
    }

    #[test]
    fn hard_splits_long_word() {
        let text = "a".repeat(300);
        let chunks = split_text(&text, 100);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].len(), 100);
        assert_eq!(chunks[1].len(), 100);
        assert_eq!(chunks[2].len(), 100);
    }

    #[test]
    fn discards_tiny_trailing() {
        // Trailing fragment of 1 char should be discarded (< 2)
        let text = format!("{} x", "a".repeat(198));
        let chunks = split_text(&text, 200);
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn preserves_all_content() {
        let text = "The quick brown fox jumps over the lazy dog. Pack my box with five dozen liquor jugs. How vexingly quick daft zebras jump.";
        let chunks = split_text(text, 50);
        let rejoined: String = chunks.join(" ");
        // Every word from the original should appear in the rejoined output
        for word in text.split_whitespace() {
            assert!(rejoined.contains(word), "missing word: {}", word);
        }
    }

    #[test]
    fn default_max_chunk_len() {
        assert_eq!(DEFAULT_MAX_CHUNK_LEN, 200);
    }

    // ── split_sentences ───────────────────────────────────────────

    #[test]
    fn split_sentences_basic() {
        let s = split_sentences("Hello world. How are you? I am fine!");
        assert_eq!(s, vec!["Hello world.", "How are you?", "I am fine!"]);
    }

    #[test]
    fn split_sentences_paragraph_break() {
        let s = split_sentences("First paragraph.\n\nSecond paragraph.");
        assert_eq!(s, vec!["First paragraph.", "Second paragraph."]);
    }

    #[test]
    fn split_sentences_single() {
        let s = split_sentences("Just one sentence");
        assert_eq!(s, vec!["Just one sentence"]);
    }

    #[test]
    fn split_sentences_empty() {
        let s = split_sentences("");
        assert!(s.is_empty());
    }

    #[test]
    fn split_sentences_trims_whitespace() {
        let s = split_sentences("  Hello.   World.  ");
        assert_eq!(s, vec!["Hello.", "World."]);
    }

    #[test]
    fn split_sentences_mixed_punctuation() {
        let s = split_sentences("Really? Yes! OK. Done");
        assert_eq!(s, vec!["Really?", "Yes!", "OK.", "Done"]);
    }
}
