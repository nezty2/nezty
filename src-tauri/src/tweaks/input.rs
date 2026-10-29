//! Muis en toetsenbord: muisversnelling uit, snelste toetsherhaling, sneltoetsen van plak-, filter-
//! en wisseltoetsen uit, en USB-energiebesparing uit. Alles omkeerbaar via het logboek.

use crate::error::{AppError, AppResult};
use crate::journal::{JournalEntry, PostAction, RevertPayload};
use crate::models::{JournalKind, Msg, TweakState};
use crate::powershell::run_cmd;
use crate::registry::{self, Hive, RegData};
use crate::state::{lock, AppState};
use crate::tweaks::regtweak::{self, RegSpec};
use crate::tweaks::{get, revert_via_journal, system, Status};
use crate::winapi;

// ---------- Muisversnelling ----------

fn mouse_specs() -> Vec<RegSpec> {
    let p = "Control Panel\\Mouse";
    vec![
        RegSpec::string(Hive::Hkcu, p, "MouseSpeed", "0", "1"),
        RegSpec::string(Hive::Hkcu, p, "MouseThreshold1", "0", "6"),
        RegSpec::string(Hive::Hkcu, p, "MouseThreshold2", "0", "10"),
    ]
}
pub fn mouse_status() -> AppResult<Status> {
    Ok(Status::simple(regtweak::state_of(&mouse_specs())?))
}
pub fn mouse_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    regtweak::apply(state, get("mouse_precision_off")?, &mouse_specs(), session, None, Some(PostAction::MouseSpi))?;
    winapi::apply_mouse_from_registry();
    Ok(())
}
pub fn mouse_disable(state: &AppState) -> AppResult<()> {
    regtweak::disable(state, get("mouse_precision_off")?, &mouse_specs(), None, Some(PostAction::MouseSpi))?;
    winapi::apply_mouse_from_registry();
    Ok(())
}

// ---------- Echt actief? ----------

/// Vergelijkt per aangezette tweak wat in het register staat met wat Windows nu echt gebruikt.
pub fn verify_live() -> Vec<crate::models::LiveCheck> {
    let live = winapi::live_input();
    let on = |r: AppResult<Status>| r.map(|s| s.state == TweakState::On).unwrap_or(false);
    vec![
        crate::models::LiveCheck { id: "mouse_precision_off".into(), saved: on(mouse_status()), live: live.mouse_acceleration == 0 },
        crate::models::LiveCheck { id: "keyboard_fast_repeat".into(), saved: on(keyboard_status()), live: live.keyboard_delay == 0 && live.keyboard_speed == 31 },
        crate::models::LiveCheck { id: "accessibility_hotkeys_off".into(), saved: on(accessibility_status()), live: !live.hotkeys.iter().any(|h| *h) },
    ]
}

/// Past de opgeslagen waarden opnieuw direct toe, zodat Windows ze meteen gebruikt.
pub fn sync_live() -> Vec<crate::models::LiveCheck> {
    winapi::apply_mouse_from_registry();
    winapi::apply_keyboard_from_registry();
    winapi::apply_accessibility_from_registry();
    verify_live()
}

// ---------- Toetsherhaling ----------

fn keyboard_specs() -> Vec<RegSpec> {
    let p = "Control Panel\\Keyboard";
    vec![
        // 0 = kortste wachttijd voor herhalen (250 ms), standaard 1 (500 ms).
        RegSpec::string(Hive::Hkcu, p, "KeyboardDelay", "0", "1"),
        // 31 = snelste herhaling (ongeveer 30 tekens per seconde), ook de standaard.
        RegSpec::string(Hive::Hkcu, p, "KeyboardSpeed", "31", "31"),
    ]
}
pub fn keyboard_status() -> AppResult<Status> {
    Ok(Status::simple(regtweak::state_of(&keyboard_specs())?))
}
pub fn keyboard_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    regtweak::apply(state, get("keyboard_fast_repeat")?, &keyboard_specs(), session, None, Some(PostAction::KeyboardSpi))?;
    winapi::apply_keyboard_from_registry();
    Ok(())
}
pub fn keyboard_disable(state: &AppState) -> AppResult<()> {
    regtweak::disable(state, get("keyboard_fast_repeat")?, &keyboard_specs(), None, Some(PostAction::KeyboardSpi))?;
    winapi::apply_keyboard_from_registry();
    Ok(())
}

// ---------- Sneltoetsen plak-, filter- en wisseltoetsen ----------

/// Registerplekken van de drie functies. Bit 0x4 is "sneltoets actief" bij alle drie.
pub const ACCESSIBILITY_KEYS: [&str; 3] = [
    "Control Panel\\Accessibility\\StickyKeys",
    "Control Panel\\Accessibility\\Keyboard Response",
    "Control Panel\\Accessibility\\ToggleKeys",
];
const DEFAULT_FLAGS: [u32; 3] = [510, 126, 62];
const HOTKEY_BIT: u32 = 0x4;

fn read_flags(path: &str, default: u32) -> AppResult<(RegData, u32)> {
    let cur = registry::read(Hive::Hkcu, path, "Flags")?;
    let val = cur.as_str().and_then(|s| s.trim().parse::<u32>().ok()).unwrap_or(default);
    Ok((cur, val))
}

pub fn accessibility_status() -> AppResult<Status> {
    for (i, path) in ACCESSIBILITY_KEYS.iter().enumerate() {
        let (_, flags) = read_flags(path, DEFAULT_FLAGS[i])?;
        if flags & HOTKEY_BIT != 0 {
            return Ok(Status::simple(TweakState::Off));
        }
    }
    Ok(Status::simple(TweakState::On))
}

fn set_hotkey_bits(state: &AppState, on: bool, session: Option<&str>, note: Option<&str>) -> AppResult<()> {
    let d = get("accessibility_hotkeys_off")?;
    for (i, path) in ACCESSIBILITY_KEYS.iter().enumerate() {
        let (old, flags) = read_flags(path, DEFAULT_FLAGS[i])?;
        let new_flags = if on { flags & !HOTKEY_BIT } else { flags | HOTKEY_BIT };
        if new_flags == flags && old != RegData::Absent {
            continue;
        }
        let new = RegData::Str(new_flags.to_string());
        registry::write(Hive::Hkcu, path, "Flags", &new)?;
        let mut e = JournalEntry::new(d.id, d.name, JournalKind::Registry, &registry::full_path(Hive::Hkcu, path, "Flags"));
        e.old_value = Some(old.display());
        e.new_value = Some(new.display());
        e.revertable = true;
        e.session_id = session.map(|s| s.to_string());
        e.note = note.map(|s| s.to_string());
        e.payload = RevertPayload::Registry {
            hive: Hive::Hkcu,
            path: path.to_string(),
            name: "Flags".into(),
            old,
            post: Some(PostAction::AccessibilitySpi),
        };
        lock(&state.journal).record(e)?;
    }
    winapi::apply_accessibility_from_registry();
    Ok(())
}

pub fn accessibility_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    set_hotkey_bits(state, true, session, None)
}
pub fn accessibility_disable(state: &AppState) -> AppResult<()> {
    if revert_via_journal(state, "accessibility_hotkeys_off", None)? && accessibility_status()?.state == TweakState::Off {
        return Ok(());
    }
    set_hotkey_bits(state, false, None, Some("manual_off"))
}

// ---------- USB-energiebesparing (selective suspend) ----------

const USB_SUB: &str = "2a737441-1930-4402-8d77-b2bebba308a3";
const USB_SETTING: &str = "48e6b7a6-50f5-4782-a5d4-53bb8f07e226";

/// Leest (AC, DC) van een energie-instelling in het opgegeven schema. De labels van powercfg zijn
/// vertaald, daarom lezen we de laatste twee hexadecimale waarden (huidige AC- en DC-index).
pub fn read_power_setting(scheme: &str, sub: &str, setting: &str) -> AppResult<Option<(u32, u32)>> {
    let (ok, out, _) = run_cmd("powercfg", &["/query", scheme, sub, setting])?;
    if !ok {
        return Ok(None);
    }
    let re = regex::Regex::new(r"0x([0-9a-fA-F]{8})").map_err(|e| AppError::msg(e.to_string()))?;
    let vals: Vec<u32> = re.captures_iter(&out).filter_map(|c| u32::from_str_radix(&c[1], 16).ok()).collect();
    if vals.len() < 2 {
        return Ok(None);
    }
    Ok(Some((vals[vals.len() - 2], vals[vals.len() - 1])))
}

pub fn write_power_setting(scheme: &str, sub: &str, setting: &str, ac: u32, dc: u32) -> AppResult<()> {
    let ac_s = ac.to_string();
    let dc_s = dc.to_string();
    for (flag, val) in [("/setacvalueindex", &ac_s), ("/setdcvalueindex", &dc_s)] {
        let (ok, _, err) = run_cmd("powercfg", &[flag, scheme, sub, setting, val])?;
        if !ok {
            let msg = err.trim().to_lowercase();
            if msg.contains("denied") || msg.contains("geweigerd") || msg.contains("refusé") {
                return Err(AppError::NeedsAdmin("powercfg".into()));
            }
            return Err(AppError::msg(format!("powercfg: {}", err.trim())));
        }
    }
    // Opnieuw activeren past de waarde direct toe, maar alleen als dit schema nog actief is.
    if let Ok((active, _)) = system::active_scheme() {
        if active == scheme.to_lowercase() {
            let _ = run_cmd("powercfg", &["/setactive", scheme]);
        }
    }
    Ok(())
}

pub fn usb_suspend_status() -> AppResult<Status> {
    let (scheme, _) = system::active_scheme()?;
    match read_power_setting(&scheme, USB_SUB, USB_SETTING)? {
        None => Ok(Status::with_detail(TweakState::Unavailable, Msg::new("usb_unavailable"))),
        Some((ac, dc)) => Ok(Status::simple(if ac == 0 && dc == 0 { TweakState::On } else { TweakState::Off })),
    }
}

fn usb_set(state: &AppState, value: u32, session: Option<&str>, note: Option<&str>) -> AppResult<()> {
    let d = get("usb_suspend_off")?;
    let (scheme, scheme_name) = system::active_scheme()?;
    let Some((old_ac, old_dc)) = read_power_setting(&scheme, USB_SUB, USB_SETTING)? else {
        return Err(AppError::coded("usb_unavailable"));
    };
    if old_ac == value && old_dc == value {
        return Ok(());
    }
    write_power_setting(&scheme, USB_SUB, USB_SETTING, value, value)?;
    let mut e = JournalEntry::new(d.id, d.name, JournalKind::PowerSetting, &format!("{scheme_name}: USB selective suspend"));
    e.old_value = Some(format!("AC {old_ac}, DC {old_dc}"));
    e.new_value = Some(format!("AC {value}, DC {value}"));
    e.revertable = true;
    e.session_id = session.map(|s| s.to_string());
    e.note = note.map(|s| s.to_string());
    e.payload = RevertPayload::PowerSetting {
        scheme,
        sub: USB_SUB.into(),
        setting: USB_SETTING.into(),
        ac: old_ac,
        dc: old_dc,
    };
    lock(&state.journal).record(e)?;
    Ok(())
}

pub fn usb_suspend_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    usb_set(state, 0, session, None)
}
pub fn usb_suspend_disable(state: &AppState) -> AppResult<()> {
    if revert_via_journal(state, "usb_suspend_off", None)? && usb_suspend_status()?.state == TweakState::Off {
        return Ok(());
    }
    usb_set(state, 1, None, Some("manual_off"))
}
