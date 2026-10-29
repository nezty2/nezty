//! Licentie. De server beslist of een sleutel geldig is en hoeveel tijd er over is; de app krijgt daarvan
//! een token dat de server met Ed25519 ondertekent. De app controleert die handtekening met de publieke
//! sleutel en vertrouwt verder niets van zichzelf.
//!
//! - Adres van de server en de publieke sleutel komen bij het bouwen binnen (NEZTY_LICENSE_URL en
//!   NEZTY_LICENSE_PUBKEY). Zonder die twee staat de licentie uit en werkt de app zoals altijd.
//! - De sessie en het token staan in Windows Credential Manager (alleen met "Ingelogd blijven").
//! - Zonder internet werkt een maandlicentie maximaal 24 uur door; een demo alleen online.
//! - "Alles terugzetten" en tweaks uitzetten werken altijd, ook zonder licentie.

use crate::error::{AppError, AppResult};
use crate::models::now_ms;
use crate::registry::{self, Hive, RegData};
use crate::state::{lock, AppState};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

const URL: Option<&str> = option_env!("NEZTY_LICENSE_URL");
const PUBKEY: Option<&str> = option_env!("NEZTY_LICENSE_PUBKEY");
const DEFAULT_TARGET: &str = "nezty/license";

pub fn enabled() -> bool {
    URL.is_some_and(|u| !u.trim().is_empty()) && verifying_key().is_some()
}

fn verifying_key() -> Option<VerifyingKey> {
    let raw = URL_SAFE_NO_PAD.decode(PUBKEY?.trim().trim_end_matches('=')).ok()?;
    VerifyingKey::from_bytes(&raw.try_into().ok()?).ok()
}

// ---------- Token ----------

/// Wat de server ondertekent. Tijden in milliseconden (servertijd).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TokenPayload {
    pub v: u32,
    pub u: Option<String>,
    pub n: Option<String>,
    pub l: String,
    pub t: String,
    pub s: String,
    pub a: Option<i64>,
    pub e: Option<i64>,
    pub h: String,
    pub d: String,
    pub i: i64,
    pub c: i64,
}

/// Controleert handtekening en apparaat. Alleen een token van onze server voor dit apparaat telt.
pub fn verify_token(token: &str, device: &str) -> Option<TokenPayload> {
    verify_with(&verifying_key()?, token, device)
}

fn verify_with(key: &VerifyingKey, token: &str, device: &str) -> Option<TokenPayload> {
    let mut parts = token.split('.');
    let (v, body, sig) = (parts.next()?, parts.next()?, parts.next()?);
    if v != "v1" || parts.next().is_some() {
        return None;
    }
    let sig = Signature::from_slice(&URL_SAFE_NO_PAD.decode(sig).ok()?).ok()?;
    key.verify_strict(format!("{v}.{body}").as_bytes(), &sig).ok()?;
    let payload: TokenPayload = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(body).ok()?).ok()?;
    (payload.v == 1 && payload.d == device).then_some(payload)
}

// ---------- Apparaat-ID ----------

/// Vast per Windows-installatie: afgeleid van MachineGuid, zodat de echte waarde de pc niet verlaat.
pub fn device_id() -> String {
    let guid = match registry::read(Hive::Hklm, "SOFTWARE\\Microsoft\\Cryptography", "MachineGuid") {
        Ok(RegData::Str(s)) => s.trim().to_lowercase(),
        _ => String::from("unknown"),
    };
    let mut h = Sha256::new();
    h.update(b"nezty-device-v1:");
    h.update(guid.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

// ---------- Windows Credential Manager ----------

mod cred {
    use crate::winapi::wide;
    use windows_sys::Win32::Security::Credentials::{CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC};

    pub fn write(target: &str, blob: &str) -> bool {
        let mut t = wide(target);
        let mut user = wide("nezty");
        let mut data = blob.as_bytes().to_vec();
        let mut c: CREDENTIALW = unsafe { std::mem::zeroed() };
        c.Type = CRED_TYPE_GENERIC;
        c.TargetName = t.as_mut_ptr();
        c.UserName = user.as_mut_ptr();
        c.CredentialBlobSize = data.len() as u32;
        c.CredentialBlob = data.as_mut_ptr();
        c.Persist = CRED_PERSIST_LOCAL_MACHINE;
        unsafe { CredWriteW(&c, 0) != 0 }
    }

    pub fn read(target: &str) -> Option<String> {
        let t = wide(target);
        let mut p: *mut CREDENTIALW = std::ptr::null_mut();
        if unsafe { CredReadW(t.as_ptr(), CRED_TYPE_GENERIC, 0, &mut p) } == 0 || p.is_null() {
            return None;
        }
        let out = unsafe {
            let c = &*p;
            let bytes = std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize);
            String::from_utf8(bytes.to_vec()).ok()
        };
        unsafe { CredFree(p as *const _) };
        out
    }

    pub fn delete(target: &str) {
        let t = wide(target);
        unsafe { CredDeleteW(t.as_ptr(), CRED_TYPE_GENERIC, 0) };
    }
}

#[derive(Serialize, Deserialize)]
struct Saved {
    session: String,
    token: String,
}

// ---------- Toestand ----------

pub struct Runtime {
    session: Option<String>,
    token: Option<String>,
    payload: Option<TokenPayload>,
    /// Moment (monotone klok) waarop het token binnenkwam, om de servertijd bij te houden.
    received: Option<Instant>,
    remember: bool,
    cred_target: String,
    device: String,
    /// Laatste reden waarom de sessie verviel (bv. "device_mismatch").
    reason: Option<String>,
    config: Option<PublicConfig>,
}

impl Default for Runtime {
    fn default() -> Self {
        Runtime { session: None, token: None, payload: None, received: None, remember: false, cred_target: DEFAULT_TARGET.into(), device: String::new(), reason: None, config: None }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Contact {
    pub discord: String,
    pub email: String,
    pub website: String,
    pub note: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct PublicConfig {
    pub demo_enabled: bool,
    pub contact: Contact,
    /// False als de server nu niet bereikbaar was (dan de laatst bekende gegevens).
    #[serde(default)]
    pub reachable: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LicenseStatus {
    /// off | signed_out | active | expired | revoked | offline
    pub mode: String,
    pub kind: Option<String>,
    pub username: Option<String>,
    pub key_hint: Option<String>,
    pub license_id: Option<String>,
    pub activated_at: Option<i64>,
    pub expires_at: Option<i64>,
    /// Uiterlijk dan moet de app weer met de server praten.
    pub check_by: Option<i64>,
    /// Geschatte servertijd nu, zodat de frontend dezelfde klok gebruikt.
    pub server_now: i64,
    pub remember: bool,
    pub reason: Option<String>,
}

impl Runtime {
    fn server_now(&self) -> i64 {
        match (&self.payload, self.received) {
            (Some(p), Some(at)) => p.i + at.elapsed().as_millis() as i64,
            _ => now_ms(),
        }
    }

    fn status(&self) -> LicenseStatus {
        let now = self.server_now();
        let mut st = LicenseStatus {
            mode: "signed_out".into(),
            kind: None,
            username: None,
            key_hint: None,
            license_id: None,
            activated_at: None,
            expires_at: None,
            check_by: None,
            server_now: now,
            remember: self.remember,
            reason: self.reason.clone(),
        };
        if !enabled() {
            st.mode = "off".into();
            return st;
        }
        let Some(p) = &self.payload else { return st };
        st.kind = Some(p.t.clone());
        st.username = p.n.clone();
        st.key_hint = Some(p.h.clone());
        st.license_id = Some(p.l.clone());
        st.activated_at = p.a;
        st.expires_at = p.e;
        st.check_by = Some(p.c);
        st.mode = if p.s == "revoked" {
            "revoked"
        } else if p.s == "expired" || p.e.is_some_and(|e| now >= e) {
            "expired"
        } else if now > p.c {
            "offline"
        } else {
            "active"
        }
        .into();
        st
    }

    fn store(&mut self, session: Option<String>, token: String, payload: TokenPayload) {
        if let Some(s) = session {
            self.session = Some(s);
        }
        self.token = Some(token);
        self.payload = Some(payload);
        self.received = Some(Instant::now());
        self.reason = None;
        self.persist();
    }

    fn persist(&self) {
        match (&self.session, &self.token, self.remember) {
            (Some(session), Some(token), true) => {
                let blob = serde_json::to_string(&Saved { session: session.clone(), token: token.clone() }).unwrap_or_default();
                cred::write(&self.cred_target, &blob);
            }
            _ => cred::delete(&self.cred_target),
        }
    }

    fn clear(&mut self, reason: Option<&str>) {
        self.session = None;
        self.token = None;
        self.payload = None;
        self.received = None;
        self.reason = reason.map(|s| s.to_string());
        cred::delete(&self.cred_target);
    }
}

// ---------- Server ----------

enum NetError {
    Offline,
    Server(String),
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .tls_config(ureq::tls::TlsConfig::builder().provider(ureq::tls::TlsProvider::NativeTls).root_certs(ureq::tls::RootCerts::PlatformVerifier).build())
        .timeout_global(Some(Duration::from_secs(12)))
        .http_status_as_error(false)
        .build()
        .into()
}

fn post(path: &str, body: serde_json::Value) -> Result<serde_json::Value, NetError> {
    let base = URL.ok_or(NetError::Offline)?.trim_end_matches('/');
    let mut res = agent().post(&format!("{base}{path}")).header("content-type", "application/json").send(body.to_string()).map_err(|_| NetError::Offline)?;
    let text = res.body_mut().read_to_string().map_err(|_| NetError::Offline)?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|_| NetError::Server("server_error".into()))?;
    if v.get("ok").and_then(|x| x.as_bool()) == Some(true) {
        Ok(v)
    } else {
        Err(NetError::Server(v.get("error").and_then(|x| x.as_str()).unwrap_or("server_error").to_string()))
    }
}

fn to_app_error(e: NetError) -> AppError {
    match e {
        NetError::Offline => AppError::coded("lic_offline"),
        NetError::Server(code) => AppError::coded(&format!("lic_{code}")),
    }
}

/// Sessie en token uit het antwoord halen en controleren.
fn accept(state: &AppState, v: &serde_json::Value, remember: Option<bool>) -> AppResult<LicenseStatus> {
    let token = v.get("token").and_then(|x| x.as_str()).ok_or_else(|| AppError::coded("lic_server_error"))?;
    let session = v.get("session").and_then(|x| x.as_str()).map(|s| s.to_string());
    let mut rt = lock(&state.license);
    let payload = verify_token(token, &rt.device).ok_or_else(|| AppError::coded("lic_bad_token"))?;
    if let Some(r) = remember {
        rt.remember = r;
    }
    rt.store(session, token.to_string(), payload);
    Ok(rt.status())
}

// ---------- Voor de rest van de app ----------

/// Laadt een bewaarde sessie (Credential Manager) en controleert het token. Draait bij het starten.
pub fn init(state: &AppState) {
    let mut rt = lock(&state.license);
    rt.device = device_id();
    if !enabled() {
        return;
    }
    if let Some(saved) = cred::read(&rt.cred_target).and_then(|s| serde_json::from_str::<Saved>(&s).ok()) {
        if let Some(p) = verify_token(&saved.token, &rt.device) {
            rt.remember = true;
            rt.session = Some(saved.session);
            rt.token = Some(saved.token);
            // Na een herstart van de app: servertijd schatten met de klok van de pc.
            rt.received = Some(Instant::now() - Duration::from_millis((now_ms() - p.i).max(0) as u64));
            rt.payload = Some(p);
        } else {
            cred::delete(&rt.cred_target);
        }
    }
}

pub fn status(state: &AppState) -> LicenseStatus {
    lock(&state.license).status()
}

/// Mag een functie gebruikt worden? Zonder licentie (of als die uit staat bij het bouwen) niets blokkeren.
pub fn require(state: &AppState) -> AppResult<()> {
    if !enabled() {
        return Ok(());
    }
    if status(state).mode == "active" {
        Ok(())
    } else {
        Err(AppError::coded("license_locked"))
    }
}

pub fn config(state: &AppState) -> PublicConfig {
    match post("/config", serde_json::json!({})) {
        Ok(v) => {
            let mut c: PublicConfig = serde_json::from_value(v).unwrap_or_default();
            c.reachable = true;
            lock(&state.license).config = Some(c.clone());
            c
        }
        Err(_) => {
            let mut c = lock(&state.license).config.clone().unwrap_or_default();
            c.reachable = false;
            c
        }
    }
}

pub fn register(state: &AppState, username: &str, password: &str, key: &str, email: Option<&str>, remember: bool) -> AppResult<LicenseStatus> {
    let device = lock(&state.license).device.clone();
    let v = post("/register", serde_json::json!({ "username": username.trim(), "password": password, "key": key.trim(), "email": email.map(str::trim).filter(|e| !e.is_empty()), "device": device })).map_err(to_app_error)?;
    accept(state, &v, Some(remember))
}

pub fn login(state: &AppState, username: &str, password: &str, remember: bool) -> AppResult<LicenseStatus> {
    let device = lock(&state.license).device.clone();
    let v = post("/login", serde_json::json!({ "username": username.trim(), "password": password, "device": device })).map_err(to_app_error)?;
    accept(state, &v, Some(remember))
}

pub fn demo(state: &AppState) -> AppResult<LicenseStatus> {
    let device = lock(&state.license).device.clone();
    let v = post("/demo", serde_json::json!({ "device": device })).map_err(to_app_error)?;
    // Een demo onthouden we niet: hij werkt alleen online en duurt maar 2 uur.
    accept(state, &v, Some(false))
}

pub fn logout(state: &AppState) -> LicenseStatus {
    let session = lock(&state.license).session.clone();
    if let Some(s) = session {
        let _ = post("/logout", serde_json::json!({ "session": s }));
    }
    let mut rt = lock(&state.license);
    rt.clear(None);
    rt.status()
}

/// Nieuw token ophalen. Bij geen internet blijft het oude token gelden tot zijn controlemoment.
pub fn refresh(state: &AppState) -> AppResult<LicenseStatus> {
    let (session, device) = {
        let rt = lock(&state.license);
        (rt.session.clone(), rt.device.clone())
    };
    let Some(session) = session else { return Ok(status(state)) };
    match post("/refresh", serde_json::json!({ "session": session, "device": device })) {
        Ok(v) => accept(state, &v, None),
        Err(NetError::Offline) => Ok(status(state)),
        Err(NetError::Server(code)) => {
            // Sessie verlopen, ander apparaat of uitgelogd door een wachtwoordreset: opnieuw inloggen.
            if matches!(code.as_str(), "session_invalid" | "device_mismatch") {
                let mut rt = lock(&state.license);
                rt.clear(Some(&code));
                return Ok(rt.status());
            }
            Err(to_app_error(NetError::Server(code)))
        }
    }
}

/// Geen toegang meer (verlopen, ingetrokken, te lang offline of uitgelogd): alles terug naar de Windows-standaard.
/// Stopt Game Boost, zet de crosshair en de wifi-gamestand uit en zet elke gelogde wijziging terug.
/// Doet niets als de licentie uit staat bij het bouwen, of als er niets meer aan staat.
pub fn enforce(app: &AppHandle, state: &AppState) {
    if !enabled() || status(state).mode == "active" {
        return;
    }
    let boosting = lock(&state.boost).is_some();
    let crosshair = state.settings_clone().crosshair_enabled;
    let wifi = crate::tweaks::wifi_gaming::is_active();
    let changes = lock(&state.journal).has_any_active();
    if !boosting && !crosshair && !wifi && !changes {
        return;
    }
    if boosting {
        let _ = crate::boost::unboost(app, state);
    }
    if crosshair {
        let _ = crate::crosshair::set_enabled(app, state, false);
    }
    let result = reset_changes(state);
    log::info!("licentie niet geldig: {} wijzigingen teruggezet", result.reverted);
    let msg = crate::models::Msg::new("license_reverted").with("count", result.reverted);
    let _ = app.emit("notice", crate::models::Notice::new("warning", msg));
    crate::boost::emit_state(app, state);
}

/// Wifi-gamestand uit en elke gelogde wijziging terug. Los van de app, zodat de zelftest het ook kan.
pub fn reset_changes(state: &AppState) -> crate::models::RevertResult {
    crate::tweaks::wifi_gaming::stop();
    let result = lock(&state.journal).revert_all();
    lock(&state.priority_applied).clear();
    result
}

/// Zelftest: zonder licentie zet nezty een gelogde wijziging terug. Gebruikt een eigen testwaarde in HKCU.
pub fn selftest_enforce() -> serde_json::Value {
    use crate::journal::{JournalEntry, RevertPayload};
    let dir = std::env::temp_dir().join("nezty-selftest-enforce");
    let _ = std::fs::remove_dir_all(&dir);
    let state = AppState::new(dir, false);
    lock(&state.license).cred_target = "nezty/license-selftest".into();
    init(&state);
    let path = r"Software\nezty-selftest";
    let wrote = registry::write(Hive::Hkcu, path, "Probe", &RegData::Dword(1)).is_ok();
    let mut e = JournalEntry::new("selftest", "Selftest", crate::models::JournalKind::Registry, r"HKCU\Software\nezty-selftest\Probe");
    e.revertable = true;
    e.payload = RevertPayload::Registry { hive: Hive::Hkcu, path: path.into(), name: "Probe".into(), old: RegData::Absent, post: None };
    let recorded = lock(&state.journal).record(e).is_ok();
    let before = registry::read(Hive::Hkcu, path, "Probe").map(|v| v != RegData::Absent).unwrap_or(false);
    let mode = status(&state).mode;
    let reverted = if mode != "active" { reset_changes(&state).reverted } else { 0 };
    let after = registry::read(Hive::Hkcu, path, "Probe").map(|v| v != RegData::Absent).unwrap_or(false);
    let left = lock(&state.journal).has_any_active();
    let _ = crate::powershell::run_cmd("reg", &["delete", r"HKCU\Software\nezty-selftest", "/f"]);
    serde_json::json!({ "enabled": enabled(), "mode": mode, "wroteTestValue": wrote && recorded, "valueBefore": before, "reverted": reverted, "valueAfter": after, "stillActiveInLog": left })
}

/// Achtergrondcontrole: demo elke minuut, anders elke 10 minuten. Stuurt de stand naar de frontend.
pub fn start(app: AppHandle, state: std::sync::Arc<AppState>) {
    if !enabled() {
        return;
    }
    let _ = std::thread::Builder::new().name("license".into()).spawn(move || {
        let mut since_refresh = Duration::from_secs(0);
        let mut last_mode = String::new();
        let _ = refresh(&state);
        enforce(&app, &state);
        let _ = app.emit("license-state", status(&state));
        loop {
            std::thread::sleep(Duration::from_secs(15));
            since_refresh += Duration::from_secs(15);
            let st = status(&state);
            let every = if st.kind.as_deref() == Some("demo") || st.mode == "offline" { 60 } else { 600 };
            if since_refresh.as_secs() >= every {
                since_refresh = Duration::from_secs(0);
                let _ = refresh(&state);
            }
            enforce(&app, &state);
            let st = status(&state);
            // Alleen sturen als er iets verandert, plus elke minuut voor de klok.
            if st.mode != last_mode || since_refresh.as_secs() % 60 == 0 {
                last_mode = st.mode.clone();
                let _ = app.emit("license-state", st);
            }
        }
    });
}

/// Zelftest: alles behalve de server, met een tijdelijke plek in Credential Manager.
pub fn selftest_cred() -> serde_json::Value {
    let target = "nezty/license-selftest";
    let wrote = cred::write(target, "{\"session\":\"s\",\"token\":\"t\"}");
    let read = cred::read(target);
    cred::delete(target);
    let gone = cred::read(target).is_none();
    serde_json::json!({ "wrote": wrote, "readBack": read, "deleted": gone, "device": device_id(), "enabled": enabled() })
}

/// Zelftest tegen een (test)server: account maken, token controleren, verversen, uitloggen.
pub fn selftest_server(key: &str) -> serde_json::Value {
    let state = AppState::new(std::env::temp_dir().join("nezty-selftest-license"), false);
    lock(&state.license).cred_target = "nezty/license-selftest".into();
    init(&state);
    let user = format!("test{}", now_ms() % 100000);
    let reg = register(&state, &user, "geheim123", key, None, true).map(|s| serde_json::to_value(s).unwrap_or_default()).map_err(|e| e.to_string());
    let saved = cred::read("nezty/license-selftest").is_some();
    let refreshed = refresh(&state).map(|s| s.mode).map_err(|e| e.to_string());
    let required = require(&state).is_ok();
    let again = register(&state, "ander", "geheim123", key, None, false).err().map(|e| e.to_string());
    let after_logout = logout(&state).mode;
    let relog = login(&state, &user, "geheim123", false).map(|s| s.mode).map_err(|e| e.to_string());
    let wrong = login(&state, &user, "fout-wachtwoord", false).err().map(|e| e.to_string());
    let tampered = {
        let rt = lock(&state.license);
        rt.token.as_ref().map(|t| {
            let mut bad = t.clone();
            bad.insert(5, 'x');
            verify_token(&bad, &rt.device).is_none()
        })
    };
    cred::delete("nezty/license-selftest");
    serde_json::json!({ "register": reg, "savedInCredentialManager": saved, "refresh": refreshed, "requireOk": required, "keyAgain": again, "afterLogout": after_logout, "relogin": relog, "wrongPassword": wrong, "tamperedTokenRejected": tampered })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_garbage() {
        let key = VerifyingKey::from_bytes(&[0u8; 32]);
        if let Ok(k) = key {
            assert!(verify_with(&k, "v1.abc.def", "x").is_none());
        }
    }
}
