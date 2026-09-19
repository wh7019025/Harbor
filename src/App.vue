<script setup lang="ts">
import { onMounted } from "vue";
import TitleBar from "./components/TitleBar.vue";
import { initUiZoom } from "./lib/uiZoom";
import PanelView from "./views/PanelView.vue";
import TaskClickView from "./views/TaskClickView.vue";

const panel = window.__HARBOR_PANEL__ ?? null;

onMounted(() => {
  initUiZoom();
});
</script>

<template>
  <div class="flex h-screen flex-col overflow-hidden bg-[var(--bg-0)]">
    <TitleBar :title="panel?.title || 'Harbor'" />
    <main class="flex min-h-0 flex-1 flex-col overflow-hidden">
      <PanelView v-if="panel" :title="panel.title" :url="panel.url" />
      <TaskClickView v-else />
    </main>
  </div>
</template>
