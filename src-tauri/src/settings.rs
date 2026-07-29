use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub taskcard_root: String,
    #[serde(default)]
    pub search_paths: Vec<String>,
    pub metrics_fast_ms: u64,
    pub metrics_slow_ms: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            taskcard_root: default_taskcard_root().display().to_string(),
            search_paths: Vec::new(),
            metrics_fast_ms: 1000,
            metrics_slow_ms: 10000,
        }
    }
}

pub fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

pub fn config_dir() -> PathBuf {
    home_dir().join(".superterm")
}

pub fn legacy_settings_path() -> PathBuf {
    home_dir().join(".config").join("superterm").join("settings.json")
}

pub fn default_taskcard_root() -> PathBuf {
    home_dir().join(".superterm").join("st_taskcfg")
}

pub fn load_settings() -> Settings {
    let path = settings_path();
    if let Ok(raw) = fs::read_to_string(&path) {
        return serde_json::from_str(&raw).unwrap_or_default();
    }
    let legacy = legacy_settings_path();
    if let Ok(raw) = fs::read_to_string(&legacy) {
        let settings = serde_json::from_str(&raw).unwrap_or_default();
        let _ = save_settings(&settings);
        return settings;
    }
    Settings::default()
}

pub fn save_settings(settings: &Settings) -> Result<(), String> {
    let dir = config_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("create config dir failed: {e}"))?;
    let path = settings_path();
    let raw = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(&path, raw).map_err(|e| format!("write settings failed: {e}"))
}

pub fn expand_path(value: &str) -> PathBuf {
    let value = value.trim();
    if value == "~" {
        return home_dir();
    }
    if let Some(rest) = value.strip_prefix("~/") {
        return home_dir().join(rest);
    }
    PathBuf::from(value)
}

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}
