"use client";

import { useEffect } from "react";
import { onServerStartup } from "@/lib/tts-ipc";
import { useReaderStore } from "@/lib/stores/reader-store";

/**
 * Listens for server-startup events from the Tauri backend
 * and updates the reader store with the current phase/progress.
 */
export function useServerStartup() {
  const setServerStatus = useReaderStore((s) => s.setServerStatus);

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    onServerStartup((event) => {
      setServerStatus(event.phase, event.message, event.progress);
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      unlisten?.();
    };
  }, [setServerStatus]);
}
