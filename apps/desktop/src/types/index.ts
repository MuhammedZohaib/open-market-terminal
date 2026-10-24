export interface MarketQuoteDto {
  key: string;
  symbol: string;
  venue: string;
  asset_class: string;
  currency: string;
  base_asset: string | null;
  price: string;
  change_24h: string | null;
  change_percent_24h: number | null;
  high_24h: string | null;
  low_24h: string | null;
  volume_24h: string | null;
  quote_volume_24h: string | null;
  bid: string | null;
  ask: string | null;
  spread: string | null;
  spread_bps: number | null;
  vwap: string | null;
  status: string;
  event_at: string;
  received_at: string;
}

export interface MarketSnapshotDto {
  quotes: MarketQuoteDto[];
  total_instruments: number;
  active_instruments: number;
  top_gainers: MarketQuoteDto[];
  top_losers: MarketQuoteDto[];
  most_active: MarketQuoteDto[];
  connection_status: string;
  last_snapshot_at: string | null;
}

export interface CandleDto {
  time: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
  is_closed: boolean;
}

export interface IndicatorsDto {
  sma20: number | null;
  sma50: number | null;
  ema12: number | null;
  ema26: number | null;
  rsi14: number | null;
  macd: number | null;
  macd_signal: number | null;
  macd_hist: number | null;
  atr14: number | null;
  bb_upper: number | null;
  bb_middle: number | null;
  bb_lower: number | null;
}

export interface ChartDataDto {
  asset_key: string;
  symbol: string;
  venue: string;
  timeframe: string;
  candles: CandleDto[];
  indicators: IndicatorsDto[];
  status: string;
  error_message: string | null;
  received_at: string | null;
}

export interface InstrumentSearchItemDto {
  key: string;
  symbol: string;
  venue: string;
  asset_class: string;
  quote_currency: string;
  base_asset: string;
  status: string;
}

export interface NewsItemDto {
  id: string;
  headline: string;
  source: string;
  url: string;
  published_at: string;
  category: string;
  ticker: string | null;
  summary: string;
  body: string | null;
  is_sec_filing: boolean;
  form: string | null;
}

export interface StatementRowDto {
  concept: string;
  label: string;
  values: (string | null)[];
  is_header: boolean;
}

export interface StatementMatrixDto {
  periods: string[];
  rows: StatementRowDto[];
}

export interface SecFilingDto {
  form: string;
  filed: string;
  report_date: string | null;
  accession: string;
  description: string;
  document: string;
  url: string;
}

export interface CompanyFactsDto {
  ticker: string;
  cik: string;
  name: string;
  facts_count: number;
  income_statement_annual: StatementMatrixDto;
  income_statement_quarterly: StatementMatrixDto;
  balance_sheet_annual: StatementMatrixDto;
  balance_sheet_quarterly: StatementMatrixDto;
  cash_flow_annual: StatementMatrixDto;
  cash_flow_quarterly: StatementMatrixDto;
  ratios_annual: StatementMatrixDto;
  ratios_quarterly: StatementMatrixDto;
  recent_filings: SecFilingDto[];
}

export interface HoldingDto {
  asset_key: string;
  symbol: string;
  venue: string;
  quantity: string;
  average_cost: string;
  cost_basis: string;
  current_price: string | null;
  market_value: string | null;
  unrealized_pnl: string | null;
  unrealized_pnl_percent: number | null;
}

export interface TransactionDto {
  id: string;
  account: string;
  asset_key: string;
  symbol: string;
  side: string;
  quantity: string;
  price: string;
  fee: string;
  currency: string;
  at: string;
  total_amount: string;
}

export interface PortfolioSummaryDto {
  account: string;
  total_cost_basis: string;
  total_market_value: string;
  total_unrealized_pnl: string;
  total_unrealized_pnl_percent: number | null;
  holdings: HoldingDto[];
  transactions: TransactionDto[];
}

export interface TreasuryTenorDto {
  tenor: string;
  yield_value: number;
}

export interface TreasuryCurveDto {
  observation_date: string;
  tenors: TreasuryTenorDto[];
  spread_2y_10y: number | null;
  spread_3m_10y: number | null;
  source: string;
}

export interface CommandOutcomeDto {
  success: boolean;
  message: string;
  workspace_target: string | null;
  asset_target: string | null;
  timeframe_target: string | null;
  statement_tab: string | null;
  quote_filter: string | null;
  alert_message: string | null;
}

export type WorkspaceType =
  | "LAUNCHPAD"
  | "MARKET"
  | "CHART"
  | "FIN"
  | "NEWS"
  | "SCAN"
  | "PORT"
  | "ECON"
  | "DES"
  | "WATCHLIST"
  | "SETTINGS";

export type LinkGroup = "A" | "B" | "NONE";

export type DensityMode = "COMPACT" | "COMFORTABLE" | "ULTRA_DENSE";
