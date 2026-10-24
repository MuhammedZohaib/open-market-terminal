import React from "react";
import {
  CheckCircle2,
  Cpu,
  Database,
  Globe,
  HardDrive,
  Keyboard,
  Layers,
  Monitor,
  RefreshCw,
  Server,
  Settings,
  Shield,
  Zap,
} from "lucide-react";
import { useWorkspaceStore } from "../../app/store/workspaceStore";

export const SettingsWorkspace: React.FC = () => {
  const { density, setDensity, connectionStatus, totalInstruments, activeInstruments } =
    useWorkspaceStore();

  const SHORTCUTS = [
    { key: "Ctrl + K / /", desc: "Open global command palette & search", cat: "Navigation" },
    { key: "F1 / 1", desc: "Switch to Launchpad (2x2 Multi-pane grid)", cat: "Workspace" },
    { key: "F2 / 2", desc: "Switch to Market Watchlist (MKT)", cat: "Workspace" },
    { key: "F3 / 3", desc: "Switch to Candlestick Chart (GP)", cat: "Workspace" },
    { key: "F4 / 4", desc: "Switch to Financials & SEC XBRL (FA)", cat: "Workspace" },
    { key: "F5 / 5", desc: "Switch to Terminal Newsfeed (CN)", cat: "Workspace" },
    { key: "F6 / 6", desc: "Switch to Security Screener (EQS)", cat: "Workspace" },
    { key: "F7 / 7", desc: "Switch to Portfolio Ledger (PORT)", cat: "Workspace" },
    { key: "F8 / 8", desc: "Switch to Treasury Yield Curve (ECON)", cat: "Workspace" },
    { key: "F9 / 9", desc: "Switch to Security Overview (DES)", cat: "Workspace" },
    { key: "F10 / ,", desc: "Open Terminal Settings", cat: "Workspace" },
    { key: "Esc", desc: "Dismiss command palette or restore panels", cat: "Interface" },
    { key: "Space / Enter", desc: "Select and link instrument", cat: "Interaction" },
    { key: "Up / Down", desc: "Navigate rows and result sets", cat: "Interaction" },
  ];

  return (
    <div className="flex-1 flex flex-col h-full bg-background overflow-y-auto p-6 font-mono text-xs select-none space-y-6">
      {/* Settings Title */}
      <div className="border-b border-border pb-4 flex items-center justify-between">
        <div>
          <h1 className="text-xl font-bold text-foreground flex items-center space-x-2">
            <Settings className="w-5 h-5 text-accent" />
            <span>TERMINAL SETTINGS & PREFERENCES</span>
          </h1>
          <p className="text-foreground-muted mt-1">
            Global workstation configuration, market data engines, and keyboard shortcuts
          </p>
        </div>
        <div className="flex items-center space-x-2">
          <span className="px-2.5 py-1 rounded bg-surface border border-border text-accent font-semibold flex items-center space-x-1.5">
            <span className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse" />
            <span>Tauri 2 Native Workstation</span>
          </span>
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        {/* Visual Ergonomics & Display Density */}
        <div className="bg-surface border border-border rounded-lg p-5 space-y-4">
          <div className="flex items-center space-x-2 text-foreground font-semibold border-b border-border pb-2">
            <Monitor className="w-4 h-4 text-accent" />
            <span>DISPLAY & ERGONOMICS</span>
          </div>

          <div className="space-y-3">
            <div>
              <label className="text-foreground-muted block mb-1.5">Workspace Density</label>
              <div className="grid grid-cols-2 gap-2">
                <button
                  onClick={() => setDensity("COMPACT")}
                  className={`p-3 rounded border text-left flex flex-col justify-between transition-colors ${
                    density === "COMPACT"
                      ? "bg-accent/10 border-accent text-accent"
                      : "bg-surface-raised border-border text-foreground hover:bg-surface-raised/80"
                  }`}
                >
                  <div className="font-bold flex items-center justify-between">
                    <span>Compact Mode</span>
                    {density === "COMPACT" && <CheckCircle2 className="w-4 h-4 text-accent" />}
                  </div>
                  <span className="text-[11px] text-foreground-muted mt-1">
                    Dense institutional layout, 11px font, maximized data throughput
                  </span>
                </button>

                <button
                  onClick={() => setDensity("COMFORTABLE")}
                  className={`p-3 rounded border text-left flex flex-col justify-between transition-colors ${
                    density === "COMFORTABLE"
                      ? "bg-accent/10 border-accent text-accent"
                      : "bg-surface-raised border-border text-foreground hover:bg-surface-raised/80"
                  }`}
                >
                  <div className="font-bold flex items-center justify-between">
                    <span>Comfortable</span>
                    {density === "COMFORTABLE" && <CheckCircle2 className="w-4 h-4 text-accent" />}
                  </div>
                  <span className="text-[11px] text-foreground-muted mt-1">
                    Relaxed padding, 13px font, standard laptop screen readability
                  </span>
                </button>
              </div>
            </div>

            <div className="pt-2">
              <label className="text-foreground-muted block mb-1.5">Color Palette</label>
              <div className="p-3 rounded bg-surface-raised border border-border flex items-center justify-between">
                <div>
                  <div className="font-bold text-foreground">Institutional Charcoal / Amber</div>
                  <div className="text-[11px] text-foreground-muted">
                    High-contrast dark mode with financial semantic colors (#10B981, #EF4444, #F59E0B)
                  </div>
                </div>
                <div className="flex space-x-1.5">
                  <span className="w-4 h-4 rounded bg-[#0B0F19] border border-border" />
                  <span className="w-4 h-4 rounded bg-[#10B981]" />
                  <span className="w-4 h-4 rounded bg-[#EF4444]" />
                  <span className="w-4 h-4 rounded bg-[#F59E0B]" />
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Data Ingestion & Engine Status */}
        <div className="bg-surface border border-border rounded-lg p-5 space-y-4">
          <div className="flex items-center space-x-2 text-foreground font-semibold border-b border-border pb-2">
            <Server className="w-4 h-4 text-accent" />
            <span>ENGINE & DATA PROVIDERS</span>
          </div>

          <div className="space-y-3">
            <div className="p-3 rounded bg-surface-raised border border-border flex items-center justify-between">
              <div className="space-y-0.5">
                <div className="font-bold text-foreground flex items-center space-x-1.5">
                  <Zap className="w-3.5 h-3.5 text-accent" />
                  <span>Binance Spot Realtime Stream</span>
                </div>
                <div className="text-[11px] text-foreground-muted">
                  Coalesced 30 FPS tick broadcast channel (Tokio headless runtime)
                </div>
              </div>
              <span className="px-2 py-0.5 rounded bg-emerald-500/10 border border-emerald-500/30 text-emerald-400 font-bold">
                {connectionStatus}
              </span>
            </div>

            <div className="p-3 rounded bg-surface-raised border border-border flex items-center justify-between">
              <div className="space-y-0.5">
                <div className="font-bold text-foreground flex items-center space-x-1.5">
                  <Globe className="w-3.5 h-3.5 text-sky-400" />
                  <span>SEC EDGAR XBRL Integration</span>
                </div>
                <div className="text-[11px] text-foreground-muted">
                  Official SEC Company Facts REST API with rate-limiting & disk cache
                </div>
              </div>
              <span className="px-2 py-0.5 rounded bg-sky-500/10 border border-sky-500/30 text-sky-400 font-bold">
                ACTIVE
              </span>
            </div>

            <div className="p-3 rounded bg-surface-raised border border-border flex items-center justify-between">
              <div className="space-y-0.5">
                <div className="font-bold text-foreground flex items-center space-x-1.5">
                  <HardDrive className="w-3.5 h-3.5 text-amber-400" />
                  <span>SQLite Embedded Storage</span>
                </div>
                <div className="text-[11px] text-foreground-muted">
                  Local persistence for watchlists, portfolio ledger, and historical cache
                </div>
              </div>
              <span className="px-2 py-0.5 rounded bg-amber-500/10 border border-amber-500/30 text-amber-400 font-bold">
                SYNCHRONIZED
              </span>
            </div>
          </div>
        </div>
      </div>

      {/* Keyboard Shortcuts Reference */}
      <div className="bg-surface border border-border rounded-lg p-5 space-y-4">
        <div className="flex items-center space-x-2 text-foreground font-semibold border-b border-border pb-2">
          <Keyboard className="w-4 h-4 text-accent" />
          <span>WORKSTATION KEYBOARD SHORTCUTS REFERENCE</span>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
          {SHORTCUTS.map((item, idx) => (
            <div
              key={idx}
              className="p-2.5 rounded bg-surface-raised border border-border flex items-center justify-between"
            >
              <div className="space-y-0.5">
                <span className="text-[10px] uppercase font-bold text-accent tracking-wider">
                  {item.cat}
                </span>
                <div className="text-foreground-muted text-[11px]">{item.desc}</div>
              </div>
              <span className="px-2 py-1 rounded bg-background border border-border text-foreground font-bold text-[11px] whitespace-nowrap ml-2">
                {item.key}
              </span>
            </div>
          ))}
        </div>
      </div>

      {/* Architecture & Verification Info */}
      <div className="bg-surface border border-border rounded-lg p-5 space-y-3">
        <div className="flex items-center space-x-2 text-foreground font-semibold border-b border-border pb-2">
          <Cpu className="w-4 h-4 text-accent" />
          <span>ARCHITECTURE & SYSTEMS DIAGNOSTICS</span>
        </div>
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-center">
          <div className="p-3 bg-surface-raised rounded border border-border">
            <div className="text-foreground-muted text-[11px]">RUST ENGINE</div>
            <div className="text-sm font-bold text-foreground mt-1">omt-service (v0.1.0)</div>
          </div>
          <div className="p-3 bg-surface-raised rounded border border-border">
            <div className="text-foreground-muted text-[11px]">DESKTOP SHELL</div>
            <div className="text-sm font-bold text-accent mt-1">Tauri 2.x</div>
          </div>
          <div className="p-3 bg-surface-raised rounded border border-border">
            <div className="text-foreground-muted text-[11px]">INSTRUMENTS LOADED</div>
            <div className="text-sm font-bold text-emerald-400 mt-1">
              {totalInstruments > 0 ? totalInstruments : 48} Instruments
            </div>
          </div>
          <div className="p-3 bg-surface-raised rounded border border-border">
            <div className="text-foreground-muted text-[11px]">IPC INTERFACE</div>
            <div className="text-sm font-bold text-foreground mt-1">Zero-Copy Commands & Events</div>
          </div>
        </div>
      </div>
    </div>
  );
};
