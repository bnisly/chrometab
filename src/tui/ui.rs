// TUI rendering — three-panel layout

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};
use crate::groups::GroupKind;
use super::{format_age_long, format_age_short, App, ExportStep, Panel, SortMode};

const FOCUSED_BORDER: Style = Style::new()
    .fg(Color::Blue)
    .add_modifier(Modifier::BOLD);
const DIM_BORDER: Style = Style::new().fg(Color::DarkGray);
const SELECTED_ITEM: Style = Style::new()
    .fg(Color::Black)
    .bg(Color::Blue)
    .add_modifier(Modifier::BOLD);
const SELECTED_ITEM_DIM: Style = Style::new()
    .fg(Color::Gray)
    .bg(Color::DarkGray);
const CHECKED_STYLE: Style = Style::new().fg(Color::Green);
const HEADER_STYLE: Style = Style::new()
    .fg(Color::Cyan)
    .add_modifier(Modifier::BOLD);
const STATUS_STYLE: Style = Style::new().fg(Color::Yellow);

pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();

    // Overall vertical split: header | content | footer
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .split(area);

    render_header(f, app, vertical[0]);

    if app.view_mode == super::ViewMode::Flat {
        // Flat mode: no groups panel — tabs gets full left side
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(vertical[1]);
        render_tabs(f, app, horizontal[0]);
        render_details(f, app, horizontal[1]);
    } else {
        // Grouped mode: three panels
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(22),
                Constraint::Percentage(50),
                Constraint::Percentage(28),
            ])
            .split(vertical[1]);
        render_groups(f, app, horizontal[0]);
        render_tabs(f, app, horizontal[1]);
        render_details(f, app, horizontal[2]);
    }

    render_footer(f, app, vertical[2]);

    // Overlays (rendered on top)
    if app.show_help {
        render_help(f, area);
    }
    if app.export_dialog.is_some() {
        render_export_dialog(f, app, area);
    }
    if app.confirm_close {
        render_confirm_close(f, app, area);
    }
    if app.age_filter_dialog.is_some() {
        render_age_filter_dialog(f, app, area);
    }
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let total = app.tabs.len();
    let selected = app.selected_tab_ids.len();
    let group_name = app
        .groups
        .get(app.selected_group)
        .map(|g| g.name.as_str())
        .unwrap_or("");

    let sort_label = match app.sort_mode {
        SortMode::BrowserOrder => "",
        SortMode::OldestFirst => "  [Oldest→]",
        SortMode::NewestFirst => "  [Newest→]",
    };

    let text = format!(
        " ChromeTab v{}  [Tabs: {}]  [Sel: {}]  [{}]  [{}]{}",
        env!("CARGO_PKG_VERSION"),
        total,
        selected,
        app.view_mode.label(),
        group_name,
        sort_label,
    );

    let header = Paragraph::new(text).style(HEADER_STYLE);
    f.render_widget(header, area);
}

fn render_groups(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Panel::Groups;
    let border_style = if focused { FOCUSED_BORDER } else { DIM_BORDER };
    let border_type = if focused { BorderType::Double } else { BorderType::Plain };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(border_type)
        .title(" Groups ")
        .border_style(border_style);

    let items: Vec<ListItem> = app
        .groups
        .iter()
        .map(|g| {
            let count = g.tab_indices.len();
            let label = format!("{} ({})", g.name, count);
            let style = match g.kind {
                GroupKind::Duplicates => Style::default().fg(Color::Red),
                GroupKind::System => Style::default().fg(Color::DarkGray),
                GroupKind::Domain => Style::default().fg(Color::White),
                GroupKind::Ungrouped => Style::default().fg(Color::Gray),
            };
            ListItem::new(Line::from(Span::styled(label, style)))
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(if focused { SELECTED_ITEM } else { SELECTED_ITEM_DIM })
        .highlight_symbol("> ");

    f.render_stateful_widget(list, area, &mut app.group_list_state);
}

fn render_tabs(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Panel::Tabs;
    let border_style = if focused { FOCUSED_BORDER } else { DIM_BORDER };
    let border_type = if focused { BorderType::Double } else { BorderType::Plain };

    let group_name = app
        .display_groups
        .get(app.selected_group)
        .map(|g| format!(" {} ", g.name))
        .unwrap_or_else(|| " Tabs ".to_string());

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(border_type)
        .title(group_name)
        .border_style(border_style);

    let filter = app.filter.to_lowercase();
    let group = app.display_groups.get(app.selected_group).cloned();

    let items: Vec<ListItem> = match group {
        None => vec![],
        Some(g) => g
            .tab_indices
            .iter()
            .enumerate()
            .filter_map(|(seq, &tab_idx)| {
                let tab = app.tabs.get(tab_idx)?;
                let title = if tab.title.is_empty() { &tab.url } else { &tab.title };

                // Apply filter
                if !filter.is_empty()
                    && !title.to_lowercase().contains(&filter)
                    && !tab.url.to_lowercase().contains(&filter)
                {
                    return None;
                }

                let checked = app.selected_tab_ids.contains(&tab.target_id);
                let checkbox = if checked { "[x]" } else { "[ ]" };
                let check_style = if checked {
                    CHECKED_STYLE
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                let age_str = app
                    .tab_ages
                    .get(&tab.target_id)
                    .map(|&d| format!("{:>3}", format_age_short(d)))
                    .unwrap_or_else(|| "   ".to_string());

                let title_display: String = title.chars().take(55).collect();
                let label = format!(" {:02}. {}", seq + 1, title_display);

                let line = Line::from(vec![
                    Span::styled(checkbox, check_style),
                    Span::styled(
                        format!(" {} ", age_str),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw(label),
                ]);

                Some(ListItem::new(line))
            })
            .collect(),
    };

    let list = List::new(items)
        .block(block)
        .highlight_style(if focused { SELECTED_ITEM } else { SELECTED_ITEM_DIM })
        .highlight_symbol("");

    f.render_stateful_widget(list, area, &mut app.tab_list_state);
}

fn render_details(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Details ")
        .border_style(DIM_BORDER);

    let tab = app.selected_tab_in_group();

    let content = match tab {
        None => "No tab selected".to_string(),
        Some(t) => {
            let group_name = app
                .groups
                .get(app.selected_group)
                .map(|g| g.name.as_str())
                .unwrap_or("—");
            let tab_type = t.tab_type.as_deref().unwrap_or("page");
            let selected = if app.selected_tab_ids.contains(&t.target_id) {
                "Yes"
            } else {
                "No"
            };
            let age_str = app
                .tab_ages
                .get(&t.target_id)
                .map(|&d| format_age_long(d))
                .unwrap_or_else(|| "unknown".to_string());
            format!(
                "Title:\n{}\n\nURL:\n{}\n\nType: {}\nGroup: {}\nAge: {}\nSelected: {}",
                t.title, t.url, tab_type, group_name, age_str, selected
            )
        }
    };

    let paragraph = Paragraph::new(content)
        .block(block)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::Gray));

    f.render_widget(paragraph, area);
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let text = if app.filter_mode {
        format!(" Filter: {}█", app.filter)
    } else if let Some(ref msg) = app.status_message {
        format!(" {}", msg)
    } else {
        " Space=Sel  Enter=Activate  d=Close  b=Bookmark  /=Filter  s=Sort  t=AgeFilter  v=View  r=Refresh  ?=Help  q=Quit"
            .to_string()
    };

    let style = if app.filter_mode || app.status_message.is_some() {
        STATUS_STYLE
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let footer = Paragraph::new(text).style(style);
    f.render_widget(footer, area);
}

fn render_help(f: &mut Frame, area: Rect) {
    let popup_area = centered_rect(60, 80, area);
    f.render_widget(Clear, popup_area);

    let help_text = vec![
        Line::from(Span::styled(" Key Bindings", Style::default().add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Arrow / hjkl", Style::default().fg(Color::Yellow)),
            Span::raw("  Navigate"),
        ]),
        Line::from(vec![
            Span::styled(" Tab          ", Style::default().fg(Color::Yellow)),
            Span::raw("  Switch panel focus"),
        ]),
        Line::from(vec![
            Span::styled(" Space        ", Style::default().fg(Color::Yellow)),
            Span::raw("  Toggle select tab"),
        ]),
        Line::from(vec![
            Span::styled(" a            ", Style::default().fg(Color::Yellow)),
            Span::raw("  Select all in group"),
        ]),
        Line::from(vec![
            Span::styled(" A            ", Style::default().fg(Color::Yellow)),
            Span::raw("  Select all tabs"),
        ]),
        Line::from(vec![
            Span::styled(" Enter        ", Style::default().fg(Color::Yellow)),
            Span::raw("  Activate tab in browser"),
        ]),
        Line::from(vec![
            Span::styled(" d            ", Style::default().fg(Color::Yellow)),
            Span::raw("  Close selected tabs"),
        ]),
        Line::from(vec![
            Span::styled(" b            ", Style::default().fg(Color::Yellow)),
            Span::raw("  Export bookmarks (HTML/Markdown)"),
        ]),
        Line::from(vec![
            Span::styled(" /            ", Style::default().fg(Color::Yellow)),
            Span::raw("  Filter tabs by text"),
        ]),
        Line::from(vec![
            Span::styled(" v            ", Style::default().fg(Color::Yellow)),
            Span::raw("  Toggle grouped / flat view"),
        ]),
        Line::from(vec![
            Span::styled(" s            ", Style::default().fg(Color::Yellow)),
            Span::raw("  Sort by age"),
        ]),
        Line::from(vec![
            Span::styled(" t            ", Style::default().fg(Color::Yellow)),
            Span::raw("  Select tabs by age threshold"),
        ]),
        Line::from(vec![
            Span::styled(" r            ", Style::default().fg(Color::Yellow)),
            Span::raw("  Refresh tab list"),
        ]),
        Line::from(vec![
            Span::styled(" ?            ", Style::default().fg(Color::Yellow)),
            Span::raw("  This help"),
        ]),
        Line::from(vec![
            Span::styled(" q / Esc      ", Style::default().fg(Color::Yellow)),
            Span::raw("  Quit"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Press any key to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Help ")
        .border_style(FOCUSED_BORDER);

    let paragraph = Paragraph::new(help_text).block(block);
    f.render_widget(paragraph, popup_area);
}

fn render_export_dialog(f: &mut Frame, app: &App, area: Rect) {
    let dialog = match app.export_dialog.as_ref() {
        Some(d) => d,
        None => return,
    };

    let popup_area = centered_rect(60, 40, area);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Export Bookmarks ")
        .border_style(FOCUSED_BORDER);

    let count = app.selected_tab_ids.len();
    let scope = if count == 0 {
        format!("all {} tabs", app.tabs.len())
    } else {
        format!("{} selected tabs", count)
    };

    let content = match dialog.step {
        ExportStep::PickFormat => {
            let html_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
            let md_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
            vec![
                Line::from(format!(" Exporting: {}", scope)),
                Line::from(""),
                Line::from(" Pick format:"),
                Line::from(""),
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled("[H] HTML (Chrome/Firefox importable)", html_style),
                ]),
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled("[M] Markdown", md_style),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "  Tab=toggle  Enter=next  Esc=cancel",
                    Style::default().fg(Color::DarkGray),
                )),
            ]
        }
        ExportStep::EditPath => {
            let fmt = match dialog.format {
                super::ExportFormat::NetscapeHtml => "HTML",
                super::ExportFormat::Markdown => "Markdown",
            };
            vec![
                Line::from(format!(" Format: {} | Exporting: {}", fmt, scope)),
                Line::from(""),
                Line::from(" Save path:"),
                Line::from(format!("  {}█", dialog.path)),
                Line::from(""),
                Line::from(Span::styled(
                    "  Enter=export  Esc=back  Backspace=delete",
                    Style::default().fg(Color::DarkGray),
                )),
            ]
        }
    };

    let paragraph = Paragraph::new(content).block(block).wrap(Wrap { trim: true });
    f.render_widget(paragraph, popup_area);
}

fn render_age_filter_dialog(f: &mut Frame, app: &App, area: Rect) {
    let dialog = match app.age_filter_dialog.as_ref() {
        Some(d) => d,
        None => return,
    };

    let popup_area = centered_rect(44, 35, area);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Select by Age ")
        .border_style(FOCUSED_BORDER);

    let content = vec![
        Line::from(""),
        Line::from(" Select tabs older than:"),
        Line::from(format!("  > {}█", dialog.input)),
        Line::from(""),
        Line::from(Span::styled(
            "  e.g. 30m, 12h, 7d",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Enter=select  Esc=cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(content).block(block);
    f.render_widget(paragraph, popup_area);
}

fn render_confirm_close(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(50, 30, area);
    f.render_widget(Clear, popup_area);

    let n = app.selected_tab_ids.len();
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Confirm ")
        .border_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD));

    let content = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!(" Close {} tab{}?", n, if n == 1 { "" } else { "s" }),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(" This cannot be undone."),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [y/Enter] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("Confirm    "),
            Span::styled("[n/Esc] ", Style::default().fg(Color::Yellow)),
            Span::raw("Cancel"),
        ]),
    ];

    let paragraph = Paragraph::new(content).block(block).alignment(Alignment::Left);
    f.render_widget(paragraph, popup_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let pad_v = (100 - percent_y) / 2;
    let pad_h = (100 - percent_x) / 2;

    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(pad_v),
            Constraint::Percentage(percent_y),
            Constraint::Percentage(pad_v),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(pad_h),
            Constraint::Percentage(percent_x),
            Constraint::Percentage(pad_h),
        ])
        .split(vert[1])[1]
}
