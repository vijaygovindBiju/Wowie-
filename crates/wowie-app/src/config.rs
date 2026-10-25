use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use wowie_coach::models::tone::{PersonalityMode, TonePreferences};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub onboarding_completed: bool,
    pub sampling_interval_seconds: u64,
    pub ai_endpoint: Option<String>,
    pub ai_model: Option<String>,
    pub directness: f32,
    pub warmth: f32,
    pub humor: f32,
    pub persona_name: String,
    pub redact_window_titles: bool,
    pub collect_browser_origins: bool,
    pub distraction_threshold_samples: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            onboarding_completed: false,
            sampling_interval_seconds: 2,
            ai_endpoint: None,
            ai_model: None,
            directness: 0.6,
            warmth: 0.8,
            humor: 0.4,
            persona_name: "Wowie".to_string(),
            redact_window_titles: false,
            collect_browser_origins: true,
            distraction_threshold_samples: 2,
        }
    }
}

impl AppSettings {
    pub fn tone_preferences(&self) -> TonePreferences {
        TonePreferences::new(&self.persona_name, PersonalityMode::Supportive)
            .unwrap_or_else(|_| TonePreferences::supportive())
    }

    pub fn load_or_default(config_path: &Path) -> Self {
        if config_path.exists() {
            if let Ok(content) = fs::read_to_string(config_path) {
                if let Ok(settings) = serde_json::from_str(&content) {
                    return settings;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self, config_path: &Path) -> Result<(), std::io::Error> {
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        fs::write(config_path, json)
    }

    pub fn default_path() -> PathBuf {
        let base = std::env::var("XDG_CONFIG_HOME").map_or_else(
            |_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                PathBuf::from(home).join(".config")
            },
            PathBuf::from,
        );
        base.join("wowie").join("settings.json")
    }
}
