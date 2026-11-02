//! Latency-metingen: ICMP-ping via de Windows IP Helper API (geen adminrechten nodig) en een
//! TCP-handshake-meting voor hosts die ICMP blokkeren (bv. cloud-regio's). Een host met `:poort`
//! wordt via TCP gemeten.

use crate::models::PingResult;
use std::ffi::c_void;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs};
use std::ptr::null;
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::NetworkManagement::IpHelper::{IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho, ICMP_ECHO_REPLY};

/// Zet een hostnaam of IP om naar een IPv4-adres.
pub fn resolve_v4(host: &str) -> Option<Ipv4Addr> {
    if let Ok(ip) = host.parse::<Ipv4Addr>() {
        return Some(ip);
    }
    let addrs = (host, 0u16).to_socket_addrs().ok()?;
    for a in addrs {
        if let IpAddr::V4(v4) = a.ip() {
            return Some(v4);
        }
    }
    None
}

/// Eén ICMP echo-request. Geeft de rondreistijd in ms, of None bij time-out/fout.
pub fn ping_once(ip: Ipv4Addr, timeout_ms: u32) -> Option<f32> {
    unsafe {
        let handle = IcmpCreateFile();
        if handle == INVALID_HANDLE_VALUE || handle.is_null() {
            return None;
        }
        let payload = [0x47u8; 32];
        let reply_size = std::mem::size_of::<ICMP_ECHO_REPLY>() + payload.len() + 8;
        let mut buffer = vec![0u8; reply_size];
        let started = Instant::now();
        let replies = IcmpSendEcho(
            handle,
            u32::from_ne_bytes(ip.octets()),
            payload.as_ptr() as *const c_void,
            payload.len() as u16,
            null(),
            buffer.as_mut_ptr() as *mut c_void,
            reply_size as u32,
            timeout_ms,
        );
        IcmpCloseHandle(handle);
        if replies == 0 {
            return None;
        }
        let reply = &*(buffer.as_ptr() as *const ICMP_ECHO_REPLY);
        if reply.Status != 0 {
            return None;
        }
        // De API geeft hele milliseconden; bij 0 ms gebruiken we onze eigen fijnere meting.
        let rtt = reply.RoundTripTime as f32;
        if rtt == 0.0 {
            let elapsed = started.elapsed().as_secs_f32() * 1000.0;
            return Some((elapsed * 100.0).round() / 100.0);
        }
        Some(rtt)
    }
}

/// Eén TCP-handshake (SYN/SYN-ACK). Geeft de verbindingstijd in ms.
pub fn tcp_ping_once(addr: SocketAddr, timeout: Duration) -> Option<f32> {
    let started = Instant::now();
    let stream = TcpStream::connect_timeout(&addr, timeout).ok()?;
    let elapsed = started.elapsed().as_secs_f32() * 1000.0;
    drop(stream);
    Some((elapsed * 10.0).round() / 10.0)
}

/// Splitst "host:poort" in (host, poort) als er een poort is.
fn split_port(host: &str) -> (String, Option<u16>) {
    if let Some((h, p)) = host.rsplit_once(':') {
        if let Ok(port) = p.parse::<u16>() {
            if !h.is_empty() && !h.contains(':') {
                return (h.to_string(), Some(port));
            }
        }
    }
    (host.to_string(), None)
}

/// Een meetdoel: ICMP naar een IP-adres, of een TCP-handshake naar IP:poort.
#[derive(Clone, Copy, Debug)]
pub enum Target {
    Icmp(Ipv4Addr),
    Tcp(SocketAddr),
}

impl Target {
    /// "1.2.3.4", "host.nl" (ICMP) of "host.nl:443" (TCP). DNS wordt hier één keer opgezocht.
    pub fn resolve(host: &str) -> Option<Target> {
        let (hostname, port) = split_port(host.trim());
        let ip = resolve_v4(&hostname)?;
        Some(match port {
            Some(p) => Target::Tcp(SocketAddr::new(IpAddr::V4(ip), p)),
            None => Target::Icmp(ip),
        })
    }

    pub fn ip(&self) -> Ipv4Addr {
        match self {
            Target::Icmp(ip) => *ip,
            Target::Tcp(SocketAddr::V4(a)) => *a.ip(),
            Target::Tcp(_) => Ipv4Addr::UNSPECIFIED,
        }
    }

    /// Eén meting in ms, of None bij time-out.
    pub fn once(&self, timeout_ms: u32) -> Option<f32> {
        match self {
            Target::Icmp(ip) => ping_once(*ip, timeout_ms),
            Target::Tcp(addr) => tcp_ping_once(*addr, Duration::from_millis(timeout_ms as u64)),
        }
    }
}

/// Statistiek van een reeks metingen. Jitter: het gemiddelde verschil tussen opeenvolgende antwoorden.
pub struct Summary {
    pub sent: u32,
    pub received: u32,
    pub min_ms: Option<f32>,
    pub avg_ms: Option<f32>,
    pub max_ms: Option<f32>,
    pub jitter_ms: Option<f32>,
    pub loss_pct: f32,
}

pub fn summarize<'a>(samples: impl IntoIterator<Item = &'a Option<f32>>) -> Summary {
    let all: Vec<Option<f32>> = samples.into_iter().copied().collect();
    let ok: Vec<f32> = all.iter().filter_map(|s| *s).collect();
    let sent = all.len() as u32;
    let received = ok.len() as u32;
    let round = |v: f32| (v * 10.0).round() / 10.0;
    let (min_ms, max_ms, avg_ms, jitter_ms) = if ok.is_empty() {
        (None, None, None, None)
    } else {
        let min = ok.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = ok.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let avg = ok.iter().sum::<f32>() / ok.len() as f32;
        let jitter = if ok.len() > 1 {
            ok.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f32>() / (ok.len() - 1) as f32
        } else {
            0.0
        };
        (Some(round(min)), Some(round(max)), Some(round(avg)), Some(round(jitter)))
    };
    Summary {
        sent,
        received,
        min_ms,
        avg_ms,
        max_ms,
        jitter_ms,
        loss_pct: if sent == 0 { 100.0 } else { ((sent - received) as f32 / sent as f32) * 100.0 },
    }
}

/// Meerdere metingen met statistiek (min, gemiddelde, max, jitter, verlies).
pub fn ping_series(host: &str, count: u32, timeout_ms: u32, gap_ms: u64) -> PingResult {
    let target = Target::resolve(host);
    let mut samples: Vec<Option<f32>> = Vec::new();
    if let Some(t) = target {
        for i in 0..count.max(1) {
            samples.push(t.once(timeout_ms));
            if i + 1 < count {
                std::thread::sleep(Duration::from_millis(gap_ms));
            }
        }
    } else {
        samples.push(None);
    }
    let s = summarize(&samples);
    PingResult {
        host: host.to_string(),
        ip: target.map(|t| t.ip().to_string()),
        sent: s.sent,
        received: s.received,
        min_ms: s.min_ms,
        avg_ms: s.avg_ms,
        max_ms: s.max_ms,
        jitter_ms: s.jitter_ms,
        loss_pct: s.loss_pct,
        samples,
    }
}

/// Eén snelle meting voor het dashboard.
pub fn quick(host: &str, timeout_ms: u32) -> Option<f32> {
    Target::resolve(host)?.once(timeout_ms)
}
