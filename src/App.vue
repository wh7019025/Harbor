<script setup lang="ts">
import { getCurrentWindow } from "@tauri-apps/api/window";
import { register, unregister } from "@tauri-apps/plugin-global-shortcut";
import { onBeforeUnmount, onMounted, ref } from "vue";
import TitleBar from "./components/TitleBar.vue";
import { initUiZoom } from "./lib/uiZoom";
import PanelView from "./views/PanelView.vue";
import TaskClickView from "./views/TaskClickView.vue";

const panel = window.__HARBOR_PANEL__ ?? null;
const appWindow = getCurrentWindow();
const isFullscreen = ref(false);
let escapeShortcutRegistered = false;

async function leaveFullscreen() {
  if (!isFullscreen.value) return;
  await appWindow.setFullscreen(false);
  await syncFullscreen(false);
}

async function exitFullscreen(event: KeyboardEvent) {
  if (event.key !== "Escape" || !isFullscreen.value) return;
  event.preventDefault();
  await leaveFullscreen();
}

async function syncFullscreen(fullscreen: boolean) {
  isFullscreen.value = fullscreen;

  if (fullscreen && !escapeShortcutRegistered) {
    await register("Escape", (event) => {
      if (event.state === "Pressed") void leaveFullscreen();
    });
    escapeShortcutRegistered = true;
    return;
  }

  if (!fullscreen && escapeShortcutRegistered) {
    await unregister("Escape");
    escapeShortcutRegistered = false;
  }
}

onMounted(() => {
  initUiZoom();
  window.addEventListener("keydown", exitFullscreen, { capture: true });
});

onBeforeUnmount(async () => {
  window.removeEventListener("keydown", exitFullscreen, { capture: true });
  if (escapeShortcutRegistered) await unregister("Escape");
});
</script>

<template>
  <div class="flex h-screen flex-col overflow-hidden bg-[var(--bg-0)]">
    <TitleBar
      v-show="!isFullscreen"
      :title="panel?.title || 'Harbor'"
      @fullscreen-change="syncFullscreen"
    />
    <main class="flex min-h-0 flex-1 flex-col overflow-hidden">
      <PanelView v-if="panel" :title="panel.title" :url="panel.url" />
      <TaskClickView v-else />
    </main>
  </div>
</template>
