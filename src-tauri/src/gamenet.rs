//! Wat er over is van het game-netwerk (tot 0.7.3: live ping, route, QoS-voorrang, WARP, Multi-Internet en de
//! geheugenbewaker). Dat is weggehaald: het gaf in de praktijk meer vertraging dan het oploste. Wat het ooit aan
//! Windows veranderde, staat nog in het logboek. Dit bestand kan die wijzigingen alleen nog exact terugzetten.

use crate::error::{AppError, AppResult};
use crate::powershell::{ps_quote, run_cmd, run_ps};
use std::path::PathBuf;
use windows_sys::Win32::NetworkManagement::IpHelper::{GetIpInterfaceEntry, InitializeIpInterfaceEntry, SetIpInterfaceEntry, MIB_IPINTERFACE_ROW};
use windows_sys::Win32::Networking::WinSock::AF_INET;

/// Haalt een QoS-regel van nezty weg. Bestaat hij al niet meer, dan is dat ook goed.
pub fn remove_policy(name: &str) -> AppResult<()> {
    run_ps(&format!("Remove-NetQosPolicy -Name {} -PolicyStore localhost -Confirm:$false -ErrorAction SilentlyContinue", ps_quote(name)))?;
    Ok(())
}

/// WARP uitzetten als nezty het had aangezet. Is WARP inmiddels verwijderd, dan staat er ook niets meer aan.
pub fn warp_revert() -> AppResult<()> {
    let base = std::env::var_os("ProgramFiles").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\Program Files"));
    let cli = base.join("Cloudflare").join("Cloudflare WARP").join("warp-cli.exe");
    if !cli.is_file() {
        return Ok(());
    }
    run_cmd(&cli.to_string_lossy(), &["disconnect"]).map(|_| ())
}

/// De metric van een verbinding terugzetten (Multi-Internet). Bestaat de verbinding niet meer (bv. een
/// losgekoppelde telefoon), dan is er niets terug te zetten.
pub fn revert_metric(luid: u64, family: u16, automatic: bool, metric: u32) -> AppResult<()> {
    unsafe {
        let mut row: MIB_IPINTERFACE_ROW = std::mem::zeroed();
        InitializeIpInterfaceEntry(&mut row);
        row.Family = family;
        row.InterfaceLuid.Value = luid;
        if GetIpInterfaceEntry(&mut row) != 0 {
            return Ok(());
        }
        row.UseAutomaticMetric = automatic;
        if !automatic {
            row.Metric = metric;
        }
        // Voorschrift van SetIpInterfaceEntry voor IPv4.
        if family == AF_INET {
            row.SitePrefixLength = 0;
        }
        match SetIpInterfaceEntry(&mut row) {
            0 => Ok(()),
            5 => Err(AppError::NeedsAdmin("SetIpInterfaceEntry".into())),
            rc => Err(AppError::msg(format!("SetIpInterfaceEntry: {rc}"))),
        }
    }
}

/// De ETW-sessie van de oude servermeting stoppen als die na een crash is blijven lopen: die kijkt naar elk
/// netwerkpakket en kost dan voor niets CPU-tijd tot de volgende herstart van Windows.
pub fn stop_old_trace() {
    let _ = run_cmd("logman", &["stop", "nezty-gamenet", "-ets"]);
}
