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

/// Meerdere metingen met statistiek (min, gemiddelde, max, jitter, verlies).
pub fn ping_series(host: &str, count: u32, timeout_ms: u32, gap_ms: u64) -> PingResult {
    let (hostname, port) = split_port(host.trim());
    let ip = resolve_v4(&hostname);
    let mut samples: Vec<Option<f32>> = Vec::new();
    if let Some(ip) = ip {
        for i in 0..count.max(1) {
            let sample = match port {
                Some(p) => tcp_ping_once(SocketAddr::new(IpAddr::V4(ip), p), Duration::from_millis(timeout_ms as u64)),
                None => ping_once(ip, timeout_ms),
            };
            samples.push(sample);
            if i + 1 < count {
                std::thread::sleep(Duration::from_millis(gap_ms));
            }
        }
    } else {
        samples.push(None);
    }
    let ok: Vec<f32> = samples.iter().filter_map(|s| *s).collect();
    let sent = samples.len() as u32;
    let received = ok.len() as u32;
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
        (Some(min), Some(max), Some((avg * 10.0).round() / 10.0), Some((jitter * 10.0).round() / 10.0))
    };
    PingResult {
        host: host.to_string(),
        ip: ip.map(|i| i.to_string()),
        sent,
        received,
        min_ms,
        avg_ms,
        max_ms,
        jitter_ms,
        loss_pct: if sent == 0 { 100.0 } else { ((sent - received) as f32 / sent as f32) * 100.0 },
        samples,
    }
}

/// Eén snelle meting voor het dashboard.
pub fn quick(host: &str, timeout_ms: u32) -> Option<f32> {
    let (hostname, port) = split_port(host.trim());
    let ip = resolve_v4(&hostname)?;
    match port {
        Some(p) => tcp_ping_once(SocketAddr::new(IpAddr::V4(ip), p), Duration::from_millis(timeout_ms as u64)),
        None => ping_once(ip, timeout_ms),
    }
}
