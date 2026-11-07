//! Opstart-apps uit de Run-sleutels en Opstartmappen, in- en uitschakelen zoals Taakbeheer dat doet
//! (StartupApproved-sleutels).
//! Items van Windows-beveiliging (SecurityHealth), andere beveiligingssoftware, anti-cheat en Windows zelf worden
//! niet getoond en nooit uitgeschakeld (`is_protected`).

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

/// Beschermd opstartitem: nooit tonen, nooit uitschakelen. Kijkt naar de naam, de opdracht en het programma.
fn is_protected(name: &str, command: &str, exe_path: Option<&str>) -> bool {
    let name_l = name.to_lowercase();
    if name_l == "securityhealth" || name_l.starts_with("windows defender") || name_l.starts_with("windows security") {
        return true;
    }
    let cmd_l = command.to_lowercase();
    let exe_l = exe_path.map(|e| e.to_lowercase()).unwrap_or_default();
    for hay in [cmd_l.as_str(), exe_l.as_str()] {
        if hay.contains("windows defender") || hay.contains("microsoft.sechealthui") || hay.contains("securityhealth") {
            return true;
        }
    }
    // Ook de naam van het item zelf (bv. "Avast Antivirus" of "Riot Vanguard"), los van het pad.
    if crate::processes::is_protected_program(&name_l, "") {
        return true;
    }
    // Het programma: beveiliging, anti-cheat of iets uit de Windows-map. Zonder bekend programma alleen de naam.
    if exe_path.is_some() {
        let file = std::path::Path::new(&exe_l).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
        if crate::processes::is_protected_program(&file, &exe_l) {
            return true;
        }
    }
    false
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
            let exe_path = exe_from_command(&command);
            if is_protected(&name, &command, exe_path.as_deref()) {
                continue;
            }
            let approved = registry::read(ahive, &apath, &name).unwrap_or(RegData::Absent);
            out.push(StartupItem {
                id: format!("{}:{}", source_slug(source), name),
                name: name.clone(),
                exe_path,
                command,
                source,
                enabled: is_enabled(&approved),
                requires_admin: hive.needs_admin(),
                kept_off: false,
                came_back: false,
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
            let display = fname.rsplit_once('.').map(|(n, _)| n.to_string()).unwrap_or(fname.clone());
            let full = entry.path().to_string_lossy().to_string();
            if is_protected(&display, &full, None) {
                continue;
            }
            let approved = registry::read(ahive, &apath, &fname).unwrap_or(RegData::Absent);
            out.push(StartupItem {
                id: format!("{}:{}", source_slug(source), fname),
                name: display,
                exe_path: Some(entry.path().to_string_lossy().to_string()),
                command: entry.path().to_string_lossy().to_string(),
                source,
                enabled: is_enabled(&approved),
                requires_admin: ahive.needs_admin(),
                kept_off: false,
                came_back: false,
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
    // Alleen wat de lijst toont mag uit: beschermde items (beveiliging, anti-cheat, Windows) staan er niet in.
    // Weer aanzetten mag altijd, bijvoorbeeld als een oudere versie zo'n item had uitgezet.
    if !enabled && !list()?.iter().any(|i| i.id == item_id) {
        return Err(AppError::NotFound(format!("startup item {name}")));
    }
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
        let mut e = JournalEntry::new("startup_apps", "Startup apps", JournalKind::Startup, name);
        e.old_value = Some(if is_enabled(&old) { "@enabled".into() } else { "@disabled".into() });
        e.new_value = Some(if enabled { "@enabled".into() } else { "@disabled".into() });
        e.revertable = true;
        e.payload = RevertPayload::Registry { hive, path: path.clone(), name: name.to_string(), old, post: None };
        lock(&state.journal).write_registry(e, &new)?;
    }
    // Onthouden wat de gebruiker uit wil hebben, zodat de bewaker het uit houdt.
    {
        let mut s = lock(&state.settings);
        s.startup_keep_off.retain(|i| i != item_id);
        if enabled {
            s.startup_came_back.retain(|i| i != item_id);
        } else {
            s.startup_keep_off.push(item_id.to_string());
        }
    }
    state.save_settings();
    list()?
        .into_iter()
        .find(|i| i.id == item_id)
        .map(|i| annotate_one(state, i))
        .ok_or_else(|| AppError::NotFound(format!("startup item {name}")))
}

/// Vult in wat nezty over een item onthoudt (uit houden, zichzelf weer aangezet).
pub fn annotate_one(state: &AppState, mut item: StartupItem) -> StartupItem {
    let s = lock(&state.settings);
    item.kept_off = s.startup_keep_off.contains(&item.id);
    item.came_back = s.startup_came_back.contains(&item.id);
    item
}

pub fn list_for(state: &AppState) -> AppResult<Vec<StartupItem>> {
    Ok(list()?.into_iter().map(|i| annotate_one(state, i)).collect())
}

/// Apps die zichzelf weer hebben aangezet terwijl de gebruiker ze uit wilde: weer uitzetten. Geeft hun namen.
pub fn enforce(state: &AppState) -> Vec<String> {
    let keep = state.settings_clone().startup_keep_off;
    if keep.is_empty() {
        return vec![];
    }
    let Ok(items) = list() else { return vec![] };
    let mut back = vec![];
    for it in items.iter().filter(|i| keep.contains(&i.id) && i.enabled) {
        if it.requires_admin && !state.elevated {
            continue;
        }
        match set_enabled(state, &it.id, false) {
            Ok(_) => {
                log::warn!("opstarten: {} zette zichzelf weer aan, weer uitgezet", it.name);
                let mut s = lock(&state.settings);
                if !s.startup_came_back.contains(&it.id) {
                    s.startup_came_back.push(it.id.clone());
                }
                back.push(it.name.clone());
            }
            Err(e) => log::warn!("opstarten: {} weer uitzetten mislukt: {e}", it.name),
        }
    }
    if !back.is_empty() {
        state.save_settings();
    }
    back
}

/// Bewaker: kort na het opstarten en daarna elke minuut kijken of een uitgezette app zichzelf weer aanzet.
/// Sommige apps (bv. SteelSeries GG) doen dat bij elke eigen start. Een melding per keer, zodat je weet waarom.
pub fn start_keeper(app: tauri::AppHandle, state: std::sync::Arc<AppState>) {
    use tauri::Emitter;
    let _ = std::thread::Builder::new().name("startup-keeper".into()).spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(15));
        if crate::license::require(&state).is_ok() && !crate::watcher::playing(&state) {
            for name in enforce(&state) {
                let _ = app.emit("notice", crate::models::Notice::new("warning", crate::models::Msg::new("startup_came_back").with("app", &name)));
            }
        }
    });
}
