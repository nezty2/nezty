//! Licentie. De server beslist of een sleutel geldig is en hoeveel tijd er over is; de app krijgt daarvan
//! een token dat de server met Ed25519 ondertekent. De app controleert die handtekening met de publieke
//! sleutel en vertrouwt verder niets van zichzelf.
//!
//! - Adres van de server en de publieke sleutel komen bij het bouwen binnen (NEZTY_LICENSE_URL en
//!   NEZTY_LICENSE_PUBKEY). Zonder die twee staat de licentie uit en werkt de app zoals altijd.
//! - De sessie staat in Windows Credential Manager (alleen met "Ingelogd blijven"). Het laatste ondertekende
//!   token staat er altijd, onder een eigen naam (`nezty/license-last`), ook zonder "Ingelogd blijven". Dat token
//!   is geen sleutel tot de licentie (zonder sessie kan de app er niets mee bij de server), maar wel het bewijs van
//!   wat de server het laatst zei: status `s`, einddatum `e` en "controleren vóór" `c`.
//! - Zonder internet werkt een maandlicentie maximaal 24 uur door; een demo alleen online.
//! - "Alles terugzetten" en tweaks uitzetten werken altijd, ook zonder licentie.
//!
//! Wanneer zet nezty alles terug (`revert_reason`, gebruikt door `enforce` en `--license-check`)?
//!   1. De server zegt het: ingetrokken, verlopen of verwijderd (`license_deleted`). Ook als dat eerder al
//!      gezegd is en het terugzetten toen niet lukte (de vlag `gone` blijft bij het laatste token bewaard).
//!   2. Het nieuwste bekende token zegt het: status revoked of expired, of de einddatum is voorbij.
//!   3. Het nieuwste bekende token is te oud: "controleren vóór" is voorbij, dus al meer dan 24 uur (demo:
//!      5 minuten) geen bevestiging van de server. Dat geldt ook na uitloggen of zonder "Ingelogd blijven":
//!      zonder sessie kan nezty niet verversen, dus blijven tweaks dan nog tot dat moment staan.
//!   4. Er is helemaal geen token (bijvoorbeeld na een update vanaf 0.4.0 zonder "Ingelogd blijven", of als het
//!      uit Credential Manager is gehaald) terwijl er wijzigingen aan staan: 24 uur respijt vanaf het moment dat
//!      nezty dat merkt (`license_no_token_since` in settings.json), daarna hetzelfde als punt 3.
//!
//! Alleen uitgelogd zijn, of minder dan 24 uur offline, zet dus niets terug. Het inlogscherm blijft wel komen
//! (de stand voor de interface is `signed_out`), en nieuwe tweaks aanzetten kan alleen met een actieve licentie.
//! De controle draait bij het starten, daarna elke 15 minuten, en bij het inloggen in Windows: via de taak
//! "nezty autostart" (die start de app zelf) of, als starten met Windows uit staat, via de stille taak
//! "nezty licentiecontrole" (`--license-check`).

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
/// Laatste ondertekende token, ook zonder "Ingelogd blijven". Alleen voor de beslissing om terug te zetten.
const LAST_TARGET: &str = "nezty/license-last";
const DAY_MS: i64 = 24 * 60 * 60 * 1000;

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

/// Wat onder `LAST_TARGET` staat: het laatste token en of de server de licentie daarna verwijderd heeft.
#[derive(Serialize, Deserialize, Default)]
struct Last {
    #[serde(default)]
    token: Option<String>,
    #[serde(default)]
    gone: bool,
}

// ---------- Toestand ----------

pub struct Runtime {
    session: Option<String>,
    token: Option<String>,
    payload: Option<TokenPayload>,
    /// Moment (monotone klok) met de geschatte servertijd (ms) op dat moment, om de servertijd bij te houden.
    /// Een vast beginpunt plus de verstreken tijd: zo kan er nooit iets van de monotone klok af (die begint na een
    /// echte herstart van Windows bijna bij nul, en `Instant - Duration` breekt dan de app af).
    clock: Option<(Instant, i64)>,
    remember: bool,
    cred_target: String,
    device: String,
    /// Laatste reden waarom de sessie verviel (bv. "device_mismatch").
    reason: Option<String>,
    config: Option<PublicConfig>,
    /// De server kent deze licentie niet meer (verwijderd door de eigenaar): telt als ingetrokken.
    gone: bool,
    /// Nieuwste ondertekende token dat deze pc kreeg, ook na uitloggen of zonder "Ingelogd blijven".
    last: Option<TokenPayload>,
    last_token: Option<String>,
    /// De server zei na dat token dat de licentie verwijderd is. Blijft bewaard tot er een nieuw token komt.
    last_gone: bool,
    last_target: String,
}

impl Default for Runtime {
    fn default() -> Self {
        Runtime {
            session: None,
            token: None,
            payload: None,
            clock: None,
            remember: false,
            cred_target: DEFAULT_TARGET.into(),
            device: String::new(),
            reason: None,
            config: None,
            gone: false,
            last: None,
            last_token: None,
            last_gone: false,
            last_target: LAST_TARGET.into(),
        }
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
        match (&self.payload, self.clock) {
            (Some(_), Some((at, base))) => base + at.elapsed().as_millis() as i64,
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
        st.mode = if p.s == "revoked" || self.gone {
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
        self.last = Some(payload.clone());
        self.last_token = Some(token.clone());
        self.last_gone = false;
        self.token = Some(token);
        self.clock = Some((Instant::now(), payload.i));
        self.payload = Some(payload);
        self.reason = None;
        self.gone = false;
        self.persist();
        self.persist_last();
    }

    /// Het laatste token altijd bewaren, los van "Ingelogd blijven".
    fn persist_last(&self) {
        let blob = serde_json::to_string(&Last { token: self.last_token.clone(), gone: self.last_gone }).unwrap_or_default();
        if !cred::write(&self.last_target, &blob) {
            log::warn!("laatste licentietoken bewaren mislukt");
        }
    }

    /// Moet alles terug? Punt 1 tot en met 3 van de regel bovenaan; punt 4 (geen token) staat in `revert_reason`.
    fn verdict(&self) -> Option<&'static str> {
        if !enabled() {
            return None;
        }
        if self.gone || self.last_gone {
            return Some("revoked");
        }
        let p = self.last.as_ref()?;
        let now = self.server_now();
        if p.s == "revoked" {
            Some("revoked")
        } else if p.s == "expired" || p.e.is_some_and(|e| now >= e) {
            Some("expired")
        } else if now > p.c {
            Some("offline")
        } else {
            None
        }
    }

    fn has_any_token(&self) -> bool {
        self.last.is_some() || self.payload.is_some() || self.gone || self.last_gone
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

    /// Uitloggen of sessie verlopen. Het laatste token blijft bewaard: het bepaalt hoe lang tweaks nog mogen blijven.
    fn clear(&mut self, reason: Option<&str>) {
        self.session = None;
        self.token = None;
        self.payload = None;
        self.clock = None;
        self.gone = false;
        self.reason = reason.map(|s| s.to_string());
        cred::delete(&self.cred_target);
    }

    /// Verwijderd op de server: sessie en bewaard token weg, maar de gegevens blijven zichtbaar als "ingetrokken".
    /// Na een herstart van de app komt het inlogscherm. Dat de server dit zei, blijft bij het laatste token staan,
    /// zodat een volgende start of de controle bij het inloggen alsnog terugzet als dat nu niet lukte.
    fn mark_gone(&mut self) {
        self.session = None;
        self.token = None;
        self.gone = true;
        self.last_gone = true;
        self.reason = Some("license_deleted".into());
        cred::delete(&self.cred_target);
        self.persist_last();
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
    let st = rt.status();
    drop(rt);
    // Er is weer een token: een eventueel respijt zonder token is voorbij.
    let had_grace = lock(&state.settings).license_no_token_since.take().is_some();
    if had_grace {
        state.save_settings();
    }
    Ok(st)
}

// ---------- Voor de rest van de app ----------

/// Laadt een bewaarde sessie (Credential Manager) en controleert het token. Draait bij het starten.
pub fn init(state: &AppState) {
    let mut rt = lock(&state.license);
    rt.device = device_id();
    if !enabled() {
        return;
    }
    let device = rt.device.clone();
    if let Some(last) = cred::read(&rt.last_target).and_then(|s| serde_json::from_str::<Last>(&s).ok()) {
        rt.last_gone = last.gone;
        if let Some(token) = last.token {
            if let Some(p) = verify_token(&token, &device) {
                rt.last = Some(p);
                rt.last_token = Some(token);
            }
        }
    }
    if let Some(saved) = cred::read(&rt.cred_target).and_then(|s| serde_json::from_str::<Saved>(&s).ok()) {
        if let Some(p) = verify_token(&saved.token, &rt.device) {
            rt.remember = true;
            // Het sessietoken is nieuwer dan het laatst bewaarde (of dat ontbreekt, zoals na een update vanaf 0.4.0).
            if rt.last.as_ref().map_or(true, |l| p.i > l.i) {
                rt.last = Some(p.clone());
                rt.last_token = Some(saved.token.clone());
                rt.last_gone = false;
                rt.persist_last();
            }
            rt.session = Some(saved.session);
            rt.token = Some(saved.token);
            // Na een herstart van de app: servertijd schatten met de klok van de pc (nooit vóór het token).
            rt.clock = Some((Instant::now(), now_ms().max(p.i)));
            rt.payload = Some(p);
        } else {
            cred::delete(&rt.cred_target);
        }
    }
}

/// Waarom alles terug moet ("revoked", "expired" of "offline"), of None als de wijzigingen mogen blijven.
/// De volledige regel staat bovenaan dit bestand.
pub fn revert_reason(state: &AppState) -> Option<&'static str> {
    if !enabled() {
        return None;
    }
    let (verdict, has_token) = {
        let rt = lock(&state.license);
        (rt.verdict(), rt.has_any_token())
    };
    if has_token {
        // Er is weer een token: het respijt zonder token begint de volgende keer opnieuw.
        let had_grace = {
            let mut s = lock(&state.settings);
            s.license_no_token_since.take().is_some()
        };
        if had_grace {
            state.save_settings();
        }
        return verdict;
    }
    // Geen enkel token: 24 uur respijt vanaf het moment dat nezty dat merkt.
    let now = now_ms();
    let since = {
        let mut s = lock(&state.settings);
        match s.license_no_token_since {
            Some(t) => Some(t),
            None => {
                s.license_no_token_since = Some(now);
                None
            }
        }
    };
    match since {
        Some(t) => (now - t > DAY_MS).then_some("offline"),
        None => {
            state.save_settings();
            None
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
    // Het licentie-id gaat mee: zo kan de server zeggen of de sleutel verwijderd is (en niet zomaar de sessie weg).
    let license_id = lock(&state.license).payload.as_ref().map(|p| p.l.clone());
    match post("/refresh", serde_json::json!({ "session": session, "device": device, "license": license_id })) {
        Ok(v) => accept(state, &v, None),
        Err(NetError::Offline) => Ok(status(state)),
        Err(NetError::Server(code)) => {
            let mut rt = lock(&state.license);
            // Verwijderd door de eigenaar: hetzelfde als ingetrokken.
            if code == "license_deleted" {
                rt.mark_gone();
                return Ok(rt.status());
            }
            // Sessie verlopen, ander apparaat of uitgelogd door een wachtwoordreset: opnieuw inloggen.
            if matches!(code.as_str(), "session_invalid" | "device_mismatch") {
                rt.clear(Some(&code));
                return Ok(rt.status());
            }
            Err(to_app_error(NetError::Server(code)))
        }
    }
}

/// Geen toegang meer (verlopen, ingetrokken, verwijderd of 24 uur zonder bevestiging, zie `revert_reason`): alles
/// terug naar de Windows-standaard. Stopt Game Boost, zet de crosshair en de wifi-gamestand uit en zet elke gelogde
/// wijziging terug. Doet niets als de licentie uit staat bij het bouwen, of als er niets meer aan staat.
/// Alleen uitgelogd zijn zet niets terug.
pub fn enforce(app: &AppHandle, state: &AppState) {
    if !enabled() || status(state).mode == "active" {
        return;
    }
    // Uitgelogd maar geen uitspraak van de server: Game Boost en de crosshair stoppen wel (die horen bij een
    // actieve licentie), de tweaks blijven staan.
    let verdict = revert_reason(state);
    let boosting = lock(&state.boost).is_some();
    let crosshair = state.settings_clone().crosshair_enabled;
    let wifi = crate::tweaks::wifi_gaming::is_active();
    let changes = lock(&state.journal).has_any_active();
    if boosting {
        let _ = crate::boost::unboost(app, state);
    }
    if crosshair {
        let _ = crate::crosshair::set_enabled(app, state, false);
    }
    if verdict.is_none() || (!wifi && !changes) {
        if boosting {
            crate::boost::emit_state(app, state);
        }
        return;
    }
    let result = reset_changes(state);
    log::info!("licentie niet geldig: {} wijzigingen teruggezet", result.reverted);
    let msg = crate::models::Msg::new("license_reverted").with("count", result.reverted);
    let _ = app.emit("notice", crate::models::Notice::new("warning", msg));
    crate::boost::emit_state(app, state);
}

/// Hoe vaak (elke 15 seconden) nezty bij het opstarten opnieuw de server vraagt voordat "offline" telt: 4 minuten.
const BOOT_RETRIES: u32 = 16;

/// Net na het inloggen op Windows is het netwerk (vooral wifi) vaak nog niet verbonden. Is "offline" de enige reden
/// om terug te zetten en is er een bewaarde sessie, dan eerst een paar minuten opnieuw proberen. Pas als de server
/// al die tijd onbereikbaar blijft, geldt de uitspraak. Ingetrokken, verlopen en verwijderd gelden meteen.
fn retry_while_offline(state: &AppState) {
    for n in 0..BOOT_RETRIES {
        let waiting = {
            let rt = lock(&state.license);
            rt.session.is_some() && rt.verdict() == Some("offline")
        };
        if !waiting {
            return;
        }
        if n == 0 {
            log::info!("licentie: server nog niet bereikbaar, opnieuw proberen voordat er iets teruggaat");
        }
        std::thread::sleep(Duration::from_secs(15));
        let _ = refresh(state);
    }
}

/// Wifi-gamestand uit en elke gelogde wijziging terug. Los van de app, zodat de zelftest het ook kan.
pub fn reset_changes(state: &AppState) -> crate::models::RevertResult {
    crate::tweaks::wifi_gaming::stop();
    let result = lock(&state.journal).revert_all();
    crate::activate::forget(state);
    result
}

/// Stille controle zonder venster (`nezty.exe --license-check`, de geplande taak bij het inloggen als starten met
/// Windows uit staat). Laadt de bewaarde tokens, vraagt de server (alleen met een bewaarde sessie) en zet alles
/// terug volgens `revert_reason`. Alleen uitgelogd zijn of "Ingelogd blijven" uit zet niets terug.
pub fn check_headless(state: &AppState) -> serde_json::Value {
    init(state);
    if !enabled() {
        return serde_json::json!({ "enabled": false });
    }
    let _ = refresh(state);
    retry_while_offline(state);
    let st = status(state);
    let pending = lock(&state.journal).has_any_active();
    let verdict = revert_reason(state);
    let reverted = if verdict.is_some() && pending { reset_changes(state).reverted } else { 0 };
    log::info!("licentiecontrole bij inloggen: {} ({}), uitspraak {:?}, {} wijzigingen teruggezet", st.mode, st.reason.clone().unwrap_or_default(), verdict, reverted);
    serde_json::json!({ "enabled": true, "mode": st.mode, "reason": st.reason, "verdict": verdict, "pendingChanges": pending, "reverted": reverted })
}

/// Bij het verwijderen van de app: sessie op de server sluiten en het bewaarde token weghalen.
pub fn forget(state: &AppState) {
    let session = lock(&state.license).session.clone();
    if let Some(s) = session {
        let _ = post("/logout", serde_json::json!({ "session": s }));
    }
    lock(&state.license).clear(None);
    cred::delete(DEFAULT_TARGET);
    cred::delete(LAST_TARGET);
}

/// Zelftest: zonder token blijft een gelogde wijziging binnen het respijt staan en gaat ze daarna terug.
/// Gebruikt een eigen testwaarde in HKCU en eigen plekken in Credential Manager.
#[cfg(feature = "selftest")]
pub fn selftest_enforce() -> serde_json::Value {
    use crate::journal::{JournalEntry, RevertPayload};
    let dir = std::env::temp_dir().join("nezty-selftest-enforce");
    let _ = std::fs::remove_dir_all(&dir);
    let state = AppState::new(dir, false);
    {
        let mut rt = lock(&state.license);
        rt.cred_target = "nezty/license-selftest".into();
        rt.last_target = "nezty/license-last-selftest".into();
    }
    init(&state);
    let path = r"Software\nezty-selftest";
    let wrote = registry::write(Hive::Hkcu, path, "Probe", &RegData::Dword(1)).is_ok();
    let mut e = JournalEntry::new("selftest", "Selftest", crate::models::JournalKind::Registry, r"HKCU\Software\nezty-selftest\Probe");
    e.revertable = true;
    e.payload = RevertPayload::Registry { hive: Hive::Hkcu, path: path.into(), name: "Probe".into(), old: RegData::Absent, post: None };
    let recorded = lock(&state.journal).record(e).is_ok();
    let before = registry::read(Hive::Hkcu, path, "Probe").map(|v| v != RegData::Absent).unwrap_or(false);
    let mode = status(&state).mode;
    // Eerste keer zonder token: respijt begint, er gaat niets terug.
    let within_grace = revert_reason(&state);
    // Respijt 25 uur geleden begonnen: nu moet alles terug.
    lock(&state.settings).license_no_token_since = Some(now_ms() - 25 * 60 * 60 * 1000);
    let after_grace = revert_reason(&state);
    let reverted = if after_grace.is_some() { reset_changes(&state).reverted } else { 0 };
    let after = registry::read(Hive::Hkcu, path, "Probe").map(|v| v != RegData::Absent).unwrap_or(false);
    let left = lock(&state.journal).has_any_active();
    let _ = registry::write(Hive::Hkcu, path, "Probe", &RegData::Absent);
    let _ = Hive::Hkcu.root().delete_subkey(path);
    serde_json::json!({ "enabled": enabled(), "mode": mode, "wroteTestValue": wrote && recorded, "valueBefore": before, "verdictWithinGrace": within_grace, "verdictAfterGrace": after_grace, "reverted": reverted, "valueAfter": after, "stillActiveInLog": left })
}

/// Achtergrondcontrole: demo elke minuut, anders elke 15 minuten. Stuurt de stand naar de frontend.
pub fn start(app: AppHandle, state: std::sync::Arc<AppState>) {
    if !enabled() {
        return;
    }
    let _ = std::thread::Builder::new().name("license".into()).spawn(move || {
        let mut since_refresh = Duration::from_secs(0);
        let mut last_mode = String::new();
        let _ = refresh(&state);
        if lock(&state.license).verdict() == Some("offline") {
            // Eerst de stand laten zien, dan pas (stil) opnieuw proberen.
            let _ = app.emit("license-state", status(&state));
            retry_while_offline(&state);
        }
        enforce(&app, &state);
        let _ = app.emit("license-state", status(&state));
        loop {
            std::thread::sleep(Duration::from_secs(15));
            since_refresh += Duration::from_secs(15);
            let st = status(&state);
            let every = if st.kind.as_deref() == Some("demo") || st.mode == "offline" { 60 } else { 900 };
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
#[cfg(feature = "selftest")]
pub fn selftest_cred() -> serde_json::Value {
    let target = "nezty/license-selftest";
    let wrote = cred::write(target, "{\"session\":\"s\",\"token\":\"t\"}");
    let read = cred::read(target);
    cred::delete(target);
    let gone = cred::read(target).is_none();
    serde_json::json!({ "wrote": wrote, "readBack": read, "deleted": gone, "device": device_id(), "enabled": enabled() })
}

/// Zelftest tegen een (test)server: account maken, token controleren, verversen, uitloggen.
#[cfg(feature = "selftest")]
pub fn selftest_server(key: &str) -> serde_json::Value {
    let state = AppState::new(std::env::temp_dir().join("nezty-selftest-license"), false);
    {
        let mut rt = lock(&state.license);
        rt.cred_target = "nezty/license-selftest".into();
        rt.last_target = "nezty/license-last-selftest".into();
    }
    init(&state);
    let user = format!("test{}", now_ms() % 100000);
    let reg = register(&state, &user, "geheim123", key, None, true).map(|s| serde_json::to_value(s).unwrap_or_default()).map_err(|e| e.to_string());
    let saved = cred::read("nezty/license-selftest").is_some();
    let refreshed = refresh(&state).map(|s| s.mode).map_err(|e| e.to_string());
    let required = require(&state).is_ok();
    let again = register(&state, "ander", "geheim123", key, None, false).err().map(|e| e.to_string());
    let after_logout = logout(&state).mode;
    // Na uitloggen blijft het laatste token bewaard en zegt het nog niet "alles terug".
    let last_kept = cred::read("nezty/license-last-selftest").is_some();
    let verdict_after_logout = revert_reason(&state);
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
    cred::delete("nezty/license-last-selftest");
    serde_json::json!({ "register": reg, "savedInCredentialManager": saved, "refresh": refreshed, "requireOk": required, "keyAgain": again, "afterLogout": after_logout, "lastTokenKeptAfterLogout": last_kept, "verdictAfterLogout": verdict_after_logout, "relogin": relog, "wrongPassword": wrong, "tamperedTokenRejected": tampered })
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
