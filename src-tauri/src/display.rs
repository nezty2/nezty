//! Beeldschermen: huidige en hoogst mogelijke verversingssnelheid, en die instellen.

use crate::error::{AppError, AppResult};
use crate::winapi::wide;
use std::collections::HashMap;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Graphics::Gdi::{
    ChangeDisplaySettingsExW, EnumDisplayDevicesW, EnumDisplaySettingsExW, CDS_UPDATEREGISTRY, DEVMODEW,
    DISPLAY_DEVICEW, DISPLAY_DEVICE_ATTACHED_TO_DESKTOP, DISPLAY_DEVICE_PRIMARY_DEVICE, DISP_CHANGE_SUCCESSFUL,
    DM_DISPLAYFREQUENCY, DM_PELSHEIGHT, DM_PELSWIDTH, ENUM_CURRENT_SETTINGS,
};

/// Vlag voor EnumDisplayDevicesW: geef de apparaat-interfacenaam (met modelcode) terug.
const EDD_GET_DEVICE_INTERFACE_NAME: u32 = 0x0000_0001;

#[derive(Clone, Debug)]
pub struct DisplayInfo {
    /// Bv. \\.\DISPLAY1
    pub device: String,
    /// Naam van het scherm zoals Windows hem kent.
    pub monitor: String,
    pub primary: bool,
    pub width: u32,
    pub height: u32,
    pub current_hz: u32,
    pub max_hz: u32,
}

fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

fn new_devmode() -> DEVMODEW {
    let mut dm = DEVMODEW::default();
    dm.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
    dm
}

/// Alle schermen die aan het bureaublad hangen, met hun huidige en hoogste verversingssnelheid
/// bij de huidige resolutie.
/// Modelcode (bv. "AOC2402") en leesbare naam (bv. "AOC 24G2W1G4") van aangesloten schermen.
fn friendly_names() -> Vec<(String, String)> {
    std::thread::spawn(|| {
        let mut out = vec![];
        let Ok(conn) = wmi::WMIConnection::with_namespace_path("root\\WMI") else { return out };
        let Ok(rows) = conn.raw_query::<HashMap<String, wmi::Variant>>("SELECT InstanceName, UserFriendlyName, ManufacturerName FROM WmiMonitorID") else {
            return out;
        };
        let text = |v: Option<&wmi::Variant>| -> String {
            match v {
                Some(wmi::Variant::Array(items)) => items
                    .iter()
                    .filter_map(|x| match x {
                        wmi::Variant::UI2(c) => Some(*c as u32),
                        wmi::Variant::UI1(c) => Some(*c as u32),
                        wmi::Variant::I4(c) => Some(*c as u32),
                        wmi::Variant::UI4(c) => Some(*c),
                        _ => None,
                    })
                    .filter(|c| *c != 0)
                    .filter_map(char::from_u32)
                    .collect(),
                _ => String::new(),
            }
        };
        for row in rows {
            let instance = match row.get("InstanceName") {
                Some(wmi::Variant::String(s)) => s.clone(),
                _ => continue,
            };
            let code = instance.split('\\').nth(1).unwrap_or_default().to_uppercase();
            let name = text(row.get("UserFriendlyName"));
            let maker = text(row.get("ManufacturerName"));
            if !code.is_empty() && !name.is_empty() {
                let full = if maker.is_empty() || name.to_uppercase().starts_with(&maker.to_uppercase()) { name } else { format!("{maker} {name}") };
                out.push((code, full));
            }
        }
        out
    })
    .join()
    .unwrap_or_default()
}

pub fn list() -> Vec<DisplayInfo> {
    let friendly = friendly_names();
    let mut out = vec![];
    let mut i = 0u32;
    loop {
        let mut dd = DISPLAY_DEVICEW { cb: std::mem::size_of::<DISPLAY_DEVICEW>() as u32, ..Default::default() };
        if unsafe { EnumDisplayDevicesW(null(), i, &mut dd, 0) } == 0 {
            break;
        }
        i += 1;
        if dd.StateFlags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP == 0 {
            continue;
        }
        let device = from_wide(&dd.DeviceName);
        let dev_w = wide(&device);
        let mut cur = new_devmode();
        if unsafe { EnumDisplaySettingsExW(dev_w.as_ptr(), ENUM_CURRENT_SETTINGS, &mut cur, 0) } == 0 {
            continue;
        }
        let (w, h, bpp) = (cur.dmPelsWidth, cur.dmPelsHeight, cur.dmBitsPerPel);
        let mut max_hz = cur.dmDisplayFrequency;
        let mut m = 0u32;
        loop {
            let mut dm = new_devmode();
            if unsafe { EnumDisplaySettingsExW(dev_w.as_ptr(), m, &mut dm, 0) } == 0 {
                break;
            }
            m += 1;
            if dm.dmPelsWidth == w && dm.dmPelsHeight == h && dm.dmBitsPerPel == bpp && dm.dmDisplayFrequency > max_hz {
                max_hz = dm.dmDisplayFrequency;
            }
        }
        // Naam van het aangesloten scherm: eerst de echte modelnaam via WMI, anders wat Windows meldt.
        let mut mon = DISPLAY_DEVICEW { cb: std::mem::size_of::<DISPLAY_DEVICEW>() as u32, ..Default::default() };
        let monitor = if unsafe { EnumDisplayDevicesW(dev_w.as_ptr(), 0, &mut mon, EDD_GET_DEVICE_INTERFACE_NAME) } != 0 {
            // DeviceID ziet eruit als \\?\DISPLAY#AOC2402#5&...#{...}; het model staat na "DISPLAY#".
            let id = from_wide(&mon.DeviceID);
            let model = id.split('#').nth(1).unwrap_or_default().to_uppercase();
            friendly.iter().find(|(code, _)| *code == model).map(|(_, n)| n.clone()).unwrap_or_else(|| from_wide(&mon.DeviceString))
        } else {
            device.clone()
        };
        out.push(DisplayInfo {
            device,
            monitor,
            primary: dd.StateFlags & DISPLAY_DEVICE_PRIMARY_DEVICE != 0,
            width: w,
            height: h,
            current_hz: cur.dmDisplayFrequency,
            max_hz,
        });
    }
    out
}

/// Zet resolutie en verversingssnelheid van één scherm (permanent, zoals in Windows-instellingen).
pub fn set_mode(device: &str, width: u32, height: u32, freq: u32) -> AppResult<()> {
    let dev_w = wide(device);
    let mut dm = new_devmode();
    if unsafe { EnumDisplaySettingsExW(dev_w.as_ptr(), ENUM_CURRENT_SETTINGS, &mut dm, 0) } == 0 {
        return Err(AppError::msg(format!("display {device} not found")));
    }
    dm.dmPelsWidth = width;
    dm.dmPelsHeight = height;
    dm.dmDisplayFrequency = freq;
    dm.dmFields = DM_PELSWIDTH | DM_PELSHEIGHT | DM_DISPLAYFREQUENCY;
    let r = unsafe { ChangeDisplaySettingsExW(dev_w.as_ptr(), &dm, null_mut(), CDS_UPDATEREGISTRY, null()) };
    if r != DISP_CHANGE_SUCCESSFUL {
        return Err(AppError::msg(format!("ChangeDisplaySettingsEx returned {r}")));
    }
    Ok(())
}
