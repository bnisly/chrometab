// Bookmark export: Netscape HTML and Markdown formats

use anyhow::Result;
use chrono::Local;
use std::fs;
use crate::chrome::Tab;
use crate::groups::TabGroup;

pub fn get_default_export_path(extension: &str) -> String {
    let date = Local::now().format("%Y-%m-%d").to_string();
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    format!("{}/chrometab-{}.{}", home, date, extension)
}

/// Export selected tabs as Netscape HTML bookmarks (importable to Chrome/Firefox).
pub fn export_netscape_html(
    groups: &[TabGroup],
    tabs: &[Tab],
    selected_ids: &std::collections::HashSet<String>,
    path: &str,
) -> Result<String> {
    let mut html = String::new();
    html.push_str("<!DOCTYPE NETSCAPE-Bookmark-file-1>\n");
    html.push_str("<!-- This is an automatically generated file.\n");
    html.push_str("     It will be read and overwritten.\n");
    html.push_str("     DO NOT EDIT! -->\n");
    html.push_str("<META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n");
    html.push_str("<TITLE>Bookmarks</TITLE>\n");
    html.push_str("<H1>Bookmarks</H1>\n");
    html.push_str("<DL><p>\n");
    html.push_str("    <DT><H3>ChromeTab Export</H3>\n");
    html.push_str("    <DL><p>\n");

    let mut exported = 0usize;

    for group in groups {
        let group_tabs: Vec<&Tab> = group
            .tab_indices
            .iter()
            .filter_map(|&i| tabs.get(i))
            .filter(|t| selected_ids.is_empty() || selected_ids.contains(&t.target_id))
            .collect();

        if group_tabs.is_empty() {
            continue;
        }

        html.push_str(&format!(
            "        <DT><H3>{}</H3>\n        <DL><p>\n",
            escape_html(&group.name)
        ));

        for tab in group_tabs {
            let title = if tab.title.is_empty() { &tab.url } else { &tab.title };
            html.push_str(&format!(
                "            <DT><A HREF=\"{}\">{}</A>\n",
                escape_html(&tab.url),
                escape_html(title)
            ));
            exported += 1;
        }

        html.push_str("        </DL><p>\n");
    }

    html.push_str("    </DL><p>\n");
    html.push_str("</DL><p>\n");

    fs::write(path, &html)?;
    Ok(format!("Exported {} bookmarks to {}", exported, path))
}

/// Export selected tabs as Markdown.
pub fn export_markdown(
    groups: &[TabGroup],
    tabs: &[Tab],
    selected_ids: &std::collections::HashSet<String>,
    path: &str,
) -> Result<String> {
    let date = Local::now().format("%Y-%m-%d").to_string();
    let mut md = format!("# ChromeTab Export — {}\n\n", date);

    let mut exported = 0usize;

    for group in groups {
        let group_tabs: Vec<&Tab> = group
            .tab_indices
            .iter()
            .filter_map(|&i| tabs.get(i))
            .filter(|t| selected_ids.is_empty() || selected_ids.contains(&t.target_id))
            .collect();

        if group_tabs.is_empty() {
            continue;
        }

        md.push_str(&format!("## {}\n\n", group.name));

        for tab in group_tabs {
            let title = if tab.title.is_empty() { &tab.url } else { &tab.title };
            md.push_str(&format!("- [{}]({})\n", escape_md(title), tab.url));
            exported += 1;
        }

        md.push('\n');
    }

    fs::write(path, &md)?;
    Ok(format!("Exported {} bookmarks to {}", exported, path))
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn escape_md(s: &str) -> String {
    s.replace('[', "\\[").replace(']', "\\]")
}
