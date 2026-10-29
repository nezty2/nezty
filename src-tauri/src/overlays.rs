//! Overlays die over je game tekenen (en zo wat vertraging of haperingen kunnen geven). nezty kijkt
//! alleen naar draaiende programma's en instellingen in Windows. Er wordt niets in de game gekeken.

use crate::error::{AppError, AppResult};
use crate::models::{OverlayInfo, TweakState};
use crate::state::AppState;
use crate::{processes, tweaks};

/// id, procesnamen (kleine letters), naam om te sluiten (leeg = alleen uitleg).
const APPS: &[(&str, &[&str], &str)] = &[
    ("discord", &["discord.exe", "discordptb.exe", "discordcanary.exe"], ""),
    ("nvidia", &["nvidia overlay.exe", "nvidia share.exe"], ""),
    ("amd", &["amdrsserv.exe", "radeonsoftware.exe"], ""),
    ("steam", &["steam.exe"], ""),
    ("overwolf", &["overwolf.exe"], "Overwolf.exe"),
    ("medal", &["medal.exe"], "Medal.exe"),
    ("rtss", &["rtss.exe"], "RTSS.exe"),
];

pub fn list(state: &AppState) -> Vec<OverlayInfo> {
    let running = processes::running_names(state);
    let game_bar_off = tweaks::status(state, "game_bar_dvr_off", None).state == TweakState::On;
    let mut out = vec![OverlayInfo { id: "game_bar".into(), active: !game_bar_off, fix: "tweak".into() }];
    for (id, names, close) in APPS {
        if names.iter().any(|n| running.contains(*n)) {
            out.push(OverlayInfo { id: (*id).into(), active: true, fix: if close.is_empty() { "manual".into() } else { "close".into() } });
        }
    }
    out
}

/// Sluit een overlay-app die je zonder gevolgen kunt sluiten (Overwolf, Medal, RivaTuner).
pub fn close(state: &AppState, id: &str) -> AppResult<Vec<OverlayInfo>> {
    let (_, _, name) = APPS.iter().find(|(i, _, c)| *i == id && !c.is_empty()).ok_or_else(|| AppError::NotFound(id.into()))?;
    processes::close_many(state, &[name.to_string()], None, None);
    std::thread::sleep(std::time::Duration::from_millis(600));
    Ok(list(state))
}
