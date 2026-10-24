import React, { useMemo, useState } from "react";
import {
  ArrowDown,
  ArrowUp,
  ArrowUpDown,
  CandlestickChart,
  ExternalLink,
  Filter,
  Plus,
  RefreshCw,
  Search,
  Star,
  Table as TableIcon,
} from "lucide-react";
import { useWorkspaceStore } from "../../app/store/workspaceStore";
import { formatPercent, formatPrice, formatVolume } from "../../lib/utils";
import { MarketQuoteDto } from "../../types";

const QUOTE_CURRENCIES = ["USDT", "USDC", "BTC", "FDUSD", "EUR", "ALL"];

export const MarketWorkspace: React.FC = () => {
  const {
    quotesMap,
    quoteFilter,
    setQuoteFilter,
    activeAsset,
    setActiveAsset,
    setWorkspace,
    totalInstruments,
    activeInstruments,
  } = useWorkspaceStore();

  const [search, setSearch] = useState("");
  const [sortField, setSortField] = useState<keyof MarketQuoteDto>("quote_volume_24h");
  const [sortAsc, setSortAsc] = useState(false);
  const [viewPreset, setViewPreset] = useState<"ALL" | "GAINERS" | "LOSERS" | "ACTIVE">("ALL");

  const quotesList = useMemo(() => Object.values(quotesMap), [quotesMap]);

  const filteredQuotes = useMemo(() => {
    return quotesList.filter((q) => {
      // Quote currency filter
      if (quoteFilter !== "ALL") {
        if (!q.currency.toUpperCase().includes(quoteFilter)) {
          return false;
        }
      }

      // Search filter
      if (search.trim()) {
        const query = search.trim().toUpperCase();
        const matchesSymbol = q.symbol.toUpperCase().includes(query);
        const matchesBase = q.base_asset?.toUpperCase().includes(query) ?? false;
        if (!matchesSymbol && !matchesBase) return false;
      }

      // Presets
      if (viewPreset === "GAINERS") {
        if ((q.change_percent_24h ?? 0) <= 0) return false;
      } else if (viewPreset === "LOSERS") {
        if ((q.change_percent_24h ?? 0) >= 0) return false;
      }

      return true;
    });
  }, [quotesList, quoteFilter, search, viewPreset]);

  const sortedQuotes = useMemo(() => {
    return [...filteredQuotes].sort((a, b) => {
      let valA: any = a[sortField];
      let valB: any = b[sortField];

      // Handle numeric sorting
      if (
        sortField === "price" ||
        sortField === "change_percent_24h" ||
        sortField === "quote_volume_24h" ||
        sortField === "volume_24h" ||
        sortField === "high_24h" ||
        sortField === "low_24h" ||
        sortField === "spread_bps"
      ) {
        valA = parseFloat(valA ?? "0") || 0;
        valB = parseFloat(valB ?? "0") || 0;
      }

      if (valA < valB) return sortAsc ? -1 : 1;
      if (valA > valB) return sortAsc ? 1 : -1;
      return 0;
    });
  }, [filteredQuotes, sortField, sortAsc]);

  const handleSort = (field: keyof MarketQuoteDto) => {
    if (sortField === field) {
      setSortAsc(!sortAsc);
    } else {
      setSortField(field);
      setSortAsc(false);
    }
  };

  const handleRowClick = (q: MarketQuoteDto) => {
    setActiveAsset(q.key, "A");
  };

  const handleRowDoubleClick = (q: MarketQuoteDto) => {
    setActiveAsset(q.key, "A");
    setWorkspace("CHART");
  };

  return (
    <div className="flex-1 flex flex-col h-full bg-background overflow-hidden text-xs font-mono select-none">
      {/* Top Filter and Controls Bar */}
      <div className="h-10 px-3 bg-surface border-b border-border flex items-center justify-between space-x-3">
        {/* Left: Quote currency tabs & Presets */}
        <div className="flex items-center space-x-2">
          <div className="flex items-center bg-surface-raised border border-border rounded p-0.5 space-x-0.5">
            {QUOTE_CURRENCIES.map((qc) => (
              <button
                key={qc}
                onClick={() => setQuoteFilter(qc)}
                className={`px-2 py-0.5 rounded text-2xs font-bold transition-colors ${
                  quoteFilter === qc
                    ? "bg-accent text-background"
                    : "text-foreground-muted hover:text-foreground"
                }`}
              >
                {qc}
              </button>
            ))}
          </div>

          <div className="h-4 w-px bg-border" />

          {/* Quick Filter Presets */}
          <div className="flex items-center space-x-1">
            {(["ALL", "ACTIVE", "GAINERS", "LOSERS"] as const).map((preset) => (
              <button
                key={preset}
                onClick={() => {
                  setViewPreset(preset);
                  if (preset === "ACTIVE") {
                    setSortField("quote_volume_24h");
                    setSortAsc(false);
                  } else if (preset === "GAINERS") {
                    setSortField("change_percent_24h");
                    setSortAsc(false);
                  } else if (preset === "LOSERS") {
                    setSortField("change_percent_24h");
                    setSortAsc(true);
                  }
                }}
                className={`px-2 py-0.5 rounded text-2xs font-bold border ${
                  viewPreset === preset
                    ? "bg-surface-raised border-accent text-accent"
                    : "bg-surface border-border text-foreground-muted hover:text-foreground"
                }`}
              >
                {preset}
              </button>
            ))}
          </div>
        </div>

        {/* Right: Search Input & Row Count */}
        <div className="flex items-center space-x-3">
          <div className="relative w-48">
            <Search className="w-3.5 h-3.5 text-foreground-muted absolute left-2 top-2" />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Filter pair..."
              className="w-full bg-surface-raised border border-border rounded pl-7 pr-2 py-0.5 text-2xs text-foreground placeholder:text-foreground-muted outline-none focus:border-accent"
            />
          </div>

          <span className="text-2xs text-foreground-muted whitespace-nowrap">
            Showing <span className="text-foreground font-bold">{sortedQuotes.length}</span> / {totalInstruments || quotesList.length} pairs
          </span>
        </div>
      </div>

      {/* Main Table View */}
      <div className="flex-1 overflow-auto">
        <table className="w-full border-collapse text-left table-dense">
          <thead className="sticky top-0 bg-surface-raised border-b border-border z-10 text-foreground-muted text-2xs">
            <tr>
              <th className="py-1.5 px-3 cursor-pointer hover:text-foreground" onClick={() => handleSort("symbol")}>
                <div className="flex items-center space-x-1">
                  <span>SYMBOL</span>
                  {sortField === "symbol" && (sortAsc ? <ArrowUp className="w-3 h-3" /> : <ArrowDown className="w-3 h-3" />)}
                </div>
              </th>
              <th className="py-1.5 px-3 text-right cursor-pointer hover:text-foreground" onClick={() => handleSort("price")}>
                <div className="flex items-center justify-end space-x-1">
                  <span>LAST PRICE</span>
                  {sortField === "price" && (sortAsc ? <ArrowUp className="w-3 h-3" /> : <ArrowDown className="w-3 h-3" />)}
                </div>
              </th>
              <th className="py-1.5 px-3 text-right cursor-pointer hover:text-foreground" onClick={() => handleSort("change_percent_24h")}>
                <div className="flex items-center justify-end space-x-1">
                  <span>24H CHG %</span>
                  {sortField === "change_percent_24h" && (sortAsc ? <ArrowUp className="w-3 h-3" /> : <ArrowDown className="w-3 h-3" />)}
                </div>
              </th>
              <th className="py-1.5 px-3 text-right cursor-pointer hover:text-foreground" onClick={() => handleSort("high_24h")}>
                <div className="flex items-center justify-end space-x-1">
                  <span>24H HIGH</span>
                  {sortField === "high_24h" && (sortAsc ? <ArrowUp className="w-3 h-3" /> : <ArrowDown className="w-3 h-3" />)}
                </div>
              </th>
              <th className="py-1.5 px-3 text-right cursor-pointer hover:text-foreground" onClick={() => handleSort("low_24h")}>
                <div className="flex items-center justify-end space-x-1">
                  <span>24H LOW</span>
                  {sortField === "low_24h" && (sortAsc ? <ArrowUp className="w-3 h-3" /> : <ArrowDown className="w-3 h-3" />)}
                </div>
              </th>
              <th className="py-1.5 px-3 text-right cursor-pointer hover:text-foreground" onClick={() => handleSort("quote_volume_24h")}>
                <div className="flex items-center justify-end space-x-1">
                  <span>24H VOL (QUOTE)</span>
                  {sortField === "quote_volume_24h" && (sortAsc ? <ArrowUp className="w-3 h-3" /> : <ArrowDown className="w-3 h-3" />)}
                </div>
              </th>
              <th className="py-1.5 px-3 text-right cursor-pointer hover:text-foreground" onClick={() => handleSort("bid")}>
                <div className="flex items-center justify-end space-x-1">
                  <span>BID</span>
                  {sortField === "bid" && (sortAsc ? <ArrowUp className="w-3 h-3" /> : <ArrowDown className="w-3 h-3" />)}
                </div>
              </th>
              <th className="py-1.5 px-3 text-right cursor-pointer hover:text-foreground" onClick={() => handleSort("ask")}>
                <div className="flex items-center justify-end space-x-1">
                  <span>ASK</span>
                  {sortField === "ask" && (sortAsc ? <ArrowUp className="w-3 h-3" /> : <ArrowDown className="w-3 h-3" />)}
                </div>
              </th>
              <th className="py-1.5 px-3 text-right cursor-pointer hover:text-foreground" onClick={() => handleSort("spread_bps")}>
                <div className="flex items-center justify-end space-x-1">
                  <span>SPREAD (BPS)</span>
                  {sortField === "spread_bps" && (sortAsc ? <ArrowUp className="w-3 h-3" /> : <ArrowDown className="w-3 h-3" />)}
                </div>
              </th>
              <th className="py-1.5 px-3 text-center">VENUE</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border-subtle font-mono text-xs">
            {sortedQuotes.map((q) => {
              const isSelected = activeAsset === q.key;
              const chg = q.change_percent_24h ?? 0;
              const isPositive = chg > 0;
              const isNegative = chg < 0;

              return (
                <tr
                  key={q.key}
                  onClick={() => handleRowClick(q)}
                  onDoubleClick={() => handleRowDoubleClick(q)}
                  className={`cursor-pointer transition-colors ${
                    isSelected
                      ? "bg-accent/15 border-l-2 border-accent text-foreground"
                      : "hover:bg-surface-raised text-foreground"
                  }`}
                >
                  <td className="py-1 px-3 whitespace-nowrap">
                    <span className="font-bold text-foreground">{q.symbol}</span>
                    {q.base_asset && (
                      <span className="text-2xs text-foreground-muted ml-1.5">
                        {q.base_asset}
                      </span>
                    )}
                  </td>
                  <td className="py-1 px-3 text-right num-tabular font-bold">
                    {formatPrice(q.price)}
                  </td>
                  <td
                    className={`py-1 px-3 text-right num-tabular font-bold ${
                      isPositive
                        ? "text-positive"
                        : isNegative
                        ? "text-negative"
                        : "text-foreground-muted"
                    }`}
                  >
                    {formatPercent(q.change_percent_24h)}
                  </td>
                  <td className="py-1 px-3 text-right num-tabular text-foreground-muted">
                    {formatPrice(q.high_24h)}
                  </td>
                  <td className="py-1 px-3 text-right num-tabular text-foreground-muted">
                    {formatPrice(q.low_24h)}
                  </td>
                  <td className="py-1 px-3 text-right num-tabular text-foreground">
                    {formatVolume(q.quote_volume_24h)} <span className="text-2xs text-foreground-muted">{q.currency}</span>
                  </td>
                  <td className="py-1 px-3 text-right num-tabular text-foreground-muted">
                    {formatPrice(q.bid)}
                  </td>
                  <td className="py-1 px-3 text-right num-tabular text-foreground-muted">
                    {formatPrice(q.ask)}
                  </td>
                  <td className="py-1 px-3 text-right num-tabular text-foreground-muted">
                    {q.spread_bps !== null ? `${q.spread_bps.toFixed(1)}` : "—"}
                  </td>
                  <td className="py-1 px-3 text-center text-2xs text-foreground-muted uppercase">
                    {q.venue}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
};
