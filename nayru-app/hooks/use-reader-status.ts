"use client";

import { useEffect, useRef } from "react";
import { getReaderStatus } from "@/lib/tts-ipc";
import { useReaderStore } from "@/lib/stores/reader-store";

/**
 * Polls the backend for TTS status every 200ms when active.
 * Stops polling when idle to avoid unnecessary IPC overhead.
 */
export function useReaderStatus() {
  const ttsState = useReaderStore((s) => s.ttsState);
  const updateStatus = useReaderStore((s) => s.updateStatus);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    const poll = async () => {
      try {
        const status = await getReaderStatus();
        updateStatus(status);
      } catch {
        // IPC not available (e.g., running in browser)
      }
    };

    // Always poll at least once to sync initial state
    poll();

    // Only keep polling when not idle
    if (ttsState !== "idle") {
      intervalRef.current = setInterval(poll, 200);
    }

    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    };
  }, [ttsState, updateStatus]);
}
