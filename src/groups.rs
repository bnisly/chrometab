// Tab grouping logic

use std::collections::HashMap;
use crate::chrome::Tab;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupKind {
    Duplicates,
    System,
    Domain,
    Ungrouped,
}

#[derive(Debug, Clone)]
pub struct TabGroup {
    pub name: String,
    pub kind: GroupKind,
    pub tab_indices: Vec<usize>,
}

/// Extract eTLD+1 from a URL (e.g. `docs.google.com` → `google.com`).
pub fn extract_domain(url: &str) -> Option<String> {
    let url = url.trim();
    let url = url.strip_prefix("https://").unwrap_or(url);
    let url = url.strip_prefix("http://").unwrap_or(url);
    let url = url.strip_prefix("www.").unwrap_or(url);
    let host = url.split('/').next()?;
    let host = host.split(':').next()?; // strip port
    let host = host.split('?').next()?; // strip query

    if host.is_empty() {
        return None;
    }

    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() >= 2 {
        Some(format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]))
    } else {
        Some(host.to_string())
    }
}

/// Returns true for system/browser-internal URLs.
pub fn is_system_url(url: &str) -> bool {
    url.starts_with("chrome://")
        || url.starts_with("chrome-extension://")
        || url.starts_with("about:")
        || url.starts_with("file://")
        || url.starts_with("edge://")
        || url.starts_with("brave://")
        || url.is_empty()
}

/// A single group containing all tabs in their original (browser) order.
pub fn flat_group(tabs: &[Tab]) -> Vec<TabGroup> {
    if tabs.is_empty() {
        return vec![];
    }
    vec![TabGroup {
        name: "All Tabs".to_string(),
        kind: GroupKind::Ungrouped,
        tab_indices: (0..tabs.len()).collect(),
    }]
}

/// Group tabs by duplicate URLs, system URLs, domain, and ungrouped.
pub fn group_tabs(tabs: &[Tab]) -> Vec<TabGroup> {
    // Find duplicate URL indices
    let mut url_map: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, tab) in tabs.iter().enumerate() {
        url_map.entry(tab.url.as_str()).or_default().push(i);
    }

    let mut duplicate_indices: Vec<usize> = url_map
        .values()
        .filter(|v| v.len() > 1)
        .flat_map(|v| v.iter().copied())
        .collect();
    duplicate_indices.sort_unstable();

    let dup_set: std::collections::HashSet<usize> = duplicate_indices.iter().copied().collect();

    let mut system_indices = Vec::new();
    let mut domain_map: HashMap<String, Vec<usize>> = HashMap::new();
    let mut ungrouped_indices = Vec::new();

    for (i, tab) in tabs.iter().enumerate() {
        if dup_set.contains(&i) {
            continue;
        }
        if is_system_url(&tab.url) {
            system_indices.push(i);
        } else if let Some(domain) = extract_domain(&tab.url) {
            domain_map.entry(domain).or_default().push(i);
        } else {
            ungrouped_indices.push(i);
        }
    }

    let mut groups = Vec::new();

    if !duplicate_indices.is_empty() {
        groups.push(TabGroup {
            name: "[DUPLICATES]".to_string(),
            kind: GroupKind::Duplicates,
            tab_indices: duplicate_indices,
        });
    }

    if !system_indices.is_empty() {
        groups.push(TabGroup {
            name: "[SYSTEM]".to_string(),
            kind: GroupKind::System,
            tab_indices: system_indices,
        });
    }

    let mut domain_groups: Vec<TabGroup> = domain_map
        .into_iter()
        .map(|(domain, indices)| TabGroup {
            name: domain,
            kind: GroupKind::Domain,
            tab_indices: indices,
        })
        .collect();

    // Sort domains by tab count (desc), then alphabetically
    domain_groups.sort_by(|a, b| {
        b.tab_indices.len().cmp(&a.tab_indices.len()).then(a.name.cmp(&b.name))
    });
    groups.extend(domain_groups);

    if !ungrouped_indices.is_empty() {
        groups.push(TabGroup {
            name: "[ungrouped]".to_string(),
            kind: GroupKind::Ungrouped,
            tab_indices: ungrouped_indices,
        });
    }

    groups
}
