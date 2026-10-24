//! Official, dated research sources. No financial estimates are substituted.
use crate::{Http, market::decimal};
use anyhow::{Context, Result, bail};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use omt_core::{
    Company, Filing, FinancialFact, MacroObservation, NewsCategory, NewsItem, clean_html_to_text,
    extract_article_content, summarize_sec_filing_document,
};
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;
#[derive(Clone)]
pub struct Sec {
    http: Http,
    user_agent: String,
    gate: Arc<Mutex<tokio::time::Instant>>,
}
impl Sec {
    pub fn new(http: Http, contact: &str) -> Result<Self> {
        if !contact.contains('@') || contact.contains("example.") || contact.len() < 8 {
            bail!("Set OMT_SEC_USER_AGENT to your application name and maintained contact email");
        }
        Ok(Self {
            http,
            user_agent: contact.into(),
            gate: Arc::new(Mutex::new(tokio::time::Instant::now())),
        })
    }
    async fn get(&self, url: &str) -> Result<Value> {
        let mut gate = self.gate.lock().await;
        tokio::time::sleep_until(*gate).await;
        *gate = tokio::time::Instant::now() + Duration::from_millis(250);
        let result = self
            .http
            .request(
                self.http
                    .client
                    .get(url)
                    .header("User-Agent", &self.user_agent)
                    .header("Accept-Encoding", "identity"),
            )
            .await;
        drop(gate);
        result
    }
    pub async fn tickers(&self) -> Result<Value> {
        self.get("https://www.sec.gov/files/company_tickers.json")
            .await
    }
    pub fn resolve(mapping: &Value, ticker: &str) -> Result<(u64, String)> {
        let row = mapping
            .as_object()
            .context("Invalid ticker mapping")?
            .values()
            .find(|r| {
                r["ticker"]
                    .as_str()
                    .is_some_and(|s| s.eq_ignore_ascii_case(ticker))
            })
            .context("Ticker not found in SEC mapping")?;
        Ok((
            row["cik_str"].as_u64().context("Invalid CIK")?,
            row["title"].as_str().unwrap_or(ticker).into(),
        ))
    }
    pub async fn company(&self, ticker: &str, mapping: &Value) -> Result<Company> {
        let (cik, name) = Self::resolve(mapping, ticker)?;
        let facts = self
            .get(&format!(
                "https://data.sec.gov/api/xbrl/companyfacts/CIK{cik:010}.json"
            ))
            .await?;
        let submissions = self
            .get(&format!(
                "https://data.sec.gov/submissions/CIK{cik:010}.json"
            ))
            .await?;
        Ok(Company {
            ticker: ticker.to_ascii_uppercase(),
            cik,
            name,
            facts: parse_facts(&facts, cik)?,
            filings: parse_filings(&submissions, cik)?,
            fetched_at: Utc::now(),
        })
    }
}
fn date(v: &Value) -> Result<NaiveDate> {
    Ok(NaiveDate::parse_from_str(
        v.as_str().context("Missing date")?,
        "%Y-%m-%d",
    )?)
}
pub fn parse_facts(v: &Value, cik: u64) -> Result<Vec<FinancialFact>> {
    let gaap = &v["facts"]["us-gaap"];
    if !gaap.is_object() {
        bail!("SEC US GAAP facts unavailable");
    }
    let metrics: [(&str, &[&str]); 29] = [
        (
            "Revenue",
            &[
                "RevenueFromContractWithCustomerExcludingAssessedTax",
                "Revenues",
                "SalesRevenueNet",
            ],
        ),
        (
            "Cost of revenue",
            &[
                "CostOfGoodsAndServicesSold",
                "CostOfRevenue",
                "CostOfGoodsSold",
            ],
        ),
        ("Gross profit", &["GrossProfit"]),
        ("R&D expense", &["ResearchAndDevelopmentExpense"]),
        ("SG&A expense", &["SellingGeneralAndAdministrativeExpense"]),
        ("Operating income", &["OperatingIncomeLoss"]),
        (
            "Pretax income",
            &[
                "IncomeLossFromContinuingOperationsBeforeIncomeTaxesExtraordinaryItemsNoncontrollingInterest",
                "IncomeLossFromContinuingOperationsBeforeIncomeTaxesMinorityInterestAndIncomeLossFromEquityMethodInvestments",
            ],
        ),
        ("Income tax", &["IncomeTaxExpenseBenefit"]),
        ("Net income", &["NetIncomeLoss"]),
        (
            "Operating cash flow",
            &["NetCashProvidedByUsedInOperatingActivities"],
        ),
        (
            "Depreciation & amort",
            &[
                "DepreciationDepletionAndAmortization",
                "DepreciationAndAmortization",
            ],
        ),
        ("CapEx", &["PaymentsToAcquirePropertyPlantAndEquipment"]),
        (
            "Investing cash flow",
            &["NetCashProvidedByUsedInInvestingActivities"],
        ),
        ("Share repurchases", &["PaymentsForRepurchaseOfCommonStock"]),
        (
            "Dividends paid",
            &["PaymentsOfDividendsCommonStock", "PaymentsOfDividends"],
        ),
        (
            "Financing cash flow",
            &["NetCashProvidedByUsedInFinancingActivities"],
        ),
        ("Cash", &["CashAndCashEquivalentsAtCarryingValue"]),
        (
            "Short-term investments",
            &[
                "MarketableSecuritiesCurrent",
                "AvailableForSaleSecuritiesDebtSecuritiesCurrent",
            ],
        ),
        ("Accounts receivable", &["AccountsReceivableNetCurrent"]),
        ("Inventory", &["InventoryNet"]),
        ("Current assets", &["AssetsCurrent"]),
        ("Property & equipment", &["PropertyPlantAndEquipmentNet"]),
        ("Total assets", &["Assets"]),
        ("Accounts payable", &["AccountsPayableCurrent"]),
        ("Current liabilities", &["LiabilitiesCurrent"]),
        (
            "Long-term debt",
            &["LongTermDebtNoncurrent", "LongTermDebt"],
        ),
        ("Total liabilities", &["Liabilities"]),
        ("Retained earnings", &["RetainedEarningsAccumulatedDeficit"]),
        (
            "Equity",
            &[
                "StockholdersEquity",
                "StockholdersEquityIncludingPortionAttributableToNoncontrollingInterest",
            ],
        ),
    ];
    let mut result = Vec::new();
    for (metric, concepts) in metrics {
        let mut period_seen = std::collections::HashSet::new();
        for concept in concepts {
            if let Some(rows) = gaap[*concept]["units"]["USD"].as_array() {
                let mut exact_seen = std::collections::HashSet::new();
                for row in rows {
                    let parsed = (|| -> Result<FinancialFact> {
                        let start = if row.get("start").is_some() {
                            Some(date(&row["start"])?)
                        } else {
                            None
                        };
                        let end = date(&row["end"])?;
                        let filed = date(&row["filed"])?;
                        if start.is_some_and(|s| s > end) || filed < end {
                            bail!("Incompatible fiscal dates");
                        }
                        let accession = row["accn"]
                            .as_str()
                            .context("Missing accession")?
                            .to_string();
                        let form = row["form"].as_str().context("Missing form")?.to_string();
                        Ok(FinancialFact {
                            metric: metric.into(),
                            concept: (*concept).into(),
                            value: decimal(&row["val"])?,
                            unit: "USD".into(),
                            start,
                            end,
                            filed,
                            source: format!(
                                "https://www.sec.gov/Archives/edgar/data/{cik}/{}/",
                                accession.replace('-', "")
                            ),
                            accession,
                            form,
                        })
                    })();
                    if let Ok(fact) = parsed {
                        let period_key = (fact.start, fact.end);
                        let exact_key = (
                            fact.start,
                            fact.end,
                            fact.filed,
                            fact.accession.clone(),
                            fact.value,
                        );
                        if !period_seen.contains(&period_key) && exact_seen.insert(exact_key) {
                            result.push(fact);
                        }
                    }
                }
                for fact in result.iter().filter(|f| f.metric == metric) {
                    period_seen.insert((fact.start, fact.end));
                }
            }
        }
    }
    Ok(result)
}
fn default_form_description(form: &str) -> &'static str {
    match form {
        "10-K" => "Annual Report (Form 10-K)",
        "10-K/A" => "Amended Annual Report (10-K/A)",
        "10-Q" => "Quarterly Report (Form 10-Q)",
        "10-Q/A" => "Amended Quarterly Report (10-Q/A)",
        "8-K" => "Current Report / Material Event (8-K)",
        "8-K/A" => "Amended Current Report (8-K/A)",
        "DEF 14A" => "Definitive Proxy Statement",
        "DEFA14A" => "Additional Proxy Soliciting Materials",
        "20-F" => "Foreign Private Issuer Annual Report",
        "6-K" => "Foreign Issuer Current Report",
        "S-8" => "Employee Benefit Plan Registration",
        "S-3" | "S-3ASR" => "Shelf Registration Statement",
        "424B2" => "Prospectus Supplement (Debt/Notes)",
        "SD" => "Specialized Disclosure Report",
        "SCHEDULE 13G" | "SC 13G" => "Beneficial Ownership Report (13G)",
        "SCHEDULE 13G/A" | "SC 13G/A" => "Amended Beneficial Ownership (13G/A)",
        "PX14A6G" => "Notice of Exempt Solicitation",
        "4" | "4/A" => "Statement of Changes in Beneficial Ownership",
        "3" | "3/A" => "Initial Statement of Beneficial Ownership",
        "144" | "144/A" => "Notice of Proposed Sale of Securities",
        _ => "SEC EDGAR Regulatory Filing",
    }
}
pub fn parse_filings(v: &Value, cik: u64) -> Result<Vec<Filing>> {
    let r = &v["filings"]["recent"];
    let forms = r["form"].as_array().context("Missing recent filings")?;
    let mut result = Vec::new();
    let allowed = [
        "10-K",
        "10-K/A",
        "10-Q",
        "10-Q/A",
        "8-K",
        "8-K/A",
        "DEF 14A",
        "DEFA14A",
        "20-F",
        "6-K",
        "S-1",
        "S-3",
        "S-3ASR",
        "S-4",
        "S-8",
        "424B2",
        "SD",
        "SCHEDULE 13G",
        "SCHEDULE 13G/A",
        "SC 13G",
        "SC 13G/A",
        "SC 13D",
        "SC 13D/A",
        "PX14A6G",
        "3",
        "4",
        "144",
    ];
    for (i, form) in forms.iter().enumerate() {
        let form = form.as_str().unwrap_or("").trim();
        if !allowed.contains(&form) {
            continue;
        }
        let accession = r["accessionNumber"][i]
            .as_str()
            .context("Missing accession")?
            .to_string();
        let raw_doc = r["primaryDocument"][i]
            .as_str()
            .context("Missing document")?
            .trim();
        if raw_doc.is_empty()
            || raw_doc.contains("..")
            || raw_doc.starts_with('/')
            || raw_doc.contains("//")
            || raw_doc.matches('/').count() > 1
            || !raw_doc
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_./".contains(c))
        {
            continue;
        }
        let display_doc = raw_doc.rsplit('/').next().unwrap_or(raw_doc).to_string();
        let report_date = r
            .get("reportDate")
            .and_then(|arr| arr.get(i))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
        let raw_desc = r
            .get("primaryDocDescription")
            .and_then(|arr| arr.get(i))
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        let description = if raw_desc.is_empty() || raw_desc.eq_ignore_ascii_case(form) {
            default_form_description(form).to_string()
        } else {
            raw_desc.to_string()
        };
        result.push(Filing {
            form: form.into(),
            filed: date(&r["filingDate"][i])?,
            report_date,
            description,
            url: format!(
                "https://www.sec.gov/Archives/edgar/data/{cik}/{}/{raw_doc}",
                accession.replace('-', "")
            ),
            accession,
            document: display_doc,
        });
    }
    result.sort_by(|a, b| b.filed.cmp(&a.filed).then(b.accession.cmp(&a.accession)));
    result.truncate(250);
    Ok(result)
}
pub async fn world_bank(http: &Http, country: &str, series: &str) -> Result<Vec<MacroObservation>> {
    if !country.chars().all(|c| c.is_ascii_alphabetic())
        || !series
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.')
    {
        bail!("Invalid World Bank country or series");
    }
    let v=http.json(&format!("https://api.worldbank.org/v2/country/{country}/indicator/{series}?format=json&per_page=100")).await?;
    parse_world_bank(&v, series)
}
pub fn parse_world_bank(v: &Value, series: &str) -> Result<Vec<MacroObservation>> {
    let rows = v[1].as_array().context("World Bank series unavailable")?;
    let known_unit = match series {
        "NY.GDP.MKTP.CD" => "current USD",
        "NY.GDP.MKTP.KD.ZG" | "FP.CPI.TOTL.ZG" => "annual %",
        "SL.UEM.TOTL.ZS" => "% of labor force",
        _ => "see indicator metadata",
    };
    let mut result = Vec::new();
    for row in rows {
        if row["value"].is_null() {
            continue;
        }
        result.push(MacroObservation {
            series: series.into(),
            label: row["indicator"]["value"].as_str().unwrap_or(series).into(),
            value: decimal(&row["value"])?,
            unit: row["unit"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or(known_unit)
                .into(),
            geography: row["country"]["value"].as_str().unwrap_or("Unknown").into(),
            date: row["date"]
                .as_str()
                .context("Missing observation date")?
                .into(),
            frequency: "Annual".into(),
            source: "World Bank Indicators API".into(),
            updated: v[0]["lastupdated"].as_str().map(str::to_owned),
        });
    }
    Ok(result)
}
pub async fn treasury(http: &Http) -> Result<Vec<MacroObservation>> {
    let url = format!(
        "https://home.treasury.gov/resource-center/data-chart-center/interest-rates/pages/xml?data=daily_treasury_yield_curve&field_tdr_date_value={}",
        Utc::now().year()
    );
    let response = http.client.get(url).send().await?.error_for_status()?;
    parse_treasury(&response.text().await?)
}
pub fn parse_treasury(xml: &str) -> Result<Vec<MacroObservation>> {
    let doc = roxmltree::Document::parse(xml)?;
    let mut all = Vec::new();
    for properties in doc
        .descendants()
        .filter(|n| n.tag_name().name() == "properties")
    {
        let date = properties
            .children()
            .find(|n| n.tag_name().name() == "NEW_DATE")
            .and_then(|n| n.text())
            .unwrap_or("");
        if date.len() < 10 {
            continue;
        }
        for node in properties
            .children()
            .filter(|n| omt_core::treasury_tenor_months(n.tag_name().name()).is_some())
        {
            if let Some(text) = node.text()
                && let Ok(value) = text.parse()
            {
                all.push(MacroObservation {
                    series: node.tag_name().name().into(),
                    label: node
                        .tag_name()
                        .name()
                        .replace("BC_", "")
                        .replace("1_5", "1.5"),
                    value,
                    unit: "percent per annum".into(),
                    geography: "United States".into(),
                    date: date[..10].into(),
                    frequency: "Daily published par yield".into(),
                    source: "US Treasury".into(),
                    updated: None,
                });
            }
        }
    }
    let latest = all.iter().map(|r| r.date.clone()).max();
    all.retain(|r| Some(&r.date) == latest.as_ref());
    all.sort_by(|a, b| {
        omt_core::treasury_tenor_months(&a.series)
            .unwrap_or_default()
            .total_cmp(&omt_core::treasury_tenor_months(&b.series).unwrap_or_default())
    });
    if all.is_empty() {
        bail!("Treasury yield curve unavailable");
    }
    Ok(all)
}
pub async fn fred(http: &Http, series: &str) -> Result<Vec<MacroObservation>> {
    let key = std::env::var("FRED_API_KEY").context("Set FRED_API_KEY to your own FRED key")?;
    if !series
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        bail!("Invalid FRED series");
    }
    let metadata = http
        .request(
            http.client
                .get("https://api.stlouisfed.org/fred/series")
                .query(&[
                    ("series_id", series),
                    ("api_key", &key),
                    ("file_type", "json"),
                ]),
        )
        .await?;
    let m = &metadata["seriess"][0];
    let v = http
        .request(
            http.client
                .get("https://api.stlouisfed.org/fred/series/observations")
                .query(&[
                    ("series_id", series),
                    ("api_key", &key),
                    ("file_type", "json"),
                    ("sort_order", "desc"),
                    ("limit", "100"),
                ]),
        )
        .await?;
    let mut result = Vec::new();
    for row in v["observations"]
        .as_array()
        .context("Missing FRED observations")?
    {
        if row["value"] == "." {
            continue;
        }
        result.push(MacroObservation {
            series: series.into(),
            label: m["title"].as_str().unwrap_or(series).into(),
            value: decimal(&row["value"])?,
            unit: m["units"].as_str().unwrap_or("Unavailable").into(),
            geography: "See FRED series metadata".into(),
            date: row["date"].as_str().unwrap_or("Unknown").into(),
            frequency: m["frequency"].as_str().unwrap_or("Unknown").into(),
            source: "FRED / original publisher in series metadata".into(),
            updated: m["last_updated"].as_str().map(str::to_owned),
        });
    }
    Ok(result)
}
fn parse_pub_date(raw: &str) -> Option<DateTime<Utc>> {
    let s = raw.trim();
    if let Ok(dt) = DateTime::parse_from_rfc2822(s) {
        return Some(dt.with_timezone(&Utc));
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    for fmt in [
        "%a, %d %b %Y %H:%M:%S GMT",
        "%a, %d %b %Y %H:%M:%S UTC",
        "%Y-%m-%dT%H:%M:%SZ",
        "%Y-%m-%d %H:%M:%S",
    ] {
        if let Ok(ndt) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
            return Some(ndt.and_utc());
        }
    }
    if s.len() >= 10
        && let Ok(nd) = NaiveDate::parse_from_str(&s[..10], "%Y-%m-%d")
    {
        return nd.and_hms_opt(12, 0, 0).map(|dt| dt.and_utc());
    }
    None
}
fn infer_news_ticker(headline: &str, default_ticker: Option<&str>) -> Option<String> {
    if let Some(t) = default_ticker {
        return Some(t.to_ascii_uppercase());
    }
    let up = headline.to_ascii_uppercase();
    for (kw, sym) in [
        ("APPLE", "AAPL"),
        ("AAPL", "AAPL"),
        ("MICROSOFT", "MSFT"),
        ("MSFT", "MSFT"),
        ("NVIDIA", "NVDA"),
        ("NVDA", "NVDA"),
        ("TESLA", "TSLA"),
        ("TSLA", "TSLA"),
        ("AMAZON", "AMZN"),
        ("AMZN", "AMZN"),
        ("ALPHABET", "GOOGL"),
        ("GOOGLE", "GOOGL"),
        ("META ", "META"),
        ("BITCOIN", "BTC"),
        ("BTC", "BTC"),
        ("ETHEREUM", "ETH"),
        ("ETH ", "ETH"),
        ("SOLANA", "SOL"),
        ("BINANCE", "BNB"),
        ("TREASURY", "UST"),
        ("FEDERAL RESERVE", "FED"),
        ("FOMC", "FED"),
    ] {
        if up.contains(kw) {
            return Some(sym.into());
        }
    }
    None
}
fn refine_category(headline: &str, summary: &str, base: NewsCategory) -> NewsCategory {
    let combined = format!("{headline} {summary}").to_ascii_uppercase();
    if combined.contains("10-K") || combined.contains("10-Q") || combined.contains("8-K") {
        return NewsCategory::Filings;
    }
    if combined.contains("EARNINGS")
        || combined.contains("EPS ")
        || combined.contains("QUARTERLY RESULTS")
        || combined.contains("REVENUE BEAT")
        || combined.contains("GUIDANCE")
    {
        return NewsCategory::Earnings;
    }
    if combined.contains("BITCOIN")
        || combined.contains("ETHEREUM")
        || combined.contains("CRYPTO")
        || combined.contains("SOLANA")
        || combined.contains("STABLECOIN")
    {
        return NewsCategory::Crypto;
    }
    if combined.contains("FEDERAL RESERVE")
        || combined.contains("INTEREST RATE")
        || combined.contains("INFLATION")
        || combined.contains("TREASURY YIELD")
        || combined.contains("CPI ")
        || combined.contains("GDP ")
        || combined.contains("CENTRAL BANK")
    {
        return NewsCategory::Macro;
    }
    base
}
pub fn parse_rss_or_atom(
    xml: &str,
    source_name: &str,
    default_category: NewsCategory,
    default_ticker: Option<&str>,
) -> Result<Vec<NewsItem>> {
    let sanitized = xml
        .replace("&nbsp;", " ")
        .replace("&mdash;", "—")
        .replace("&ndash;", "–")
        .replace("&rsquo;", "'")
        .replace("&lsquo;", "'")
        .replace("&ldquo;", "\"")
        .replace("&rdquo;", "\"");
    let doc = roxmltree::Document::parse(&sanitized).context("Malformed RSS/Atom XML")?;
    let fetched_at = Utc::now();
    let mut items = Vec::new();
    for node in doc
        .descendants()
        .filter(|n| matches!(n.tag_name().name(), "item" | "entry"))
    {
        let child_text = |names: &[&str]| -> Option<String> {
            node.children()
                .find(|c| names.contains(&c.tag_name().name()))
                .and_then(|c| c.text())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        };
        let Some(raw_title) = child_text(&["title"]) else {
            continue;
        };
        let headline = clean_html_to_text(&raw_title)
            .lines()
            .next()
            .unwrap_or(&raw_title)
            .trim()
            .to_string();
        if headline.is_empty() {
            continue;
        }
        let link = child_text(&["link"])
            .or_else(|| {
                node.children()
                    .find(|c| c.tag_name().name() == "link")
                    .and_then(|c| c.attribute("href"))
                    .map(|s| s.trim().to_string())
            })
            .unwrap_or_default();
        if !link.starts_with("https://") && !link.starts_with("http://") {
            continue;
        }
        let raw_desc =
            child_text(&["encoded", "content", "description", "summary"]).unwrap_or_default();
        let summary_clean = clean_html_to_text(&raw_desc);
        let summary = if summary_clean.is_empty() {
            format!(
                "Published by {source_name}. Select and press 'r' to fetch article text or 'o' to open original publisher page."
            )
        } else {
            summary_clean
        };
        let published_at = child_text(&["pubDate", "published", "updated", "date"])
            .and_then(|d| parse_pub_date(&d))
            .unwrap_or(fetched_at);
        let guid = child_text(&["guid", "id"]).unwrap_or_else(|| link.clone());
        let category = refine_category(&headline, &summary, default_category);
        let ticker = infer_news_ticker(&headline, default_ticker);
        items.push(NewsItem {
            id: format!("{source_name}:{guid}"),
            published_at,
            fetched_at,
            source: source_name.to_string(),
            ticker,
            category,
            headline,
            summary,
            body: None,
            url: link,
            is_sec_filing: false,
            form: None,
            accession: None,
            report_date: None,
        });
    }
    Ok(items)
}
pub fn dedup_news(items: Vec<NewsItem>) -> Vec<NewsItem> {
    let mut seen_urls = std::collections::HashSet::new();
    let mut seen_titles = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let title_key: String = item
            .headline
            .to_ascii_lowercase()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .take(64)
            .collect();
        if title_key.is_empty() {
            continue;
        }
        let url_ok = item.url.is_empty() || seen_urls.insert(item.url.clone());
        let title_ok = seen_titles.insert(title_key);
        if url_ok && title_ok {
            out.push(item);
        }
    }
    out.sort_by(|a, b| {
        b.published_at
            .cmp(&a.published_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    out.truncate(300);
    out
}
pub async fn fetch_news(
    http: &Http,
    ticker: Option<&str>,
    category: NewsCategory,
) -> Result<Vec<NewsItem>> {
    let mut feeds: Vec<(&str, String, NewsCategory, Option<String>)> = Vec::new();
    if let Some(sym) = ticker {
        let clean_sym = sym
            .rsplit(':')
            .next()
            .unwrap_or(sym)
            .trim()
            .to_ascii_uppercase();
        let yahoo_sym = if clean_sym.ends_with("USDT") || clean_sym.ends_with("USDC") {
            let base = clean_sym.trim_end_matches("USDT").trim_end_matches("USDC");
            format!("{base}-USD")
        } else {
            clean_sym.clone()
        };
        let cat =
            if yahoo_sym.ends_with("-USD") || matches!(clean_sym.as_str(), "BTC" | "ETH" | "SOL") {
                NewsCategory::Crypto
            } else {
                NewsCategory::Equities
            };
        feeds.push((
            "Yahoo Finance",
            format!(
                "https://feeds.finance.yahoo.com/rss/2.0/headline?s={yahoo_sym}&region=US&lang=en-US"
            ),
            cat,
            Some(clean_sym.trim_end_matches("USDT").to_string()),
        ));
    }
    match category {
        NewsCategory::Crypto => {
            feeds.push((
                "CoinDesk",
                "https://www.coindesk.com/arc/outboundfeeds/rss/".into(),
                NewsCategory::Crypto,
                None,
            ));
            feeds.push((
                "Cointelegraph",
                "https://cointelegraph.com/rss".into(),
                NewsCategory::Crypto,
                None,
            ));
        }
        NewsCategory::Macro => {
            feeds.push((
                "Federal Reserve",
                "https://www.federalreserve.gov/feeds/press_all.xml".into(),
                NewsCategory::Macro,
                Some("FED".into()),
            ));
            feeds.push((
                "CNBC Economy",
                "https://search.cnbc.com/rs/search/combinedcms/view.xml?partnerId=wrss01&id=20910258".into(),
                NewsCategory::Macro,
                None,
            ));
        }
        NewsCategory::Earnings => {
            feeds.push((
                "CNBC Earnings",
                "https://search.cnbc.com/rs/search/combinedcms/view.xml?partnerId=wrss01&id=15839135".into(),
                NewsCategory::Earnings,
                None,
            ));
        }
        _ => {
            feeds.push((
                "CNBC Markets",
                "https://search.cnbc.com/rs/search/combinedcms/view.xml?partnerId=wrss01&id=10000664".into(),
                NewsCategory::Markets,
                None,
            ));
            feeds.push((
                "CNBC Economy",
                "https://search.cnbc.com/rs/search/combinedcms/view.xml?partnerId=wrss01&id=20910258".into(),
                NewsCategory::Macro,
                None,
            ));
            feeds.push((
                "CNBC Earnings",
                "https://search.cnbc.com/rs/search/combinedcms/view.xml?partnerId=wrss01&id=15839135".into(),
                NewsCategory::Earnings,
                None,
            ));
            feeds.push((
                "Federal Reserve",
                "https://www.federalreserve.gov/feeds/press_all.xml".into(),
                NewsCategory::Macro,
                Some("FED".into()),
            ));
            feeds.push((
                "CoinDesk",
                "https://www.coindesk.com/arc/outboundfeeds/rss/".into(),
                NewsCategory::Crypto,
                None,
            ));
        }
    }
    let mut all = Vec::new();
    for (src, url, cat, def_ticker) in feeds {
        if let Ok(xml) = http.text(&url).await
            && let Ok(parsed) = parse_rss_or_atom(&xml, src, cat, def_ticker.as_deref())
        {
            all.extend(parsed);
        }
    }
    if all.is_empty() {
        bail!("No news feeds reachable; displaying cached headlines and SEC filings");
    }
    Ok(dedup_news(all))
}
pub async fn fetch_article_or_filing_body(
    http: &Http,
    sec_user_agent: Option<&str>,
    url: &str,
    is_sec_filing: bool,
    form: Option<&str>,
    description: &str,
) -> Result<String> {
    if !url.starts_with("https://") {
        bail!("Only HTTPS publisher and SEC EDGAR URLs are permitted");
    }
    let req = if url.contains("sec.gov/") {
        let ua = sec_user_agent
            .filter(|s| s.contains('@') && !s.contains("example."))
            .context("Set OMT_SEC_USER_AGENT to fetch SEC EDGAR filing body text")?;
        http.client
            .get(url)
            .header("User-Agent", ua)
            .header("Accept-Encoding", "identity")
    } else {
        http.client
            .get(url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
            )
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
            )
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Accept-Encoding", "identity")
            .header(
                "Sec-Ch-Ua",
                "\"Chromium\";v=\"130\", \"Google Chrome\";v=\"130\", \"Not?A_Brand\";v=\"99\"",
            )
            .header("Sec-Ch-Ua-Mobile", "?0")
            .header("Sec-Ch-Ua-Platform", "\"Windows\"")
            .header("Sec-Fetch-Dest", "document")
            .header("Sec-Fetch-Mode", "navigate")
            .header("Sec-Fetch-Site", "none")
            .header("Sec-Fetch-User", "?1")
            .header("Upgrade-Insecure-Requests", "1")
    };
    let raw = match http.request_text(req).await {
        Ok(text) => text,
        Err(e) => {
            if !description.trim().is_empty() {
                return Ok(format!(
                    "{description}\n\n[Publisher connection: {e}. Press 'o' to open original article in browser.]\n\nCanonical Source: {url}"
                ));
            }
            return Err(e);
        }
    };
    if is_sec_filing || url.contains("sec.gov/Archives/edgar/") {
        return Ok(summarize_sec_filing_document(
            form.unwrap_or("SEC"),
            description,
            &raw,
        ));
    }
    let extracted = extract_article_content(&raw, url);
    if extracted.is_excerpt
        && extracted
            .body
            .starts_with("Full-text programmatic extraction restricted")
        && !description.trim().is_empty()
    {
        return Ok(format!(
            "{description}\n\n[Publisher restricts full programmatic extraction. Press 'o' to open in browser.]\n\nCanonical Source: {url}"
        ));
    }
    Ok(extracted.body)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_fields_are_not_zero() {
        let v = serde_json::json!({"facts":{"us-gaap":{"Assets":{"units":{"USD":[{"end":"2024-12-31","filed":"2025-02-01","accn":"0001-25-000001","form":"10-K"}]}}}}});
        assert!(parse_facts(&v, 1).unwrap().is_empty());
    }
    #[test]
    fn no_fake_sec_contact() {
        assert!(Sec::new(Http::new().unwrap(), "OMT user@example.org").is_err());
    }
    #[test]
    fn treasury_negative_and_null() {
        let xml = "<feed xmlns:d='d'><properties><d:NEW_DATE>2025-01-01T00:00:00</d:NEW_DATE><d:BC_1MONTH>-0.25</d:BC_1MONTH><d:BC_2MONTH/></properties></feed>";
        let rows = parse_treasury(xml).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].value.to_string(), "-0.25");
    }
    #[test]
    fn sec_filings_freshness_sorting_and_xsl_paths() {
        let sub = serde_json::json!({
            "filings": {
                "recent": {
                    "form": ["8-K", "10-Q", "DEF 14A", "4", "8-K"],
                    "filingDate": ["2026-01-02", "2026-01-30", "2026-01-08", "2026-04-02", "2026-02-24"],
                    "reportDate": ["2026-01-02", "2025-12-27", "2026-02-24", "2026-04-01", "2026-02-24"],
                    "accessionNumber": [
                        "0000320193-26-000001",
                        "0000320193-26-000006",
                        "0001193125-26-005000",
                        "0000320193-26-000025",
                        "0000320193-26-000015"
                    ],
                    "primaryDocument": [
                        "aapl-20260102.htm",
                        "aapl-20251227.htm",
                        "d123456ddef14a.htm",
                        "xslF345X06/wk-form4_1743633000.xml",
                        "../malicious.htm"
                    ],
                    "primaryDocDescription": [
                        "8-K",
                        "10-Q",
                        "DEF 14A",
                        "FORM 4",
                        "8-K"
                    ]
                }
            }
        });
        let rows = parse_filings(&sub, 320193).unwrap();
        // "../malicious.htm" is rejected; the other 4 valid filings are kept and sorted newest-first
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].form, "4");
        assert_eq!(rows[0].filed.to_string(), "2026-04-02");
        assert_eq!(rows[0].document, "wk-form4_1743633000.xml");
        assert!(rows[0].url.ends_with("/xslF345X06/wk-form4_1743633000.xml"));
        assert_eq!(rows[1].form, "10-Q");
        assert_eq!(rows[1].filed.to_string(), "2026-01-30");
        assert_eq!(rows[1].report_date.unwrap().to_string(), "2025-12-27");
        assert_eq!(rows[2].form, "DEF 14A");
        assert_eq!(rows[3].filed.to_string(), "2026-01-02");
    }
}
