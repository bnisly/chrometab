// File: src/main.rs
// Author: Hadi Cahyadi <cumulus13@gmail.com>
// Description: A powerful CLI tool for managing Chrome tabs via Chrome DevTools Protocol
// License: MIT

mod chrome;
mod groups;
mod platform;
mod tui;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use regex::Regex;
use std::collections::HashMap;
use std::io::{self, Write};

use chrome::{ChromeClient, Tab};
use platform::{bring_browser_to_front, resolve_browser, BrowserKind};

// ============================================================================
// CLI Arguments
// ============================================================================

#[derive(Parser)]
#[command(
    name = "chrometab",
    version = "1.1.0",
    author = "Hadi Cahyadi <cumulus13@gmail.com>",
    about = "Manage Chrome tabs via Chrome DevTools Protocol"
)]
struct Cli {
    /// Pattern to match tab titles or URLs
    pattern: Option<String>,

    /// List all available tabs and exit
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

    /// Browser for window activation (auto = detect from CDP version)
    #[arg(long, default_value = "auto", value_parser = ["chrome", "brave", "auto"], env = "CHROMETAB_BROWSER")]
    browser: String,

    /// Enable debug output
    #[arg(long)]
    debug: bool,

    /// Launch TUI mode (also auto-detected when run interactively)
    #[arg(long)]
    tui: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run as WebSocket server for remote control
    Serve {
        #[arg(short = 'H', long, default_value = "localhost")]
        host: String,
        #[arg(short = 'P', long, default_value = "8765")]
        port: u16,
    },
    /// Send pattern to WebSocket server
    Client {
        pattern: String,
        #[arg(short = 'H', long, default_value = "localhost")]
        host: String,
        #[arg(short = 'P', long, default_value = "8765")]
        port: u16,
    },
}

// ============================================================================
// Text-mode UI helpers
// ============================================================================

fn pattern_match(text: &str, pattern: &str) -> bool {
    if pattern.is_empty() {
        return true;
    }
    let regex_pattern = pattern.replace('*', ".*");
    match Regex::new(&format!("(?i){}", regex_pattern)) {
        Ok(r) => r.is_match(text),
        Err(_) => false,
    }
}

fn get_terminal_width() -> usize {
    crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80)
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
        let title = if tab.title.is_empty() { &tab.url } else { &tab.title };
        let num_str = format!("{:0width$}", idx + 1, width = num_width);

        if matches.len() == 1 {
            print!("{}", title);
        } else {
            print!("{}. {}", num_str, title);
        }

        if show_url && !tab.url.is_empty() {
            let decoded = urlencoding::decode(&tab.url)
                .unwrap_or(std::borrow::Cow::Borrowed(&tab.url));
            let max_len = term_width.saturating_sub(title.len() + 8);
            let url_str = if decoded.len() > max_len {
                format!("{}...", &decoded[..max_len.min(decoded.len())])
            } else {
                decoded.to_string()
            };
            print!(" || {}", url_str);
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

fn print_duplicate_tabs(
    duplicates: &HashMap<String, Vec<usize>>,
    tabs: &[Tab],
    show_url: bool,
) -> Vec<usize> {
    if duplicates.is_empty() {
        println!("No duplicate tabs found.");
        return Vec::new();
    }

    println!("Found {} URLs with duplicate tabs:\n", duplicates.len());

    let mut all_matches = Vec::new();
    let mut group_num = 1;

    for (url, indices) in duplicates {
        println!("Group {}: {} tabs with same URL", group_num, indices.len());

        if show_url {
            let decoded =
                urlencoding::decode(url).unwrap_or(std::borrow::Cow::Borrowed(url));
            println!("URL: {}", decoded);
        }
        println!();

        for &idx in indices {
            let tab = &tabs[idx];
            let match_idx = all_matches.len() + 1;
            println!("  {:02}. {}", match_idx, tab.title);
            all_matches.push(idx);
        }

        println!();
        group_num += 1;
    }

    all_matches
}

fn read_input(prompt: &str) -> String {
    print!("{}", prompt);
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
            if let (Ok(s), Ok(e)) =
                (start.trim().parse::<usize>(), end.trim().parse::<usize>())
            {
                numbers.extend(s..=e);
            }
        } else if let Ok(n) = part.parse::<usize>() {
            numbers.push(n);
        }
    }
    numbers
}

// ============================================================================
// Text interactive loop
// ============================================================================

fn interactive_mode<'a>(
    client: &'a ChromeClient,
    tabs: &'a [Tab],
    pattern: Option<&'a str>,
    show_url: bool,
    find_duplicate: bool,
    browser: BrowserKind,
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
                            println!("Exit...");
                            std::process::exit(0);
                        }
                        "s" => {
                            return interactive_mode(
                                client, tabs, pattern, !show_url, find_duplicate, browser,
                            )
                            .await;
                        }
                        "r" => return Ok(()),
                        "u" => {
                            let url = read_input("URL: ");
                            if !url.is_empty() && url != "q" {
                                client.open_new_tab(&url).await?;
                                println!("New tab opened: {}", url);
                            }
                        }
                        _ if choice.ends_with('c') => {
                            let nums_str = &choice[..choice.len() - 1];
                            let numbers = parse_numbers(nums_str);
                            for num in numbers {
                                if num > 0 && num <= matches.len() {
                                    let tab_idx = matches[num - 1];
                                    client.close_tab(&tabs[tab_idx].target_id).await?;
                                    println!("Tab closed.");
                                }
                            }
                        }
                        _ => {
                            if let Ok(num) = choice.parse::<usize>() {
                                if num > 0 && num <= matches.len() {
                                    let tab_idx = matches[num - 1];
                                    client.activate_tab(&tabs[tab_idx].target_id).await?;
                                    bring_browser_to_front(browser)?;
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
                bring_browser_to_front(browser)?;
            } else if !matches.is_empty() {
                loop {
                    let choice = read_input(
                        "Select number, '[n,n1-nx]c' = close, 'u' = new tab, 's' = show URL, 'r' = refresh, 'fd' = find duplicates [q = quit]: ",
                    );
                    match choice.as_str() {
                        "q" | "x" | "quit" | "exit" => {
                            println!("Exit...");
                            std::process::exit(0);
                        }
                        "s" => {
                            return interactive_mode(
                                client, tabs, pattern, !show_url, false, browser,
                            )
                            .await;
                        }
                        "r" => return Ok(()),
                        "fd" => {
                            return interactive_mode(
                                client, tabs, None, show_url, true, browser,
                            )
                            .await;
                        }
                        "u" => {
                            let url = read_input("URL: ");
                            if !url.is_empty() && url != "q" {
                                client.open_new_tab(&url).await?;
                                println!("New tab opened: {}", url);
                            }
                        }
                        _ if choice.ends_with('c') => {
                            let nums_str = &choice[..choice.len() - 1];
                            let numbers = parse_numbers(nums_str);
                            for num in numbers {
                                if num > 0 && num <= matches.len() {
                                    let tab_idx = matches[num - 1];
                                    client.close_tab(&tabs[tab_idx].target_id).await?;
                                    println!("Tab closed.");
                                }
                            }
                        }
                        _ => {
                            if let Ok(num) = choice.parse::<usize>() {
                                if num > 0 && num <= matches.len() {
                                    let tab_idx = matches[num - 1];
                                    client.activate_tab(&tabs[tab_idx].target_id).await?;
                                    bring_browser_to_front(browser)?;
                                    break;
                                }
                            } else {
                                return interactive_mode(
                                    client, tabs, Some(&choice), false, false, browser,
                                )
                                .await;
                            }
                        }
                    }
                }
            } else {
                println!("No matching tabs found.");
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
    client
        .get_websocket_url()
        .await
        .context("Failed to connect to Chrome. Is Chrome running with --remote-debugging-port=9222?")?;

    // Open a URL and exit
    if let Some(url) = &cli.url {
        client.open_new_tab(url).await?;
        println!("New tab opened: {}", url);
        return Ok(());
    }

    // Subcommands
    if let Some(command) = cli.command {
        match command {
            Commands::Serve { host, port } => {
                println!("WebSocket server mode not yet implemented");
                println!("Server would run at ws://{}:{}", host, port);
            }
            Commands::Client { pattern, host, port } => {
                println!("WebSocket client mode not yet implemented");
                println!(
                    "Would connect to ws://{}:{} with pattern: {}",
                    host, port, pattern
                );
            }
        }
        return Ok(());
    }

    let resolved_browser = resolve_browser(&cli.browser, client.version_info());

    // Decide between TUI and text mode.
    // TUI is used when: --tui is passed, OR stdout is a TTY and no specific
    // text-mode flags are given.
    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stdout());
    let use_tui = cli.tui
        || (is_tty
            && !cli.list
            && cli.pattern.is_none()
            && !cli.find_duplicate);

    if use_tui {
        tui::run(&client, resolved_browser).await?;
        return Ok(());
    }

    // --list: print all tabs and exit
    if cli.list {
        let tabs = client.get_tabs(cli.active_only).await?;
        print_tabs(&tabs, None, cli.show_url);
        return Ok(());
    }

    // Text interactive loop
    loop {
        let tabs = client.get_tabs(cli.active_only).await?;

        let result = interactive_mode(
            &client,
            &tabs,
            cli.pattern.as_deref(),
            cli.show_url,
            cli.find_duplicate,
            resolved_browser,
        )
        .await;

        if result.is_err() || cli.pattern.is_some() {
            break;
        }

        // Refresh connection for next iteration
        client.get_websocket_url().await?;
    }

    Ok(())
}
