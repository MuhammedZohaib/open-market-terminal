import React, { useEffect, useState } from "react";
import {
  Activity,
  ArrowRight,
  ExternalLink,
  Globe,
  Info,
  LineChart as LineIcon,
  RefreshCw,
  TrendingDown,
  TrendingUp,
} from "lucide-react";
import {
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { getTreasuryCurve } from "../../lib/tauri/ipc";
import { TreasuryCurveDto, TreasuryTenorDto } from "../../types";

export const EconomicsWorkspace: React.FC = () => {
  const [curve, setCurve] = useState<TreasuryCurveDto | null>(null);
  const [loading, setLoading] = useState(false);
  const [selectedTenor, setSelectedTenor] = useState<TreasuryTenorDto | null>(null);

  useEffect(() => {
    setLoading(true);
    getTreasuryCurve()
      .then((data) => {
        setCurve(data);
        setLoading(false);
        if (data && data.tenors.length > 0) {
          setSelectedTenor(data.tenors[data.tenors.length - 1]);
        }
      })
      .catch((err) => {
        console.error("Failed to fetch Treasury curve:", err);
        setLoading(false);
      });
  }, []);

  const chartData = (curve?.tenors || []).map((t) => ({
    tenor: t.tenor.replace("US Treasury ", "").replace(" Yield", ""),
    yield: t.yield_value,
  }));

  const spread2y10y = curve?.spread_2y_10y ?? null;
  const spread3m10y = curve?.spread_3m_10y ?? null;

  return (
    <div className="flex-1 flex flex-col h-full bg-background overflow-hidden text-xs font-mono select-none">
      {/* Top Header Bar */}
      <div className="h-10 px-3 bg-surface border-b border-border flex items-center justify-between">
        <div className="flex items-center space-x-3">
          <div className="flex items-center space-x-1.5 font-bold text-accent">
            <Globe className="w-4 h-4" />
            <span className="text-xs uppercase">U.S. TREASURY YIELD CURVE (ECON / YCRV)</span>
          </div>

          <div className="h-4 w-px bg-border" />

          {curve && (
            <span className="text-2xs text-foreground-muted">
              Official Par Yield Curve Observation: <span className="text-foreground font-bold">{curve.observation_date}</span>
            </span>
          )}
        </div>

        {/* Key Spread Cards */}
        <div className="flex items-center space-x-4">
          <div className="flex items-center space-x-2 bg-surface-raised border border-border px-2.5 py-1 rounded">
            <span className="text-2xs text-foreground-muted">2Y/10Y SPREAD:</span>
            <span
              className={`font-bold num-tabular ${
                spread2y10y !== null && spread2y10y >= 0 ? "text-positive" : "text-negative"
              }`}
            >
              {spread2y10y !== null ? `${spread2y10y > 0 ? "+" : ""}${spread2y10y.toFixed(2)}%` : "—"}
            </span>
            <span className="text-2xs text-foreground-muted">
              {spread2y10y !== null && spread2y10y < 0 ? "(Inverted)" : "(Normal)"}
            </span>
          </div>

          <div className="flex items-center space-x-2 bg-surface-raised border border-border px-2.5 py-1 rounded">
            <span className="text-2xs text-foreground-muted">3M/10Y SPREAD:</span>
            <span
              className={`font-bold num-tabular ${
                spread3m10y !== null && spread3m10y >= 0 ? "text-positive" : "text-negative"
              }`}
            >
              {spread3m10y !== null ? `${spread3m10y > 0 ? "+" : ""}${spread3m10y.toFixed(2)}%` : "—"}
            </span>
          </div>
        </div>
      </div>

      {/* Main Content Area */}
      <div className="flex-1 flex flex-col lg:flex-row overflow-hidden">
        {/* Left: Interactive Yield Curve Chart */}
        <div className="flex-1 flex flex-col p-4 border-r border-border bg-background">
          <div className="flex items-center justify-between mb-2">
            <span className="text-2xs font-bold text-foreground-muted uppercase tracking-wider flex items-center space-x-1.5">
              <LineIcon className="w-3.5 h-3.5 text-accent" />
              <span>Daily Par Yield Curve Structure (1M through 30Y)</span>
            </span>
            <span className="text-2xs text-foreground-muted">Source: U.S. Department of the Treasury</span>
          </div>

          {loading ? (
            <div className="flex-1 flex flex-col items-center justify-center space-y-2">
              <RefreshCw className="w-6 h-6 text-accent animate-spin" />
              <span className="text-2xs text-foreground-muted">Loading Treasury yield data...</span>
            </div>
          ) : chartData.length > 0 ? (
            <div className="flex-1 w-full h-full min-h-[300px]">
              <ResponsiveContainer width="100%" height="100%">
                <LineChart data={chartData} margin={{ top: 20, right: 30, left: 10, bottom: 20 }}>
                  <CartesianGrid strokeDasharray="3 3" stroke="#1c242d" />
                  <XAxis dataKey="tenor" stroke="#8b97a6" fontSize={11} tickLine={false} />
                  <YAxis
                    stroke="#8b97a6"
                    fontSize={11}
                    tickLine={false}
                    domain={["auto", "auto"]}
                    tickFormatter={(v) => `${v.toFixed(2)}%`}
                  />
                  <Tooltip
                    contentStyle={{
                      backgroundColor: "#171d25",
                      borderColor: "#29313b",
                      fontSize: "11px",
                      color: "#e9edf2",
                    }}
                    formatter={(val: any) => [`${parseFloat(val).toFixed(2)}%`, "Yield"]}
                  />
                  <Line
                    type="monotone"
                    dataKey="yield"
                    stroke="#e9a441"
                    strokeWidth={2}
                    dot={{ fill: "#e9a441", r: 4 }}
                    activeDot={{ r: 6, fill: "#f4b85c" }}
                  />
                </LineChart>
              </ResponsiveContainer>
            </div>
          ) : (
            <div className="flex-1 flex flex-col items-center justify-center text-foreground-muted p-8 text-center">
              <Globe className="w-8 h-8 opacity-40 mb-2" />
              <p>No Treasury curve data currently loaded in cache.</p>
              <p className="text-2xs mt-1">Connect to network to fetch latest official Treasury observations.</p>
            </div>
          )}
        </div>

        {/* Right: Tenor Observations Table & Source Provenance */}
        <div className="w-80 bg-surface flex flex-col overflow-hidden">
          <div className="p-2.5 bg-surface-raised border-b border-border text-2xs font-bold text-foreground-muted">
            TENOR OBSERVATIONS
          </div>

          <div className="flex-1 overflow-y-auto">
            <table className="w-full border-collapse table-dense">
              <thead className="sticky top-0 bg-surface border-b border-border text-foreground-muted text-2xs">
                <tr>
                  <th className="py-1 px-3 text-left">MATURITY</th>
                  <th className="py-1 px-3 text-right">PAR YIELD</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border-subtle">
                {(curve?.tenors || []).map((t) => (
                  <tr
                    key={t.tenor}
                    onClick={() => setSelectedTenor(t)}
                    className="hover:bg-surface-raised cursor-pointer transition-colors"
                  >
                    <td className="py-1.5 px-3 font-bold text-foreground">{t.tenor}</td>
                    <td className="py-1.5 px-3 text-right num-tabular font-bold text-accent">
                      {t.yield_value.toFixed(2)}%
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {/* Provenance Box */}
          <div className="p-3 bg-surface-raised border-t border-border space-y-2 text-2xs">
            <span className="text-foreground-muted font-bold block uppercase flex items-center space-x-1">
              <Info className="w-3.5 h-3.5 text-accent" />
              <span>Official Treasury Provenance</span>
            </span>
            <p className="text-foreground-muted">
              Yields derived directly from the official U.S. Treasury daily XML feed. Represents closing market bids on OTC Treasury securities.
            </p>
          </div>
        </div>
      </div>
    </div>
  );
};
