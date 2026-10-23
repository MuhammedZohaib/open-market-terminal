#![forbid(unsafe_code)]
pub mod market;
pub mod research;
use anyhow::{Result, bail};
use reqwest::Client;
use serde_json::Value;
use std::{collections::HashMap, sync::Arc, time::Duration};
#[derive(Clone)]
pub struct Http {
    pub client: Client,
    cooldowns: Arc<tokio::sync::Mutex<HashMap<String, tokio::time::Instant>>>,
}
impl Http {
    pub fn new() -> Result<Self> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        Ok(Self {
            cooldowns: Arc::default(),
            client: Client::builder()
                .timeout(Duration::from_secs(15))
                .connect_timeout(Duration::from_secs(8))
                .user_agent("OpenMarketTerminal/0.1")
                .build()?,
        })
    }
    pub async fn json(&self, url: &str) -> Result<Value> {
        self.request(self.client.get(url)).await
    }
    pub async fn text(&self, url: &str) -> Result<String> {
        self.request_text(self.client.get(url)).await
    }
    pub async fn request_text(&self, request: reqwest::RequestBuilder) -> Result<String> {
        let request = request
            .build()
            .map_err(|_| anyhow::anyhow!("Invalid provider request"))?;
        let host = request.url().host_str().unwrap_or("unknown").to_string();
        let deadline = self.cooldowns.lock().await.get(&host).copied();
        if let Some(deadline) = deadline {
            tokio::time::sleep_until(deadline).await;
        }
        let response = self
            .client
            .execute(request)
            .await
            .map_err(|_| anyhow::anyhow!("Network request failed (connection or timeout)"))?;
        let status = response.status();
        if status.as_u16() == 429 || status.as_u16() == 418 {
            self.cooldowns
                .lock()
                .await
                .insert(host, tokio::time::Instant::now() + Duration::from_secs(60));
            bail!("Provider rate limit; retry after 60s");
        }
        if !status.is_success() {
            bail!("Provider HTTP {status}");
        }
        Ok(response.text().await?)
    }
    pub async fn request(&self, request: reqwest::RequestBuilder) -> Result<Value> {
        let request = request
            .build()
            .map_err(|_| anyhow::anyhow!("Invalid provider request"))?;
        let host = request.url().host_str().unwrap_or("unknown").to_string();
        let deadline = self.cooldowns.lock().await.get(&host).copied();
        if let Some(deadline) = deadline {
            tokio::time::sleep_until(deadline).await;
        }
        let response = self
            .client
            .execute(request)
            .await
            .map_err(|_| anyhow::anyhow!("Network request failed (connection or timeout)"))?;
        let status = response.status();
        if status.as_u16() == 429 || status.as_u16() == 418 {
            let retry = response
                .headers()
                .get("retry-after")
                .and_then(|s| s.to_str().ok())
                .unwrap_or("60");
            let seconds = retry
                .parse::<u64>()
                .ok()
                .or_else(|| {
                    chrono::DateTime::parse_from_rfc2822(retry).ok().map(|d| {
                        (d.with_timezone(&chrono::Utc) - chrono::Utc::now())
                            .num_seconds()
                            .max(1) as u64
                    })
                })
                .unwrap_or(60);
            self.cooldowns.lock().await.insert(
                host,
                tokio::time::Instant::now() + Duration::from_secs(seconds.min(86400)),
            );
            bail!("Provider rate limit; retry after {seconds}s");
        }
        if !status.is_success() {
            bail!("Provider HTTP {status}");
        }
        Ok(response.json().await?)
    }
}
