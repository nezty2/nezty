//! Live metingen: CPU en RAM (sysinfo), GPU (NVML voor NVIDIA, anders WMI-prestatiemeters),
//! temperatuur (NVML / ACPI via WMI, als beschikbaar) en ping.

use crate::models::{now_ms, MetricsSample};
use crate::ping;
use crate::state::AppState;
use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
use nvml_wrapper::Nvml;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
use tauri::{AppHandle, Emitter};
use wmi::{Variant, WMIConnection};

fn variant_f64(v: &Variant) -> Option<f64> {
    Some(match v {
        Variant::UI8(x) => *x as f64,
        Variant::UI4(x) => *x as f64,
        Variant::UI2(x) => *x as f64,
        Variant::UI1(x) => *x as f64,
        Variant::I8(x) => *x as f64,
        Variant::I4(x) => *x as f64,
        Variant::I2(x) => *x as f64,
        Variant::R8(x) => *x,
        Variant::R4(x) => *x as f64,
        Variant::String(s) => s.parse::<f64>().ok()?,
        _ => return None,
    })
}

/// GPU-belasting via WMI: per adapter (luid) de drukste engine, daarvan de hoogste adapter.
fn wmi_gpu_pct(conn: &WMIConnection) -> Option<f32> {
    let rows: Vec<HashMap<String, Variant>> = conn
        .raw_query("SELECT Name, UtilizationPercentage FROM Win32_PerfFormattedData_GPUPerformanceCounters_GPUEngine")
        .ok()?;
    let mut per_engine: HashMap<(String, String), f64> = HashMap::new();
    for row in rows {
        let name = match row.get("Name") {
            Some(Variant::String(s)) => s.clone(),
            _ => continue,
        };
        let util = row.get("UtilizationPercentage").and_then(variant_f64).unwrap_or(0.0);
        let luid = name.split("_phys").next().and_then(|s| s.split("luid_").nth(1)).unwrap_or("").to_string();
        let engtype = name.split("engtype_").nth(1).unwrap_or("").to_string();
        if engtype.is_empty() {
            continue;
        }
        *per_engine.entry((luid, engtype)).or_insert(0.0) += util;
    }
    let mut per_luid: HashMap<String, f64> = HashMap::new();
    for ((luid, _), v) in per_engine {
        let e = per_luid.entry(luid).or_insert(0.0);
        if v > *e {
            *e = v;
        }
    }
    per_luid.values().cloned().fold(None, |acc: Option<f64>, v| Some(acc.map_or(v, |a| a.max(v)))).map(|v| v.min(100.0) as f32)
}

fn wmi_cpu_temp(conn: &WMIConnection) -> Option<f32> {
    let rows: Vec<HashMap<String, Variant>> = conn.raw_query("SELECT CurrentTemperature FROM MSAcpi_ThermalZoneTemperature").ok()?;
    let mut best: Option<f32> = None;
    for row in rows {
        if let Some(v) = row.get("CurrentTemperature").and_then(variant_f64) {
            let c = (v / 10.0 - 273.15) as f32;
            if c > 0.0 && c < 130.0 {
                best = Some(best.map_or(c, |b| b.max(c)));
            }
        }
    }
    best
}

/// Namen van CPU en GPU voor het infopaneel. Draait in een eigen thread vanwege COM.
pub fn hardware_names() -> (String, String) {
    let mut sys = System::new();
    sys.refresh_cpu_usage();
    let cpu = sys.cpus().first().map(|c| c.brand().trim().to_string()).unwrap_or_else(|| "Onbekende CPU".into());
    let mut gpu = String::new();
    if let Ok(nvml) = Nvml::init() {
        if let Ok(d) = nvml.device_by_index(0) {
            if let Ok(n) = d.name() {
                gpu = n;
            }
        }
    }
    if gpu.is_empty() {
        if let Ok(conn) = WMIConnection::new() {
            if let Ok(rows) = conn.raw_query::<HashMap<String, Variant>>("SELECT Name, AdapterRAM FROM Win32_VideoController") {
                let mut best: Option<(f64, String)> = None;
                for row in rows {
                    let name = match row.get("Name") {
                        Some(Variant::String(s)) => s.clone(),
                        _ => continue,
                    };
                    let ram = row.get("AdapterRAM").and_then(variant_f64).unwrap_or(0.0);
                    if best.as_ref().map(|(r, _)| ram > *r).unwrap_or(true) {
                        best = Some((ram, name));
                    }
                }
                if let Some((_, n)) = best {
                    gpu = n;
                }
            }
        }
    }
    if gpu.is_empty() {
        gpu = "Onbekende GPU".into();
    }
    (cpu, gpu)
}

pub fn start(app: AppHandle, state: Arc<AppState>) {
    let ping_shared: Arc<Mutex<(Option<f32>, String)>> = Arc::new(Mutex::new((None, String::new())));

    // Ping in een eigen thread, zodat een time-out de sampler niet ophoudt.
    {
        let shared = ping_shared.clone();
        let st = state.clone();
        let _ = std::thread::Builder::new().name("ping".into()).spawn(move || loop {
            let host = st.settings_clone().dashboard_ping_host;
            let r = if host.trim().is_empty() { None } else { ping::quick(&host, 1500) };
            if let Ok(mut g) = shared.lock() {
                *g = (r, host);
            }
            std::thread::sleep(Duration::from_secs(2));
        });
    }

    let _ = std::thread::Builder::new().name("metrics".into()).spawn(move || {
        let mut sys = System::new();
        sys.refresh_memory();
        sys.refresh_cpu_usage();
        let nvml = Nvml::init().ok();
        let wmi_cimv2 = WMIConnection::new().ok();
        let wmi_acpi = WMIConnection::with_namespace_path("root\\WMI").ok();
        let mut cpu_temp_ok = wmi_acpi.is_some();
        let mut last_cpu_temp: Option<f32> = None;
        let mut last_gpu: (Option<f32>, Option<f32>, Option<u64>, Option<u64>) = (None, None, None, None);
        let mut tick: u64 = 0;
        loop {
            std::thread::sleep(Duration::from_secs(1));
            tick += 1;
            sys.refresh_cpu_usage();
            sys.refresh_memory();
            if tick % 2 == 0 {
                sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
            }
            // GPU
            if let Some(nv) = &nvml {
                if let Ok(d) = nv.device_by_index(0) {
                    let util = d.utilization_rates().ok().map(|u| u.gpu as f32);
                    let temp = d.temperature(TemperatureSensor::Gpu).ok().map(|t| t as f32);
                    let mem = d.memory_info().ok();
                    last_gpu = (util, temp, mem.as_ref().map(|m| m.used / 1_048_576), mem.as_ref().map(|m| m.total / 1_048_576));
                }
            } else if tick % 2 == 0 {
                if let Some(conn) = &wmi_cimv2 {
                    last_gpu.0 = wmi_gpu_pct(conn);
                }
            }
            // CPU-temperatuur (vaak niet beschikbaar zonder stuurprogramma van de fabrikant).
            if cpu_temp_ok && tick % 5 == 1 {
                if let Some(conn) = &wmi_acpi {
                    match wmi_cpu_temp(conn) {
                        Some(t) => last_cpu_temp = Some(t),
                        None => {
                            cpu_temp_ok = false;
                            last_cpu_temp = None;
                        }
                    }
                }
            }
            let (ping_ms, ping_host) = ping_shared.lock().map(|g| g.clone()).unwrap_or((None, String::new()));
            let total = sys.total_memory() / 1_048_576;
            let used = sys.used_memory() / 1_048_576;
            let sample = MetricsSample {
                ts: now_ms(),
                cpu_pct: (sys.global_cpu_usage() * 10.0).round() / 10.0,
                ram_used_mb: used,
                ram_total_mb: total,
                ram_free_mb: sys.available_memory() / 1_048_576,
                gpu_pct: last_gpu.0,
                gpu_temp_c: last_gpu.1,
                cpu_temp_c: last_cpu_temp,
                gpu_mem_used_mb: last_gpu.2,
                gpu_mem_total_mb: last_gpu.3,
                process_count: sys.processes().len() as u32,
                ping_ms,
                ping_host,
            };
            state.push_sample(sample.clone());
            let _ = app.emit("metrics", sample);
        }
    });
}
