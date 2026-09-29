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
const emit = defineEmits<{
  fullscreenChange: [fullscreen: boolean];
}>();

const appWindow = getCurrentWindow();
const isFullscreen = ref(false);
let unlistenResized: (() => void) | null = null;
let dragOrigin: { x: number; y: number } | null = null;
let dragStarted = false;

async function syncFullscreen() {
  isFullscreen.value = await appWindow.isFullscreen();
  emit("fullscreenChange", isFullscreen.value);
}

function beginDrag(event: MouseEvent) {
  if (event.button !== 0 || isFullscreen.value) return;
  dragOrigin = { x: event.screenX, y: event.screenY };
  dragStarted = false;
}

function continueDrag(event: MouseEvent) {
  if (!dragOrigin || dragStarted || event.buttons !== 1) return;
  const distance = Math.hypot(event.screenX - dragOrigin.x, event.screenY - dragOrigin.y);
  if (distance < 4) return;
  dragStarted = true;
  dragOrigin = null;
  void appWindow.startDragging().catch(endDrag);
}

function endDrag() {
  dragOrigin = null;
  dragStarted = false;
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
  await syncFullscreen();
}

async function close() {
  await appWindow.close();
}

onMounted(async () => {
  window.addEventListener("mousemove", continueDrag);
  window.addEventListener("mouseup", endDrag);
  window.addEventListener("blur", endDrag);
  await syncFullscreen();
  unlistenResized = await appWindow.onResized(() => {
    void syncFullscreen();
  });
});

onBeforeUnmount(() => {
  window.removeEventListener("mousemove", continueDrag);
  window.removeEventListener("mouseup", endDrag);
  window.removeEventListener("blur", endDrag);
  unlistenResized?.();
});
</script>

<template>
  <header
    class="titlebar flex h-10 shrink-0 select-none items-center border-b border-[var(--line-soft)] bg-[var(--bg-1)]"
    @mousedown="beginDrag"
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
