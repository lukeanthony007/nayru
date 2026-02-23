"use client";

import { useReaderStore } from "@/lib/stores/reader-store";
import { useReaderStatus } from "@/hooks/use-reader-status";
import { useServerStartup } from "@/hooks/use-server-startup";
import { TextEditor } from "@/components/reader/text-editor";
import { SentenceDisplay } from "@/components/reader/sentence-display";
import { PlaybackBar } from "@/components/reader/playback-bar";

export default function ReaderPage() {
  const mode = useReaderStore((s) => s.mode);
  const serverPhase = useReaderStore((s) => s.serverPhase);
  const serverMessage = useReaderStore((s) => s.serverMessage);
  const serverProgress = useReaderStore((s) => s.serverProgress);

  useReaderStatus();
  useServerStartup();

  return (
    <div className="flex flex-col h-screen">
      {serverPhase !== "ready" && (
        <ServerStatusBanner
          phase={serverPhase}
          message={serverMessage}
          progress={serverProgress}
        />
      )}
      {mode === "edit" ? <TextEditor /> : <SentenceDisplay />}
      <PlaybackBar />
    </div>
  );
}

function ServerStatusBanner({
  phase,
  message,
  progress,
}: {
  phase: string;
  message: string;
  progress: number | null;
}) {
  const isError = phase === "error";

  return (
    <div
      className={`flex items-center gap-3 px-4 py-2 text-xs ${
        isError
          ? "bg-red-500/10 text-red-400 border-b border-red-500/20"
          : "bg-white/[0.03] text-white/40 border-b border-white/[0.06]"
      }`}
    >
      {!isError && (
        <div className="w-3 h-3 border-2 border-white/20 border-t-white/60 rounded-full animate-spin" />
      )}
      <span>{message}</span>
      {phase === "downloading" && progress != null && (
        <div className="flex-1 max-w-48 h-1 bg-white/10 rounded-full overflow-hidden">
          <div
            className="h-full bg-emerald-500/60 rounded-full transition-all duration-300"
            style={{ width: `${Math.min(progress, 100)}%` }}
          />
        </div>
      )}
    </div>
  );
}
