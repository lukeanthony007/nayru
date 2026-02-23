"use client";

import { SidebarProvider } from "@/components/layouts/sidebar/sidebar-context";

export function Providers({ children }: { children: React.ReactNode }) {
  return <SidebarProvider>{children}</SidebarProvider>;
}
