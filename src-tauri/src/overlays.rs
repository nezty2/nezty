//! Overlays die over je game tekenen (en zo wat vertraging of haperingen kunnen geven). nezty kijkt
//! alleen naar draaiende programma's en instellingen in Windows. Er wordt niets in de game gekeken.

use crate::error::{AppError, AppResult};
use crate::models::{OverlayInfo, TweakState};
use crate::state::AppState;
use crate::{processes, tweaks};

/// id, procesnamen (kleine letters), naam om te sluiten (leeg = niet sluiten), link naar de instelling in de app
/// zelf (leeg = alleen uitleg). De overlay-instelling van Discord en Steam staat in je account bij die app, dus
/// nezty kan niet zien of hij aan staat en kan hem niet zelf uitzetten. De link opent wel precies die instelling.
const APPS: &[(&str, &[&str], &str, &str)] = &[
    ("discord", &["discord.exe", "discordptb.exe", "discordcanary.exe"], "", "discord://-/settings/overlay"),
    ("nvidia", &["nvidia overlay.exe", "nvidia share.exe"], "", ""),
    ("amd", &["amdrsserv.exe", "radeonsoftware.exe"], "", ""),
    ("steam", &["steam.exe"], "", "steam://settings/ingame"),
    ("overwolf", &["overwolf.exe"], "Overwolf.exe", ""),
    ("medal", &["medal.exe"], "Medal.exe", ""),
    ("rtss", &["rtss.exe"], "RTSS.exe", ""),
];

pub fn list(state: &AppState) -> Vec<OverlayInfo> {
    let running = processes::running_names(state);
    let game_bar_off = tweaks::status(state, "game_bar_dvr_off", None).state == TweakState::On;
    let mut out = vec![OverlayInfo { id: "game_bar".into(), active: !game_bar_off, fix: "tweak".into(), open: None }];
    for (id, names, close, link) in APPS {
        if names.iter().any(|n| running.contains(*n)) {
            let fix = if !close.is_empty() {
                "close"
            } else if !link.is_empty() {
                "open"
            } else {
                "manual"
            };
            out.push(OverlayInfo { id: (*id).into(), active: true, fix: fix.into(), open: (!link.is_empty()).then(|| link.to_string()) });
        }
    }
    out
}

/// Sluit een overlay-app die je zonder gevolgen kunt sluiten (Overwolf, Medal, RivaTuner).
pub fn close(state: &AppState, id: &str) -> AppResult<Vec<OverlayInfo>> {
    let (_, _, name, _) = APPS.iter().find(|(i, _, c, _)| *i == id && !c.is_empty()).ok_or_else(|| AppError::NotFound(id.into()))?;
    processes::close_many(state, &[name.to_string()], None, None);
    std::thread::sleep(std::time::Duration::from_millis(600));
    Ok(list(state))
}
