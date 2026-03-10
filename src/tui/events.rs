// Key event handling for TUI mode

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::chrome::ChromeClient;
use super::{App, ExportDialog, ExportFormat, ExportStep, Panel};

pub async fn handle_key(app: &mut App, client: &ChromeClient, key: KeyEvent) -> Result<()> {
    // Help overlay: any key closes it
    if app.show_help {
        app.show_help = false;
        return Ok(());
    }

    // Export dialog
    if app.export_dialog.is_some() {
        return handle_export_key(app, client, key).await;
    }

    // Confirm close dialog
    if app.confirm_close {
        return handle_confirm_close_key(app, client, key).await;
    }

    // Filter mode
    if app.filter_mode {
        handle_filter_key(app, key);
        return Ok(());
    }

    handle_normal_key(app, client, key).await
}

async fn handle_normal_key(
    app: &mut App,
    client: &ChromeClient,
    key: KeyEvent,
) -> Result<()> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => {
            app.should_quit = true;
        }
        KeyCode::Esc => {
            if !app.filter.is_empty() {
                app.filter.clear();
                app.selected_tab = 0;
                app.tab_list_state.select(Some(0));
            } else {
                app.should_quit = true;
            }
        }
        KeyCode::Char('?') => {
            app.show_help = true;
        }

        // Panel focus (Tab/Shift-Tab; no effect in flat mode — there's no groups panel)
        KeyCode::Tab | KeyCode::BackTab => {
            if app.view_mode == super::ViewMode::Grouped {
                app.focus = match app.focus {
                    Panel::Groups => Panel::Tabs,
                    Panel::Tabs => Panel::Groups,
                };
            }
        }

        // Navigation — page scroll (PageUp/PageDown and Ctrl-B/Ctrl-F)
        KeyCode::PageUp | KeyCode::Char('b')
            if key.modifiers.contains(KeyModifiers::CONTROL) =>
        {
            let step = page_size();
            match app.focus {
                Panel::Groups => {
                    app.selected_group = app.selected_group.saturating_sub(step);
                    app.group_list_state.select(Some(app.selected_group));
                    app.selected_tab = 0;
                    app.tab_list_state.select(Some(0));
                }
                Panel::Tabs => {
                    app.selected_tab = app.selected_tab.saturating_sub(step);
                    app.tab_list_state.select(Some(app.selected_tab));
                }
            }
        }
        KeyCode::PageDown | KeyCode::Char('f')
            if key.modifiers.contains(KeyModifiers::CONTROL) =>
        {
            let step = page_size();
            match app.focus {
                Panel::Groups => {
                    let max = app.display_groups.len().saturating_sub(1);
                    app.selected_group = (app.selected_group + step).min(max);
                    app.group_list_state.select(Some(app.selected_group));
                    app.selected_tab = 0;
                    app.tab_list_state.select(Some(0));
                }
                Panel::Tabs => {
                    let max = app.current_group_tab_count().saturating_sub(1);
                    app.selected_tab = (app.selected_tab + step).min(max);
                    app.tab_list_state.select(Some(app.selected_tab));
                }
            }
        }

        // Navigation — line by line
        KeyCode::Up | KeyCode::Char('k') => match app.focus {
            Panel::Groups => {
                if app.selected_group > 0 {
                    app.selected_group -= 1;
                    app.group_list_state.select(Some(app.selected_group));
                    app.selected_tab = 0;
                    app.tab_list_state.select(Some(0));
                }
            }
            Panel::Tabs => {
                if app.selected_tab > 0 {
                    app.selected_tab -= 1;
                    app.tab_list_state.select(Some(app.selected_tab));
                }
            }
        },
        KeyCode::Down | KeyCode::Char('j') => match app.focus {
            Panel::Groups => {
                if app.selected_group + 1 < app.display_groups.len() {
                    app.selected_group += 1;
                    app.group_list_state.select(Some(app.selected_group));
                    app.selected_tab = 0;
                    app.tab_list_state.select(Some(0));
                }
            }
            Panel::Tabs => {
                let count = app.current_group_tab_count();
                if app.selected_tab + 1 < count {
                    app.selected_tab += 1;
                    app.tab_list_state.select(Some(app.selected_tab));
                }
            }
        },
        KeyCode::Left | KeyCode::Char('h') => {
            if app.focus == Panel::Tabs {
                app.focus = Panel::Groups;
            }
        }
        KeyCode::Right | KeyCode::Char('l') => {
            if app.focus == Panel::Groups {
                app.focus = Panel::Tabs;
            }
        }

        // Selection
        KeyCode::Char(' ') => {
            app.toggle_select_current();
        }
        KeyCode::Char('a') => {
            let before = app.selected_tab_ids.len();
            app.select_all_in_group();
            let after = app.selected_tab_ids.len();
            if after < before {
                app.status_message = Some("Deselected group tabs".to_string());
            } else {
                app.status_message = Some(format!("Selected {} tabs in group", after));
            }
        }
        KeyCode::Char('A') => {
            let before = app.selected_tab_ids.len();
            app.select_all();
            let after = app.selected_tab_ids.len();
            if after < before {
                app.status_message = Some("Deselected all tabs".to_string());
            } else {
                app.status_message = Some(format!("Selected all {} tabs", after));
            }
        }

        // Activate tab (makes it the active tab in the browser; no window-focus change
        // in TUI mode to avoid accidentally launching a different browser)
        KeyCode::Enter => {
            if let Some(tab) = app.selected_tab_in_group().map(|t| t.target_id.clone()) {
                match client.activate_tab(&tab).await {
                    Ok(_) => {
                        app.status_message = Some("Tab activated".to_string());
                    }
                    Err(e) => {
                        app.status_message = Some(format!("Error: {}", e));
                    }
                }
            }
        }

        // Close selected tabs
        KeyCode::Char('d') | KeyCode::Char('D') => {
            if app.selected_tab_ids.is_empty() {
                // Select current tab and close
                app.toggle_select_current();
            }
            if !app.selected_tab_ids.is_empty() {
                app.confirm_close = true;
            }
        }

        // Bookmark / export
        KeyCode::Char('b') | KeyCode::Char('B') => {
            use crate::tui::export::get_default_export_path;
            app.export_dialog = Some(ExportDialog {
                step: ExportStep::PickFormat,
                format: ExportFormat::NetscapeHtml,
                path: get_default_export_path("html"),
            });
        }

        // Filter
        KeyCode::Char('/') => {
            app.filter_mode = true;
            app.status_message = None;
        }

        // Toggle view mode (grouped ↔ flat/appearance order)
        KeyCode::Char('v') | KeyCode::Char('V') => {
            app.toggle_view_mode();
            app.status_message = Some(format!("View: {}", app.view_mode.label()));
        }

        // Refresh
        KeyCode::Char('r') | KeyCode::Char('R') => {
            match client.get_tabs(false).await {
                Ok(tabs) => {
                    let count = tabs.len();
                    app.refresh(tabs);
                    app.status_message = Some(format!("Refreshed — {} tabs", count));
                }
                Err(e) => {
                    app.status_message = Some(format!("Refresh failed: {}", e));
                }
            }
        }

        // Not implemented
        KeyCode::Char('g') => {
            app.status_message = Some("Manual grouping: select tabs, press g to assign name (coming soon)".to_string());
        }

        _ => {}
    }

    // Clear status on navigation (but not on selection changes)
    Ok(())
}

fn handle_filter_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Enter => {
            app.filter_mode = false;
        }
        KeyCode::Backspace => {
            app.filter.pop();
            app.selected_tab = 0;
            app.tab_list_state.select(Some(0));
        }
        KeyCode::Char(c) if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT => {
            app.filter.push(c);
            app.selected_tab = 0;
            app.tab_list_state.select(Some(0));
        }
        _ => {}
    }
}

async fn handle_confirm_close_key(
    app: &mut App,
    client: &ChromeClient,
    key: KeyEvent,
) -> Result<()> {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
            app.confirm_close = false;
            let ids: Vec<String> = app.selected_tab_ids.drain().collect();
            let mut closed = 0usize;
            let mut errors = 0usize;
            for id in &ids {
                match client.close_tab(id).await {
                    Ok(_) => closed += 1,
                    Err(_) => errors += 1,
                }
            }
            // Refresh after closing
            match client.get_tabs(false).await {
                Ok(tabs) => app.refresh(tabs),
                Err(_) => {}
            }
            if errors > 0 {
                app.status_message = Some(format!("Closed {} tabs ({} errors)", closed, errors));
            } else {
                app.status_message = Some(format!("Closed {} tabs", closed));
            }
        }
        _ => {
            app.confirm_close = false;
            app.status_message = Some("Close cancelled".to_string());
        }
    }
    Ok(())
}

async fn handle_export_key(
    app: &mut App,
    _client: &ChromeClient,
    key: KeyEvent,
) -> Result<()> {
    let step = app.export_dialog.as_ref().map(|d| d.step.clone());

    match step {
        Some(ExportStep::PickFormat) => match key.code {
            KeyCode::Char('h') | KeyCode::Char('H') => {
                use crate::tui::export::get_default_export_path;
                if let Some(ref mut dialog) = app.export_dialog {
                    dialog.format = ExportFormat::NetscapeHtml;
                    dialog.path = get_default_export_path("html");
                    dialog.step = ExportStep::EditPath;
                }
            }
            KeyCode::Char('m') | KeyCode::Char('M') => {
                use crate::tui::export::get_default_export_path;
                if let Some(ref mut dialog) = app.export_dialog {
                    dialog.format = ExportFormat::Markdown;
                    dialog.path = get_default_export_path("md");
                    dialog.step = ExportStep::EditPath;
                }
            }
            KeyCode::Tab => {
                if let Some(ref mut dialog) = app.export_dialog {
                    use crate::tui::export::get_default_export_path;
                    dialog.format = match dialog.format {
                        ExportFormat::NetscapeHtml => {
                            dialog.path = get_default_export_path("md");
                            ExportFormat::Markdown
                        }
                        ExportFormat::Markdown => {
                            dialog.path = get_default_export_path("html");
                            ExportFormat::NetscapeHtml
                        }
                    };
                }
            }
            KeyCode::Enter => {
                if let Some(ref mut dialog) = app.export_dialog {
                    dialog.step = ExportStep::EditPath;
                }
            }
            KeyCode::Esc => {
                app.export_dialog = None;
            }
            _ => {}
        },

        Some(ExportStep::EditPath) => match key.code {
            KeyCode::Enter => {
                let result = do_export(app);
                app.export_dialog = None;
                match result {
                    Ok(msg) => app.status_message = Some(msg),
                    Err(e) => app.status_message = Some(format!("Export failed: {}", e)),
                }
            }
            KeyCode::Esc => {
                if let Some(ref mut dialog) = app.export_dialog {
                    dialog.step = ExportStep::PickFormat;
                }
            }
            KeyCode::Backspace => {
                if let Some(ref mut dialog) = app.export_dialog {
                    dialog.path.pop();
                }
            }
            KeyCode::Char(c) if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT => {
                if let Some(ref mut dialog) = app.export_dialog {
                    dialog.path.push(c);
                }
            }
            _ => {}
        },

        None => {
            app.export_dialog = None;
        }
    }

    Ok(())
}

fn do_export(app: &App) -> Result<String> {
    use crate::tui::export::{export_netscape_html, export_markdown};

    let dialog = app.export_dialog.as_ref().unwrap();
    // If nothing selected, export all
    let ids = &app.selected_tab_ids;

    match dialog.format {
        ExportFormat::NetscapeHtml => {
            export_netscape_html(&app.groups, &app.tabs, ids, &dialog.path)
        }
        ExportFormat::Markdown => {
            export_markdown(&app.groups, &app.tabs, ids, &dialog.path)
        }
    }
}

/// Estimate visible list rows based on terminal height.
/// Subtracts: header(1) + footer(1) + panel borders(2).
fn page_size() -> usize {
    crossterm::terminal::size()
        .map(|(_, h)| (h as usize).saturating_sub(4))
        .unwrap_or(10)
        .max(1)
}
