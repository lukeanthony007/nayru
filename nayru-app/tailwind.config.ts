import type { Config } from "tailwindcss";
import defaultTheme from "tailwindcss/defaultTheme";

const config: Config = {
  content: ["./app/**/*.{ts,tsx}", "./components/**/*.{ts,tsx}", "./lib/**/*.{ts,tsx}", "./hooks/**/*.{ts,tsx}"],
  darkMode: ["class"],
  theme: {
    extend: {
      fontFamily: {
        sans: ["Satoshi", "system-ui", "-apple-system", "Segoe UI", ...defaultTheme.fontFamily.sans],
      },
      screens: {
        sidebar: "850px",
      },
      colors: {
        current: "currentColor",
        transparent: "transparent",
        white: "#FFFFFF",

        background: {
          DEFAULT: "#09090b",
          secondary: "#0c0c0f",
          tertiary: "#121215",
        },

        surface: {
          DEFAULT: "#18181b",
          elevated: "#27272a",
          hover: "#3f3f46",
        },

        accent: {
          emerald: "#10b981",
          violet: "#a78bfa",
          amber: "#fbbf24",
          red: "#f87171",
        },

        border: {
          DEFAULT: "rgba(255, 255, 255, 0.08)",
          subtle: "rgba(255, 255, 255, 0.04)",
          accent: "rgba(16, 185, 129, 0.3)",
        },

        text: {
          primary: "#fafafa",
          secondary: "#a1a1aa",
          muted: "#71717a",
          accent: "#10b981",
        },
      },
    },
  },
  plugins: [],
};

export default config;
