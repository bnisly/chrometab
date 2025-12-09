// File: src\main.rs
// Author: Hadi Cahyadi <cumulus13@gmail.com>
// Date: 2025-12-10
// Description: A powerful CLI tool for managing Chrome tabs via Chrome DevTools Protocol
// License: MIT

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::*;
use futures_util::{SinkExt, StreamExt};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, Write};
use tokio_tungstenite::{connect_async, tungstenite::Message};

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Tab {
    title: String,
    url: String,
    #[serde(rename = "targetId")]
    target_id: String,
    #[serde(rename = "type")]
    tab_type: Option<String>,
    #[serde(rename = "browserContextId")]
    browser_context_id: Option<String>,
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
    result: Option<serde_json::Value>,
    #[serde(default)]
    #[allow(dead_code)]
    error: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct VersionInfo {
    #[serde(rename = "webSocketDebuggerUrl")]
    websocket_debugger_url: String,
}

#[derive(Debug, Deserialize)]
struct TargetsResult {
    #[serde(rename = "targetInfos")]
    target_infos: Vec<Tab>,
}

// ============================================================================
// CLI Arguments
// ============================================================================

#[derive(Parser)]
#[command(
    name = "chrometab",
    version = "1.0.0",
    author = "Hadi Cahyadi <cumulus13@gmail.com>",
    about = "Manage Chrome tabs via Chrome DevTools Protocol"
)]
struct Cli {
    /// Pattern to match tab titles or URLs
    pattern: Option<String>,

    /// List all available tabs
    #[arg(short, long)]
    list: bool,

    /// Show only active tabs (exclude chrome-extension://)
    #[arg(short, long)]
    active_only: bool,

    /// Show URLs alongside tab titles
    #[arg(short, long)]
    show_url: bool,

    /// Find and list tabs with duplicate URLs
    #[arg(long)]
    find_duplicate: bool,

    /// Open a new tab with the specified URL
    #[arg(short, long)]
    url: Option<String>,

    /// Force platform for window activation (windows/linux/darwin)
    #[arg(short, long)]
    force: Option<String>,

    /// Chrome remote debugging host
    #[arg(long, default_value = "localhost")]
    host: String,

    /// Chrome remote debugging port
    #[arg(long, default_value = "9222")]
    port: u16,

    /// Enable debug output
    #[arg(long)]
    debug: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run as WebSocket server for remote control
    Serve {
        /// WebSocket server host
        #[arg(short = 'H', long, default_value = "localhost")]
        host: String,

        /// WebSocket server port
        #[arg(short = 'P', long, default_value = "8765")]
        port: u16,
    },
    /// Send pattern to WebSocket server
    Client {
        /// Pattern to search for
        pattern: String,

        /// WebSocket server host
        #[arg(short = 'H', long, default_value = "localhost")]
        host: String,

        /// WebSocket server port
        #[arg(short = 'P', long, default_value = "8765")]
        port: u16,
    },
}

// ============================================================================
// Chrome DevTools Protocol Client
// ============================================================================

struct ChromeClient {
    host: String,
    port: u16,
    ws_url: Option<String>,
}

impl ChromeClient {
    fn new(host: String, port: u16) -> Self {
        Self {
            host,
            port,
            ws_url: None,
        }
    }

    async fn get_websocket_url(&mut self) -> Result<String> {
        let url = format!("http://{}:{}/json/version", self.host, self.port);
        println!("{} \"{}\"", "Chrome Server:".bright_cyan(), url);

        let response = reqwest::get(&url)
            .await
            .context("Failed to connect to Chrome")?;

        let version_info: VersionInfo = response
            .json()
            .await
            .context("Failed to parse version info")?;

        self.ws_url = Some(version_info.websocket_debugger_url.clone());
        Ok(version_info.websocket_debugger_url)
    }

    async fn get_tabs(&self, active_only: bool) -> Result<Vec<Tab>> {
        let ws_url = self
            .ws_url
            .as_ref()
            .context("WebSocket URL not initialized")?;

        let (ws_stream, _) = connect_async(ws_url)
            .await
            .context("Failed to connect to WebSocket")?;

        let (mut write, mut read) = ws_stream.split();

        let command = CDPCommand {
            id: 1,
            method: "Target.getTargets".to_string(),
            params: None,
        };

        write
            .send(Message::Text(serde_json::to_string(&command)?))
            .await?;

        let response = read.next().await.context("No response from Chrome")??;
        let cdp_response: CDPResponse = serde_json::from_str(response.to_text()?)?;

        let targets: TargetsResult = serde_json::from_value(
            cdp_response
                .result
                .context("No result in CDP response")?
                .clone(),
        )?;

        let mut tabs = targets.target_infos;

        if active_only {
            tabs.retain(|tab| !tab.url.starts_with("chrome-extension://"));
        }

        Ok(tabs)
    }

    async fn activate_tab(&self, target_id: &str) -> Result<()> {
        let ws_url = self
            .ws_url
            .as_ref()
            .context("WebSocket URL not initialized")?;

        let (ws_stream, _) = connect_async(ws_url).await?;
        let (mut write, _read) = ws_stream.split();

        let command = CDPCommand {
            id: 2,
            method: "Target.activateTarget".to_string(),
            params: Some(serde_json::json!({
                "targetId": target_id
            })),
        };

        write
            .send(Message::Text(serde_json::to_string(&command)?))
            .await?;

        Ok(())
    }

    async fn close_tab(&self, target_id: &str) -> Result<()> {
        let ws_url = self
            .ws_url
            .as_ref()
            .context("WebSocket URL not initialized")?;

        let (ws_stream, _) = connect_async(ws_url).await?;
        let (mut write, mut read) = ws_stream.split();

        let command = CDPCommand {
            id: 1,
            method: "Target.closeTarget".to_string(),
            params: Some(serde_json::json!({
                "targetId": target_id
            })),
        };

        write
            .send(Message::Text(serde_json::to_string(&command)?))
            .await?;

        let _response = read.next().await.context("No response")??;
        println!(
            "{}",
            format!("Tab {} closed successfully", target_id).bright_cyan()
        );

        Ok(())
    }

    async fn open_new_tab(&self, url: &str) -> Result<()> {
        let ws_url = self
            .ws_url
            .as_ref()
            .context("WebSocket URL not initialized")?;

        let (ws_stream, _) = connect_async(ws_url).await?;
        let (mut write, mut read) = ws_stream.split();

        let command = CDPCommand {
            id: 1,
            method: "Target.createTarget".to_string(),
            params: Some(serde_json::json!({
                "url": url
            })),
        };

        write
            .send(Message::Text(serde_json::to_string(&command)?))
            .await?;

        let response = read.next().await.context("No response")??;
        let cdp_response: CDPResponse = serde_json::from_str(response.to_text()?)?;

        if cdp_response.result.is_some() {
            println!("{}", format!("New tab opened: {}", url).bright_cyan());
        }

        Ok(())
    }
}

// ============================================================================
// Window Management
// ============================================================================

#[cfg(target_os = "windows")]
fn bring_chrome_to_front() -> Result<()> {
    use winapi::shared::windef::HWND;
    use winapi::um::winuser::*;

    unsafe {
        let mut chrome_hwnd: HWND = std::ptr::null_mut();

        unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: isize) -> i32 {
            let mut text: [u16; 512] = [0; 512];
            let len = GetWindowTextW(hwnd, text.as_mut_ptr(), 512);

            if len > 0 {
                let title = String::from_utf16_lossy(&text[..len as usize]);
                if title.contains("Google Chrome") || title.contains("Chrome") {
                    if IsWindowVisible(hwnd) != 0 {
                        *(lparam as *mut HWND) = hwnd;
                        return 0; // Stop enumeration
                    }
                }
            }
            1 // Continue enumeration
        }

        EnumWindows(
            Some(enum_windows_callback),
            &mut chrome_hwnd as *mut _ as isize,
        );

        if !chrome_hwnd.is_null() {
            ShowWindow(chrome_hwnd, SW_RESTORE);
            ShowWindow(chrome_hwnd, SW_SHOW);
            SetForegroundWindow(chrome_hwnd);
            println!("{}", "Chrome window brought to front".bright_cyan());
            Ok(())
        } else {
            println!("{}", "Chrome window not found".yellow());
            Ok(())
        }
    }
}

#[cfg(target_os = "macos")]
fn bring_chrome_to_front() -> Result<()> {
    std::process::Command::new("osascript")
        .args(&["-e", "tell application \"Google Chrome\" to activate"])
        .output()?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn bring_chrome_to_front() -> Result<()> {
    std::process::Command::new("xdotool")
        .args(&["search", "--name", "Google Chrome", "windowactivate"])
        .output()?;
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn bring_chrome_to_front() -> Result<()> {
    println!("{}", "Platform not supported for window activation".yellow());
    Ok(())
}

// ============================================================================
// UI Functions
// ============================================================================

fn pattern_match(text: &str, pattern: &str) -> bool {
    if pattern.is_empty() {
        return true;
    }

    let regex_pattern = pattern.replace("*", ".*");
    let regex = match Regex::new(&format!("(?i){}", regex_pattern)) {
        Ok(r) => r,
        Err(_) => return false,
    };

    regex.is_match(text)
}

fn get_terminal_width() -> usize {
    use terminal_size::{terminal_size, Width};
    
    if let Some((Width(w), _)) = terminal_size() {
        w as usize
    } else {
        80
    }
}

fn print_tabs(tabs: &[Tab], pattern: Option<&str>, show_url: bool) -> Vec<usize> {
    let mut matches = Vec::new();

    for (i, tab) in tabs.iter().enumerate() {
        if let Some(pat) = pattern {
            if !pattern_match(&tab.title, pat) && !pattern_match(&tab.url, pat) {
                continue;
            }
        }
        matches.push(i);
    }

    if matches.is_empty() {
        return matches;
    }

    let term_width = get_terminal_width();
    let num_width = matches.len().to_string().len();

    for (idx, &tab_idx) in matches.iter().enumerate() {
        let tab = &tabs[tab_idx];
        let title = if tab.title.is_empty() {
            &tab.url
        } else {
            &tab.title
        };

        let num_str = format!("{:0width$}", idx + 1, width = num_width);

        if matches.len() == 1 {
            print!("{}", title.bright_yellow());
        } else {
            print!("{}. {}", num_str.bright_cyan(), title.bright_yellow());
        }

        if show_url && !tab.url.is_empty() {
            let decoded = urlencoding::decode(&tab.url).unwrap_or(std::borrow::Cow::Borrowed(&tab.url));
            let max_len = term_width.saturating_sub(title.len() + 8);
            let url_str = if decoded.len() > max_len {
                format!("{}...", &decoded[..max_len.min(decoded.len())])
            } else {
                decoded.to_string()
            };
            print!(" || {}", url_str.bright_blue());
        }

        println!();
    }

    matches
}

fn find_duplicate_tabs(tabs: &[Tab]) -> HashMap<String, Vec<usize>> {
    let mut url_map: HashMap<String, Vec<usize>> = HashMap::new();

    for (i, tab) in tabs.iter().enumerate() {
        url_map.entry(tab.url.clone()).or_default().push(i);
    }

    url_map.into_iter().filter(|(_, v)| v.len() > 1).collect()
}

fn print_duplicate_tabs(duplicates: &HashMap<String, Vec<usize>>, tabs: &[Tab], show_url: bool) -> Vec<usize> {
    if duplicates.is_empty() {
        println!("{}", "No duplicate tabs found.".yellow());
        return Vec::new();
    }

    println!(
        "{}\n",
        format!("Found {} URLs with duplicate tabs:", duplicates.len()).bright_red()
    );

    let mut all_matches = Vec::new();
    let mut group_num = 1;

    for (url, indices) in duplicates {
        println!(
            "{} {}",
            format!("Group {}:", group_num).bright_cyan(),
            format!("{} tabs with same URL", indices.len()).white()
        );

        if show_url {
            let decoded = urlencoding::decode(url).unwrap_or(std::borrow::Cow::Borrowed(url));
            println!("{} {}", "URL:".white(), decoded);
        }
        println!();

        for &idx in indices {
            let tab = &tabs[idx];
            let match_idx = all_matches.len() + 1;
            println!(
                "  {}. {}",
                format!("{:02}", match_idx).bright_cyan(),
                tab.title.bright_yellow()
            );
            all_matches.push(idx);
        }

        println!();
        group_num += 1;
    }

    all_matches
}

fn read_input(prompt: &str) -> String {
    print!("{}", prompt.bright_cyan());
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().to_string()
}

fn parse_numbers(input: &str) -> Vec<usize> {
    let mut numbers = Vec::new();

    for part in input.split(',') {
        let part = part.trim();
        if let Some((start, end)) = part.split_once('-') {
            if let (Ok(s), Ok(e)) = (start.trim().parse::<usize>(), end.trim().parse::<usize>()) {
                numbers.extend(s..=e);
            }
        } else if let Ok(n) = part.parse::<usize>() {
            numbers.push(n);
        }
    }

    numbers
}

// ============================================================================
// Main Logic
// ============================================================================

fn interactive_mode<'a>(
    client: &'a ChromeClient,
    tabs: &'a [Tab],
    pattern: Option<&'a str>,
    show_url: bool,
    find_duplicate: bool,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + 'a>> {
    Box::pin(async move {
        if find_duplicate {
            let duplicates = find_duplicate_tabs(tabs);
            let matches = print_duplicate_tabs(&duplicates, tabs, show_url);

            if !matches.is_empty() {
                loop {
                    let choice = read_input(
                        "Select tab number, '[n,n1-nx]c' = close, 'u' = new tab, 's' = show URL, 'r' = refresh [q = quit]: ",
                    );

                    match choice.as_str() {
                        "q" | "x" | "quit" | "exit" => {
                            println!("{}", "Exit...".bright_red());
                            std::process::exit(0);
                        }
                        "s" => {
                            return interactive_mode(client, tabs, pattern, !show_url, find_duplicate)
                                .await;
                        }
                        "r" => return Ok(()),
                        "u" => {
                            let url = read_input("URL: ");
                            if !url.is_empty() && url != "q" {
                                client.open_new_tab(&url).await?;
                            }
                        }
                        _ if choice.ends_with('c') => {
                            let nums_str = &choice[..choice.len() - 1];
                            let numbers = parse_numbers(nums_str);
                            for num in numbers {
                                if num > 0 && num <= matches.len() {
                                    let tab_idx = matches[num - 1];
                                    client.close_tab(&tabs[tab_idx].target_id).await?;
                                }
                            }
                        }
                        _ => {
                            if let Ok(num) = choice.parse::<usize>() {
                                if num > 0 && num <= matches.len() {
                                    let tab_idx = matches[num - 1];
                                    client.activate_tab(&tabs[tab_idx].target_id).await?;
                                    bring_chrome_to_front()?;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        } else {
            let matches = print_tabs(tabs, pattern, show_url);

            if matches.len() == 1 {
                let tab = &tabs[matches[0]];
                client.activate_tab(&tab.target_id).await?;
                bring_chrome_to_front()?;
            } else if !matches.is_empty() {
                loop {
                    let choice = read_input(
                        "Select number, '[n,n1-nx]c' = close, 'u' = new tab, 's' = show URL, 'r' = refresh, 'fd' = find duplicates [q = quit]: ",
                    );

                    match choice.as_str() {
                        "q" | "x" | "quit" | "exit" => {
                            println!("{}", "Exit...".bright_red());
                            std::process::exit(0);
                        }
                        "s" => {
                            return interactive_mode(client, tabs, pattern, !show_url, false).await;
                        }
                        "r" => return Ok(()),
                        "fd" => {
                            return interactive_mode(client, tabs, None, show_url, true).await;
                        }
                        "u" => {
                            let url = read_input("URL: ");
                            if !url.is_empty() && url != "q" {
                                client.open_new_tab(&url).await?;
                            }
                        }
                        _ if choice.ends_with('c') => {
                            let nums_str = &choice[..choice.len() - 1];
                            let numbers = parse_numbers(nums_str);
                            for num in numbers {
                                if num > 0 && num <= matches.len() {
                                    let tab_idx = matches[num - 1];
                                    client.close_tab(&tabs[tab_idx].target_id).await?;
                                }
                            }
                        }
                        _ => {
                            if let Ok(num) = choice.parse::<usize>() {
                                if num > 0 && num <= matches.len() {
                                    let tab_idx = matches[num - 1];
                                    client.activate_tab(&tabs[tab_idx].target_id).await?;
                                    bring_chrome_to_front()?;
                                    break;
                                }
                            } else {
                                return interactive_mode(client, tabs, Some(&choice), false, false)
                                    .await;
                            }
                        }
                    }
                }
            } else {
                println!("{}", "No matching tabs found.".yellow());
            }
        }

        Ok(())
    })
}

// ============================================================================
// Main Entry Point
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut client = ChromeClient::new(cli.host.clone(), cli.port);
    client.get_websocket_url().await?;

    // Handle URL opening
    if let Some(url) = &cli.url {
        client.open_new_tab(url).await?;
        return Ok(());
    }

    // Handle subcommands (serve/client)
    if let Some(command) = cli.command {
        match command {
            Commands::Serve { host, port } => {
                println!("WebSocket server mode not yet implemented");
                println!("Server would run at ws://{}:{}", host, port);
            }
            Commands::Client {
                pattern,
                host,
                port,
            } => {
                println!("WebSocket client mode not yet implemented");
                println!("Would connect to ws://{}:{} with pattern: {}", host, port, pattern);
            }
        }
        return Ok(());
    }

    // Main interactive mode
    loop {
        let tabs = client.get_tabs(cli.active_only).await?;

        let result = interactive_mode(
            &client,
            &tabs,
            cli.pattern.as_deref(),
            cli.show_url,
            cli.find_duplicate,
        )
        .await;

        if result.is_err() || cli.pattern.is_some() {
            break;
        }

        // Refresh tabs for next iteration
        client.get_websocket_url().await?;
    }

    Ok(())
}