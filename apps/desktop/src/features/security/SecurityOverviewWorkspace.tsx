import React from "react";
import {
  Activity,
  BarChart2,
  BookOpen,
  CandlestickChart,
  DollarSign,
  ExternalLink,
  Globe,
  Layers,
  Newspaper,
  Shield,
  TrendingDown,
  TrendingUp,
} from "lucide-react";
import { useWorkspaceStore } from "../../app/store/workspaceStore";
import { formatPercent, formatPrice, formatVolume } from "../../lib/utils";

export const SecurityOverviewWorkspace: React.FC = () => {
  const { activeAsset, quotesMap, setWorkspace } = useWorkspaceStore();
  const q = quotesMap[activeAsset];

  const sym = activeAsset.includes(":") ? activeAsset.split(":")[1] : activeAsset;
  const venue = activeAsset.includes(":") ? activeAsset.split(":")[0] : "BINANCE";

  const isPositive = (q?.change_percent_24h ?? 0) >= 0;

  return (
    <div className="flex-1 flex flex-col h-full bg-background overflow-y-auto p-5 font-mono select-none text-xs space-y-4">
      {/* Security Tearsheet Header */}
      <div className="bg-surface border border-border rounded-lg p-5 flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div className="space-y-1">
          <div className="flex items-center space-x-2">
            <span className="text-2xl font-bold text-foreground">{sym}</span>
            <span className="text-xs px-2 py-0.5 rounded bg-surface-raised border border-border text-accent font-bold uppercase">
              {venue}
            </span>
            <span className="text-xs px-2 py-0.5 rounded bg-surface-raised border border-border text-foreground-muted uppercase">
              {q ? q.asset_class : "CRYPTO"}
            </span>
          </div>
          <p className="text-foreground-muted text-xs">
            {q?.base_asset ? `${q.base_asset} / ${q.currency}` : "Security Instrument Tearsheet"} · Status: {q?.status || "Ready"}
          </p>
        </div>

        {/* Big Price Card */}
        {q && (
          <div className="flex items-baseline space-x-4">
            <div className="text-right">
              <span className="text-2xs text-foreground-muted block">LAST PRICE</span>
              <span className="text-2xl font-bold text-foreground num-tabular">
                ${formatPrice(q.price)}
              </span>
            </div>
            <div className="text-right">
              <span className="text-2xs text-foreground-muted block">24H CHANGE</span>
              <span
                className={`text-xl font-bold num-tabular ${
                  isPositive ? "text-positive" : "text-negative"
                }`}
              >
                {formatPercent(q.change_percent_24h)}
              </span>
            </div>
          </div>
        )}
      </div>

      {/* Main Grid: Quote Depth, Reference, and Related Actions */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        {/* Card 1: 24h Trading Statistics */}
        <div className="bg-surface border border-border rounded-lg p-4 space-y-3">
          <div className="text-2xs font-bold text-accent uppercase tracking-wider flex items-center space-x-1.5 border-b border-border pb-2">
            <Activity className="w-3.5 h-3.5" />
            <span>24h Trading Statistics</span>
          </div>

          <div className="space-y-2 text-2xs">
            <div className="flex justify-between">
              <span className="text-foreground-muted">24h High:</span>
              <span className="font-bold text-foreground num-tabular">{formatPrice(q?.high_24h)}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-foreground-muted">24h Low:</span>
              <span className="font-bold text-foreground num-tabular">{formatPrice(q?.low_24h)}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-foreground-muted">24h Base Volume:</span>
              <span className="font-bold text-foreground num-tabular">{formatVolume(q?.volume_24h)}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-foreground-muted">24h Quote Volume:</span>
              <span className="font-bold text-accent num-tabular">{formatVolume(q?.quote_volume_24h)} {q?.currency}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-foreground-muted">24h VWAP:</span>
              <span className="font-bold text-foreground num-tabular">{formatPrice(q?.vwap)}</span>
            </div>
          </div>
        </div>

        {/* Card 2: Order Book & Liquidity Depth */}
        <div className="bg-surface border border-border rounded-lg p-4 space-y-3">
          <div className="text-2xs font-bold text-terminal-cyan uppercase tracking-wider flex items-center space-x-1.5 border-b border-border pb-2">
            <Layers className="w-3.5 h-3.5" />
            <span>Best Bid / Ask Liquidity</span>
          </div>

          <div className="space-y-2 text-2xs">
            <div className="flex justify-between">
              <span className="text-foreground-muted">Best Bid:</span>
              <span className="font-bold text-positive num-tabular">{formatPrice(q?.bid)}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-foreground-muted">Best Ask:</span>
              <span className="font-bold text-negative num-tabular">{formatPrice(q?.ask)}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-foreground-muted">Spread (Units):</span>
              <span className="font-bold text-foreground num-tabular">{q?.spread ? formatPrice(q.spread) : "—"}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-foreground-muted">Spread (bps):</span>
              <span className="font-bold text-accent num-tabular">{q?.spread_bps !== null && q?.spread_bps !== undefined ? `${q.spread_bps.toFixed(1)} bps` : "—"}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-foreground-muted">Upstream Event:</span>
              <span className="text-foreground-muted">{q ? new Date(q.event_at).toLocaleTimeString() : "—"}</span>
            </div>
          </div>
        </div>

        {/* Card 3: Bloomberg Related Functions */}
        <div className="bg-surface border border-border rounded-lg p-4 space-y-3">
          <div className="text-2xs font-bold text-foreground-muted uppercase tracking-wider border-b border-border pb-2">
            Related Terminal Functions
          </div>

          <div className="space-y-1.5">
            <button
              onClick={() => setWorkspace("CHART")}
              className="w-full flex items-center justify-between px-3 py-2 bg-surface-raised hover:bg-surface-hover border border-border rounded transition-colors text-left"
            >
              <div className="flex items-center space-x-2">
                <CandlestickChart className="w-4 h-4 text-accent" />
                <span className="font-bold text-foreground">GP &lt;GO&gt;</span>
              </div>
              <span className="text-2xs text-foreground-muted">Candlestick Chart</span>
            </button>

            <button
              onClick={() => setWorkspace("NEWS")}
              className="w-full flex items-center justify-between px-3 py-2 bg-surface-raised hover:bg-surface-hover border border-border rounded transition-colors text-left"
            >
              <div className="flex items-center space-x-2">
                <Newspaper className="w-4 h-4 text-accent" />
                <span className="font-bold text-foreground">CN &lt;GO&gt;</span>
              </div>
              <span className="text-2xs text-foreground-muted">Company &amp; Pair News</span>
            </button>

            <button
              onClick={() => setWorkspace("FIN")}
              className="w-full flex items-center justify-between px-3 py-2 bg-surface-raised hover:bg-surface-hover border border-border rounded transition-colors text-left"
            >
              <div className="flex items-center space-x-2">
                <BarChart2 className="w-4 h-4 text-accent" />
                <span className="font-bold text-foreground">FA &lt;GO&gt;</span>
              </div>
              <span className="text-2xs text-foreground-muted">SEC Filings &amp; Financials</span>
            </button>

            <button
              onClick={() => setWorkspace("LAUNCHPAD")}
              className="w-full flex items-center justify-between px-3 py-2 bg-surface-raised hover:bg-surface-hover border border-border rounded transition-colors text-left"
            >
              <div className="flex items-center space-x-2">
                <Layers className="w-4 h-4 text-accent" />
                <span className="font-bold text-foreground">LP &lt;GO&gt;</span>
              </div>
              <span className="text-2xs text-foreground-muted">Launchpad Workstation</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
