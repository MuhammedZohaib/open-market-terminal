import React, { useState } from "react";
import {
  ExternalLink,
  Maximize2,
  Minimize2,
  LayoutGrid,
  TrendingUp,
  BarChart2,
  Newspaper,
  Shield,
  Layers,
} from "lucide-react";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";
import { useWorkspaceStore } from "../../app/store/workspaceStore";
import { WorkspaceType } from "../../types";
import { MarketWorkspace } from "../market/MarketWorkspace";
import { ChartWorkspace } from "../chart/ChartWorkspace";
import { NewsWorkspace } from "../news/NewsWorkspace";
import { SecurityOverviewWorkspace } from "../security/SecurityOverviewWorkspace";

type PanelId = "MKT" | "GP" | "CN" | "DES";

interface PanelMeta {
  id: PanelId;
  targetWs: WorkspaceType;
  mnemonic: string;
  title: string;
  icon: React.ReactNode;
}

const PANELS: Record<PanelId, PanelMeta> = {
  MKT: {
    id: "MKT",
    targetWs: "MARKET",
    mnemonic: "MKT",
    title: "Market Watchlist",
    icon: <BarChart2 className="w-3.5 h-3.5 text-accent" />,
  },
  GP: {
    id: "GP",
    targetWs: "CHART",
    mnemonic: "GP",
    title: "Candlestick Chart",
    icon: <TrendingUp className="w-3.5 h-3.5 text-emerald-400" />,
  },
  CN: {
    id: "CN",
    targetWs: "NEWS",
    mnemonic: "CN",
    title: "News Feed",
    icon: <Newspaper className="w-3.5 h-3.5 text-sky-400" />,
  },
  DES: {
    id: "DES",
    targetWs: "DES",
    mnemonic: "DES",
    title: "Security Overview",
    icon: <Shield className="w-3.5 h-3.5 text-amber-400" />,
  },
};

export const LaunchpadWorkspace: React.FC = () => {
  const { setWorkspace, activeAsset } = useWorkspaceStore();
  const [maximizedPanel, setMaximizedPanel] = useState<PanelId | null>(null);

  const toggleMaximize = (id: PanelId) => {
    setMaximizedPanel((prev) => (prev === id ? null : id));
  };

  const renderPanelHeader = (id: PanelId) => {
    const meta = PANELS[id];
    const isMax = maximizedPanel === id;

    return (
      <div className="h-7 bg-surface-raised border-b border-border px-2.5 flex items-center justify-between select-none">
        <div className="flex items-center space-x-2">
          <span className="font-bold text-[10px] px-1.5 py-0.5 rounded bg-surface border border-border text-accent">
            [{meta.mnemonic}]
          </span>
          <span className="font-semibold text-xs text-foreground flex items-center space-x-1.5">
            {meta.icon}
            <span className="ml-1">{meta.title}</span>
          </span>
          {/* Link Group indicator */}
          <span className="inline-flex items-center space-x-1 text-[10px] px-1.5 py-0.2 rounded bg-amber-500/10 border border-amber-500/30 text-amber-400">
            <span className="w-1.5 h-1.5 rounded-full bg-amber-400 animate-pulse" />
            <span>LINK A</span>
          </span>
        </div>

        <div className="flex items-center space-x-1">
          <button
            onClick={() => setWorkspace(meta.targetWs)}
            title={`Expand to full ${meta.mnemonic} workspace`}
            className="p-1 rounded text-foreground-muted hover:text-foreground hover:bg-surface transition-colors"
          >
            <ExternalLink className="w-3.5 h-3.5" />
          </button>
          <button
            onClick={() => toggleMaximize(id)}
            title={isMax ? "Restore 2x2 grid" : "Maximize panel"}
            className="p-1 rounded text-foreground-muted hover:text-foreground hover:bg-surface transition-colors"
          >
            {isMax ? (
              <Minimize2 className="w-3.5 h-3.5 text-accent" />
            ) : (
              <Maximize2 className="w-3.5 h-3.5" />
            )}
          </button>
        </div>
      </div>
    );
  };

  const renderPanelContent = (id: PanelId) => {
    switch (id) {
      case "MKT":
        return <MarketWorkspace />;
      case "GP":
        return <ChartWorkspace />;
      case "CN":
        return <NewsWorkspace />;
      case "DES":
        return <SecurityOverviewWorkspace />;
    }
  };

  const renderPanel = (id: PanelId) => (
    <div className="flex flex-col h-full w-full bg-surface overflow-hidden border border-border/60">
      {renderPanelHeader(id)}
      <div className="flex-1 min-h-0 overflow-hidden relative">
        {renderPanelContent(id)}
      </div>
    </div>
  );

  return (
    <div className="flex-1 flex flex-col h-full w-full bg-background overflow-hidden p-1.5 font-mono">
      {maximizedPanel ? (
        <div className="flex flex-col h-full w-full">
          <div className="bg-accent/10 border border-accent/30 text-accent text-xs px-3 py-1 mb-1.5 rounded flex items-center justify-between">
            <span className="flex items-center space-x-2">
              <LayoutGrid className="w-4 h-4" />
              <span>
                Panel <strong>[{PANELS[maximizedPanel].mnemonic}]</strong> maximized. Showing full viewport.
              </span>
            </span>
            <button
              onClick={() => setMaximizedPanel(null)}
              className="text-xs px-2 py-0.5 rounded bg-accent text-black font-bold hover:bg-accent/90"
            >
              Restore 2x2 Grid (Esc)
            </button>
          </div>
          <div className="flex-1 min-h-0">
            {renderPanel(maximizedPanel)}
          </div>
        </div>
      ) : (
        <PanelGroup direction="horizontal" className="h-full w-full">
          {/* Left Column: Market & News */}
          <Panel defaultSize={48} minSize={25}>
            <PanelGroup direction="vertical" className="h-full w-full">
              <Panel defaultSize={52} minSize={20}>
                {renderPanel("MKT")}
              </Panel>
              <PanelResizeHandle className="h-1.5 bg-background hover:bg-accent/40 active:bg-accent transition-colors cursor-row-resize" />
              <Panel defaultSize={48} minSize={20}>
                {renderPanel("CN")}
              </Panel>
            </PanelGroup>
          </Panel>

          <PanelResizeHandle className="w-1.5 bg-background hover:bg-accent/40 active:bg-accent transition-colors cursor-col-resize" />

          {/* Right Column: Chart & Security Overview */}
          <Panel defaultSize={52} minSize={25}>
            <PanelGroup direction="vertical" className="h-full w-full">
              <Panel defaultSize={55} minSize={20}>
                {renderPanel("GP")}
              </Panel>
              <PanelResizeHandle className="h-1.5 bg-background hover:bg-accent/40 active:bg-accent transition-colors cursor-row-resize" />
              <Panel defaultSize={45} minSize={20}>
                {renderPanel("DES")}
              </Panel>
            </PanelGroup>
          </Panel>
        </PanelGroup>
      )}
    </div>
  );
};
