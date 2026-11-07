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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct Fixture { dir: PathBuf, key: String }
    impl Fixture {
        fn new() -> Self {
            let id = uuid::Uuid::new_v4();
            Self { dir: std::env::temp_dir().join(format!("nezty-roundtrip-{id}")), key: format!(r"Software\Nezty-Test-{id}") }
        }
        fn app(&self) -> AppState { AppState::new(self.dir.clone(), false) }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = Hive::Hkcu.root().delete_subkey_all(&self.key);
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn apply_disable_and_bulk_restore_after_restart() {
        let f = Fixture::new();
        let def = crate::tweaks::get("game_mode").unwrap();
        let specs = [RegSpec::dword(Hive::Hkcu, &f.key, "absent", 1, 0),
            RegSpec::string(Hive::Hkcu, &f.key, "custom", "on", "off")];
        let custom = RegData::Str("original".into());
        registry::write(Hive::Hkcu, &f.key, "custom", &custom).unwrap();
        {
            let app = f.app();
            apply(&app, def, &specs, Some("boost"), None, None).unwrap();
            assert_eq!(state_of(&specs).unwrap(), TweakState::On);
        }
        let app = f.app();
        disable(&app, def, &specs, None, None).unwrap();
        assert_eq!(registry::read(Hive::Hkcu, &f.key, "absent").unwrap(), RegData::Absent);
        assert_eq!(registry::read(Hive::Hkcu, &f.key, "custom").unwrap(), custom);
        // A manually disabled setting that was originally on must also be undoable.
        registry::write(Hive::Hkcu, &f.key, "absent", &RegData::Dword(1)).unwrap();
        disable(&app, def, &specs[..1], None, None).unwrap();
        assert_eq!(registry::read(Hive::Hkcu, &f.key, "absent").unwrap(), RegData::Dword(0));
        drop(app);
        let result = lock(&f.app().journal).revert_all();
        assert!(result.failed.is_empty());
        assert_eq!(result.reverted, 1);
        assert_eq!(registry::read(Hive::Hkcu, &f.key, "absent").unwrap(), RegData::Dword(1));
        assert_eq!(lock(&f.app().journal).revert_all().reverted, 0);
    }

    #[test]
    fn failed_journal_write_leaves_registry_and_memory_unchanged() {
        let f = Fixture::new();
        let app = f.app();
        let def = crate::tweaks::get("game_mode").unwrap();
        let spec = [RegSpec::dword(Hive::Hkcu, &f.key, "probe", 1, 0)];
        std::fs::create_dir(f.dir.join("journal.json.tmp")).unwrap();
        assert!(apply(&app, def, &spec, None, None, None).is_err());
        assert_eq!(registry::read(Hive::Hkcu, &f.key, "probe").unwrap(), RegData::Absent);
        registry::write(Hive::Hkcu, &f.key, "probe", &RegData::Dword(1)).unwrap();
        assert!(disable(&app, def, &spec, None, None).is_err());
        assert_eq!(registry::read(Hive::Hkcu, &f.key, "probe").unwrap(), RegData::Dword(1));
        assert!(lock(&app.journal).entries.is_empty());
    }
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
        let mut e = JournalEntry::new(def.id, def.name, JournalKind::Registry, &registry::full_path(s.hive, &s.path, &s.name));
        e.old_value = Some(old.display());
        e.new_value = Some(s.on.display());
        e.revertable = true;
        e.session_id = session.map(|x| x.to_string());
        e.game_id = game_id.map(|x| x.to_string());
        e.payload = RevertPayload::Registry { hive: s.hive, path: s.path.clone(), name: s.name.clone(), old, post };
        lock(&state.journal).write_registry(e, &s.on)?;
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
        let mut e = JournalEntry::new(def.id, def.name, JournalKind::Registry, &registry::full_path(s.hive, &s.path, &s.name));
        e.old_value = Some(old.display());
        e.new_value = Some(s.off.display());
        e.revertable = true;
        e.note = Some("manual_off".to_string());
        e.game_id = game_id.map(|x| x.to_string());
        e.payload = RevertPayload::Registry { hive: s.hive, path: s.path.clone(), name: s.name.clone(), old, post };
        lock(&state.journal).write_registry(e, &s.off)?;
    }
    Ok(())
}
