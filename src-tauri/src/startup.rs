//! Opstart-apps uit de Run-sleutels en Opstartmappen, in- en uitschakelen zoals Taakbeheer dat doet
//! (StartupApproved-sleutels).

use crate::error::{AppError, AppResult};
use crate::journal::{JournalEntry, RevertPayload};
use crate::models::{JournalKind, StartupItem, StartupSource};
use crate::registry::{self, Hive, RegData};
use crate::state::{lock, AppState};
use std::path::PathBuf;

const RUN: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN32: &str = "Software\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Run";
const APPROVED: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved";

fn source_slug(s: StartupSource) -> &'static str {
    match s {
        StartupSource::HkcuRun => "hkcu_run",
        StartupSource::HklmRun => "hklm_run",
        StartupSource::HklmRun32 => "hklm_run32",
        StartupSource::UserFolder => "user_folder",
        StartupSource::CommonFolder => "common_folder",
    }
}

fn source_from_slug(s: &str) -> Option<StartupSource> {
    Some(match s {
        "hkcu_run" => StartupSource::HkcuRun,
        "hklm_run" => StartupSource::HklmRun,
        "hklm_run32" => StartupSource::HklmRun32,
        "user_folder" => StartupSource::UserFolder,
        "common_folder" => StartupSource::CommonFolder,
        _ => return None,
    })
}

/// (hive, StartupApproved-subsleutel) voor een bron.
fn approved_key(source: StartupSource) -> (Hive, String) {
    match source {
        StartupSource::HkcuRun => (Hive::Hkcu, format!("{APPROVED}\\Run")),
        StartupSource::HklmRun => (Hive::Hklm, format!("{APPROVED}\\Run")),
        StartupSource::HklmRun32 => (Hive::Hklm, format!("{APPROVED}\\Run32")),
        StartupSource::UserFolder => (Hive::Hkcu, format!("{APPROVED}\\StartupFolder")),
        StartupSource::CommonFolder => (Hive::Hklm, format!("{APPROVED}\\StartupFolder")),
    }
}

fn is_enabled(approved: &RegData) -> bool {
    match approved {
        RegData::Binary(b) if !b.is_empty() => b[0] & 1 == 0,
        _ => true,
    }
}

fn folder(source: StartupSource) -> Option<PathBuf> {
    match source {
        StartupSource::UserFolder => std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Microsoft\\Windows\\Start Menu\\Programs\\Startup")),
        StartupSource::CommonFolder => std::env::var_os("PROGRAMDATA").map(|a| PathBuf::from(a).join("Microsoft\\Windows\\Start Menu\\Programs\\Startup")),
        _ => None,
    }
}

/// Haalt het programmapad uit een opstartopdracht ("C:\pad\app.exe" --vlag of %windir%\x.exe).
fn exe_from_command(cmd: &str) -> Option<String> {
    let re = regex::Regex::new(r"%([^%]+)%").ok()?;
    let expanded = re.replace_all(cmd.trim(), |c: &regex::Captures| std::env::var(&c[1]).unwrap_or_else(|_| c[0].to_string())).to_string();
    let path = if let Some(rest) = expanded.strip_prefix('"') {
        rest.split('"').next()?.to_string()
    } else {
        let lower = expanded.to_lowercase();
        let end = lower.find(".exe")? + 4;
        expanded[..end].to_string()
    };
    if std::path::Path::new(&path).is_file() {
        Some(path)
    } else {
        None
    }
}

pub fn list() -> AppResult<Vec<StartupItem>> {
    let mut out = vec![];
    let reg_sources = [
        (StartupSource::HkcuRun, Hive::Hkcu, RUN),
        (StartupSource::HklmRun, Hive::Hklm, RUN),
        (StartupSource::HklmRun32, Hive::Hklm, RUN32),
    ];
    for (source, hive, path) in reg_sources {
        let (ahive, apath) = approved_key(source);
        for (name, data) in registry::values(hive, path)? {
            let command = data.as_str().map(|s| s.to_string()).unwrap_or_else(|| data.display());
            let approved = registry::read(ahive, &apath, &name).unwrap_or(RegData::Absent);
            out.push(StartupItem {
                id: format!("{}:{}", source_slug(source), name),
                name: name.clone(),
                exe_path: exe_from_command(&command),
                command,
                source,
                enabled: is_enabled(&approved),
                requires_admin: hive.needs_admin(),
            });
        }
    }
    for source in [StartupSource::UserFolder, StartupSource::CommonFolder] {
        let Some(dir) = folder(source) else { continue };
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        let (ahive, apath) = approved_key(source);
        for entry in rd.flatten() {
            let fname = entry.file_name().to_string_lossy().to_string();
            if fname.eq_ignore_ascii_case("desktop.ini") {
                continue;
            }
            let approved = registry::read(ahive, &apath, &fname).unwrap_or(RegData::Absent);
            let display = fname.rsplit_once('.').map(|(n, _)| n.to_string()).unwrap_or(fname.clone());
            out.push(StartupItem {
                id: format!("{}:{}", source_slug(source), fname),
                name: display,
                exe_path: Some(entry.path().to_string_lossy().to_string()),
                command: entry.path().to_string_lossy().to_string(),
                source,
                enabled: is_enabled(&approved),
                requires_admin: ahive.needs_admin(),
            });
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

fn filetime_now() -> [u8; 8] {
    // FILETIME: 100ns-intervallen sinds 1601-01-01.
    let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let ft = (unix / 100) as u64 + 116_444_736_000_000_000u64;
    ft.to_le_bytes()
}

pub fn set_enabled(state: &AppState, item_id: &str, enabled: bool) -> AppResult<StartupItem> {
    let (slug, name) = item_id.split_once(':').ok_or_else(|| AppError::msg("invalid startup item"))?;
    let source = source_from_slug(slug).ok_or_else(|| AppError::msg("unknown startup source"))?;
    let (hive, path) = approved_key(source);
    if hive.needs_admin() && !state.elevated {
        return Err(AppError::NeedsAdmin(format!("startup item {name}")));
    }
    let old = registry::read(hive, &path, name)?;
    let new = if enabled {
        RegData::Binary(vec![0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
    } else {
        let mut b = vec![0x03, 0, 0, 0];
        b.extend_from_slice(&filetime_now());
        RegData::Binary(b)
    };
    if is_enabled(&old) != enabled {
        registry::write(hive, &path, name, &new)?;
        let mut e = JournalEntry::new("startup_apps", "Startup apps", JournalKind::Startup, name);
        e.old_value = Some(if is_enabled(&old) { "@enabled".into() } else { "@disabled".into() });
        e.new_value = Some(if enabled { "@enabled".into() } else { "@disabled".into() });
        e.revertable = true;
        e.payload = RevertPayload::Registry { hive, path: path.clone(), name: name.to_string(), old, post: None };
        lock(&state.journal).record(e)?;
    }
    list()?
        .into_iter()
        .find(|i| i.id == item_id)
        .ok_or_else(|| AppError::NotFound(format!("startup item {name}")))
}
