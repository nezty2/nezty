//! "Grootste winst": controles die bij veel mensen echt verschil maken. Elke controle geeft een
//! status, een eerlijk winstlabel en (waar veilig) een knop om het op te lossen.

use crate::display;
use crate::error::{AppError, AppResult};
use crate::journal::{JournalEntry, RevertPayload};
use crate::models::*;
use crate::state::{lock, AppState};
use crate::tweaks::{self, per_game, system};
use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;
use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use wmi::{Variant, WMIConnection};

#[derive(Clone, Debug, Default)]
pub struct GpuInfo {
    pub name: String,
    pub vendor: String,
    pub driver_version: String,
    pub driver_date: Option<chrono::NaiveDate>,
    pub discrete: bool,
}

#[derive(Clone, Debug, Default)]
pub struct GpuLayout {
    pub gpus: Vec<GpuInfo>,
    pub hybrid: bool,
}

fn vstr(row: &HashMap<String, Variant>, k: &str) -> String {
    match row.get(k) {
        Some(Variant::String(s)) => s.clone(),
        _ => String::new(),
    }
}

fn vnum(row: &HashMap<String, Variant>, k: &str) -> Option<u64> {
    match row.get(k)? {
        Variant::UI1(v) => Some(*v as u64),
        Variant::UI2(v) => Some(*v as u64),
        Variant::UI4(v) => Some(*v as u64),
        Variant::UI8(v) => Some(*v),
        Variant::I2(v) => Some(*v as u64),
        Variant::I4(v) => Some(*v as u64),
        Variant::I8(v) => Some(*v as u64),
        Variant::String(s) => s.parse().ok(),
        _ => None,
    }
}

fn is_discrete(vendor: &str, name: &str) -> bool {
    let n = name.to_lowercase();
    match vendor {
        "nvidia" => true,
        "amd" => n.contains(" rx ") || n.contains("rx ") || n.contains("radeon pro") || n.contains("radeon vii") || n.contains("vega 56") || n.contains("vega 64"),
        "intel" => regex::Regex::new(r"(?i)arc\(tm\)\s*[ab]\d{3}|arc [ab]\d{3}").map(|r| r.is_match(name)).unwrap_or(false),
        _ => false,
    }
}

fn detect_gpus() -> GpuLayout {
    let mut gpus = vec![];
    if let Ok(conn) = WMIConnection::new() {
        if let Ok(rows) = conn.raw_query::<HashMap<String, Variant>>("SELECT Name, PNPDeviceID, DriverVersion, DriverDate FROM Win32_VideoController") {
            for row in rows {
                let name = vstr(&row, "Name");
                let pnp = vstr(&row, "PNPDeviceID").to_uppercase();
                let vendor = if pnp.contains("VEN_10DE") {
                    "nvidia"
                } else if pnp.contains("VEN_1002") || pnp.contains("VEN_1022") {
                    "amd"
                } else if pnp.contains("VEN_8086") {
                    "intel"
                } else {
                    continue; // bv. Microsoft Basic Display Adapter of virtuele schermen
                };
                let date_raw = vstr(&row, "DriverDate");
                let driver_date = if date_raw.len() >= 8 { chrono::NaiveDate::parse_from_str(&date_raw[..8], "%Y%m%d").ok() } else { None };
                gpus.push(GpuInfo {
                    discrete: is_discrete(vendor, &name),
                    name,
                    vendor: vendor.to_string(),
                    driver_version: vstr(&row, "DriverVersion"),
                    driver_date,
                });
            }
        }
    }
    let hybrid = gpus.iter().any(|g| g.discrete) && gpus.iter().any(|g| !g.discrete);
    GpuLayout { gpus, hybrid }
}

/// GPU-indeling, één keer bepaald en daarna uit het geheugen.
pub fn gpu_layout() -> &'static GpuLayout {
    static LAYOUT: OnceLock<GpuLayout> = OnceLock::new();
    LAYOUT.get_or_init(|| std::thread::spawn(detect_gpus).join().unwrap_or_default())
}

/// NVIDIA-driverversie zoals NVIDIA hem noemt (32.0.15.6094 wordt 560.94).
fn nvidia_version(win: &str) -> Option<String> {
    let parts: Vec<&str> = win.split('.').collect();
    if parts.len() < 4 {
        return None;
    }
    let digits = format!("{}{}", parts[2], parts[3]);
    if digits.len() < 5 {
        return None;
    }
    let last5 = &digits[digits.len() - 5..];
    Some(format!("{}.{}", &last5[..3], &last5[3..]))
}

fn check(id: &str, status: CheckStatus, gain: Gain, variant: &str) -> BigWin {
    BigWin { id: id.into(), status, gain, variant: variant.into(), params: BTreeMap::new(), action: CheckAction::None, link: None }
}

fn p(mut w: BigWin, k: &str, v: impl ToString) -> BigWin {
    w.params.insert(k.into(), v.to_string());
    w
}

// ---------- 1. Verversingssnelheid ----------

fn refresh_check() -> BigWin {
    let displays = display::list();
    if displays.is_empty() {
        return check("refresh_rate", CheckStatus::Unavailable, Gain::Big, "none");
    }
    let low: Vec<&display::DisplayInfo> = displays.iter().filter(|d| d.max_hz >= d.current_hz + 5).collect();
    if let Some(d) = low.first() {
        let gain = if d.current_hz <= 75 && d.max_hz >= 100 { Gain::Big } else { Gain::Measurable };
        let mut w = check("refresh_rate", CheckStatus::Issue, gain, if low.len() > 1 { "several" } else { "one" });
        w = p(w, "monitor", &d.monitor);
        w = p(w, "current", d.current_hz);
        w = p(w, "max", d.max_hz);
        w = p(w, "count", low.len());
        w.action = CheckAction::Fix;
        return w;
    }
    let d = displays.iter().find(|d| d.primary).unwrap_or(&displays[0]);
    p(p(check("refresh_rate", CheckStatus::Ok, Gain::Big, "ok"), "current", d.current_hz), "monitor", &d.monitor)
}

fn refresh_fix(state: &AppState) -> AppResult<()> {
    for d in display::list() {
        if d.max_hz < d.current_hz + 5 {
            continue;
        }
        display::set_mode(&d.device, d.width, d.height, d.max_hz)?;
        let mut e = JournalEntry::new("refresh_rate", "Highest refresh rate", JournalKind::Display, &format!("{} ({})", d.monitor, d.device));
        e.old_value = Some(format!("{} Hz", d.current_hz));
        e.new_value = Some(format!("{} Hz", d.max_hz));
        e.revertable = true;
        e.payload = RevertPayload::DisplayMode { device: d.device.clone(), width: d.width, height: d.height, freq: d.current_hz };
        lock(&state.journal).record(e)?;
    }
    // Controle: staat alles nu op het maximum?
    if display::list().iter().any(|d| d.max_hz >= d.current_hz + 5) {
        return Err(AppError::coded("verify_failed"));
    }
    Ok(())
}

// ---------- 2. Game op de snelle videokaart ----------

fn games_without_gpu_pref(state: &AppState) -> Vec<Game> {
    state.all_games().into_iter().filter(|g| g.exe_path.as_deref().map(|e| !per_game::gpu_pref_is_high(e)).unwrap_or(false)).collect()
}

fn gpu_choice_check(state: &AppState) -> BigWin {
    let layout = gpu_layout();
    if !layout.hybrid {
        return check("gpu_choice", CheckStatus::Ok, Gain::Big, "single");
    }
    let dgpu = layout.gpus.iter().find(|g| g.discrete).map(|g| g.name.clone()).unwrap_or_default();
    let missing = games_without_gpu_pref(state);
    if missing.is_empty() {
        return p(check("gpu_choice", CheckStatus::Ok, Gain::Big, "set"), "gpu", dgpu);
    }
    let names: Vec<String> = missing.iter().take(3).map(|g| g.name.clone()).collect();
    let mut w = check("gpu_choice", CheckStatus::Issue, Gain::Big, "missing");
    w = p(w, "gpu", dgpu);
    w = p(w, "count", missing.len());
    w = p(w, "names", names.join(", "));
    w.action = CheckAction::Fix;
    w
}

fn gpu_choice_fix(state: &AppState) -> AppResult<()> {
    for g in games_without_gpu_pref(state) {
        tweaks::apply(state, "gpu_high_perf", None, None, Some(&g))?;
    }
    if !games_without_gpu_pref(state).is_empty() {
        return Err(AppError::coded("verify_failed"));
    }
    Ok(())
}

// ---------- 3. Stroom en energieplan ----------

fn power_check() -> BigWin {
    let mut ps: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
    let ok = unsafe { GetSystemPowerStatus(&mut ps) } != 0;
    let has_battery = ok && ps.BatteryFlag != 128 && ps.BatteryFlag != 255;
    if has_battery && ps.ACLineStatus == 0 {
        return p(check("power_source", CheckStatus::Issue, Gain::Big, "battery"), "percent", ps.BatteryLifePercent);
    }
    if has_battery && ps.SystemStatusFlag == 1 {
        let mut w = check("power_source", CheckStatus::Issue, Gain::Big, "battery_saver");
        w.action = CheckAction::Link;
        w.link = Some("ms-settings:batterysaver".into());
        return w;
    }
    if let Ok((guid, name)) = system::active_scheme() {
        if guid == system::GUID_SAVER {
            let mut w = p(check("power_source", CheckStatus::Issue, Gain::Big, "saver_plan"), "plan", name);
            w.action = CheckAction::Fix;
            return w;
        }
        return p(check("power_source", CheckStatus::Ok, Gain::Big, if has_battery { "plugged" } else { "desktop" }), "plan", name);
    }
    check("power_source", CheckStatus::Ok, Gain::Big, if has_battery { "plugged" } else { "desktop" })
}

fn power_fix(state: &AppState) -> AppResult<()> {
    tweaks::apply(state, "power_plan", Some("high"), None, None)?;
    tweaks::verify(state, "power_plan", true, None)
}

// ---------- 4. Werkgeheugen (XMP/EXPO) ----------

fn rated_from_part(part: &str) -> Option<u32> {
    let part = part.trim().to_uppercase();
    // Kingston Fury: KF560C36... = DDR5-6000, KF436C16... = DDR4-3600.
    if let Some(c) = regex::Regex::new(r"KF\d(\d{2})C").ok()?.captures(&part) {
        return c[1].parse::<u32>().ok().map(|v| v * 100);
    }
    // Crucial: CP16G60C36U5B of CT16G56C46U5 = 6000 / 5600.
    if let Some(c) = regex::Regex::new(r"^C[PT]\d+G(\d{2})C\d{2}").ok()?.captures(&part) {
        return c[1].parse::<u32>().ok().map(|v| v * 100);
    }
    // Algemeen: een getal van vier cijfers tussen 2133 en 8800 (G.Skill F5-6000J..., Corsair ...B6000C36).
    let re = regex::Regex::new(r"(\d{4})").ok()?;
    let found = re.captures_iter(&part).filter_map(|c| c[1].parse::<u32>().ok()).find(|v| (2133..=8800).contains(v));
    found
}

/// (ingestelde snelheid, maximale snelheid, onderdeelnummer, geheugentype) per geheugenmodule.
type RamRow = (Option<u64>, Option<u64>, String, Option<u64>);

fn ram_check() -> BigWin {
    // WMI-waarden zijn niet thread-safe; zet ze in de eigen thread om naar gewone waarden.
    let rows: Vec<RamRow> = std::thread::spawn(|| {
        WMIConnection::new()
            .ok()
            .and_then(|c| c.raw_query::<HashMap<String, Variant>>("SELECT Speed, ConfiguredClockSpeed, PartNumber, SMBIOSMemoryType FROM Win32_PhysicalMemory").ok())
            .unwrap_or_default()
            .iter()
            .map(|r| (vnum(r, "ConfiguredClockSpeed"), vnum(r, "Speed"), vstr(r, "PartNumber"), vnum(r, "SMBIOSMemoryType")))
            .collect()
    })
    .join()
    .unwrap_or_default();
    if rows.is_empty() {
        return check("ram_speed", CheckStatus::Unavailable, Gain::Big, "unknown");
    }
    let configured = rows.iter().filter_map(|r| r.0).min().unwrap_or(0) as u32;
    let speed = rows.iter().filter_map(|r| r.1).max().unwrap_or(0) as u32;
    let part = rows[0].2.clone();
    let mem_type = match rows[0].3.unwrap_or(0) {
        34 => "DDR5",
        26 => "DDR4",
        24 => "DDR3",
        _ => "",
    };
    let rated = rated_from_part(&part).unwrap_or(0).max(speed);
    if configured == 0 {
        return check("ram_speed", CheckStatus::Unavailable, Gain::Big, "unknown");
    }
    // Sommige systemen melden de klok in MHz (de helft van MT/s).
    let effective = if rated > 0 && (configured * 2).abs_diff(rated) <= rated / 20 { configured * 2 } else { configured };
    let base = |w: BigWin| p(p(p(w, "configured", effective), "rated", rated), "type", mem_type);
    if rated > effective + 200 {
        return base(check("ram_speed", CheckStatus::Issue, Gain::Big, "xmp_off"));
    }
    let maybe_slow = (mem_type == "DDR5" && effective <= 4800) || (mem_type == "DDR4" && effective <= 2400);
    if maybe_slow {
        return base(check("ram_speed", CheckStatus::Info, Gain::Big, "maybe"));
    }
    base(check("ram_speed", CheckStatus::Ok, Gain::Big, "ok"))
}

// ---------- 5. Videokaartdriver ----------

fn driver_check() -> BigWin {
    let layout = gpu_layout();
    let gpu = layout.gpus.iter().find(|g| g.discrete).or_else(|| layout.gpus.first());
    let Some(g) = gpu else {
        return check("gpu_driver", CheckStatus::Unavailable, Gain::Measurable, "unknown");
    };
    let version = if g.vendor == "nvidia" { nvidia_version(&g.driver_version).unwrap_or(g.driver_version.clone()) } else { g.driver_version.clone() };
    let link = match g.vendor.as_str() {
        "nvidia" => "https://www.nvidia.com/en-us/drivers/",
        "amd" => "https://www.amd.com/en/support/download/drivers.html",
        _ => "https://www.intel.com/content/www/us/en/download-center/home.html",
    };
    let days = g.driver_date.map(|d| (chrono::Local::now().date_naive() - d).num_days()).unwrap_or(-1);
    let old = days > 180;
    let mut w = check("gpu_driver", if old { CheckStatus::Issue } else { CheckStatus::Ok }, Gain::Measurable, if old { "old" } else { "ok" });
    w = p(w, "gpu", &g.name);
    w = p(w, "version", version);
    w = p(w, "date", g.driver_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default());
    w = p(w, "days", days.max(0));
    w = p(w, "vendor", &g.vendor);
    w.action = CheckAction::Link;
    w.link = Some(link.into());
    w
}

// ---------- 6. Zware achtergrond-apps ----------

fn heavy_check(state: &AppState) -> BigWin {
    let list = crate::processes::list(state);
    let total_ram_mb = {
        let mut sys = lock(&state.procs);
        sys.refresh_memory();
        sys.total_memory() / 1_048_576
    };
    let mut heavy: Vec<&ProcessInfo> = list
        .iter()
        .filter(|p| p.category != ProcessCategory::Game)
        .filter(|p| p.cpu_pct >= 10.0 || p.memory_mb >= 800.0)
        .collect();
    heavy.sort_by(|a, b| b.memory_mb.partial_cmp(&a.memory_mb).unwrap_or(std::cmp::Ordering::Equal));
    if heavy.is_empty() {
        return check("heavy_apps", CheckStatus::Ok, Gain::Measurable, "ok");
    }
    let cpu_hog = heavy.iter().any(|p| p.cpu_pct >= 10.0);
    let mem: f32 = heavy.iter().map(|p| p.memory_mb).sum();
    let names: Vec<String> = heavy.iter().take(3).map(|p| p.title.clone()).collect();
    let (status, variant) = if cpu_hog {
        (CheckStatus::Issue, "cpu")
    } else if total_ram_mb <= 16_500 {
        (CheckStatus::Issue, "memory")
    } else {
        (CheckStatus::Info, "plenty_ram")
    };
    let mut w = check("heavy_apps", status, Gain::Measurable, variant);
    w = p(w, "names", names.join(", "));
    w = p(w, "mb", mem.round() as u64);
    w = p(w, "count", heavy.len());
    w.action = CheckAction::OpenPage;
    w
}

// ---------- Alleen laptop ----------

fn vendor_app_param(w: BigWin) -> BigWin {
    let d = crate::device::detected();
    match crate::device::vendor_app(&d.manufacturer, &d.model) {
        Some(app) => p(w, "app", app),
        None => w,
    }
}

/// Twee videokaarten: loopt het ingebouwde scherm via de snelle kaart (MUX of "alleen dGPU") of via de
/// zuinige? Of een laptop een MUX-switch heeft, kan Windows niet zeggen; we leggen uit waar je het vindt.
fn laptop_gpu_mode_check() -> BigWin {
    let layout = gpu_layout();
    if !layout.hybrid {
        return check("laptop_gpu_mode", CheckStatus::Ok, Gain::Big, "single");
    }
    let dgpu = layout.gpus.iter().find(|g| g.discrete).map(|g| g.name.clone()).unwrap_or_default();
    let displays = display::list();
    let Some(panel) = displays.iter().find(|d| d.internal) else {
        return vendor_app_param(p(check("laptop_gpu_mode", CheckStatus::Info, Gain::Big, "no_internal"), "dgpu", dgpu));
    };
    let on_dgpu = layout.gpus.iter().any(|g| g.discrete && g.name.eq_ignore_ascii_case(panel.adapter.trim()));
    if on_dgpu {
        return p(check("laptop_gpu_mode", CheckStatus::Ok, Gain::Big, "dgpu"), "dgpu", dgpu);
    }
    let mut w = check("laptop_gpu_mode", CheckStatus::Info, Gain::Big, "hybrid");
    w = p(w, "dgpu", dgpu);
    w = p(w, "igpu", panel.adapter.trim());
    vendor_app_param(w)
}

/// Energiebesparing van het ingebouwde scherm (Intel DPST, AMD Vari-Bright) en dynamische verversingssnelheid.
/// Die staan in de app van Intel of AMD en in Windows; we kunnen ze niet veilig zelf uitlezen.
fn laptop_display_check() -> BigWin {
    let layout = gpu_layout();
    let igpu = layout.gpus.iter().find(|g| !g.discrete).map(|g| g.vendor.clone()).unwrap_or_default();
    let variant = match igpu.as_str() {
        "intel" => "intel",
        "amd" => "amd",
        _ => "other",
    };
    let mut w = check("laptop_display", CheckStatus::Info, Gain::Small, variant);
    if let Some(panel) = display::list().into_iter().find(|d| d.internal) {
        w = p(w, "hz", panel.current_hz);
        w = p(w, "max", panel.max_hz);
    }
    w.action = CheckAction::Link;
    w.link = Some("ms-settings:display-advanced".into());
    w
}

/// Hitte: temperatuur en of Windows of de videokaart terugschakelt. Alleen uitlezen, nooit ventilatoren aansturen.
fn laptop_heat_check(state: &AppState) -> BigWin {
    let (last, throttle) = state.recent_heat(120);
    let cpu = last.as_ref().and_then(|s| s.cpu_temp_c);
    let gpu = last.as_ref().and_then(|s| s.gpu_temp_c);
    let mut w = match throttle {
        Some(true) => check("laptop_heat", CheckStatus::Issue, Gain::Big, "throttling"),
        Some(false) => check("laptop_heat", CheckStatus::Ok, Gain::Big, "ok"),
        None => check("laptop_heat", CheckStatus::Info, Gain::Big, "unknown"),
    };
    if let Some(t) = cpu {
        w = p(w, "cpu", t.round() as i32);
    }
    if let Some(t) = gpu {
        w = p(w, "gpu", t.round() as i32);
    }
    vendor_app_param(w)
}

/// Wifi of kabel. Op een laptop speelt bijna iedereen via wifi; een kabel is de grootste ping-winst.
fn laptop_wifi_check() -> BigWin {
    let out = crate::powershell::run_ps(
        "@(Get-NetAdapter -Physical -ErrorAction SilentlyContinue | Where-Object { $_.Status -eq 'Up' } | ForEach-Object { [string]$_.PhysicalMediaType }) -join '|'",
    )
    .unwrap_or_default()
    .to_lowercase();
    let wifi = out.contains("802.11") || out.contains("wireless");
    let cable = out.contains("802.3") || out.contains("ethernet");
    if cable {
        return check("laptop_wifi", CheckStatus::Ok, Gain::Big, "cable");
    }
    if wifi {
        return check("laptop_wifi", CheckStatus::Issue, Gain::Big, "wifi");
    }
    check("laptop_wifi", CheckStatus::Unavailable, Gain::Big, "none")
}

/// Op de accu remt de driver van de snelle videokaart vaak zelf de FPS (NVIDIA Battery Boost houdt standaard
/// 30 FPS aan). Dat zit in de app van NVIDIA of AMD; nezty legt uit waar.
fn gpu_battery_limit_check() -> Option<BigWin> {
    let layout = gpu_layout();
    let dgpu = layout.gpus.iter().find(|g| g.discrete)?;
    let variant = match dgpu.vendor.as_str() {
        "nvidia" => "nvidia",
        "amd" => "amd",
        _ => return None,
    };
    let on_battery = crate::metrics::battery().1 == Some(false);
    let mut w = check("gpu_battery_limit", if on_battery { CheckStatus::Issue } else { CheckStatus::Info }, Gain::Big, variant);
    w = p(w, "gpu", &dgpu.name);
    Some(w)
}

/// Hulp-apps van de fabrikant die nu draaien en tijdens het gamen dicht kunnen.
fn vendor_apps_check(state: &AppState) -> BigWin {
    let list: Vec<ProcessInfo> = crate::processes::list(state).into_iter().filter(|p| p.category == ProcessCategory::Vendor).collect();
    if list.is_empty() {
        return check("vendor_apps", CheckStatus::Ok, Gain::Small, "none");
    }
    let mem: f32 = list.iter().map(|p| p.memory_mb).sum();
    let names: Vec<String> = list.iter().take(3).map(|p| p.title.clone()).collect();
    let mut w = check("vendor_apps", CheckStatus::Info, Gain::Small, "found");
    w = p(w, "names", names.join(", "));
    w = p(w, "count", list.len());
    w = p(w, "mb", mem.round() as u64);
    w.action = CheckAction::OpenPage;
    w
}

pub fn all(state: &AppState) -> Vec<BigWin> {
    let mut out = vec![refresh_check(), gpu_choice_check(state), power_check(), ram_check(), driver_check(), heavy_check(state)];
    if crate::device::is_laptop(state) {
        out.push(laptop_heat_check(state));
        out.push(laptop_gpu_mode_check());
        out.push(laptop_wifi_check());
        out.push(laptop_display_check());
        if let Some(w) = gpu_battery_limit_check() {
            out.push(w);
        }
        out.push(vendor_apps_check(state));
    }
    out
}

pub fn fix(state: &AppState, id: &str) -> AppResult<()> {
    match id {
        "refresh_rate" => refresh_fix(state),
        "gpu_choice" => gpu_choice_fix(state),
        "power_source" => power_fix(state),
        _ => Err(AppError::NotFound(format!("check {id}"))),
    }
}
