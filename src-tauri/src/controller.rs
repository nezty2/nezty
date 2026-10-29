//! Controllers: Xbox-compatibele controllers via XInput en PlayStation-controllers (DualShock 4, DualSense)
//! via HID. Toont of ze met een kabel of draadloos verbonden zijn, de batterij, en meet hoe vaak ze een nieuwe
//! stand doorgeven. Alleen uitlezen: er wordt niets aan de controller of aan Windows veranderd.

use serde::Serialize;
use std::time::{Duration, Instant};
use windows_sys::core::GUID;
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW,
    DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
};
use windows_sys::Win32::Devices::HumanInterfaceDevice::HidD_GetHidGuid;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_IO_PENDING, GENERIC_READ, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0};
use windows_sys::Win32::Storage::FileSystem::{CreateFileW, ReadFile, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
use windows_sys::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows_sys::Win32::System::IO::{CancelIo, GetOverlappedResult, OVERLAPPED};
use windows_sys::Win32::UI::Input::XboxController::{
    XInputGetBatteryInformation, XInputGetState, BATTERY_DEVTYPE_GAMEPAD, BATTERY_LEVEL_EMPTY, BATTERY_LEVEL_FULL,
    BATTERY_LEVEL_LOW, BATTERY_LEVEL_MEDIUM, BATTERY_TYPE_DISCONNECTED, BATTERY_TYPE_WIRED, XINPUT_BATTERY_INFORMATION,
    XINPUT_STATE,
};

const ERROR_SUCCESS: u32 = 0;
const SONY_VID: &str = "054c";

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ControllerInfo {
    /// "xinput:<plek>" of "hid:<apparaatpad>".
    pub id: String,
    /// "xbox" of "playstation".
    pub kind: String,
    /// Model als sleutel voor de vertaling: "xbox", "dualsense", "dualsense_edge", "dualshock4", "ps_adapter".
    pub model: String,
    /// XInput-plek 0 tot en met 3 (alleen Xbox).
    pub slot: Option<u32>,
    /// true = kabel, false = draadloos, None = onbekend.
    pub wired: Option<bool>,
    /// Xbox: "empty" | "low" | "medium" | "full" | "unknown".
    pub battery: Option<String>,
    /// PlayStation: batterij in procenten (bij benadering).
    pub battery_percent: Option<u8>,
    pub charging: Option<bool>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ControllerRate {
    pub id: String,
    /// Geschatte rapportsnelheid. None als er te weinig gegevens waren.
    pub hz: Option<u32>,
    /// Aantal nieuwe standen of rapporten tijdens de meting.
    pub changes: u32,
    pub seconds: f32,
    pub connected: bool,
}

// ---------- Xbox (XInput) ----------

fn xinput_state(slot: u32) -> Option<XINPUT_STATE> {
    let mut s: XINPUT_STATE = unsafe { std::mem::zeroed() };
    (unsafe { XInputGetState(slot, &mut s) } == ERROR_SUCCESS).then_some(s)
}

fn xbox_list() -> Vec<ControllerInfo> {
    (0..4u32)
        .filter(|&slot| xinput_state(slot).is_some())
        .map(|slot| {
            let mut b: XINPUT_BATTERY_INFORMATION = unsafe { std::mem::zeroed() };
            let ok = unsafe { XInputGetBatteryInformation(slot, BATTERY_DEVTYPE_GAMEPAD, &mut b) } == ERROR_SUCCESS;
            let (wired, battery) = if !ok || b.BatteryType == BATTERY_TYPE_DISCONNECTED {
                (None, None)
            } else if b.BatteryType == BATTERY_TYPE_WIRED {
                (Some(true), None)
            } else {
                let level = match b.BatteryLevel {
                    l if l == BATTERY_LEVEL_EMPTY => "empty",
                    l if l == BATTERY_LEVEL_LOW => "low",
                    l if l == BATTERY_LEVEL_MEDIUM => "medium",
                    l if l == BATTERY_LEVEL_FULL => "full",
                    _ => "unknown",
                };
                (Some(false), Some(level.to_string()))
            };
            ControllerInfo {
                id: format!("xinput:{slot}"),
                kind: "xbox".into(),
                model: "xbox".into(),
                slot: Some(slot),
                wired,
                battery,
                battery_percent: None,
                charging: None,
            }
        })
        .collect()
}

fn xbox_measure(id: &str, slot: u32, seconds: f32) -> ControllerRate {
    let Some(first) = xinput_state(slot) else {
        return ControllerRate { id: id.into(), hz: None, changes: 0, seconds, connected: false };
    };
    let mut last_packet = first.dwPacketNumber;
    let mut last_change: Option<Instant> = None;
    let mut intervals: Vec<f64> = Vec::new();
    let mut changes = 0u32;
    let end = Instant::now() + Duration::from_secs_f32(seconds);
    while Instant::now() < end {
        match xinput_state(slot) {
            Some(s) if s.dwPacketNumber != last_packet => {
                let now = Instant::now();
                if let Some(prev) = last_change {
                    let ms = now.duration_since(prev).as_secs_f64() * 1000.0;
                    // Langer dan 30 ms = de stick stond even stil; dat telt niet mee.
                    if ms > 0.2 && ms < 30.0 {
                        intervals.push(ms);
                    }
                }
                last_change = Some(now);
                last_packet = s.dwPacketNumber;
                changes += 1;
            }
            Some(_) => {}
            None => return ControllerRate { id: id.into(), hz: None, changes, seconds, connected: false },
        }
        // Ongeveer elke 0,2 ms kijken: snel genoeg voor 1000 Hz, zonder een hele kern vol te laten draaien.
        let pause = Instant::now() + Duration::from_micros(200);
        while Instant::now() < pause {
            std::hint::spin_loop();
        }
        std::thread::yield_now();
    }
    ControllerRate { id: id.into(), hz: median_hz(&mut intervals, 40), changes, seconds, connected: true }
}

fn median_hz(intervals: &mut [f64], min: usize) -> Option<u32> {
    if intervals.len() < min {
        return None;
    }
    intervals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = intervals[intervals.len() / 2];
    (median > 0.0).then(|| (1000.0 / median).round() as u32)
}

// ---------- PlayStation (HID) ----------

/// Model en verbinding uit een HID-apparaatpad. USB: "hid#vid_054c&pid_0ce6...",
/// Bluetooth: "hid#{00001124-...}_vid&0002054c_pid&0ce6...".
pub fn parse_sony_path(path: &str) -> Option<(String, Option<bool>)> {
    let p = path.to_ascii_lowercase();
    let pid = if let Some(i) = p.find(&format!("vid_{SONY_VID}&pid_")) {
        p.get(i + 13..i + 17)?.to_string()
    } else if let Some(i) = p.find(&format!("vid&0002{SONY_VID}_pid&")) {
        p.get(i + 17..i + 21)?.to_string()
    } else {
        return None;
    };
    let model = match pid.as_str() {
        "0ce6" => "dualsense",
        "0df2" => "dualsense_edge",
        "05c4" | "09cc" => "dualshock4",
        "0ba0" => "ps_adapter",
        _ => return None,
    };
    let bluetooth = p.contains("{00001124-0000-1000-8000-00805f9b34fb}") || p.contains("bthenum") || p.contains("bthledevice");
    // De draadloze PlayStation-adapter zit met USB in de pc, maar de controller zelf is draadloos.
    let wired = if bluetooth || model == "ps_adapter" { Some(false) } else { Some(true) };
    Some((model.to_string(), wired))
}

/// Batterij uit een invoerrapport (eerste byte = rapport-id). Bij benadering, afgerond op tientallen.
pub fn parse_sony_battery(model: &str, buf: &[u8]) -> Option<(u8, bool)> {
    let id = *buf.first()?;
    let at = match (model, id, buf.len()) {
        ("dualsense" | "dualsense_edge", 0x01, n) if n >= 64 => 53,
        ("dualsense" | "dualsense_edge", 0x31, n) if n >= 64 => 54,
        ("dualshock4" | "ps_adapter", 0x01, n) if n >= 64 => 30,
        ("dualshock4", 0x11, n) if n >= 64 => 32,
        _ => return None,
    };
    let b = *buf.get(at)?;
    if model.starts_with("dualsense") {
        let level = b & 0x0f;
        let state = (b >> 4) & 0x0f;
        Some(((level.min(10) * 10).max(5), state == 1))
    } else {
        let level = b & 0x0f;
        let cable = b & 0x10 != 0;
        Some(((level.min(10) * 10).max(5), cable && level < 11))
    }
}

fn hid_paths() -> Vec<String> {
    let mut out = Vec::new();
    unsafe {
        let mut guid: GUID = std::mem::zeroed();
        HidD_GetHidGuid(&mut guid);
        let set = SetupDiGetClassDevsW(&guid, std::ptr::null(), std::ptr::null_mut(), DIGCF_PRESENT | DIGCF_DEVICEINTERFACE);
        if set == -1 {
            return out;
        }
        let mut i = 0u32;
        loop {
            let mut data: SP_DEVICE_INTERFACE_DATA = std::mem::zeroed();
            data.cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;
            if SetupDiEnumDeviceInterfaces(set, std::ptr::null(), &guid, i, &mut data) == 0 {
                break;
            }
            i += 1;
            let mut needed = 0u32;
            SetupDiGetDeviceInterfaceDetailW(set, &data, std::ptr::null_mut(), 0, &mut needed, std::ptr::null_mut());
            if needed == 0 {
                continue;
            }
            // u64-buffer voor een correcte uitlijning van de structuur.
            let mut buf = vec![0u64; (needed as usize).div_ceil(8)];
            let detail = buf.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
            (*detail).cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
            if SetupDiGetDeviceInterfaceDetailW(set, &data, detail, needed, std::ptr::null_mut(), std::ptr::null_mut()) == 0 {
                continue;
            }
            let start = std::ptr::addr_of!((*detail).DevicePath) as *const u16;
            let max = (needed as usize).saturating_sub(4) / 2;
            let slice = std::slice::from_raw_parts(start, max);
            let len = slice.iter().position(|&c| c == 0).unwrap_or(max);
            out.push(String::from_utf16_lossy(&slice[..len]));
        }
        SetupDiDestroyDeviceInfoList(set);
    }
    out
}

/// Opent een HID-apparaat alleen om te lezen, gedeeld met andere programma's (zoals Steam of de game).
struct Hid {
    handle: HANDLE,
    event: HANDLE,
}

impl Hid {
    fn open(path: &str) -> Option<Hid> {
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let handle = CreateFileW(
                wide.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED,
                std::ptr::null_mut(),
            );
            if handle == INVALID_HANDLE_VALUE {
                return None;
            }
            let event = CreateEventW(std::ptr::null(), 1, 0, std::ptr::null());
            if event.is_null() {
                CloseHandle(handle);
                return None;
            }
            Some(Hid { handle, event })
        }
    }

    /// Leest één invoerrapport. None bij een time-out of fout.
    fn read(&self, buf: &mut [u8], timeout_ms: u32) -> Option<usize> {
        unsafe {
            let mut ov: OVERLAPPED = std::mem::zeroed();
            ov.hEvent = self.event;
            let mut got = 0u32;
            if ReadFile(self.handle, buf.as_mut_ptr(), buf.len() as u32, &mut got, &mut ov) == 0 {
                if GetLastError() != ERROR_IO_PENDING {
                    return None;
                }
                if WaitForSingleObject(self.event, timeout_ms) != WAIT_OBJECT_0 {
                    CancelIo(self.handle);
                    GetOverlappedResult(self.handle, &ov, &mut got, 1);
                    return None;
                }
                if GetOverlappedResult(self.handle, &ov, &mut got, 0) == 0 {
                    return None;
                }
            }
            Some(got as usize)
        }
    }
}

impl Drop for Hid {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.event);
            CloseHandle(self.handle);
        }
    }
}

fn playstation_list() -> Vec<ControllerInfo> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for path in hid_paths() {
        let Some((model, wired)) = parse_sony_path(&path) else { continue };
        // Eén apparaat kan meerdere HID-onderdelen hebben (&col01, &col02). Die tellen als één controller.
        let lower = path.to_ascii_lowercase();
        let base = match lower.find("&col") {
            Some(i) => lower[..i].to_string() + &lower[(i + 6).min(lower.len())..],
            None => lower.clone(),
        };
        if !seen.insert(base) {
            continue;
        }
        let mut battery_percent = None;
        let mut charging = None;
        if let Some(dev) = Hid::open(&path) {
            let mut buf = [0u8; 600];
            if let Some(n) = dev.read(&mut buf, 250) {
                if let Some((pct, chg)) = parse_sony_battery(&model, &buf[..n]) {
                    battery_percent = Some(pct);
                    charging = Some(chg);
                }
            }
        }
        out.push(ControllerInfo {
            id: format!("hid:{path}"),
            kind: "playstation".into(),
            model,
            slot: None,
            wired,
            battery: None,
            battery_percent,
            charging,
        });
    }
    out
}

fn hid_measure(id: &str, path: &str, seconds: f32) -> ControllerRate {
    let Some(dev) = Hid::open(path) else {
        return ControllerRate { id: id.into(), hz: None, changes: 0, seconds, connected: false };
    };
    let mut buf = [0u8; 600];
    let mut last: Option<Instant> = None;
    let mut intervals = Vec::new();
    let mut reports = 0u32;
    let end = Instant::now() + Duration::from_secs_f32(seconds);
    while Instant::now() < end {
        match dev.read(&mut buf, 500) {
            Some(n) if n > 0 => {
                let now = Instant::now();
                if let Some(prev) = last {
                    let ms = now.duration_since(prev).as_secs_f64() * 1000.0;
                    if ms > 0.2 && ms < 30.0 {
                        intervals.push(ms);
                    }
                }
                last = Some(now);
                reports += 1;
            }
            _ => {
                // Geen rapport binnen een halve seconde: controller weg of in rust (sommige draadloze controllers
                // sturen alleen iets bij beweging). Blijf proberen tot de tijd om is.
                if Hid::open(path).is_none() {
                    return ControllerRate { id: id.into(), hz: None, changes: reports, seconds, connected: false };
                }
            }
        }
    }
    ControllerRate { id: id.into(), hz: median_hz(&mut intervals, 40), changes: reports, seconds, connected: true }
}

// ---------- Samen ----------

/// Alle aangesloten controllers: eerst Xbox-compatibele, dan PlayStation.
pub fn list() -> Vec<ControllerInfo> {
    let mut all = xbox_list();
    all.extend(playstation_list());
    all
}

/// Meet een paar seconden hoe vaak de controller een nieuwe stand doorgeeft.
pub fn measure(id: &str, seconds: f32) -> ControllerRate {
    let seconds = seconds.clamp(2.0, 8.0);
    if let Some(slot) = id.strip_prefix("xinput:").and_then(|s| s.parse::<u32>().ok()) {
        return xbox_measure(id, slot, seconds);
    }
    if let Some(path) = id.strip_prefix("hid:") {
        if parse_sony_path(path).is_some() {
            return hid_measure(id, path, seconds);
        }
    }
    ControllerRate { id: id.into(), hz: None, changes: 0, seconds, connected: false }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sony_paths() {
        let usb = r"\\?\hid#vid_054c&pid_0ce6&mi_03#8&2a3b&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}";
        assert_eq!(parse_sony_path(usb), Some(("dualsense".into(), Some(true))));
        let bt = r"\\?\hid#{00001124-0000-1000-8000-00805f9b34fb}_vid&0002054c_pid&09cc#9&1&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}";
        assert_eq!(parse_sony_path(bt), Some(("dualshock4".into(), Some(false))));
        assert_eq!(parse_sony_path(r"\\?\hid#vid_046d&pid_c539#x"), None);
    }

    #[test]
    fn sony_battery() {
        let mut usb = vec![0u8; 64];
        usb[0] = 0x01;
        usb[53] = 0x18; // laden, niveau 8
        assert_eq!(parse_sony_battery("dualsense", &usb), Some((80, true)));
        let mut ds4 = vec![0u8; 64];
        ds4[0] = 0x01;
        ds4[30] = 0x05; // geen kabel, niveau 5
        assert_eq!(parse_sony_battery("dualshock4", &ds4), Some((50, false)));
        assert_eq!(parse_sony_battery("dualsense", &[0x01, 0, 0]), None);
    }
}
