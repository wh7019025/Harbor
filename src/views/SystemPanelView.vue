<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { getSettings } from "../api/settings";
import {
  getFastSystemMetrics,
  getSlowSystemMetrics,
  getSystemMetrics,
  type FastSystemMetrics,
  type SlowSystemMetrics,
  type SystemMetrics,
} from "../api/systemMetrics";
import {
  clampPercent,
  formatBytes,
  formatBytesPerSecond,
  formatFrequency,
  formatPercent,
} from "../lib/utils";

const metrics = ref<SystemMetrics | null>(null);
const loading = ref(true);
const error = ref("");
let fastTimer: number | undefined;
let slowTimer: number | undefined;

const cpuPercent = computed(() => clampPercent(metrics.value?.cpu_usage_percent ?? null));
const rxRate = computed(() => metrics.value?.network_rx_bytes_per_sec ?? null);
const txRate = computed(() => metrics.value?.network_tx_bytes_per_sec ?? null);
const memoryPercent = computed(() => clampPercent(metrics.value?.memory.usage_percent ?? null));
const swapPercent = computed(() => clampPercent(metrics.value?.swap.usage_percent ?? null));
const diskPercent = computed(() => clampPercent(metrics.value?.root_disk.usage_percent ?? null));
const totalRate = computed(() => {
  if (rxRate.value == null && txRate.value == null) return null;
  return (rxRate.value ?? 0) + (txRate.value ?? 0);
});

const cells = computed(() => [
  { label: "CPU", value: formatPercent(cpuPercent.value), detail: "" },
  {
    label: "MEM",
    value: formatPercent(memoryPercent.value),
    detail: `${formatBytes(metrics.value?.memory.used_bytes)} / ${formatBytes(metrics.value?.memory.total_bytes)}`,
  },
  {
    label: "SWAP",
    value: formatPercent(swapPercent.value),
    detail: `${formatBytes(metrics.value?.swap.used_bytes)} / ${formatBytes(metrics.value?.swap.total_bytes)}`,
  },
  {
    label: "DISK",
    value: formatPercent(diskPercent.value),
    detail: `${formatBytes(metrics.value?.root_disk.used_bytes)} / ${formatBytes(metrics.value?.root_disk.total_bytes)}`,
  },
  {
    label: "RX",
    value: formatBytesPerSecond(rxRate.value),
    detail: formatBytes(metrics.value?.network_rx_total_bytes),
  },
  {
    label: "TX",
    value: formatBytesPerSecond(txRate.value),
    detail: formatBytes(metrics.value?.network_tx_total_bytes),
  },
  {
    label: "NET",
    value: formatBytesPerSecond(totalRate.value),
    detail: "",
  },
]);

async function loadMetrics() {
  try {
    metrics.value = await getSystemMetrics();
    error.value = "";
  } catch (err) {
    error.value = err instanceof Error ? err.message : "指标读取失败";
  } finally {
    loading.value = false;
  }
}

async function loadFastMetrics() {
  try {
    mergeFastMetrics(await getFastSystemMetrics());
    error.value = "";
  } catch (err) {
    error.value = err instanceof Error ? err.message : "指标读取失败";
  }
}

async function loadSlowMetrics() {
  try {
    mergeSlowMetrics(await getSlowSystemMetrics());
    error.value = "";
  } catch (err) {
    error.value = err instanceof Error ? err.message : "指标读取失败";
  }
}

function mergeFastMetrics(next: FastSystemMetrics) {
  if (!metrics.value) return;
  metrics.value = {
    ...metrics.value,
    timestamp_ms: next.timestamp_ms,
    cpu_usage_percent: next.cpu_usage_percent,
    cpu_cores: next.cpu_cores,
    network_rx_bytes_per_sec: next.network_rx_bytes_per_sec,
    network_tx_bytes_per_sec: next.network_tx_bytes_per_sec,
    network_rx_total_bytes: next.network_rx_total_bytes,
    network_tx_total_bytes: next.network_tx_total_bytes,
    gpus: next.gpus,
  };
}

function formatTemp(value: number | null | undefined) {
  if (value == null) return "--";
  return `${value.toFixed(0)}°C`;
}

function mergeSlowMetrics(next: SlowSystemMetrics) {
  if (!metrics.value) return;
  metrics.value = {
    ...metrics.value,
    memory: next.memory,
    swap: next.swap,
    root_disk: next.root_disk,
  };
}

onMounted(async () => {
  const settings = await getSettings().catch(() => null);
  await loadMetrics();
  fastTimer = window.setInterval(loadFastMetrics, settings?.metrics_fast_ms ?? 1000);
  slowTimer = window.setInterval(loadSlowMetrics, settings?.metrics_slow_ms ?? 10000);
});

onBeforeUnmount(() => {
  if (fastTimer != null) window.clearInterval(fastTimer);
  if (slowTimer != null) window.clearInterval(slowTimer);
});
</script>

<template>
  <section class="st-shell flex h-full w-full flex-col gap-3 p-3">
    <p
      v-if="error"
      class="shrink-0 border border-[color-mix(in_srgb,var(--danger)_40%,var(--line))] px-2 py-1.5 text-sm text-[#f48771]"
    >
      {{ error }}
    </p>

    <div class="metrics-auto shrink-0">
      <div v-for="cell in cells" :key="cell.label" class="metric-tile min-w-0">
        <p class="readout text-[11px] text-[var(--faint)]">{{ cell.label }}</p>
        <p class="metric-value readout mt-1.5 font-medium tracking-tight text-[var(--ink-bright)]">
          {{ loading ? "--" : cell.value }}
        </p>
        <p v-if="cell.detail" class="readout mt-1 truncate text-xs text-[var(--muted)]">
          {{ cell.detail }}
        </p>
      </div>
    </div>

    <div
      v-if="(metrics?.gpus.length ?? 0) > 0"
      class="metrics-auto shrink-0"
    >
      <div v-for="gpu in metrics?.gpus ?? []" :key="gpu.index" class="metric-tile min-w-0">
        <p class="readout truncate text-[11px] text-[var(--faint)]" :title="gpu.name">
          GPU{{ gpu.index }} · {{ gpu.name }}
        </p>
        <p class="metric-value readout mt-1.5 font-medium tracking-tight text-[var(--ink-bright)]">
          {{ loading ? "--" : formatPercent(clampPercent(gpu.utilization_percent)) }}
        </p>
        <p class="readout mt-1 truncate text-xs text-[var(--muted)]">
          {{ formatBytes(gpu.memory_used_bytes) }} / {{ formatBytes(gpu.memory_total_bytes) }}
          · {{ formatPercent(clampPercent(gpu.memory_usage_percent)) }}
          · {{ formatTemp(gpu.temperature_c) }}
        </p>
      </div>
    </div>

    <div class="flex min-h-0 flex-1 flex-col rounded-lg border border-[var(--line-soft)] bg-[var(--bg-1)] px-3 py-2.5">
      <div class="mb-2 flex shrink-0 items-baseline justify-between">
        <span class="kicker">cores</span>
        <span class="readout text-xs text-[var(--faint)]">{{ metrics?.cpu_cores.length ?? 0 }}</span>
      </div>
      <div class="cores-auto min-h-0 flex-1 content-start overflow-auto">
        <div
          v-for="core in metrics?.cpu_cores ?? []"
          :key="core.id"
          class="core-chip flex min-w-0 items-baseline gap-2 rounded-md border border-[var(--line-soft)] bg-[var(--bg-0)] px-2.5 py-2"
        >
          <span class="readout shrink-0 text-[11px] text-[var(--faint)]">C{{ core.id }}</span>
          <span class="readout text-sm text-[var(--ink-bright)]">
            {{ formatPercent(clampPercent(core.usage_percent)) }}
          </span>
          <span class="readout ml-auto truncate text-[11px] text-[var(--muted)]">
            {{ formatFrequency(core.frequency_mhz) }}
          </span>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.metrics-auto {
  display: grid;
  gap: 0.75rem;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 148px), 1fr));
}

.cores-auto {
  display: grid;
  gap: 0.5rem;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 132px), 1fr));
  align-content: start;
}

.metric-value {
  font-size: clamp(1.2rem, 2.4vw + 0.4rem, 2rem);
}
</style>
