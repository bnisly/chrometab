// Chrome DevTools Protocol client

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio_tungstenite::{connect_async, tungstenite::Message};


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Tab {
    pub title: String,
    pub url: String,
    #[serde(rename = "targetId", alias = "id")]
    pub target_id: String,
    #[serde(rename = "type")]
    pub tab_type: Option<String>,
    #[serde(rename = "browserContextId")]
    pub browser_context_id: Option<String>,
    #[serde(rename = "webSocketDebuggerUrl", default)]
    pub debugger_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CDPCommand {
    id: i32,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct CDPResponse {
    #[allow(dead_code)]
    id: Option<i32>,
    #[serde(default)]
    #[allow(dead_code)]
    result: Option<serde_json::Value>,
    #[serde(default)]
    #[allow(dead_code)]
    error: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionInfo {
    #[serde(rename = "webSocketDebuggerUrl")]
    pub websocket_debugger_url: String,
    #[serde(rename = "Browser", default)]
    pub browser: Option<String>,
}

pub struct ChromeClient {
    pub host: String,
    pub port: u16,
    ws_url: Option<String>,
    version_info: Option<VersionInfo>,
}

impl ChromeClient {
    pub fn new(host: String, port: u16) -> Self {
        Self { host, port, ws_url: None, version_info: None }
    }

    pub async fn get_websocket_url(&mut self) -> Result<String> {
        let url = format!("http://{}:{}/json/version", self.host, self.port);
        let response = reqwest::get(&url)
            .await
            .context("Failed to connect to browser")?;
        let version_info: VersionInfo = response
            .json()
            .await
            .context("Failed to parse version info")?;
        let ws = version_info.websocket_debugger_url.clone();
        self.ws_url = Some(ws.clone());
        self.version_info = Some(version_info);
        Ok(ws)
    }

    pub fn version_info(&self) -> Option<&VersionInfo> {
        self.version_info.as_ref()
    }

    /// Fetch tabs in browser tab-strip order via the REST /json endpoint.
    pub async fn get_tabs(&self, active_only: bool) -> Result<Vec<Tab>> {
        let url = format!("http://{}:{}/json", self.host, self.port);
        let mut tabs: Vec<Tab> = reqwest::get(&url)
            .await
            .context("Failed to connect to browser")?
            .json()
            .await
            .context("Failed to parse tab list")?;
        if active_only {
            tabs.retain(|t| !t.url.starts_with("chrome-extension://"));
        }
        Ok(tabs)
    }

    pub async fn activate_tab(&self, target_id: &str) -> Result<()> {
        let ws_url = self.ws_url.as_ref().context("Not connected to browser")?;
        let (ws_stream, _) = connect_async(ws_url).await?;
        let (mut write, _) = ws_stream.split();
        let command = CDPCommand {
            id: 2,
            method: "Target.activateTarget".to_string(),
            params: Some(serde_json::json!({ "targetId": target_id })),
        };
        write.send(Message::Text(serde_json::to_string(&command)?)).await?;
        Ok(())
    }

    pub async fn close_tab(&self, target_id: &str) -> Result<()> {
        let ws_url = self.ws_url.as_ref().context("Not connected to browser")?;
        let (ws_stream, _) = connect_async(ws_url).await?;
        let (mut write, mut read) = ws_stream.split();
        let command = CDPCommand {
            id: 1,
            method: "Target.closeTarget".to_string(),
            params: Some(serde_json::json!({ "targetId": target_id })),
        };
        write.send(Message::Text(serde_json::to_string(&command)?)).await?;
        let _ = read.next().await.context("No response")??;
        Ok(())
    }

    /// Fetch `performance.timing.navigationStart` (ms since epoch) from a tab's
    /// own WebSocket debugger URL. Returns `None` on any error or unsupported page.
    pub async fn fetch_navigation_start(debugger_url: &str) -> Option<u64> {
        let (ws_stream, _) = connect_async(debugger_url).await.ok()?;
        let (mut write, mut read) = ws_stream.split();

        let command = CDPCommand {
            id: 1,
            method: "Runtime.evaluate".to_string(),
            params: Some(serde_json::json!({
                "expression": "performance.timing.navigationStart",
                "returnByValue": true,
            })),
        };
        write
            .send(Message::Text(serde_json::to_string(&command).ok()?))
            .await
            .ok()?;

        let msg = match tokio::time::timeout(
            std::time::Duration::from_secs(2),
            read.next(),
        )
        .await
        {
            Ok(Some(Ok(m))) => m,
            _ => return None,
        };

        let text = msg.to_text().ok()?;
        let v: serde_json::Value = serde_json::from_str(text).ok()?;
        let nav_start = v["result"]["result"]["value"].as_f64()?;
        if nav_start > 0.0 {
            Some(nav_start as u64)
        } else {
            None
        }
    }

    /// Fetch ages for all tabs in parallel. Returns a map of target_id → age.
    pub async fn fetch_all_ages(tabs: &[Tab]) -> HashMap<String, std::time::Duration> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let futures: Vec<_> = tabs
            .iter()
            .filter_map(|tab| {
                tab.debugger_url.as_ref().map(|url| {
                    let target_id = tab.target_id.clone();
                    let url = url.clone();
                    async move {
                        let nav_start =
                            ChromeClient::fetch_navigation_start(&url).await?;
                        if nav_start > 0 && now_ms >= nav_start {
                            Some((
                                target_id,
                                std::time::Duration::from_millis(now_ms - nav_start),
                            ))
                        } else {
                            None
                        }
                    }
                })
            })
            .collect();

        futures_util::future::join_all(futures)
            .await
            .into_iter()
            .flatten()
            .collect()
    }

    pub async fn open_new_tab(&self, url: &str) -> Result<()> {
        let ws_url = self.ws_url.as_ref().context("Not connected to browser")?;
        let (ws_stream, _) = connect_async(ws_url).await?;
        let (mut write, mut read) = ws_stream.split();
        let command = CDPCommand {
            id: 1,
            method: "Target.createTarget".to_string(),
            params: Some(serde_json::json!({ "url": url })),
        };
        write.send(Message::Text(serde_json::to_string(&command)?)).await?;
        let response = read.next().await.context("No response")??;
        let _: CDPResponse = serde_json::from_str(response.to_text()?)?;
        Ok(())
    }
}
