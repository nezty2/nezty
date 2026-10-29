//! Bufferbloat-test: hoeveel stijgt je ping als je verbinding vol zit? Eerst de ping in rust, dan tijdens
//! een download en tijdens een upload via de snelheidstest van Cloudflare. Een grote stijging betekent
//! dat je router pakketjes laat wachten. Dat los je op met QoS of SQM in de router, niet met tweaks.

use crate::error::{AppError, AppResult};
use crate::models::BufferbloatResult;
use crate::ping;
use std::io::Read;
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

const TARGET: Ipv4Addr = Ipv4Addr::new(1, 1, 1, 1);
const DOWN_URL: &str = "https://speed.cloudflare.com/__down?bytes=50000000";
const UP_URL: &str = "https://speed.cloudflare.com/__up";
const STREAMS: usize = 4;
/// Maximaal 8 seconden belasting per richting.
const LOAD_TIME: Duration = Duration::from_secs(8);
const UP_BYTES: u64 = 20_000_000;

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        // Windows-certificaatopslag gebruiken, net als de browser.
        .tls_config(ureq::tls::TlsConfig::builder().provider(ureq::tls::TlsProvider::NativeTls).root_certs(ureq::tls::RootCerts::PlatformVerifier).build())
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .into()
}

fn median(v: &mut [f32]) -> Option<f32> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Some(v[v.len() / 2])
}

/// Pingt tot `stop` gezet wordt of de tijd om is. Geeft de geslaagde metingen terug.
fn ping_while(stop: &AtomicBool, max: Duration, gap: Duration) -> Vec<f32> {
    let start = Instant::now();
    let mut out = vec![];
    while !stop.load(Ordering::Relaxed) && start.elapsed() < max {
        if let Some(ms) = ping::ping_once(TARGET, 1000) {
            out.push(ms);
        }
        std::thread::sleep(gap);
    }
    out
}

/// Bron voor de upload: nullen, tot de limiet of tot er gestopt wordt.
struct Zeros {
    left: u64,
    stop: Arc<AtomicBool>,
    sent: Arc<AtomicU64>,
}

impl Read for Zeros {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.left == 0 || self.stop.load(Ordering::Relaxed) {
            return Ok(0);
        }
        let n = buf.len().min(self.left as usize).min(64 * 1024);
        buf[..n].fill(0);
        self.left -= n as u64;
        self.sent.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

/// Belast de verbinding (download of upload) en pingt ondertussen. Geeft pings en Mbit/s terug.
fn loaded(upload: bool) -> (Vec<f32>, Option<f32>, Vec<String>) {
    let errors = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let stop = Arc::new(AtomicBool::new(false));
    let moved = Arc::new(AtomicU64::new(0));
    let agent = agent();
    let mut handles = vec![];
    let started = Instant::now();
    for _ in 0..STREAMS {
        let (stop, moved, agent, errors) = (stop.clone(), moved.clone(), agent.clone(), errors.clone());
        handles.push(std::thread::spawn(move || {
            if upload {
                let body = Zeros { left: UP_BYTES, stop: stop.clone(), sent: moved.clone() };
                if let Err(e) = agent
                    .post(UP_URL)
                    .header("User-Agent", "nezty-bufferbloat")
                    .header("Content-Type", "application/octet-stream")
                    .send(ureq::SendBody::from_owned_reader(body))
                {
                    if let Ok(mut l) = errors.lock() {
                        l.push(format!("upload: {e}"));
                    }
                }
                return;
            }
            match agent.get(DOWN_URL).header("User-Agent", "nezty-bufferbloat").call() {
                Err(e) => {
                    if let Ok(mut l) = errors.lock() {
                        l.push(format!("download: {e}"));
                    }
                }
                Ok(mut resp) => {
                let mut reader = resp.body_mut().with_config().limit(60_000_000).reader();
                let mut buf = vec![0u8; 64 * 1024];
                while !stop.load(Ordering::Relaxed) {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Err(e) => {
                            if let Ok(mut l) = errors.lock() {
                                l.push(format!("download read: {e}"));
                            }
                            break;
                        }
                        Ok(n) => {
                            moved.fetch_add(n as u64, Ordering::Relaxed);
                        }
                    }
                }
                }
            }
        }));
    }
    // Eerst even laten oplopen, dan meten zolang er nog verkeer loopt.
    std::thread::sleep(Duration::from_millis(700));
    let done = Arc::new(AtomicBool::new(false));
    let watcher = {
        let (done, handles_done) = (done.clone(), Arc::new(handles));
        let h = handles_done.clone();
        std::thread::spawn(move || {
            while !h.iter().all(|t| t.is_finished()) {
                std::thread::sleep(Duration::from_millis(50));
            }
            done.store(true, Ordering::Relaxed);
            handles_done
        })
    };
    let samples = ping_while(&done, LOAD_TIME, Duration::from_millis(120));
    stop.store(true, Ordering::Relaxed);
    if let Ok(handles) = watcher.join() {
        if let Ok(list) = Arc::try_unwrap(handles) {
            for h in list {
                let _ = h.join();
            }
        }
    }
    let secs = started.elapsed().as_secs_f32().max(0.1);
    let bytes = moved.load(Ordering::Relaxed);
    let mbps = (bytes > 1_000_000).then(|| bytes as f32 * 8.0 / 1_000_000.0 / secs);
    let errs = errors.lock().map(|l| l.clone()).unwrap_or_default();
    (samples, mbps, errs)
}

/// Cijfer zoals bij de bekende bufferbloat-tests: hoeveel ms komt er bij onder belasting?
pub fn grade(increase_ms: f32) -> &'static str {
    match increase_ms {
        x if x < 5.0 => "A+",
        x if x < 30.0 => "A",
        x if x < 60.0 => "B",
        x if x < 200.0 => "C",
        x if x < 400.0 => "D",
        _ => "F",
    }
}

pub fn run(app: &AppHandle) -> AppResult<BufferbloatResult> {
    run_with(|phase| {
        let _ = app.emit("bufferbloat-phase", phase);
    })
}

pub fn run_with(progress: impl Fn(&str)) -> AppResult<BufferbloatResult> {
    progress("idle");
    let stop = AtomicBool::new(false);
    let mut idle = ping_while(&stop, Duration::from_secs(3), Duration::from_millis(150));
    let idle_ms = median(&mut idle).ok_or_else(|| AppError::coded("bufferbloat_offline"))?;

    progress("download");
    let (mut down, down_mbps, mut errors) = loaded(false);
    progress("upload");
    let (mut up, up_mbps, up_errors) = loaded(true);
    errors.extend(up_errors);
    progress("done");

    let down_ms = median(&mut down);
    let up_ms = median(&mut up);
    if down_mbps.is_none() && up_mbps.is_none() {
        errors.dedup();
        return Err(AppError::coded("bufferbloat_no_load").with("detail", errors.join("; ")));
    }
    let worst = down_ms.unwrap_or(idle_ms).max(up_ms.unwrap_or(idle_ms));
    let increase = (worst - idle_ms).max(0.0);
    Ok(BufferbloatResult {
        idle_ms,
        download_ms: down_ms,
        upload_ms: up_ms,
        download_mbps: down_mbps,
        upload_mbps: up_mbps,
        increase_ms: increase,
        grade: grade(increase).into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grades() {
        assert_eq!(grade(2.0), "A+");
        assert_eq!(grade(25.0), "A");
        assert_eq!(grade(45.0), "B");
        assert_eq!(grade(150.0), "C");
        assert_eq!(grade(500.0), "F");
    }
}
