<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import {
  getHarborCopyProgress,
  getHarborCoreStatus,
  getSettings,
  type HarborCopyProgress,
  type HarborCoreStatus,
} from "../api/settings";
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
const coreStatus = ref<HarborCoreStatus | null>(null);
const deployProgress = ref<HarborCopyProgress | null>(null);
let fastTimer: number | undefined;
let slowTimer: number | undefined;
let coreTimer: number | undefined;
let pollingCore = false;

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

const coreConnection = computed(() => {
  const status = coreStatus.value;
  const target = connectionTarget(status?.listen_url);
  if (deployProgress.value?.active) {
    return {
      text: `正在连接 ${target} · 部署 ${deployProgress.value.percent}%`,
      tone: "text-[var(--accent)]",
      dot: "bg-[var(--accent)] animate-pulse",
    };
  }
  if (!status) {
    return { text: `正在连接 ${target}`, tone: "text-[var(--faint)]", dot: "bg-[var(--faint)]" };
  }
  if (status.reachable && status.compatible) {
    return { text: `已连接 ${target} · 正常`, tone: "text-[var(--muted)]", dot: "bg-[var(--running)]" };
  }
  if (status.reachable) {
    return { text: `已连接 ${target} · 版本不符`, tone: "text-[var(--warn)]", dot: "bg-[var(--warn)]" };
  }
  return { text: `未连接 ${target} · 已关闭`, tone: "text-[var(--danger)]", dot: "bg-[var(--danger)]" };
});

function connectionTarget(listenUrl?: string | null) {
  if (!listenUrl) return "localhost";
  try {
    const hostname = new URL(listenUrl).hostname;
    return hostname === "127.0.0.1" || hostname === "::1" ? "localhost" : hostname;
  } catch {
    return listenUrl;
  }
}

async function pollCoreStatus() {
  if (pollingCore) return;
  pollingCore = true;
  try {
    const [status, progress] = await Promise.all([
      getHarborCoreStatus(),
      getHarborCopyProgress(),
    ]);
    coreStatus.value = status;
    deployProgress.value = progress;
  } catch {
    coreStatus.value = null;
  } finally {
    pollingCore = false;
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
  void pollCoreStatus();
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
  coreTimer = window.setInterval(() => {
    void pollCoreStatus();
  }, 2500);
});

onBeforeUnmount(() => {
  if (fastTimer != null) window.clearInterval(fastTimer);
  if (slowTimer != null) window.clearInterval(slowTimer);
  if (coreTimer != null) window.clearInterval(coreTimer);
});
</script>

<template>
  <footer class="shrink-0 border-t border-[var(--line-soft)] pt-1.5">
    <div class="flex min-w-0 items-baseline justify-between gap-3 text-[10px] leading-none">
      <div class="flex min-w-0 flex-wrap items-baseline gap-x-3.5 gap-y-1">
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
      <div
        class="readout ml-auto flex shrink-0 items-center gap-1.5 whitespace-nowrap text-right"
        :class="coreConnection.tone"
        :title="coreStatus?.error ?? coreStatus?.listen_url ?? ''"
      >
        <span class="h-1.5 w-1.5 rounded-full" :class="coreConnection.dot" />
        <span>{{ coreConnection.text }}</span>
      </div>
    </div>
  </footer>
</template>
