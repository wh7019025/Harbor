<script setup lang="ts">
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Fullscreen, Minimize2, Minus, Square, X } from "lucide-vue-next";
import { onBeforeUnmount, onMounted, ref } from "vue";

const props = withDefaults(
  defineProps<{
    title?: string;
    resizable?: boolean;
  }>(),
  {
    title: "Harbor",
    resizable: true,
  },
);

const appWindow = getCurrentWindow();
const isFullscreen = ref(false);
let unlistenResized: (() => void) | null = null;

async function syncFullscreen() {
  isFullscreen.value = await appWindow.isFullscreen();
}

async function drag() {
  await appWindow.startDragging();
}

async function minimize() {
  await appWindow.minimize();
}

async function toggleMaximize() {
  if (!props.resizable) return;
  await appWindow.toggleMaximize();
}

async function toggleFullscreen() {
  if (!props.resizable) return;
  const next = !isFullscreen.value;
  await appWindow.setFullscreen(next);
  isFullscreen.value = next;
}

async function close() {
  await appWindow.close();
}

onMounted(async () => {
  await syncFullscreen();
  unlistenResized = await appWindow.onResized(() => {
    void syncFullscreen();
  });
});

onBeforeUnmount(() => {
  unlistenResized?.();
});
</script>

<template>
  <header
    class="titlebar flex h-10 shrink-0 select-none items-center border-b border-[var(--line-soft)] bg-[var(--bg-1)]"
    @mousedown="drag"
  >
    <div class="flex min-w-0 flex-1 items-center gap-2 px-3.5">
      <span class="h-1.5 w-1.5 shrink-0 rounded-full bg-[var(--accent)]" />
      <span class="truncate text-[12px] font-medium tracking-wide text-[var(--muted)]">
        {{ title }}
      </span>
    </div>
    <div class="flex h-full shrink-0" @mousedown.stop>
      <button
        type="button"
        class="flex h-full w-11 items-center justify-center text-[var(--muted)] transition-colors duration-150 hover:bg-[var(--surface-hover)] hover:text-[var(--ink-bright)]"
        aria-label="Minimize"
        @click="minimize"
      >
        <Minus class="h-3.5 w-3.5" stroke-width="1.75" />
      </button>
      <button
        v-if="resizable"
        type="button"
        class="flex h-full w-11 items-center justify-center text-[var(--muted)] transition-colors duration-150 hover:bg-[var(--surface-hover)] hover:text-[var(--ink-bright)]"
        aria-label="Maximize"
        @click="toggleMaximize"
      >
        <Square class="h-3 w-3" stroke-width="1.75" />
      </button>
      <button
        v-if="resizable"
        type="button"
        class="flex h-full w-11 items-center justify-center text-[var(--muted)] transition-colors duration-150 hover:bg-[var(--surface-hover)] hover:text-[var(--ink-bright)]"
        :aria-label="isFullscreen ? 'Exit fullscreen' : 'Fullscreen'"
        :title="isFullscreen ? '退出全屏' : '全屏'"
        @click="toggleFullscreen"
      >
        <Minimize2 v-if="isFullscreen" class="h-3.5 w-3.5" stroke-width="1.75" />
        <Fullscreen v-else class="h-3.5 w-3.5" stroke-width="1.75" />
      </button>
      <button
        type="button"
        class="flex h-full w-11 items-center justify-center text-[var(--muted)] transition-colors duration-150 hover:bg-[#e81123] hover:text-white"
        aria-label="Close"
        @click="close"
      >
        <X class="h-3.5 w-3.5" stroke-width="1.75" />
      </button>
    </div>
  </header>
</template>
