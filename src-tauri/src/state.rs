//! Gedeelde app-state (instellingen, logboek, profielen, metingen, boost-sessie, caches).

use crate::journal::Journal;
use crate::models::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, MutexGuard};

pub const HISTORY_LEN: usize = 120;

pub struct MetricsStore {
    pub history: VecDeque<MetricsSample>,
}

pub struct AppState {
    pub data_dir: PathBuf,
    pub elevated: bool,
    pub settings: Mutex<Settings>,
    pub journal: Mutex<Journal>,
    pub profiles: Mutex<Vec<GameProfile>>,
    pub metrics: Mutex<MetricsStore>,
    pub boost: Mutex<Option<BoostReport>>,
    pub games: Mutex<Vec<Game>>,
    pub running_games: Mutex<HashSet<String>>,
    pub watcher_running: AtomicBool,
    pub restore_point: Mutex<RestorePointStatus>,
    pub procs: Mutex<sysinfo::System>,
    /// Laatst gesloten apps (om ze opnieuw te kunnen starten).
    pub last_closed: Mutex<Vec<ClosedProcess>>,
    /// Vriendelijke namen per exe-pad.
    pub titles: Mutex<HashMap<String, String>>,
    /// Pictogrammen (PNG-data-URL) per exe-pad.
    pub icons: Mutex<HashMap<String, Option<String>>>,
    /// Loopt er een afbeeldingen-ophaalronde?
    pub covers_busy: AtomicBool,
    /// Licentie: sessie, ondertekend token en apparaat-ID.
    pub license: Mutex<crate::license::Runtime>,
}

/// Lock die een "poisoned" mutex gewoon doorgeeft in plaats van te panikeren.
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl AppState {
    /// De periodieke controle kan net het logboek hebben. Wacht op de schrijver
    /// zodat het venster bij een gelijktijdige start gewoon kan openen.
    pub fn open_desktop(data_dir: PathBuf, elevated: bool) -> crate::error::AppResult<Self> {
        let start = std::time::Instant::now();
        loop {
            match Self::open(data_dir.clone(), elevated, false) {
                Err(crate::error::AppError::Io(e)) if e.raw_os_error() == Some(32) && start.elapsed().as_secs() < 60 =>
                    std::thread::sleep(std::time::Duration::from_millis(250)),
                result => return result,
            }
        }
    }
    #[cfg(any(test, feature = "selftest"))]
    pub fn new(data_dir: PathBuf, elevated: bool) -> Self {
        let journal = Journal::load(&data_dir);
        Self::with_journal(data_dir, elevated, journal)
    }

    pub fn open(data_dir: PathBuf, elevated: bool, uninstalling: bool) -> crate::error::AppResult<Self> {
        #[cfg(feature = "selftest")]
        if std::env::var_os("NZTY_DATA_DIR").is_some_and(|p| !p.is_empty()) {
            return Ok(Self::new(data_dir, elevated));
        }
        let journal = Journal::open(&data_dir, uninstalling)?;
        Ok(Self::with_journal(data_dir, elevated, journal))
    }

    fn with_journal(data_dir: PathBuf, elevated: bool, journal: Journal) -> Self {
        std::fs::create_dir_all(&data_dir).ok();
        let settings = load_json::<Settings>(&data_dir.join("settings.json")).unwrap_or_default();
        let profiles = load_json::<Vec<GameProfile>>(&data_dir.join("profiles.json")).unwrap_or_default();
        AppState {
            data_dir,
            elevated,
            settings: Mutex::new(settings),
            journal: Mutex::new(journal),
            profiles: Mutex::new(profiles),
            metrics: Mutex::new(MetricsStore { history: VecDeque::with_capacity(HISTORY_LEN) }),
            boost: Mutex::new(None),
            games: Mutex::new(vec![]),
            running_games: Mutex::new(HashSet::new()),
            watcher_running: AtomicBool::new(false),
            restore_point: Mutex::new(RestorePointStatus::default()),
            procs: Mutex::new(sysinfo::System::new()),
            last_closed: Mutex::new(vec![]),
            titles: Mutex::new(HashMap::new()),
            icons: Mutex::new(HashMap::new()),
            covers_busy: AtomicBool::new(false),
            license: Mutex::new(crate::license::Runtime::default()),
        }
    }

    pub fn background_dir(&self) -> PathBuf {
        self.data_dir.join("background")
    }

    pub fn covers_dir(&self) -> PathBuf {
        self.data_dir.join("covers")
    }

    pub fn save_settings(&self) {
        let s = lock(&self.settings).clone();
        if let Err(e) = save_json(&self.data_dir.join("settings.json"), &s) {
            log::error!("instellingen opslaan mislukt: {e}");
        }
    }

    pub fn save_profiles(&self) {
        let p = lock(&self.profiles).clone();
        if let Err(e) = save_json(&self.data_dir.join("profiles.json"), &p) {
            log::error!("profielen opslaan mislukt: {e}");
        }
    }

    pub fn settings_clone(&self) -> Settings {
        lock(&self.settings).clone()
    }

    /// Profiel van een game, of een nieuw standaardprofiel als er nog geen is.
    pub fn profile_for(&self, game_id: &str) -> GameProfile {
        let profiles = lock(&self.profiles);
        profiles
            .iter()
            .find(|p| p.game_id == game_id)
            .cloned()
            .unwrap_or_else(|| GameProfile::new(game_id, &lock(&self.settings).default_profile_tweaks))
    }

    pub fn upsert_profile(&self, profile: GameProfile) {
        {
            let mut profiles = lock(&self.profiles);
            if let Some(p) = profiles.iter_mut().find(|p| p.game_id == profile.game_id) {
                *p = profile;
            } else {
                profiles.push(profile);
            }
        }
        self.save_profiles();
    }

    /// De laatste meting en of er in de laatste `secs` seconden terugschakeling door hitte was.
    pub fn recent_heat(&self, secs: usize) -> (Option<MetricsSample>, Option<bool>) {
        let m = lock(&self.metrics);
        let last = m.history.back().cloned();
        let recent: Vec<Option<bool>> = m.history.iter().rev().take(secs).map(|s| s.thermal_throttle).collect();
        let throttle = if recent.iter().all(|t| t.is_none()) { None } else { Some(recent.iter().any(|t| *t == Some(true))) };
        (last, throttle)
    }

    pub fn push_sample(&self, sample: MetricsSample) {
        let mut m = lock(&self.metrics);
        if m.history.len() >= HISTORY_LEN {
            m.history.pop_front();
        }
        m.history.push_back(sample);
    }

    /// Alle games (gevonden en zelf toegevoegd), met het pad naar de afbeelding als die in de cache staat.
    pub fn all_games(&self) -> Vec<Game> {
        let mut games = lock(&self.games).clone();
        games.extend(lock(&self.settings).custom_games.iter().cloned());
        let dir = self.covers_dir();
        for g in games.iter_mut() {
            if g.cover.is_none() {
                g.cover = crate::covers::cached(&dir, &g.id).map(|p| p.to_string_lossy().to_string());
            }
            if g.icon.is_none() {
                g.icon = crate::covers::cached_icon(&dir, &g.id).map(|p| p.to_string_lossy().to_string());
            }
        }
        games
    }
}

/// Leest settings.json of profiles.json. Is het bestand weg of onleesbaar (bv. half geschreven toen nezty in
/// quarantaine ging of hard werd afgesloten), dan de vorige versie uit `<naam>.bak`.
fn load_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    let read = |p: &Path| -> Option<T> {
        let s = std::fs::read_to_string(p).ok()?;
        serde_json::from_str(s.trim_start_matches('\u{feff}')).ok()
    };
    if let Some(v) = read(path) {
        return Some(v);
    }
    let bak = backup_path(path);
    let v = read(&bak)?;
    log::warn!("{} niet leesbaar: vorige versie uit {} gebruikt", path.display(), bak.display());
    Some(v)
}

fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".bak");
    path.with_file_name(name)
}

/// Schrijft eerst naar een tijdelijk bestand in dezelfde map en zet dat daarna in één keer op zijn plek.
/// Een onderbroken schrijfactie laat zo nooit een half bestand achter.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".tmp");
    let tmp = path.with_file_name(name);
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

/// Atomisch opslaan, met de vorige (leesbare) versie als `<naam>.bak`.
fn save_json<T: serde::Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
    // Alleen een leesbare vorige versie wordt de reserve: een kapot bestand overschrijft nooit een goede .bak.
    if let Ok(old) = std::fs::read_to_string(path) {
        if serde_json::from_str::<serde_json::Value>(old.trim_start_matches('\u{feff}')).is_ok() {
            let _ = write_atomic(&backup_path(path), old.as_bytes());
        }
    }
    write_atomic(path, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_to_backup() {
        let dir = std::env::temp_dir().join(format!("nezty-state-{}", uuid::Uuid::new_v4()));
        let f = dir.join("settings.json");
        save_json(&f, &vec![1, 2]).unwrap();
        save_json(&f, &vec![3]).unwrap();
        assert_eq!(load_json::<Vec<i32>>(&backup_path(&f)), Some(vec![1, 2]));
        // Half geschreven hoofdbestand: de vorige versie komt terug, en een kapot bestand wordt nooit de reserve.
        std::fs::write(&f, "{ kapot").unwrap();
        assert_eq!(load_json::<Vec<i32>>(&f), Some(vec![1, 2]));
        save_json(&f, &vec![4]).unwrap();
        assert_eq!(load_json::<Vec<i32>>(&backup_path(&f)), Some(vec![1, 2]));
        assert_eq!(load_json::<Vec<i32>>(&f), Some(vec![4]));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
