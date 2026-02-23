"use client";

import { cn } from "@/lib/utils";
import { useReaderStore } from "@/lib/stores/reader-store";
import { speakFrom } from "@/lib/tts-ipc";
import { useCallback } from "react";

export function SentenceDisplay() {
  const text = useReaderStore((s) => s.text);
  const sentences = useReaderStore((s) => s.sentences);
  const currentSentenceIndex = useReaderStore((s) => s.currentSentenceIndex);
  const updateStatus = useReaderStore((s) => s.updateStatus);

  const handleSentenceClick = useCallback(
    async (index: number) => {
      try {
        const status = await speakFrom(text, index);
        updateStatus(status);
      } catch {
        // IPC not available
      }
    },
    [text, updateStatus],
  );

  if (sentences.length === 0) {
    return (
      <div className="flex flex-1 items-center justify-center text-white/20 text-sm">
        No text to display
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto p-6">
      <div className="max-w-2xl mx-auto leading-relaxed text-base">
        {sentences.map((sentence, index) => (
          <span
            key={index}
            onClick={() => handleSentenceClick(index)}
            className={cn(
              "cursor-pointer rounded-sm px-0.5 -mx-0.5 transition-colors duration-150",
              index === currentSentenceIndex
                ? "bg-emerald-500/20 text-white/90"
                : "text-white/60 hover:text-white/80 hover:bg-white/[0.04]",
            )}
          >
            {sentence}{" "}
          </span>
        ))}
      </div>
    </div>
  );
}
