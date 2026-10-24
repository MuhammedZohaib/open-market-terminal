import React, { useEffect } from "react";
import { useWorkspaceStore } from "./app/store/workspaceStore";
import { Header } from "./components/shell/Header";
import { WorkspaceNav } from "./components/shell/WorkspaceNav";
import { CommandPalette } from "./components/terminal/CommandPalette";
import { ChartWorkspace } from "./features/chart/ChartWorkspace";
import { EconomicsWorkspace } from "./features/economics/EconomicsWorkspace";
import { FinancialsWorkspace } from "./features/financials/FinancialsWorkspace";
import { LaunchpadWorkspace } from "./features/launchpad/LaunchpadWorkspace";
import { MarketWorkspace } from "./features/market/MarketWorkspace";
import { NewsWorkspace } from "./features/news/NewsWorkspace";
import { PortfolioWorkspace } from "./features/portfolio/PortfolioWorkspace";
import { ScreenerWorkspace } from "./features/screener/ScreenerWorkspace";
import { SecurityOverviewWorkspace } from "./features/security/SecurityOverviewWorkspace";
import { SettingsWorkspace } from "./features/settings/SettingsWorkspace";
import { getMarketSnapshot, subscribeQuoteStream } from "./lib/tauri/ipc";
import { MarketQuoteDto, WorkspaceType } from "./types";

export const App: React.FC = () => {
  const {
    workspace,
    setWorkspace,
    density,
    setMarketSnapshot,
    batchUpdateQuotes,
    setConnectionStatus,
    setCommandPaletteOpen,
  } = useWorkspaceStore();

  // Load initial snapshot and attach streaming listener
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    setConnectionStatus("Connecting...");
    getMarketSnapshot()
      .then((snapshot) => {
        setMarketSnapshot(snapshot);
        setConnectionStatus("Connected");
      })
      .catch((err) => {
        console.warn("Could not load initial market snapshot:", err);
        setConnectionStatus("Offline Cache");
      });

    subscribeQuoteStream((quotes: MarketQuoteDto[]) => {
      batchUpdateQuotes(quotes);
      setConnectionStatus("Live (30 FPS)");
    })
      .then((cleanup: () => void) => {
        unlisten = cleanup;
      })
      .catch((err: unknown) => {
        console.warn("Tauri event listener registration skipped:", err);
      });

    return () => {
      if (unlisten) unlisten();
    };
  }, [setMarketSnapshot, batchUpdateQuotes, setConnectionStatus]);

  // Global workstation keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const activeEl = document.activeElement;
      const isInput =
        activeEl?.tagName === "INPUT" ||
        activeEl?.tagName === "TEXTAREA" ||
        activeEl?.getAttribute("contenteditable") === "true";

      // Ctrl + K or '/' (when not typing in an input) opens command palette
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setCommandPaletteOpen(true);
        return;
      }

      if (e.key === "/" && !isInput) {
        e.preventDefault();
        setCommandPaletteOpen(true);
        return;
      }

      // Function key workspace switching
      const F_KEYS: Record<string, WorkspaceType> = {
        F1: "LAUNCHPAD",
        F2: "MARKET",
        F3: "CHART",
        F4: "FIN",
        F5: "NEWS",
        F6: "SCAN",
        F7: "PORT",
        F8: "ECON",
        F9: "DES",
        F10: "SETTINGS",
      };

      if (F_KEYS[e.key]) {
        e.preventDefault();
        setWorkspace(F_KEYS[e.key]);
        return;
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [setCommandPaletteOpen, setWorkspace]);

  const renderActiveWorkspace = () => {
    switch (workspace) {
      case "LAUNCHPAD":
        return <LaunchpadWorkspace />;
      case "MARKET":
        return <MarketWorkspace />;
      case "CHART":
        return <ChartWorkspace />;
      case "FIN":
        return <FinancialsWorkspace />;
      case "NEWS":
        return <NewsWorkspace />;
      case "SCAN":
        return <ScreenerWorkspace />;
      case "PORT":
        return <PortfolioWorkspace />;
      case "ECON":
        return <EconomicsWorkspace />;
      case "DES":
        return <SecurityOverviewWorkspace />;
      case "SETTINGS":
        return <SettingsWorkspace />;
      default:
        return <LaunchpadWorkspace />;
    }
  };

  const densityClass =
    density === "COMPACT"
      ? "text-[11px] leading-tight"
      : density === "ULTRA_DENSE"
      ? "text-[10px] leading-none"
      : "text-[13px] leading-normal";

  return (
    <div
      className={`h-screen w-screen flex flex-col bg-background text-foreground font-mono select-none overflow-hidden ${densityClass}`}
    >
      {/* Workstation Top Navigation & Status Bar */}
      <Header />

      {/* Mnemonic Workspace Selector Tabs */}
      <WorkspaceNav />

      {/* Main Terminal Viewport */}
      <main className="flex-1 min-h-0 flex flex-col overflow-hidden relative">
        {renderActiveWorkspace()}
      </main>

      {/* Bloomberg-Style Command Palette (Ctrl+K) */}
      <CommandPalette />
    </div>
  );
};

export default App;
