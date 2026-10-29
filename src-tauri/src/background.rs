//! Eigen achtergrond voor de app. De gekozen afbeelding wordt naar de gegevensmap gekopieerd, zodat de
//! app hem ook vindt als het origineel verplaatst wordt. Er staat altijd hooguit één afbeelding.

use crate::error::{AppError, AppResult};
use crate::models::Settings;
use crate::state::{lock, AppState};
use std::path::{Path, PathBuf};

const EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "gif"];
const MAX_BYTES: u64 = 20 * 1024 * 1024;

fn clear(dir: &Path) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// Volledig pad van de huidige achtergrond, als die er is.
pub fn current(state: &AppState) -> Option<PathBuf> {
    let name = lock(&state.settings).background_image.clone()?;
    let p = state.background_dir().join(name);
    p.is_file().then_some(p)
}

/// Zet een nieuwe achtergrond (pad naar een afbeelding) of haalt hem weg (`None`).
pub fn set(state: &AppState, source: Option<&str>) -> AppResult<Settings> {
    let dir = state.background_dir();
    std::fs::create_dir_all(&dir)?;
    let name = match source {
        None => {
            clear(&dir);
            None
        }
        Some(src) => {
            let src = Path::new(src);
            let ext = src.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            if !EXTENSIONS.contains(&ext.as_str()) {
                return Err(AppError::coded("background_type"));
            }
            if std::fs::metadata(src)?.len() > MAX_BYTES {
                return Err(AppError::coded("background_too_big"));
            }
            clear(&dir);
            let name = format!("background-{}.{ext}", chrono::Local::now().format("%Y%m%d%H%M%S"));
            std::fs::copy(src, dir.join(&name))?;
            Some(name)
        }
    };
    lock(&state.settings).background_image = name;
    state.save_settings();
    Ok(state.settings_clone())
}
