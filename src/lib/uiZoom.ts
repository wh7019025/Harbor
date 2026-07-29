import { getCurrentWebview } from "@tauri-apps/api/webview";

const STORAGE_KEY = "harbor.uiZoom";
const MIN = 0.7;
const MAX = 2;
const STEP = 0.1;

let zoom = 1;

function clamp(value: number) {
  return Math.round(Math.min(MAX, Math.max(MIN, value)) * 100) / 100;
}

async function apply(next: number) {
  zoom = clamp(next);
  localStorage.setItem(STORAGE_KEY, String(zoom));
  try {
    await getCurrentWebview().setZoom(zoom);
  } catch {
    document.documentElement.style.zoom = String(zoom);
  }
}

function onKeyDown(event: KeyboardEvent) {
  if (!(event.ctrlKey || event.metaKey) || event.altKey) return;
  const key = event.key;
  if (key === "+" || key === "=" || key === "Add") {
    event.preventDefault();
    void apply(zoom + STEP);
    return;
  }
  if (key === "-" || key === "_" || key === "Subtract") {
    event.preventDefault();
    void apply(zoom - STEP);
    return;
  }
  if (key === "0") {
    event.preventDefault();
    void apply(1);
  }
}

function onWheel(event: WheelEvent) {
  if (!(event.ctrlKey || event.metaKey)) return;
  event.preventDefault();
  void apply(zoom + (event.deltaY < 0 ? STEP : -STEP));
}

export function initUiZoom() {
  const saved = Number(localStorage.getItem(STORAGE_KEY));
  if (Number.isFinite(saved)) {
    zoom = clamp(saved);
  }
  void apply(zoom);
  window.addEventListener("keydown", onKeyDown);
  window.addEventListener("wheel", onWheel, { passive: false });
}
