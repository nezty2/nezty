//! Welke programma's gebruiken nu je internet? Meet een paar seconden het TCP-verkeer per programma met
//! de TCP-statistieken van Windows (vereist beheerdersrechten). nezty telt alleen bytes, het kijkt
//! nooit naar de inhoud van het verkeer.

use crate::models::{BandwidthReport, BandwidthUser};
use crate::powershell::run_cmd;
use crate::state::{lock, AppState};
use std::collections::HashMap;
use std::ffi::c_void;
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, GetPerTcp6ConnectionEStats, GetPerTcpConnectionEStats, SetPerTcp6ConnectionEStats, SetPerTcpConnectionEStats,
    MIB_TCP6ROW, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_LH, MIB_TCPROW_LH_0, MIB_TCPROW_OWNER_PID, TCP_ESTATS_DATA_ROD_v0,
    TCP_ESTATS_DATA_RW_v0, TCP_TABLE_OWNER_PID_CONNECTIONS, TcpConnectionEstatsData,
};
use windows_sys::Win32::Networking::WinSock::{IN6_ADDR, IN6_ADDR_0};

const AF_INET: u32 = 2;
const AF_INET6: u32 = 23;
const ESTABLISHED: u32 = 5;

enum Row {
    V4(MIB_TCPROW_LH),
    V6(MIB_TCP6ROW),
}

/// Leest een tabel met verbindingen in een goed uitgelijnde buffer.
fn table(af: u32) -> Vec<u64> {
    let mut size: u32 = 0;
    unsafe {
        GetExtendedTcpTable(std::ptr::null_mut(), &mut size, 0, af, TCP_TABLE_OWNER_PID_CONNECTIONS, 0);
        for _ in 0..3 {
            let mut buf = vec![0u64; (size as usize / 8) + 64];
            let mut len = (buf.len() * 8) as u32;
            let r = GetExtendedTcpTable(buf.as_mut_ptr() as *mut c_void, &mut len, 0, af, TCP_TABLE_OWNER_PID_CONNECTIONS, 0);
            if r == 0 {
                return buf;
            }
            size = len;
        }
    }
    vec![]
}

fn connections() -> Vec<(Row, u32)> {
    let mut out = vec![];
    unsafe {
        let v4 = table(AF_INET);
        if !v4.is_empty() {
            let count = *(v4.as_ptr() as *const u32) as usize;
            let rows = std::slice::from_raw_parts((v4.as_ptr() as *const u8).add(4) as *const MIB_TCPROW_OWNER_PID, count);
            for r in rows.iter().filter(|r| r.dwState == ESTABLISHED) {
                let row = MIB_TCPROW_LH {
                    Anonymous: MIB_TCPROW_LH_0 { dwState: r.dwState },
                    dwLocalAddr: r.dwLocalAddr,
                    dwLocalPort: r.dwLocalPort,
                    dwRemoteAddr: r.dwRemoteAddr,
                    dwRemotePort: r.dwRemotePort,
                };
                out.push((Row::V4(row), r.dwOwningPid));
            }
        }
        let v6 = table(AF_INET6);
        if !v6.is_empty() {
            let count = *(v6.as_ptr() as *const u32) as usize;
            let rows = std::slice::from_raw_parts((v6.as_ptr() as *const u8).add(4) as *const MIB_TCP6ROW_OWNER_PID, count);
            for r in rows.iter().filter(|r| r.dwState == ESTABLISHED) {
                let row = MIB_TCP6ROW {
                    State: r.dwState as i32,
                    LocalAddr: IN6_ADDR { u: IN6_ADDR_0 { Byte: r.ucLocalAddr } },
                    dwLocalScopeId: r.dwLocalScopeId,
                    dwLocalPort: r.dwLocalPort,
                    RemoteAddr: IN6_ADDR { u: IN6_ADDR_0 { Byte: r.ucRemoteAddr } },
                    dwRemoteScopeId: r.dwRemoteScopeId,
                    dwRemotePort: r.dwRemotePort,
                };
                out.push((Row::V6(row), r.dwOwningPid));
            }
        }
    }
    out
}

fn enable(row: &Row, on: bool) -> bool {
    let rw = TCP_ESTATS_DATA_RW_v0 { EnableCollection: on };
    let size = std::mem::size_of::<TCP_ESTATS_DATA_RW_v0>() as u32;
    let r = unsafe {
        match row {
            Row::V4(r4) => SetPerTcpConnectionEStats(r4, TcpConnectionEstatsData, &rw as *const _ as *const u8, 0, size, 0),
            Row::V6(r6) => SetPerTcp6ConnectionEStats(r6, TcpConnectionEstatsData, &rw as *const _ as *const u8, 0, size, 0),
        }
    };
    r == 0
}

/// Ontvangen en verzonden bytes van één verbinding (sinds het aanzetten van de statistiek).
fn bytes(row: &Row) -> Option<(u64, u64)> {
    let mut rod: TCP_ESTATS_DATA_ROD_v0 = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<TCP_ESTATS_DATA_ROD_v0>() as u32;
    let null = std::ptr::null_mut();
    let r = unsafe {
        match row {
            Row::V4(r4) => GetPerTcpConnectionEStats(r4, TcpConnectionEstatsData, null, 0, 0, null, 0, 0, &mut rod as *mut _ as *mut u8, 0, size),
            Row::V6(r6) => GetPerTcp6ConnectionEStats(r6, TcpConnectionEstatsData, null, 0, 0, null, 0, 0, &mut rod as *mut _ as *mut u8, 0, size),
        }
    };
    (r == 0).then_some((rod.DataBytesIn, rod.DataBytesOut))
}

/// Welke Windows-diensten draaien in welk proces (svchost.exe host er vaak meerdere).
fn services_by_pid() -> HashMap<u32, String> {
    let mut map = HashMap::new();
    if let Ok((true, out, _)) = run_cmd("tasklist", &["/svc", "/fo", "csv", "/nh"]) {
        for line in out.lines() {
            let cols: Vec<&str> = line.split("\",\"").map(|c| c.trim_matches('"')).collect();
            if cols.len() >= 3 {
                if let Ok(pid) = cols[1].parse::<u32>() {
                    map.insert(pid, cols[2].to_lowercase());
                }
            }
        }
    }
    map
}

fn label(name: &str, services: Option<&String>) -> Option<&'static str> {
    if name == "svchost.exe" {
        let s = services?;
        if s.contains("dosvc") {
            return Some("delivery_optimization");
        }
        if s.contains("wuauserv") || s.contains("bits") || s.contains("usosvc") {
            return Some("windows_update");
        }
        return None;
    }
    Some(match name {
        "onedrive.exe" => "onedrive",
        "steam.exe" | "steamservice.exe" | "steamwebhelper.exe" => "steam",
        "epicgameslauncher.exe" | "epicwebhelper.exe" => "epic",
        "battle.net.exe" | "agent.exe" => "battlenet",
        "eadesktop.exe" | "eabackgroundservice.exe" => "ea",
        "upc.exe" | "ubisoftconnect.exe" => "ubisoft",
        "chrome.exe" | "msedge.exe" | "firefox.exe" | "brave.exe" | "opera.exe" => "browser",
        "discord.exe" => "discord",
        "spotify.exe" => "spotify",
        _ => return None,
    })
}

pub fn measure(state: &AppState, seconds: f32) -> BandwidthReport {
    let seconds = seconds.clamp(1.0, 10.0);
    let rows = connections();
    let found = rows.len() as u32;
    let enabled: Vec<(Row, u32)> = rows.into_iter().filter(|(row, _)| enable(row, true)).collect();
    let start: Vec<Option<(u64, u64)>> = enabled.iter().map(|(row, _)| bytes(row)).collect();
    let services = services_by_pid();
    let t0 = Instant::now();
    let left = Duration::from_secs_f32(seconds).saturating_sub(t0.elapsed());
    std::thread::sleep(left);
    let elapsed = t0.elapsed().as_secs_f32().max(0.5);

    let mut per_pid: HashMap<u32, (u64, u64)> = HashMap::new();
    for ((row, pid), before) in enabled.iter().zip(start) {
        if let (Some((in0, out0)), Some((in1, out1))) = (before, bytes(row)) {
            let e = per_pid.entry(*pid).or_default();
            e.0 += in1.saturating_sub(in0);
            e.1 += out1.saturating_sub(out0);
        }
        enable(row, false);
    }

    // Per programmanaam optellen (Chrome heeft bijvoorbeeld veel processen).
    let mut sys = lock(&state.procs);
    let pids: Vec<Pid> = per_pid.keys().map(|p| Pid::from_u32(*p)).collect();
    sys.refresh_processes_specifics(ProcessesToUpdate::Some(&pids), true, ProcessRefreshKind::nothing());
    let mut by_name: HashMap<(String, Option<&'static str>), (u64, u64)> = HashMap::new();
    let mut total = (0u64, 0u64);
    for (pid, (down, up)) in per_pid {
        total.0 += down;
        total.1 += up;
        let name = sys.process(Pid::from_u32(pid)).map(|p| p.name().to_string_lossy().to_string()).unwrap_or_else(|| format!("PID {pid}"));
        let lower = name.to_lowercase();
        let key = (name, label(&lower, services.get(&pid)));
        let e = by_name.entry(key).or_default();
        e.0 += down;
        e.1 += up;
    }
    drop(sys);
    let kbps = |b: u64| (b as f32 * 8.0 / 1000.0) / elapsed;
    let mut users: Vec<BandwidthUser> = by_name
        .into_iter()
        .map(|((name, label), (down, up))| BandwidthUser { name, label: label.map(|l| l.to_string()), down_kbps: kbps(down), up_kbps: kbps(up) })
        // Alles onder 200 kbit/s is verwaarloosbaar voor je ping.
        .filter(|u| u.down_kbps + u.up_kbps >= 200.0)
        .collect();
    users.sort_by(|a, b| (b.down_kbps + b.up_kbps).partial_cmp(&(a.down_kbps + a.up_kbps)).unwrap_or(std::cmp::Ordering::Equal));
    users.truncate(10);
    BandwidthReport {
        seconds: elapsed,
        found,
        connections: enabled.len() as u32,
        total_down_kbps: kbps(total.0),
        total_up_kbps: kbps(total.1),
        users,
        elevated: state.elevated,
    }
}
