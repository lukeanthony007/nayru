/**
 * Split text into sentences. Must mirror the Rust `split_sentences()` in nayru-core.
 *
 * Splits at sentence-ending punctuation (. ! ?) followed by whitespace,
 * or at paragraph breaks (double newlines).
 */
export function splitSentences(text: string): string[] {
  return text
    .split(/(?<=[.!?])\s+|\n\n+/)
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}
