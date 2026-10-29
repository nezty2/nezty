//! Tweaks per game: volledig-scherm-optimalisaties, GPU-voorkeur en procesprioriteit.

use crate::error::{AppError, AppResult};
use crate::journal::{JournalEntry, RevertPayload};
use crate::models::{Game, JournalKind, Msg, TweakState};
use crate::registry::{self, Hive, RegData};
use crate::state::{lock, AppState};
use crate::tweaks::{get, revert_via_journal, Status};
use crate::winapi;
use sysinfo::ProcessesToUpdate;

const LAYERS: &str = "Software\\Microsoft\\Windows NT\\CurrentVersion\\AppCompatFlags\\Layers";
const GPU_PREFS: &str = "Software\\Microsoft\\DirectX\\UserGpuPreferences";
const FSO_FLAG: &str = "DISABLEDXMAXIMIZEDWINDOWEDMODE";

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

// ---------- Volledig-scherm-optimalisaties ----------

fn layer_tokens(data: &RegData) -> Vec<String> {
    data.as_str().map(|s| s.split_whitespace().map(|t| t.to_string()).collect()).unwrap_or_default()
}

pub fn fullscreen_status(game: Option<&Game>) -> AppResult<Status> {
    let (_, exe) = match exe_of(game) {
        Ok(v) => v,
        Err(e) => return Ok(unavailable(e)),
    };
    let cur = registry::read(Hive::Hkcu, LAYERS, &exe)?;
    let on = layer_tokens(&cur).iter().any(|t| t == FSO_FLAG);
    Ok(Status::simple(if on { TweakState::On } else { TweakState::Off }))
}

pub fn fullscreen_apply(state: &AppState, game: Option<&Game>, session: Option<&str>) -> AppResult<()> {
    let (g, exe) = exe_of(game)?;
    let old = registry::read(Hive::Hkcu, LAYERS, &exe)?;
    let mut tokens = layer_tokens(&old);
    if tokens.iter().any(|t| t == FSO_FLAG) {
        return Ok(());
    }
    if tokens.first().map(|t| t != "~").unwrap_or(true) {
        tokens.insert(0, "~".to_string());
    }
    tokens.push(FSO_FLAG.to_string());
    let new = RegData::Str(tokens.join(" "));
    registry::write(Hive::Hkcu, LAYERS, &exe, &new)?;
    record_reg(state, "fullscreen_opt_off", g, LAYERS, &exe, old, &new, session, None)
}

pub fn fullscreen_disable(state: &AppState, game: Option<&Game>) -> AppResult<()> {
    let (g, exe) = exe_of(game)?;
    if revert_via_journal(state, "fullscreen_opt_off", Some(&g.id))? && fullscreen_status(game)?.state == TweakState::Off {
        return Ok(());
    }
    let old = registry::read(Hive::Hkcu, LAYERS, &exe)?;
    let tokens: Vec<String> = layer_tokens(&old).into_iter().filter(|t| t != FSO_FLAG).collect();
    if tokens.len() == layer_tokens(&old).len() {
        return Ok(());
    }
    let new = if tokens.iter().all(|t| t == "~") { RegData::Absent } else { RegData::Str(tokens.join(" ")) };
    registry::write(Hive::Hkcu, LAYERS, &exe, &new)?;
    record_reg(state, "fullscreen_opt_off", g, LAYERS, &exe, old, &new, None, Some("manual_off"))
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

// ---------- Procesprioriteit ----------

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

pub fn priority_status(state: &AppState, game: Option<&Game>) -> AppResult<Status> {
    let g = match game {
        Some(g) => g,
        None => return Ok(Status::with_detail(TweakState::Unavailable, Msg::new("game_required"))),
    };
    let pids = running_pids(state, g);
    if pids.is_empty() {
        return Ok(Status::with_detail(TweakState::Off, Msg::new("game_not_running")));
    }
    let high = pids.iter().any(|p| winapi::get_priority_class(*p).map(|c| c == winapi::PRIORITY_HIGH).unwrap_or(false));
    Ok(Status::with_detail(if high { TweakState::On } else { TweakState::Off }, Msg::new("procs_running").with("count", pids.len())))
}

/// Zet prioriteit Hoog op alle draaiende processen van de game. Geeft het aantal aangepaste processen.
pub fn priority_apply(state: &AppState, game: Option<&Game>, session: Option<&str>) -> AppResult<usize> {
    let g = game.ok_or_else(|| AppError::coded("game_required"))?;
    let d = get("high_priority")?;
    let mut changed = 0;
    for pid in running_pids(state, g) {
        if lock(&state.priority_applied).contains(&pid) {
            continue;
        }
        let old = match winapi::set_priority_class(pid, winapi::PRIORITY_HIGH) {
            Ok(old) => old,
            Err(e) => {
                log::warn!("prioriteit pid {pid}: {e}");
                continue;
            }
        };
        lock(&state.priority_applied).insert(pid);
        changed += 1;
        let mut e = JournalEntry::new(d.id, d.name, JournalKind::Priority, &format!("{} (pid {pid})", g.name));
        e.old_value = Some(winapi::priority_label(old));
        e.new_value = Some(winapi::priority_label(winapi::PRIORITY_HIGH));
        e.revertable = true;
        e.session_id = session.map(|s| s.to_string());
        e.game_id = Some(g.id.clone());
        e.payload = RevertPayload::Priority { pid, old_class: old };
        lock(&state.journal).record(e)?;
    }
    Ok(changed)
}

pub fn priority_disable(state: &AppState, game: Option<&Game>) -> AppResult<()> {
    let g = game.ok_or_else(|| AppError::coded("game_required"))?;
    revert_via_journal(state, "high_priority", Some(&g.id))?;
    for pid in running_pids(state, g) {
        lock(&state.priority_applied).remove(&pid);
    }
    Ok(())
}
