//! Draaiende apps van de gebruiker. Systeem-, beveiligings- en anti-cheatprocessen worden niet
//! getoond en nooit gesloten. Ook de game zelf en nezty (en een oude nzty) worden nooit gesloten.

use crate::error::{AppError, AppResult};
use crate::icons;
use crate::journal::{JournalEntry, RevertPayload};
use crate::models::{CloseResult, ClosedProcess, JournalKind, ProcessCategory, ProcessInfo};
use crate::registry::{self, Hive, RegData};
use crate::state::{lock, AppState};
use std::collections::HashSet;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, UpdateKind};

const SYSTEM_PROCS: &[&str] = &[
    "system", "registry", "memory compression", "secure system", "smss.exe", "csrss.exe", "wininit.exe", "winlogon.exe",
    "services.exe", "lsass.exe", "lsaiso.exe", "svchost.exe", "dwm.exe", "explorer.exe", "fontdrvhost.exe", "sihost.exe",
    "taskhostw.exe", "ctfmon.exe", "runtimebroker.exe", "searchhost.exe", "searchindexer.exe", "startmenuexperiencehost.exe",
    "shellexperiencehost.exe", "textinputhost.exe", "audiodg.exe", "conhost.exe", "wmiprvse.exe", "spoolsv.exe", "dllhost.exe",
    "applicationframehost.exe", "systemsettings.exe", "sgrmbroker.exe", "wudfhost.exe", "dashost.exe", "unsecapp.exe",
    "lockapp.exe", "logonui.exe", "userinit.exe", "taskmgr.exe", "widgets.exe", "msedgewebview2.exe", "nvcontainer.exe",
    "nvdisplay.container.exe", "rtkauduservice64.exe", "gamingservices.exe", "gamingservicesnet.exe", "crossdeviceservice.exe",
    "backgroundtaskhost.exe", "wlanext.exe", "mousocoreworker.exe", "compattelrunner.exe", "powershell.exe", "pwsh.exe",
    "cmd.exe", "openconsole.exe", "windowsterminal.exe", "presentmon.exe", "amdrsserv.exe", "atieclxx.exe", "atiesrxx.exe",
];

const SECURITY_PROCS: &[&str] = &[
    "msmpeng.exe", "nissrv.exe", "mpdefendercoreservice.exe", "mpcmdrun.exe", "smartscreen.exe", "securityhealthservice.exe",
    "securityhealthsystray.exe", "securityhealthhost.exe", "avp.exe", "avpui.exe", "avastsvc.exe", "avastui.exe", "afwserv.exe",
    "avgsvc.exe", "avgui.exe", "bdagent.exe", "bdservicehost.exe", "vsserv.exe", "mcshield.exe", "mfemms.exe", "mcapexe.exe",
    "nortonsecurity.exe", "nsservice.exe", "ekrn.exe", "egui.exe", "mbamservice.exe", "mbam.exe", "mbamtray.exe",
    "csfalconservice.exe", "csfalconcontainer.exe", "sentinelagent.exe", "sentinelservicehost.exe", "cylancesvc.exe", "wrsa.exe",
    "ccsvchst.exe", "zaprivacyservice.exe", "vsmon.exe",
];
const SECURITY_PREFIXES: &[&str] = &["sophos", "malwarebytes", "fsav", "f-secure", "kaspersky", "bitdefender", "mcafee", "norton", "eset", "avast", "avg", "webroot", "crowdstrike"];

const ANTICHEAT_PROCS: &[&str] = &[
    "easyanticheat.exe", "easyanticheat_eos.exe", "easyanticheatlauncher.exe", "beservice.exe", "beclient.exe", "vgc.exe",
    "vgtray.exe", "faceitservice.exe", "faceitclient.exe", "faceit.exe", "pnkbstra.exe", "pnkbstrb.exe", "xigncode3.exe",
    "ricochet.exe",
    // Epic Online Services: overlay en diensten waar Fortnite en andere games op leunen.
    "eosoverlayrenderer-win64-shipping.exe", "eosoverlayrenderer-win32-shipping.exe", "epiconlineservices.exe",
    "epiconlineserviceshost.exe", "epiconlineservicesuserhelper.exe",
];

const BROWSERS: &[&str] = &["chrome.exe", "msedge.exe", "firefox.exe", "brave.exe", "opera.exe", "opera_gx.exe", "vivaldi.exe", "iexplore.exe", "arc.exe", "zen.exe", "librewolf.exe", "waterfox.exe", "chromium.exe"];
const LAUNCHERS: &[&str] = &["steam.exe", "steamwebhelper.exe", "epicgameslauncher.exe", "epicwebhelper.exe", "battle.net.exe", "agent.exe", "eadesktop.exe", "eabackgroundservice.exe", "origin.exe", "riotclientservices.exe", "riotclientux.exe", "riotclientuxrender.exe", "ubisoftconnect.exe", "upc.exe", "galaxyclient.exe", "galaxyclient helper.exe", "rockstarservice.exe", "playnite.desktopapp.exe", "xboxpcapp.exe", "xboxpcappft.exe"];
const CLOUD: &[&str] = &["onedrive.exe", "dropbox.exe", "googledrivefs.exe", "icloudservices.exe", "iclouddrive.exe", "megasync.exe", "nextcloud.exe", "pcloud.exe", "box.exe", "boxsync.exe"];
const UPDATERS: &[&str] = &["googleupdate.exe", "updater.exe", "adobeupdateservice.exe", "armsvc.exe", "jusched.exe", "microsoftedgeupdate.exe", "adobe desktop service.exe", "creative cloud.exe", "ccxprocess.exe", "coresync.exe", "nvidia web helper.exe", "nvbackend.exe"];
const COMMUNICATION: &[&str] = &["discord.exe", "teams.exe", "ms-teams.exe", "slack.exe", "zoom.exe", "telegram.exe", "whatsapp.exe", "signal.exe", "skype.exe", "element.exe", "guilded.exe", "teamspeak3.exe", "ts3client_win64.exe", "mumble.exe", "teamspeak.exe"];
const MEDIA: &[&str] = &["spotify.exe", "itunes.exe", "vlc.exe", "musicbee.exe", "amazon music.exe", "wallpaper32.exe", "wallpaper64.exe", "tidal.exe", "deezer.exe", "obs64.exe", "nvidia overlay.exe", "nvidia share.exe", "nvsphelper64.exe", "medal.exe", "plays.exe", "xsplit.core.exe", "streamlabs obs.exe"];
const RGB: &[&str] = &["icue.exe", "nzxt cam.exe", "armourycrate.exe", "armourycrate.service.exe", "lightingservice.exe", "rgbfusion.exe", "msi center.exe", "msicenter.exe", "mystic_light.exe", "razer synapse 3.exe", "razer synapse.exe", "rzsynapse.exe", "razercentral.exe", "logioptionsplus_agent.exe", "lghub.exe", "lghub_agent.exe", "openrgb.exe", "signalrgb.exe", "corsair.service.exe", "steelseriesgg.exe", "steelseriesengine3.exe"];

/// Is dit proces beschermd? Zo ja, welke categorie.
fn protected_category(name_lower: &str, exe_lower: Option<&str>) -> Option<ProcessCategory> {
    if SYSTEM_PROCS.contains(&name_lower) {
        return Some(ProcessCategory::System);
    }
    if SECURITY_PROCS.contains(&name_lower) || SECURITY_PREFIXES.iter().any(|p| name_lower.starts_with(p)) {
        return Some(ProcessCategory::Security);
    }
    if ANTICHEAT_PROCS.contains(&name_lower) || name_lower.contains("anticheat") || name_lower.contains("battleye") {
        return Some(ProcessCategory::Anticheat);
    }
    match exe_lower {
        None => Some(ProcessCategory::System),
        Some(exe) => {
            let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into()).to_lowercase();
            if exe.starts_with(&windir) {
                return Some(ProcessCategory::System);
            }
            if exe.contains("windows defender") || exe.contains("\\windowsapps\\microsoft.securityhealth") {
                return Some(ProcessCategory::Security);
            }
            None
        }
    }
}

fn category(name_lower: &str) -> ProcessCategory {
    if BROWSERS.contains(&name_lower) {
        ProcessCategory::Browser
    } else if LAUNCHERS.contains(&name_lower) {
        ProcessCategory::Launcher
    } else if CLOUD.contains(&name_lower) {
        ProcessCategory::Cloud
    } else if UPDATERS.contains(&name_lower) || name_lower.contains("update") {
        ProcessCategory::Updater
    } else if COMMUNICATION.contains(&name_lower) {
        ProcessCategory::Communication
    } else if MEDIA.contains(&name_lower) {
        ProcessCategory::Media
    } else if RGB.contains(&name_lower) {
        ProcessCategory::Rgb
    } else {
        ProcessCategory::Other
    }
}

/// Apps die tijdens het gamen veilig dicht kunnen: browsers, cloud-sync en updaters.
fn is_unneeded(cat: ProcessCategory) -> bool {
    matches!(cat, ProcessCategory::Browser | ProcessCategory::Cloud | ProcessCategory::Updater)
}

fn refresh_kind() -> ProcessRefreshKind {
    ProcessRefreshKind::nothing().with_cpu().with_memory().with_exe(UpdateKind::OnlyIfNotSet)
}

fn game_process_names(state: &AppState) -> HashSet<String> {
    state.all_games().iter().flat_map(|g| g.process_names.iter().map(|n| n.to_lowercase())).collect()
}

fn title_for(state: &AppState, exe: &str, fallback: &str) -> String {
    if let Some(t) = lock(&state.titles).get(exe) {
        return t.clone();
    }
    let t = icons::file_description(exe).unwrap_or_else(|| fallback.trim_end_matches(".exe").trim_end_matches(".EXE").to_string());
    lock(&state.titles).insert(exe.to_string(), t.clone());
    t
}

/// Apps van de gebruiker, samengevoegd per naam (zoals Taakbeheer).
pub fn list(state: &AppState) -> Vec<ProcessInfo> {
    let own_pid = std::process::id();
    let games = game_process_names(state);
    let raw: Vec<(u32, String, Option<String>, f32, f32)> = {
        let mut sys = lock(&state.procs);
        if sys.cpus().is_empty() {
            sys.refresh_cpu_usage();
        }
        let cores = sys.cpus().len().max(1) as f32;
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh_kind());
        sys.processes()
            .iter()
            .map(|(pid, p)| {
                (
                    pid.as_u32(),
                    p.name().to_string_lossy().to_string(),
                    p.exe().map(|e| e.to_string_lossy().to_string()),
                    p.memory() as f32 / 1_048_576.0,
                    p.cpu_usage() / cores,
                )
            })
            .collect()
    };
    let mut out: Vec<ProcessInfo> = vec![];
    for (pid, name, exe, mem_mb, cpu) in raw {
        if pid == 0 || pid == own_pid {
            continue;
        }
        let name_lower = name.to_lowercase();
        if name_lower == "nezty.exe" || name_lower == "nzty.exe" {
            continue;
        }
        let exe_lower = exe.as_deref().map(|e| e.to_lowercase());
        if protected_category(&name_lower, exe_lower.as_deref()).is_some() {
            continue;
        }
        if let Some(existing) = out.iter_mut().find(|x| x.name.eq_ignore_ascii_case(&name)) {
            existing.memory_mb += mem_mb;
            existing.cpu_pct += cpu;
            existing.instances += 1;
            continue;
        }
        let cat = if games.contains(&name_lower) { ProcessCategory::Game } else { category(&name_lower) };
        let title = exe.as_deref().map(|e| title_for(state, e, &name)).unwrap_or_else(|| name.clone());
        out.push(ProcessInfo {
            pid,
            name,
            title,
            exe_path: exe,
            memory_mb: mem_mb,
            cpu_pct: cpu,
            unneeded: is_unneeded(cat),
            category: cat,
            instances: 1,
        });
    }
    for p in out.iter_mut() {
        p.memory_mb = (p.memory_mb * 10.0).round() / 10.0;
        p.cpu_pct = (p.cpu_pct * 10.0).round() / 10.0;
    }
    out.sort_by(|a, b| b.memory_mb.partial_cmp(&a.memory_mb).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// Beëindigt alle processen met deze naam. Beschermde processen en games worden geweigerd.
pub fn kill_by_name(state: &AppState, name: &str) -> AppResult<Vec<ClosedProcess>> {
    let lower = name.to_lowercase();
    if protected_category(&lower, Some("")).is_some() || lower == "nezty.exe" || lower == "nzty.exe" {
        return Err(AppError::coded("protected_process").with("name", name));
    }
    if game_process_names(state).contains(&lower) {
        return Err(AppError::coded("game_process").with("name", name));
    }
    let own = std::process::id();
    let mut sys = lock(&state.procs);
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh_kind());
    let mut killed = vec![];
    for (pid, p) in sys.processes() {
        if pid.as_u32() == own || p.name().to_string_lossy().to_lowercase() != lower {
            continue;
        }
        let exe = p.exe().map(|e| e.to_string_lossy().to_string());
        if protected_category(&lower, exe.as_deref().map(|e| e.to_lowercase()).as_deref()).is_some() {
            return Err(AppError::coded("protected_process").with("name", name));
        }
        let mem = p.memory() as f32 / 1_048_576.0;
        if p.kill() {
            killed.push(ClosedProcess { name: name.to_string(), exe_path: exe, memory_mb: mem });
        }
    }
    Ok(killed)
}

pub fn kill_pid(state: &AppState, pid: u32) -> AppResult<Vec<ClosedProcess>> {
    let name = {
        let mut sys = lock(&state.procs);
        sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[Pid::from_u32(pid)]), true, refresh_kind());
        sys.process(Pid::from_u32(pid)).map(|p| p.name().to_string_lossy().to_string())
    };
    let name = name.ok_or_else(|| AppError::NotFound(format!("process {pid}")))?;
    kill_by_name(state, &name)
}

/// Sluit de opgegeven apps (op naam) en logt dat. Niet omkeerbaar, maar de exe-paden worden bewaard
/// zodat de app ze later opnieuw kan starten.
pub fn close_many(state: &AppState, names: &[String], session: Option<&str>, game_id: Option<&str>) -> CloseResult {
    let mut closed = vec![];
    let mut failed = vec![];
    let mut seen = HashSet::new();
    for name in names {
        if !seen.insert(name.to_lowercase()) {
            continue;
        }
        match kill_by_name(state, name) {
            Ok(list) => {
                if list.is_empty() {
                    continue;
                }
                let mem: f32 = list.iter().map(|c| c.memory_mb).sum();
                let exe = list.iter().find_map(|c| c.exe_path.clone());
                let mut e = JournalEntry::new("close_apps", "Close background apps", JournalKind::Process, name);
                e.old_value = Some("@running".into());
                e.new_value = Some("@closed".into());
                e.revertable = false;
                e.session_id = session.map(|s| s.to_string());
                e.game_id = game_id.map(|s| s.to_string());
                e.note = Some("not_revertable".into());
                let _ = lock(&state.journal).record(e);
                closed.push(ClosedProcess { name: name.clone(), exe_path: exe, memory_mb: (mem * 10.0).round() / 10.0 });
            }
            Err(_) => failed.push(name.clone()),
        }
    }
    let freed_mb = closed.iter().map(|c| c.memory_mb).sum();
    *lock(&state.last_closed) = closed.clone();
    CloseResult { closed, failed, freed_mb }
}

// ---------- Store-achtergrondapps (alleen de hoofdschakelaar) ----------

const BAA: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\BackgroundAccessApplications";

/// `true` als Store-apps op de achtergrond mogen draaien (de Windows-standaard).
pub fn store_background_allowed() -> AppResult<bool> {
    Ok(!matches!(registry::read(Hive::Hkcu, BAA, "GlobalUserDisabled")?, RegData::Dword(1)))
}

pub fn set_store_background(state: &AppState, allowed: bool) -> AppResult<()> {
    let old = registry::read(Hive::Hkcu, BAA, "GlobalUserDisabled")?;
    let new = RegData::Dword(if allowed { 0 } else { 1 });
    if old == new {
        return Ok(());
    }
    registry::write(Hive::Hkcu, BAA, "GlobalUserDisabled", &new)?;
    let mut e = JournalEntry::new("store_background_apps", "Store apps in background", JournalKind::StoreApp, &registry::full_path(Hive::Hkcu, BAA, "GlobalUserDisabled"));
    e.old_value = Some(old.display());
    e.new_value = Some(new.display());
    e.revertable = true;
    e.payload = RevertPayload::Registry { hive: Hive::Hkcu, path: BAA.into(), name: "GlobalUserDisabled".into(), old, post: None };
    lock(&state.journal).record(e)?;
    Ok(())
}
