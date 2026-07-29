import { invoke } from "@tauri-apps/api/core";

export interface Settings {
  taskcard_root: string;
  search_paths: string[];
  metrics_fast_ms: number;
  metrics_slow_ms: number;
}

export function getSettings() {
  return invoke<Settings>("get_settings");
}

export function updateSettings(next: Settings) {
  return invoke<Settings>("update_settings", { next });
}

export function getAppVersion() {
  return invoke<string>("app_version");
}
