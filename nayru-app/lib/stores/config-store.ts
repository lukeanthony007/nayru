import { create } from "zustand";
import { persist } from "zustand/middleware";

export interface ConfigState {
  theme: "light" | "dark" | "system";
  setTheme: (theme: "light" | "dark" | "system") => void;
  backgroundOpacity: number;
  setBackgroundOpacity: (opacity: number) => void;
}

export const useConfigStore = create<ConfigState>()(
  persist(
    (set) => ({
      theme: "dark",
      setTheme: (theme) => set({ theme }),
      backgroundOpacity: 0.55,
      setBackgroundOpacity: (backgroundOpacity) => set({ backgroundOpacity }),
    }),
    {
      name: "nayru-config",
    }
  )
);
