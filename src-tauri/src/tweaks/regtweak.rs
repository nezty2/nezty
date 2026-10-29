//! Generieke registertweak: een lijst waarden met een "aan"- en "uit"-waarde, met logboek.

use crate::error::{AppError, AppResult};
use crate::journal::{JournalEntry, PostAction, RevertPayload};
use crate::models::{JournalKind, TweakState};
use crate::registry::{self, Hive, RegData};
use crate::state::{lock, AppState};
use crate::tweaks::{revert_via_journal, TweakDef};

#[derive(Clone, Debug)]
pub struct RegSpec {
    pub hive: Hive,
    pub path: String,
    pub name: String,
    pub on: RegData,
    pub off: RegData,
    /// Ontbrekende waarde telt als "aan" (Windows-standaard is al goed).
    pub absent_is_on: bool,
}

impl RegSpec {
    pub fn dword(hive: Hive, path: &str, name: &str, on: u32, off: u32) -> Self {
        RegSpec {
            hive,
            path: path.to_string(),
            name: name.to_string(),
            on: RegData::Dword(on),
            off: RegData::Dword(off),
            absent_is_on: false,
        }
    }
    pub fn string(hive: Hive, path: &str, name: &str, on: &str, off: &str) -> Self {
        RegSpec {
            hive,
            path: path.to_string(),
            name: name.to_string(),
            on: RegData::Str(on.to_string()),
            off: RegData::Str(off.to_string()),
            absent_is_on: false,
        }
    }
    pub fn absent_on(mut self) -> Self {
        self.absent_is_on = true;
        self
    }
    pub fn off_absent(mut self) -> Self {
        self.off = RegData::Absent;
        self
    }

    fn is_on(&self, current: &RegData) -> bool {
        if *current == self.on {
            return true;
        }
        // "0" en 0 als tekst: vergelijk getallen in tekstwaarden soepel ("0" == "00").
        if let (RegData::Str(a), RegData::Str(b)) = (current, &self.on) {
            if let (Ok(x), Ok(y)) = (a.trim().parse::<i64>(), b.trim().parse::<i64>()) {
                return x == y;
            }
        }
        self.absent_is_on && *current == RegData::Absent
    }
}

pub fn state_of(specs: &[RegSpec]) -> AppResult<TweakState> {
    if specs.is_empty() {
        return Ok(TweakState::Unavailable);
    }
    for s in specs {
        let cur = registry::read(s.hive, &s.path, &s.name)?;
        if !s.is_on(&cur) {
            return Ok(TweakState::Off);
        }
    }
    Ok(TweakState::On)
}

fn ensure_rights(state: &AppState, specs: &[RegSpec], name: &str) -> AppResult<()> {
    if specs.iter().any(|s| s.hive.needs_admin()) && !state.elevated {
        return Err(AppError::NeedsAdmin(name.to_string()));
    }
    Ok(())
}

/// Schrijft de "aan"-waarden en logt elke daadwerkelijke wijziging. Waarden die al goed staan
/// worden niet opnieuw geschreven.
pub fn apply(
    state: &AppState,
    def: &TweakDef,
    specs: &[RegSpec],
    session: Option<&str>,
    game_id: Option<&str>,
    post: Option<PostAction>,
) -> AppResult<()> {
    ensure_rights(state, specs, def.name)?;
    for s in specs {
        let old = registry::read(s.hive, &s.path, &s.name)?;
        if s.is_on(&old) {
            continue;
        }
        registry::write(s.hive, &s.path, &s.name, &s.on)?;
        let mut e = JournalEntry::new(def.id, def.name, JournalKind::Registry, &registry::full_path(s.hive, &s.path, &s.name));
        e.old_value = Some(old.display());
        e.new_value = Some(s.on.display());
        e.revertable = true;
        e.session_id = session.map(|x| x.to_string());
        e.game_id = game_id.map(|x| x.to_string());
        e.payload = RevertPayload::Registry { hive: s.hive, path: s.path.clone(), name: s.name.clone(), old, post };
        lock(&state.journal).record(e)?;
    }
    Ok(())
}

/// Zet uit: via het logboek als de app het heeft aangezet, anders naar de "uit"-waarden (ook gelogd).
pub fn disable(
    state: &AppState,
    def: &TweakDef,
    specs: &[RegSpec],
    game_id: Option<&str>,
    post: Option<PostAction>,
) -> AppResult<()> {
    ensure_rights(state, specs, def.name)?;
    if revert_via_journal(state, def.id, game_id)? {
        // Als de oude waarde zelf al "aan" was, staat de tweak na terugzetten nog aan.
        // Zet dan alsnog de Windows-standaard, zodat de schakelaar doet wat de gebruiker vraagt.
        if state_of(specs)? == TweakState::Off {
            return Ok(());
        }
    }
    for s in specs {
        let old = registry::read(s.hive, &s.path, &s.name)?;
        if old == s.off {
            continue;
        }
        registry::write(s.hive, &s.path, &s.name, &s.off)?;
        let mut e = JournalEntry::new(def.id, def.name, JournalKind::Registry, &registry::full_path(s.hive, &s.path, &s.name));
        e.old_value = Some(old.display());
        e.new_value = Some(s.off.display());
        e.revertable = true;
        e.note = Some("manual_off".to_string());
        e.game_id = game_id.map(|x| x.to_string());
        e.payload = RevertPayload::Registry { hive: s.hive, path: s.path.clone(), name: s.name.clone(), old, post };
        lock(&state.journal).record(e)?;
    }
    Ok(())
}
