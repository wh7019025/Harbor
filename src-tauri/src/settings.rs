use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
            taskcard_root: default_taskcard_root_display(),
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
    home_dir().join(".harbor")
}

fn legacy_superterm_dir() -> PathBuf {
    home_dir().join(".superterm")
}

pub fn default_taskcard_root() -> PathBuf {
    config_dir().join("harbor_taskcfg")
}

fn default_taskcard_root_display() -> String {
    "~/.harbor/harbor_taskcfg".to_string()
}

pub fn migrate_legacy_layout() -> Result<(), String> {
    migrate_legacy_taskcfg_dir()?;
    migrate_settings_file()?;
    Ok(())
}

fn migrate_legacy_taskcfg_dir() -> Result<(), String> {
    let harbor_root = default_taskcard_root();
    if harbor_root.exists() {
        return Ok(());
    }
    let candidates = [
        legacy_superterm_dir().join("harbor_taskcfg"),
        legacy_superterm_dir().join("st_taskcfg"),
        config_dir().join("st_taskcfg"),
        config_dir().join("harbor_taskcfg"),
    ];
    for old in candidates {
        if !old.is_dir() || old == harbor_root {
            continue;
        }
        if let Some(parent) = harbor_root.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("create {} failed: {e}", parent.display()))?;
        }
        fs::rename(&old, &harbor_root).map_err(|e| {
            format!(
                "rename {} -> {} failed: {e}",
                old.display(),
                harbor_root.display()
            )
        })?;
        return Ok(());
    }
    Ok(())
}

fn migrate_settings_file() -> Result<(), String> {
    let path = settings_path();
    if !path.is_file() {
        return Ok(());
    }
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut settings: Settings = serde_json::from_str(&raw).unwrap_or_default();
    let before = settings.clone();

    settings.taskcard_root = normalize_taskcard_root(settings.taskcard_root.as_str());
    settings.search_paths = settings
        .search_paths
        .into_iter()
        .map(|item| normalize_search_path(item.as_str()))
        .collect();

    if settings != before {
        save_settings(&settings)?;
    }
    Ok(())
}

fn normalize_taskcard_root(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return default_taskcard_root_display();
    }
    let expanded = expand_path(trimmed);
    let legacy_roots = [
        legacy_superterm_dir().join("harbor_taskcfg"),
        legacy_superterm_dir().join("st_taskcfg"),
        config_dir().join("st_taskcfg"),
    ];
    if legacy_roots.iter().any(|path| expanded == *path) || trimmed.contains(".superterm") {
        return default_taskcard_root_display();
    }
    if trimmed.contains("st_taskcfg") {
        return trimmed.replace("st_taskcfg", "harbor_taskcfg");
    }
    trimmed.to_string()
}

fn normalize_search_path(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.contains("SuperTerm") {
        let harbor_path = trimmed.replace("SuperTerm", "Harbor");
        if PathBuf::from(&harbor_path).is_dir() {
            return harbor_path;
        }
    }
    trimmed.to_string()
}

pub fn load_settings() -> Settings {
    let _ = migrate_legacy_layout();
    let path = settings_path();
    if let Ok(raw) = fs::read_to_string(&path) {
        return serde_json::from_str(&raw).unwrap_or_default();
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
    expand_path_with_base(value, &home_dir())
}

pub fn expand_path_with_base(value: &str, base: &Path) -> PathBuf {
    let value = value.trim();
    if value.is_empty() {
        return base.to_path_buf();
    }
    if value == "~" {
        return home_dir();
    }
    if let Some(rest) = value.strip_prefix("~/") {
        return home_dir().join(rest);
    }
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        base.join(path)
    }
}

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_path_with_base_supports_tilde_and_relative() {
        let home = home_dir();
        assert_eq!(expand_path_with_base("~", &home), home);
        assert_eq!(
            expand_path_with_base("~/projects/foo", &home),
            home.join("projects/foo")
        );
        assert_eq!(
            expand_path_with_base("relative/dir", &home),
            home.join("relative/dir")
        );
        assert_eq!(
            expand_path_with_base("/abs/path", &home),
            PathBuf::from("/abs/path")
        );
    }

    #[test]
    fn normalize_taskcard_root_from_superterm() {
        assert_eq!(
            normalize_taskcard_root("~/.superterm/harbor_taskcfg"),
            default_taskcard_root_display()
        );
        assert_eq!(
            normalize_taskcard_root("~/.superterm/st_taskcfg"),
            default_taskcard_root_display()
        );
    }

    #[test]
    fn normalize_search_path_superterm_to_harbor() {
        let harbor = home_dir().join("Harbor");
        if harbor.is_dir() {
            assert_eq!(
                normalize_search_path("/home/se/SuperTerm"),
                harbor.display().to_string()
            );
        }
    }
}
