import React, { useEffect, useState } from "react";
import {
  ArrowDown,
  ArrowUp,
  Briefcase,
  DollarSign,
  Download,
  PieChart as PieIcon,
  Plus,
  RefreshCw,
  Trash2,
} from "lucide-react";
import {
  Cell,
  Pie,
  PieChart,
  ResponsiveContainer,
  Tooltip,
} from "recharts";
import { useWorkspaceStore } from "../../app/store/workspaceStore";
import {
  addTransaction,
  deleteTransaction,
  getPortfolio,
} from "../../lib/tauri/ipc";
import { formatPercent, formatPrice } from "../../lib/utils";
import { PortfolioSummaryDto, TransactionDto } from "../../types";

const ALLOCATION_COLORS = [
  "#e9a441",
  "#38b887",
  "#38c0e0",
  "#68a6ee",
  "#ed6976",
  "#f4b85c",
  "#a78bfa",
];

export const PortfolioWorkspace: React.FC = () => {
  const { setActiveAsset, setWorkspace } = useWorkspaceStore();
  const [portfolio, setPortfolio] = useState<PortfolioSummaryDto | null>(null);
  const [loading, setLoading] = useState(false);
  const [addModalOpen, setAddModalOpen] = useState(false);

  // Form states for new transaction
  const [formAsset, setFormAsset] = useState("BINANCE:BTCUSDT");
  const [formSide, setFormSide] = useState<"BUY" | "SELL">("BUY");
  const [formQty, setFormQty] = useState("");
  const [formPrice, setFormPrice] = useState("");
  const [formFee, setFormFee] = useState("0");
  const [formCurrency, setFormCurrency] = useState("USDT");

  const refreshPortfolio = () => {
    setLoading(true);
    getPortfolio("default")
      .then((data) => {
        setPortfolio(data);
        setLoading(false);
      })
      .catch((err) => {
        console.error("Failed to load portfolio:", err);
        setLoading(false);
      });
  };

  useEffect(() => {
    refreshPortfolio();
  }, []);

  const handleAddSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!formAsset || !formQty || !formPrice) return;

    const tx: TransactionDto = {
      id: `tx-${Date.now()}`,
      account: "default",
      asset_key: formAsset.trim().toUpperCase(),
      symbol: formAsset.includes(":") ? formAsset.split(":")[1] : formAsset,
      side: formSide,
      quantity: formQty,
      price: formPrice,
      fee: formFee || "0",
      currency: formCurrency,
      at: new Date().toISOString(),
      total_amount: (
        parseFloat(formQty) * parseFloat(formPrice) +
        parseFloat(formFee || "0")
      ).toString(),
    };

    try {
      await addTransaction(tx);
      setAddModalOpen(false);
      setFormQty("");
      setFormPrice("");
      refreshPortfolio();
    } catch (err) {
      alert(`Transaction failed: ${err}`);
    }
  };

  const handleDelete = async (id: string) => {
    if (!confirm("Are you sure you want to delete this transaction from the SQLite ledger?"))
      return;
    try {
      await deleteTransaction(id);
      refreshPortfolio();
    } catch (err) {
      alert(`Delete failed: ${err}`);
    }
  };

  // Prepare allocation data
  const allocationData = (portfolio?.holdings || [])
    .filter((h) => parseFloat(h.market_value || "0") > 0)
    .map((h) => ({
      name: h.symbol,
      value: parseFloat(h.market_value || "0"),
    }));

  const totalCostNum = parseFloat(portfolio?.total_cost_basis || "0");
  const totalMarketNum = parseFloat(portfolio?.total_market_value || "0");
  const totalPnlNum = parseFloat(portfolio?.total_unrealized_pnl || "0");
  const isPnlPositive = totalPnlNum >= 0;

  return (
    <div className="flex-1 flex flex-col h-full bg-background overflow-hidden text-xs font-mono select-none">
      {/* Top Header Metrics Bar */}
      <div className="h-16 px-4 bg-surface border-b border-border flex items-center justify-between">
        <div className="flex items-center space-x-6">
          <div className="flex items-center space-x-2">
            <Briefcase className="w-5 h-5 text-accent" />
            <div>
              <span className="text-2xs text-foreground-muted block leading-none">PORTFOLIO</span>
              <span className="text-sm font-bold text-foreground">ACCOUNT: DEFAULT</span>
            </div>
          </div>

          <div className="h-8 w-px bg-border" />

          <div>
            <span className="text-2xs text-foreground-muted block leading-none">MARKET VALUE</span>
            <span className="text-base font-bold text-foreground num-tabular">
              ${formatPrice(totalMarketNum)}
            </span>
          </div>

          <div>
            <span className="text-2xs text-foreground-muted block leading-none">COST BASIS</span>
            <span className="text-base font-bold text-foreground num-tabular">
              ${formatPrice(totalCostNum)}
            </span>
          </div>

          <div>
            <span className="text-2xs text-foreground-muted block leading-none">UNREALIZED P&amp;L</span>
            <div className="flex items-baseline space-x-1.5">
              <span
                className={`text-base font-bold num-tabular ${
                  isPnlPositive ? "text-positive" : "text-negative"
                }`}
              >
                {isPnlPositive ? "+" : ""}${formatPrice(totalPnlNum)}
              </span>
              <span
                className={`text-xs font-bold num-tabular ${
                  isPnlPositive ? "text-positive" : "text-negative"
                }`}
              >
                ({formatPercent(portfolio?.total_unrealized_pnl_percent)})
              </span>
            </div>
          </div>
        </div>

        {/* Action Buttons */}
        <div className="flex items-center space-x-2">
          <button
            onClick={() => setAddModalOpen(true)}
            className="flex items-center space-x-1 px-3 py-1.5 bg-accent hover:bg-accent-hover text-background font-bold rounded transition-colors text-xs"
          >
            <Plus className="w-3.5 h-3.5" />
            <span>Add Transaction</span>
          </button>

          <button
            onClick={refreshPortfolio}
            className="p-1.5 bg-surface-raised hover:bg-surface-hover border border-border rounded text-foreground-muted hover:text-foreground transition-colors"
            title="Refresh Ledger"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${loading ? "animate-spin text-accent" : ""}`} />
          </button>
        </div>
      </div>

      {/* Main Regions: Holdings Table & Allocation Chart */}
      <div className="flex-1 flex flex-col lg:flex-row overflow-hidden">
        {/* Left: Holdings Table */}
        <div className="flex-1 flex flex-col border-r border-border overflow-hidden">
          <div className="px-3 py-1.5 bg-surface-raised border-b border-border flex justify-between items-center text-2xs font-bold text-foreground-muted">
            <span>CURRENT HOLDINGS ({portfolio?.holdings.length ?? 0})</span>
            <span>SQLite WAL Ledger</span>
          </div>

          <div className="flex-1 overflow-auto">
            <table className="w-full border-collapse table-dense">
              <thead className="sticky top-0 bg-surface-raised border-b border-border text-foreground-muted text-2xs">
                <tr>
                  <th className="py-1.5 px-3 text-left">ASSET</th>
                  <th className="py-1.5 px-3 text-right">QUANTITY</th>
                  <th className="py-1.5 px-3 text-right">AVG COST</th>
                  <th className="py-1.5 px-3 text-right">COST BASIS</th>
                  <th className="py-1.5 px-3 text-right">CURRENT PRICE</th>
                  <th className="py-1.5 px-3 text-right">MARKET VALUE</th>
                  <th className="py-1.5 px-3 text-right">UNREALIZED P&amp;L</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border-subtle">
                {(portfolio?.holdings || []).map((h) => {
                  const pnl = parseFloat(h.unrealized_pnl || "0");
                  const isPos = pnl >= 0;

                  return (
                    <tr
                      key={h.asset_key}
                      onClick={() => setActiveAsset(h.asset_key, "A")}
                      onDoubleClick={() => {
                        setActiveAsset(h.asset_key, "A");
                        setWorkspace("CHART");
                      }}
                      className="hover:bg-surface-raised cursor-pointer transition-colors"
                    >
                      <td className="py-1.5 px-3 whitespace-nowrap">
                        <span className="font-bold text-foreground">{h.symbol}</span>
                        <span className="text-2xs text-foreground-muted ml-1.5">[{h.venue}]</span>
                      </td>
                      <td className="py-1.5 px-3 text-right num-tabular font-bold">
                        {h.quantity}
                      </td>
                      <td className="py-1.5 px-3 text-right num-tabular text-foreground-muted">
                        ${formatPrice(h.average_cost)}
                      </td>
                      <td className="py-1.5 px-3 text-right num-tabular text-foreground-muted">
                        ${formatPrice(h.cost_basis)}
                      </td>
                      <td className="py-1.5 px-3 text-right num-tabular font-bold text-foreground">
                        ${formatPrice(h.current_price)}
                      </td>
                      <td className="py-1.5 px-3 text-right num-tabular font-bold text-accent">
                        ${formatPrice(h.market_value)}
                      </td>
                      <td
                        className={`py-1.5 px-3 text-right num-tabular font-bold ${
                          isPos ? "text-positive" : "text-negative"
                        }`}
                      >
                        {isPos ? "+" : ""}${formatPrice(h.unrealized_pnl)} ({formatPercent(h.unrealized_pnl_percent)})
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>

          {/* Bottom Region: Recent Transactions */}
          <div className="h-48 border-t border-border flex flex-col bg-surface">
            <div className="px-3 py-1.5 bg-surface-raised border-b border-border flex justify-between items-center text-2xs font-bold text-foreground-muted">
              <span>TRANSACTION LEDGER ({portfolio?.transactions.length ?? 0})</span>
              <span>FIFO Lot Matching</span>
            </div>
            <div className="flex-1 overflow-auto">
              <table className="w-full border-collapse table-dense">
                <thead className="sticky top-0 bg-surface-raised border-b border-border text-foreground-muted text-2xs">
                  <tr>
                    <th className="py-1 px-3 text-left">TIMESTAMP</th>
                    <th className="py-1 px-3 text-left">SIDE</th>
                    <th className="py-1 px-3 text-left">ASSET</th>
                    <th className="py-1 px-3 text-right">QUANTITY</th>
                    <th className="py-1 px-3 text-right">PRICE</th>
                    <th className="py-1 px-3 text-right">FEE</th>
                    <th className="py-1 px-3 text-right">TOTAL</th>
                    <th className="py-1 px-3 text-center">ACTION</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-border-subtle text-2xs">
                  {(portfolio?.transactions || []).map((t) => (
                    <tr key={t.id} className="hover:bg-surface-raised transition-colors">
                      <td className="py-1 px-3 text-foreground-muted">
                        {new Date(t.at).toLocaleString()}
                      </td>
                      <td className="py-1 px-3">
                        <span
                          className={`font-bold px-1.5 py-0.2 rounded ${
                            t.side === "BUY"
                              ? "bg-positive/20 text-positive"
                              : "bg-negative/20 text-negative"
                          }`}
                        >
                          {t.side}
                        </span>
                      </td>
                      <td className="py-1 px-3 font-bold text-foreground">{t.symbol}</td>
                      <td className="py-1 px-3 text-right num-tabular">{t.quantity}</td>
                      <td className="py-1 px-3 text-right num-tabular">${formatPrice(t.price)}</td>
                      <td className="py-1 px-3 text-right num-tabular text-foreground-muted">
                        ${formatPrice(t.fee)}
                      </td>
                      <td className="py-1 px-3 text-right num-tabular font-bold text-foreground">
                        ${formatPrice(t.total_amount)}
                      </td>
                      <td className="py-1 px-3 text-center">
                        <button
                          onClick={() => handleDelete(t.id)}
                          className="text-foreground-muted hover:text-negative transition-colors"
                          title="Delete transaction"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        </div>

        {/* Right: Allocation Breakdown Chart */}
        <div className="w-80 bg-surface p-4 flex flex-col space-y-4">
          <div className="text-2xs font-bold text-foreground-muted uppercase tracking-wider flex items-center space-x-1.5">
            <PieIcon className="w-3.5 h-3.5 text-accent" />
            <span>Asset Allocation</span>
          </div>

          {allocationData.length > 0 ? (
            <div className="h-56 w-full">
              <ResponsiveContainer width="100%" height="100%">
                <PieChart>
                  <Pie
                    data={allocationData}
                    cx="50%"
                    cy="50%"
                    innerRadius={50}
                    outerRadius={80}
                    paddingAngle={3}
                    dataKey="value"
                  >
                    {allocationData.map((entry, index) => (
                      <Cell
                        key={`cell-${index}`}
                        fill={ALLOCATION_COLORS[index % ALLOCATION_COLORS.length]}
                      />
                    ))}
                  </Pie>
                  <Tooltip
                    contentStyle={{
                      backgroundColor: "#171d25",
                      borderColor: "#29313b",
                      fontSize: "11px",
                      color: "#e9edf2",
                    }}
                    formatter={(val: any) => [`$${formatPrice(String(val))}`, "Market Value"]}
                  />
                </PieChart>
              </ResponsiveContainer>
            </div>
          ) : (
            <div className="flex-1 flex items-center justify-center text-foreground-muted text-2xs text-center p-4">
              No active holdings to display allocation.
            </div>
          )}

          {/* Allocation Legend */}
          <div className="space-y-1.5 overflow-y-auto max-h-48 text-2xs">
            {allocationData.map((item, idx) => {
              const pct = totalMarketNum > 0 ? (item.value / totalMarketNum) * 100 : 0;
              return (
                <div key={item.name} className="flex items-center justify-between">
                  <div className="flex items-center space-x-2">
                    <span
                      className="w-2.5 h-2.5 rounded-full"
                      style={{
                        backgroundColor:
                          ALLOCATION_COLORS[idx % ALLOCATION_COLORS.length],
                      }}
                    />
                    <span className="font-bold text-foreground">{item.name}</span>
                  </div>
                  <div className="flex items-center space-x-2 num-tabular">
                    <span className="text-foreground-muted">${formatPrice(item.value)}</span>
                    <span className="font-bold text-accent">{pct.toFixed(1)}%</span>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </div>

      {/* Add Transaction Modal */}
      {addModalOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 backdrop-blur-sm">
          <form
            onSubmit={handleAddSubmit}
            className="w-full max-w-md bg-surface border border-border shadow-2xl rounded-lg p-5 space-y-4"
          >
            <div className="flex justify-between items-center border-b border-border pb-2">
              <h3 className="font-bold text-foreground text-sm flex items-center space-x-2">
                <Plus className="w-4 h-4 text-accent" />
                <span>Record New Transaction</span>
              </h3>
              <button
                type="button"
                onClick={() => setAddModalOpen(false)}
                className="text-foreground-muted hover:text-foreground"
              >
                ✕
              </button>
            </div>

            <div className="space-y-3 text-xs">
              <div>
                <label className="text-2xs text-foreground-muted block mb-1">Asset Key (e.g. BINANCE:BTCUSDT or IEX:AAPL)</label>
                <input
                  type="text"
                  required
                  value={formAsset}
                  onChange={(e) => setFormAsset(e.target.value.toUpperCase())}
                  className="w-full bg-surface-raised border border-border rounded px-2.5 py-1.5 text-foreground font-mono outline-none focus:border-accent"
                />
              </div>

              <div className="grid grid-cols-2 gap-2">
                <div>
                  <label className="text-2xs text-foreground-muted block mb-1">Side</label>
                  <select
                    value={formSide}
                    onChange={(e) => setFormSide(e.target.value as any)}
                    className="w-full bg-surface-raised border border-border rounded px-2.5 py-1.5 text-foreground font-mono outline-none focus:border-accent"
                  >
                    <option value="BUY">BUY</option>
                    <option value="SELL">SELL</option>
                  </select>
                </div>
                <div>
                  <label className="text-2xs text-foreground-muted block mb-1">Settlement Currency</label>
                  <input
                    type="text"
                    required
                    value={formCurrency}
                    onChange={(e) => setFormCurrency(e.target.value.toUpperCase())}
                    className="w-full bg-surface-raised border border-border rounded px-2.5 py-1.5 text-foreground font-mono outline-none focus:border-accent"
                  />
                </div>
              </div>

              <div className="grid grid-cols-3 gap-2">
                <div>
                  <label className="text-2xs text-foreground-muted block mb-1">Quantity</label>
                  <input
                    type="number"
                    step="any"
                    required
                    value={formQty}
                    onChange={(e) => setFormQty(e.target.value)}
                    placeholder="1.5"
                    className="w-full bg-surface-raised border border-border rounded px-2.5 py-1.5 text-foreground font-mono outline-none focus:border-accent"
                  />
                </div>
                <div>
                  <label className="text-2xs text-foreground-muted block mb-1">Price</label>
                  <input
                    type="number"
                    step="any"
                    required
                    value={formPrice}
                    onChange={(e) => setFormPrice(e.target.value)}
                    placeholder="85000"
                    className="w-full bg-surface-raised border border-border rounded px-2.5 py-1.5 text-foreground font-mono outline-none focus:border-accent"
                  />
                </div>
                <div>
                  <label className="text-2xs text-foreground-muted block mb-1">Fee</label>
                  <input
                    type="number"
                    step="any"
                    value={formFee}
                    onChange={(e) => setFormFee(e.target.value)}
                    placeholder="5.00"
                    className="w-full bg-surface-raised border border-border rounded px-2.5 py-1.5 text-foreground font-mono outline-none focus:border-accent"
                  />
                </div>
              </div>
            </div>

            <div className="flex justify-end space-x-2 pt-2 border-t border-border">
              <button
                type="button"
                onClick={() => setAddModalOpen(false)}
                className="px-3 py-1.5 rounded bg-surface-raised hover:bg-surface-hover border border-border text-foreground-muted hover:text-foreground text-xs"
              >
                Cancel
              </button>
              <button
                type="submit"
                className="px-4 py-1.5 rounded bg-accent hover:bg-accent-hover text-background font-bold text-xs"
              >
                Save to Ledger
              </button>
            </div>
          </form>
        </div>
      )}
    </div>
  );
};
