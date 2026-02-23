/**
 * Tauri IPC wrappers for TTS commands.
 */

export type ServerPhase =
  | "checking"
  | "installing"
  | "downloading"
  | "starting"
  | "ready"
  | "error";

export interface ServerStartupEvent {
  phase: ServerPhase;
  message: string;
  progress: number | null;
}

export async function onServerStartup(
  callback: (event: ServerStartupEvent) => void,
): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  const unlisten = await listen<ServerStartupEvent>("server-startup", (e) =>
    callback(e.payload),
  );
  return unlisten;
}

export async function getServerStatus(): Promise<boolean> {
  return invoke<boolean>("get_server_status");
}

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
