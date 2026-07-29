import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

export type FeatureWindow = "system-panel" | "task-click" | "setting" | "agent-help";

const windowConfig: Record<
  FeatureWindow,
  { title: string; width: number; height: number }
> = {
  "system-panel": { title: "SystemPanel", width: 720, height: 460 },
  "task-click": { title: "TaskClick", width: 1180, height: 820 },
  setting: { title: "Setting", width: 520, height: 600 },
  "agent-help": { title: "AgentHelp", width: 820, height: 720 },
};

function featureUrl(label: FeatureWindow) {
  return `${window.location.origin}/?window=${label}`;
}

export async function openFeatureWindow(label: FeatureWindow) {
  const existing = await WebviewWindow.getByLabel(label);
  if (existing) {
    await existing.close();
  }

  const config = windowConfig[label];
  const created = new WebviewWindow(label, {
    url: featureUrl(label),
    title: config.title,
    width: config.width,
    height: config.height,
    center: true,
    resizable: true,
    decorations: false,
    focus: true,
  });

  created.once("tauri://error", (event) => {
    console.error("failed to open window", label, event);
  });
}

export function currentWindowLabel(): string {
  const params = new URLSearchParams(window.location.search);
  return params.get("window") || "launcher";
}
