// TUI entry point — App state and event loop

pub mod events;
pub mod export;
pub mod ui;

use std::collections::{HashMap, HashSet};
use std::io;
use std::time::Duration;
use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, widgets::ListState, Terminal};

use crate::chrome::{ChromeClient, Tab};
use crate::groups::{flat_group, group_tabs, TabGroup};
use crate::platform::BrowserKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Groups,
    Tabs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Grouped,
    Flat,
}

impl ViewMode {
    pub fn label(self) -> &'static str {
        match self {
            ViewMode::Grouped => "Grouped",
            ViewMode::Flat => "Flat",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    BrowserOrder,
    OldestFirst,
    NewestFirst,
}

#[derive(Debug, Clone)]
pub struct AgeFilterDialog {
    pub input: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportFormat {
    NetscapeHtml,
    Markdown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportStep {
    PickFormat,
    EditPath,
}

#[derive(Debug)]
pub struct ExportDialog {
    pub step: ExportStep,
    pub format: ExportFormat,
    pub path: String,
}

pub struct App {
    pub tabs: Vec<Tab>,
    /// Underlying grouped data (always kept up to date).
    pub groups: Vec<TabGroup>,
    /// What is currently displayed in the Groups panel.
    pub display_groups: Vec<TabGroup>,
    pub view_mode: ViewMode,
    pub selected_group: usize,
    pub selected_tab: usize,
    pub selected_tab_ids: HashSet<String>,
    pub focus: Panel,
    pub filter: String,
    pub filter_mode: bool,
    pub status_message: Option<String>,
    pub show_help: bool,
    pub export_dialog: Option<ExportDialog>,
    pub confirm_close: bool,
    #[allow(dead_code)]
    pub browser: BrowserKind,
    pub should_quit: bool,
    pub group_list_state: ListState,
    pub tab_list_state: ListState,
    pub tab_ages: HashMap<String, Duration>,
    pub sort_mode: SortMode,
    pub age_filter_dialog: Option<AgeFilterDialog>,
}

impl App {
    pub fn new(tabs: Vec<Tab>, browser: BrowserKind) -> Self {
        let groups = group_tabs(&tabs);
        let display_groups = groups.clone();
        let mut group_list_state = ListState::default();
        let mut tab_list_state = ListState::default();
        if !display_groups.is_empty() {
            group_list_state.select(Some(0));
        }
        if !display_groups.is_empty() && !display_groups[0].tab_indices.is_empty() {
            tab_list_state.select(Some(0));
        }
        Self {
            tabs,
            groups,
            display_groups,
            view_mode: ViewMode::Grouped,
            selected_group: 0,
            selected_tab: 0,
            selected_tab_ids: HashSet::new(),
            focus: Panel::Groups,
            filter: String::new(),
            filter_mode: false,
            status_message: None,
            show_help: false,
            export_dialog: None,
            confirm_close: false,
            browser,
            should_quit: false,
            group_list_state,
            tab_list_state,
            tab_ages: HashMap::new(),
            sort_mode: SortMode::BrowserOrder,
            age_filter_dialog: None,
        }
    }

    pub fn toggle_view_mode(&mut self) {
        self.view_mode = match self.view_mode {
            ViewMode::Grouped => ViewMode::Flat,
            ViewMode::Flat => ViewMode::Grouped,
        };
        self.display_groups = self.make_display_groups();
        self.apply_sort();
        self.selected_group = 0;
        let has_group = !self.display_groups.is_empty();
        self.group_list_state.select(if has_group { Some(0) } else { None });
        self.selected_tab = 0;
        self.tab_list_state.select(if has_group { Some(0) } else { None });
        self.filter.clear();
        // In flat mode there is no groups panel, so focus must be on tabs
        if self.view_mode == ViewMode::Flat {
            self.focus = Panel::Tabs;
        }
    }

    fn make_display_groups(&self) -> Vec<TabGroup> {
        match self.view_mode {
            ViewMode::Grouped => self.groups.clone(),
            ViewMode::Flat => flat_group(&self.tabs),
        }
    }

    /// Count of (filtered) tabs in the current group.
    pub fn current_group_tab_count(&self) -> usize {
        let group = match self.display_groups.get(self.selected_group) {
            Some(g) => g,
            None => return 0,
        };
        let filter = self.filter.to_lowercase();
        if filter.is_empty() {
            return group.tab_indices.len();
        }
        group
            .tab_indices
            .iter()
            .filter_map(|&i| self.tabs.get(i))
            .filter(|t| {
                t.title.to_lowercase().contains(&filter)
                    || t.url.to_lowercase().contains(&filter)
            })
            .count()
    }

    /// Get the tab currently under the cursor (respecting filter).
    pub fn selected_tab_in_group(&self) -> Option<&Tab> {
        let group = self.display_groups.get(self.selected_group)?;
        let filter = self.filter.to_lowercase();
        let mut count = 0usize;
        for &i in &group.tab_indices {
            let tab = self.tabs.get(i)?;
            if !filter.is_empty()
                && !tab.title.to_lowercase().contains(&filter)
                && !tab.url.to_lowercase().contains(&filter)
            {
                continue;
            }
            if count == self.selected_tab {
                return Some(tab);
            }
            count += 1;
        }
        None
    }

    pub fn toggle_select_current(&mut self) {
        if let Some(id) = self.selected_tab_in_group().map(|t| t.target_id.clone()) {
            if self.selected_tab_ids.contains(&id) {
                self.selected_tab_ids.remove(&id);
            } else {
                self.selected_tab_ids.insert(id);
            }
        }
    }

    pub fn select_all_in_group(&mut self) {
        let group = match self.display_groups.get(self.selected_group) {
            Some(g) => g.clone(),
            None => return,
        };
        let ids: Vec<String> = group
            .tab_indices
            .iter()
            .filter_map(|&i| self.tabs.get(i).map(|t| t.target_id.clone()))
            .collect();
        let all_selected = ids.iter().all(|id| self.selected_tab_ids.contains(id));
        if all_selected {
            for id in &ids {
                self.selected_tab_ids.remove(id);
            }
        } else {
            for id in ids {
                self.selected_tab_ids.insert(id);
            }
        }
    }

    pub fn select_all(&mut self) {
        let all_selected = self
            .tabs
            .iter()
            .all(|t| self.selected_tab_ids.contains(&t.target_id));
        if all_selected {
            self.selected_tab_ids.clear();
        } else {
            for tab in &self.tabs {
                self.selected_tab_ids.insert(tab.target_id.clone());
            }
        }
    }

    pub fn refresh(&mut self, tabs: Vec<Tab>) {
        self.tabs = tabs;
        self.groups = group_tabs(&self.tabs);
        self.display_groups = self.make_display_groups();
        self.apply_sort();
        let ng = self.display_groups.len();
        if ng == 0 {
            self.selected_group = 0;
            self.group_list_state.select(None);
            self.selected_tab = 0;
            self.tab_list_state.select(None);
            return;
        }
        if self.selected_group >= ng {
            self.selected_group = ng - 1;
            self.group_list_state.select(Some(self.selected_group));
        }
        let nt = self.current_group_tab_count();
        if nt == 0 {
            self.selected_tab = 0;
            self.tab_list_state.select(None);
        } else if self.selected_tab >= nt {
            self.selected_tab = nt - 1;
            self.tab_list_state.select(Some(self.selected_tab));
        }
    }

    /// Sort tab_indices within each display_group by the current sort_mode.
    /// Does nothing when sort_mode is BrowserOrder.
    pub fn apply_sort(&mut self) {
        if self.sort_mode == SortMode::BrowserOrder {
            return;
        }
        let sort_mode = self.sort_mode;
        let tab_ages = &self.tab_ages;
        let tabs = &self.tabs;
        for group in &mut self.display_groups {
            group.tab_indices.sort_by(|&a, &b| {
                let age_a = tabs
                    .get(a)
                    .and_then(|t| tab_ages.get(&t.target_id))
                    .copied();
                let age_b = tabs
                    .get(b)
                    .and_then(|t| tab_ages.get(&t.target_id))
                    .copied();
                match (age_a, age_b) {
                    (None, None) => std::cmp::Ordering::Equal,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (Some(a_dur), Some(b_dur)) => match sort_mode {
                        SortMode::OldestFirst => b_dur.cmp(&a_dur),
                        SortMode::NewestFirst => a_dur.cmp(&b_dur),
                        SortMode::BrowserOrder => std::cmp::Ordering::Equal,
                    },
                }
            });
        }
    }

    /// Advance sort_mode by one step and re-sort display_groups.
    pub fn cycle_sort(&mut self) {
        self.sort_mode = match self.sort_mode {
            SortMode::BrowserOrder => SortMode::OldestFirst,
            SortMode::OldestFirst => SortMode::NewestFirst,
            SortMode::NewestFirst => SortMode::BrowserOrder,
        };
        // Rebuild from browser order first, then apply new sort.
        self.display_groups = self.make_display_groups();
        self.apply_sort();
    }

    /// Insert into selected_tab_ids all tabs whose age exceeds `threshold`.
    pub fn select_older_than(&mut self, threshold: Duration) {
        for tab in &self.tabs {
            if let Some(&age) = self.tab_ages.get(&tab.target_id) {
                if age > threshold {
                    self.selected_tab_ids.insert(tab.target_id.clone());
                }
            }
        }
    }
}

/// Parse age threshold strings like "30m", "12h", "7d" into a Duration.
pub fn parse_age_threshold(s: &str) -> Option<Duration> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (num_str, unit) = if let Some(n) = s.strip_suffix('d') {
        (n, 'd')
    } else if let Some(n) = s.strip_suffix('h') {
        (n, 'h')
    } else if let Some(n) = s.strip_suffix('m') {
        (n, 'm')
    } else {
        return None;
    };
    let n: u64 = num_str.trim().parse().ok()?;
    let secs = match unit {
        'm' => n * 60,
        'h' => n * 3600,
        'd' => n * 86400,
        _ => return None,
    };
    Some(Duration::from_secs(secs))
}

/// Compact age label — always 3 characters: "now", "30m", " 4h", " 7d".
pub fn format_age_short(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        "now".to_string()
    } else if secs < 3600 {
        format!("{:2}m", (secs / 60).min(99))
    } else if secs < 86400 {
        format!("{:2}h", (secs / 3600).min(99))
    } else {
        format!("{:2}d", (secs / 86400).min(99))
    }
}

/// Verbose age label for the details panel.
pub fn format_age_long(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        "just now".to_string()
    } else if secs < 3600 {
        let m = secs / 60;
        format!("{} minute{}", m, if m == 1 { "" } else { "s" })
    } else if secs < 86400 {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        if m > 0 {
            format!(
                "{} hour{} {} minute{}",
                h, if h == 1 { "" } else { "s" },
                m, if m == 1 { "" } else { "s" }
            )
        } else {
            format!("{} hour{}", h, if h == 1 { "" } else { "s" })
        }
    } else {
        let d = secs / 86400;
        let h = (secs % 86400) / 3600;
        if h > 0 {
            format!(
                "{} day{} {} hour{}",
                d, if d == 1 { "" } else { "s" },
                h, if h == 1 { "" } else { "s" }
            )
        } else {
            format!("{} day{}", d, if d == 1 { "" } else { "s" })
        }
    }
}

/// Entry point for TUI mode.
pub async fn run(client: &ChromeClient, browser: BrowserKind) -> Result<()> {
    let tabs = client.get_tabs(false).await?;
    let mut app = App::new(tabs, browser);

    // Fetch initial tab ages in parallel (best-effort; failures yield no age data).
    let ages = ChromeClient::fetch_all_ages(&app.tabs).await;
    app.tab_ages = ages;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_event_loop(&mut terminal, &mut app, client).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    client: &ChromeClient,
) -> Result<()> {
    use crossterm::event::{Event, EventStream};
    use futures_util::StreamExt;

    let mut reader = EventStream::new();

    loop {
        terminal.draw(|f| ui::render(f, app))?;

        tokio::select! {
            maybe_event = reader.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) => {
                        // Clear status message on any key (except in filter mode)
                        if !app.filter_mode && !app.show_help && app.export_dialog.is_none() && !app.confirm_close {
                            app.status_message = None;
                        }
                        events::handle_key(app, client, key).await?;
                    }
                    Some(Ok(_)) => {} // Mouse, resize, etc.
                    Some(Err(e)) => return Err(e.into()),
                    None => break,
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
