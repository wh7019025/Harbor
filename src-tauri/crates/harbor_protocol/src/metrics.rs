use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CpuCoreMetric {
    pub id: usize,
    pub usage_percent: Option<f64>,
    pub frequency_mhz: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MemoryMetrics {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub usage_percent: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DiskMetrics {
    pub path: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub usage_percent: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GpuDeviceMetrics {
    pub index: u32,
    pub name: String,
    pub utilization_percent: Option<f64>,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub memory_usage_percent: Option<f64>,
    pub temperature_c: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SystemMetrics {
    pub timestamp_ms: u128,
    pub cpu_usage_percent: Option<f64>,
    pub cpu_cores: Vec<CpuCoreMetric>,
    pub memory: MemoryMetrics,
    pub swap: MemoryMetrics,
    pub root_disk: DiskMetrics,
    pub network_rx_bytes_per_sec: Option<f64>,
    pub network_tx_bytes_per_sec: Option<f64>,
    pub network_rx_total_bytes: u64,
    pub network_tx_total_bytes: u64,
    pub gpus: Vec<GpuDeviceMetrics>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PerformanceMetrics {
    pub timestamp_ms: u128,
    pub cpu_usage_percent: Option<f64>,
    pub cpu_cores: Vec<CpuCoreMetric>,
    pub network_rx_bytes_per_sec: Option<f64>,
    pub network_tx_bytes_per_sec: Option<f64>,
    pub network_rx_total_bytes: u64,
    pub network_tx_total_bytes: u64,
    pub gpus: Vec<GpuDeviceMetrics>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ResourceMetrics {
    pub timestamp_ms: u128,
    pub memory: MemoryMetrics,
    pub swap: MemoryMetrics,
    pub root_disk: DiskMetrics,
}
