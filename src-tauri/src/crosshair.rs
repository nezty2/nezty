//! Crosshair-overlay: een klein, transparant, altijd-bovenop en doorklikbaar venster in het midden
//! van het hoofdscherm. Puur een venster bovenop de game: er wordt niets in de game geïnjecteerd.
//! Zichtbaar in venster- en borderless-modus, niet in exclusief volledig scherm.

use crate::error::{AppError, AppResult};
use crate::state::{lock, AppState};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub const LABEL: &str = "crosshair";
/// Logische grootte van het overlayvenster (ruim genoeg voor de grootste crosshair).
const SIZE: f64 = 220.0;

fn place(app: &AppHandle, win: &tauri::WebviewWindow) -> AppResult<()> {
    let monitor = app
        .primary_monitor()
        .map_err(|e| AppError::msg(e.to_string()))?
        .ok_or_else(|| AppError::msg("no primary monitor"))?;
    let scale = monitor.scale_factor();
    let px = (SIZE * scale).round() as u32;
    let pos = monitor.position();
    let size = monitor.size();
    let x = pos.x + (size.width as i32 - px as i32) / 2;
    let y = pos.y + (size.height as i32 - px as i32) / 2;
    win.set_size(PhysicalSize::new(px, px)).map_err(|e| AppError::msg(e.to_string()))?;
    win.set_position(PhysicalPosition::new(x, y)).map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

fn ensure_window(app: &AppHandle) -> AppResult<tauri::WebviewWindow> {
    if let Some(w) = app.get_webview_window(LABEL) {
        return Ok(w);
    }
    let win = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html#crosshair".into()))
        .title("nezty crosshair")
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .resizable(false)
        .focused(false)
        .focusable(false)
        .visible(false)
        .inner_size(SIZE, SIZE)
        .build()
        .map_err(|e| AppError::msg(e.to_string()))?;
    win.set_ignore_cursor_events(true).map_err(|e| AppError::msg(e.to_string()))?;
    place(app, &win)?;
    Ok(win)
}

/// Houdt de overlay buiten Alt+Tab en de taakbalk en zorgt dat hij nooit de focus pakt,
/// zodat je tijdens het gamen niet per ongeluk naar een onzichtbaar venster wisselt.
/// Tao zet bij elke zichtbaarheidswijziging WS_EX_APPWINDOW terug. Daarom gebeurt dit na elke `show()`,
/// op de hoofdthread, dus na de vensteropdrachten die al in de wachtrij staan.
fn keep_out_of_alt_tab(app: &AppHandle, win: &tauri::WebviewWindow) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
        SWP_NOSIZE, SWP_NOZORDER, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };
    let Ok(hwnd) = win.hwnd() else { return };
    let raw = hwnd.0 as isize;
    let _ = app.run_on_main_thread(move || unsafe {
        let h = raw as windows_sys::Win32::Foundation::HWND;
        let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
        let wanted = (ex & !(WS_EX_APPWINDOW as isize)) | (WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) as isize;
        if wanted != ex {
            SetWindowLongPtrW(h, GWL_EXSTYLE, wanted);
            SetWindowPos(
                h,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
    });
}

/// Zet de overlay aan of uit en bewaart dat in de instellingen.
pub fn set_enabled(app: &AppHandle, state: &AppState, enabled: bool) -> AppResult<()> {
    if enabled {
        let win = ensure_window(app)?;
        place(app, &win)?;
        win.show().map_err(|e| AppError::msg(e.to_string()))?;
        win.set_ignore_cursor_events(true).map_err(|e| AppError::msg(e.to_string()))?;
        let _ = win.set_always_on_top(true);
        keep_out_of_alt_tab(app, &win);
    } else if let Some(win) = app.get_webview_window(LABEL) {
        let _ = win.hide();
    }
    lock(&state.settings).crosshair_enabled = enabled;
    state.save_settings();
    let _ = app.emit("crosshair-state", enabled);
    Ok(())
}

/// Wisselt de overlay. Draait buiten de event-loop, zodat het maken van het venster niet vastloopt.
pub fn toggle(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(state) = app.try_state::<Arc<AppState>>() {
            let enabled = !lock(&state.settings).crosshair_enabled;
            if let Err(e) = set_enabled(&app, &state, enabled) {
                log::warn!("crosshair: {e}");
            }
        }
    });
}

/// Stuurt de nieuwe instellingen naar het overlayvenster.
pub fn push_config(app: &AppHandle, state: &AppState) {
    let cfg = lock(&state.settings).crosshair.clone();
    let _ = app.emit("crosshair-config", cfg);
}

/// Registreert de sneltoets (bv. "ctrl+alt+c"). Een lege tekst haalt de sneltoets weg.
pub fn register_hotkey(app: &AppHandle, hotkey: &str) -> AppResult<()> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    if hotkey.trim().is_empty() {
        return Ok(());
    }
    let shortcut: Shortcut = hotkey.parse().map_err(|_| AppError::coded("hotkey_invalid").with("hotkey", hotkey))?;
    gs.register(shortcut).map_err(|_| AppError::coded("hotkey_taken").with("hotkey", hotkey))?;
    Ok(())
}

/// Plugin-instelling: de handler zet de overlay aan of uit bij het indrukken van de sneltoets.
pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                toggle(app);
            }
        })
        .build()
}
