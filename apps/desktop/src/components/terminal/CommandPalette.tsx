import React, { useEffect, useRef, useState } from "react";
import {
  ArrowRight,
  CandlestickChart,
  CornerDownLeft,
  DollarSign,
  FileText,
  Search,
  Sliders,
  TrendingUp,
  X,
} from "lucide-react";
import { useWorkspaceStore } from "../../app/store/workspaceStore";
import { executeTerminalCommand, searchInstruments } from "../../lib/tauri/ipc";
import { InstrumentSearchItemDto, WorkspaceType } from "../../types";

export const CommandPalette: React.FC = () => {
  const {
    commandPaletteOpen,
    setCommandPaletteOpen,
    setActiveAsset,
    setWorkspace,
    recentSecurities,
  } = useWorkspaceStore();

  const [query, setQuery] = useState("");
  const [searchResults, setSearchResults] = useState<InstrumentSearchItemDto[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setCommandPaletteOpen(!commandPaletteOpen);
      } else if (e.key === "/" && !commandPaletteOpen && document.activeElement?.tagName !== "INPUT") {
        e.preventDefault();
        setCommandPaletteOpen(true);
      } else if (e.key === "Escape" && commandPaletteOpen) {
        setCommandPaletteOpen(false);
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [commandPaletteOpen, setCommandPaletteOpen]);

  useEffect(() => {
    if (commandPaletteOpen) {
      setTimeout(() => inputRef.current?.focus(), 50);
      setQuery("");
      setSelectedIndex(0);
    }
  }, [commandPaletteOpen]);

  useEffect(() => {
    if (!query.trim()) {
      setSearchResults([]);
      return;
    }

    const timer = setTimeout(async () => {
      try {
        const results = await searchInstruments(query.trim());
        setSearchResults(results);
        setSelectedIndex(0);
      } catch (err) {
        console.error("Search failed:", err);
      }
    }, 150);

    return () => clearTimeout(timer);
  }, [query]);

  if (!commandPaletteOpen) return null;

  const handleSelectAsset = (key: string) => {
    setActiveAsset(key, "A");
    setCommandPaletteOpen(false);
  };

  const handleExecute = async () => {
    const text = query.trim();
    if (!text) return;

    // Check if selecting an item from the search list
    if (searchResults.length > 0 && selectedIndex < searchResults.length) {
      handleSelectAsset(searchResults[selectedIndex].key);
      return;
    }

    try {
      const outcome = await executeTerminalCommand(text);
      if (outcome.workspace_target) {
        setWorkspace(outcome.workspace_target as WorkspaceType);
      }
      if (outcome.asset_target) {
        setActiveAsset(outcome.asset_target, "A");
      }
    } catch (err) {
      console.error("Failed to execute command:", err);
    }

    setCommandPaletteOpen(false);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIndex((prev) => Math.min(prev + 1, searchResults.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIndex((prev) => Math.max(prev - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      handleExecute();
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center pt-20 bg-background/80 backdrop-blur-sm">
      <div className="w-full max-w-2xl bg-surface border border-border shadow-2xl rounded-lg overflow-hidden flex flex-col font-sans">
        {/* Search Input Bar */}
        <div className="flex items-center px-4 py-3 border-b border-border bg-surface-raised space-x-3">
          <Search className="w-4 h-4 text-accent animate-pulse" />
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Type a ticker (e.g. BTCUSDT, NVDA, AAPL) or command (e.g. GP, FA, CN, EQS, PORT)..."
            className="flex-1 bg-transparent border-none outline-none text-foreground placeholder:text-foreground-muted font-mono text-sm"
          />
          <button
            onClick={() => setCommandPaletteOpen(false)}
            className="text-foreground-muted hover:text-foreground transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Results & Quick Actions */}
        <div className="max-h-96 overflow-y-auto p-2 space-y-1">
          {searchResults.length > 0 ? (
            <div>
              <div className="px-2 py-1 text-2xs font-mono uppercase text-foreground-muted font-bold">
                Instruments ({searchResults.length})
              </div>
              {searchResults.map((item, idx) => (
                <button
                  key={item.key}
                  onClick={() => handleSelectAsset(item.key)}
                  className={`w-full flex items-center justify-between px-3 py-2 rounded text-left transition-colors font-mono text-xs ${
                    idx === selectedIndex
                      ? "bg-accent/20 border border-accent/40 text-foreground"
                      : "hover:bg-surface-raised text-foreground-muted hover:text-foreground"
                  }`}
                >
                  <div className="flex items-center space-x-2">
                    <CandlestickChart className="w-4 h-4 text-accent" />
                    <span className="font-bold text-foreground">{item.symbol}</span>
                    <span className="text-2xs text-foreground-muted">[{item.venue}]</span>
                    <span className="text-2xs px-1.5 py-0.5 rounded bg-surface-raised border border-border text-foreground-muted">
                      {item.asset_class}
                    </span>
                  </div>
                  <div className="flex items-center space-x-2 text-2xs text-foreground-muted">
                    <span>{item.base_asset}/{item.quote_currency}</span>
                    <CornerDownLeft className="w-3.5 h-3.5 text-accent" />
                  </div>
                </button>
              ))}
            </div>
          ) : query.trim() ? (
            <div className="px-4 py-6 text-center text-foreground-muted text-xs font-mono">
              Press <kbd className="px-1.5 py-0.5 bg-surface-raised border border-border rounded text-accent font-bold">Enter</kbd> to execute "{query}" via Bloomberg Command Grammar
            </div>
          ) : (
            <div>
              <div className="px-2 py-1 text-2xs font-mono uppercase text-foreground-muted font-bold">
                Recent Securities
              </div>
              <div className="grid grid-cols-2 gap-1 mb-3">
                {recentSecurities.map((sec) => (
                  <button
                    key={sec}
                    onClick={() => handleSelectAsset(sec)}
                    className="flex items-center space-x-2 px-3 py-1.5 rounded bg-surface-raised hover:bg-surface-hover border border-border text-left font-mono text-xs transition-colors"
                  >
                    <TrendingUp className="w-3.5 h-3.5 text-accent" />
                    <span className="font-bold text-foreground">{sec}</span>
                  </button>
                ))}
              </div>

              <div className="px-2 py-1 text-2xs font-mono uppercase text-foreground-muted font-bold">
                Quick Bloomberg Navigation Commands
              </div>
              <div className="grid grid-cols-3 gap-1">
                {[
                  { cmd: "MKT", desc: "Market Monitor" },
                  { cmd: "GP", desc: "Candlestick Chart" },
                  { cmd: "FA", desc: "SEC Financials" },
                  { cmd: "CN", desc: "Live News Workstation" },
                  { cmd: "EQS", desc: "Screener" },
                  { cmd: "PORT", desc: "Portfolio Ledger" },
                  { cmd: "ECON", desc: "US Treasury Curve" },
                  { cmd: "DES", desc: "Security Tearsheet" },
                  { cmd: "LP", desc: "Launchpad 2×2" },
                ].map((c) => (
                  <button
                    key={c.cmd}
                    onClick={() => {
                      executeTerminalCommand(c.cmd);
                      setCommandPaletteOpen(false);
                    }}
                    className="flex flex-col px-2.5 py-1.5 rounded bg-surface-raised hover:bg-surface-hover border border-border text-left font-mono transition-colors"
                  >
                    <span className="text-xs font-bold text-accent">{c.cmd} &lt;GO&gt;</span>
                    <span className="text-2xs text-foreground-muted">{c.desc}</span>
                  </button>
                ))}
              </div>
            </div>
          )}
        </div>

        {/* Footer info bar */}
        <div className="px-4 py-2 bg-surface-raised border-t border-border flex items-center justify-between text-2xs font-mono text-foreground-muted">
          <span>Navigate: ↑ ↓ · Select: Enter · Dismiss: Esc</span>
          <span className="text-accent font-bold">Open Market Terminal</span>
        </div>
      </div>
    </div>
  );
};
