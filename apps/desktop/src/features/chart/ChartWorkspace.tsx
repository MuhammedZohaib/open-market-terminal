import React, { useEffect, useRef, useState } from "react";
import {
  ColorType,
  createChart,
  IChartApi,
  ISeriesApi,
  LineStyle,
} from "lightweight-charts";
import {
  Activity,
  ArrowDown,
  ArrowUp,
  BarChart2,
  Calendar,
  CandlestickChart,
  Maximize2,
  Minimize2,
  RefreshCw,
  Sliders,
  TrendingUp,
} from "lucide-react";
import { useWorkspaceStore } from "../../app/store/workspaceStore";
import { getHistoricalCandles } from "../../lib/tauri/ipc";
import { formatPercent, formatPrice, formatVolume } from "../../lib/utils";
import { ChartDataDto } from "../../types";

const TIMEFRAMES = ["1m", "5m", "15m", "1h", "4h", "1d"];

export const ChartWorkspace: React.FC = () => {
  const { activeAsset, activeTimeframe, setActiveTimeframe, quotesMap } =
    useWorkspaceStore();

  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const candleSeriesRef = useRef<ISeriesApi<"Candlestick"> | null>(null);
  const volumeSeriesRef = useRef<ISeriesApi<"Histogram"> | null>(null);
  const sma20SeriesRef = useRef<ISeriesApi<"Line"> | null>(null);
  const sma50SeriesRef = useRef<ISeriesApi<"Line"> | null>(null);

  const [chartData, setChartData] = useState<ChartDataDto | null>(null);
  const [loading, setLoading] = useState(true);
  const [showIndicators, setShowIndicators] = useState(true);
  const [hoveredCandle, setHoveredCandle] = useState<any>(null);

  const curQuote = quotesMap[activeAsset];

  // Fetch historical data
  useEffect(() => {
    let isMounted = true;
    setLoading(true);

    getHistoricalCandles(activeAsset, activeTimeframe)
      .then((data) => {
        if (isMounted) {
          setChartData(data);
          setLoading(false);
        }
      })
      .catch((err) => {
        console.error("Failed to load historical candles:", err);
        if (isMounted) setLoading(false);
      });

    return () => {
      isMounted = false;
    };
  }, [activeAsset, activeTimeframe]);

  // Initialize and update Lightweight Charts
  useEffect(() => {
    if (!containerRef.current || !chartData) return;

    // Clean up previous chart if any
    if (chartRef.current) {
      chartRef.current.remove();
      chartRef.current = null;
    }

    const container = containerRef.current;
    const chart = createChart(container, {
      width: container.clientWidth,
      height: container.clientHeight,
      layout: {
        background: { type: ColorType.Solid, color: "#0b0e12" },
        textColor: "#8b97a6",
        fontFamily: "'JetBrains Mono', monospace",
        fontSize: 11,
      },
      grid: {
        vertLines: { color: "#1c242d" },
        horzLines: { color: "#1c242d" },
      },
      crosshair: {
        mode: 1, // CrosshairMode.Normal
        vertLine: {
          color: "#e9a441",
          width: 1,
          style: LineStyle.Dashed,
          labelBackgroundColor: "#171d25",
        },
        horzLine: {
          color: "#e9a441",
          width: 1,
          style: LineStyle.Dashed,
          labelBackgroundColor: "#171d25",
        },
      },
      timeScale: {
        borderColor: "#29313b",
        timeVisible: true,
        secondsVisible: false,
      },
      rightPriceScale: {
        borderColor: "#29313b",
      },
    });

    // Main Candlestick Series
    const candleSeries = chart.addCandlestickSeries({
      upColor: "#38b887",
      downColor: "#ed6976",
      borderVisible: false,
      wickUpColor: "#38b887",
      wickDownColor: "#ed6976",
    });

    // Volume Histogram Series
    const volumeSeries = chart.addHistogramSeries({
      color: "#26a69a",
      priceFormat: {
        type: "volume",
      },
      priceScaleId: "", // Set as overlay
    });

    volumeSeries.priceScale().applyOptions({
      scaleMargins: {
        top: 0.8,
        bottom: 0,
      },
    });

    // Format candle data for TradingView
    const formattedCandles = chartData.candles.map((c) => ({
      time: c.time as any,
      open: c.open,
      high: c.high,
      low: c.low,
      close: c.close,
    }));

    const formattedVolumes = chartData.candles.map((c) => ({
      time: c.time as any,
      value: c.volume,
      color: c.close >= c.open ? "rgba(56, 184, 135, 0.3)" : "rgba(237, 105, 118, 0.3)",
    }));

    candleSeries.setData(formattedCandles);
    volumeSeries.setData(formattedVolumes);

    // SMA lines if enabled
    let sma20Series: ISeriesApi<"Line"> | null = null;
    let sma50Series: ISeriesApi<"Line"> | null = null;

    if (showIndicators) {
      sma20Series = chart.addLineSeries({
        color: "#e9a441",
        lineWidth: 2,
        title: "SMA 20",
      });

      sma50Series = chart.addLineSeries({
        color: "#38c0e0",
        lineWidth: 2,
        title: "SMA 50",
      });

      const sma20Data: any[] = [];
      const sma50Data: any[] = [];

      chartData.candles.forEach((c, idx) => {
        const ind = chartData.indicators[idx];
        if (ind?.sma20 !== null && ind?.sma20 !== undefined) {
          sma20Data.push({ time: c.time as any, value: ind.sma20 });
        }
        if (ind?.sma50 !== null && ind?.sma50 !== undefined) {
          sma50Data.push({ time: c.time as any, value: ind.sma50 });
        }
      });

      sma20Series.setData(sma20Data);
      sma50Series.setData(sma50Data);
    }

    // Subscribe to crosshair move for OHLC tooltip
    chart.subscribeCrosshairMove((param) => {
      if (
        param.point === undefined ||
        !param.time ||
        param.point.x < 0 ||
        param.point.x > container.clientWidth ||
        param.point.y < 0 ||
        param.point.y > container.clientHeight
      ) {
        setHoveredCandle(null);
      } else {
        const barData = param.seriesData.get(candleSeries) as any;
        const volData = param.seriesData.get(volumeSeries) as any;
        if (barData) {
          setHoveredCandle({
            ...barData,
            volume: volData?.value,
          });
        }
      }
    });

    chart.timeScale().fitContent();

    chartRef.current = chart;
    candleSeriesRef.current = candleSeries;
    volumeSeriesRef.current = volumeSeries;
    sma20SeriesRef.current = sma20Series;
    sma50SeriesRef.current = sma50Series;

    // Handle container resize
    const handleResize = () => {
      if (chartRef.current && containerRef.current) {
        chartRef.current.applyOptions({
          width: containerRef.current.clientWidth,
          height: containerRef.current.clientHeight,
        });
      }
    };

    window.addEventListener("resize", handleResize);

    return () => {
      window.removeEventListener("resize", handleResize);
      if (chartRef.current) {
        chartRef.current.remove();
        chartRef.current = null;
      }
    };
  }, [chartData, showIndicators]);

  // Live price tick incremental update to the latest candle
  useEffect(() => {
    if (!curQuote || !candleSeriesRef.current || !chartData || chartData.candles.length === 0)
      return;

    const currentPrice = parseFloat(curQuote.price);
    if (isNaN(currentPrice)) return;

    const lastCandle = chartData.candles[chartData.candles.length - 1];
    const updatedLastCandle = {
      time: lastCandle.time as any,
      open: lastCandle.open,
      high: Math.max(lastCandle.high, currentPrice),
      low: Math.min(lastCandle.low, currentPrice),
      close: currentPrice,
    };

    candleSeriesRef.current.update(updatedLastCandle);
  }, [curQuote, chartData]);

  const activeSym = activeAsset.includes(":") ? activeAsset.split(":")[1] : activeAsset;

  return (
    <div className="flex-1 flex flex-col h-full bg-background overflow-hidden font-mono select-none">
      {/* Top Chart Toolbar */}
      <div className="h-10 px-3 bg-surface border-b border-border flex items-center justify-between z-20">
        <div className="flex items-center space-x-3">
          <div className="flex items-center space-x-1.5">
            <span className="font-bold text-foreground text-sm">{activeSym}</span>
            <span className="text-2xs text-foreground-muted">[{activeAsset.split(":")[0]}]</span>
          </div>

          <div className="h-4 w-px bg-border" />

          {/* Timeframe Selector */}
          <div className="flex items-center bg-surface-raised border border-border rounded p-0.5 space-x-0.5">
            {TIMEFRAMES.map((tf) => (
              <button
                key={tf}
                onClick={() => setActiveTimeframe(tf)}
                className={`px-2 py-0.5 rounded text-2xs font-bold transition-colors ${
                  activeTimeframe.toLowerCase() === tf
                    ? "bg-accent text-background"
                    : "text-foreground-muted hover:text-foreground"
                }`}
              >
                {tf.toUpperCase()}
              </button>
            ))}
          </div>

          <div className="h-4 w-px bg-border" />

          {/* Indicators Toggle */}
          <button
            onClick={() => setShowIndicators(!showIndicators)}
            className={`flex items-center space-x-1 px-2 py-0.5 rounded text-2xs font-bold border transition-colors ${
              showIndicators
                ? "bg-surface-raised border-accent text-accent"
                : "bg-surface border-border text-foreground-muted hover:text-foreground"
            }`}
          >
            <Sliders className="w-3 h-3" />
            <span>SMA 20/50</span>
          </button>
        </div>

        {/* Right Info: Live Price & OHLC Display */}
        <div className="flex items-center space-x-4 text-2xs">
          {curQuote && (
            <div className="flex items-center space-x-2">
              <span className="text-foreground-muted">LAST:</span>
              <span className="font-bold text-foreground num-tabular text-sm">
                {formatPrice(curQuote.price)}
              </span>
              <span
                className={`num-tabular font-bold ${
                  (curQuote.change_percent_24h ?? 0) >= 0 ? "text-positive" : "text-negative"
                }`}
              >
                {formatPercent(curQuote.change_percent_24h)}
              </span>
            </div>
          )}

          <button
            onClick={() => chartRef.current?.timeScale().fitContent()}
            className="px-2 py-0.5 rounded bg-surface-raised border border-border hover:border-accent text-foreground-muted hover:text-foreground text-2xs"
            title="Auto-fit viewport"
          >
            Fit
          </button>
        </div>
      </div>

      {/* Crosshair OHLC Floating Header */}
      {hoveredCandle && (
        <div className="h-6 px-3 bg-surface-raised border-b border-border-subtle flex items-center space-x-4 text-2xs font-mono text-foreground-muted z-10">
          <div>O: <span className="text-foreground num-tabular font-bold">{formatPrice(hoveredCandle.open)}</span></div>
          <div>H: <span className="text-positive num-tabular font-bold">{formatPrice(hoveredCandle.high)}</span></div>
          <div>L: <span className="text-negative num-tabular font-bold">{formatPrice(hoveredCandle.low)}</span></div>
          <div>C: <span className="text-foreground num-tabular font-bold">{formatPrice(hoveredCandle.close)}</span></div>
          {hoveredCandle.volume && (
            <div>Vol: <span className="text-accent num-tabular">{formatVolume(hoveredCandle.volume)}</span></div>
          )}
        </div>
      )}

      {/* Chart Canvas Area */}
      <div className="flex-1 relative w-full h-full overflow-hidden">
        {loading && (
          <div className="absolute inset-0 z-20 flex flex-col items-center justify-center bg-background/60 backdrop-blur-xs space-y-2">
            <RefreshCw className="w-6 h-6 text-accent animate-spin" />
            <span className="text-2xs text-foreground-muted font-mono">
              Loading {activeSym} historical candles...
            </span>
          </div>
        )}

        <div ref={containerRef} className="w-full h-full" />
      </div>
    </div>
  );
};
