"use client";

import { useEffect } from "react";
import { useConfigStore } from "@/lib/stores/config-store";

function resolveTheme(theme: "light" | "dark" | "system"): "light" | "dark" {
  if (theme !== "system") return theme;
  if (typeof window === "undefined") return "dark";
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function ThemeSync() {
  const theme = useConfigStore((s) => s.theme);

  useEffect(() => {
    const resolved = resolveTheme(theme);
    document.documentElement.classList.remove("dark", "light");
    document.documentElement.classList.add(resolved);
    document.documentElement.style.colorScheme = resolved;
  }, [theme]);

  useEffect(() => {
    if (theme !== "system") return;
    const mql = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => {
      const resolved = resolveTheme("system");
      document.documentElement.classList.remove("dark", "light");
      document.documentElement.classList.add(resolved);
      document.documentElement.style.colorScheme = resolved;
    };
    mql.addEventListener("change", handler);
    return () => mql.removeEventListener("change", handler);
  }, [theme]);

  return null;
}
