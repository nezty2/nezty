//! Pollingrate van de muis meten: een paar seconden lang de ruwe muisberichten van Windows tellen (raw input, ook
//! als nezty niet de focus heeft). De muis moet bewegen tijdens de meting. nezty rapporteert wat Windows aflevert:
//! bij 8000 Hz kan dat iets lager uitkomen, omdat Windows berichten samenvoegt als ze sneller komen dan ze gelezen
//! worden.

use crate::models::MouseRate;
use crate::winapi::wide;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::{GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER, RIDEV_INPUTSINK, RIDEV_REMOVE, RID_INPUT, RIM_TYPEMOUSE};
use windows_sys::Win32::UI::WindowsAndMessaging::{CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, PeekMessageW, RegisterClassW, TranslateMessage, MSG, PM_REMOVE, WM_INPUT, WNDCLASSW, WS_OVERLAPPED};

static MOVES: AtomicU32 = AtomicU32::new(0);
static INPUTS: AtomicU32 = AtomicU32::new(0);

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_INPUT {
        INPUTS.fetch_add(1, Ordering::Relaxed);
        let mut raw: RAWINPUT = std::mem::zeroed();
        let mut size = std::mem::size_of::<RAWINPUT>() as u32;
        let n = GetRawInputData(lparam as HRAWINPUT, RID_INPUT, &mut raw as *mut RAWINPUT as *mut _, &mut size, std::mem::size_of::<RAWINPUTHEADER>() as u32);
        if n != u32::MAX && raw.header.dwType == RIM_TYPEMOUSE {
            let m = raw.data.mouse;
            if m.lLastX != 0 || m.lLastY != 0 {
                MOVES.fetch_add(1, Ordering::Relaxed);
            }
        }
        return 0;
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// Meet `seconds` lang. Hz = bewegingsberichten per seconde over de drukste helve van de meting, afgerond.
pub fn measure(seconds: f32) -> MouseRate {
    let seconds = seconds.clamp(1.0, 10.0);
    unsafe {
        let class = wide("nezty-mouse-rate");
        let hinst = GetModuleHandleW(null());
        let wc = WNDCLASSW { style: 0, lpfnWndProc: Some(wnd_proc), cbClsExtra: 0, cbWndExtra: 0, hInstance: hinst, hIcon: null_mut(), hCursor: null_mut(), hbrBackground: null_mut(), lpszMenuName: null(), lpszClassName: class.as_ptr() };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(0, class.as_ptr(), class.as_ptr(), WS_OVERLAPPED, 0, 0, 0, 0, null_mut(), null_mut(), hinst, null());
        if hwnd.is_null() {
            log::warn!("muismeting: venster maken mislukt ({})", windows_sys::Win32::Foundation::GetLastError());
            return MouseRate { hz: None, moves: 0, seconds };
        }
        // Generic desktop, muis (usage page 1, usage 2), ook zonder focus.
        let dev = RAWINPUTDEVICE { usUsagePage: 1, usUsage: 2, dwFlags: RIDEV_INPUTSINK, hwndTarget: hwnd };
        if RegisterRawInputDevices(&dev, 1, std::mem::size_of::<RAWINPUTDEVICE>() as u32) == 0 {
            log::warn!("muismeting: raw input registreren mislukt ({})", windows_sys::Win32::Foundation::GetLastError());
            DestroyWindow(hwnd);
            return MouseRate { hz: None, moves: 0, seconds };
        }
        MOVES.store(0, Ordering::Relaxed);
        INPUTS.store(0, Ordering::Relaxed);
        let started = Instant::now();
        // Per 100 ms tellen, zodat de stilstaande stukken (muis even niet bewogen) niet meetellen.
        let mut buckets: Vec<u32> = Vec::new();
        let mut last_bucket = Instant::now();
        let mut last_count = 0u32;
        let mut msg: MSG = std::mem::zeroed();
        while started.elapsed() < Duration::from_secs_f32(seconds) {
            while PeekMessageW(&mut msg, hwnd, 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            if last_bucket.elapsed() >= Duration::from_millis(100) {
                let now = MOVES.load(Ordering::Relaxed);
                buckets.push(now - last_count);
                last_count = now;
                last_bucket = Instant::now();
            }
            std::thread::sleep(Duration::from_micros(50));
        }
        let off = RAWINPUTDEVICE { usUsagePage: 1, usUsage: 2, dwFlags: RIDEV_REMOVE, hwndTarget: null_mut() };
        RegisterRawInputDevices(&off, 1, std::mem::size_of::<RAWINPUTDEVICE>() as u32);
        DestroyWindow(hwnd);
        let moves = MOVES.load(Ordering::Relaxed);
        log::info!("muismeting: {} WM_INPUT, {moves} bewegingen, {} stukjes", INPUTS.load(Ordering::Relaxed), buckets.len());
        // De drukste helft van de stukjes van 100 ms: daar bewoog de muis echt.
        buckets.sort_unstable_by(|a, b| b.cmp(a));
        let busy: Vec<u32> = buckets.iter().copied().filter(|b| *b > 0).collect();
        let half = &busy[..busy.len().div_ceil(2).max(1).min(busy.len())];
        let hz = if half.is_empty() || moves < 200 {
            None
        } else {
            let per_100ms = half.iter().sum::<u32>() as f32 / half.len() as f32;
            Some((per_100ms * 10.0).round() as u32)
        };
        MouseRate { hz, moves, seconds }
    }
}
