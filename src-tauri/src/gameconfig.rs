//! Instellingen in configbestanden van andere games (naast Fortnite). Alleen sleutels die zeker
//! kloppen: V-Sync in Minecraft Java (options.txt) en Counter-Strike 2 (cs2_video.txt). Wijzigen kan
//! alleen als de game dicht is, anders schrijft de game zijn bestand bij het afsluiten terug.

use crate::error::{AppError, AppResult};
use crate::journal::{JournalEntry, RevertPayload};
use crate::models::{GameConfigCheck, JournalKind};
use crate::processes;
use crate::state::{lock, AppState};
use std::path::{Path, PathBuf};

struct Spec {
    game_id: &'static str,
    check: &'static str,
    game: &'static str,
    processes: &'static [&'static str],
    /// "colon" (sleutel:waarde) of "vdf" ("sleutel"  "waarde").
    format: &'static str,
    key: &'static str,
    recommended: &'static str,
}

const SPECS: &[Spec] = &[
    Spec { game_id: "minecraft:java", check: "vsync", game: "Minecraft", processes: &["javaw.exe"], format: "colon", key: "enableVsync", recommended: "false" },
    Spec { game_id: "steam:cs2", check: "vsync", game: "Counter-Strike 2", processes: &["cs2.exe"], format: "vdf", key: "setting.mat_vsync", recommended: "0" },
];

fn files(spec: &Spec) -> Vec<PathBuf> {
    match spec.game_id {
        "minecraft:java" => {
            let p = PathBuf::from(std::env::var("APPDATA").unwrap_or_default()).join(".minecraft").join("options.txt");
            if p.is_file() { vec![p] } else { vec![] }
        }
        "steam:cs2" => {
            let Some(steam) = crate::registry::read(crate::registry::Hive::Hkcu, r"Software\Valve\Steam", "SteamPath").ok().and_then(|d| d.as_str().map(|s| s.to_string())) else {
                return vec![];
            };
            let Ok(users) = std::fs::read_dir(PathBuf::from(steam).join("userdata")) else { return vec![] };
            users.flatten().map(|u| u.path().join(r"730\local\cfg\cs2_video.txt")).filter(|p| p.is_file()).collect()
        }
        _ => vec![],
    }
}

/// Waarde van een sleutel in de tekst.
pub fn read_key(text: &str, format: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let t = line.trim();
        if format == "colon" {
            if let Some(v) = t.strip_prefix(key).and_then(|r| r.strip_prefix(':')) {
                return Some(v.trim().to_string());
            }
        } else {
            let quoted = format!("\"{key}\"");
            if let Some(rest) = t.strip_prefix(&quoted) {
                let rest = rest.trim();
                return rest.strip_prefix('"').and_then(|r| r.split('"').next()).map(|v| v.to_string());
            }
        }
    }
    None
}

/// Zet een bestaande sleutel op een nieuwe waarde. De opmaak van de regel blijft gelijk.
pub fn write_key(text: &str, format: &str, key: &str, value: &str) -> Option<String> {
    let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut changed = false;
    let lines: Vec<String> = text
        .split(nl)
        .map(|line| {
            let t = line.trim_start();
            let indent = &line[..line.len() - t.len()];
            if changed {
                return line.to_string();
            }
            if format == "colon" && t.strip_prefix(key).is_some_and(|r| r.starts_with(':')) {
                changed = true;
                return format!("{indent}{key}:{value}");
            }
            let quoted = format!("\"{key}\"");
            if format == "vdf" && t.starts_with(&quoted) {
                let rest = &t[quoted.len()..];
                let gap_len = rest.len() - rest.trim_start().len();
                changed = true;
                return format!("{indent}{quoted}{}\"{value}\"", &rest[..gap_len]);
            }
            line.to_string()
        })
        .collect();
    changed.then(|| lines.join(nl))
}

pub fn list(game_id: &str) -> Vec<GameConfigCheck> {
    let mut out = vec![];
    for spec in SPECS.iter().filter(|s| s.game_id == game_id) {
        for file in files(spec) {
            let current = std::fs::read_to_string(&file).ok().and_then(|t| read_key(&t, spec.format, spec.key));
            out.push(GameConfigCheck {
                game_id: spec.game_id.into(),
                id: spec.check.into(),
                file: file.to_string_lossy().to_string(),
                ok: current.as_deref() == Some(spec.recommended),
                current,
                recommended: spec.recommended.into(),
            });
        }
    }
    out
}

/// Zet alles wat nog niet goed staat voor deze game. Elke sleutel komt apart in het logboek.
pub fn apply(state: &AppState, game_id: &str) -> AppResult<Vec<GameConfigCheck>> {
    let running = processes::running_names(state);
    for spec in SPECS.iter().filter(|s| s.game_id == game_id) {
        if spec.processes.iter().any(|p| running.contains(*p)) {
            return Err(AppError::coded("close_app_first").with("app", spec.game));
        }
        for file in files(spec) {
            let text = std::fs::read_to_string(&file)?;
            let Some(old) = read_key(&text, spec.format, spec.key) else { continue };
            if old == spec.recommended {
                continue;
            }
            let Some(new_text) = write_key(&text, spec.format, spec.key, spec.recommended) else { continue };
            std::fs::write(&file, new_text)?;
            let mut e = JournalEntry::new("game_config", &format!("{}: {}", spec.game, spec.key), JournalKind::File, &file.to_string_lossy());
            e.old_value = Some(old.clone());
            e.new_value = Some(spec.recommended.into());
            e.revertable = true;
            e.game_id = Some(spec.game_id.into());
            e.payload = RevertPayload::TextKey { path: file.to_string_lossy().to_string(), format: spec.format.into(), key: spec.key.into(), old };
            lock(&state.journal).record(e)?;
        }
    }
    Ok(list(game_id))
}

/// Terugzetten van één sleutel (gebruikt door het logboek).
pub fn revert_key(path: &Path, format: &str, key: &str, old: &str) -> AppResult<()> {
    let text = std::fs::read_to_string(path)?;
    if let Some(t) = write_key(&text, format, key, old) {
        std::fs::write(path, t)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_roundtrip() {
        let mc = "version:3955\r\nenableVsync:true\r\nmaxFps:240\r\n";
        assert_eq!(read_key(mc, "colon", "enableVsync").as_deref(), Some("true"));
        let changed = write_key(mc, "colon", "enableVsync", "false").unwrap();
        assert_eq!(read_key(&changed, "colon", "enableVsync").as_deref(), Some("false"));
        assert_eq!(write_key(&changed, "colon", "enableVsync", "true").unwrap(), mc);
        let vdf = "\"video.cfg\"\n{\n\t\"setting.mat_vsync\"\t\t\"1\"\n}\n";
        assert_eq!(read_key(vdf, "vdf", "setting.mat_vsync").as_deref(), Some("1"));
        let v2 = write_key(vdf, "vdf", "setting.mat_vsync", "0").unwrap();
        assert_eq!(v2, "\"video.cfg\"\n{\n\t\"setting.mat_vsync\"\t\t\"0\"\n}\n");
    }
}
