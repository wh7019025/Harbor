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
import { clampPercent, formatBytes, formatBytesPerSecond, formatPercent } from "../lib/utils";

const metrics = ref<SystemMetrics | null>(null);
let fastTimer: number | undefined;
let slowTimer: number | undefined;

const valueWidth: Record<string, string> = {
  CPU: "3.25rem",
  MEM: "7.75rem",
  DISK: "3.25rem",
  RX: "4.5rem",
  TX: "4.5rem",
  GPU: "10.75rem",
};

const cells = computed(() => {
  const m = metrics.value;
  const gpu = m?.gpus[0];
  const gpuTemp = gpu?.temperature_c != null ? `${gpu.temperature_c.toFixed(0)}°C` : "--";

  return [
    { label: "CPU", value: formatPercent(clampPercent(m?.cpu_usage_percent)) },
    {
      label: "MEM",
      value: m
        ? `${formatPercent(clampPercent(m.memory.usage_percent))} ${formatBytes(m.memory.used_bytes)}/${formatBytes(m.memory.total_bytes)}`
        : "--",
    },
    { label: "DISK", value: formatPercent(clampPercent(m?.root_disk.usage_percent)) },
    { label: "RX", value: formatBytesPerSecond(m?.network_rx_bytes_per_sec) },
    { label: "TX", value: formatBytesPerSecond(m?.network_tx_bytes_per_sec) },
    {
      label: "GPU",
      value: gpu
        ? `${formatPercent(clampPercent(gpu.utilization_percent))} ${formatBytes(gpu.memory_used_bytes)}/${formatBytes(gpu.memory_total_bytes)} ${gpuTemp}`
        : "--",
    },
  ];
});

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
  try {
    metrics.value = await getSystemMetrics();
  } catch {
    // Keep placeholder values when metrics are unavailable.
  }
  fastTimer = window.setInterval(async () => {
    try {
      mergeFastMetrics(await getFastSystemMetrics());
    } catch {
      // Keep last values when sample fails.
    }
  }, settings?.metrics_fast_ms ?? 1000);
  slowTimer = window.setInterval(async () => {
    try {
      mergeSlowMetrics(await getSlowSystemMetrics());
    } catch {
      // Keep last values when sample fails.
    }
  }, settings?.metrics_slow_ms ?? 10000);
});

onBeforeUnmount(() => {
  if (fastTimer != null) window.clearInterval(fastTimer);
  if (slowTimer != null) window.clearInterval(slowTimer);
});
</script>

<template>
  <footer class="shrink-0 border-t border-[var(--line-soft)] pt-1.5">
    <div class="flex w-fit max-w-full flex-wrap items-baseline gap-x-3.5 gap-y-1 text-[10px] leading-none">
      <div v-for="cell in cells" :key="cell.label" class="flex items-baseline gap-1">
        <span class="readout shrink-0 text-[var(--faint)]">{{ cell.label }}</span>
        <span
          class="readout inline-block truncate tabular-nums text-[var(--muted)]"
          :style="{ width: valueWidth[cell.label] }"
          :title="cell.value"
        >
          {{ cell.value }}
        </span>
      </div>
    </div>
  </footer>
</template>
