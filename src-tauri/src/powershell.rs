//! Hulpfuncties om PowerShell en losse Windows-programma's (powercfg, netsh, ipconfig) zonder
//! zichtbaar consolevenster uit te voeren.

use crate::error::{AppError, AppResult};
use std::os::windows::process::CommandExt;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn system_tool(relative: &str) -> AppResult<std::path::PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
    let mut path = [0u16; 32768];
    let len = unsafe { GetSystemDirectoryW(path.as_mut_ptr(), path.len() as u32) } as usize;
    if len == 0 || len >= path.len() { return Err(AppError::msg("Windows system directory unavailable")); }
    Ok(std::path::PathBuf::from(std::ffi::OsString::from_wide(&path[..len])).join(relative))
}

/// Voert een PowerShell-script uit en geeft stdout (UTF-8) terug.
pub fn run_ps(script: &str) -> AppResult<String> {
    let full = format!(
        "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; $ErrorActionPreference = 'Stop'; {}",
        script
    );
    let out = Command::new(system_tool(r"WindowsPowerShell\v1.0\powershell.exe")?)
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

/// Levert één PowerShell-stringargument. Ook typografische aanhalingstekens,
/// regeleinden en andere tekens uit apparaatnamen blijven uitsluitend gegevens.
pub fn ps_quote(s: &str) -> String {
    use base64::Engine;
    let value = base64::engine::general_purpose::STANDARD.encode(s.as_bytes());
    format!("([Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{value}')))")
}
