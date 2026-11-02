//! Voorrang voor gameverkeer via de QoS-engine van Windows (zoals de Traffic Shaper van ExitLag, maar zonder eigen
//! driver): een QoS-regel per game markeert alle uitgaande pakketten van dat programma met DSCP 46 (Expedited
//! Forwarding). Op wifi met WMM krijgen die pakketten voorrang in de wachtrij (DSCP 32-47 = "video", zie Microsoft
//! Learn, Manage QoS Policy). Op een kabel helpt het alleen als de router DSCP gebruikt; dat zegt de app er eerlijk bij.
//!
//! Op een pc zonder domein past Windows QoS-regels pas toe met "Do not use NLA" = 1 onder Tcpip\QoS (Microsoft
//! documenteert dat, met een herstart). Dat is de losse tweak qos_all_networks; game_qos zet hem zo nodig mee aan.

use crate::error::{AppError, AppResult};
use crate::journal::{JournalEntry, RevertPayload};
use crate::models::{Game, JournalKind, Msg, TweakState};
use crate::powershell::{ps_quote, run_ps};
use crate::registry::Hive;
use crate::state::{lock, AppState};
use crate::tweaks::regtweak::{self, RegSpec};
use crate::tweaks::{get, revert_via_journal, Status};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Expedited Forwarding: de waarde voor spraak en andere tijdkritische pakketten.
pub const DSCP: u8 = 46;
const PREFIX: &str = "nezty ";

fn nla_specs() -> Vec<RegSpec> {
    vec![RegSpec::string(Hive::Hklm, "SYSTEM\\CurrentControlSet\\Services\\Tcpip\\QoS", "Do not use NLA", "1", "0").off_absent()]
}

pub fn nla_status() -> AppResult<Status> {
    Ok(Status::simple(regtweak::state_of(&nla_specs())?))
}

pub fn nla_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    regtweak::apply(state, get("qos_all_networks")?, &nla_specs(), session, None, None)
}

pub fn nla_disable(state: &AppState) -> AppResult<()> {
    regtweak::disable(state, get("qos_all_networks")?, &nla_specs(), None, None)
}

/// Een QoS-regel van nezty: naam, programma en DSCP-waarde.
#[derive(Clone)]
struct Policy {
    name: String,
    app: String,
    dscp: i64,
}

/// De regels lezen kost een PowerShell-aanroep: een paar seconden bewaren.
static CACHE: Mutex<Option<(Instant, Vec<Policy>)>> = Mutex::new(None);

fn policies() -> AppResult<Vec<Policy>> {
    if let Some((t, list)) = lock(&CACHE).as_ref() {
        if t.elapsed() < Duration::from_secs(3) {
            return Ok(list.clone());
        }
    }
    let out = run_ps(&format!(
        "@(Get-NetQosPolicy -PolicyStore localhost -ErrorAction SilentlyContinue | Where-Object {{ $_.Name -like {} }} | Select-Object Name, AppPathNameMatchCondition, DSCPAction) | ConvertTo-Json -Compress",
        ps_quote(&format!("{PREFIX}*"))
    ))?;
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap_or(serde_json::Value::Null);
    // Eén regel geeft een object, meerdere een lijst.
    let items: Vec<serde_json::Value> = match v {
        serde_json::Value::Array(a) => a,
        serde_json::Value::Object(_) => vec![v],
        _ => vec![],
    };
    let list: Vec<Policy> = items
        .iter()
        .filter_map(|p| {
            Some(Policy { name: p["Name"].as_str()?.to_string(), app: p["AppPathNameMatchCondition"].as_str().unwrap_or_default().to_string(), dscp: p["DSCPAction"].as_i64().unwrap_or(-1) })
        })
        .collect();
    *lock(&CACHE) = Some((Instant::now(), list.clone()));
    Ok(list)
}

fn forget_cache() {
    *lock(&CACHE) = None;
}

/// De programma's van een game (zoals Windows ze start): daar komt een regel voor.
fn exes(game: &Game) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for n in &game.process_names {
        let n = n.trim();
        if n.to_lowercase().ends_with(".exe") && !out.iter().any(|x| x.eq_ignore_ascii_case(n)) {
            out.push(n.to_string());
        }
    }
    out
}

/// Naam van de regel, alleen met tekens die Windows overal accepteert.
fn policy_name(game: &Game, exe: &str) -> String {
    let clean = |s: &str| s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '_' { c } else { '-' }).collect::<String>();
    format!("{PREFIX}{} {}", clean(&game.id), clean(exe))
}

pub fn game_status(state: &AppState, game: Option<&Game>) -> AppResult<Status> {
    let Some(g) = game else { return Ok(Status::with_detail(TweakState::Unavailable, Msg::new("game_required"))) };
    let exes = exes(g);
    if exes.is_empty() {
        return Ok(Status::with_detail(TweakState::Unavailable, Msg::new("no_exe").with("game", &g.name)));
    }
    let list = policies()?;
    let on = exes.iter().all(|e| list.iter().any(|p| p.name == policy_name(g, e) && p.app.eq_ignore_ascii_case(e) && p.dscp == DSCP as i64));
    let mut st = Status::simple(if on { TweakState::On } else { TweakState::Off });
    // De regel staat er, maar Windows past hem pas toe na de herstart voor "op elk netwerk".
    if on && crate::restart::pending(state, "qos_all_networks") {
        st.detail = Some(Msg::new("qos_after_restart"));
    }
    Ok(st)
}

pub fn game_apply(state: &AppState, game: Option<&Game>, session: Option<&str>) -> AppResult<()> {
    let g = game.ok_or_else(|| AppError::coded("game_required"))?;
    let exes = exes(g);
    if exes.is_empty() {
        return Err(AppError::coded("no_exe").with("game", &g.name));
    }
    if nla_status()?.state != TweakState::On {
        crate::tweaks::apply(state, "qos_all_networks", None, session, None)?;
    }
    let d = get("game_qos")?;
    let existing = policies()?;
    for exe in &exes {
        let name = policy_name(g, exe);
        if existing.iter().any(|p| p.name == name) {
            continue;
        }
        run_ps(&format!(
            "New-NetQosPolicy -Name {} -AppPathNameMatchCondition {} -IPProtocolMatchCondition Both -NetworkProfile All -DSCPAction {DSCP} -PolicyStore localhost | Out-Null",
            ps_quote(&name),
            ps_quote(exe)
        ))?;
        forget_cache();
        let mut e = JournalEntry::new(d.id, d.name, JournalKind::Setting, &format!("QoS: {exe}"));
        e.old_value = Some("-".into());
        e.new_value = Some(format!("DSCP {DSCP}"));
        e.revertable = true;
        e.session_id = session.map(|s| s.to_string());
        e.game_id = Some(g.id.clone());
        e.payload = RevertPayload::QosPolicy { name };
        lock(&state.journal).record(e)?;
    }
    Ok(())
}

pub fn game_disable(state: &AppState, game: Option<&Game>) -> AppResult<()> {
    let g = game.ok_or_else(|| AppError::coded("game_required"))?;
    revert_via_journal(state, "game_qos", Some(&g.id))?;
    // Regels van nezty die niet (meer) in het logboek staan, ook weghalen.
    let left = policies()?;
    for exe in exes(g) {
        let name = policy_name(g, &exe);
        if left.iter().any(|p| p.name == name) {
            remove_policy(&name)?;
        }
    }
    Ok(())
}

/// Haalt een regel weg. Bestaat hij al niet meer, dan is dat ook goed.
pub fn remove_policy(name: &str) -> AppResult<()> {
    run_ps(&format!("Remove-NetQosPolicy -Name {} -PolicyStore localhost -Confirm:$false -ErrorAction SilentlyContinue", ps_quote(name)))?;
    forget_cache();
    Ok(())
}
