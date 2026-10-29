//! Hulpfuncties om PowerShell en losse Windows-programma's (powercfg, netsh, ipconfig) zonder
//! zichtbaar consolevenster uit te voeren.

use crate::error::{AppError, AppResult};
use std::os::windows::process::CommandExt;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Voert een PowerShell-script uit en geeft stdout (UTF-8) terug.
pub fn run_ps(script: &str) -> AppResult<String> {
    let full = format!(
        "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; $ErrorActionPreference = 'Stop'; {}",
        script
    );
    let out = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &full,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    if !out.status.success() {
        let first = stderr
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("onbekende fout")
            .to_string();
        let lower = first.to_lowercase();
        if lower.contains("access is denied")
            || lower.contains("toegang geweigerd")
            || lower.contains("administrator")
            || lower.contains("beheerder")
        {
            return Err(AppError::NeedsAdmin(first));
        }
        return Err(AppError::Msg(format!("PowerShell: {first}")));
    }
    Ok(stdout)
}

/// Voert een programma uit en geeft (gelukt, stdout, stderr) terug.
pub fn run_cmd(program: &str, args: &[&str]) -> AppResult<(bool, String, String)> {
    let out = Command::new(program)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    Ok((
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    ))
}

/// Start een programma zonder op het einde te wachten (bv. om een gesloten app opnieuw te starten).
pub fn spawn_detached(program: &str, args: &[&str], cwd: Option<&str>) -> AppResult<()> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.spawn()?;
    Ok(())
}

/// Maakt een string veilig voor gebruik binnen enkele aanhalingstekens in PowerShell.
pub fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}
