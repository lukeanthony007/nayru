/**
 * Tauri IPC wrappers for TTS commands.
 */

export interface ReaderStatus {
  state: "idle" | "converting" | "playing";
  current_sentence_index: number | null;
  total_sentences: number;
  voice: string;
  speed: number;
}

export interface TtsConfig {
  kokoro_url: string;
  voice: string;
  speed: number;
}

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke: tauriInvoke } = await import("@tauri-apps/api/core");
  return tauriInvoke<T>(cmd, args);
}

export async function speakFrom(text: string, sentenceIndex: number): Promise<ReaderStatus> {
  return invoke<ReaderStatus>("speak_from", { text, sentenceIndex });
}

export async function ttsStop(): Promise<void> {
  return invoke("tts_stop");
}

export async function ttsPause(): Promise<void> {
  return invoke("tts_pause");
}

export async function ttsResume(): Promise<void> {
  return invoke("tts_resume");
}

export async function ttsSkipSentence(): Promise<ReaderStatus> {
  return invoke<ReaderStatus>("tts_skip_sentence");
}

export async function getReaderStatus(): Promise<ReaderStatus> {
  return invoke<ReaderStatus>("get_reader_status");
}

export async function setTtsConfig(patch: Partial<TtsConfig>): Promise<void> {
  return invoke("set_tts_config", { patch });
}

export async function getTtsConfig(): Promise<TtsConfig> {
  return invoke<TtsConfig>("get_tts_config");
}
