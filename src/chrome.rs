// Chrome DevTools Protocol client

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
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
