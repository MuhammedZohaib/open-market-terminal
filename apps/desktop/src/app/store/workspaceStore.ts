import { create } from "zustand";
import {
  DensityMode,
  LinkGroup,
  MarketQuoteDto,
  MarketSnapshotDto,
  WorkspaceType,
} from "../../types";

interface WorkspaceState {
  workspace: WorkspaceType;
  activeAsset: string;
  activeTimeframe: string;
  linkGroupAAsset: string;
  linkGroupBAsset: string;
  density: DensityMode;
  commandPaletteOpen: boolean;
  selectedSymbol: string | null;
  quoteFilter: string;
  connectionStatus: string;
  recentSecurities: string[];
  quotesMap: Record<string, MarketQuoteDto>;
  topGainers: MarketQuoteDto[];
  topLosers: MarketQuoteDto[];
  mostActive: MarketQuoteDto[];
  totalInstruments: number;
  activeInstruments: number;

  setWorkspace: (ws: WorkspaceType) => void;
  setActiveAsset: (asset: string, group?: LinkGroup) => void;
  setActiveTimeframe: (tf: string) => void;
  setSelectedSymbol: (sym: string | null) => void;
  setQuoteFilter: (filter: string) => void;
  setDensity: (density: DensityMode) => void;
  setCommandPaletteOpen: (open: boolean) => void;
  setConnectionStatus: (status: string) => void;
  setMarketSnapshot: (snapshot: MarketSnapshotDto) => void;
  batchUpdateQuotes: (quotes: MarketQuoteDto[]) => void;
}

export const useWorkspaceStore = create<WorkspaceState>((set) => ({
  workspace: "LAUNCHPAD",
  activeAsset: "BINANCE:BTCUSDT",
  activeTimeframe: "1h",
  linkGroupAAsset: "BINANCE:BTCUSDT",
  linkGroupBAsset: "BINANCE:ETHUSDT",
  density: "COMPACT",
  commandPaletteOpen: false,
  selectedSymbol: "BTCUSDT",
  quoteFilter: "USDT",
  connectionStatus: "Connecting",
  recentSecurities: ["BINANCE:BTCUSDT", "BINANCE:ETHUSDT", "IEX:AAPL", "IEX:NVDA"],
  quotesMap: {},
  topGainers: [],
  topLosers: [],
  mostActive: [],
  totalInstruments: 0,
  activeInstruments: 0,

  setWorkspace: (workspace) => set({ workspace }),

  setActiveAsset: (asset, group = "A") =>
    set((state) => {
      const recent = [asset, ...state.recentSecurities.filter((a) => a !== asset)].slice(0, 16);
      const sym = asset.includes(":") ? asset.split(":")[1] : asset;

      if (group === "A") {
        return {
          activeAsset: asset,
          linkGroupAAsset: asset,
          selectedSymbol: sym,
          recentSecurities: recent,
        };
      } else if (group === "B") {
        return {
          activeAsset: asset,
          linkGroupBAsset: asset,
          selectedSymbol: sym,
          recentSecurities: recent,
        };
      } else {
        return {
          activeAsset: asset,
          selectedSymbol: sym,
          recentSecurities: recent,
        };
      }
    }),

  setActiveTimeframe: (activeTimeframe) => set({ activeTimeframe }),
  setSelectedSymbol: (selectedSymbol) => set({ selectedSymbol }),
  setQuoteFilter: (quoteFilter) => set({ quoteFilter }),
  setDensity: (density) => set({ density }),
  setCommandPaletteOpen: (commandPaletteOpen) => set({ commandPaletteOpen }),
  setConnectionStatus: (connectionStatus) => set({ connectionStatus }),

  setMarketSnapshot: (snapshot) =>
    set((state) => {
      const map = { ...state.quotesMap };
      for (const q of snapshot.quotes) {
        map[q.key] = q;
      }
      return {
        quotesMap: map,
        totalInstruments: snapshot.total_instruments,
        activeInstruments: snapshot.active_instruments,
        topGainers: snapshot.top_gainers,
        topLosers: snapshot.top_losers,
        mostActive: snapshot.most_active,
        connectionStatus: snapshot.connection_status || state.connectionStatus,
      };
    }),

  batchUpdateQuotes: (incomingQuotes) =>
    set((state) => {
      const map = { ...state.quotesMap };
      for (const q of incomingQuotes) {
        map[q.key] = q;
      }
      return { quotesMap: map };
    }),
}));
