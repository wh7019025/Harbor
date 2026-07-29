<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { getMiniMetrics } from "../api/systemMetrics";
import { clampPercent, formatPercent } from "../lib/utils";
import { openFeatureWindow, type FeatureWindow } from "../lib/windows";

const cpu = ref<number | null>(null);
const mem = ref<number | null>(null);
const opening = ref<string | null>(null);
const entered = ref(false);
let timer: number | undefined;

const entries: Array<{
  id: FeatureWindow;
  index: string;
  title: string;
}> = [
  { id: "system-panel", index: "01", title: "SystemPanel" },
  { id: "task-click", index: "02", title: "TaskClick" },
  { id: "setting", index: "03", title: "Setting" },
  { id: "agent-help", index: "04", title: "AgentHelp" },
];

async function refreshMini() {
  try {
    const metrics = await getMiniMetrics();
    cpu.value = clampPercent(metrics.cpu_usage_percent);
    mem.value = clampPercent(metrics.memory_usage_percent);
  } catch {
    // Keep last values when sample fails.
  }
}

async function open(id: FeatureWindow) {
  opening.value = id;
  try {
    await openFeatureWindow(id);
  } finally {
    window.setTimeout(() => {
      opening.value = null;
    }, 240);
  }
}

onMounted(() => {
  requestAnimationFrame(() => {
    entered.value = true;
  });
  void refreshMini();
  timer = window.setInterval(refreshMini, 1500);
});

onBeforeUnmount(() => {
  if (timer != null) window.clearInterval(timer);
});
</script>

<template>
  <div class="st-shell flex h-full flex-col px-6 pb-5 pt-5">
    <header
      class="transition-all duration-500 ease-out"
      :class="entered ? 'translate-y-0 opacity-100' : 'translate-y-2 opacity-0'"
    >
      <div class="flex items-center gap-2.5">
        <span class="h-2 w-2 rounded-full bg-[var(--accent)] shadow-[0_0_0_3px_var(--accent-soft)]" />
        <h1 class="text-[1.85rem] font-semibold tracking-[-0.03em] text-[var(--ink-bright)]">
          Harbor
        </h1>
      </div>
    </header>

    <nav class="mt-7 flex flex-1 flex-col gap-1.5">
      <button
        v-for="(entry, index) in entries"
        :key="entry.id"
        type="button"
        class="nav-row"
        :class="[
          entered ? 'translate-y-0 opacity-100' : 'translate-y-2 opacity-0',
          opening === entry.id ? 'is-active' : '',
        ]"
        :style="{
          transitionDelay: entered ? `${80 + index * 50}ms` : '0ms',
          transitionProperty: 'background, color, transform, opacity, translate',
        }"
        @click="open(entry.id)"
      >
        <span class="nav-index readout text-[11px] text-[var(--faint)]">
          {{ entry.index }}
        </span>
        <span class="text-[15px] font-medium tracking-tight text-[var(--ink-bright)]">
          {{ entry.title }}
        </span>
        <span class="nav-chevron">›</span>
      </button>
    </nav>

    <footer
      class="mt-5 rounded-lg border border-[var(--line-soft)] bg-[var(--bg-1)] px-3.5 py-3 transition-all duration-500"
      :class="entered ? 'translate-y-0 opacity-100' : 'translate-y-2 opacity-0'"
      style="transition-delay: 220ms"
    >
      <div class="grid grid-cols-2 gap-4">
        <div>
          <div class="mb-1.5 flex items-baseline justify-between">
            <span class="readout text-[11px] text-[var(--faint)]">CPU</span>
            <span class="readout text-xs text-[var(--ink)]">{{ formatPercent(cpu) }}</span>
          </div>
          <div class="bar">
            <span :style="{ width: `${cpu ?? 0}%` }" />
          </div>
        </div>
        <div>
          <div class="mb-1.5 flex items-baseline justify-between">
            <span class="readout text-[11px] text-[var(--faint)]">MEM</span>
            <span class="readout text-xs text-[var(--ink)]">{{ formatPercent(mem) }}</span>
          </div>
          <div class="bar">
            <span class="alt" :style="{ width: `${mem ?? 0}%` }" />
          </div>
        </div>
      </div>
    </footer>
  </div>
</template>
