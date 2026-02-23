"use client";

import { useEffect, useState, useCallback } from "react";
import { cn } from "@/lib/utils";
import { useConfigStore } from "@/lib/stores/config-store";
import { getTtsConfig, setTtsConfig, speakFrom } from "@/lib/tts-ipc";

const VOICES = [
  "af_jadzia",
  "af_heart",
  "af_star",
  "af_bella",
  "af_nicole",
  "af_sarah",
  "af_sky",
  "am_adam",
  "am_michael",
];

export default function SettingsPage() {
  const theme = useConfigStore((s) => s.theme);
  const setTheme = useConfigStore((s) => s.setTheme);
  const backgroundOpacity = useConfigStore((s) => s.backgroundOpacity);
  const setBackgroundOpacity = useConfigStore((s) => s.setBackgroundOpacity);

  const [voice, setVoice] = useState("af_heart");
  const [speed, setSpeed] = useState(1.0);
  const [kokoroUrl, setKokoroUrl] = useState("http://localhost:3001");
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    getTtsConfig()
      .then((config) => {
        setVoice(config.voice);
        setSpeed(config.speed);
        setKokoroUrl(config.kokoro_url);
        setLoaded(true);
      })
      .catch(() => setLoaded(true));
  }, []);

  const handleVoiceChange = useCallback(
    async (newVoice: string) => {
      setVoice(newVoice);
      try {
        await setTtsConfig({ voice: newVoice });
      } catch {}
    },
    [],
  );

  const handleSpeedChange = useCallback(
    async (newSpeed: number) => {
      setSpeed(newSpeed);
      try {
        await setTtsConfig({ speed: newSpeed });
      } catch {}
    },
    [],
  );

  const handleUrlChange = useCallback(
    async (url: string) => {
      setKokoroUrl(url);
      try {
        await setTtsConfig({ kokoro_url: url });
      } catch {}
    },
    [],
  );

  const handlePreview = useCallback(async () => {
    try {
      await speakFrom(
        "This is a preview of the selected voice and speed.",
        0,
      );
    } catch {}
  }, []);

  if (!loaded) return null;

  return (
    <div className="flex flex-col h-screen overflow-y-auto">
      <div className="max-w-lg mx-auto w-full px-6 py-10 space-y-10">
        <h1 className="text-lg font-medium text-white/80">Settings</h1>

        {/* Voice */}
        <Section title="Voice">
          <Label text="Voice">
            <select
              value={voice}
              onChange={(e) => handleVoiceChange(e.target.value)}
              className="w-full bg-white/[0.04] border border-white/[0.08] rounded-lg px-3 py-2 text-sm text-white/70 outline-none focus:border-emerald-500/30"
            >
              {VOICES.map((v) => (
                <option key={v} value={v}>
                  {v}
                </option>
              ))}
            </select>
          </Label>

          <Label text={`Speed: ${speed.toFixed(2)}x`}>
            <input
              type="range"
              min={0.5}
              max={2}
              step={0.05}
              value={speed}
              onChange={(e) => handleSpeedChange(parseFloat(e.target.value))}
              className="w-full accent-emerald-500"
            />
          </Label>

          <button
            onClick={handlePreview}
            className="text-sm text-emerald-400 hover:text-emerald-300 transition-colors"
          >
            Preview voice
          </button>
        </Section>

        {/* Service */}
        <Section title="Service">
          <Label text="Kokoro URL">
            <input
              type="text"
              value={kokoroUrl}
              onChange={(e) => handleUrlChange(e.target.value)}
              className="w-full bg-white/[0.04] border border-white/[0.08] rounded-lg px-3 py-2 text-sm text-white/70 outline-none focus:border-emerald-500/30"
              placeholder="http://localhost:3001"
            />
          </Label>
        </Section>

        {/* Appearance */}
        <Section title="Appearance">
          <Label text="Theme">
            <div className="flex gap-2">
              {(["light", "dark", "system"] as const).map((t) => (
                <button
                  key={t}
                  onClick={() => setTheme(t)}
                  className={cn(
                    "flex-1 px-3 py-1.5 rounded-lg text-sm capitalize transition-colors border",
                    theme === t
                      ? "border-emerald-500/30 text-emerald-400 bg-emerald-500/10"
                      : "border-white/[0.08] text-white/40 hover:text-white/60 hover:bg-white/[0.04]",
                  )}
                >
                  {t}
                </button>
              ))}
            </div>
          </Label>

          <Label text={`Background opacity: ${Math.round(backgroundOpacity * 100)}%`}>
            <input
              type="range"
              min={0}
              max={1}
              step={0.05}
              value={backgroundOpacity}
              onChange={(e) =>
                setBackgroundOpacity(parseFloat(e.target.value))
              }
              className="w-full accent-emerald-500"
            />
          </Label>
        </Section>
      </div>
    </div>
  );
}

function Section({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-4">
      <h2 className="text-sm font-medium text-white/50">{title}</h2>
      <div className="space-y-4">{children}</div>
    </div>
  );
}

function Label({
  text,
  children,
}: {
  text: string;
  children: React.ReactNode;
}) {
  return (
    <label className="block space-y-1.5">
      <span className="text-xs text-white/30">{text}</span>
      {children}
    </label>
  );
}
