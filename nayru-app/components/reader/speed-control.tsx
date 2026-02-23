"use client";

import { cn } from "@/lib/utils";
import { useState, useRef, useEffect } from "react";

const SPEEDS = [0.5, 0.75, 1, 1.25, 1.5, 2];

export function SpeedControl({
  speed,
  onSpeedChange,
}: {
  speed: number;
  onSpeedChange: (speed: number) => void;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  return (
    <div className="relative" ref={ref}>
      <button
        onClick={() => setOpen(!open)}
        className="px-2 py-1 text-xs text-white/50 hover:text-white/80 transition-colors rounded hover:bg-white/[0.06]"
      >
        {speed}x
      </button>
      {open && (
        <div className="absolute bottom-full mb-1 left-1/2 -translate-x-1/2 bg-zinc-900 border border-white/10 rounded-lg py-1 shadow-xl">
          {SPEEDS.map((s) => (
            <button
              key={s}
              onClick={() => {
                onSpeedChange(s);
                setOpen(false);
              }}
              className={cn(
                "block w-full px-4 py-1.5 text-xs text-left whitespace-nowrap transition-colors",
                s === speed
                  ? "text-emerald-400 bg-white/[0.04]"
                  : "text-white/50 hover:text-white/80 hover:bg-white/[0.04]",
              )}
            >
              {s}x
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
