use std::ffi::CString;
use std::fs;
use std::time::Instant;

use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
use nvml_wrapper::Nvml;
use serde::Serialize;

pub struct SystemMetricsSampler {
    previous: Option<SystemSample>,
    nvml: Option<Nvml>,
    nvml_checked: bool,
}

impl Default for SystemMetricsSampler {
    fn default() -> Self {
        Self {
            previous: None,
            nvml: None,
            nvml_checked: false,
        }
    }
}

#[derive(Clone, Copy)]
struct CpuCounters {
    idle: u64,
    total: u64,
}

#[derive(Clone)]
struct CpuSnapshot {
    total: CpuCounters,
    cores: Vec<CpuCounters>,
}

#[derive(Clone, Copy)]
struct NetCounters {
    rx_bytes: u64,
    tx_bytes: u64,
}

#[derive(Clone)]
struct SystemSample {
    captured_at: Instant,
    cpu: CpuSnapshot,
    net: NetCounters,
}

#[derive(Serialize)]
pub struct CpuCoreMetric {
    pub id: usize,
    pub usage_percent: Option<f64>,
    pub frequency_mhz: Option<f64>,
}

#[derive(Serialize)]
pub struct MemoryMetrics {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub usage_percent: Option<f64>,
}

#[derive(Serialize)]
pub struct DiskMetrics {
    pub path: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub usage_percent: Option<f64>,
}

#[derive(Clone, Serialize)]
pub struct GpuDeviceMetrics {
    pub index: u32,
    pub name: String,
    pub utilization_percent: Option<f64>,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub memory_usage_percent: Option<f64>,
    pub temperature_c: Option<f64>,
}

#[derive(Serialize)]
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

#[derive(Serialize)]
pub struct FastSystemMetrics {
    pub timestamp_ms: u128,
    pub cpu_usage_percent: Option<f64>,
    pub cpu_cores: Vec<CpuCoreMetric>,
    pub network_rx_bytes_per_sec: Option<f64>,
    pub network_tx_bytes_per_sec: Option<f64>,
    pub network_rx_total_bytes: u64,
    pub network_tx_total_bytes: u64,
    pub gpus: Vec<GpuDeviceMetrics>,
}

#[derive(Serialize)]
pub struct SlowSystemMetrics {
    pub timestamp_ms: u128,
    pub memory: MemoryMetrics,
    pub swap: MemoryMetrics,
    pub root_disk: DiskMetrics,
}

impl SystemMetricsSampler {
    pub fn sample(&mut self) -> SystemMetrics {
        let fast = self.sample_fast();
        let slow = sample_slow_metrics();

        SystemMetrics {
            timestamp_ms: fast.timestamp_ms,
            cpu_usage_percent: fast.cpu_usage_percent,
            cpu_cores: fast.cpu_cores,
            memory: slow.memory,
            swap: slow.swap,
            root_disk: slow.root_disk,
            network_rx_bytes_per_sec: fast.network_rx_bytes_per_sec,
            network_tx_bytes_per_sec: fast.network_tx_bytes_per_sec,
            network_rx_total_bytes: fast.network_rx_total_bytes,
            network_tx_total_bytes: fast.network_tx_total_bytes,
            gpus: fast.gpus,
        }
    }

    pub fn sample_fast(&mut self) -> FastSystemMetrics {
        let sample = SystemSample {
            captured_at: Instant::now(),
            cpu: read_cpu_snapshot().unwrap_or_else(empty_cpu_snapshot),
            net: read_net_counters().unwrap_or(NetCounters {
                rx_bytes: 0,
                tx_bytes: 0,
            }),
        };

        let frequencies = read_cpu_frequencies_mhz(sample.cpu.cores.len());
        let (cpu_usage_percent, cpu_cores, network_rx_bytes_per_sec, network_tx_bytes_per_sec) =
            match &self.previous {
                Some(previous) => {
                    let elapsed = sample
                        .captured_at
                        .duration_since(previous.captured_at)
                        .as_secs_f64();
                    let cpu_usage = cpu_usage_percent(previous.cpu.total, sample.cpu.total);
                    let cores = sample
                        .cpu
                        .cores
                        .iter()
                        .enumerate()
                        .map(|(idx, current)| CpuCoreMetric {
                            id: idx,
                            usage_percent: previous
                                .cpu
                                .cores
                                .get(idx)
                                .and_then(|previous| cpu_usage_percent(*previous, *current)),
                            frequency_mhz: frequencies.get(idx).copied().flatten(),
                        })
                        .collect();

                    if elapsed > 0.0 {
                        (
                            cpu_usage,
                            cores,
                            Some(
                                sample.net.rx_bytes.saturating_sub(previous.net.rx_bytes) as f64
                                    / elapsed,
                            ),
                            Some(
                                sample.net.tx_bytes.saturating_sub(previous.net.tx_bytes) as f64
                                    / elapsed,
                            ),
                        )
                    } else {
                        (cpu_usage, cores, None, None)
                    }
                }
                None => {
                    let cores = sample
                        .cpu
                        .cores
                        .iter()
                        .enumerate()
                        .map(|(idx, _)| CpuCoreMetric {
                            id: idx,
                            usage_percent: None,
                            frequency_mhz: frequencies.get(idx).copied().flatten(),
                        })
                        .collect();
                    (None, cores, None, None)
                }
            };

        self.previous = Some(sample.clone());
        let gpus = self.sample_nvidia_gpus();

        FastSystemMetrics {
            timestamp_ms: unix_timestamp_ms(),
            cpu_usage_percent,
            cpu_cores,
            network_rx_bytes_per_sec,
            network_tx_bytes_per_sec,
            network_rx_total_bytes: sample.net.rx_bytes,
            network_tx_total_bytes: sample.net.tx_bytes,
            gpus,
        }
    }

    fn ensure_nvml(&mut self) {
        if self.nvml_checked {
            return;
        }
        self.nvml_checked = true;
        self.nvml = Nvml::init().ok();
    }

    fn sample_nvidia_gpus(&mut self) -> Vec<GpuDeviceMetrics> {
        self.ensure_nvml();
        let Some(nvml) = self.nvml.as_ref() else {
            return Vec::new();
        };
        let Ok(count) = nvml.device_count() else {
            return Vec::new();
        };
        let mut devices = Vec::with_capacity(count as usize);
        for index in 0..count {
            let Ok(device) = nvml.device_by_index(index) else {
                continue;
            };
            let name = device
                .name()
                .unwrap_or_else(|_| format!("GPU {index}"));
            let utilization_percent = device
                .utilization_rates()
                .ok()
                .map(|rates| f64::from(rates.gpu));
            let memory = device.memory_info().ok();
            let (memory_used_bytes, memory_total_bytes, memory_usage_percent) = match memory {
                Some(info) if info.total > 0 => (
                    info.used,
                    info.total,
                    Some((info.used as f64 / info.total as f64) * 100.0),
                ),
                Some(info) => (info.used, info.total, None),
                None => (0, 0, None),
            };
            let temperature_c = device
                .temperature(TemperatureSensor::Gpu)
                .ok()
                .map(f64::from);
            devices.push(GpuDeviceMetrics {
                index,
                name,
                utilization_percent,
                memory_used_bytes,
                memory_total_bytes,
                memory_usage_percent,
                temperature_c,
            });
        }
        devices
    }
}

pub fn sample_slow_metrics() -> SlowSystemMetrics {
    SlowSystemMetrics {
        timestamp_ms: unix_timestamp_ms(),
        memory: read_memory_metrics("Mem").unwrap_or_else(empty_memory_metrics),
        swap: read_memory_metrics("Swap").unwrap_or_else(empty_memory_metrics),
        root_disk: read_disk_metrics("/").unwrap_or_else(|| empty_disk_metrics("/")),
    }
}

fn read_cpu_snapshot() -> Option<CpuSnapshot> {
    let stat = fs::read_to_string("/proc/stat").ok()?;
    let mut total: Option<CpuCounters> = None;
    let mut cores = Vec::new();

    for line in stat.lines() {
        let mut parts = line.split_whitespace();
        let Some(label) = parts.next() else {
            continue;
        };
        if label != "cpu" && !label.starts_with("cpu") {
            if total.is_some() {
                break;
            }
            continue;
        }

        let values: Vec<u64> = parts.filter_map(|part| part.parse::<u64>().ok()).collect();
        if values.len() < 5 {
            continue;
        }
        let idle = values.get(3).copied().unwrap_or(0) + values.get(4).copied().unwrap_or(0);
        let counters = CpuCounters {
            idle,
            total: values.iter().sum(),
        };

        if label == "cpu" {
            total = Some(counters);
        } else {
            cores.push(counters);
        }
    }

    Some(CpuSnapshot {
        total: total?,
        cores,
    })
}

fn empty_cpu_snapshot() -> CpuSnapshot {
    CpuSnapshot {
        total: CpuCounters { idle: 0, total: 0 },
        cores: Vec::new(),
    }
}

fn read_cpu_frequencies_mhz(core_count: usize) -> Vec<Option<f64>> {
    let mut freqs = vec![None; core_count];
    for (idx, slot) in freqs.iter_mut().enumerate() {
        let path = format!("/sys/devices/system/cpu/cpu{idx}/cpufreq/scaling_cur_freq");
        if let Ok(raw) = fs::read_to_string(path) {
            if let Ok(khz) = raw.trim().parse::<f64>() {
                *slot = Some(khz / 1000.0);
            }
        }
    }

    if freqs.iter().all(Option::is_none) {
        if let Ok(cpuinfo) = fs::read_to_string("/proc/cpuinfo") {
            let mut idx = 0;
            for line in cpuinfo.lines() {
                if !line.starts_with("cpu MHz") {
                    continue;
                }
                if idx >= freqs.len() {
                    break;
                }
                if let Some((_, value)) = line.split_once(':') {
                    freqs[idx] = value.trim().parse::<f64>().ok();
                    idx += 1;
                }
            }
        }
    }

    freqs
}

fn read_memory_metrics(prefix: &str) -> Option<MemoryMetrics> {
    let meminfo = fs::read_to_string("/proc/meminfo").ok()?;
    let total = meminfo_value_kb(&meminfo, &format!("{prefix}Total"))?.saturating_mul(1024);
    let available_key = if prefix == "Mem" {
        "MemAvailable"
    } else {
        "SwapFree"
    };
    let available = meminfo_value_kb(&meminfo, available_key)?.saturating_mul(1024);
    let used = total.saturating_sub(available);
    Some(MemoryMetrics {
        total_bytes: total,
        used_bytes: used,
        available_bytes: available,
        usage_percent: percent(used, total),
    })
}

fn meminfo_value_kb(meminfo: &str, key: &str) -> Option<u64> {
    let needle = format!("{key}:");
    meminfo
        .lines()
        .find_map(|line| line.strip_prefix(&needle))
        .and_then(|value| value.split_whitespace().next())
        .and_then(|value| value.parse::<u64>().ok())
}

fn empty_memory_metrics() -> MemoryMetrics {
    MemoryMetrics {
        total_bytes: 0,
        used_bytes: 0,
        available_bytes: 0,
        usage_percent: None,
    }
}

fn read_disk_metrics(path: &str) -> Option<DiskMetrics> {
    let c_path = CString::new(path).ok()?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    let result = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };
    if result != 0 {
        return None;
    }
    let stat = unsafe { stat.assume_init() };
    let block_size = stat.f_frsize as u64;
    let total = (stat.f_blocks as u64).saturating_mul(block_size);
    let available = (stat.f_bavail as u64).saturating_mul(block_size);
    let used = total.saturating_sub(available);
    Some(DiskMetrics {
        path: path.to_string(),
        total_bytes: total,
        used_bytes: used,
        available_bytes: available,
        usage_percent: percent(used, total),
    })
}

fn empty_disk_metrics(path: &str) -> DiskMetrics {
    DiskMetrics {
        path: path.to_string(),
        total_bytes: 0,
        used_bytes: 0,
        available_bytes: 0,
        usage_percent: None,
    }
}

fn read_net_counters() -> Option<NetCounters> {
    let dev = fs::read_to_string("/proc/net/dev").ok()?;
    let mut rx_bytes = 0_u64;
    let mut tx_bytes = 0_u64;

    for line in dev.lines().skip(2) {
        let (name, data) = line.split_once(':')?;
        if name.trim() == "lo" {
            continue;
        }
        let fields: Vec<&str> = data.split_whitespace().collect();
        if fields.len() < 16 {
            continue;
        }
        rx_bytes = rx_bytes.saturating_add(fields[0].parse::<u64>().unwrap_or(0));
        tx_bytes = tx_bytes.saturating_add(fields[8].parse::<u64>().unwrap_or(0));
    }

    Some(NetCounters { rx_bytes, tx_bytes })
}

fn cpu_usage_percent(previous: CpuCounters, current: CpuCounters) -> Option<f64> {
    let total_delta = current.total.saturating_sub(previous.total);
    if total_delta == 0 {
        return None;
    }
    let idle_delta = current.idle.saturating_sub(previous.idle);
    let busy_delta = total_delta.saturating_sub(idle_delta);
    Some((busy_delta as f64 / total_delta as f64) * 100.0)
}

fn percent(used: u64, total: u64) -> Option<f64> {
    if total == 0 {
        return None;
    }
    Some((used as f64 / total as f64) * 100.0)
}

fn unix_timestamp_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}
