import React, { useMemo, useState } from "react";
import {
  ArrowDown,
  ArrowUp,
  CandlestickChart,
  Check,
  ExternalLink,
  Eye,
  Filter,
  RefreshCw,
  Search,
  SlidersHorizontal,
} from "lucide-react";
import { useWorkspaceStore } from "../../app/store/workspaceStore";
import { formatPercent, formatPrice, formatVolume } from "../../lib/utils";
import { MarketQuoteDto } from "../../types";

export const ScreenerWorkspace: React.FC = () => {
  const { quotesMap, activeAsset, setActiveAsset, setWorkspace } =
    useWorkspaceStore();

  const [assetClass, setAssetClass] = useState<"CRYPTO" | "EQUITY">("CRYPTO");
  const [minVolume, setMinVolume] = useState<number>(0);
  const [minChange, setMinChange] = useState<number>(-100);
  const [maxChange, setMaxChange] = useState<number>(100);
  const [quoteCurrency, setQuoteCurrency] = useState("ALL");
  const [search, setSearch] = useState("");
  const [selectedKey, setSelectedKey] = useState<string>(activeAsset);

  const quotesList = useMemo(() => Object.values(quotesMap), [quotesMap]);

  const filteredResults = useMemo(() => {
    return quotesList.filter((q) => {
      // Currency filter
      if (quoteCurrency !== "ALL" && !q.currency.toUpperCase().includes(quoteCurrency)) {
        return false;
      }

      // Volume filter
      const vol = parseFloat(q.quote_volume_24h || "0") || 0;
      if (vol < minVolume) return false;

      // Change % range
      const chg = q.change_percent_24h ?? 0;
      if (chg < minChange || chg > maxChange) return false;

      // Search text
      if (search.trim()) {
        const query = search.trim().toUpperCase();
        if (
          !q.symbol.toUpperCase().includes(query) &&
          !(q.base_asset?.toUpperCase().includes(query) ?? false)
        ) {
          return false;
        }
      }

      return true;
    });
  }, [quotesList, quoteCurrency, minVolume, minChange, maxChange, search]);

  const selectedQuote =
    quotesMap[selectedKey] || filteredResults[0] || null;

  return (
    <div className="flex-1 flex flex-col h-full bg-background overflow-hidden text-xs font-mono select-none">
      {/* Top Screener Bar */}
      <div className="h-10 px-3 bg-surface border-b border-border flex items-center justify-between">
        <div className="flex items-center space-x-2">
          <div className="flex items-center space-x-1.5 font-bold text-accent">
            <SlidersHorizontal className="w-4 h-4" />
            <span className="text-xs uppercase">SCREENER (SCAN / EQS)</span>
          </div>

          <div className="h-4 w-px bg-border mx-1" />

          {/* Active Filter Chips */}
          <div className="flex items-center space-x-1">
            {minVolume > 0 && (
              <span className="px-2 py-0.5 rounded bg-surface-raised border border-border text-2xs text-foreground">
                Vol &gt; {formatVolume(minVolume)}
              </span>
            )}
            {(minChange > -100 || maxChange < 100) && (
              <span className="px-2 py-0.5 rounded bg-surface-raised border border-border text-2xs text-foreground">
                Chg: {minChange}% to {maxChange}%
              </span>
            )}
            {quoteCurrency !== "ALL" && (
              <span className="px-2 py-0.5 rounded bg-surface-raised border border-border text-2xs text-accent">
                Quote: {quoteCurrency}
              </span>
            )}
          </div>
        </div>

        <div className="text-2xs text-foreground-muted">
          Matches: <span className="text-foreground font-bold">{filteredResults.length}</span> / {quotesList.length}
        </div>
      </div>

      {/* Main 3-Region Layout */}
      <div className="flex-1 flex overflow-hidden">
        {/* Left: Filter Sidebar Drawer */}
        <div className="w-64 bg-surface border-r border-border p-3 flex flex-col space-y-4 overflow-y-auto">
          <div className="text-2xs font-bold text-foreground-muted uppercase tracking-wider flex items-center space-x-1">
            <Filter className="w-3.5 h-3.5 text-accent" />
            <span>Screen Criteria</span>
          </div>

          {/* Quote Currency */}
          <div className="space-y-1">
            <label className="text-2xs text-foreground-muted block">Quote Asset</label>
            <div className="grid grid-cols-3 gap-1">
              {["ALL", "USDT", "USDC", "BTC", "FDUSD", "EUR"].map((qc) => (
                <button
                  key={qc}
                  onClick={() => setQuoteCurrency(qc)}
                  className={`px-1.5 py-1 rounded text-2xs font-bold border transition-colors ${
                    quoteCurrency === qc
                      ? "bg-accent text-background border-accent"
                      : "bg-surface-raised border-border text-foreground-muted hover:text-foreground"
                  }`}
                >
                  {qc}
                </button>
              ))}
            </div>
          </div>

          {/* Min 24h Volume */}
          <div className="space-y-1">
            <div className="flex justify-between text-2xs">
              <span className="text-foreground-muted">Min 24h Volume</span>
              <span className="text-accent font-bold">{formatVolume(minVolume)}</span>
            </div>
            <input
              type="range"
              min={0}
              max={100_000_000}
              step={1_000_000}
              value={minVolume}
              onChange={(e) => setMinVolume(Number(e.target.value))}
              className="w-full accent-accent cursor-pointer"
            />
            <div className="flex justify-between text-2xs text-foreground-muted">
              <span>$0</span>
              <span>$50M</span>
              <span>$100M+</span>
            </div>
          </div>

          {/* 24h Change % Range */}
          <div className="space-y-1">
            <div className="flex justify-between text-2xs">
              <span className="text-foreground-muted">24h Change Min %</span>
              <span className={`font-bold ${minChange >= 0 ? "text-positive" : "text-negative"}`}>
                {minChange}%
              </span>
            </div>
            <input
              type="range"
              min={-50}
              max={50}
              step={5}
              value={minChange}
              onChange={(e) => setMinChange(Number(e.target.value))}
              className="w-full accent-accent cursor-pointer"
            />
          </div>

          {/* Quick Reset */}
          <button
            onClick={() => {
              setMinVolume(0);
              setMinChange(-100);
              setMaxChange(100);
              setQuoteCurrency("ALL");
              setSearch("");
            }}
            className="w-full py-1.5 rounded bg-surface-raised hover:bg-surface-hover border border-border text-2xs text-foreground-muted hover:text-foreground transition-colors font-bold"
          >
            Reset Filters
          </button>
        </div>

        {/* Center: Virtualized Results Table */}
        <div className="flex-1 overflow-auto">
          <table className="w-full border-collapse table-dense">
            <thead className="sticky top-0 bg-surface-raised border-b border-border text-foreground-muted text-2xs z-10">
              <tr>
                <th className="py-1.5 px-3 text-left">SYMBOL</th>
                <th className="py-1.5 px-3 text-right">LAST</th>
                <th className="py-1.5 px-3 text-right">24H CHG</th>
                <th className="py-1.5 px-3 text-right">24H VOL</th>
                <th className="py-1.5 px-3 text-right">24H HIGH</th>
                <th className="py-1.5 px-3 text-right">24H LOW</th>
                <th className="py-1.5 px-3 text-center">VENUE</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border-subtle">
              {filteredResults.map((q) => {
                const isSelected = selectedQuote?.key === q.key;
                const chg = q.change_percent_24h ?? 0;

                return (
                  <tr
                    key={q.key}
                    onClick={() => setSelectedKey(q.key)}
                    onDoubleClick={() => {
                      setActiveAsset(q.key, "A");
                      setWorkspace("CHART");
                    }}
                    className={`cursor-pointer transition-colors ${
                      isSelected
                        ? "bg-accent/15 border-l-2 border-accent text-foreground font-bold"
                        : "hover:bg-surface-raised text-foreground"
                    }`}
                  >
                    <td className="py-1.5 px-3 whitespace-nowrap">
                      <span>{q.symbol}</span>
                      {q.base_asset && (
                        <span className="text-2xs text-foreground-muted ml-1.5">
                          {q.base_asset}
                        </span>
                      )}
                    </td>
                    <td className="py-1.5 px-3 text-right num-tabular font-bold">
                      {formatPrice(q.price)}
                    </td>
                    <td
                      className={`py-1.5 px-3 text-right num-tabular font-bold ${
                        chg > 0
                          ? "text-positive"
                          : chg < 0
                          ? "text-negative"
                          : "text-foreground-muted"
                      }`}
                    >
                      {formatPercent(q.change_percent_24h)}
                    </td>
                    <td className="py-1.5 px-3 text-right num-tabular">
                      {formatVolume(q.quote_volume_24h)} {q.currency}
                    </td>
                    <td className="py-1.5 px-3 text-right num-tabular text-foreground-muted">
                      {formatPrice(q.high_24h)}
                    </td>
                    <td className="py-1.5 px-3 text-right num-tabular text-foreground-muted">
                      {formatPrice(q.low_24h)}
                    </td>
                    <td className="py-1.5 px-3 text-center text-2xs text-foreground-muted uppercase">
                      {q.venue}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>

        {/* Right: Security Detail Inspector */}
        {selectedQuote && (
          <div className="w-80 bg-surface border-l border-border p-3 flex flex-col space-y-3 overflow-y-auto hidden lg:flex">
            <div className="text-2xs font-bold text-foreground-muted uppercase tracking-wider flex items-center justify-between">
              <span>Security Inspector</span>
              <span className="text-accent uppercase">{selectedQuote.venue}</span>
            </div>

            <div className="bg-surface-raised border border-border rounded p-3 space-y-2">
              <div className="flex items-baseline justify-between">
                <span className="text-base font-bold text-foreground">
                  {selectedQuote.symbol}
                </span>
                <span className="text-xs text-foreground-muted font-bold">
                  {selectedQuote.base_asset}/{selectedQuote.currency}
                </span>
              </div>

              <div className="flex items-baseline justify-between border-b border-border-subtle pb-2">
                <span className="text-lg font-bold num-tabular text-foreground">
                  {formatPrice(selectedQuote.price)}
                </span>
                <span
                  className={`text-xs font-bold num-tabular ${
                    (selectedQuote.change_percent_24h ?? 0) >= 0
                      ? "text-positive"
                      : "text-negative"
                  }`}
                >
                  {formatPercent(selectedQuote.change_percent_24h)}
                </span>
              </div>

              <div className="grid grid-cols-2 gap-2 text-2xs pt-1">
                <div>
                  <span className="text-foreground-muted block">24h High:</span>
                  <span className="num-tabular font-bold">{formatPrice(selectedQuote.high_24h)}</span>
                </div>
                <div>
                  <span className="text-foreground-muted block">24h Low:</span>
                  <span className="num-tabular font-bold">{formatPrice(selectedQuote.low_24h)}</span>
                </div>
                <div>
                  <span className="text-foreground-muted block">24h Volume:</span>
                  <span className="num-tabular font-bold">{formatVolume(selectedQuote.volume_24h)}</span>
                </div>
                <div>
                  <span className="text-foreground-muted block">Quote Volume:</span>
                  <span className="num-tabular font-bold">{formatVolume(selectedQuote.quote_volume_24h)}</span>
                </div>
              </div>
            </div>

            <div className="space-y-1.5">
              <button
                onClick={() => {
                  setActiveAsset(selectedQuote.key, "A");
                  setWorkspace("CHART");
                }}
                className="w-full flex items-center justify-center space-x-1.5 px-3 py-2 bg-accent hover:bg-accent-hover text-background font-bold rounded transition-colors text-xs"
              >
                <CandlestickChart className="w-4 h-4" />
                <span>Open in Chart (GP)</span>
              </button>

              <button
                onClick={() => {
                  setActiveAsset(selectedQuote.key, "A");
                  setWorkspace("LAUNCHPAD");
                }}
                className="w-full flex items-center justify-center space-x-1.5 px-3 py-1.5 bg-surface-raised hover:bg-surface-hover border border-border text-foreground font-bold rounded transition-colors text-xs"
              >
                <Eye className="w-3.5 h-3.5 text-accent" />
                <span>Launchpad Quad View</span>
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
