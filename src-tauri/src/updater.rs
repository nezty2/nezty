//! Updates. De updatebron (latest.json) en de openbare sleutel staan in tauri.conf.json onder
//! plugins."nezty-update". Een update wordt alleen uitgevoerd als:
//!   1. de versie hoger is dan de huidige,
//!   2. het adres https is (http alleen in een testbuild met allowInsecure),
//!   3. de minisign-handtekening klopt met de ingebouwde openbare sleutel.
//! Daarna start de installer in updatemodus (/P /UPDATE /R): passief, gegevens blijven staan
//! en nezty start na afloop vanzelf weer.

use crate::error::{AppError, AppResult};
use base64::Engine;
use serde::Serialize;
use std::io::Read;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub notes: Option<String>,
}

struct Source {
    endpoints: Vec<String>,
    pubkey: String,
    allow_insecure: bool,
}

struct Found {
    info: UpdateInfo,
    url: String,
    signature: String,
}

fn source(app: &AppHandle) -> Option<Source> {
    let cfg = app.config().plugins.0.get("nezty-update")?;
    let endpoints: Vec<String> = cfg
        .get("endpoints")?
        .as_array()?
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty())
        .collect();
    if endpoints.is_empty() {
        return None;
    }
    Some(Source {
        endpoints,
        pubkey: cfg.get("pubkey")?.as_str()?.to_string(),
        allow_insecure: cfg.get("allowInsecure").and_then(|v| v.as_bool()).unwrap_or(false),
    })
}

/// Is er een updatebron ingebouwd?
pub fn configured(app: &AppHandle) -> bool {
    source(app).is_some()
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .tls_config(ureq::tls::TlsConfig::builder().provider(ureq::tls::TlsProvider::NativeTls).build())
        .timeout_global(Some(Duration::from_secs(120)))
        .http_status_as_error(true)
        .build()
        .into()
}

fn allowed_url(url: &str, allow_insecure: bool) -> bool {
    url.starts_with("https://") || (allow_insecure && url.starts_with("http://"))
}

/// "1.2.3" (eventueel met v ervoor of -beta erachter) als drie getallen.
fn parse_version(v: &str) -> Option<[u64; 3]> {
    let core = v.trim().trim_start_matches('v').split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    Some([parts.next()??, parts.next().unwrap_or(Some(0))?, parts.next().unwrap_or(Some(0))?])
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

fn find(app: &AppHandle) -> AppResult<Option<Found>> {
    let src = source(app).ok_or_else(|| AppError::msg("no update source configured"))?;
    let current = app.package_info().version.to_string();
    let agent = agent();
    let mut last_err = None;
    for ep in &src.endpoints {
        if !allowed_url(ep, src.allow_insecure) {
            last_err = Some(AppError::msg("update source must use https"));
            continue;
        }
        let text = match agent
            .get(ep)
            .header("User-Agent", "nezty-updater")
            .header("Cache-Control", "no-cache")
            .call()
            .and_then(|mut r| r.body_mut().with_config().limit(1024 * 1024).read_to_string())
        {
            Ok(t) => t,
            Err(e) => {
                last_err = Some(AppError::msg(format!("update check failed: {e}")));
                continue;
            }
        };
        let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| AppError::msg(format!("latest.json: {e}")))?;
        let version = v["version"].as_str().unwrap_or_default().to_string();
        if !is_newer(&version, &current) {
            return Ok(None);
        }
        let platform = ["windows-x86_64-nsis", "windows-x86_64"].iter().map(|k| &v["platforms"][*k]).find(|p| p.is_object());
        let Some(p) = platform else {
            return Err(AppError::msg("latest.json has no windows-x86_64 entry"));
        };
        let url = p["url"].as_str().unwrap_or_default().to_string();
        let signature = p["signature"].as_str().unwrap_or_default().to_string();
        if url.is_empty() || signature.is_empty() || !allowed_url(&url, src.allow_insecure) {
            return Err(AppError::msg("latest.json has an invalid url or signature"));
        }
        let notes = v["notes"].as_str().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        return Ok(Some(Found { info: UpdateInfo { version, notes }, url, signature }));
    }
    Err(last_err.unwrap_or_else(|| AppError::msg("update check failed")))
}

/// Zoekt naar een nieuwere versie. None als je de nieuwste hebt.
pub fn check(app: &AppHandle) -> AppResult<Option<UpdateInfo>> {
    Ok(find(app)?.map(|f| f.info))
}

/// Controleert de minisign-handtekening (base64, zoals Tauri die maakt) met de openbare sleutel.
pub fn verify(pubkey_b64: &str, signature_b64: &str, data: &[u8]) -> AppResult<()> {
    let b64 = base64::engine::general_purpose::STANDARD;
    let pk_text = String::from_utf8(b64.decode(pubkey_b64.trim()).map_err(|_| AppError::msg("bad public key"))?)
        .map_err(|_| AppError::msg("bad public key"))?;
    let sig_text = String::from_utf8(b64.decode(signature_b64.trim()).map_err(|_| AppError::msg("bad signature"))?)
        .map_err(|_| AppError::msg("bad signature"))?;
    let pk = minisign_verify::PublicKey::decode(&pk_text).map_err(|_| AppError::msg("bad public key"))?;
    let sig = minisign_verify::Signature::decode(&sig_text).map_err(|_| AppError::msg("bad signature"))?;
    pk.verify(data, &sig, false).map_err(|_| AppError::msg("update signature does not match"))
}

/// Downloadt de update, controleert de handtekening en start de installer. nezty sluit daarna af;
/// de installer start nezty na afloop weer (/R).
pub fn install(app: &AppHandle) -> AppResult<()> {
    let src = source(app).ok_or_else(|| AppError::msg("no update source configured"))?;
    let found = find(app)?.ok_or_else(|| AppError::msg("no update available"))?;

    let mut resp = agent()
        .get(&found.url)
        .header("User-Agent", "nezty-updater")
        .call()
        .map_err(|e| AppError::msg(format!("download failed: {e}")))?;
    let total: Option<u64> = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok());
    let mut reader = resp.body_mut().with_config().limit(500 * 1024 * 1024).reader();
    let mut data = Vec::with_capacity(total.unwrap_or(4 * 1024 * 1024) as usize);
    let mut buf = [0u8; 64 * 1024];
    let mut last_pct = -1i64;
    loop {
        let n = reader.read(&mut buf).map_err(|e| AppError::msg(format!("download failed: {e}")))?;
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
        if let Some(t) = total.filter(|t| *t > 0) {
            let pct = ((data.len() as u64 * 100) / t).min(100) as i64;
            if pct != last_pct {
                last_pct = pct;
                let _ = app.emit("update-progress", pct);
            }
        }
    }

    // Eerst de handtekening, pas daarna iets op schijf zetten en uitvoeren.
    verify(&src.pubkey, &found.signature, &data)?;

    let path = std::env::temp_dir().join(format!("nezty-update-{}.exe", found.info.version));
    std::fs::write(&path, &data)?;
    std::process::Command::new(&path)
        .args(["/P", "/UPDATE", "/R"])
        .spawn()
        .map_err(|e| AppError::msg(format!("could not start the installer: {e}")))?;
    let _ = app.emit("update-progress", 100);

    // Even wachten zodat de interface de melding kan tonen, dan afsluiten zodat de installer bestanden kan vervangen.
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(800));
        handle.exit(0);
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions() {
        assert!(is_newer("0.3.1", "0.3.0"));
        assert!(is_newer("v1.0.0", "0.9.9"));
        assert!(!is_newer("0.3.0", "0.3.0"));
        assert!(!is_newer("0.2.9", "0.3.0"));
        assert!(!is_newer("rubbish", "0.3.0"));
    }
}
