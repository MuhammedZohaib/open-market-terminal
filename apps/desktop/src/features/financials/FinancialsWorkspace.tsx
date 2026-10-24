import React, { useEffect, useState } from "react";
import {
  ArrowDown,
  ArrowUp,
  BarChart2,
  Calendar,
  ExternalLink,
  FileText,
  Info,
  RefreshCw,
  Search,
  Table,
  TrendingUp,
} from "lucide-react";
import {
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { useWorkspaceStore } from "../../app/store/workspaceStore";
import { getCompanyFacts } from "../../lib/tauri/ipc";
import { CompanyFactsDto, StatementMatrixDto } from "../../types";

type StatementTab =
  | "INCOME"
  | "BALANCE"
  | "CASH"
  | "RATIOS"
  | "FILINGS";

export const FinancialsWorkspace: React.FC = () => {
  const { activeAsset } = useWorkspaceStore();
  const [ticker, setTicker] = useState("AAPL");
  const [facts, setFacts] = useState<CompanyFactsDto | null>(null);
  const [loading, setLoading] = useState(false);
  const [activeTab, setActiveTab] = useState<StatementTab>("INCOME");
  const [isQuarterly, setIsQuarterly] = useState(false);
  const [selectedMetric, setSelectedMetric] = useState<string>("Revenue");

  // Synchronize ticker from activeAsset if it's an equity
  useEffect(() => {
    const sym = activeAsset.includes(":") ? activeAsset.split(":")[1] : activeAsset;
    // If it looks like a stock symbol rather than crypto pair (doesn't end with USDT/USDC)
    if (!sym.endsWith("USDT") && !sym.endsWith("USDC") && sym.length <= 5) {
      setTicker(sym);
    }
  }, [activeAsset]);

  // Fetch facts
  useEffect(() => {
    let isMounted = true;
    setLoading(true);

    getCompanyFacts(ticker)
      .then((data) => {
        if (isMounted) {
          setFacts(data);
          setLoading(false);
        }
      })
      .catch((err) => {
        console.error("Failed to load SEC company facts:", err);
        if (isMounted) setLoading(false);
      });

    return () => {
      isMounted = false;
    };
  }, [ticker]);

  const activeMatrix: StatementMatrixDto | null = facts
    ? activeTab === "INCOME"
      ? isQuarterly
        ? facts.income_statement_quarterly
        : facts.income_statement_annual
      : activeTab === "BALANCE"
      ? isQuarterly
        ? facts.balance_sheet_quarterly
        : facts.balance_sheet_annual
      : activeTab === "CASH"
      ? isQuarterly
        ? facts.cash_flow_quarterly
        : facts.cash_flow_annual
      : isQuarterly
      ? facts.ratios_quarterly
      : facts.ratios_annual
    : null;

  // Format data for metric trend chart
  const trendData = React.useMemo(() => {
    if (!activeMatrix) return [];
    const row = activeMatrix.rows.find((r) => r.concept === selectedMetric);
    if (!row) return [];

    return activeMatrix.periods
      .map((period, idx) => {
        const rawVal = row.values[idx];
        const num = rawVal ? parseFloat(rawVal) : 0;
        return {
          period,
          value: num,
          display: rawVal || "—",
        };
      })
      .reverse(); // Chronological for chart
  }, [activeMatrix, selectedMetric]);

  const formatCellValue = (val: string | null) => {
    if (!val) return "—";
    const num = parseFloat(val);
    if (isNaN(num)) return val;
    if (Math.abs(num) >= 1_000_000_000) {
      return `$${(num / 1_000_000_000).toFixed(2)}B`;
    } else if (Math.abs(num) >= 1_000_000) {
      return `$${(num / 1_000_000).toFixed(2)}M`;
    } else if (Math.abs(num) >= 1_000) {
      return `$${(num / 1_000).toFixed(2)}K`;
    }
    return `$${num.toFixed(2)}`;
  };

  return (
    <div className="flex-1 flex flex-col h-full bg-background overflow-hidden text-xs font-mono select-none">
      {/* Top Header & Ticker Search */}
      <div className="h-10 px-3 bg-surface border-b border-border flex items-center justify-between">
        <div className="flex items-center space-x-3">
          <div className="flex items-center space-x-1.5">
            <span className="font-bold text-foreground text-sm">{ticker}</span>
            <span className="text-2xs text-foreground-muted">
              {facts ? facts.name : "SEC EDGAR Data"}
            </span>
            {facts && (
              <span className="text-2xs px-1.5 py-0.5 rounded bg-surface-raised border border-border text-foreground-muted">
                CIK: {facts.cik}
              </span>
            )}
          </div>

          <div className="h-4 w-px bg-border" />

          {/* Statement Tabs */}
          <div className="flex items-center bg-surface-raised border border-border rounded p-0.5 space-x-0.5">
            {(
              [
                { id: "INCOME", label: "Income" },
                { id: "BALANCE", label: "Balance Sheet" },
                { id: "CASH", label: "Cash Flow" },
                { id: "RATIOS", label: "Ratios" },
                { id: "FILINGS", label: "Filings" },
              ] as const
            ).map((t) => (
              <button
                key={t.id}
                onClick={() => setActiveTab(t.id)}
                className={`px-2 py-0.5 rounded text-2xs font-bold transition-colors ${
                  activeTab === t.id
                    ? "bg-accent text-background"
                    : "text-foreground-muted hover:text-foreground"
                }`}
              >
                {t.label}
              </button>
            ))}
          </div>

          {activeTab !== "FILINGS" && (
            <div className="flex items-center bg-surface-raised border border-border rounded p-0.5 space-x-0.5">
              <button
                onClick={() => setIsQuarterly(false)}
                className={`px-2 py-0.5 rounded text-2xs font-bold transition-colors ${
                  !isQuarterly
                    ? "bg-surface-hover text-foreground font-bold"
                    : "text-foreground-muted hover:text-foreground"
                }`}
              >
                Annual (10-K)
              </button>
              <button
                onClick={() => setIsQuarterly(true)}
                className={`px-2 py-0.5 rounded text-2xs font-bold transition-colors ${
                  isQuarterly
                    ? "bg-surface-hover text-foreground font-bold"
                    : "text-foreground-muted hover:text-foreground"
                }`}
              >
                Quarterly (10-Q)
              </button>
            </div>
          )}
        </div>

        {/* Ticker switcher */}
        <div className="flex items-center space-x-2">
          <span className="text-2xs text-foreground-muted">Ticker:</span>
          <input
            type="text"
            value={ticker}
            onChange={(e) => setTicker(e.target.value.toUpperCase())}
            className="w-20 bg-surface-raised border border-border rounded px-2 py-0.5 text-2xs text-foreground font-bold outline-none focus:border-accent uppercase"
          />
        </div>
      </div>

      {/* Main Content Area */}
      <div className="flex-1 flex flex-col md:flex-row overflow-hidden">
        {/* Left / Center: Financial Statement Table */}
        <div className="flex-1 flex flex-col overflow-hidden border-r border-border">
          {loading ? (
            <div className="flex-1 flex flex-col items-center justify-center space-y-2">
              <RefreshCw className="w-5 h-5 text-accent animate-spin" />
              <span className="text-2xs text-foreground-muted">
                Parsing SEC EDGAR XBRL facts for {ticker}...
              </span>
            </div>
          ) : activeTab === "FILINGS" ? (
            <div className="flex-1 overflow-auto p-2">
              <div className="text-2xs font-bold text-foreground-muted uppercase px-2 py-1">
                Recent Regulatory Filings ({facts?.recent_filings.length ?? 0})
              </div>
              <table className="w-full border-collapse table-dense">
                <thead className="sticky top-0 bg-surface-raised border-b border-border text-foreground-muted text-2xs">
                  <tr>
                    <th className="py-1.5 px-3">FORM</th>
                    <th className="py-1.5 px-3">FILED</th>
                    <th className="py-1.5 px-3">PERIOD</th>
                    <th className="py-1.5 px-3">DESCRIPTION</th>
                    <th className="py-1.5 px-3 text-right">SEC LINK</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-border-subtle">
                  {facts?.recent_filings.map((f) => (
                    <tr key={f.accession} className="hover:bg-surface-raised transition-colors">
                      <td className="py-1.5 px-3 font-bold text-accent">{f.form}</td>
                      <td className="py-1.5 px-3 text-foreground">{f.filed}</td>
                      <td className="py-1.5 px-3 text-foreground-muted">{f.report_date || "—"}</td>
                      <td className="py-1.5 px-3 text-foreground-muted truncate max-w-xs">{f.description || f.document}</td>
                      <td className="py-1.5 px-3 text-right">
                        <a
                          href={f.url}
                          target="_blank"
                          rel="noreferrer"
                          className="inline-flex items-center space-x-1 text-accent hover:underline text-2xs"
                        >
                          <span>EDGAR</span>
                          <ExternalLink className="w-3 h-3" />
                        </a>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : activeMatrix ? (
            <div className="flex-1 overflow-auto">
              <table className="w-full border-collapse table-dense">
                <thead className="sticky top-0 bg-surface-raised border-b border-border text-foreground-muted text-2xs z-10">
                  <tr>
                    <th className="py-1.5 px-3">CONCEPT / METRIC</th>
                    {activeMatrix.periods.map((p) => (
                      <th key={p} className="py-1.5 px-3 text-right num-tabular">
                        {p}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody className="divide-y divide-border-subtle">
                  {activeMatrix.rows.map((row) => {
                    const isSelected = selectedMetric === row.concept;
                    return (
                      <tr
                        key={row.concept}
                        onClick={() => setSelectedMetric(row.concept)}
                        className={`cursor-pointer transition-colors ${
                          isSelected
                            ? "bg-accent/15 border-l-2 border-accent text-foreground font-bold"
                            : "hover:bg-surface-raised text-foreground"
                        }`}
                      >
                        <td className="py-1.5 px-3 whitespace-nowrap font-medium flex items-center justify-between">
                          <span>{row.label}</span>
                          {isSelected && (
                            <TrendingUp className="w-3.5 h-3.5 text-accent ml-2" />
                          )}
                        </td>
                        {row.values.map((val, idx) => (
                          <td
                            key={idx}
                            className="py-1.5 px-3 text-right num-tabular whitespace-nowrap"
                          >
                            {formatCellValue(val)}
                          </td>
                        ))}
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          ) : (
            <div className="flex-1 flex flex-col items-center justify-center p-6 text-center text-foreground-muted">
              <FileText className="w-8 h-8 text-foreground-muted mb-2 opacity-50" />
              <p>No SEC financial filings available for "{ticker}".</p>
              <p className="text-2xs mt-1">
                Enter a US equity ticker (e.g. AAPL, NVDA, MSFT, GOOGL) or ensure network is online.
              </p>
            </div>
          )}

          {/* Lower Chart: Selected Metric Trend Visualizer */}
          {activeTab !== "FILINGS" && trendData.length > 0 && (
            <div className="h-44 bg-surface border-t border-border flex flex-col p-2.5">
              <div className="flex items-center justify-between mb-1 px-1">
                <span className="text-2xs font-bold text-accent uppercase tracking-wider flex items-center space-x-1">
                  <BarChart2 className="w-3.5 h-3.5" />
                  <span>Historical Trend: {selectedMetric}</span>
                </span>
                <span className="text-2xs text-foreground-muted">
                  {isQuarterly ? "Direct Reported Quarters (10-Q)" : "Fiscal Years (10-K)"}
                </span>
              </div>
              <div className="flex-1 w-full h-full">
                <ResponsiveContainer width="100%" height="100%">
                  <BarChart data={trendData} margin={{ top: 5, right: 10, left: 10, bottom: 0 }}>
                    <CartesianGrid strokeDasharray="3 3" stroke="#1c242d" />
                    <XAxis dataKey="period" stroke="#8b97a6" fontSize={10} tickLine={false} />
                    <YAxis
                      stroke="#8b97a6"
                      fontSize={10}
                      tickLine={false}
                      tickFormatter={(v) => (v >= 1e9 ? `${(v / 1e9).toFixed(0)}B` : `${(v / 1e6).toFixed(0)}M`)}
                    />
                    <Tooltip
                      contentStyle={{ backgroundColor: "#171d25", borderColor: "#29313b", fontSize: "11px", color: "#e9edf2" }}
                      formatter={(val: any) => [formatCellValue(String(val)), selectedMetric]}
                    />
                    <Bar dataKey="value" fill="#e9a441" radius={[2, 2, 0, 0]} />
                  </BarChart>
                </ResponsiveContainer>
              </div>
            </div>
          )}
        </div>

        {/* Right Pane: Source Provenance Inspector */}
        <div className="w-72 bg-surface p-3 flex flex-col space-y-3 overflow-y-auto hidden lg:flex">
          <div className="text-2xs font-bold text-foreground-muted uppercase tracking-wider flex items-center space-x-1">
            <Info className="w-3.5 h-3.5 text-accent" />
            <span>SEC Provenance & Fact Inspector</span>
          </div>

          <div className="bg-surface-raised border border-border rounded p-2.5 space-y-2 text-2xs">
            <div>
              <span className="text-foreground-muted block">Active Metric:</span>
              <span className="text-accent font-bold text-xs">{selectedMetric}</span>
            </div>
            <div>
              <span className="text-foreground-muted block">Reporting Authority:</span>
              <span className="text-foreground">U.S. Securities and Exchange Commission</span>
            </div>
            <div>
              <span className="text-foreground-muted block">GAAP Taxonomy:</span>
              <span className="text-foreground">US-GAAP 2024 / XBRL Normalized</span>
            </div>
            <div>
              <span className="text-foreground-muted block">Accounting Principle:</span>
              <span className="text-foreground">Direct reported periods only. Never derives Q4 or treats YTD as quarter.</span>
            </div>
          </div>

          <div className="bg-surface-raised border border-border rounded p-2.5 space-y-1.5 text-2xs">
            <span className="text-foreground-muted font-bold block uppercase">Company Overview</span>
            <div className="flex justify-between">
              <span className="text-foreground-muted">Ticker:</span>
              <span className="text-foreground font-bold">{facts?.ticker || ticker}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-foreground-muted">CIK:</span>
              <span className="text-foreground">{facts?.cik || "—"}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-foreground-muted">Normalized Facts:</span>
              <span className="text-accent font-bold">{facts?.facts_count || 0}</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
