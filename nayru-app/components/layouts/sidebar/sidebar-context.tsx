"use client";

import { useIsMobile } from "@/hooks/use-mobile";
import { createContext, useContext, useEffect, useState } from "react";

const SIDEBAR_STORAGE_KEY = "nayru-sidebar-mode";

export type SidebarMode = "hidden" | "docked" | "expanded";

type SidebarContextType = {
  mode: SidebarMode;
  setMode: (mode: SidebarMode) => void;
  isMobile: boolean;
  isOpen: boolean;
  isCollapsed: boolean;
  toggleSidebar: () => void;
  toggleCollapse: () => void;
};

const SidebarContext = createContext<SidebarContextType | null>(null);

export function useSidebarContext() {
  const context = useContext(SidebarContext);
  if (!context) {
    throw new Error("useSidebarContext must be used within a SidebarProvider");
  }
  return context;
}

function getInitialMode(): SidebarMode {
  if (typeof window === "undefined") return "hidden";
  try {
    const stored = localStorage.getItem(SIDEBAR_STORAGE_KEY) as SidebarMode;
    if (stored === "hidden" || stored === "docked" || stored === "expanded") {
      return stored;
    }
  } catch {}
  return "hidden";
}

export function SidebarProvider({ children }: { children: React.ReactNode }) {
  const [mode, setModeState] = useState<SidebarMode>("hidden");
  const [isHydrated, setIsHydrated] = useState(false);
  const [lastDockMode, setLastDockMode] = useState<"docked" | "expanded">("docked");
  const isMobile = useIsMobile();

  useEffect(() => {
    const initial = getInitialMode();
    setModeState(initial);
    if (initial !== "hidden") {
      setLastDockMode(initial as "docked" | "expanded");
    }
    setIsHydrated(true);
  }, []);

  const setMode = (newMode: SidebarMode) => {
    setModeState(newMode);
    if (newMode !== "hidden") {
      setLastDockMode(newMode);
    }
    try {
      localStorage.setItem(SIDEBAR_STORAGE_KEY, newMode);
    } catch {}
  };

  const toggleSidebar = () => {
    if (mode === "hidden") {
      setMode(lastDockMode);
    } else {
      setMode("hidden");
    }
  };

  const toggleCollapse = () => {
    if (mode === "hidden") {
      setMode("docked");
    } else if (mode === "docked") {
      setMode("expanded");
    } else {
      setMode("docked");
    }
  };

  const currentMode = isHydrated ? mode : "hidden";

  return (
    <SidebarContext.Provider
      value={{
        mode: currentMode,
        setMode,
        isMobile,
        isOpen: currentMode !== "hidden",
        isCollapsed: currentMode === "docked",
        toggleSidebar,
        toggleCollapse,
      }}
    >
      {children}
    </SidebarContext.Provider>
  );
}
