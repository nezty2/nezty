//! Live meting naar één doel (de gameserver of een datacenter): elke halve seconde één meting, met de statistiek
//! van de laatste 30 seconden. Draait op een eigen thread en stuurt elke meting als event "netmon-sample", zodat
//! de interface nooit bevriest. Stopt vanzelf na 30 minuten.

use crate::error::{AppError, AppResult};
use crate::models::{now_ms, LiveSample, LiveStats};
use crate::ping::{summarize, Target};
use crate::state::lock;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// De lopende meting: voor welk doel, en de vlag om hem te stoppen.
static RUNNING: Mutex<Option<(String, Arc<AtomicBool>)>> = Mutex::new(None);
/// 60 metingen van een halve seconde: 30 seconden.
const WINDOW: usize = 60;
const INTERVAL: Duration = Duration::from_millis(500);
const MAX_RUN: Duration = Duration::from_secs(30 * 60);

pub fn start(app: AppHandle, host: String) -> AppResult<()> {
    let target = Target::resolve(&host).ok_or_else(|| AppError::coded("host_unknown").with("host", &host))?;
    stop();
    let flag = Arc::new(AtomicBool::new(false));
    *lock(&RUNNING) = Some((host.clone(), flag.clone()));
    // Een TCP-meting maakt telkens een verbinding met het datacenter: die eens per seconde, ping elke halve.
    let interval = if matches!(target, Target::Tcp(_)) { Duration::from_secs(1) } else { INTERVAL };
    std::thread::Builder::new().name("netmon".into()).spawn(move || {
        let started = Instant::now();
        let mut window: VecDeque<Option<f32>> = VecDeque::with_capacity(WINDOW);
        while !flag.load(Ordering::Relaxed) && started.elapsed() < MAX_RUN {
            let t0 = Instant::now();
            let ms = target.once(1000);
            if window.len() == WINDOW {
                window.pop_front();
            }
            window.push_back(ms);
            let s = summarize(window.iter());
            let stats = LiveStats { samples: s.sent, avg_ms: s.avg_ms, min_ms: s.min_ms, max_ms: s.max_ms, jitter_ms: s.jitter_ms, loss_pct: s.loss_pct };
            if flag.load(Ordering::Relaxed) {
                break;
            }
            let _ = app.emit("netmon-sample", LiveSample { host: host.clone(), ts: now_ms(), ms, stats });
            if let Some(rest) = interval.checked_sub(t0.elapsed()) {
                std::thread::sleep(rest);
            }
        }
    })?;
    Ok(())
}

pub fn stop() {
    if let Some((_, flag)) = lock(&RUNNING).take() {
        flag.store(true, Ordering::Relaxed);
    }
}

/// Stopt alleen als de lopende meting voor dit doel is. Zo kan het stoppen van een oude meting (bij het wisselen
/// van server) nooit de nieuwe stoppen, ook niet als de aanroepen in een andere volgorde aankomen.
pub fn stop_host(host: &str) {
    let mut running = lock(&RUNNING);
    if running.as_ref().is_some_and(|(h, _)| h == host) {
        if let Some((_, flag)) = running.take() {
            flag.store(true, Ordering::Relaxed);
        }
    }
}
