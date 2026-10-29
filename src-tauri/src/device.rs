//! Apparaattype: pc of laptop. Bepaald uit het behuizingstype (Win32_SystemEnclosure) en een ingebouwde
//! batterij (Win32_Battery). Een desktop met een UPS heeft een "batterij", maar geen laptopbehuizing,
//! en telt dus als pc. De gebruiker kan het in Instellingen handmatig vastzetten.

use crate::models::{DeviceInfo, DeviceKind, DeviceSetting};
use crate::state::AppState;
use std::collections::HashMap;
use std::sync::OnceLock;
use wmi::{Variant, WMIConnection};

/// SMBIOS-behuizingen van draagbare apparaten: Portable, Laptop, Notebook, Sub Notebook, Tablet,
/// Convertible, Detachable.
const LAPTOP_CHASSIS: [u32; 7] = [8, 9, 10, 14, 30, 31, 32];
/// Duidelijk een vaste pc: Desktop, Low Profile, Pizza Box, Mini Tower, Tower, All in One, Space-saving,
/// Main Server, Mini PC, Stick PC.
const DESKTOP_CHASSIS: [u32; 10] = [3, 4, 5, 6, 7, 13, 15, 17, 35, 36];

#[derive(Clone, Debug, Default)]
pub struct Detected {
    pub chassis: Vec<u32>,
    pub has_battery: bool,
    pub manufacturer: String,
    pub model: String,
    pub kind: DeviceKind,
}

fn vstr(row: &HashMap<String, Variant>, k: &str) -> String {
    match row.get(k) {
        Some(Variant::String(s)) => s.trim().to_string(),
        _ => String::new(),
    }
}

fn detect_now() -> Detected {
    let mut d = Detected::default();
    if let Ok(conn) = WMIConnection::new() {
        if let Ok(rows) = conn.raw_query::<HashMap<String, Variant>>("SELECT ChassisTypes FROM Win32_SystemEnclosure") {
            for row in rows {
                if let Some(Variant::Array(list)) = row.get("ChassisTypes") {
                    for v in list {
                        let n = match v {
                            Variant::UI1(x) => *x as u32,
                            Variant::UI2(x) => *x as u32,
                            Variant::UI4(x) => *x,
                            Variant::I2(x) => *x as u32,
                            Variant::I4(x) => *x as u32,
                            _ => continue,
                        };
                        d.chassis.push(n);
                    }
                }
            }
        }
        if let Ok(rows) = conn.raw_query::<HashMap<String, Variant>>("SELECT Name FROM Win32_Battery") {
            d.has_battery = !rows.is_empty();
        }
        if let Ok(rows) = conn.raw_query::<HashMap<String, Variant>>("SELECT Manufacturer, Model FROM Win32_ComputerSystem") {
            if let Some(row) = rows.first() {
                d.manufacturer = vstr(row, "Manufacturer");
                d.model = vstr(row, "Model");
            }
        }
    }
    let laptop_case = d.chassis.iter().any(|c| LAPTOP_CHASSIS.contains(c));
    let desktop_case = d.chassis.iter().any(|c| DESKTOP_CHASSIS.contains(c));
    // Laptopbehuizing telt altijd. Een batterij telt alleen als de behuizing niet duidelijk een pc is (UPS).
    d.kind = if laptop_case || (d.has_battery && !desktop_case) { DeviceKind::Laptop } else { DeviceKind::Pc };
    d
}

/// Detectie, één keer per start (WMI in een eigen thread vanwege COM).
pub fn detected() -> &'static Detected {
    static D: OnceLock<Detected> = OnceLock::new();
    D.get_or_init(|| std::thread::spawn(detect_now).join().unwrap_or_default())
}

/// Het type waar nezty mee rekent: de handmatige keuze, anders de detectie.
pub fn effective(state: &AppState) -> DeviceKind {
    match state.settings_clone().device_type {
        DeviceSetting::Pc => DeviceKind::Pc,
        DeviceSetting::Laptop => DeviceKind::Laptop,
        DeviceSetting::Auto => detected().kind,
    }
}

pub fn is_laptop(state: &AppState) -> bool {
    effective(state) == DeviceKind::Laptop
}

/// De app van het merk waarin je de videokaartmodus (MUX), het ventilatorprofiel en de prestatiemodus kiest.
pub fn vendor_app(manufacturer: &str, model: &str) -> Option<&'static str> {
    let m = format!("{} {}", manufacturer, model).to_lowercase();
    if m.contains("alienware") {
        Some("Alienware Command Center")
    } else if m.contains("asus") || m.contains("rog") || m.contains("tuf") {
        Some("Armoury Crate")
    } else if m.contains("lenovo") {
        if m.contains("legion") {
            Some("Legion Space / Lenovo Vantage")
        } else {
            Some("Lenovo Vantage")
        }
    } else if m.contains("hp") || m.contains("hewlett") || m.contains("omen") || m.contains("victus") {
        Some("OMEN Gaming Hub")
    } else if m.contains("micro-star") || m.contains("msi") {
        Some("MSI Center")
    } else if m.contains("acer") {
        if m.contains("predator") || m.contains("helios") || m.contains("triton") {
            Some("PredatorSense")
        } else {
            Some("NitroSense")
        }
    } else if m.contains("razer") {
        Some("Razer Synapse")
    } else if m.contains("gigabyte") || m.contains("aorus") {
        Some("Gigabyte Control Center")
    } else if m.contains("dell") {
        Some("Dell Power Manager / Alienware Command Center")
    } else if m.contains("samsung") {
        Some("Samsung Settings")
    } else {
        None
    }
}

pub fn info(state: &AppState) -> DeviceInfo {
    let d = detected();
    let setting = state.settings_clone().device_type;
    let layout = crate::bigwins::gpu_layout();
    let displays = crate::display::list();
    let display_gpu = displays.iter().find(|x| x.internal).or_else(|| displays.iter().find(|x| x.primary)).map(|x| x.adapter.trim().to_string()).filter(|s| !s.is_empty());
    DeviceInfo {
        detected: d.kind,
        effective: effective(state),
        setting,
        chassis: d.chassis.clone(),
        has_battery: d.has_battery,
        manufacturer: d.manufacturer.clone(),
        model: d.model.clone(),
        vendor_app: vendor_app(&d.manufacturer, &d.model).map(|s| s.to_string()),
        hybrid: layout.hybrid,
        display_gpu,
        fast_gpu: layout.gpus.iter().find(|g| g.discrete).map(|g| g.name.clone()),
    }
}
