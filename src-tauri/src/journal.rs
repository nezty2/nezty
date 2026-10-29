//! Het logboek: elke wijziging wordt met de oude waarde opgeslagen, zodat "Alles terugzetten"
//! alles exact kan herstellen. Wordt als JSON in de app-datamap bewaard.

use crate::error::{AppError, AppResult};
use crate::models::{now_ms, JournalKind, Msg, RevertFailure, RevertResult};
use crate::powershell::{ps_quote, run_cmd, run_ps};
use crate::registry::{self, Hive, RegData};
use crate::{display, winapi};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
        }
    }
}

pub struct Journal {
    path: PathBuf,
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
    /// Leest het logboek. Is (een deel) niet te lezen, dan komt er eerst een kopie naast
    /// (journal.unreadable-<tijd>.json). Een onleesbaar logboek wordt nooit stil overschreven.
    pub fn load(dir: &Path) -> Journal {
        let path = dir.join("journal.json");
        let parsed = match std::fs::read_to_string(&path) {
            Ok(text) => parse(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Parsed::Missing,
            Err(_) => Parsed::Unreadable,
        };
        let mut j = Journal { path, entries: Vec::new(), unknown: Vec::new(), backup: None, blocked: false };
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
                j.blocked = j.backup.is_none();
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
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(&all)?)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    pub fn record(&mut self, entry: JournalEntry) -> AppResult<()> {
        self.entries.push(entry);
        self.save()
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

    pub fn has_active(&self, tweak_id: &str, game_id: Option<&str>) -> bool {
        !self.active_ids(tweak_id, game_id).is_empty()
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
        order.sort_by(|a, b| self.entries[*b].ts.cmp(&self.entries[*a].ts));
        for i in order {
            let entry = &self.entries[i];
            if !entry.revertable {
                result.skipped += 1;
                continue;
            }
            match execute_revert(&entry.payload) {
                Ok(()) => {
                    self.entries[i].reverted = true;
                    result.reverted += 1;
                }
                Err(e) => result.failed.push(RevertFailure { id: entry.id.clone(), target: entry.target.clone(), error: e.to_string() }),
            }
        }
        if let Err(e) = self.save() {
            log::error!("logboek opslaan mislukt: {e}");
        }
        result
    }

    pub fn revert_all(&mut self) -> RevertResult {
        let ids: Vec<String> = self.entries.iter().filter(|e| !e.reverted).map(|e| e.id.clone()).collect();
        self.revert_ids(&ids)
    }
}

/// Voert één terugzet-actie uit.
pub fn execute_revert(payload: &RevertPayload) -> AppResult<()> {
    match payload {
        RevertPayload::Registry { hive, path, name, old, post } => {
            registry::write(*hive, path, name, old)?;
            match post {
                Some(PostAction::MouseSpi) => winapi::apply_mouse_from_registry(),
                Some(PostAction::UiEffects) => {
                    winapi::set_ui_effects(true);
                    winapi::broadcast_setting_change("WindowMetrics");
                }
                Some(PostAction::KeyboardSpi) => winapi::apply_keyboard_from_registry(),
                Some(PostAction::AccessibilitySpi) => winapi::apply_accessibility_from_registry(),
                None => {}
            }
            Ok(())
        }
        RevertPayload::PowerPlan { old_guid, created_guid } => {
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
            run_ps(&format!("Set-NetAdapterPowerManagement -Name {} -AllowComputerToTurnOffDevice {}", ps_quote(name), old_value))?;
            Ok(())
        }
        RevertPayload::Priority { pid, old_class } => {
            // Als het proces al weg is, valt er niets terug te zetten.
            if winapi::process_exists(*pid) {
                winapi::set_priority_class(*pid, *old_class)?;
            }
            Ok(())
        }
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
        RevertPayload::None => Err(AppError::coded("not_revertable")),
    }
}
