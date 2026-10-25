use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MonitoringState {
    Active,
    Paused,
    Stopped,
    Unavailable,
}

impl MonitoringState {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowMetadata {
    pub app_id: Option<String>,
    pub window_title: Option<String>,
    pub origin: Option<String>,
    pub is_idle: bool,
}

impl WindowMetadata {
    pub fn empty() -> Self {
        Self {
            app_id: None,
            window_title: None,
            origin: None,
            is_idle: false,
        }
    }

    pub fn new(
        app_id: Option<String>,
        window_title: Option<String>,
        origin: Option<String>,
        is_idle: bool,
    ) -> Self {
        Self {
            app_id,
            window_title,
            origin,
            is_idle,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObserverConfig {
    pub sampling_interval_ms: u64,
    pub idle_threshold_seconds: u64,
    pub enable_origin_heuristic: bool,
}

impl Default for ObserverConfig {
    fn default() -> Self {
        Self {
            sampling_interval_ms: 2000,
            idle_threshold_seconds: 300,
            enable_origin_heuristic: true,
        }
    }
}

impl ObserverConfig {
    pub fn interval(&self) -> Duration {
        Duration::from_millis(self.sampling_interval_ms)
    }
}

pub const KNOWN_TLDS: &[&str] = &[
    "com", "org", "net", "io", "tv", "dev", "ai", "app", "edu", "gov", "rs", "co", "uk", "de",
    "ca", "fr", "jp", "me", "info",
];

/// Extracts browser origin (domain) from application class and window title.
pub fn extract_browser_origin(class: &str, title: &str) -> Option<String> {
    let class_lower = class.to_lowercase();
    let is_browser = class_lower.contains("browser")
        || class_lower.contains("firefox")
        || class_lower.contains("chrome")
        || class_lower.contains("chromium")
        || class_lower.contains("zen")
        || class_lower.contains("floorp")
        || class_lower.contains("brave")
        || class_lower.contains("opera")
        || class_lower.contains("vivaldi")
        || class_lower.contains("edge")
        || class_lower.contains("epiphany")
        || class_lower.contains("librewolf");

    if !is_browser {
        return None;
    }

    let title_lower = title.to_lowercase();

    // 1. Known popular and entertainment sites
    if title_lower.contains("youtube") {
        return Some("youtube.com".to_string());
    } else if title_lower.contains("github") {
        return Some("github.com".to_string());
    } else if title_lower.contains("gitlab") {
        return Some("gitlab.com".to_string());
    } else if title_lower.contains("reddit") {
        return Some("reddit.com".to_string());
    } else if title_lower.contains("stackoverflow") {
        return Some("stackoverflow.com".to_string());
    } else if title_lower.contains("stackexchange") {
        return Some("stackexchange.com".to_string());
    } else if title_lower.contains("twitch") {
        return Some("twitch.tv".to_string());
    } else if title_lower.contains("docs.rs") {
        return Some("docs.rs".to_string());
    } else if title_lower.contains("crates.io") {
        return Some("crates.io".to_string());
    } else if title_lower.contains("twitter") || title_lower.contains("x.com") {
        return Some("twitter.com".to_string());
    } else if title_lower.contains("wikipedia") {
        return Some("wikipedia.org".to_string());
    } else if title_lower.contains("netflix") {
        return Some("netflix.com".to_string());
    } else if title_lower.contains("discord") {
        return Some("discord.com".to_string());
    } else if title_lower.contains("coursera") {
        return Some("coursera.org".to_string());
    } else if title_lower.contains("udemy") {
        return Some("udemy.com".to_string());
    } else if title_lower.contains("tiktok") {
        return Some("tiktok.com".to_string());
    } else if title_lower.contains("instagram") {
        return Some("instagram.com".to_string());
    } else if title_lower.contains("facebook") {
        return Some("facebook.com".to_string());
    }

    // 2. Pattern extraction for embedded domains in window title
    for token in
        title.split(|c: char| c.is_whitespace() || c == '-' || c == '—' || c == '|' || c == '•')
    {
        let clean = token
            .trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '-')
            .to_lowercase();
        let clean = clean
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .trim_start_matches("www.");
        if let Some(domain) = clean.split('/').next() {
            if domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.') {
                let parts: Vec<&str> = domain.split('.').collect();
                if parts.len() >= 2 {
                    let tld = parts.last().unwrap();
                    if KNOWN_TLDS.contains(tld)
                        && parts.iter().all(|p| {
                            !p.is_empty() && p.chars().all(|ch| ch.is_alphanumeric() || ch == '-')
                        })
                    {
                        return Some(domain.to_string());
                    }
                }
            }
        }
    }

    None
}
