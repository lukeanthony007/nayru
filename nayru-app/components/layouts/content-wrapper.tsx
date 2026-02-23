"use client";

import { cn } from "@/lib/utils";
import { useSidebarContext } from "./sidebar/sidebar-context";

export function ContentWrapper({ children }: { children: React.ReactNode }) {
  const { mode } = useSidebarContext();

  const getPadding = () => {
    switch (mode) {
      case "hidden": return "pl-0";
      case "docked": return "pl-14";
      case "expanded": return "pl-50";
    }
  };

  return (
    <div className={cn(
      "relative w-full flex flex-col flex-1 transition-[padding] duration-300 ease-out",
      getPadding()
    )}>
      {children}
    </div>
  );
}
