"use client";

import { useEffect } from "react";

export function TransparencyDetector() {
  useEffect(() => {
    const manualOverride = localStorage.getItem("nayru-transparent-mode");
    if (manualOverride === "true") {
      document.documentElement.classList.add("transparent-mode");
      return;
    } else if (manualOverride === "false") {
      document.documentElement.classList.remove("transparent-mode");
      return;
    }

    const ua = navigator.userAgent;
    const isFirefoxBased = ua.includes("Firefox") && !ua.includes("Chrome");
    const isLinux = ua.includes("Linux") && !ua.includes("Android");
    const supportsBackdrop = CSS.supports("backdrop-filter", "blur(1px)");

    if (isFirefoxBased && isLinux && supportsBackdrop) {
      document.documentElement.classList.add("transparent-mode");
    }
  }, []);

  return null;
}
