import { invoke } from "@tauri-apps/api/core";

export type PathOpenTarget = "file_manager" | "vscode" | "cursor";

export interface PathOpeners {
  file_manager: boolean;
  vscode: boolean;
  cursor: boolean;
}

const STORAGE_KEY = "harbor.pathOpener";

let cachedOpeners: PathOpeners | null = null;

export function fetchPathOpeners() {
  if (cachedOpeners) return Promise.resolve(cachedOpeners);
  return invoke<PathOpeners>("path_openers").then((openers) => {
    cachedOpeners = openers;
    return openers;
  });
}

export function openPath(path: string, target: PathOpenTarget) {
  return invoke<void>("path_open", { path, target });
}

export function resolveConfigBasePath(prefixPath: string) {
  return invoke<string>("taskcard_resolve_config_base_path", { prefixPath });
}

export function loadPreferredOpener(openers: PathOpeners): PathOpenTarget {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved === "file_manager" && openers.file_manager) return "file_manager";
    if (saved === "vscode" && openers.vscode) return "vscode";
    if (saved === "cursor" && openers.cursor) return "cursor";
  } catch {
    // ignore storage errors
  }
  if (openers.cursor) return "cursor";
  if (openers.vscode) return "vscode";
  return "file_manager";
}

export function savePreferredOpener(target: PathOpenTarget) {
  localStorage.setItem(STORAGE_KEY, target);
}
