//! Pictogram in het systeemvak (rechtsonder bij de klok) en het naar voren halen van het hoofdvenster.
//! Bij starten met Windows kan nezty verborgen in het systeemvak starten. Dubbelklikken op de app,
//! of klikken op het pictogram, haalt het venster altijd naar voren.

use crate::winapi;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

struct Items {
    open: MenuItem<Wry>,
    crosshair: MenuItem<Wry>,
    quit: MenuItem<Wry>,
}

fn labels(lang: &str) -> [&'static str; 3] {
    match lang {
        "nl" => ["nezty openen", "Crosshair aan of uit", "Afsluiten"],
        "fr" => ["Ouvrir nezty", "Afficher ou masquer le viseur", "Quitter"],
        _ => ["Open nezty", "Crosshair on or off", "Quit"],
    }
}

/// Toont het hoofdvenster, haalt het uit geminimaliseerd en zet het op de voorgrond.
pub fn show_main(app: &AppHandle) {
    let Some(w) = app.get_webview_window("main") else { return };
    let _ = w.show();
    let _ = w.unminimize();
    let _ = w.set_focus();
    if let Ok(hwnd) = w.hwnd() {
        let raw = hwnd.0 as isize;
        let _ = app.run_on_main_thread(move || winapi::force_foreground(raw));
    }
}

pub fn create(app: &AppHandle, lang: &str) -> tauri::Result<()> {
    let [open_l, crosshair_l, quit_l] = labels(lang);
    let open = MenuItem::with_id(app, "open", open_l, true, None::<&str>)?;
    let crosshair = MenuItem::with_id(app, "crosshair", crosshair_l, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", quit_l, true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &crosshair, &sep, &quit])?;
    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("nezty")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "crosshair" => crate::crosshair::toggle(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    app.manage(Items { open, crosshair, quit });
    Ok(())
}

/// Past de teksten in het menu aan na het wisselen van taal.
pub fn set_language(app: &AppHandle, lang: &str) {
    if let Some(items) = app.try_state::<Items>() {
        let [open_l, crosshair_l, quit_l] = labels(lang);
        let _ = items.open.set_text(open_l);
        let _ = items.crosshair.set_text(crosshair_l);
        let _ = items.quit.set_text(quit_l);
    }
}
