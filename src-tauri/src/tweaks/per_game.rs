//! Tweak per game: GPU-voorkeur (de game op de snelle videokaart). Plus welke processen van een game draaien.

use crate::error::{AppError, AppResult};
use crate::journal::{JournalEntry, RevertPayload};
use crate::models::{Game, JournalKind, Msg, TweakState};
use crate::registry::{self, Hive, RegData};
use crate::state::{lock, AppState};
use crate::tweaks::{get, revert_via_journal, Status};
use sysinfo::ProcessesToUpdate;

const GPU_PREFS: &str = "Software\\Microsoft\\DirectX\\UserGpuPreferences";

fn exe_of(game: Option<&Game>) -> AppResult<(&Game, String)> {
    let g = game.ok_or_else(|| AppError::coded("game_required"))?;
    let exe = g.exe_path.clone().ok_or_else(|| AppError::coded("no_exe").with("game", &g.name))?;
    Ok((g, exe))
}

fn unavailable(e: AppError) -> Status {
    match e {
        AppError::Coded { key, params } => Status::with_detail(TweakState::Unavailable, Msg { key, params }),
        other => Status::with_detail(TweakState::Unavailable, Msg::new("status_error").with("detail", other.to_string())),
    }
}

#[allow(clippy::too_many_arguments)]
fn record_reg(state: &AppState, tweak_id: &str, game: &Game, path: &str, name: &str, old: RegData, new: &RegData, session: Option<&str>, note: Option<&str>) -> AppResult<()> {
    let d = get(tweak_id)?;
    let mut e = JournalEntry::new(d.id, d.name, JournalKind::Registry, &registry::full_path(Hive::Hkcu, path, name));
    e.old_value = Some(old.display());
    e.new_value = Some(new.display());
    e.revertable = true;
    e.session_id = session.map(|s| s.to_string());
    e.game_id = Some(game.id.clone());
    e.note = note.map(|s| s.to_string());
    e.payload = RevertPayload::Registry { hive: Hive::Hkcu, path: path.to_string(), name: name.to_string(), old, post: None };
    lock(&state.journal).record(e)
}

// ---------- GPU-voorkeur ----------

fn gpu_pairs(data: &RegData) -> Vec<(String, String)> {
    data.as_str()
        .map(|s| {
            s.split(';')
                .filter(|p| !p.trim().is_empty())
                .filter_map(|p| p.split_once('=').map(|(k, v)| (k.trim().to_string(), v.trim().to_string())))
                .collect()
        })
        .unwrap_or_default()
}

fn gpu_join(pairs: &[(String, String)]) -> String {
    pairs.iter().map(|(k, v)| format!("{k}={v};")).collect::<Vec<_>>().join("")
}

pub fn gpu_pref_is_high(exe: &str) -> bool {
    registry::read(Hive::Hkcu, GPU_PREFS, exe)
        .map(|cur| gpu_pairs(&cur).iter().any(|(k, v)| k == "GpuPreference" && v == "2"))
        .unwrap_or(false)
}

pub fn gpu_pref_status(game: Option<&Game>) -> AppResult<Status> {
    let (_, exe) = match exe_of(game) {
        Ok(v) => v,
        Err(e) => return Ok(unavailable(e)),
    };
    Ok(Status::simple(if gpu_pref_is_high(&exe) { TweakState::On } else { TweakState::Off }))
}

pub fn gpu_pref_apply(state: &AppState, game: Option<&Game>, session: Option<&str>) -> AppResult<()> {
    let (g, exe) = exe_of(game)?;
    let old = registry::read(Hive::Hkcu, GPU_PREFS, &exe)?;
    let mut pairs = gpu_pairs(&old);
    if pairs.iter().any(|(k, v)| k == "GpuPreference" && v == "2") {
        return Ok(());
    }
    pairs.retain(|(k, _)| k != "GpuPreference");
    pairs.push(("GpuPreference".to_string(), "2".to_string()));
    let new = RegData::Str(gpu_join(&pairs));
    registry::write(Hive::Hkcu, GPU_PREFS, &exe, &new)?;
    record_reg(state, "gpu_high_perf", g, GPU_PREFS, &exe, old, &new, session, None)
}

pub fn gpu_pref_disable(state: &AppState, game: Option<&Game>) -> AppResult<()> {
    let (g, exe) = exe_of(game)?;
    if revert_via_journal(state, "gpu_high_perf", Some(&g.id))? && !gpu_pref_is_high(&exe) {
        return Ok(());
    }
    let old = registry::read(Hive::Hkcu, GPU_PREFS, &exe)?;
    let mut pairs = gpu_pairs(&old);
    let before = pairs.len();
    pairs.retain(|(k, _)| k != "GpuPreference");
    if pairs.len() == before {
        return Ok(());
    }
    let new = if pairs.is_empty() { RegData::Absent } else { RegData::Str(gpu_join(&pairs)) };
    registry::write(Hive::Hkcu, GPU_PREFS, &exe, &new)?;
    record_reg(state, "gpu_high_perf", g, GPU_PREFS, &exe, old, &new, None, Some("manual_off"))
}

// ---------- Draaiende processen ----------

/// Pids van draaiende processen van een game.
pub fn running_pids(state: &AppState, game: &Game) -> Vec<u32> {
    let names: Vec<String> = game.process_names.iter().map(|n| n.to_lowercase()).collect();
    let mut sys = lock(&state.procs);
    sys.refresh_processes(ProcessesToUpdate::All, true);
    sys.processes()
        .iter()
        .filter(|(_, p)| names.contains(&p.name().to_string_lossy().to_lowercase()))
        .map(|(pid, _)| pid.as_u32())
        .collect()
}
