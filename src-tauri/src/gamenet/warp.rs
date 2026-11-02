//! Snellere route via Cloudflare WARP: hetzelfde idee als de routes van ExitLag (verkeer over een ander netwerk
//! sturen), maar over het netwerk van Cloudflare in plaats van eigen servers. nezty installeert niets en accepteert
//! geen voorwaarden namens jou. Is WARP er, dan meet nezty de ping naar je server zonder en met WARP, en houdt WARP
//! alleen aan als het echt sneller is. Dat staat in het logboek, dus "Alles terugzetten" zet het weer uit.

use crate::error::{AppError, AppResult};
use crate::journal::{JournalEntry, RevertPayload};
use crate::models::{JournalKind, Msg, PingResult, WarpInfo, WarpTest};
use crate::ping::ping_series;
use crate::powershell::run_cmd;
use crate::state::{lock, AppState};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const TWEAK: &str = "warp_route";

fn cli() -> Option<PathBuf> {
    let base = std::env::var_os("ProgramFiles").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\Program Files"));
    let p = base.join("Cloudflare").join("Cloudflare WARP").join("warp-cli.exe");
    p.is_file().then_some(p)
}

fn run(args: &[&str]) -> AppResult<String> {
    let path = cli().ok_or_else(|| AppError::coded("warp_missing"))?;
    let (_, out, err) = run_cmd(&path.to_string_lossy(), args)?;
    Ok(format!("{out}\n{err}"))
}

fn parse_status(text: &str) -> &'static str {
    let t = text.to_lowercase();
    if t.contains("terms of service") || t.contains("accept-tos") {
        "needs_tos"
    } else if t.contains("registration missing") || t.contains("missing registration") {
        "unregistered"
    } else if t.contains("disconnected") {
        "disconnected"
    } else if t.contains("connecting") {
        "connecting"
    } else if t.contains("connected") {
        "connected"
    } else {
        "unknown"
    }
}

fn status() -> &'static str {
    match run(&["status"]) {
        Ok(t) => parse_status(&t),
        Err(_) => "not_installed",
    }
}

fn wait_for(wanted: &str, timeout: Duration) -> bool {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if status() == wanted {
            return true;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    false
}

/// Wijzigingen van nezty die niet meer gelden (WARP is in de eigen app uitgezet) als teruggezet markeren.
fn forget_ours(state: &AppState) {
    let mut j = lock(&state.journal);
    let ids = j.active_ids(TWEAK, None);
    j.mark_reverted(&ids);
}

pub fn info(state: &AppState) -> WarpInfo {
    if cli().is_none() {
        return WarpInfo { installed: false, status: "not_installed".into(), connected_by_nezty: false };
    }
    let st = status();
    let mut ours = lock(&state.journal).has_active(TWEAK, None);
    if ours && st == "disconnected" {
        forget_ours(state);
        ours = false;
    }
    WarpInfo { installed: true, status: st.into(), connected_by_nezty: ours }
}

pub fn disconnect() -> AppResult<()> {
    run(&["disconnect"]).map(|_| ())
}

/// Terugzetten via het logboek. Is WARP inmiddels verwijderd, dan staat er ook niets meer aan.
pub fn revert() -> AppResult<()> {
    if cli().is_none() {
        return Ok(());
    }
    disconnect()
}

/// Is WARP echt beter? Minstens 2 ms of 5% sneller, en niet meer verlies.
fn better(direct: &PingResult, warp: &PingResult) -> bool {
    match (direct.avg_ms, warp.avg_ms) {
        (Some(d), Some(w)) => w <= d - (d * 0.05).max(2.0) && warp.loss_pct <= direct.loss_pct + 5.0,
        (None, Some(_)) => true,
        _ => false,
    }
}

/// Meet `host` zonder en met WARP. Sneller met WARP: WARP blijft aan (in het logboek). Anders weer uit.
/// Niet midden in een potje doen: het wisselen van route laat je game opnieuw verbinden.
pub fn test(state: &AppState, host: &str) -> AppResult<WarpTest> {
    if cli().is_none() {
        return Err(AppError::coded("warp_missing"));
    }
    let ours = lock(&state.journal).has_active(TWEAK, None);
    match status() {
        "needs_tos" | "unregistered" => return Err(AppError::coded("warp_setup")),
        "connected" | "connecting" if !ours => return Err(AppError::coded("warp_already_on")),
        "disconnected" => {}
        _ => {
            disconnect()?;
            wait_for("disconnected", Duration::from_secs(10));
        }
    }
    let direct = ping_series(host, 12, 1500, 100);
    run(&["connect"])?;
    if !wait_for("connected", Duration::from_secs(20)) {
        let _ = disconnect();
        forget_ours(state);
        return Ok(WarpTest { host: host.to_string(), direct, warp: None, decision: "error".into(), detail: Some(Msg::new("warp_no_connect")) });
    }
    // Even laten wennen: de eerste pakketten na het verbinden zijn niet representatief.
    std::thread::sleep(Duration::from_millis(800));
    let warp = ping_series(host, 12, 1500, 100);

    if warp.received == 0 && direct.received > 0 {
        // Via WARP komt de ping niet terug (sommige servers): dan is er niets eerlijks te vergelijken.
        let _ = disconnect();
        forget_ours(state);
        return Ok(WarpTest { host: host.to_string(), direct, warp: Some(warp), decision: "error".into(), detail: Some(Msg::new("warp_no_reply")) });
    }
    if better(&direct, &warp) {
        if !ours {
            let mut e = JournalEntry::new(TWEAK, "Cloudflare WARP route", JournalKind::Setting, "Cloudflare WARP");
            e.old_value = Some("off".into());
            e.new_value = Some("on".into());
            e.revertable = true;
            e.payload = RevertPayload::Warp;
            lock(&state.journal).record(e)?;
        }
        Ok(WarpTest { host: host.to_string(), direct, warp: Some(warp), decision: "warp".into(), detail: None })
    } else {
        let _ = disconnect();
        forget_ours(state);
        Ok(WarpTest { host: host.to_string(), direct, warp: Some(warp), decision: "direct".into(), detail: None })
    }
}

/// Zet WARP uit als nezty het heeft aangezet (via het logboek).
pub fn stop_ours(state: &AppState) -> AppResult<()> {
    crate::tweaks::revert_via_journal(state, TWEAK, None)?;
    Ok(())
}
