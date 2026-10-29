//! FPS meten met Intel PresentMon (meegeleverd, ondertekend door Intel). Leest de tijd tussen frames
//! (MsBetweenPresents) en berekent gemiddelde FPS en 1%-low. Vereist administratorrechten (ETW).

use crate::error::{AppError, AppResult};
use crate::models::{FpsResult, Game};
use crate::state::AppState;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use tauri::{AppHandle, Manager};

pub const DEFAULT_SECONDS: u32 = 10;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn presentmon_path(app: &AppHandle) -> AppResult<PathBuf> {
    let candidates = [
        app.path().resolve("resources/PresentMon.exe", tauri::path::BaseDirectory::Resource).ok(),
        app.path().resolve("PresentMon.exe", tauri::path::BaseDirectory::Resource).ok(),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|p| p.is_file())
        .ok_or_else(|| AppError::coded("fps_tool_missing"))
}

fn process_name(state: &AppState, game: &Game) -> Option<String> {
    let pids = crate::tweaks::per_game::running_pids(state, game);
    if pids.is_empty() {
        return None;
    }
    // De naam van het proces dat echt draait (games hebben soms meerdere exe-namen).
    let sys = crate::state::lock(&state.procs);
    pids.iter().find_map(|pid| sys.process(sysinfo::Pid::from_u32(*pid)).map(|p| p.name().to_string_lossy().to_string()))
}

/// Meet `seconds` seconden de framerate van de draaiende game.
pub fn measure(app: &AppHandle, state: &AppState, game: &Game, seconds: u32) -> AppResult<FpsResult> {
    if !state.elevated {
        return Err(AppError::coded("fps_needs_admin"));
    }
    let exe = presentmon_path(app)?;
    let proc_name = process_name(state, game).ok_or_else(|| AppError::coded("fps_game_not_running"))?;
    let out_file = std::env::temp_dir().join(format!("nezty-fps-{}.csv", uuid::Uuid::new_v4()));
    let status = Command::new(&exe)
        .args([
            "--process_name",
            &proc_name,
            "--output_file",
            &out_file.to_string_lossy(),
            "--timed",
            &seconds.to_string(),
            "--terminate_after_timed",
            "--no_console_stats",
            "--stop_existing_session",
            "--session_name",
            "nezty_fps",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .status()?;
    let text = std::fs::read_to_string(&out_file).unwrap_or_default();
    let _ = std::fs::remove_file(&out_file);
    if !status.success() && text.is_empty() {
        return Err(AppError::coded("fps_failed").with("detail", format!("PresentMon exit {:?}", status.code())));
    }
    parse_csv(&text, seconds).ok_or_else(|| AppError::coded("fps_no_frames"))
}

/// Haalt de frametijden uit de CSV en rekent gemiddelde FPS en 1%-low uit.
pub fn parse_csv(text: &str, seconds: u32) -> Option<FpsResult> {
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next()?.split(',').map(|s| s.trim()).collect();
    let col = header
        .iter()
        .position(|h| h.eq_ignore_ascii_case("MsBetweenPresents"))
        .or_else(|| header.iter().position(|h| h.eq_ignore_ascii_case("FrameTime")))?;
    let mut times: Vec<f64> = lines
        .filter_map(|l| l.split(',').nth(col))
        .filter_map(|v| v.trim().parse::<f64>().ok())
        .filter(|v| *v > 0.0 && *v < 1000.0)
        .collect();
    if times.len() < 10 {
        return None;
    }
    let total_ms: f64 = times.iter().sum();
    let avg_fps = 1000.0 * times.len() as f64 / total_ms;
    // 1%-low: gemiddelde FPS van de traagste 1% frames.
    times.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let n = (times.len() / 100).max(1);
    let slow_avg = times[..n].iter().sum::<f64>() / n as f64;
    Some(FpsResult {
        avg_fps: (avg_fps * 10.0).round() as f32 / 10.0,
        low1_fps: ((1000.0 / slow_avg) * 10.0).round() as f32 / 10.0,
        frames: times.len() as u32,
        seconds: seconds as f32,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_csv;

    #[test]
    fn parses_presentmon_csv() {
        let mut csv = String::from("Application,ProcessID,MsBetweenPresents,MsInPresentAPI\n");
        for i in 0..200 {
            let ft = if i % 100 == 0 { 20.0 } else { 5.0 };
            csv.push_str(&format!("game.exe,1,{ft},0.1\n"));
        }
        let r = parse_csv(&csv, 1).unwrap();
        assert_eq!(r.frames, 200);
        assert!(r.avg_fps > 190.0 && r.avg_fps < 200.0, "avg {}", r.avg_fps);
        assert!((r.low1_fps - 50.0).abs() < 0.5, "low {}", r.low1_fps);
    }
}
