"use client";

import { useEffect } from "react";
import { useConfigStore } from "@/lib/stores/config-store";

function resolveTheme(theme: "light" | "dark" | "system"): "light" | "dark" {
  if (theme !== "system") return theme;
  if (typeof window === "undefined") return "dark";
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function BackgroundOpacitySync() {
  const backgroundOpacity = useConfigStore((s) => s.backgroundOpacity);
  const theme = useConfigStore((s) => s.theme);

  useEffect(() => {
    const resolved = resolveTheme(theme);
    const color =
      resolved === "light"
        ? `rgba(250, 250, 250, ${backgroundOpacity})`
        : `rgba(9, 9, 11, ${backgroundOpacity})`;
    document.documentElement.style.backgroundColor = color;
    document.body.style.backgroundColor = color;
  }, [backgroundOpacity, theme]);

  useEffect(() => {
    if (theme !== "system") return;
    const mql = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => {
      const resolved = resolveTheme("system");
      const color =
        resolved === "light"
          ? `rgba(250, 250, 250, ${backgroundOpacity})`
          : `rgba(9, 9, 11, ${backgroundOpacity})`;
      document.documentElement.style.backgroundColor = color;
      document.body.style.backgroundColor = color;
    };
    mql.addEventListener("change", handler);
    return () => mql.removeEventListener("change", handler);
  }, [theme, backgroundOpacity]);

  return null;
}
