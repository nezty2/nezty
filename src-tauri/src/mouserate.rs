//! Pollingrate van de muis meten zoals de driver hem ziet: via Windows raw input op een verborgen venster.
//! Elk rapport van de muis komt als WM_INPUT binnen, ook bij 8000 Hz. De browser-kant kan dat niet: die
//! rondt tijdstippen af op 0,1 ms, waardoor alles boven ongeveer 4000 Hz verkeerd uitkomt.

use crate::error::{AppError, AppResult};
use crate::models::MouseRate;
use std::time::{Duration, Instant};
use windows_sys::Win32::UI::Input::{
    GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER, RIDEV_INPUTSINK, RIDEV_REMOVE,
    RID_INPUT, RIM_TYPEMOUSE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, MsgWaitForMultipleObjects, PeekMessageW, RegisterClassW, HWND_MESSAGE,
    MSG, PM_REMOVE, QS_ALLINPUT, WM_INPUT, WNDCLASSW,
};

const STANDARD: [u32; 7] = [125, 250, 500, 1000, 2000, 4000, 8000];
/// Lengte van het venster waarin we het hoogste aantal rapporten zoeken (de muis beweegt niet continu).
const WINDOW: Duration = Duration::from_millis(250);

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Ronden naar een gangbare pollingrate als de meting er dichtbij zit, anders naar tientallen.
pub fn snap(hz: f32) -> u32 {
    for s in STANDARD {
        if (hz - s as f32).abs() / (s as f32) < 0.12 {
            return s;
        }
    }
    ((hz / 10.0).round() * 10.0) as u32
}

/// Telt uit een lijst tijdstippen het hoogste aantal rapporten in een venster van 250 ms en rekent dat om naar Hz.
pub fn rate_from(stamps: &[Duration]) -> f32 {
    let mut best = 0usize;
    let mut start = 0usize;
    for (i, t) in stamps.iter().enumerate() {
        while *t - stamps[start] > WINDOW {
            start += 1;
        }
        best = best.max(i - start + 1);
    }
    best as f32 / WINDOW.as_secs_f32()
}

pub fn measure(seconds: f32) -> AppResult<MouseRate> {
    let seconds = seconds.clamp(1.0, 10.0);
    let class = wide("nezty_rawinput");
    unsafe {
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(DefWindowProcW);
        wc.lpszClassName = class.as_ptr();
        // Een tweede registratie faalt (klasse bestaat al): dat is prima.
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(0, class.as_ptr(), wide("nezty").as_ptr(), 0, 0, 0, 0, 0, HWND_MESSAGE, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null());
        if hwnd.is_null() {
            return Err(AppError::msg("raw input window could not be created"));
        }
        let mut dev = RAWINPUTDEVICE { usUsagePage: 1, usUsage: 2, dwFlags: RIDEV_INPUTSINK, hwndTarget: hwnd };
        if RegisterRawInputDevices(&dev, 1, std::mem::size_of::<RAWINPUTDEVICE>() as u32) == 0 {
            DestroyWindow(hwnd);
            return Err(AppError::msg("raw input could not be registered"));
        }

        let start = Instant::now();
        let deadline = start + Duration::from_secs_f32(seconds);
        let mut stamps: Vec<Duration> = Vec::with_capacity(90_000);
        let mut msg: MSG = std::mem::zeroed();
        let mut wm_input = 0u32;
        let mut wakeups = 0u32;
        loop {
            wakeups += 1;
            while PeekMessageW(&mut msg, hwnd, 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_INPUT {
                    wm_input += 1;
                    let mut raw: RAWINPUT = std::mem::zeroed();
                    let mut size = std::mem::size_of::<RAWINPUT>() as u32;
                    let got = GetRawInputData(msg.lParam as HRAWINPUT, RID_INPUT, &mut raw as *mut _ as *mut _, &mut size, std::mem::size_of::<RAWINPUTHEADER>() as u32);
                    // Alleen echte beweging telt: knoppen en wiel hebben een eigen rapport.
                    if got != u32::MAX && raw.header.dwType == RIM_TYPEMOUSE && (raw.data.mouse.lLastX != 0 || raw.data.mouse.lLastY != 0) {
                        stamps.push(start.elapsed());
                    }
                }
                DispatchMessageW(&msg);
            }
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            // Slapen tot er een bericht is of de tijd om is: geen polling, dus geen afronding op de Windows-timer.
            let left = (deadline - now).as_millis().clamp(1, 1000) as u32;
            MsgWaitForMultipleObjects(0, std::ptr::null(), 0, left, QS_ALLINPUT);
        }

        dev.dwFlags = RIDEV_REMOVE;
        dev.hwndTarget = std::ptr::null_mut();
        RegisterRawInputDevices(&dev, 1, std::mem::size_of::<RAWINPUTDEVICE>() as u32);
        DestroyWindow(hwnd);

        let reports = stamps.len() as u32;
        let raw_hz = if reports >= 30 { rate_from(&stamps) } else { 0.0 };
        Ok(MouseRate { hz: if raw_hz > 0.0 { snap(raw_hz) } else { 0 }, raw_hz, reports, seconds, wm_input, wakeups })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_rate_in_busiest_window() {
        // 1000 Hz gedurende 0,5 s, dan 1 s niets, dan 500 Hz.
        let mut s: Vec<Duration> = (0..500).map(|i| Duration::from_micros(i * 1000)).collect();
        s.extend((0..250).map(|i| Duration::from_micros(1_500_000 + i * 2000)));
        assert_eq!(snap(rate_from(&s)), 1000);
        assert_eq!(snap(7900.0), 8000);
        assert_eq!(snap(5300.0), 5300);
    }
}
