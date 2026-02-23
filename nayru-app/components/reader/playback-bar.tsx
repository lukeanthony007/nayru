"use client";

import { cn } from "@/lib/utils";
import { useReaderStore } from "@/lib/stores/reader-store";
import {
  speakFrom,
  ttsStop,
  ttsPause,
  ttsResume,
  ttsSkipSentence,
  setTtsConfig,
} from "@/lib/tts-ipc";
import { SpeedControl } from "./speed-control";
import { useCallback, useState } from "react";

export function PlaybackBar() {
  const text = useReaderStore((s) => s.text);
  const sentences = useReaderStore((s) => s.sentences);
  const ttsState = useReaderStore((s) => s.ttsState);
  const currentSentenceIndex = useReaderStore((s) => s.currentSentenceIndex);
  const totalSentences = useReaderStore((s) => s.totalSentences);
  const mode = useReaderStore((s) => s.mode);
  const setMode = useReaderStore((s) => s.setMode);
  const updateStatus = useReaderStore((s) => s.updateStatus);
  const [speed, setSpeed] = useState(1);

  const handlePlay = useCallback(async () => {
    if (ttsState === "idle") {
      try {
        const status = await speakFrom(text, 0);
        updateStatus(status);
      } catch {}
    } else {
      try {
        await ttsResume();
      } catch {}
    }
  }, [text, ttsState, updateStatus]);

  const handlePause = useCallback(async () => {
    try {
      await ttsPause();
    } catch {}
  }, []);

  const handleStop = useCallback(async () => {
    try {
      await ttsStop();
      updateStatus({
        state: "idle",
        current_sentence_index: null,
        total_sentences: 0,
      });
    } catch {}
  }, [updateStatus]);

  const handleSkip = useCallback(async () => {
    try {
      const status = await ttsSkipSentence();
      updateStatus(status);
    } catch {}
  }, [updateStatus]);

  const handleSpeedChange = useCallback(async (newSpeed: number) => {
    setSpeed(newSpeed);
    try {
      await setTtsConfig({ speed: newSpeed });
    } catch {}
  }, []);

  const isPlaying = ttsState === "playing";
  const isActive = ttsState !== "idle";
  const hasText = sentences.length > 0;

  return (
    <div className="flex items-center gap-2 px-4 py-3 border-t border-white/[0.06] bg-white/[0.02]">
      {/* Play/Pause */}
      <button
        onClick={isPlaying ? handlePause : handlePlay}
        disabled={!hasText}
        className={cn(
          "flex items-center justify-center w-8 h-8 rounded-full transition-colors",
          hasText
            ? "text-emerald-400 hover:bg-emerald-500/20"
            : "text-white/20 cursor-not-allowed",
        )}
        title={isPlaying ? "Pause" : "Play"}
      >
        {isPlaying ? <PauseIcon /> : <PlayIcon />}
      </button>

      {/* Stop */}
      <button
        onClick={handleStop}
        disabled={!isActive}
        className={cn(
          "flex items-center justify-center w-8 h-8 rounded-full transition-colors",
          isActive
            ? "text-white/50 hover:text-white/80 hover:bg-white/[0.06]"
            : "text-white/20 cursor-not-allowed",
        )}
        title="Stop"
      >
        <StopIcon />
      </button>

      {/* Skip */}
      <button
        onClick={handleSkip}
        disabled={!isActive}
        className={cn(
          "flex items-center justify-center w-8 h-8 rounded-full transition-colors",
          isActive
            ? "text-white/50 hover:text-white/80 hover:bg-white/[0.06]"
            : "text-white/20 cursor-not-allowed",
        )}
        title="Next sentence"
      >
        <SkipIcon />
      </button>

      {/* Speed */}
      <SpeedControl speed={speed} onSpeedChange={handleSpeedChange} />

      {/* Sentence progress */}
      <div className="flex-1" />
      {isActive && (
        <span className="text-xs text-white/30 tabular-nums">
          {(currentSentenceIndex ?? 0) + 1} / {totalSentences}
        </span>
      )}

      {/* Edit toggle */}
      <button
        onClick={() => setMode(mode === "edit" ? "read" : "edit")}
        className="flex items-center justify-center w-8 h-8 rounded-full text-white/30 hover:text-white/60 hover:bg-white/[0.06] transition-colors"
        title={mode === "edit" ? "Read mode" : "Edit text"}
      >
        {mode === "edit" ? <BookIcon /> : <PencilIcon />}
      </button>
    </div>
  );
}

function PlayIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
      <path d="M8 5v14l11-7z" />
    </svg>
  );
}

function PauseIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
      <path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z" />
    </svg>
  );
}

function StopIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
      <path d="M6 6h12v12H6z" />
    </svg>
  );
}

function SkipIcon() {
  return (
    <svg
      width="14"
      height="14"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <polygon points="5,4 15,12 5,20" fill="currentColor" />
      <line x1="19" y1="5" x2="19" y2="19" />
    </svg>
  );
}

function PencilIcon() {
  return (
    <svg
      width="14"
      height="14"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.5}
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z" />
    </svg>
  );
}

function BookIcon() {
  return (
    <svg
      width="14"
      height="14"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.5}
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z" />
      <path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z" />
    </svg>
  );
}
