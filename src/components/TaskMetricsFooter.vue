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
  getPerformanceMetrics,
  getResourceMetrics,
  getSystemMetrics,
  type PerformanceMetrics,
  type ResourceMetrics,
  type SystemMetrics,
} from "../api/systemMetrics";
import { clampPercent, formatBytes, formatBytesPerSecond, formatPercent } from "../lib/utils";

const metrics = ref<SystemMetrics | null>(null);
const coreStatus = ref<HarborCoreStatus | null>(null);
const deployProgress = ref<HarborCopyProgress | null>(null);
let performanceTimer: number | undefined;
let resourceTimer: number | undefined;
let coreTimer: number | undefined;
let pollingCore = false;
let pollingSystemMetrics = false;
let pollingPerformanceMetrics = false;
let pollingResourceMetrics = false;

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
    const artifact = deployProgress.value.artifact === "ttyd" ? "ttyd" : "harbor_core";
    return {
      text: `正在连接 ${target} · 部署 ${artifact} ${deployProgress.value.percent}%`,
      tone: "text-[var(--accent)]",
      dot: "bg-[var(--accent)] animate-pulse",
    };
  }
  if (!status) {
    return { text: `正在连接 ${target}`, tone: "text-[var(--faint)]", dot: "bg-[var(--faint)]" };
  }
  if (status.reachable && status.compatible && status.connected) {
    return { text: `已连接 ${target} · 正常`, tone: "text-[var(--muted)]", dot: "bg-[var(--running)]" };
  }
  if (status.reachable && status.access_occupied && status.access_owner_version) {
    return {
      text: `已连接 ${target} · Harbor ${status.access_owner_version} 使用中`,
      tone: "text-[var(--warn)]",
      dot: "bg-[var(--warn)]",
    };
  }
  if (status.reachable && status.compatible) {
    return { text: `可连接 ${target} · 等待连接`, tone: "text-[var(--faint)]", dot: "bg-[var(--faint)]" };
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
    const previousSource = metricSourceKey(coreStatus.value);
    const [status, progress] = await Promise.all([
      getHarborCoreStatus(),
      getHarborCopyProgress(),
    ]);
    coreStatus.value = status;
    deployProgress.value = progress;
    if (previousSource && previousSource !== metricSourceKey(status)) metrics.value = null;
    if (status.connected && !metrics.value) void pollSystemMetrics();
  } catch {
    coreStatus.value = null;
  } finally {
    pollingCore = false;
  }
}

function metricSourceKey(status: HarborCoreStatus | null) {
  if (!status) return "";
  return `${status.mode}:${status.workspace_id ?? ""}:${status.listen_url}`;
}

async function pollSystemMetrics() {
  if (pollingSystemMetrics) return;
  pollingSystemMetrics = true;
  try {
    metrics.value = await getSystemMetrics();
  } catch {
    // Retry after the selected Core becomes connected.
  } finally {
    pollingSystemMetrics = false;
  }
}

async function pollPerformanceMetrics() {
  if (pollingPerformanceMetrics) return;
  if (!metrics.value) {
    if (coreStatus.value?.connected) await pollSystemMetrics();
    return;
  }
  pollingPerformanceMetrics = true;
  try {
    mergePerformanceMetrics(await getPerformanceMetrics());
  } catch {
    // Keep the last complete sample during transient connection failures.
  } finally {
    pollingPerformanceMetrics = false;
  }
}

async function pollResourceMetrics() {
  if (pollingResourceMetrics || !metrics.value) return;
  pollingResourceMetrics = true;
  try {
    mergeResourceMetrics(await getResourceMetrics());
  } catch {
    // Keep the last complete sample during transient connection failures.
  } finally {
    pollingResourceMetrics = false;
  }
}

function mergePerformanceMetrics(next: PerformanceMetrics) {
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

function mergeResourceMetrics(next: ResourceMetrics) {
  if (!metrics.value) return;
  metrics.value = {
    ...metrics.value,
    memory: next.memory,
    swap: next.swap,
    root_disk: next.root_disk,
  };
}

onMounted(async () => {
  // --- 阶段 1：读取轮询配置并探测当前 Core ---
  const settings = await getSettings().catch(() => null);
  void pollCoreStatus();
  await pollSystemMetrics();

  // --- 阶段 2：持续更新性能与资源指标，断线重连后自动恢复 ---
  performanceTimer = window.setInterval(() => {
    void pollPerformanceMetrics();
  }, settings?.performance_metrics_interval_ms ?? 1000);
  resourceTimer = window.setInterval(() => {
    void pollResourceMetrics();
  }, settings?.resource_metrics_interval_ms ?? 10000);
  coreTimer = window.setInterval(() => {
    void pollCoreStatus();
  }, 2500);
});

onBeforeUnmount(() => {
  if (performanceTimer != null) window.clearInterval(performanceTimer);
  if (resourceTimer != null) window.clearInterval(resourceTimer);
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
