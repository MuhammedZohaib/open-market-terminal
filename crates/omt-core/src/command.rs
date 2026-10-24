use crate::{AssetId, Timeframe};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Workspace {
    Market,
    Chart,
    Fin,
    News,
    Scan,
    Port,
    Econ,
    Impact,
}
impl Workspace {
    pub const ALL: [Self; 8] = [
        Self::Market,
        Self::Chart,
        Self::Fin,
        Self::News,
        Self::Scan,
        Self::Port,
        Self::Econ,
        Self::Impact,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Market => "MARKET",
            Self::Chart => "CHART",
            Self::Fin => "FIN",
            Self::News => "NEWS",
            Self::Scan => "SCAN",
            Self::Port => "PORT",
            Self::Econ => "ECON",
            Self::Impact => "IMPACT",
        }
    }
    pub fn mnemonic(self) -> &'static str {
        match self {
            Self::Market => "MKT",
            Self::Chart => "GP",
            Self::Fin => "FA",
            Self::News => "CN",
            Self::Scan => "EQS",
            Self::Port => "PRTU",
            Self::Econ => "ECST",
            Self::Impact => "EVT",
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Open(Workspace, Option<AssetId>, Option<Timeframe>),
    Watch(bool, AssetId),
    Watchlist(String),
    Help,
    Settings,
    Raw(Vec<String>),
}
pub const COMMANDS: &[&str] = &[
    "ALERT",
    "ALERT LIST",
    "ALERT ADD",
    "ALERT REMOVE",
    "STUDY",
    "IMPACT",
    "IMPACT AAPL SPY",
    "COMPARE",
    "EVENT",
    "MARKET",
    "CRYPTO",
    "CRYPTO ALL",
    "CRYPTO USDT",
    "CRYPTO USDC",
    "CRYPTO BTC",
    "WEI",
    "MOST",
    "CHART",
    "FIN",
    "NEWS",
    "TOP",
    "NEWS CRYPTO",
    "NEWS MACRO",
    "NEWS EQUITIES",
    "NEWS MARKETS",
    "NEWS EARNINGS",
    "FILINGS",
    "FILINGS AAPL",
    "NEWS AAPL SEC",
    "SCAN",
    "SCAN CRYPTO",
    "SCAN EQUITIES",
    "PORT",
    "ECON",
    "GP",
    "GIP",
    "FA",
    "DES",
    "CN",
    "EQS",
    "PRTU",
    "ECST",
    "YCRV",
    "LAUNCHPAD",
    "LAYOUT STANDARD",
    "LAYOUT FULL",
    "LAYOUT SPLIT",
    "LAYOUT QUAD",
    "LAYOUT WATCH",
    "LAYOUT SOURCE",
    "WATCH ADD",
    "WATCH REMOVE",
    "WATCHLIST",
    "BUY",
    "ACCOUNT",
    "PRESET",
    "SELL",
    "DELTX",
    "IMPORT",
    "EXPORT",
    "WB",
    "FRED",
    "TREASURY",
    "FILTER",
    "FILTER CLEAR",
    "QUOTE USDT",
    "QUOTE USDC",
    "QUOTE BTC",
    "QUOTE ALL",
    "MINVOL",
    "STATUS TRADING",
    "STATUS ALL",
    "SORT",
    "SORT QVOL DESC",
    "SORT CHANGE DESC",
    "SORT CHANGE ASC",
    "SORT PRICE DESC",
    "SORT SYMBOL ASC",
    "INDEX",
    "IND",
    "OPEN",
    "READ",
    "REFRESH",
    "BACK",
    "HELP",
    "SETTINGS",
];
fn parse_workspace(token: &str) -> Option<(Workspace, Option<Timeframe>)> {
    match token {
        "MARKET" | "MKT" => Some((Workspace::Market, None)),
        "CHART" | "GP" => Some((Workspace::Chart, None)),
        "GIP" => Some((Workspace::Chart, Some(Timeframe::M5))),
        "FIN" | "FA" => Some((Workspace::Fin, None)),
        "NEWS" | "CN" | "CF" => Some((Workspace::News, None)),
        "SCAN" | "SRCH" => Some((Workspace::Scan, None)),
        "PORT" | "PRTU" => Some((Workspace::Port, None)),
        "ECON" | "ECST" | "ECO" => Some((Workspace::Econ, None)),
        _ => None,
    }
}
fn is_sector_tag(token: &str) -> bool {
    matches!(
        token.to_ascii_uppercase().as_str(),
        "<EQUITY>"
            | "EQUITY"
            | "<CURNCY>"
            | "CURNCY"
            | "<COMDTY>"
            | "COMDTY"
            | "<INDEX>"
            | "<CORP>"
            | "CORP"
            | "<GOVT>"
            | "GOVT"
    )
}
pub fn parse(text: &str) -> Result<Command, String> {
    let mut parts: Vec<_> = text.split_whitespace().collect();
    if parts.len() > 1
        && matches!(
            parts.last().map(|s| s.to_ascii_uppercase()).as_deref(),
            Some("<GO>" | "GO")
        )
    {
        parts.pop();
    }
    if parts.is_empty() || (parts.len() == 1 && parts[0].eq_ignore_ascii_case("<GO>")) {
        return Err("Enter a command or symbol".into());
    }
    let head = parts[0].to_ascii_uppercase();
    if matches!(
        head.as_str(),
        "IMPACT" | "COMPARE" | "EVENT" | "STUDY" | "ALERT" | "PLOT"
    ) {
        return Ok(Command::Raw(parts.iter().map(|s| s.to_string()).collect()));
    }
    if parts.len() >= 2
        && !matches!(
            head.as_str(),
            "WB" | "SCAN"
                | "SRCH"
                | "MARKET"
                | "MKT"
                | "CRYPTO"
                | "NEWS"
                | "CN"
                | "FILINGS"
                | "CF"
                | "FILTER"
                | "SORT"
                | "QUOTE"
                | "STATUS"
                | "LAYOUT"
                | "PRESET"
                | "WATCH"
                | "BUY"
                | "SELL"
                | "INDEX"
        )
    {
        let mut cleaned = vec![parts[0]];
        let mut idx = 1;
        while idx < parts.len() {
            let cur_up = parts[idx].to_ascii_uppercase();
            let next_is_eq = parts
                .get(idx + 1)
                .is_some_and(|n| matches!(n.to_ascii_uppercase().as_str(), "EQUITY" | "<EQUITY>"));
            if (cur_up == "US" && next_is_eq) || is_sector_tag(parts[idx]) {
                idx += 1;
                continue;
            }
            cleaned.push(parts[idx]);
            idx += 1;
        }
        parts = cleaned;
    }
    if matches!(
        head.as_str(),
        "CRYPTO" | "WEI" | "MOST" | "TOP" | "LAUNCHPAD"
    ) {
        return Ok(Command::Raw(parts.iter().map(|s| s.to_string()).collect()));
    }
    if matches!(head.as_str(), "FILINGS" | "CF") {
        let mut raw = vec!["FILINGS".to_string()];
        raw.extend(parts[1..].iter().map(|s| s.to_string()));
        return Ok(Command::Raw(raw));
    }
    if head == "NEWS" && parts.len() >= 2 {
        let second = parts[1].to_ascii_uppercase();
        if crate::NewsCategory::from_token(&second).is_some()
            || parts
                .get(2)
                .is_some_and(|t| crate::NewsCategory::from_token(t).is_some())
        {
            return Ok(Command::Raw(parts.iter().map(|s| s.to_string()).collect()));
        }
    }
    if head == "CN" && parts.len() == 1 {
        return Ok(Command::Raw(vec!["CN".into()]));
    }
    if head == "EQS" {
        let mut raw = vec!["SCAN".to_string(), "EQUITIES".to_string()];
        raw.extend(parts[1..].iter().map(|s| s.to_string()));
        return Ok(Command::Raw(raw));
    }
    if matches!(head.as_str(), "SCAN" | "SRCH") && parts.len() >= 2 {
        let sub = parts[1].to_ascii_uppercase();
        if matches!(
            sub.as_str(),
            "CRYPTO" | "EQUITIES" | "EQUITY" | "STOCKS" | "SEC"
        ) {
            return Ok(Command::Raw(parts.iter().map(|s| s.to_string()).collect()));
        }
    }
    if matches!(head.as_str(), "MARKET" | "MKT") && parts.len() >= 2 {
        let sub = parts[1].to_ascii_uppercase();
        if matches!(
            sub.as_str(),
            "ALL"
                | "USDT"
                | "USDC"
                | "BTC"
                | "ETH"
                | "BNB"
                | "FDUSD"
                | "EUR"
                | "TRY"
                | "GAINERS"
                | "LOSERS"
                | "ACTIVE"
                | "OVERVIEW"
        ) {
            return Ok(Command::Raw(parts.iter().map(|s| s.to_string()).collect()));
        }
    }
    if matches!(head.as_str(), "FIN" | "FA") && parts.len() >= 2 {
        let last = parts.last().unwrap().to_ascii_uppercase();
        if matches!(
            last.as_str(),
            "OVERVIEW" | "DES" | "INCOME" | "BALANCE" | "BS" | "CASH" | "CF" | "RATIOS"
        ) {
            if parts.len() == 2 {
                return Ok(Command::Raw(vec!["FIN_TAB".into(), last]));
            } else if parts.len() == 3 {
                return Ok(Command::Raw(vec![
                    "FIN_TAB".into(),
                    last,
                    parts[1].to_ascii_uppercase(),
                ]));
            }
        }
    }
    if let Some((w, default_tf)) = parse_workspace(&head) {
        if parts.len() > 3 {
            return Err("Use WORKSPACE [SYMBOL] [TIMEFRAME]".into());
        }
        let asset = parts.get(1).map(|s| s.parse()).transpose()?;
        let tf = parts.get(2).map(|s| s.parse()).transpose()?.or(default_tf);
        return Ok(Command::Open(w, asset, tf));
    }
    if head == "DES" {
        if parts.len() > 2 {
            return Err("Use DES [SYMBOL]".into());
        }
        let mut raw = vec!["DES".to_string()];
        if let Some(s) = parts.get(1) {
            raw.push(s.to_string());
        }
        return Ok(Command::Raw(raw));
    }
    if matches!(head.as_str(), "YCRV" | "GC") && parts.len() == 1 {
        return Ok(Command::Raw(vec!["TREASURY".into()]));
    }
    match head.as_str() {
        "HELP" => Ok(Command::Help),
        "SETTINGS" => Ok(Command::Settings),
        "WATCH" if parts.len() == 3 => {
            let add = match parts[1].to_ascii_uppercase().as_str() {
                "ADD" => true,
                "REMOVE" => false,
                _ => return Err("WATCH ADD|REMOVE SYMBOL".into()),
            };
            Ok(Command::Watch(add, parts[2].parse()?))
        }
        "WATCHLIST" if parts.len() == 2 => Ok(Command::Watchlist(parts[1].into())),
        "BUY" | "SELL" | "DELTX" | "IMPORT" | "EXPORT" | "WB" | "FRED" | "TREASURY" | "FILTER"
        | "SORT" | "INDEX" | "ACCOUNT" | "PRESET" | "IND" | "QUOTE" | "MINVOL" | "STATUS"
        | "OPEN" | "READ" | "REFRESH" | "LAYOUT" | "BACK" | "PLOT" => {
            Ok(Command::Raw(parts.iter().map(|s| s.to_string()).collect()))
        }
        _ if parts.len() >= 2 => {
            let second = parts[1].to_ascii_uppercase();
            if matches!(second.as_str(), "FILINGS" | "CF") && parts.len() == 2 {
                return Ok(Command::Raw(vec![
                    "FILINGS".into(),
                    parts[0].to_ascii_uppercase(),
                ]));
            }
            if let Some((w, default_tf)) = parse_workspace(&second) {
                if parts.len() > 3 {
                    return Err("Use SYMBOL FUNCTION [TIMEFRAME]".into());
                }
                let asset: AssetId = parts[0].parse()?;
                let tf = parts.get(2).map(|s| s.parse()).transpose()?.or(default_tf);
                return Ok(Command::Open(w, Some(asset), tf));
            }
            if second == "DES" && parts.len() == 2 {
                return Ok(Command::Raw(vec![
                    "DES".into(),
                    parts[0].to_ascii_uppercase(),
                ]));
            }
            Err("Unknown command. Press ? for help.".into())
        }
        _ if parts.len() == 1 => Ok(Command::Open(
            Workspace::Chart,
            Some(parts[0].parse()?),
            None,
        )),
        _ => Err("Unknown command. Press ? for help.".into()),
    }
}
pub fn suggestions(input: &str, symbols: &[String]) -> Vec<String> {
    let query = input.to_ascii_uppercase();
    COMMANDS
        .iter()
        .map(|s| s.to_string())
        .chain(symbols.iter().cloned())
        .filter(|s| s.to_ascii_uppercase().contains(&query))
        .take(8)
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parsing() {
        assert!(matches!(
            parse("cHaRt btcusdt 1H"),
            Ok(Command::Open(Workspace::Chart, _, Some(Timeframe::H1)))
        ));
        assert!(matches!(
            parse("BTCUSDT GP 1H <GO>"),
            Ok(Command::Open(Workspace::Chart, _, Some(Timeframe::H1)))
        ));
        assert!(matches!(
            parse("AAPL FA <GO>"),
            Ok(Command::Open(Workspace::Fin, _, None))
        ));
        assert!(matches!(
            parse("AAPL DES"),
            Ok(Command::Raw(v)) if v == vec!["DES", "AAPL"]
        ));
        assert!(matches!(
            parse("YCRV <GO>"),
            Ok(Command::Raw(v)) if v == vec!["TREASURY"]
        ));
        assert!(matches!(
            parse("CRYPTO ALL <GO>"),
            Ok(Command::Raw(v)) if v == vec!["CRYPTO", "ALL"]
        ));
        assert!(matches!(
            parse("WEI <GO>"),
            Ok(Command::Raw(v)) if v == vec!["WEI"]
        ));
        assert!(matches!(
            parse("SCAN CRYPTO"),
            Ok(Command::Raw(v)) if v == vec!["SCAN", "CRYPTO"]
        ));
        assert!(matches!(
            parse("EQS <GO>"),
            Ok(Command::Raw(v)) if v == vec!["SCAN", "EQUITIES"]
        ));
        assert!(matches!(
            parse("FIN AAPL BALANCE"),
            Ok(Command::Raw(v)) if v == vec!["FIN_TAB", "BALANCE", "AAPL"]
        ));
        assert!(matches!(
            parse("MARKET GAINERS"),
            Ok(Command::Raw(v)) if v == vec!["MARKET", "GAINERS"]
        ));
        assert!(matches!(
            parse("PLOT EVENT"),
            Ok(Command::Raw(v)) if v == vec!["PLOT", "EVENT"]
        ));
        assert!(matches!(
            parse("PLOT"),
            Ok(Command::Raw(v)) if v == vec!["PLOT"]
        ));
        assert_eq!(Workspace::Market.mnemonic(), "MKT");
        assert!(parse("WATCH DELETE BTCUSDT").is_err());
        assert!(parse(" ").is_err());
    }
}
