#![forbid(unsafe_code)]
use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use omt_core::{
    AssetId, Company, Worksheet, WorksheetColumn,
    portfolio::{Transaction, positions},
};
use rusqlite::{Connection, params};
use serde::{Serialize, de::DeserializeOwned};
use std::path::Path;
pub struct Store {
    db: Connection,
}
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Self::from_connection(Connection::open(path)?)
    }
    pub fn memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }
    fn from_connection(db: Connection) -> Result<Self> {
        db.busy_timeout(std::time::Duration::from_secs(2))?;
        let version: i64 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version > 1 {
            bail!("Database was created by a newer OMT; refusing to modify it");
        }
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; BEGIN; CREATE TABLE IF NOT EXISTS watchlists(name TEXT PRIMARY KEY); CREATE TABLE IF NOT EXISTS assets(id TEXT PRIMARY KEY,json TEXT NOT NULL); CREATE TABLE IF NOT EXISTS watch_items(list TEXT NOT NULL REFERENCES watchlists(name),asset TEXT NOT NULL REFERENCES assets(id),PRIMARY KEY(list,asset)); CREATE TABLE IF NOT EXISTS worksheets(name TEXT PRIMARY KEY,json TEXT NOT NULL); CREATE TABLE IF NOT EXISTS transactions(id TEXT PRIMARY KEY,account TEXT NOT NULL,asset TEXT NOT NULL,at TEXT NOT NULL,json TEXT NOT NULL); CREATE INDEX IF NOT EXISTS transaction_time ON transactions(account,at); CREATE TABLE IF NOT EXISTS cache(kind TEXT NOT NULL,key TEXT NOT NULL,at TEXT NOT NULL,json TEXT NOT NULL,PRIMARY KEY(kind,key)); CREATE INDEX IF NOT EXISTS cache_expiry ON cache(at); CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY,value TEXT NOT NULL); PRAGMA user_version=1; COMMIT;")?;
        Ok(Self { db })
    }
    pub fn save_worksheet(&self, ws: &Worksheet) -> Result<()> {
        let json = serde_json::to_string(ws)?;
        self.db.execute(
            "INSERT INTO worksheets (name, json) VALUES (?1, ?2) ON CONFLICT(name) DO UPDATE SET json = ?2",
            params![ws.name, json],
        )?;
        self.ensure_watchlist(&ws.name)?;
        for asset in &ws.symbols {
            self.watch(&ws.name, asset, true)?;
        }
        Ok(())
    }
    pub fn load_worksheet(&self, name: &str) -> Result<Option<Worksheet>> {
        let mut stmt = self
            .db
            .prepare("SELECT json FROM worksheets WHERE name = ?1")?;
        let mut rows = stmt.query([name])?;
        if let Some(row) = rows.next()? {
            let json: String = row.get(0)?;
            let ws: Worksheet = serde_json::from_str(&json)?;
            return Ok(Some(ws));
        }
        // Fallback: migrate from legacy watchlists if exists
        let legacy_items = self.watchlist(name)?;
        if !legacy_items.is_empty() || self.watchlists()?.iter().any(|w| w == name) {
            let ws = Worksheet {
                name: name.to_string(),
                columns: WorksheetColumn::default_columns(),
                symbols: legacy_items,
            };
            let _ = self.save_worksheet(&ws);
            return Ok(Some(ws));
        }
        Ok(None)
    }
    pub fn list_worksheets(&self) -> Result<Vec<String>> {
        let mut names = Vec::new();
        let mut stmt = self
            .db
            .prepare("SELECT name FROM worksheets ORDER BY name")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        for name in rows {
            names.push(name?);
        }
        for legacy in self.watchlists()? {
            if !names.contains(&legacy) {
                names.push(legacy);
            }
        }
        names.sort();
        Ok(names)
    }
    pub fn delete_worksheet(&self, name: &str) -> Result<bool> {
        let count = self
            .db
            .execute("DELETE FROM worksheets WHERE name = ?1", [name])?;
        let _ = self
            .db
            .execute("DELETE FROM watch_items WHERE list = ?1", [name]);
        let _ = self
            .db
            .execute("DELETE FROM watchlists WHERE name = ?1", [name]);
        Ok(count > 0)
    }
    pub fn ensure_watchlist(&self, name: &str) -> Result<()> {
        self.db
            .execute("INSERT OR IGNORE INTO watchlists VALUES (?1)", [name])?;
        Ok(())
    }
    pub fn watchlists(&self) -> Result<Vec<String>> {
        Ok(self
            .db
            .prepare("SELECT name FROM watchlists ORDER BY name")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }
    pub fn watch(&self, list: &str, asset: &AssetId, add: bool) -> Result<()> {
        self.ensure_watchlist(list)?;
        if add {
            self.db.execute(
                "INSERT OR IGNORE INTO assets VALUES (?1,?2)",
                params![asset.key(), serde_json::to_string(asset)?],
            )?;
            self.db.execute(
                "INSERT OR IGNORE INTO watch_items VALUES (?1,?2)",
                params![list, asset.key()],
            )?;
        } else {
            self.db.execute(
                "DELETE FROM watch_items WHERE list=?1 AND asset=?2",
                params![list, asset.key()],
            )?;
        }
        Ok(())
    }
    pub fn watchlist(&self, name: &str) -> Result<Vec<AssetId>> {
        let rows=self.db.prepare("SELECT a.json FROM assets a JOIN watch_items w ON a.id=w.asset WHERE w.list=?1 ORDER BY a.id")?.query_map([name],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|s| Ok(serde_json::from_str(&s)?))
            .collect()
    }
    pub fn put<T: Serialize>(&self, kind: &str, key: &str, value: &T) -> Result<()> {
        self.put_at(kind, key, value, Utc::now())
    }
    pub fn put_at<T: Serialize>(
        &self,
        kind: &str,
        key: &str,
        value: &T,
        at: DateTime<Utc>,
    ) -> Result<()> {
        self.db.execute("INSERT INTO cache VALUES (?1,?2,?3,?4) ON CONFLICT(kind,key) DO UPDATE SET at=excluded.at,json=excluded.json",params![kind,key,at.to_rfc3339(),serde_json::to_string(value)?])?;
        Ok(())
    }
    pub fn get<T: DeserializeOwned>(
        &self,
        kind: &str,
        key: &str,
    ) -> Result<Option<(T, DateTime<Utc>)>> {
        let mut statement = self
            .db
            .prepare("SELECT json,at FROM cache WHERE kind=?1 AND key=?2")?;
        let mut rows = statement.query(params![kind, key])?;
        if let Some(row) = rows.next()? {
            let json: String = row.get(0)?;
            let at: String = row.get(1)?;
            return Ok(serde_json::from_str(&json).ok().zip(
                DateTime::parse_from_rfc3339(&at)
                    .ok()
                    .map(|d| d.with_timezone(&Utc)),
            ));
        }
        Ok(None)
    }
    pub fn companies(&self) -> Result<Vec<Company>> {
        let rows = self
            .db
            .prepare("SELECT json FROM cache WHERE kind='company'")?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows
            .into_iter()
            .filter_map(|s| serde_json::from_str(&s).ok())
            .collect())
    }
    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        use rusqlite::OptionalExtension;
        Ok(self
            .db
            .query_row("SELECT value FROM settings WHERE key=?1", [key], |r| {
                r.get(0)
            })
            .optional()?)
    }
    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        self.db.execute("INSERT INTO settings VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,value])?;
        Ok(())
    }
    pub fn transactions(&self) -> Result<Vec<Transaction>> {
        let rows = self
            .db
            .prepare("SELECT json FROM transactions ORDER BY at,rowid")?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|s| {
                serde_json::from_str(&s).context("Invalid financial record; restore from backup")
            })
            .collect()
    }
    pub fn add_transactions(&mut self, items: &[Transaction]) -> Result<()> {
        let mut all = self.transactions()?;
        all.extend_from_slice(items);
        positions(&all)?;
        let tx = self.db.transaction()?;
        for item in items {
            tx.execute(
                "INSERT INTO transactions VALUES (?1,?2,?3,?4,?5)",
                params![
                    item.id,
                    item.account,
                    item.asset.key(),
                    item.at.to_rfc3339(),
                    serde_json::to_string(item)?
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn delete_transaction(&mut self, id: &str) -> Result<()> {
        let mut all = self.transactions()?;
        let before = all.len();
        all.retain(|t| t.id != id);
        if all.len() == before {
            bail!("Transaction '{id}' not found");
        }
        positions(&all)?;
        self.db
            .execute("DELETE FROM transactions WHERE id=?1", [id])?;
        Ok(())
    }
    pub fn export_csv(&self, path: &Path) -> Result<usize> {
        let items = self.transactions()?;
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .context("Export needs a new file (existing files are preserved)")?;
        let mut writer = csv::Writer::from_writer(file);
        writer.write_record([
            "id",
            "account",
            "asset",
            "side",
            "quantity",
            "price",
            "fee",
            "currency",
            "timestamp",
        ])?;
        for t in &items {
            writer.write_record([
                t.id.clone(),
                t.account.clone(),
                t.asset.key(),
                format!("{:?}", t.side),
                t.quantity.to_string(),
                t.price.to_string(),
                t.fee.to_string(),
                t.currency.clone(),
                t.at.to_rfc3339(),
            ])?;
        }
        writer.flush()?;
        Ok(items.len())
    }
    pub fn import_csv(&mut self, path: &Path) -> Result<usize> {
        let mut reader = csv::Reader::from_path(path)?;
        let mut items = Vec::new();
        for row in reader.records() {
            let r = row?;
            if r.len() != 9 {
                bail!("Expected 9 CSV columns");
            }
            items.push(Transaction {
                id: r[0].into(),
                account: r[1].into(),
                asset: r[2].parse().map_err(anyhow::Error::msg)?,
                side: match r[3].to_ascii_lowercase().as_str() {
                    "buy" => omt_core::portfolio::Side::Buy,
                    "sell" => omt_core::portfolio::Side::Sell,
                    _ => bail!("Invalid side"),
                },
                quantity: r[4].parse()?,
                price: r[5].parse()?,
                fee: r[6].parse()?,
                currency: r[7].into(),
                at: DateTime::parse_from_rfc3339(&r[8])?.with_timezone(&Utc),
            });
        }
        self.add_transactions(&items)?;
        Ok(items.len())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persistence_and_cache() {
        let s = Store::memory().unwrap();
        let a = "BTCUSDT".parse().unwrap();
        s.watch("first", &a, true).unwrap();
        s.watch("first", &a, true).unwrap();
        s.watch("second", &a, true).unwrap();
        assert_eq!(s.watchlist("first").unwrap().len(), 1);
        s.watch("first", &a, false).unwrap();
        assert!(s.watchlist("first").unwrap().is_empty());
        assert_eq!(s.watchlist("second").unwrap().len(), 1);
        s.put("bars", "x", &vec![1, 2]).unwrap();
        assert_eq!(
            s.get::<Vec<i32>>("bars", "x").unwrap().unwrap().0,
            vec![1, 2]
        );
        s.db.execute("UPDATE cache SET json='invalid'", []).unwrap();
        assert!(s.get::<Vec<i32>>("bars", "x").unwrap().is_none());
    }

    #[test]
    fn test_worksheet_persistence() {
        let s = Store::memory().unwrap();
        let btc: AssetId = "BTCUSDT".parse().unwrap();
        let eth: AssetId = "ETHUSDT".parse().unwrap();

        let mut ws = Worksheet::new("CRYPTO");
        ws.columns = vec![
            WorksheetColumn::Symbol,
            WorksheetColumn::Last,
            WorksheetColumn::ChangePct,
            WorksheetColumn::Volume,
        ];
        ws.add(btc.clone());
        ws.add(eth.clone());

        s.save_worksheet(&ws).unwrap();

        let loaded = s.load_worksheet("CRYPTO").unwrap().unwrap();
        assert_eq!(loaded.name, "CRYPTO");
        assert_eq!(loaded.symbols.len(), 2);
        assert_eq!(loaded.symbols[0], btc);
        assert_eq!(loaded.symbols[1], eth);
        assert_eq!(loaded.columns.len(), 4);

        let list = s.list_worksheets().unwrap();
        assert!(list.contains(&"CRYPTO".to_string()));

        // Also verifies backwards-compatible watchlist sync
        let legacy_items = s.watchlist("CRYPTO").unwrap();
        assert_eq!(legacy_items.len(), 2);

        // Delete worksheet
        assert!(s.delete_worksheet("CRYPTO").unwrap());
        assert!(s.load_worksheet("CRYPTO").unwrap().is_none());
        assert!(!s.delete_worksheet("CRYPTO").unwrap()); // already deleted
    }
}
