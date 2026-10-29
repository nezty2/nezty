//! Tijdelijke bestanden en shader-caches: eerst een overzicht, dan pas verwijderen.

use crate::error::AppResult;
use crate::journal::JournalEntry;
use crate::models::{CleanupItem, CleanupPreview, CleanupResult, JournalKind, Msg};
use crate::state::{lock, AppState};
use std::path::{Path, PathBuf};

struct Candidate {
    id: &'static str,
    path: PathBuf,
    shader: bool,
    needs_admin: bool,
}

fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from)
}

fn candidates() -> Vec<Candidate> {
    let local = env_path("LOCALAPPDATA").unwrap_or_default();
    let program_data = env_path("PROGRAMDATA").unwrap_or_else(|| PathBuf::from("C:\\ProgramData"));
    let windir = env_path("WINDIR").unwrap_or_else(|| PathBuf::from("C:\\Windows"));
    let c = |id, path, shader, needs_admin| Candidate { id, path, shader, needs_admin };
    vec![
        c("temp", env_path("TEMP").unwrap_or_default(), false, false),
        c("wintemp", windir.join("Temp"), false, true),
        c("nv-dx", local.join("NVIDIA").join("DXCache"), true, false),
        c("nv-gl", local.join("NVIDIA").join("GLCache"), true, false),
        c("nv-cache", local.join("NVIDIA Corporation").join("NV_Cache"), true, false),
        c("nv-cache-pd", program_data.join("NVIDIA Corporation").join("NV_Cache"), true, true),
        c("amd-dx", local.join("AMD").join("DxCache"), true, false),
        c("amd-gl", local.join("AMD").join("GLCache"), true, false),
        c("amd-vk", local.join("AMD").join("VkCache"), true, false),
        c("intel", local.join("Intel").join("ShaderCache"), true, false),
        c("d3d", local.join("D3DSCache"), true, false),
    ]
}

fn dir_stats(path: &Path) -> (u64, u64) {
    let mut size = 0u64;
    let mut files = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                size += meta.len();
                files += 1;
            }
        }
    }
    (size, files)
}

pub fn preview(state: &AppState) -> CleanupPreview {
    let mut items = vec![];
    for c in candidates() {
        if !c.path.is_dir() {
            continue;
        }
        let (size, files) = dir_stats(&c.path);
        if files == 0 {
            continue;
        }
        items.push(CleanupItem {
            id: c.id.to_string(),
            path: c.path.to_string_lossy().to_string(),
            size_bytes: size,
            file_count: files,
            shader_cache: c.shader,
            needs_admin: c.needs_admin && !state.elevated,
        });
    }
    let total_bytes = items.iter().map(|i| i.size_bytes).sum();
    CleanupPreview { items, total_bytes }
}

fn clean_dir(path: &Path, result: &mut CleanupResult) {
    let Ok(rd) = std::fs::read_dir(path) else { return };
    for entry in rd.flatten() {
        let p = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            clean_dir(&p, result);
            let _ = std::fs::remove_dir(&p);
        } else {
            let len = meta.len();
            match std::fs::remove_file(&p) {
                Ok(()) => {
                    result.deleted_files += 1;
                    result.freed_bytes += len;
                }
                Err(_) => result.skipped_files += 1,
            }
        }
    }
}

pub fn run(state: &AppState, item_ids: &[String]) -> AppResult<CleanupResult> {
    let mut result = CleanupResult::default();
    let cands = candidates();
    for id in item_ids {
        let Some(c) = cands.iter().find(|c| c.id == id) else { continue };
        if c.needs_admin && !state.elevated {
            result.admin_skipped.push(c.id.to_string());
            continue;
        }
        if !c.path.is_dir() {
            continue;
        }
        clean_dir(&c.path, &mut result);
    }
    let mut e = JournalEntry::new("cleanup_temp", "Clean temp files and shader cache", JournalKind::Cleanup, "%TEMP%, shader cache");
    e.detail = Some(
        Msg::new("cleanup_done")
            .with("files", result.deleted_files)
            .with("mb", result.freed_bytes / 1_048_576)
            .with("skipped", result.skipped_files),
    );
    e.revertable = false;
    e.note = Some("not_revertable".into());
    lock(&state.journal).record(e)?;
    Ok(result)
}
