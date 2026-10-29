//! Fortnite: GameUserSettings.ini lezen en (met back-up) aanpassen.

use crate::error::{AppError, AppResult};
use crate::journal::{JournalEntry, RevertPayload};
use crate::models::{FortniteSettings, JournalKind};
use crate::state::{lock, AppState};
use std::path::{Path, PathBuf};

const SECTION: &str = "[/Script/FortniteGame.FortGameUserSettings]";

pub fn ini_path() -> PathBuf {
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_default();
    local.join("FortniteGame\\Saved\\Config\\WindowsClient\\GameUserSettings.ini")
}

fn read_text(path: &Path) -> AppResult<String> {
    let bytes = std::fs::read(path)?;
    // UTF-16 LE met BOM komt voor bij Unreal-ini's.
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        let u16s: Vec<u16> = bytes[2..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        return Ok(String::from_utf16_lossy(&u16s));
    }
    let s = String::from_utf8_lossy(&bytes).to_string();
    Ok(s.trim_start_matches('\u{feff}').to_string())
}

fn section_value(text: &str, key: &str) -> Option<String> {
    let mut in_section = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_section = t.eq_ignore_ascii_case(SECTION);
            continue;
        }
        if in_section {
            if let Some((k, v)) = t.split_once('=') {
                if k.trim().eq_ignore_ascii_case(key) {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
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
    let text = if exists { read_text(&path)? } else { String::new() };
    let num = |k: &str| section_value(&text, k).and_then(|v| v.parse::<f32>().ok());
    let int = |k: &str| section_value(&text, k).and_then(|v| v.parse::<f32>().ok()).map(|f| f as u32);
    Ok(FortniteSettings {
        ini_path: path.to_string_lossy().to_string(),
        exists,
        read_only,
        frame_rate_limit: num("FrameRateLimit"),
        vsync: section_value(&text, "bUseVSync").map(|v| v.eq_ignore_ascii_case("true")),
        fullscreen_mode: int("FullscreenMode"),
        res_x: int("ResolutionSizeX"),
        res_y: int("ResolutionSizeY"),
        backups: list_backups(&path),
    })
}

/// Zet een sleutel in de Fortnite-sectie; voegt toe als hij ontbreekt. Behoudt de rest van het bestand.
fn set_key(text: &mut String, key: &str, value: &str) {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = text.split(newline).map(|s| s.to_string()).collect();
    let mut section_start: Option<usize> = None;
    let mut in_section = false;
    for (i, line) in lines.iter_mut().enumerate() {
        let t = line.trim();
        if t.starts_with('[') {
            in_section = t.eq_ignore_ascii_case(SECTION);
            if in_section {
                section_start = Some(i);
            }
            continue;
        }
        if in_section {
            if let Some((k, _)) = t.split_once('=') {
                if k.trim().eq_ignore_ascii_case(key) {
                    *line = format!("{key}={value}");
                    *text = lines.join(newline);
                    return;
                }
            }
        }
    }
    match section_start {
        Some(i) => lines.insert(i + 1, format!("{key}={value}")),
        None => {
            if !lines.last().map(|l| l.is_empty()).unwrap_or(true) {
                lines.push(String::new());
            }
            lines.push(SECTION.to_string());
            lines.push(format!("{key}={value}"));
        }
    }
    *text = lines.join(newline);
}

pub fn backup(path: &Path) -> AppResult<PathBuf> {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let backup = path.with_file_name(format!("GameUserSettings.ini.nzty-backup-{stamp}"));
    std::fs::copy(path, &backup)?;
    Ok(backup)
}

pub fn save(state: &AppState, settings: &FortniteSettings) -> AppResult<FortniteSettings> {
    let path = ini_path();
    if !path.is_file() {
        return Err(AppError::coded("ini_missing"));
    }
    let backup_path = backup(&path)?;
    let mut text = read_text(&path)?;
    if let Some(v) = settings.frame_rate_limit {
        set_key(&mut text, "FrameRateLimit", &format!("{:.6}", v));
    }
    if let Some(v) = settings.vsync {
        set_key(&mut text, "bUseVSync", if v { "True" } else { "False" });
    }
    if let Some(v) = settings.fullscreen_mode {
        set_key(&mut text, "FullscreenMode", &v.to_string());
        set_key(&mut text, "LastConfirmedFullscreenMode", &v.to_string());
        set_key(&mut text, "PreferredFullscreenMode", &v.to_string());
    }
    if let (Some(x), Some(y)) = (settings.res_x, settings.res_y) {
        set_key(&mut text, "ResolutionSizeX", &x.to_string());
        set_key(&mut text, "ResolutionSizeY", &y.to_string());
        set_key(&mut text, "LastUserConfirmedResolutionSizeX", &x.to_string());
        set_key(&mut text, "LastUserConfirmedResolutionSizeY", &y.to_string());
    }
    let was_readonly = std::fs::metadata(&path)?.permissions().readonly();
    if was_readonly {
        let mut p = std::fs::metadata(&path)?.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        p.set_readonly(false);
        std::fs::set_permissions(&path, p)?;
    }
    std::fs::write(&path, text.as_bytes())?;
    if was_readonly {
        let mut p = std::fs::metadata(&path)?.permissions();
        p.set_readonly(true);
        std::fs::set_permissions(&path, p)?;
    }
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
