//! De route naar een server, zoals tracert en pathping: elke tussenstap met eigen ping, verlies en schommeling.
//! Via ICMP met een oplopende TTL (IcmpSendEcho, geen beheerdersrechten nodig). Daarna de conclusie waar het
//! begint. Eerlijk: een router die zelf traag of soms niet op ping antwoordt maar het verkeer gewoon doorstuurt,
//! is geen probleem. Alleen verlies dat tot aan de server blijft, is echt verlies.

use crate::models::{RouteHop, RouteReport, RouteSummary};
use crate::ping::{summarize, Target};
use std::collections::HashMap;
use std::ffi::c_void;
use std::net::Ipv4Addr;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::NetworkManagement::IpHelper::{IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho, ICMP_ECHO_REPLY, IP_OPTION_INFORMATION, IP_SUCCESS, IP_TTL_EXPIRED_TRANSIT};

const MAX_HOPS: u8 = 30;

/// Eén antwoord: van welk adres, hoe snel, en of het de server zelf was.
struct Probe {
    from: Option<Ipv4Addr>,
    rtt: Option<f32>,
    reached: bool,
}

const NO_REPLY: Probe = Probe { from: None, rtt: None, reached: false };

fn probe(dest: Ipv4Addr, ttl: u8, timeout_ms: u32) -> Probe {
    unsafe {
        let handle = IcmpCreateFile();
        if handle == INVALID_HANDLE_VALUE || handle.is_null() {
            return NO_REPLY;
        }
        let payload = [0x6eu8; 32];
        // Ruim en goed uitgelijnd: een "TTL verlopen"-antwoord bevat ook de kop van het oorspronkelijke pakket.
        let mut buffer = vec![0u64; 64];
        let opts = IP_OPTION_INFORMATION { Ttl: ttl, Tos: 0, Flags: 0, OptionsSize: 0, OptionsData: std::ptr::null_mut() };
        let started = Instant::now();
        let n = IcmpSendEcho(
            handle,
            u32::from_ne_bytes(dest.octets()),
            payload.as_ptr() as *const c_void,
            payload.len() as u16,
            &opts,
            buffer.as_mut_ptr() as *mut c_void,
            (buffer.len() * 8) as u32,
            timeout_ms,
        );
        let elapsed = started.elapsed().as_secs_f32() * 1000.0;
        IcmpCloseHandle(handle);
        if n == 0 {
            return NO_REPLY;
        }
        let reply = &*(buffer.as_ptr() as *const ICMP_ECHO_REPLY);
        let from = Ipv4Addr::from(reply.Address.to_ne_bytes());
        // De API geeft hele milliseconden; bij 0 ms de eigen, fijnere meting.
        let rtt = if reply.RoundTripTime == 0 { (elapsed * 100.0).round() / 100.0 } else { reply.RoundTripTime as f32 };
        match reply.Status {
            IP_SUCCESS => Probe { from: Some(from), rtt: Some(rtt), reached: true },
            IP_TTL_EXPIRED_TRANSIT => Probe { from: Some(from), rtt: Some(rtt), reached: false },
            _ => NO_REPLY,
        }
    }
}

/// Thuisnetwerk of het interne netwerk van de provider (ook CGNAT, 100.64.0.0/10).
fn is_private(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    ip.is_private() || ip.is_loopback() || ip.is_link_local() || (o[0] == 100 && (64..128).contains(&o[1]))
}

/// Naam van een adres (reverse DNS). Vaak staat daar de provider en de stad in.
fn host_name(ip: Ipv4Addr) -> Option<String> {
    use windows_sys::Win32::Networking::WinSock::{GetNameInfoW, WSAStartup, AF_INET, IN_ADDR, IN_ADDR_0, NI_NAMEREQD, SOCKADDR, SOCKADDR_IN, WSADATA};
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| unsafe {
        let mut data: WSADATA = std::mem::zeroed();
        WSAStartup(0x202, &mut data);
    });
    let sa = SOCKADDR_IN { sin_family: AF_INET, sin_port: 0, sin_addr: IN_ADDR { S_un: IN_ADDR_0 { S_addr: u32::from_ne_bytes(ip.octets()) } }, sin_zero: [0; 8] };
    let mut name = [0u16; 256];
    let rc = unsafe {
        GetNameInfoW(&sa as *const SOCKADDR_IN as *const SOCKADDR, std::mem::size_of::<SOCKADDR_IN>() as i32, name.as_mut_ptr(), name.len() as u32, std::ptr::null_mut(), 0, NI_NAMEREQD as i32)
    };
    if rc != 0 {
        return None;
    }
    let len = name.iter().position(|c| *c == 0).unwrap_or(0);
    let s = String::from_utf16_lossy(&name[..len]);
    (!s.is_empty() && s != ip.to_string()).then_some(s)
}

/// Namen van alle adressen tegelijk, maar nooit langer dan `budget`: sommige adressen hebben geen naam en
/// laten de opzoeking lang hangen.
fn host_names(ips: &[Ipv4Addr], budget: Duration) -> HashMap<Ipv4Addr, String> {
    let (tx, rx) = mpsc::channel();
    for ip in ips {
        let (tx, ip) = (tx.clone(), *ip);
        std::thread::spawn(move || {
            if let Some(n) = host_name(ip) {
                let _ = tx.send((ip, n));
            }
        });
    }
    drop(tx);
    let deadline = Instant::now() + budget;
    let mut out = HashMap::new();
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok((ip, n)) => {
                out.insert(ip, n);
            }
            Err(_) => break,
        }
    }
    out
}

/// Meet de hele route naar `host` ("1.2.3.4", "naam" of "naam:443"). `probes` metingen per stap.
pub fn route(host: &str, probes: u32) -> RouteReport {
    let started = Instant::now();
    let probes = probes.clamp(3, 20);
    let empty_summary = || RouteSummary { kind: "unknown".into(), hop: None, step_from: None, step_to: None, step_ms: None };
    let Some(dest) = Target::resolve(host).map(|t| t.ip()) else {
        return RouteReport { target: host.to_string(), ip: None, reached: false, hops: vec![], summary: empty_summary(), measure_host: None, duration_ms: 0 };
    };

    // 1. De weg vinden: alle stappen tegelijk, één meting per stap.
    let (tx, rx) = mpsc::channel();
    for ttl in 1..=MAX_HOPS {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send((ttl, probe(dest, ttl, 1500)));
        });
    }
    drop(tx);
    let mut first: Vec<Probe> = (0..=MAX_HOPS).map(|_| NO_REPLY).collect();
    for (ttl, p) in rx {
        first[ttl as usize] = p;
    }
    let reached_at = (1..=MAX_HOPS).find(|t| first[*t as usize].reached);
    let last = reached_at.or_else(|| (1..=MAX_HOPS).rev().find(|t| first[*t as usize].from.is_some())).unwrap_or(0);

    // 2. Elke stap een paar keer meten, tegelijk en met tussenpozen: routers beperken hun ping-antwoorden.
    // Een router die de eerste keer zweeg, zwijgt bijna altijd: die nog maar twee keer proberen.
    let (tx, rx) = mpsc::channel();
    for ttl in 1..=last {
        let tx = tx.clone();
        let count = if first[ttl as usize].from.is_some() { probes } else { 2 };
        std::thread::spawn(move || {
            let mut out = Vec::with_capacity(count as usize);
            for i in 0..count {
                out.push(probe(dest, ttl, 1000));
                if i + 1 < count {
                    std::thread::sleep(Duration::from_millis(250));
                }
            }
            let _ = tx.send((ttl, out));
        });
    }
    drop(tx);
    let mut per_hop: Vec<Vec<Probe>> = (0..=last).map(|_| Vec::new()).collect();
    for (ttl, list) in rx {
        per_hop[ttl as usize] = list;
    }

    let mut hops: Vec<RouteHop> = Vec::new();
    for ttl in 1..=last {
        let list = &per_hop[ttl as usize];
        // Het adres dat het vaakst antwoordde (bij meerdere paden kan het wisselen).
        let mut count: HashMap<Ipv4Addr, u32> = HashMap::new();
        for p in list.iter().chain(std::iter::once(&first[ttl as usize])) {
            if let Some(a) = p.from {
                *count.entry(a).or_default() += 1;
            }
        }
        let ip = count.into_iter().max_by_key(|(_, n)| *n).map(|(a, _)| a);
        let rtts: Vec<Option<f32>> = list.iter().map(|p| p.rtt).collect();
        let s = summarize(&rtts);
        hops.push(RouteHop {
            ttl,
            ip: ip.map(|a| a.to_string()),
            host_name: None,
            private: ip.is_some_and(is_private),
            sent: s.sent,
            received: s.received,
            avg_ms: s.avg_ms,
            min_ms: s.min_ms,
            max_ms: s.max_ms,
            jitter_ms: s.jitter_ms,
            loss_pct: s.loss_pct,
            verdict: String::new(),
            is_target: reached_at == Some(ttl),
        });
    }
    // De server antwoordt niet op ping: dan toch als laatste stap tonen, zonder meting.
    if reached_at.is_none() && last > 0 {
        hops.push(RouteHop {
            ttl: last + 1,
            ip: Some(dest.to_string()),
            host_name: None,
            private: is_private(dest),
            sent: 0,
            received: 0,
            avg_ms: None,
            min_ms: None,
            max_ms: None,
            jitter_ms: None,
            loss_pct: 100.0,
            verdict: String::new(),
            is_target: true,
        });
    }

    let ips: Vec<Ipv4Addr> = hops.iter().filter_map(|h| h.ip.as_deref()?.parse().ok()).collect();
    let names = host_names(&ips, Duration::from_millis(2500));
    for h in hops.iter_mut() {
        h.host_name = h.ip.as_deref().and_then(|ip| ip.parse::<Ipv4Addr>().ok()).and_then(|ip| names.get(&ip).cloned());
    }
    let summary = analyze(&mut hops);
    let measure_host = if reached_at.is_some() {
        Some(dest.to_string())
    } else {
        hops.iter().rev().find(|h| h.received > 0 && h.verdict != "icmp_limited").and_then(|h| h.ip.clone())
    };
    RouteReport { target: host.to_string(), ip: Some(dest.to_string()), reached: reached_at.is_some(), hops, summary, measure_host, duration_ms: started.elapsed().as_millis() as u32 }
}

/// Oordeel per stap en de conclusie voor de hele route.
fn analyze(hops: &mut [RouteHop]) -> RouteSummary {
    let answering: Vec<usize> = hops.iter().enumerate().filter(|(_, h)| h.received > 0).map(|(i, _)| i).collect();
    for h in hops.iter_mut() {
        h.verdict = if h.received == 0 { "silent" } else { "ok" }.into();
    }

    // Verlies telt alleen als het bij alle latere stappen blijft (minstens de helft ervan). Anders beantwoordt
    // alleen die router ping met lage voorrang, en loopt het echte verkeer er gewoon doorheen.
    let mut loss_start: Option<usize> = None;
    for (k, &i) in answering.iter().enumerate() {
        let loss = hops[i].loss_pct;
        if loss < 10.0 {
            continue;
        }
        let later = &answering[k + 1..];
        let real = if later.is_empty() { hops[i].is_target } else { later.iter().all(|&j| hops[j].loss_pct >= loss * 0.5) };
        if real {
            hops[i].verdict = "loss".into();
            loss_start.get_or_insert(i);
        } else {
            hops[i].verdict = "icmp_limited".into();
        }
    }
    // Een stap die veel trager is dan de stap erna: die router antwoordt zelf traag, het verkeer niet.
    for k in 0..answering.len().saturating_sub(1) {
        let (i, j) = (answering[k], answering[k + 1]);
        if let (Some(a), Some(b)) = (hops[i].min_ms, hops[j].min_ms) {
            if a > b + 20.0 && hops[i].verdict == "ok" {
                hops[i].verdict = "icmp_limited".into();
            }
        }
    }

    // De grootste stap in ping, tussen stappen die eerlijk antwoorden (minimum: minder ruis dan het gemiddelde).
    let honest: Vec<usize> = answering.iter().copied().filter(|&i| hops[i].verdict != "icmp_limited").collect();
    let mut step: Option<(u8, u8, f32)> = None;
    let mut prev: Option<(u8, f32)> = Some((0, 0.0));
    for &i in &honest {
        let Some(ms) = hops[i].min_ms else { continue };
        if let Some((pt, pm)) = prev {
            let d = ms - pm;
            if d >= 5.0 && step.map_or(true, |(_, _, s)| d > s) {
                step = Some((pt, hops[i].ttl, (d * 10.0).round() / 10.0));
            }
        }
        prev = Some((hops[i].ttl, ms));
    }

    let first_hop = answering.first().map(|&i| &hops[i]);
    let kind = if let Some(i) = loss_start {
        let pos = answering.iter().position(|&a| a == i).unwrap_or(0);
        if hops[i].ttl == 1 || (hops[i].private && hops[i].ttl <= 2) {
            "home"
        } else if pos + 3 >= answering.len() || hops[i].is_target {
            "remote"
        } else {
            "provider"
        }
    } else if first_hop.is_some_and(|h| h.ttl == 1 && (h.avg_ms.unwrap_or(0.0) > 15.0 || h.jitter_ms.unwrap_or(0.0) > 10.0)) {
        // De eerste stap is je eigen router: is die al traag of onrustig, dan zit het thuis (wifi, router vol).
        "home"
    } else if answering.is_empty() {
        "unknown"
    } else {
        "clean"
    };
    RouteSummary {
        kind: kind.into(),
        hop: loss_start.map(|i| hops[i].ttl).or_else(|| (kind == "home").then_some(1)),
        step_from: step.map(|s| s.0),
        step_to: step.map(|s| s.1),
        step_ms: step.map(|s| s.2),
    }
}
