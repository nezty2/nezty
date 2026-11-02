//! Multi-Internet: meerdere internetverbindingen (kabel, wifi, een tweede netwerkkaart of een telefoon via USB).
//! nezty meet elke verbinding apart, met een ping vanaf het eigen adres van die verbinding, en kan er één voorrang
//! geven via de interfacemetric van Windows (lager = eerst). Staat in het logboek en gaat exact terug.
//!
//! Eerlijk: wisselen midden in een potje laat je game opnieuw verbinden. Naadloos wisselen kan alleen met een eigen
//! server in het midden (zo doet ExitLag dat); een app op je pc kan dat niet. De automatische stand wisselt daarom
//! alleen als de huidige verbinding echt geen internet meer geeft. Windows zelf merkt dat pas als de kabel eruit is.

use crate::error::{AppError, AppResult};
use crate::journal::{JournalEntry, RevertPayload};
use crate::models::{AdapterKind, JournalKind, Msg, MultiNetStatus, NetConnection, Notice};
use crate::state::{lock, AppState};
use std::collections::HashMap;
use std::ffi::c_void;
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetBestInterface, GetIpInterfaceEntry, IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho2Ex, InitializeIpInterfaceEntry, SetIpInterfaceEntry, ICMP_ECHO_REPLY, MIB_IPINTERFACE_ROW,
};
use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6};

const TWEAK: &str = "preferred_connection";
/// Lager dan wat Windows zelf kiest (kabel 25 of meer, wifi 35 of meer).
const PREFERRED_METRIC: u32 = 5;
const PROBE_HOST: Ipv4Addr = Ipv4Addr::new(1, 1, 1, 1);

/// Een verbinding met internet (status Up, IPv4 en een router).
#[derive(Clone)]
struct Conn {
    name: String,
    description: String,
    kind: AdapterKind,
    luid: u64,
    ipv4: Ipv4Addr,
    gateway: String,
    link_speed_mbps: Option<f64>,
    metric: u32,
    automatic: bool,
    active: bool,
}

fn get_row(luid: u64, family: u16) -> AppResult<MIB_IPINTERFACE_ROW> {
    unsafe {
        let mut row: MIB_IPINTERFACE_ROW = std::mem::zeroed();
        InitializeIpInterfaceEntry(&mut row);
        row.Family = family;
        row.InterfaceLuid.Value = luid;
        let rc = GetIpInterfaceEntry(&mut row);
        if rc != 0 {
            return Err(AppError::msg(format!("GetIpInterfaceEntry: {rc}")));
        }
        Ok(row)
    }
}

/// Zet de metric van één verbinding (ook gebruikt om terug te zetten).
pub fn set_metric(luid: u64, family: u16, automatic: bool, metric: u32) -> AppResult<()> {
    let mut row = get_row(luid, family)?;
    row.UseAutomaticMetric = automatic;
    if !automatic {
        row.Metric = metric;
    }
    // Voorschrift van SetIpInterfaceEntry voor IPv4.
    if family == AF_INET {
        row.SitePrefixLength = 0;
    }
    match unsafe { SetIpInterfaceEntry(&mut row) } {
        0 => Ok(()),
        5 => Err(AppError::NeedsAdmin("SetIpInterfaceEntry".into())),
        rc => Err(AppError::msg(format!("SetIpInterfaceEntry: {rc}"))),
    }
}

/// Terugzetten via het logboek. Bestaat de verbinding niet meer (bv. een losgekoppelde telefoon), dan is er niets
/// terug te zetten.
pub fn revert_metric(luid: u64, family: u16, automatic: bool, metric: u32) -> AppResult<()> {
    if get_row(luid, family).is_err() {
        return Ok(());
    }
    set_metric(luid, family, automatic, metric)
}

fn connections() -> Vec<Conn> {
    // Via welke verbinding gaat het internetverkeer nu? Dat beslist Windows zelf (route + metric).
    let mut best_index = 0u32;
    let best = unsafe { GetBestInterface(u32::from_ne_bytes(PROBE_HOST.octets()), &mut best_index) } == 0;
    crate::netinfo::scan()
        .into_iter()
        .filter(|s| s.adapter.status.eq_ignore_ascii_case("Up"))
        .filter_map(|s| {
            let ipv4: Ipv4Addr = s.adapter.ipv4.as_deref()?.parse().ok()?;
            let gateway = s.adapter.gateway.clone()?;
            let row = get_row(s.luid, AF_INET).ok()?;
            Some(Conn {
                name: s.adapter.name,
                description: s.adapter.description,
                kind: s.adapter.kind,
                luid: s.luid,
                ipv4,
                gateway,
                link_speed_mbps: s.adapter.link_speed_mbps,
                metric: row.Metric,
                automatic: row.UseAutomaticMetric,
                active: best && s.if_index == best_index,
            })
        })
        .collect()
}

/// Eén ping naar 1.1.1.1 vanaf het adres van deze verbinding (dan gaat hij ook echt via deze verbinding).
fn probe_once(c: &Conn, timeout_ms: u32) -> Option<f32> {
    unsafe {
        let handle = IcmpCreateFile();
        if handle == INVALID_HANDLE_VALUE || handle.is_null() {
            return None;
        }
        let payload = [0x6du8; 32];
        let mut buffer = vec![0u64; 16];
        let started = Instant::now();
        let n = IcmpSendEcho2Ex(
            handle,
            std::ptr::null_mut(),
            None,
            std::ptr::null(),
            u32::from_ne_bytes(c.ipv4.octets()),
            u32::from_ne_bytes(PROBE_HOST.octets()),
            payload.as_ptr() as *const c_void,
            payload.len() as u16,
            std::ptr::null(),
            buffer.as_mut_ptr() as *mut c_void,
            (buffer.len() * 8) as u32,
            timeout_ms,
        );
        let elapsed = started.elapsed().as_secs_f32() * 1000.0;
        IcmpCloseHandle(handle);
        if n == 0 {
            return None;
        }
        let reply = &*(buffer.as_ptr() as *const ICMP_ECHO_REPLY);
        if reply.Status != 0 {
            return None;
        }
        Some(if reply.RoundTripTime == 0 { (elapsed * 100.0).round() / 100.0 } else { reply.RoundTripTime as f32 })
    }
}

/// De verbinding die nezty voorrang gaf (uit het logboek), als die er is.
fn preferred_luid(state: &AppState) -> Option<u64> {
    let j = lock(&state.journal);
    j.entries.iter().rev().filter(|e| e.tweak_id == TWEAK && !e.reverted && e.revertable).find_map(|e| match e.payload {
        RevertPayload::InterfaceMetric { luid, .. } => Some(luid),
        _ => None,
    })
}

/// Alle verbindingen, met (als `measure`) een korte meting per verbinding, tegelijk.
pub fn status(state: &AppState, measure: bool) -> MultiNetStatus {
    let conns = connections();
    let ours = preferred_luid(state);
    let pings: Vec<(Option<f32>, Option<f32>)> = if measure {
        let handles: Vec<_> = conns
            .iter()
            .cloned()
            .map(|c| {
                std::thread::spawn(move || {
                    let samples: Vec<Option<f32>> = (0..4).map(|_| probe_once(&c, 800)).collect();
                    let s = crate::ping::summarize(&samples);
                    (s.avg_ms, Some(s.loss_pct))
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap_or((None, None))).collect()
    } else {
        vec![(None, None); conns.len()]
    };
    let connections = conns
        .into_iter()
        .zip(pings)
        .map(|(c, (ping_ms, loss_pct))| NetConnection {
            name: c.name,
            description: c.description,
            kind: c.kind,
            luid: c.luid.to_string(),
            ipv4: Some(c.ipv4.to_string()),
            gateway: Some(c.gateway),
            link_speed_mbps: c.link_speed_mbps,
            metric: c.metric,
            automatic_metric: c.automatic,
            active: c.active,
            preferred_by_nezty: ours == Some(c.luid),
            ping_ms,
            loss_pct,
        })
        .collect();
    MultiNetStatus { connections, auto: state.settings_clone().multinet_auto, elevated: state.elevated }
}

/// Geeft één verbinding voorrang: haar metric gaat naar 5 (IPv4 en, als die er is, IPv6). Een eerdere keuze van
/// nezty gaat eerst terug, zodat er altijd maar één voorkeur is.
pub fn set_preferred(state: &AppState, luid: u64) -> AppResult<()> {
    if !state.elevated {
        return Err(AppError::NeedsAdmin("SetIpInterfaceEntry".into()));
    }
    let conn = connections().into_iter().find(|c| c.luid == luid).ok_or_else(|| AppError::coded("connection_gone"))?;
    crate::tweaks::revert_via_journal(state, TWEAK, None)?;
    let mut done = false;
    for family in [AF_INET, AF_INET6] {
        let Ok(row) = get_row(luid, family) else { continue };
        let (old_auto, old_metric) = (row.UseAutomaticMetric, row.Metric);
        match set_metric(luid, family, false, PREFERRED_METRIC) {
            Ok(()) => {}
            Err(e) if family == AF_INET => return Err(e),
            Err(_) => continue,
        }
        let ip = if family == AF_INET { "IPv4" } else { "IPv6" };
        let mut e = JournalEntry::new(TWEAK, "Preferred internet connection", JournalKind::Adapter, &format!("{} ({ip})", conn.name));
        e.old_value = Some(if old_auto { format!("auto ({old_metric})") } else { old_metric.to_string() });
        e.new_value = Some(PREFERRED_METRIC.to_string());
        e.revertable = true;
        e.payload = RevertPayload::InterfaceMetric { luid, family, automatic: old_auto, metric: old_metric };
        lock(&state.journal).record(e)?;
        done = true;
    }
    if !done {
        return Err(AppError::msg("no interface changed"));
    }
    Ok(())
}

/// Voorkeur weer aan Windows laten (exact terug via het logboek).
pub fn clear_preferred(state: &AppState) -> AppResult<()> {
    crate::tweaks::revert_via_journal(state, TWEAK, None)?;
    Ok(())
}

/// Automatisch wisselen: elke seconde elke verbinding meten. Geeft de verbinding die Windows nu gebruikt vier keer
/// achter elkaar geen antwoord, en een andere drie keer wel, dan krijgt die andere voorrang. Hooguit eens per minuut.
pub fn start(app: AppHandle, state: Arc<AppState>) {
    let _ = std::thread::Builder::new().name("multinet".into()).spawn(move || {
        let mut fails: HashMap<u64, u32> = HashMap::new();
        let mut oks: HashMap<u64, u32> = HashMap::new();
        let mut last_switch: Option<Instant> = None;
        loop {
            let enabled = state.settings_clone().multinet_auto && state.elevated && crate::license::require(&state).is_ok();
            let conns = if enabled { connections() } else { vec![] };
            if conns.len() < 2 {
                fails.clear();
                oks.clear();
                std::thread::sleep(Duration::from_secs(3));
                continue;
            }
            let t0 = Instant::now();
            for c in &conns {
                if probe_once(c, 800).is_some() {
                    *oks.entry(c.luid).or_default() += 1;
                    fails.insert(c.luid, 0);
                } else {
                    *fails.entry(c.luid).or_default() += 1;
                    oks.insert(c.luid, 0);
                }
            }
            let cooled = last_switch.map_or(true, |t| t.elapsed() > Duration::from_secs(60));
            if let Some(active) = conns.iter().find(|c| c.active).filter(|c| cooled && fails.get(&c.luid).copied().unwrap_or(0) >= 4) {
                if let Some(other) = conns.iter().find(|c| c.luid != active.luid && oks.get(&c.luid).copied().unwrap_or(0) >= 3) {
                    match set_preferred(&state, other.luid) {
                        Ok(()) => {
                            last_switch = Some(Instant::now());
                            log::warn!("multi-internet: {} geeft geen internet, nu via {}", active.name, other.name);
                            let _ = app.emit("notice", Notice::new("warning", Msg::new("multinet_switched").with("from", &active.name).with("to", &other.name)));
                            let _ = app.emit("multinet-changed", ());
                        }
                        Err(e) => log::warn!("multi-internet wisselen mislukt: {e}"),
                    }
                }
            }
            if let Some(rest) = Duration::from_secs(1).checked_sub(t0.elapsed()) {
                std::thread::sleep(rest);
            }
        }
    });
}
