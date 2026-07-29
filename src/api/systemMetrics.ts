import { invoke } from "@tauri-apps/api/core";

export interface CpuCoreMetric {
  id: number;
  usage_percent: number | null;
  frequency_mhz: number | null;
}

export interface MemoryMetrics {
  total_bytes: number;
  used_bytes: number;
  available_bytes: number;
  usage_percent: number | null;
}

export interface DiskMetrics {
  path: string;
  total_bytes: number;
  used_bytes: number;
  available_bytes: number;
  usage_percent: number | null;
}

export interface GpuDeviceMetrics {
  index: number;
  name: string;
  utilization_percent: number | null;
  memory_used_bytes: number;
  memory_total_bytes: number;
  memory_usage_percent: number | null;
  temperature_c: number | null;
}

export interface SystemMetrics {
  timestamp_ms: number;
  cpu_usage_percent: number | null;
  cpu_cores: CpuCoreMetric[];
  memory: MemoryMetrics;
  swap: MemoryMetrics;
  root_disk: DiskMetrics;
  network_rx_bytes_per_sec: number | null;
  network_tx_bytes_per_sec: number | null;
  network_rx_total_bytes: number;
  network_tx_total_bytes: number;
  gpus: GpuDeviceMetrics[];
}

export interface FastSystemMetrics {
  timestamp_ms: number;
  cpu_usage_percent: number | null;
  cpu_cores: CpuCoreMetric[];
  network_rx_bytes_per_sec: number | null;
  network_tx_bytes_per_sec: number | null;
  network_rx_total_bytes: number;
  network_tx_total_bytes: number;
  gpus: GpuDeviceMetrics[];
}

export interface SlowSystemMetrics {
  timestamp_ms: number;
  memory: MemoryMetrics;
  swap: MemoryMetrics;
  root_disk: DiskMetrics;
}

export interface MiniMetrics {
  cpu_usage_percent: number | null;
  memory_usage_percent: number | null;
}

export function getSystemMetrics() {
  return invoke<SystemMetrics>("get_system_metrics");
}

export function getFastSystemMetrics() {
  return invoke<FastSystemMetrics>("get_fast_system_metrics");
}

export function getSlowSystemMetrics() {
  return invoke<SlowSystemMetrics>("get_slow_system_metrics");
}

export function getMiniMetrics() {
  return invoke<MiniMetrics>("get_mini_metrics");
}
