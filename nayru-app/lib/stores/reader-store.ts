import { create } from "zustand";
import { splitSentences } from "@/lib/sentences";

export interface ReaderState {
  text: string;
  setText: (text: string) => void;
  sentences: string[];
  mode: "edit" | "read";
  setMode: (mode: "edit" | "read") => void;

  // TTS status (updated by polling)
  currentSentenceIndex: number | null;
  ttsState: "idle" | "converting" | "playing";
  totalSentences: number;

  updateStatus: (status: {
    state: "idle" | "converting" | "playing";
    current_sentence_index: number | null;
    total_sentences: number;
  }) => void;
}

export const useReaderStore = create<ReaderState>()((set) => ({
  text: "",
  setText: (text) =>
    set({
      text,
      sentences: splitSentences(text),
    }),
  sentences: [],
  mode: "edit",
  setMode: (mode) => set({ mode }),

  currentSentenceIndex: null,
  ttsState: "idle",
  totalSentences: 0,

  updateStatus: (status) =>
    set({
      ttsState: status.state,
      currentSentenceIndex: status.current_sentence_index,
      totalSentences: status.total_sentences,
    }),
}));
