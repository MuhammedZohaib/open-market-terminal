import React, { useEffect, useState } from "react";
import {
  Activity,
  Command,
  Maximize2,
  Minimize2,
  RefreshCw,
  Search,
  Settings,
  Sliders,
  Wifi,
  WifiOff,
} from "lucide-react";
import { useWorkspaceStore } from "../../app/store/workspaceStore";
import { DensityMode, LinkGroup } from "../../types";

export const Header: React.FC = () => {
  const {
    workspace,
    activeAsset,
    activeTimeframe,
    linkGroupAAsset,
    linkGroupBAsset,
    density,
    connectionStatus,
    setDensity,
    setCommandPaletteOpen,
    setActiveAsset,
    setWorkspace,
  } = useWorkspaceStore();

  const [timeUtc, setTimeUtc] = useState("");
  const [timeLocal, setTimeLocal] = useState("");

  useEffect(() => {
    const updateTime = () => {
      const now = new Date();
      setTimeUtc(
        now.toISOString().substring(11, 19) + " UTC"
      );
      setTimeLocal(
        now.toLocaleTimeString("en-US", { hour12: false })
      );
    };
    updateTime();
    const interval = setInterval(updateTime, 1000);
    return () => clearInterval(interval);
  }, []);

  const [linkGroup, setLinkGroup] = useState<LinkGroup>("A");

  const cycleLinkGroup = () => {
    const nextGroup: LinkGroup =
      linkGroup === "A" ? "B" : linkGroup === "B" ? "NONE" : "A";
    setLinkGroup(nextGroup);
  };

  const isConnected =
    connectionStatus.toLowerCase().includes("live") ||
    connectionStatus.toLowerCase().includes("trading") ||
    connectionStatus.toLowerCase().includes("ready");

  const sym = activeAsset.includes(":") ? activeAsset.split(":")[1] : activeAsset;
  const venue = activeAsset.includes(":") ? activeAsset.split(":")[0] : "BINANCE";

  return (
    <header className="h-10 bg-surface border-b border-border flex items-center justify-between px-3 text-xs select-none z-30">
      {/* Left: Logo & Active Security Badge */}
      <div className="flex items-center space-x-3">
        <div className="flex items-center space-x-1.5 cursor-pointer" onClick={() => setWorkspace("LAUNCHPAD")}>
          <div className="w-5 h-5 rounded bg-accent flex items-center justify-center font-bold text-background text-2xs tracking-tighter">
            OMT
          </div>
          <span className="font-bold text-foreground tracking-wider uppercase hidden sm:inline">
            Open Market Terminal
          </span>
        </div>

        <div className="h-4 w-px bg-border mx-1" />

        {/* Security Context & Link Group Badge */}
        <div className="flex items-center space-x-1.5 bg-surface-raised border border-border rounded px-2 py-0.5">
          <button
            onClick={cycleLinkGroup}
            className={`px-1 rounded text-2xs font-bold transition-colors ${
              linkGroup === "A"
                ? "bg-accent/20 text-accent border border-accent/40"
                : linkGroup === "B"
                ? "bg-terminal-cyan/20 text-terminal-cyan border border-terminal-cyan/40"
                : "bg-surface-hover text-foreground-muted border border-border"
            }`}
            title="Cycle Bloomberg Link Group (A, B, or Unlinked)"
          >
            LG: {linkGroup}
          </button>

          <span className="font-mono font-bold text-foreground text-sm">{sym}</span>
          <span className="text-foreground-muted text-2xs uppercase">[{venue}]</span>
          <span className="text-foreground-muted text-2xs">· {activeTimeframe.toUpperCase()}</span>
        </div>
      </div>

      {/* Center: Universal Command Search Trigger */}
      <div className="flex-1 max-w-md mx-4">
        <button
          onClick={() => setCommandPaletteOpen(true)}
          className="w-full flex items-center justify-between bg-background border border-border hover:border-accent/60 rounded px-2.5 py-1 text-foreground-muted hover:text-foreground text-xs transition-colors group"
        >
          <div className="flex items-center space-x-2">
            <Search className="w-3.5 h-3.5 text-accent" />
            <span className="font-mono text-xs">
              <span className="text-foreground-muted">RUN &gt; </span>
              <span className="group-hover:text-foreground">Search ticker or command...</span>
            </span>
          </div>
          <kbd className="hidden sm:inline-flex items-center space-x-0.5 bg-surface-raised border border-border px-1.5 py-0.2 rounded text-2xs text-foreground-muted font-mono">
            <span>Ctrl</span>
            <span>+</span>
            <span>K</span>
          </kbd>
        </button>
      </div>

      {/* Right: Telemetry, Density, Status & Clocks */}
      <div className="flex items-center space-x-3 text-2xs text-foreground-muted font-mono">
        {/* Density Mode Switcher */}
        <div className="hidden lg:flex items-center bg-surface-raised border border-border rounded p-0.5 space-x-0.5">
          {(["COMPACT", "COMFORTABLE", "ULTRA_DENSE"] as DensityMode[]).map((d) => (
            <button
              key={d}
              onClick={() => setDensity(d)}
              className={`px-1.5 py-0.5 rounded text-2xs transition-colors ${
                density === d
                  ? "bg-accent text-background font-bold"
                  : "hover:text-foreground text-foreground-muted"
              }`}
              title={`Switch density to ${d}`}
            >
              {d === "COMPACT" ? "Compact" : d === "COMFORTABLE" ? "Comfort" : "Dense"}
            </button>
          ))}
        </div>

        {/* Connection Status Pill */}
        <div
          className={`flex items-center space-x-1.5 px-2 py-0.5 rounded border text-2xs ${
            isConnected
              ? "bg-positive/10 border-positive/30 text-positive"
              : "bg-accent/10 border-accent/30 text-accent"
          }`}
          title={`Market feed: ${connectionStatus}`}
        >
          <span className={`w-1.5 h-1.5 rounded-full ${isConnected ? "bg-positive animate-pulse" : "bg-accent"}`} />
          <span className="font-sans uppercase tracking-tight">{connectionStatus}</span>
        </div>

        {/* Clocks */}
        <div className="hidden xl:flex flex-col text-right leading-none">
          <span className="text-foreground font-bold">{timeUtc}</span>
          <span className="text-foreground-muted text-2xs">{timeLocal} LOC</span>
        </div>
      </div>
    </header>
  );
};
