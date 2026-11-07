//! Het logboek: elke wijziging wordt met de oude waarde opgeslagen, zodat "Alles terugzetten"
//! alles exact kan herstellen. Wordt als JSON in de app-datamap bewaard.

use crate::error::{AppError, AppResult};
use crate::models::{now_ms, JournalKind, Msg, RevertFailure, RevertResult};
use crate::powershell::{ps_quote, run_cmd, run_ps};
use crate::registry::{self, Hive, RegData};
use crate::{display, winapi};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::os::windows::fs::OpenOptionsExt;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PostAction {
    /// Na het terugzetten van de muiswaarden ook SystemParametersInfo(SPI_SETMOUSE) aanroepen.
    MouseSpi,
    /// Na het terugzetten van visuele effecten de UI-effecten weer aanzetten.
    UiEffects,
    /// Toetsenbordvertraging en -snelheid opnieuw toepassen vanuit het register.
    KeyboardSpi,
    /// Plaktoetsen, filtertoetsen en wisseltoetsen opnieuw toepassen vanuit het register.
    AccessibilitySpi,
    /// Sneltoetsen voor taal- en indelingswissel opnieuw inlezen.
    LangToggleSpi,
    /// Windows-energiemodus (overlay) opnieuw toepassen vanuit het register.
    PowerOverlay,
    /// Vensteranimaties en transparantie direct opnieuw toepassen vanuit het register.
    SnappyUi,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RevertPayload {
    Registry {
        hive: Hive,
        path: String,
        name: String,
        old: RegData,
        #[serde(default)]
        post: Option<PostAction>,
    },
    PowerPlan {
        old_guid: String,
        created_guid: Option<String>,
        /// Bij een zelf gemaakte kopie: "high" of "ultimate". Zo herkent nezty de kopie later terug.
        #[serde(default)]
        created_option: Option<String>,
    },
    PowerSetting {
        scheme: String,
        sub: String,
        setting: String,
        ac: u32,
        dc: u32,
    },
    DisplayMode {
        device: String,
        width: u32,
        height: u32,
        freq: u32,
    },
    File {
        path: String,
        backup_path: String,
    },
    Adapter {
        name: String,
        old_value: String,
    },
    Priority {
        pid: u32,
        old_class: u32,
    },
    /// Het vinkje "De computer mag dit apparaat uitschakelen" van één apparaat (WMI MSPower_DeviceEnable).
    DevicePower {
        instance: String,
        enable: bool,
    },
    /// Wifi-gamestand van nezty (achtergrondzoektocht uit zolang nezty draait).
    WifiGaming,
    /// Touchpad-instelling "aan laten als er een muis is".
    TouchpadMouse {
        allow: bool,
    },
    /// Eén sleutel in een ini-bestand. `old` leeg: de sleutel stond er niet en wordt weer weggehaald.
    IniKey {
        path: String,
        section: String,
        key: String,
        old: Option<String>,
    },
    /// Eén waarde in een JSON-bestand (JSON-pointer). `old` is de oude waarde als JSON-tekst, leeg als hij ontbrak.
    JsonKey {
        path: String,
        pointer: String,
        old: Option<String>,
        #[serde(default)]
        pretty: bool,
    },
    /// Eén sleutel in een eenvoudig tekstbestand van een game (sleutel:waarde of "sleutel" "waarde").
    TextKey {
        path: String,
        format: String,
        key: String,
        old: String,
    },
    /// Een geavanceerde eigenschap van een netwerkkaart (bv. *InterruptModeration).
    AdapterProperty {
        name: String,
        keyword: String,
        old_value: String,
    },
    /// Een QoS-regel van Windows die nezty heeft gemaakt (voorrang voor gameverkeer): terugzetten = weghalen.
    QosPolicy {
        name: String,
    },
    /// Cloudflare WARP die nezty na de routetest aan liet: terugzetten = verbinding verbreken.
    Warp,
    /// Interfacemetric van een internetverbinding (Multi-Internet), met de oude waarde.
    InterfaceMetric {
        luid: u64,
        family: u16,
        automatic: bool,
        metric: u32,
    },
    /// Een DWORD-instelling in het globale NVIDIA-driverprofiel (NVAPI DRS). `old` None = terug naar driver-standaard.
    NvDrsDword {
        setting_id: u32,
        old: Option<u32>,
    },
    None,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JournalEntry {
    pub id: String,
    pub ts: i64,
    pub tweak_id: String,
    pub tweak_name: String,
    pub kind: JournalKind,
    pub target: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub reverted: bool,
    pub revertable: bool,
    pub session_id: Option<String>,
    pub game_id: Option<String>,
    /// Vertaalbare notitie-sleutel (bv. "manual_off"); oudere items bevatten vrije tekst.
    pub note: Option<String>,
    /// Gestructureerde omschrijving voor de frontend (vertaald).
    #[serde(default)]
    pub detail: Option<Msg>,
    #[serde(default = "default_payload")]
    pub payload: RevertPayload,
    /// Er staat een vluchtige registersleutel voor deze wijziging (zie restart.rs): na een echte herstart is die weg.
    #[serde(default)]
    pub restart_marker: bool,
}

fn default_payload() -> RevertPayload {
    RevertPayload::None
}

impl JournalEntry {
    pub fn new(tweak_id: &str, tweak_name: &str, kind: JournalKind, target: &str) -> Self {
        JournalEntry {
            id: uuid::Uuid::new_v4().to_string(),
            ts: now_ms(),
            tweak_id: tweak_id.to_string(),
            tweak_name: tweak_name.to_string(),
            kind,
            target: target.to_string(),
            old_value: None,
            new_value: None,
            reverted: false,
            revertable: false,
            session_id: None,
            game_id: None,
            note: None,
            detail: None,
            payload: RevertPayload::None,
            restart_marker: false,
        }
    }
}

pub struct Journal {
    path: PathBuf,
    _writer: Option<std::fs::File>,
    _installation: Option<std::fs::File>,
    pub entries: Vec<JournalEntry>,
    /// Regels die deze versie niet kan lezen (bv. van een nieuwere versie). Ze blijven ongewijzigd
    /// in het bestand staan, zodat er nooit oude waarden voor "Alles terugzetten" verloren gaan.
    unknown: Vec<serde_json::Value>,
    /// Kopie van een logboek dat (deels) niet te lezen was, als die gemaakt is.
    pub backup: Option<PathBuf>,
    /// Onleesbaar en geen kopie gelukt: dan niets meer wegschrijven.
    blocked: bool,
}

/// Uitkomst van het lezen van het logboekbestand.
enum Parsed {
    Missing,
    Ok(Vec<JournalEntry>, Vec<serde_json::Value>),
    Unreadable,
}

fn parse(text: &str) -> Parsed {
    let Ok(values) = serde_json::from_str::<Vec<serde_json::Value>>(text) else {
        return Parsed::Unreadable;
    };
    let mut entries = Vec::new();
    let mut unknown = Vec::new();
    for v in values {
        match serde_json::from_value::<JournalEntry>(v.clone()) {
            Ok(e) => entries.push(e),
            Err(_) => unknown.push(v),
        }
    }
    Parsed::Ok(entries, unknown)
}

impl Journal {
    /// Only the elevated process may supply executable recovery records. Keep a
    /// protected snapshot for review when moving from the old AppData format.
    pub fn open(legacy: &Path, uninstalling: bool) -> AppResult<Self> {
        let dir = crate::protected::recovery_dir()?;
        let machine = dir.parent().ok_or_else(|| AppError::msg("Missing recovery directory"))?.join("installation.lock");
        if !machine.try_exists()? {
            match std::fs::File::create_new(&machine) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e.into()),
            }
        }
        let share = if uninstalling { 0 } else { windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ };
        let installation = std::fs::OpenOptions::new().read(true).share_mode(share).open(machine)
            .map_err(|e| AppError::msg(format!("Sluit andere Nezty-sessies en probeer het opnieuw ({e}).")))?;
        let writer = std::fs::OpenOptions::new().write(true).create(true).truncate(false)
            .share_mode(0).open(dir.join("writer.lock"))?;
        let path = dir.join("journal.json");
        if !path.try_exists()? {
            Self::prepare_legacy(&legacy.join("journal.json"), &dir)?;
        }
        crate::protected::check(&path)?;
        let mut journal = Self::load(&dir);
        if journal.blocked { return Err(AppError::msg(format!("Onleesbaar hersteljournaal: {}", path.display()))); }
        journal._writer = Some(writer);
        journal._installation = Some(installation);
        Ok(journal)
    }

    fn prepare_legacy(source: &Path, dir: &Path) -> AppResult<()> {
        let review = dir.join("legacy-review.json");
        if !review.try_exists()? {
            let bytes = match std::fs::read(source) {
                Ok(bytes) => bytes,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => b"[]".to_vec(),
                Err(e) => return Err(e.into()),
            };
            let inactive = std::str::from_utf8(&bytes).ok().is_some_and(|text|
                matches!(parse(text), Parsed::Ok(entries, unknown) if unknown.is_empty() &&
                    entries.iter().all(|e| e.reverted || !e.revertable)));
            if inactive {
                crate::state::write_atomic(&dir.join("journal.json"), &bytes)?;
                return Ok(());
            }
            crate::state::write_atomic(&review, &bytes)?;
        }
        Err(AppError::msg(format!("Controleer als beheerder de oude herstelgegevens in {}. Hernoem de gecontroleerde kopie naar journal.json en start Nezty opnieuw.", review.display())))
    }

    /// Leest het logboek. Is (een deel) niet te lezen, dan komt er eerst een kopie naast
    /// (journal.unreadable-<tijd>.json). Een onleesbaar logboek wordt nooit stil overschreven.
    pub fn load(dir: &Path) -> Journal {
        let path = dir.join("journal.json");
        let parsed = match std::fs::read_to_string(&path) {
            Ok(text) => parse(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Parsed::Missing,
            Err(_) => Parsed::Unreadable,
        };
        let mut j = Journal { path, entries: Vec::new(), unknown: Vec::new(), backup: None, blocked: false, _writer: None, _installation: None };
        match parsed {
            Parsed::Missing => {}
            Parsed::Ok(entries, unknown) => {
                j.entries = entries;
                if !unknown.is_empty() {
                    log::warn!("logboek: {} regel(s) niet leesbaar, ze blijven bewaard", unknown.len());
                    j.unknown = unknown;
                    j.backup = j.make_backup();
                }
            }
            Parsed::Unreadable => {
                log::error!("logboek niet leesbaar: kopie gemaakt voordat er iets wordt overschreven");
                j.backup = j.make_backup();
                j.blocked = true;
            }
        }
        j
    }

    fn make_backup(&self) -> Option<PathBuf> {
        let target = self.path.with_file_name(format!("journal.unreadable-{}.json", now_ms()));
        match std::fs::copy(&self.path, &target) {
            Ok(_) => Some(target),
            Err(e) => {
                log::error!("kopie van logboek mislukt: {e}");
                None
            }
        }
    }

    pub fn save(&self) -> AppResult<()> {
        if self.blocked {
            return Err(AppError::coded("journal_unreadable"));
        }
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut all: Vec<serde_json::Value> = self.unknown.clone();
        for e in &self.entries {
            all.push(serde_json::to_value(e)?);
        }
        crate::state::write_atomic(&self.path, serde_json::to_string_pretty(&all)?.as_bytes())?;
        Ok(())
    }

    pub fn record(&mut self, entry: JournalEntry) -> AppResult<()> {
        self.entries.push(entry);
        if let Err(e) = self.save() {
            self.entries.pop();
            return Err(e);
        }
        Ok(())
    }

    /// Keep the journal lock across comparison, durable recording and mutation.
    /// If Windows fails partway, the saved inverse remains available for retry.
    pub fn write_registry(&mut self, entry: JournalEntry, new: &registry::RegData) -> AppResult<()> {
        let RevertPayload::Registry { hive, path, name, old, .. } = &entry.payload else {
            return Err(AppError::msg("Missing registry recovery data"));
        };
        if !registry::matches_exact(*hive, path, name, old)? {
            return Err(AppError::msg("De oorspronkelijke registerwaarde kan niet exact worden teruggezet of is ondertussen gewijzigd."));
        }
        let (hive, path, name, old) = (*hive, path.clone(), name.clone(), old.clone());
        self.record(entry)?;
        if let Err(e) = registry::write(hive, &path, &name, new) {
            // A definite no-op does not need future recovery. If saving this
            // status also fails, leave the durable inverse eligible for retry.
            if registry::matches_exact(hive, &path, &name, &old).unwrap_or(false) {
                let index = self.entries.len() - 1;
                self.entries[index].reverted = true;
                if self.save().is_err() { self.entries[index].reverted = false; }
            }
            return Err(e);
        }
        Ok(())
    }

    pub fn write_power(&mut self, entry: JournalEntry, new_ac: u32, new_dc: u32) -> AppResult<()> {
        let RevertPayload::PowerSetting { scheme, sub, setting, ac, dc } = &entry.payload else {
            return Err(AppError::msg("Missing power recovery data"));
        };
        if crate::tweaks::input::read_power_setting(scheme, sub, setting)? != Some((*ac, *dc)) {
            return Err(AppError::msg("De energie-instelling is ondertussen gewijzigd."));
        }
        let (scheme, sub, setting) = (scheme.clone(), sub.clone(), setting.clone());
        self.record(entry)?;
        crate::tweaks::input::write_power_setting(&scheme, &sub, &setting, new_ac, new_dc)
    }

    /// Actieve (niet teruggezette) wijzigingen van een tweak, eventueel voor één game.
    pub fn active_ids(&self, tweak_id: &str, game_id: Option<&str>) -> Vec<String> {
        self.entries
            .iter()
            .filter(|e| e.tweak_id == tweak_id && !e.reverted && e.revertable)
            .filter(|e| match game_id {
                Some(g) => e.game_id.as_deref() == Some(g),
                None => true,
            })
            .map(|e| e.id.clone())
            .collect()
    }

    /// Tijdstip (ms) van de laatste nog geldende wijziging voor deze tweak.
    pub fn latest_active_ts(&self, tweak_id: &str, game_id: Option<&str>) -> Option<i64> {
        self.entries
            .iter()
            .filter(|e| e.tweak_id == tweak_id && !e.reverted && e.revertable)
            .filter(|e| match game_id {
                Some(g) => e.game_id.as_deref() == Some(g),
                None => true,
            })
            .map(|e| e.ts)
            .max()
    }

    /// De laatste nog geldende wijziging voor deze tweak.
    pub fn latest_active(&self, tweak_id: &str, game_id: Option<&str>) -> Option<&JournalEntry> {
        self.entries
            .iter()
            .filter(|e| e.tweak_id == tweak_id && !e.reverted && e.revertable)
            .filter(|e| match game_id {
                Some(g) => e.game_id.as_deref() == Some(g),
                None => true,
            })
            .max_by_key(|e| e.ts)
    }

    /// Markeert wijzigingen als teruggezet zonder iets uit te voeren (voor wat vanzelf al weg is).
    pub fn mark_reverted(&mut self, ids: &[String]) {
        if ids.is_empty() {
            return;
        }
        for e in self.entries.iter_mut().filter(|e| ids.contains(&e.id)) {
            e.reverted = true;
        }
        if let Err(e) = self.save() {
            log::error!("logboek opslaan mislukt: {e}");
        }
    }

    pub fn has_active(&self, tweak_id: &str, game_id: Option<&str>) -> bool {
        !self.active_ids(tweak_id, game_id).is_empty()
    }

    /// Maakt de nog geldende wijzigingen van een tweak (eventueel voor één game) los van hun Game Boost-sessie,
    /// zodat het stoppen van de boost ze niet terugzet. True als er iets losgemaakt is.
    pub fn detach_from_session(&mut self, tweak_id: &str, game_id: Option<&str>) -> bool {
        let ids = self.active_ids(tweak_id, game_id);
        let mut changed = false;
        for e in self.entries.iter_mut().filter(|e| ids.contains(&e.id) && e.session_id.is_some()) {
            e.session_id = None;
            changed = true;
        }
        if changed {
            if let Err(e) = self.save() {
                log::error!("logboek opslaan mislukt: {e}");
            }
        }
        changed
    }

    pub fn session_ids(&self, session_id: &str) -> Vec<String> {
        self.entries
            .iter()
            .filter(|e| e.session_id.as_deref() == Some(session_id) && !e.reverted)
            .map(|e| e.id.clone())
            .collect()
    }

    /// Zet de gegeven wijzigingen terug, nieuwste eerst.
    pub fn revert_ids(&mut self, ids: &[String]) -> RevertResult {
        let mut result = RevertResult::default();
        let mut order: Vec<usize> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| ids.contains(&e.id) && !e.reverted)
            .map(|(i, _)| i)
            .collect();
        order.reverse(); // The append order survives clock adjustments and equal timestamps.
        let mut failed_targets = std::collections::HashSet::new();
        for i in order {
            let entry = &self.entries[i];
            if !entry.revertable || failed_targets.contains(&entry.target) {
                result.skipped += 1;
                continue;
            }
            match execute_revert(&entry.payload) {
                Ok(()) => {
                    self.entries[i].reverted = true;
                    if let Err(e) = self.save() {
                        self.entries[i].reverted = false;
                        result.failed.push(RevertFailure { id: self.entries[i].id.clone(), target: self.path.display().to_string(), error: e.to_string() });
                        break;
                    }
                    result.reverted += 1;
                }
                Err(e) => {
                    failed_targets.insert(entry.target.clone());
                    result.failed.push(RevertFailure { id: entry.id.clone(), target: entry.target.clone(), error: e.to_string() });
                }
            }
        }
        result
    }

    /// Staat er nog iets aan dat terug te zetten is?
    pub fn has_any_active(&self) -> bool {
        self.entries.iter().any(|e| !e.reverted && e.revertable)
    }

    /// A machine-wide uninstall must leave the app available to other users
    /// who still need it to undo changes in their own Windows accounts.
    pub fn check_other_users(&self) -> AppResult<()> {
        let own = self.path.parent().ok_or_else(|| AppError::msg("Missing journal directory"))?;
        let root = own.parent().ok_or_else(|| AppError::msg("Missing recovery directory"))?;
        for entry in std::fs::read_dir(root)? {
            let dir = entry?.path();
            if dir == own || dir == root.join("installation.lock") { continue; }
            crate::protected::check(&dir)?;
            if !dir.is_dir() { continue; }
            let path = dir.join("journal.json");
            let complete = if path.try_exists()? {
                matches!(parse(&std::fs::read_to_string(path)?), Parsed::Ok(entries, unknown)
                    if unknown.is_empty() && entries.iter().all(|e| e.reverted || !e.revertable))
            } else { !dir.join("legacy-review.json").try_exists()? };
            if !complete { return Err(AppError::msg(format!("Open Nezty in het andere Windows-account om eerst de wijzigingen terug te zetten: {}", dir.display()))); }
        }
        Ok(())
    }

    pub fn revert_all(&mut self) -> RevertResult {
        let ids: Vec<String> = self.entries.iter().filter(|e| !e.reverted).map(|e| e.id.clone()).collect();
        let mut result = self.revert_ids(&ids);
        if self.blocked || !self.unknown.is_empty() {
            result.failed.push(RevertFailure { id: String::new(), target: self.path.display().to_string(), error: "Onleesbare herstelgegevens".into() });
        }
        result
    }
}

fn adapter_restore_command(name: &str, old_value: &str) -> AppResult<String> {
    let enable = match old_value.to_ascii_lowercase().as_str() {
        "enabled" => "$true",
        "disabled" => "$false",
        _ => return Err(AppError::msg("Unknown adapter power management state")),
    };
    Ok(format!("& {{ {} }} -Name {} -Enable {enable}", include_str!("restore_adapter.ps1"), ps_quote(name)))
}

/// Voert één terugzet-actie uit.
pub fn execute_revert(payload: &RevertPayload) -> AppResult<()> {
    match payload {
        RevertPayload::Registry { hive, path, name, old, post } => {
            registry::write(*hive, path, name, old)?;
            if !registry::matches_exact(*hive, path, name, old)? {
                return Err(AppError::coded("verify_failed"));
            }
            match post {
                Some(PostAction::MouseSpi) => winapi::apply_mouse_from_registry(),
                Some(PostAction::UiEffects) => {
                    winapi::set_ui_effects(true);
                    // Vensteranimaties weer zoals ze nu in het register staan.
                    let min_animate = registry::read(Hive::Hkcu, "Control Panel\\Desktop\\WindowMetrics", "MinAnimate").ok();
                    winapi::set_min_animate(min_animate.as_ref().and_then(|d| d.as_str()).map(|v| v.trim() != "0").unwrap_or(true));
                    winapi::broadcast_setting_change("WindowMetrics");
                }
                Some(PostAction::KeyboardSpi) => winapi::apply_keyboard_from_registry(),
                Some(PostAction::AccessibilitySpi) => winapi::apply_accessibility_from_registry(),
                Some(PostAction::LangToggleSpi) => winapi::apply_lang_toggle_from_registry(),
                Some(PostAction::PowerOverlay) => crate::tweaks::laptop::apply_overlay_from_registry(),
                Some(PostAction::SnappyUi) => winapi::apply_snappy_ui_from_registry(),
                None => {}
            }
            Ok(())
        }
        RevertPayload::PowerPlan { old_guid, created_guid, .. } => {
            let (ok, _, err) = run_cmd("powercfg", &["/setactive", old_guid])?;
            if !ok {
                return Err(AppError::msg(format!("powercfg: {}", err.trim())));
            }
            if let Some(g) = created_guid {
                let _ = run_cmd("powercfg", &["/delete", g]);
            }
            Ok(())
        }
        RevertPayload::PowerSetting { scheme, sub, setting, ac, dc } => {
            crate::tweaks::input::write_power_setting(scheme, sub, setting, *ac, *dc)
        }
        RevertPayload::DisplayMode { device, width, height, freq } => display::set_mode(device, *width, *height, *freq),
        RevertPayload::File { path, backup_path } => {
            if !Path::new(backup_path).exists() {
                return Err(AppError::NotFound(backup_path.clone()));
            }
            let was_readonly = std::fs::metadata(path).map(|m| m.permissions().readonly()).unwrap_or(false);
            if was_readonly {
                let mut p = std::fs::metadata(path)?.permissions();
                #[allow(clippy::permissions_set_readonly_false)]
                p.set_readonly(false);
                std::fs::set_permissions(path, p)?;
            }
            std::fs::copy(backup_path, path)?;
            if was_readonly {
                let mut p = std::fs::metadata(path)?.permissions();
                p.set_readonly(true);
                std::fs::set_permissions(path, p)?;
            }
            Ok(())
        }
        RevertPayload::Adapter { name, old_value } => {
            run_ps(&adapter_restore_command(name, old_value)?)?;
            Ok(())
        }
        RevertPayload::Priority { pid, old_class } => {
            // Als het proces al weg is, valt er niets terug te zetten.
            if winapi::process_exists(*pid) {
                winapi::set_priority_class(*pid, *old_class)?;
            }
            Ok(())
        }
        RevertPayload::DevicePower { instance, enable } => crate::tweaks::input_extra::set_device_power(instance, *enable),
        RevertPayload::WifiGaming => {
            crate::tweaks::wifi_gaming::stop();
            Ok(())
        }
        RevertPayload::TouchpadMouse { allow } => crate::tweaks::laptop::touchpad_set_allow(*allow),
        RevertPayload::IniKey { path, section, key, old } => {
            let p = Path::new(path);
            let (mut text, utf16) = crate::ini::read_text(p)?;
            match old {
                Some(v) => crate::ini::set(&mut text, section, key, v),
                None => crate::ini::remove(&mut text, section, key),
            }
            crate::ini::write_text(p, &text, utf16)
        }
        RevertPayload::JsonKey { path, pointer, old, pretty } => {
            let old = match old {
                Some(t) => Some(serde_json::from_str(t).map_err(|e| AppError::msg(e.to_string()))?),
                None => None,
            };
            crate::jsonfile::set_pointer(Path::new(path), pointer, old, *pretty)
        }
        RevertPayload::TextKey { path, format, key, old } => crate::gameconfig::revert_key(Path::new(path), format, key, old),
        RevertPayload::AdapterProperty { name, keyword, old_value } => {
            run_ps(&format!(
                "Set-NetAdapterAdvancedProperty -Name {} -RegistryKeyword {} -RegistryValue {} -NoRestart:$false",
                ps_quote(name),
                ps_quote(keyword),
                ps_quote(old_value)
            ))?;
            Ok(())
        }
        RevertPayload::QosPolicy { name } => crate::gamenet::remove_policy(name),
        RevertPayload::Warp => crate::gamenet::warp_revert(),
        RevertPayload::InterfaceMetric { luid, family, automatic, metric } => crate::gamenet::revert_metric(*luid, *family, *automatic, *metric),
        RevertPayload::NvDrsDword { setting_id, old } => crate::tweaks::smoothness::nvdrs_write_dword(*setting_id, *old),
        RevertPayload::None => Err(AppError::coded("not_revertable")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_untrusted_adapter_expression() {
        let payload = RevertPayload::Adapter { name: "test".into(), old_value: "Enabled; throw 'injected'".into() };
        assert!(execute_revert(&payload).unwrap_err().to_string().contains("Unknown adapter"));
    }

    #[test]
    fn adapter_restore_checks_both_states_and_verifies_the_driver_result() {
        // These local functions shadow the Windows CIM cmdlets in this child process.
        let provider = r#"
            $script:enabled = $false
            function Get-CimInstance($ClassName, $Namespace) {
                if ($ClassName -eq 'Win32_NetworkAdapter') {
                    [pscustomobject]@{ NetConnectionID = "test's adapter"; PNPDeviceID = 'PCI\test' }
                } else {
                    [pscustomobject]@{ InstanceName = 'PCI\test_0'; Enable = $script:enabled }
                }
            }
            function Set-CimInstance($InputObject, $Property, $ErrorAction) { $script:enabled = $Property.Enable }
        "#;
        let enable = adapter_restore_command("test's adapter", "Enabled").unwrap();
        let disable = adapter_restore_command("test's adapter", "Disabled").unwrap();
        let result = run_ps(&format!("{provider}\n{enable}; $script:enabled; {disable}; $script:enabled")).unwrap();
        assert_eq!(result.lines().collect::<Vec<_>>(), ["True", "False"]);
        let ignores_write = provider.replace("$script:enabled = $Property.Enable", "");
        assert!(run_ps(&format!("{ignores_write}\n{enable}")).is_err());
    }

    #[test]
    fn restores_game_settings_after_reopen_without_losing_other_options() {
        let dir = std::env::temp_dir().join(format!("nezty-game-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("GameUserSettings.ini");
        let original = b"[Game]\r\nbUseVSync=True\r\nResolutionSizeX=1920\r\n";
        std::fs::write(&path, original).unwrap();
        let changes = [("[Game]", "bUseVSync", "False".to_string()), ("[Game]", "LowLatency", "1".to_string())];
        let mut journal = Journal::load(&dir);
        for entry in crate::fortnite::write_changes(&path, &changes).unwrap() {
            journal.record(entry).unwrap();
        }
        assert_ne!(std::fs::read(&path).unwrap(), original);
        drop(journal);
        let result = Journal::load(&dir).revert_all();
        assert!(result.failed.is_empty());
        assert_eq!(result.reverted, 2);
        assert_eq!(std::fs::read(&path).unwrap(), original);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_restore_and_failed_save_remain_retryable() {
        let dir = std::env::temp_dir().join(format!("nezty-retry-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("game.ini");
        let backup = dir.join("original.ini");
        std::fs::write(&path, "new").unwrap();
        let mut entry = JournalEntry::new("fortnite_ini", "test", JournalKind::File, "game.ini");
        entry.revertable = true;
        entry.payload = RevertPayload::File { path: path.display().to_string(), backup_path: backup.display().to_string() };
        Journal::load(&dir).record(entry).unwrap();
        assert_eq!(Journal::load(&dir).revert_all().failed.len(), 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
        std::fs::write(&backup, "original").unwrap();
        let obstacle = dir.join("journal.json.tmp");
        std::fs::create_dir(&obstacle).unwrap();
        assert_eq!(Journal::load(&dir).revert_all().failed.len(), 1);
        assert!(Journal::load(&dir).has_any_active());
        std::fs::remove_dir(&obstacle).unwrap();
        let result = Journal::load(&dir).revert_all();
        assert!(result.failed.is_empty());
        assert_eq!(result.reverted, 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
        assert!(!Journal::load(&dir).has_any_active());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn legacy_pending_or_unknown_requires_review_of_frozen_copy() {
        let dir = std::env::temp_dir().join(format!("nezty-migrate-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let old = dir.join("legacy.json");
        let pending = br#"[{"futureVersion":true}]"#;
        std::fs::write(&old, pending).unwrap();
        assert!(Journal::prepare_legacy(&old, &dir).is_err());
        std::fs::write(&old, "[]").unwrap();
        assert!(Journal::prepare_legacy(&old, &dir).is_err());
        assert!(!dir.join("journal.json").exists());
        assert_eq!(std::fs::read(dir.join("legacy-review.json")).unwrap(), pending);
        std::fs::rename(dir.join("legacy-review.json"), dir.join("journal.json")).unwrap();
        assert_eq!(Journal::load(&dir).revert_all().failed.len(), 1);
        std::fs::write(dir.join("journal.json"), "{").unwrap();
        let mut corrupted = Journal::load(&dir);
        assert_eq!(corrupted.revert_all().failed.len(), 1);
        assert!(corrupted.save().is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
