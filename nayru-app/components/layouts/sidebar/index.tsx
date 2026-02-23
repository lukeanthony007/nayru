"use client";

import { cn } from "@/lib/utils";
import { usePathname } from "next/navigation";
import { useRef, useState, useCallback } from "react";
import { BookOpenIcon, FolderIcon, SettingsIcon } from "./icons";
import { MenuItem } from "./menu-item";
import { useSidebarContext, SidebarMode } from "./sidebar-context";

export function Sidebar() {
  const pathname = usePathname();
  const { mode, setMode } = useSidebarContext();
  const [isDragging, setIsDragging] = useState(false);
  const dragStartX = useRef(0);
  const startMode = useRef<SidebarMode>(mode);

  const handleDragStart = useCallback((e: React.MouseEvent | React.TouchEvent) => {
    if (mode === "hidden") return;
    setIsDragging(true);
    startMode.current = mode;
    const clientX = "touches" in e ? e.touches[0].clientX : e.clientX;
    dragStartX.current = clientX;

    const handleMove = (moveEvent: MouseEvent | TouchEvent) => {
      const currentX = "touches" in moveEvent ? moveEvent.touches[0].clientX : moveEvent.clientX;
      const delta = currentX - dragStartX.current;
      if (startMode.current === "docked" && delta > 50) {
        setMode("expanded");
        startMode.current = "expanded";
        dragStartX.current = currentX;
      } else if (startMode.current === "expanded" && delta < -50) {
        setMode("docked");
        startMode.current = "docked";
        dragStartX.current = currentX;
      }
    };

    const handleEnd = () => {
      setIsDragging(false);
      document.removeEventListener("mousemove", handleMove);
      document.removeEventListener("mouseup", handleEnd);
      document.removeEventListener("touchmove", handleMove);
      document.removeEventListener("touchend", handleEnd);
    };

    document.addEventListener("mousemove", handleMove);
    document.addEventListener("mouseup", handleEnd);
    document.addEventListener("touchmove", handleMove);
    document.addEventListener("touchend", handleEnd);
  }, [mode, setMode]);

  const getWidth = () => {
    switch (mode) {
      case "hidden": return "w-0";
      case "docked": return "w-14";
      case "expanded": return "w-50";
    }
  };

  const showLabels = mode === "expanded";
  const isVisible = mode !== "hidden";

  return (
    <>
      {isVisible && (
        <div
          className={cn(
            "fixed w-3 cursor-ew-resize z-50 group",
            !isDragging && "transition-[left] duration-300 ease-out"
          )}
          style={{ top: 0, bottom: 0, left: mode === "docked" ? 44 : 188 }}
          onMouseDown={handleDragStart}
          onTouchStart={handleDragStart}
        >
          <div className={cn(
            "absolute right-0 top-0 bottom-0 w-0.5 transition-opacity duration-200",
            isDragging ? "opacity-100 bg-white/20" : "opacity-0 group-hover:opacity-100 bg-white/10"
          )} />
        </div>
      )}

      <aside
        className={cn(
          "fixed left-0 top-0 h-screen overflow-visible z-30 bg-background select-none",
          getWidth(),
          !isVisible && "opacity-0 pointer-events-none",
          !isDragging && "transition-[width,opacity] duration-300 ease-out",
        )}
        aria-label="Main navigation"
      >
        <div className="relative h-full pb-3 px-2 !bg-transparent">
          {/* Reader */}
          <div className="flex items-center h-12">
            <MenuItem
              className={cn(
                "group relative flex items-center",
                "h-8 gap-2 !bg-transparent hover:!bg-transparent",
                showLabels ? "px-4" : "justify-center w-full",
                pathname === "/" ? "text-white/50" : "text-white/30 hover:text-white/50"
              )}
              as="link" href="/" isActive={pathname === "/"}
              title={!showLabels ? "Reader" : undefined}
            >
              <BookOpenIcon className="size-4 shrink-0" />
              {showLabels && <span className="text-sm">Reader</span>}
            </MenuItem>
          </div>

          <div className="mt-2" />

          {/* Library */}
          <MenuItem
            className={cn(
              "group relative flex items-center",
              "h-10 gap-2 !bg-transparent hover:!bg-transparent",
              showLabels ? "px-4" : "justify-center",
              pathname === "/library" ? "text-white/50" : "text-white/30 hover:text-white/50"
            )}
            as="link" href="/library" isActive={pathname === "/library"}
            title={!showLabels ? "Library" : undefined}
          >
            <FolderIcon className="size-4 shrink-0" />
            {showLabels && <span className="text-sm">Library</span>}
          </MenuItem>

          {/* Settings */}
          <div className="absolute bottom-[30px] left-2 right-2">
            <MenuItem
              className={cn(
                "group relative flex items-center",
                "h-10 gap-2 !bg-transparent hover:!bg-transparent",
                showLabels ? "px-4" : "justify-center",
                pathname === "/settings" ? "text-white/50" : "text-white/30 hover:text-white/50"
              )}
              as="link" href="/settings" isActive={pathname === "/settings"}
              title={!showLabels ? "Settings" : undefined}
            >
              <SettingsIcon className="size-4 shrink-0" />
              {showLabels && <span className="text-sm">Settings</span>}
            </MenuItem>
          </div>
        </div>
      </aside>
    </>
  );
}
