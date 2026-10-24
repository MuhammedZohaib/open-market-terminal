/** @type {import('tailwindcss').Config} */
export default {
  darkMode: ["class"],
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        background: "#0b0e12",
        surface: {
          DEFAULT: "#10151b",
          raised: "#171d25",
          hover: "#202832",
        },
        border: {
          DEFAULT: "#29313b",
          subtle: "#1c242d",
        },
        foreground: {
          DEFAULT: "#e9edf2",
          muted: "#8b97a6",
        },
        accent: {
          DEFAULT: "#e9a441",
          hover: "#f4b85c",
        },
        positive: {
          DEFAULT: "#38b887",
          muted: "rgba(56, 184, 135, 0.15)",
        },
        negative: {
          DEFAULT: "#ed6976",
          muted: "rgba(237, 105, 118, 0.15)",
        },
        terminal: {
          amber: "#e9a441",
          cyan: "#38c0e0",
          blue: "#68a6ee",
          gray: "#5a6675",
        },
      },
      fontFamily: {
        sans: ["Inter", "-apple-system", "BlinkMacSystemFont", "Segoe UI", "sans-serif"],
        mono: ["JetBrains Mono", "SF Mono", "Consolas", "monospace"],
      },
      fontSize: {
        "2xs": ["10px", "14px"],
        xs: ["11px", "15px"],
        sm: ["12px", "16px"],
        base: ["13px", "18px"],
        md: ["14px", "20px"],
        lg: ["16px", "22px"],
        xl: ["20px", "26px"],
        "2xl": ["24px", "30px"],
      },
    },
  },
  plugins: [],
};
