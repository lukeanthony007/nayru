"use client";

import { useReaderStore } from "@/lib/stores/reader-store";
import { useReaderStatus } from "@/hooks/use-reader-status";
import { TextEditor } from "@/components/reader/text-editor";
import { SentenceDisplay } from "@/components/reader/sentence-display";
import { PlaybackBar } from "@/components/reader/playback-bar";

export default function ReaderPage() {
  const mode = useReaderStore((s) => s.mode);

  useReaderStatus();

  return (
    <div className="flex flex-col h-screen">
      {mode === "edit" ? <TextEditor /> : <SentenceDisplay />}
      <PlaybackBar />
    </div>
  );
}
