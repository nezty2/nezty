//! Game-netwerk, zoals ExitLag, maar alleen wat echt werkt en te meten is: live ping, jitter en verlies naar de
//! echte gameserver, de route ernaartoe met de plek waar het misgaat, voorrang voor gameverkeer via de QoS van
//! Windows, een routetest via Cloudflare WARP, meerdere internetverbindingen en een geheugenbewaker.
//! Elke wijziging aan Windows staat in het logboek en gaat met "Alles terugzetten" exact terug.

pub mod etw;
pub mod geo;
pub mod memguard;
pub mod monitor;
pub mod multinet;
pub mod qos;
pub mod trace;
pub mod warp;

use crate::models::{Game, GameServer, Msg};
use crate::state::AppState;
use crate::tweaks::per_game;

/// Onder dit aantal pakketten per seconde is het geen gameserver (een game in het menu stuurt weinig).
const MIN_GAME_PPS: f32 = 8.0;

/// Met welke server praat deze game nu? Kijkt naar het echte verkeer van het gameproces (zie etw.rs).
pub fn detect_server(state: &AppState, game: &Game) -> GameServer {
    let mut out = GameServer { status: "not_running".into(), game_id: game.id.clone(), ip: None, port: None, packets_per_sec: None, location: None, detail: None };
    let pids = per_game::running_pids(state, game);
    if pids.is_empty() {
        return out;
    }
    if !state.elevated {
        out.status = "needs_admin".into();
        return out;
    }
    if let Err(e) = etw::ensure_running() {
        out.status = "error".into();
        out.detail = Some(Msg::new("status_error").with("detail", e.to_string()));
        return out;
    }
    etw::watch(&pids);
    match etw::busiest_peer(&pids) {
        Some(p) if p.packets_per_sec >= MIN_GAME_PPS => {
            out.status = "found".into();
            out.ip = Some(p.ip.to_string());
            out.port = Some(p.port);
            out.packets_per_sec = Some((p.packets_per_sec * 10.0).round() / 10.0);
            out.location = geo::locate(state, p.ip);
        }
        // Net begonnen met kijken, of de game zit nog in het menu (geen gameserver).
        _ => out.status = if etw::watching_secs() < 4 { "listening" } else { "idle" }.into(),
    }
    out
}

/// Bij het afsluiten van nezty: de ETW-sessie en de live meting stoppen.
pub fn shutdown() {
    monitor::stop();
    etw::stop();
}
