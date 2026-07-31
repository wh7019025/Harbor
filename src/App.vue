<script setup lang="ts">
import { onMounted } from "vue";
import TitleBar from "./components/TitleBar.vue";
import { initUiZoom } from "./lib/uiZoom";
import TaskClickView from "./views/TaskClickView.vue";

onMounted(() => {
  initUiZoom();
});
</script>

<template>
  <div class="flex h-screen flex-col overflow-hidden bg-[var(--bg-0)]">
    <TitleBar title="Harbor" />
    <main class="min-h-0 flex-1 overflow-auto">
      <TaskClickView />
    </main>
  </div>
</template>
