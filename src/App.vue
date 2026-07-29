<script setup lang="ts">
import { computed, onMounted } from "vue";
import TitleBar from "./components/TitleBar.vue";
import { initUiZoom } from "./lib/uiZoom";
import { currentWindowLabel } from "./lib/windows";
import AgentHelpView from "./views/AgentHelpView.vue";
import LauncherView from "./views/LauncherView.vue";
import SettingView from "./views/SettingView.vue";
import SystemPanelView from "./views/SystemPanelView.vue";
import TaskClickView from "./views/TaskClickView.vue";

const label = computed(() => currentWindowLabel());

const title = computed(() => {
  switch (label.value) {
    case "system-panel":
      return "SystemPanel";
    case "task-click":
      return "TaskClick";
    case "setting":
      return "Setting";
    case "agent-help":
      return "AgentHelp";
    default:
      return "Harbor";
  }
});

const resizable = computed(() => label.value !== "launcher");

onMounted(() => {
  initUiZoom();
});
</script>

<template>
  <div class="flex h-screen flex-col overflow-hidden bg-[var(--bg-0)]">
    <TitleBar :title="title" :resizable="resizable" />
    <main class="min-h-0 flex-1 overflow-auto">
      <LauncherView v-if="label === 'launcher'" />
      <SystemPanelView v-else-if="label === 'system-panel'" />
      <TaskClickView v-else-if="label === 'task-click'" />
      <SettingView v-else-if="label === 'setting'" />
      <AgentHelpView v-else-if="label === 'agent-help'" />
      <LauncherView v-else />
    </main>
  </div>
</template>
