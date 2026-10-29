//! "Test vóór en na": meet tijdens het spelen de FPS, 1%-low, haperingen en frametijden (PresentMon) en
//! tegelijk de ping, schommeling en pakketverlies naar de gekozen regio. Alleen meten, niets beloven.

use crate::models::FpsDetail;

/// Aantal punten in de frametijdgrafiek.
const GRAPH_POINTS: usize = 240;

fn median(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    s.get(s.len() / 2).copied().unwrap_or(0.0)
}

fn column(header: &[&str], names: &[&str]) -> Option<usize> {
    names.iter().find_map(|n| header.iter().position(|h| h.eq_ignore_ascii_case(n)))
}

/// Rekent alles uit wat we tonen. Een hapering is een frame dat minstens twee keer zo lang duurt als normaal.
pub fn analyze(text: &str, seconds: f32) -> Option<FpsDetail> {
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next()?.split(',').map(|s| s.trim()).collect();
    let ft_col = column(&header, &["MsBetweenPresents", "FrameTime"])?;
    let gpu_col = column(&header, &["GPUBusy", "MsGPUBusy"]);
    let cpu_col = column(&header, &["CPUBusy", "MsCPUBusy"]);
    let mut times = vec![];
    let mut gpu = vec![];
    let mut cpu = vec![];
    for line in lines {
        let cols: Vec<&str> = line.split(',').collect();
        let Some(ft) = cols.get(ft_col).and_then(|v| v.trim().parse::<f64>().ok()) else { continue };
        if !(ft > 0.0 && ft < 1000.0) {
            continue;
        }
        times.push(ft);
        if let Some(g) = gpu_col.and_then(|c| cols.get(c)).and_then(|v| v.trim().parse::<f64>().ok()) {
            gpu.push(g);
        }
        if let Some(c) = cpu_col.and_then(|c| cols.get(c)).and_then(|v| v.trim().parse::<f64>().ok()) {
            cpu.push(c);
        }
    }
    if times.len() < 30 {
        return None;
    }
    let total: f64 = times.iter().sum();
    let avg_fps = 1000.0 * times.len() as f64 / total;
    let med = median(&times);
    let mut sorted = times.clone();
    sorted.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let n1 = (times.len() / 100).max(1);
    let low1 = 1000.0 / (sorted[..n1].iter().sum::<f64>() / n1 as f64);
    let stutters = times.iter().filter(|t| **t >= 2.0 * med && **t - med >= 4.0).count() as u32;
    let minutes = (total / 60_000.0).max(1.0 / 60.0);

    // Grafiek: per tijdvak de langste frametijd, zodat pieken zichtbaar blijven.
    let mut graph = vec![0f32; GRAPH_POINTS];
    let mut t = 0.0;
    for ft in &times {
        let i = (((t / total) * GRAPH_POINTS as f64) as usize).min(GRAPH_POINTS - 1);
        graph[i] = graph[i].max(*ft as f32);
        t += ft;
    }
    // Wie is de grens? Werkt de videokaart (of processor) bijna de hele frametijd, dan zit daar de rem.
    let share = |v: &[f64]| (v.len() * 2 >= times.len()).then(|| (median(v) / med).min(1.0) as f32);
    let round = |x: f64| (x * 10.0).round() as f32 / 10.0;
    Some(FpsDetail {
        avg_fps: round(avg_fps),
        low1_fps: round(low1),
        frames: times.len() as u32,
        seconds,
        median_ms: round(med),
        stutters,
        stutters_per_min: round(stutters as f64 / minutes),
        gpu_busy: share(&gpu),
        cpu_busy: share(&cpu),
        graph,
    })
}
