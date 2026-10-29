//! Fortnite: GameUserSettings.ini lezen en (met back-up) aanpassen, plus de aanbevolen instellingen
//! voor snel bouwen en editten. Fortnite schrijft zijn instellingen bij het afsluiten terug, dus
//! wijzigen kan alleen als Fortnite dicht is.

use crate::error::{AppError, AppResult};
use crate::journal::{JournalEntry, RevertPayload};
use crate::models::{FortniteCheck, FortniteOverview, FortniteSettings, JournalKind};
use crate::state::{lock, AppState};
use crate::{bigwins, display, ini, processes};
use std::path::{Path, PathBuf};

const SECTION: &str = "[/Script/FortniteGame.FortGameUserSettings]";
const RHI_SECTION: &str = "[D3DRHIPreference]";
const PROCESSES: [&str; 4] = [
    "fortniteclient-win64-shipping.exe",
    "fortniteclient-win64-shipping_eac_eos.exe",
    "fortniteclient-win64-shipping_be.exe",
    "fortnitelauncher.exe",
];

pub fn ini_path() -> PathBuf {
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_default();
    local.join("FortniteGame\\Saved\\Config\\WindowsClient\\GameUserSettings.ini")
}

pub fn is_running(state: &AppState) -> bool {
    let names = processes::running_names(state);
    PROCESSES.iter().any(|p| names.contains(*p))
}

fn list_backups(path: &Path) -> Vec<String> {
    let Some(dir) = path.parent() else { return vec![] };
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut v: Vec<String> = rd
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.contains(".nzty-backup-"))
        .collect();
    v.sort_by(|a, b| b.cmp(a));
    v
}

pub fn read() -> AppResult<FortniteSettings> {
    let path = ini_path();
    let exists = path.is_file();
    let read_only = std::fs::metadata(&path).map(|m| m.permissions().readonly()).unwrap_or(false);
    let text = if exists { ini::read_text(&path)?.0 } else { String::new() };
    let num = |k: &str| ini::value(&text, SECTION, k).and_then(|v| v.parse::<f32>().ok());
    let int = |k: &str| num(k).map(|f| f as u32);
    Ok(FortniteSettings {
        ini_path: path.to_string_lossy().to_string(),
        exists,
        read_only,
        frame_rate_limit: num("FrameRateLimit"),
        vsync: ini::value(&text, SECTION, "bUseVSync").map(|v| v.eq_ignore_ascii_case("true")),
        fullscreen_mode: int("FullscreenMode"),
        res_x: int("ResolutionSizeX"),
        res_y: int("ResolutionSizeY"),
        backups: list_backups(&path),
    })
}

pub fn backup(path: &Path) -> AppResult<PathBuf> {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let backup = path.with_file_name(format!("GameUserSettings.ini.nzty-backup-{stamp}"));
    std::fs::copy(path, &backup)?;
    Ok(backup)
}

fn ensure_editable(state: &AppState, path: &Path) -> AppResult<()> {
    if !path.is_file() {
        return Err(AppError::coded("ini_missing"));
    }
    if is_running(state) {
        return Err(AppError::coded("fortnite_running"));
    }
    Ok(())
}

pub fn save(state: &AppState, settings: &FortniteSettings) -> AppResult<FortniteSettings> {
    let path = ini_path();
    ensure_editable(state, &path)?;
    let backup_path = backup(&path)?;
    let (mut text, utf16) = ini::read_text(&path)?;
    if let Some(v) = settings.frame_rate_limit {
        ini::set(&mut text, SECTION, "FrameRateLimit", &format!("{:.6}", v));
    }
    if let Some(v) = settings.vsync {
        ini::set(&mut text, SECTION, "bUseVSync", if v { "True" } else { "False" });
    }
    if let Some(v) = settings.fullscreen_mode {
        for k in ["FullscreenMode", "LastConfirmedFullscreenMode", "PreferredFullscreenMode"] {
            ini::set(&mut text, SECTION, k, &v.to_string());
        }
    }
    if let (Some(x), Some(y)) = (settings.res_x, settings.res_y) {
        ini::set(&mut text, SECTION, "ResolutionSizeX", &x.to_string());
        ini::set(&mut text, SECTION, "ResolutionSizeY", &y.to_string());
        ini::set(&mut text, SECTION, "LastUserConfirmedResolutionSizeX", &x.to_string());
        ini::set(&mut text, SECTION, "LastUserConfirmedResolutionSizeY", &y.to_string());
    }
    ini::write_text(&path, &text, utf16)?;
    let mut e = JournalEntry::new("fortnite_ini", "Fortnite GameUserSettings.ini", JournalKind::File, &path.to_string_lossy());
    e.old_value = Some(format!("backup: {}", backup_path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()));
    e.new_value = Some(format!(
        "FrameRateLimit {}, VSync {}, FullscreenMode {}, Resolution {}x{}",
        settings.frame_rate_limit.map(|v| v.to_string()).unwrap_or("-".into()),
        settings.vsync.map(|v| if v { "on" } else { "off" }).unwrap_or("-"),
        settings.fullscreen_mode.map(|v| v.to_string()).unwrap_or("-".into()),
        settings.res_x.map(|v| v.to_string()).unwrap_or("-".into()),
        settings.res_y.map(|v| v.to_string()).unwrap_or("-".into()),
    ));
    e.revertable = true;
    e.payload = RevertPayload::File { path: path.to_string_lossy().to_string(), backup_path: backup_path.to_string_lossy().to_string() };
    lock(&state.journal).record(e)?;
    read()
}

pub fn restore_backup(state: &AppState, backup_name: &str) -> AppResult<FortniteSettings> {
    let path = ini_path();
    if is_running(state) {
        return Err(AppError::coded("fortnite_running"));
    }
    let dir = path.parent().ok_or_else(|| AppError::msg("unknown folder"))?;
    let file = Path::new(backup_name).file_name().ok_or_else(|| AppError::msg("invalid backup name"))?;
    let backup_path = dir.join(file);
    if !backup_path.is_file() {
        return Err(AppError::NotFound(backup_path.to_string_lossy().to_string()));
    }
    // Eerst de huidige versie veiligstellen, dan de back-up terugzetten.
    let safety = if path.is_file() { Some(backup(&path)?) } else { None };
    crate::journal::execute_revert(&RevertPayload::File {
        path: path.to_string_lossy().to_string(),
        backup_path: backup_path.to_string_lossy().to_string(),
    })?;
    let mut e = JournalEntry::new("fortnite_ini", "Fortnite GameUserSettings.ini", JournalKind::File, &path.to_string_lossy());
    e.old_value = safety.map(|s| format!("saved as {}", s.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()));
    e.new_value = Some(format!("restored from {}", file.to_string_lossy()));
    e.revertable = false;
    lock(&state.journal).record(e)?;
    read()
}

// ---------- Aanbevolen instellingen ----------

/// De snelle videokaart (los, anders de eerste) met merk.
fn main_gpu() -> (String, Option<String>, bool) {
    let layout = bigwins::gpu_layout();
    let gpu = layout.gpus.iter().find(|g| g.discrete).or(layout.gpus.first());
    match gpu {
        Some(g) => (g.vendor.to_lowercase(), Some(g.name.clone()), g.discrete),
        None => ("unknown".into(), None, false),
    }
}

fn render_mode(text: &str) -> &'static str {
    let level = ini::value(text, RHI_SECTION, "PreferredFeatureLevel").unwrap_or_default().to_lowercase();
    let rhi = ini::value(text, RHI_SECTION, "PreferredRHI").unwrap_or_default().to_lowercase();
    if level == "es31" {
        "performance"
    } else if rhi == "dx12" {
        "dx12"
    } else if rhi == "dx11" {
        "dx11"
    } else {
        "default"
    }
}

/// De controles voor een ini-tekst. Met NVIDIA Reflex mag de framerate onbeperkt, anders gelijk aan het scherm.
pub fn checks_for(text: &str, nvidia: bool, max_hz: u32) -> Vec<FortniteCheck> {
    let mut checks = vec![];
    let vsync = ini::value(text, SECTION, "bUseVSync");
    checks.push(FortniteCheck {
        id: "vsync".into(),
        ok: vsync.as_deref().is_some_and(|v| v.eq_ignore_ascii_case("false")),
        current: vsync,
        recommended: "False".into(),
    });
    let fps_target: f32 = if nvidia { 0.0 } else { max_hz as f32 };
    let fps = ini::value(text, SECTION, "FrameRateLimit");
    checks.push(FortniteCheck {
        id: "fps_limit".into(),
        ok: fps.as_deref().and_then(|v| v.parse::<f32>().ok()).is_some_and(|v| (v - fps_target).abs() < 0.5),
        current: fps,
        recommended: format!("{fps_target:.0}"),
    });
    if nvidia {
        let reflex = ini::value(text, SECTION, "LatencyTweak2");
        checks.push(FortniteCheck { id: "reflex".into(), ok: reflex.as_deref() == Some("2"), current: reflex, recommended: "2".into() });
    }
    let mode = render_mode(text);
    checks.push(FortniteCheck { id: "render_mode".into(), ok: mode == "performance", current: Some(mode.into()), recommended: "performance".into() });
    checks
}

pub fn overview(state: &AppState) -> AppResult<FortniteOverview> {
    let path = ini_path();
    let exists = path.is_file();
    let text = if exists { ini::read_text(&path)?.0 } else { String::new() };
    let (vendor, gpu_name, discrete) = main_gpu();
    let nvidia = vendor.contains("nvidia");
    let displays = display::list();
    let max_hz = displays.iter().find(|d| d.primary).or(displays.first()).map(|d| d.max_hz).unwrap_or(60);
    let advice = if !discrete {
        "igpu"
    } else if nvidia {
        "nvidia"
    } else if vendor.contains("amd") || vendor.contains("advanced micro") {
        "amd"
    } else {
        "other"
    };
    Ok(FortniteOverview {
        ini_exists: exists,
        running: is_running(state),
        gpu_vendor: vendor,
        gpu_name,
        max_hz,
        render_mode: render_mode(&text).into(),
        advice: advice.into(),
        checks: checks_for(&text, nvidia, max_hz),
    })
}

/// Welke sleutels er moeten veranderen voor de gekozen aanbevelingen. Wat al goed staat, valt weg.
pub fn changes_for(checks: &[FortniteCheck], ids: &[String]) -> Vec<(&'static str, &'static str, String)> {
    let mut changes = vec![];
    for c in checks.iter().filter(|c| !c.ok && ids.contains(&c.id)) {
        match c.id.as_str() {
            "vsync" => changes.push((SECTION, "bUseVSync", "False".to_string())),
            "fps_limit" => changes.push((SECTION, "FrameRateLimit", format!("{:.6}", c.recommended.parse::<f32>().unwrap_or(0.0)))),
            "reflex" => changes.push((SECTION, "LatencyTweak2", "2".to_string())),
            "render_mode" => changes.push((RHI_SECTION, "PreferredFeatureLevel", "es31".to_string())),
            _ => {}
        }
    }
    changes
}

/// Schrijft de wijzigingen en geeft per sleutel een logboekregel terug (nog niet opgeslagen).
pub fn write_changes(path: &Path, changes: &[(&str, &str, String)]) -> AppResult<Vec<JournalEntry>> {
    let (mut text, utf16) = ini::read_text(path)?;
    let mut entries = vec![];
    for (section, key, value) in changes {
        let old = ini::value(&text, section, key);
        ini::set(&mut text, section, key, value);
        let mut e = JournalEntry::new("fortnite_ini", &format!("Fortnite: {key}"), JournalKind::File, &format!("GameUserSettings.ini {section} {key}"));
        e.old_value = Some(old.clone().unwrap_or_else(|| "-".into()));
        e.new_value = Some(value.clone());
        e.revertable = true;
        e.payload = RevertPayload::IniKey { path: path.to_string_lossy().to_string(), section: section.to_string(), key: key.to_string(), old };
        entries.push(e);
    }
    ini::write_text(path, &text, utf16)?;
    Ok(entries)
}

/// Zet de gekozen aanbevelingen (id's uit `overview`). Wat al goed staat, blijft ongemoeid. Elke
/// gewijzigde sleutel komt apart in het logboek, zodat terugzetten precies die ene waarde herstelt.
pub fn apply_recommended(state: &AppState, ids: &[String]) -> AppResult<FortniteOverview> {
    let path = ini_path();
    ensure_editable(state, &path)?;
    let before = overview(state)?;
    let changes = changes_for(&before.checks, ids);
    if changes.is_empty() {
        return Ok(before);
    }
    backup(&path)?;
    let entries = write_changes(&path, &changes)?;
    let mut journal = lock(&state.journal);
    for e in entries {
        journal.record(e)?;
    }
    drop(journal);
    overview(state)
}
