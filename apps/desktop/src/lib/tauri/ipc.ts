import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import {
  ChartDataDto,
  CommandOutcomeDto,
  CompanyFactsDto,
  InstrumentSearchItemDto,
  MarketQuoteDto,
  MarketSnapshotDto,
  NewsItemDto,
  PortfolioSummaryDto,
  TransactionDto,
  TreasuryCurveDto,
} from "../../types";
import {
  generateMockCandles,
  MOCK_COMPANY_FACTS,
  MOCK_NEWS,
  MOCK_PORTFOLIO,
  MOCK_QUOTES,
  MOCK_SNAPSHOT,
  MOCK_TREASURY_CURVE,
} from "./mockData";

export const isTauri = () => {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
};

export async function getMarketSnapshot(): Promise<MarketSnapshotDto> {
  if (!isTauri()) {
    return MOCK_SNAPSHOT;
  }
  return await invoke<MarketSnapshotDto>("get_market_snapshot");
}

export async function getHistoricalCandles(
  assetKey: string,
  timeframe: string
): Promise<ChartDataDto> {
  if (!isTauri()) {
    const sym = assetKey.includes(":") ? assetKey.split(":")[1] : assetKey;
    const base = sym.includes("BTC")
      ? 81000
      : sym.includes("ETH")
      ? 2380
      : sym.includes("SOL")
      ? 102
      : sym.includes("AAPL")
      ? 234
      : sym.includes("NVDA")
      ? 138
      : 100;
    const res = generateMockCandles(base);
    res.asset_key = assetKey;
    res.symbol = sym;
    res.timeframe = timeframe;
    return res;
  }
  return await invoke<ChartDataDto>("get_historical_candles", { assetKey, timeframe });
}

export async function searchInstruments(query: string): Promise<InstrumentSearchItemDto[]> {
  if (!isTauri()) {
    const q = query.trim().toUpperCase();
    return MOCK_QUOTES.filter(
      (m) => m.symbol.includes(q) || m.key.includes(q) || (m.base_asset && m.base_asset.includes(q))
    ).map((m) => ({
      key: m.key,
      symbol: m.symbol,
      venue: m.venue,
      asset_class: m.asset_class,
      quote_currency: m.currency,
      base_asset: m.base_asset || m.symbol,
      status: m.status,
    }));
  }
  return await invoke<InstrumentSearchItemDto[]>("search_instruments", { query });
}

export async function getCompanyFacts(ticker: string): Promise<CompanyFactsDto | null> {
  if (!isTauri()) {
    return {
      ...MOCK_COMPANY_FACTS,
      ticker: ticker.toUpperCase(),
      name: ticker.toUpperCase() === "NVDA" ? "NVIDIA Corporation" : MOCK_COMPANY_FACTS.name,
    };
  }
  return await invoke<CompanyFactsDto | null>("get_company_facts", { ticker });
}

export async function getNews(
  category?: string,
  ticker?: string
): Promise<NewsItemDto[]> {
  if (!isTauri()) {
    if (!category || category === "ALL") return MOCK_NEWS;
    return MOCK_NEWS.filter(
      (n) => n.category.toUpperCase() === category.toUpperCase() || n.ticker === ticker
    );
  }
  return await invoke<NewsItemDto[]>("get_news", { category, ticker });
}

export async function getArticle(id: string): Promise<string | null> {
  if (!isTauri()) {
    const item = MOCK_NEWS.find((n) => n.id === id);
    if (!item) return null;
    return `# ${item.headline}\n\n**Published:** ${item.published_at} | **Source:** ${item.source}\n\n${item.summary}\n\nInstitutional desks continue to monitor order flows and liquidity profiles across active spot and futures venues. Technical indicators and depth profiles demonstrate balanced participation with tight bid-ask spreads across top tiers.\n\nRisk management protocols recommend ongoing assessment of macroeconomic volatility and rate path expectations.`;
  }
  return await invoke<string | null>("get_article", { id });
}

export async function getTreasuryCurve(): Promise<TreasuryCurveDto | null> {
  if (!isTauri()) {
    return MOCK_TREASURY_CURVE;
  }
  return await invoke<TreasuryCurveDto | null>("get_treasury_curve");
}

export async function getPortfolio(account: string = "default"): Promise<PortfolioSummaryDto> {
  if (!isTauri()) {
    return MOCK_PORTFOLIO;
  }
  return await invoke<PortfolioSummaryDto>("get_portfolio", { account });
}

export async function addTransaction(transaction: TransactionDto): Promise<void> {
  if (!isTauri()) {
    MOCK_PORTFOLIO.transactions.unshift(transaction);
    return;
  }
  await invoke("add_transaction", { transaction });
}

export async function deleteTransaction(id: string): Promise<void> {
  if (!isTauri()) {
    MOCK_PORTFOLIO.transactions = MOCK_PORTFOLIO.transactions.filter((t) => t.id !== id);
    return;
  }
  await invoke("delete_transaction", { id });
}

export async function executeTerminalCommand(command: string): Promise<CommandOutcomeDto> {
  if (!isTauri()) {
    const upper = command.trim().toUpperCase();
    let ws: string | null = null;
    let asset: string | null = null;

    if (upper === "LP" || upper === "LAUNCHPAD") ws = "LAUNCHPAD";
    else if (upper === "MKT" || upper === "MARKET") ws = "MARKET";
    else if (upper === "GP" || upper === "CHART") ws = "CHART";
    else if (upper === "FA" || upper === "FIN") ws = "FIN";
    else if (upper === "CN" || upper === "NEWS") ws = "NEWS";
    else if (upper === "EQS" || upper === "SCAN") ws = "SCAN";
    else if (upper === "PORT") ws = "PORT";
    else if (upper === "ECON") ws = "ECON";
    else if (upper === "DES") ws = "DES";

    for (const q of MOCK_QUOTES) {
      if (upper.includes(q.symbol)) {
        asset = q.key;
        break;
      }
    }

    return {
      success: true,
      message: `Executed in browser preview: ${command}`,
      workspace_target: ws,
      asset_target: asset,
      timeframe_target: null,
      statement_tab: null,
      quote_filter: null,
      alert_message: null,
    };
  }
  return await invoke<CommandOutcomeDto>("execute_terminal_command", { command });
}

export async function getWatchlists(): Promise<string[]> {
  if (!isTauri()) return ["default", "longterm", "crypto_focus"];
  return await invoke<string[]>("get_watchlists");
}

export async function getWatchlist(name: string): Promise<string[]> {
  if (!isTauri()) return MOCK_QUOTES.map((q) => q.key);
  return await invoke<string[]>("get_watchlist", { name });
}

export async function watchAsset(
  name: string,
  assetKey: string,
  add: boolean
): Promise<void> {
  if (!isTauri()) return;
  await invoke("watch_asset", { name, assetKey, add });
}

export async function getSetting(key: string): Promise<string | null> {
  if (!isTauri()) return null;
  return await invoke<string | null>("get_setting", { key });
}

export async function saveSetting(key: string, value: string): Promise<void> {
  if (!isTauri()) return;
  await invoke("save_setting", { key, value });
}

export async function subscribeQuoteStream(
  callback: (quotes: MarketQuoteDto[]) => void
): Promise<UnlistenFn> {
  if (!isTauri()) {
    // In web preview, simulate live ticks so prices and tables pulse dynamically
    const interval = setInterval(() => {
      const updated = MOCK_QUOTES.map((q) => {
        if (q.symbol === "BTCUSDT" || q.symbol === "ETHUSDT" || q.symbol === "SOLUSDT") {
          const delta = (Math.random() - 0.49) * (parseFloat(q.price) * 0.0008);
          const newPrice = (parseFloat(q.price) + delta).toFixed(q.symbol === "BTCUSDT" ? 2 : 2);
          const pct = ((delta / parseFloat(q.price)) * 100).toFixed(2);
          return {
            ...q,
            price: newPrice,
            change_percent_24h: (q.change_percent_24h ?? 0) + parseFloat(pct),
            received_at: new Date().toISOString(),
          };
        }
        return q;
      });
      callback(updated);
    }, 1500);

    return () => clearInterval(interval);
  }

  return await listen<MarketQuoteDto[]>("quote_stream", (event) => {
    callback(event.payload);
  });
}
