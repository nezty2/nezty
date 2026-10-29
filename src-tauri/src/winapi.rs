//! Alle directe Win32-aanroepen op één plek: adminrechten, procesprioriteit, muis- en UI-instellingen.

use crate::error::{AppError, AppResult};
use crate::registry::{self, Hive};
use std::ffi::c_void;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE};
use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetPriorityClass, OpenProcess, OpenProcessToken, SetPriorityClass, HIGH_PRIORITY_CLASS,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_INFORMATION, REALTIME_PRIORITY_CLASS,
};
use windows_sys::Win32::UI::Accessibility::{FILTERKEYS, STICKYKEYS, TOGGLEKEYS};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SendMessageTimeoutW, SystemParametersInfoW, ANIMATIONINFO, HWND_BROADCAST, SMTO_ABORTIFHUNG, SPIF_SENDCHANGE,
    SPI_GETFILTERKEYS, SPI_GETSTICKYKEYS, SPI_GETTOGGLEKEYS, SPI_SETANIMATION, SPI_SETFILTERKEYS, SPI_SETKEYBOARDDELAY,
    SPI_SETKEYBOARDSPEED, SPI_SETMOUSE, SPI_SETSTICKYKEYS, SPI_SETTOGGLEKEYS, SPI_SETUIEFFECTS, WM_SETTINGCHANGE,
};

pub const PRIORITY_HIGH: u32 = HIGH_PRIORITY_CLASS;

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Draait dit proces met administratorrechten?
pub fn is_elevated() -> bool {
    unsafe {
        let mut token: HANDLE = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut returned: u32 = 0;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut TOKEN_ELEVATION as *mut c_void,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        );
        CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}

pub fn process_exists(pid: u32) -> bool {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return false;
        }
        CloseHandle(h);
        true
    }
}

pub fn get_priority_class(pid: u32) -> AppResult<u32> {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return Err(AppError::msg(format!("process {pid} cannot be opened (error {})", GetLastError())));
        }
        let class = GetPriorityClass(h);
        CloseHandle(h);
        if class == 0 {
            return Err(AppError::msg(format!("priority of process {pid} not readable")));
        }
        Ok(class)
    }
}

/// Zet de prioriteitsklasse. "Realtime" wordt nooit toegestaan.
pub fn set_priority_class(pid: u32, class: u32) -> AppResult<u32> {
    if class == REALTIME_PRIORITY_CLASS {
        return Err(AppError::msg("Realtime priority is intentionally not supported."));
    }
    unsafe {
        let h = OpenProcess(PROCESS_SET_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            let code = GetLastError();
            if code == 5 {
                return Err(AppError::NeedsAdmin(format!("change priority of process {pid}")));
            }
            return Err(AppError::msg(format!("process {pid} cannot be opened (error {code})")));
        }
        let old = GetPriorityClass(h);
        let ok = SetPriorityClass(h, class);
        CloseHandle(h);
        if ok == 0 {
            return Err(AppError::msg(format!("priority of process {pid} cannot be changed")));
        }
        Ok(old)
    }
}

/// SystemParametersInfo(SPI_SETMOUSE) met [drempel1, drempel2, versnelling].
pub fn set_mouse_params(params: [i32; 3]) {
    let mut p = params;
    unsafe {
        SystemParametersInfoW(SPI_SETMOUSE, 0, p.as_mut_ptr() as *mut c_void, SPIF_SENDCHANGE);
    }
}

/// Leest MouseThreshold1/2 en MouseSpeed uit het register en past ze direct toe.
pub fn apply_mouse_from_registry() {
    let read = |name: &str, default: i32| -> i32 {
        registry::read(Hive::Hkcu, "Control Panel\\Mouse", name)
            .ok()
            .and_then(|d| d.as_str().and_then(|s| s.trim().parse::<i32>().ok()))
            .unwrap_or(default)
    };
    set_mouse_params([read("MouseThreshold1", 6), read("MouseThreshold2", 10), read("MouseSpeed", 1)]);
}

/// Master-schakelaar voor UI-effecten (SPI_SETUIEFFECTS).
pub fn set_ui_effects(enabled: bool) {
    unsafe {
        SystemParametersInfoW(SPI_SETUIEFFECTS, 0, (enabled as usize) as *mut c_void, SPIF_SENDCHANGE);
    }
}

/// Vensteranimaties (minimaliseren/maximaliseren) aan of uit.
pub fn set_min_animate(enabled: bool) {
    let mut info = ANIMATIONINFO { cbSize: std::mem::size_of::<ANIMATIONINFO>() as u32, iMinAnimate: enabled as i32 };
    unsafe {
        SystemParametersInfoW(SPI_SETANIMATION, info.cbSize, &mut info as *mut ANIMATIONINFO as *mut c_void, SPIF_SENDCHANGE);
    }
}

/// Laat andere programma's weten dat een instelling is veranderd.
pub fn broadcast_setting_change(area: &str) {
    let w = wide(area);
    let mut result: usize = 0;
    unsafe {
        SendMessageTimeoutW(HWND_BROADCAST, WM_SETTINGCHANGE, 0, w.as_ptr() as isize, SMTO_ABORTIFHUNG, 2000, &mut result);
    }
}

/// Leesbare naam van een prioriteitsklasse.
pub fn priority_label(class: u32) -> String {
    match class {
        0x20 => "Normal".into(),
        0x40 => "Idle".into(),
        0x80 => "High".into(),
        0x100 => "Realtime".into(),
        0x4000 => "Below normal".into(),
        0x8000 => "Above normal".into(),
        other => format!("0x{other:x}"),
    }
}

fn reg_u32(path: &str, name: &str, default: u32) -> u32 {
    registry::read(Hive::Hkcu, path, name)
        .ok()
        .and_then(|d| d.as_str().and_then(|s| s.trim().parse::<u32>().ok()))
        .unwrap_or(default)
}

/// Past toetsenbordvertraging (0-3) en herhaalsnelheid (0-31) direct toe vanuit het register.
pub fn apply_keyboard_from_registry() {
    let delay = reg_u32("Control Panel\\Keyboard", "KeyboardDelay", 1).min(3);
    let speed = reg_u32("Control Panel\\Keyboard", "KeyboardSpeed", 31).min(31);
    unsafe {
        SystemParametersInfoW(SPI_SETKEYBOARDDELAY, delay, std::ptr::null_mut(), SPIF_SENDCHANGE);
        SystemParametersInfoW(SPI_SETKEYBOARDSPEED, speed, std::ptr::null_mut(), SPIF_SENDCHANGE);
    }
}

/// Past de vlaggen van plaktoetsen, filtertoetsen en wisseltoetsen direct toe vanuit het register.
pub fn apply_accessibility_from_registry() {
    let sticky = reg_u32("Control Panel\\Accessibility\\StickyKeys", "Flags", 510);
    let filter = reg_u32("Control Panel\\Accessibility\\Keyboard Response", "Flags", 126);
    let toggle = reg_u32("Control Panel\\Accessibility\\ToggleKeys", "Flags", 62);
    unsafe {
        let mut sk = STICKYKEYS { cbSize: std::mem::size_of::<STICKYKEYS>() as u32, dwFlags: 0 };
        if SystemParametersInfoW(SPI_GETSTICKYKEYS, sk.cbSize, &mut sk as *mut _ as *mut c_void, 0) != 0 {
            sk.dwFlags = sticky;
            SystemParametersInfoW(SPI_SETSTICKYKEYS, sk.cbSize, &mut sk as *mut _ as *mut c_void, SPIF_SENDCHANGE);
        }
        let mut fk: FILTERKEYS = std::mem::zeroed();
        fk.cbSize = std::mem::size_of::<FILTERKEYS>() as u32;
        if SystemParametersInfoW(SPI_GETFILTERKEYS, fk.cbSize, &mut fk as *mut _ as *mut c_void, 0) != 0 {
            fk.dwFlags = filter;
            SystemParametersInfoW(SPI_SETFILTERKEYS, fk.cbSize, &mut fk as *mut _ as *mut c_void, SPIF_SENDCHANGE);
        }
        let mut tk = TOGGLEKEYS { cbSize: std::mem::size_of::<TOGGLEKEYS>() as u32, dwFlags: 0 };
        if SystemParametersInfoW(SPI_GETTOGGLEKEYS, tk.cbSize, &mut tk as *mut _ as *mut c_void, 0) != 0 {
            tk.dwFlags = toggle;
            SystemParametersInfoW(SPI_SETTOGGLEKEYS, tk.cbSize, &mut tk as *mut _ as *mut c_void, SPIF_SENDCHANGE);
        }
    }
}

/// Wat Windows nu echt gebruikt voor muis en toetsenbord (niet wat in het register staat).
pub struct LiveInput {
    pub mouse_acceleration: i32,
    pub keyboard_delay: u32,
    pub keyboard_speed: u32,
    /// Sneltoets actief voor plak-, filter- en wisseltoetsen.
    pub hotkeys: [bool; 3],
}

pub fn live_input() -> LiveInput {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SPI_GETKEYBOARDDELAY, SPI_GETKEYBOARDSPEED, SPI_GETMOUSE};
    unsafe {
        let mut mouse = [0i32; 3];
        SystemParametersInfoW(SPI_GETMOUSE, 0, mouse.as_mut_ptr() as *mut c_void, 0);
        let mut delay: u32 = 1;
        SystemParametersInfoW(SPI_GETKEYBOARDDELAY, 0, &mut delay as *mut u32 as *mut c_void, 0);
        let mut speed: u32 = 31;
        SystemParametersInfoW(SPI_GETKEYBOARDSPEED, 0, &mut speed as *mut u32 as *mut c_void, 0);
        let mut sk = STICKYKEYS { cbSize: std::mem::size_of::<STICKYKEYS>() as u32, dwFlags: 0 };
        SystemParametersInfoW(SPI_GETSTICKYKEYS, sk.cbSize, &mut sk as *mut _ as *mut c_void, 0);
        let mut fk: FILTERKEYS = std::mem::zeroed();
        fk.cbSize = std::mem::size_of::<FILTERKEYS>() as u32;
        SystemParametersInfoW(SPI_GETFILTERKEYS, fk.cbSize, &mut fk as *mut _ as *mut c_void, 0);
        let mut tk = TOGGLEKEYS { cbSize: std::mem::size_of::<TOGGLEKEYS>() as u32, dwFlags: 0 };
        SystemParametersInfoW(SPI_GETTOGGLEKEYS, tk.cbSize, &mut tk as *mut _ as *mut c_void, 0);
        LiveInput {
            mouse_acceleration: mouse[2],
            keyboard_delay: delay,
            keyboard_speed: speed,
            hotkeys: [sk.dwFlags & 0x4 != 0, fk.dwFlags & 0x4 != 0, tk.dwFlags & 0x4 != 0],
        }
    }
}

/// Geeft andere processen toestemming om een venster op de voorgrond te zetten. Een tweede start van
/// nezty (dubbelklik terwijl hij al draait) geeft zo zijn voorgrondrecht door aan het eerste venster.
pub fn allow_foreground_any() {
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(windows_sys::Win32::UI::WindowsAndMessaging::ASFW_ANY);
    }
}

/// Zet een venster echt op de voorgrond, ook als Windows dat voor een achtergrondproces tegenhoudt.
/// Koppelt daarvoor kort de invoer aan die van het huidige voorgrondvenster. Moet op de UI-thread draaien.
pub fn force_foreground(raw_hwnd: isize) {
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE,
        SW_SHOW,
    };
    let hwnd = raw_hwnd as windows_sys::Win32::Foundation::HWND;
    unsafe {
        ShowWindow(hwnd, if IsIconic(hwnd) != 0 { SW_RESTORE } else { SW_SHOW });
        let fg = GetForegroundWindow();
        if fg == hwnd {
            return;
        }
        let me = GetCurrentThreadId();
        let other = if fg.is_null() { 0 } else { GetWindowThreadProcessId(fg, null_mut()) };
        let attached = other != 0 && other != me && AttachThreadInput(me, other, 1) != 0;
        BringWindowToTop(hwnd);
        SetForegroundWindow(hwnd);
        if attached {
            AttachThreadInput(me, other, 0);
        }
    }
}
