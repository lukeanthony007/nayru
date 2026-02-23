"use client";

import { useReaderStore } from "@/lib/stores/reader-store";

export function TextEditor() {
  const text = useReaderStore((s) => s.text);
  const setText = useReaderStore((s) => s.setText);

  return (
    <div className="flex flex-1 flex-col p-6">
      <textarea
        className="flex-1 w-full resize-none rounded-lg bg-white/[0.03] border border-white/[0.06] p-6 text-sm text-white/80 placeholder-white/20 outline-none focus:border-emerald-500/30 transition-colors"
        placeholder="Paste your text here..."
        value={text}
        onChange={(e) => setText(e.target.value)}
        spellCheck={false}
      />
    </div>
  );
}
