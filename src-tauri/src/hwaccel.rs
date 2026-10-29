//! Hardwareversnelling van Discord en browsers. Uit betekent minder werk voor je videokaart terwijl je
//! gamet. Alleen aan te passen als de app dicht is, anders schrijft die app zijn eigen bestand terug.

use crate::error::{AppError, AppResult};
use crate::journal::{JournalEntry, RevertPayload};
use crate::models::{HwAccelInfo, JournalKind};
use crate::state::{lock, AppState};
use crate::{jsonfile, processes};
use serde_json::Value;
use std::path::PathBuf;

struct Target {
    id: &'static str,
    name: &'static str,
    process: &'static str,
    roaming: bool,
    rel: &'static str,
    pointer: &'static str,
    pretty: bool,
}

const CHROMIUM: &str = "/hardware_acceleration_mode/enabled";
const TARGETS: &[Target] = &[
    Target { id: "discord", name: "Discord", process: "discord.exe", roaming: true, rel: "discord\\settings.json", pointer: "/enableHardwareAcceleration", pretty: true },
    Target { id: "chrome", name: "Google Chrome", process: "chrome.exe", roaming: false, rel: "Google\\Chrome\\User Data\\Local State", pointer: CHROMIUM, pretty: false },
    Target { id: "edge", name: "Microsoft Edge", process: "msedge.exe", roaming: false, rel: "Microsoft\\Edge\\User Data\\Local State", pointer: CHROMIUM, pretty: false },
    Target { id: "brave", name: "Brave", process: "brave.exe", roaming: false, rel: "BraveSoftware\\Brave-Browser\\User Data\\Local State", pointer: CHROMIUM, pretty: false },
    Target { id: "opera_gx", name: "Opera GX", process: "opera.exe", roaming: true, rel: "Opera Software\\Opera GX Stable\\Local State", pointer: CHROMIUM, pretty: false },
];

fn path_of(t: &Target) -> PathBuf {
    let base = std::env::var_os(if t.roaming { "APPDATA" } else { "LOCALAPPDATA" }).map(PathBuf::from).unwrap_or_default();
    base.join(t.rel)
}

/// Alleen apps die geïnstalleerd zijn (het instellingenbestand bestaat).
pub fn list(state: &AppState) -> Vec<HwAccelInfo> {
    let running = processes::running_names(state);
    TARGETS
        .iter()
        .filter_map(|t| {
            let path = path_of(t);
            if !path.is_file() {
                return None;
            }
            // Ontbreekt de waarde, dan staat hardwareversnelling aan (standaard in Chromium en Discord).
            let enabled = jsonfile::get(&path, t.pointer).ok()?.and_then(|v| v.as_bool()).unwrap_or(true);
            Some(HwAccelInfo { id: t.id.into(), name: t.name.into(), enabled, running: running.contains(t.process) })
        })
        .collect()
}

pub fn set(state: &AppState, id: &str, enabled: bool) -> AppResult<Vec<HwAccelInfo>> {
    let t = TARGETS.iter().find(|t| t.id == id).ok_or_else(|| AppError::NotFound(id.into()))?;
    let path = path_of(t);
    if !path.is_file() {
        return Err(AppError::NotFound(path.to_string_lossy().to_string()));
    }
    if processes::running_names(state).contains(t.process) {
        return Err(AppError::coded("close_app_first").with("app", t.name));
    }
    let old = jsonfile::get(&path, t.pointer)?;
    if old.as_ref().and_then(Value::as_bool).unwrap_or(true) == enabled {
        return Ok(list(state));
    }
    // Extra veiligheidskopie naast het bestand.
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let file = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    std::fs::copy(&path, path.with_file_name(format!("{file}.nezty-backup-{stamp}")))?;
    jsonfile::set_pointer(&path, t.pointer, Some(Value::Bool(enabled)), t.pretty)?;

    let mut e = JournalEntry::new("hw_accel", &format!("Hardware acceleration {}", t.name), JournalKind::File, &path.to_string_lossy());
    e.old_value = Some(if old.as_ref().and_then(Value::as_bool).unwrap_or(true) { "on".into() } else { "off".into() });
    e.new_value = Some(if enabled { "on".into() } else { "off".into() });
    e.revertable = true;
    e.payload = RevertPayload::JsonKey {
        path: path.to_string_lossy().to_string(),
        pointer: t.pointer.into(),
        old: old.map(|v| v.to_string()),
        pretty: t.pretty,
    };
    lock(&state.journal).record(e)?;
    Ok(list(state))
}
