//! Afbeeldingen van games. Volgorde: eerst lokaal uit de launcher (Steam-bibliotheekcache,
//! Xbox/Store-app-bestanden, de Epic-catalogus), dan de Steam-CDN voor bekende games, en als laatste
//! SteamGridDB met een eigen API-sleutel. Alles komt in een cache in de app-datamap.

use crate::models::{Game, Launcher};
use crate::registry::{self, Hive, RegData};
use crate::state::AppState;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// Bestandsnaam-veilige versie van een game-id.
pub fn safe_name(id: &str) -> String {
    id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

/// Pad naar de afbeelding in de cache als die bestaat.
pub fn cached(dir: &Path, id: &str) -> Option<PathBuf> {
    let base = safe_name(id);
    ["jpg", "png", "webp"].iter().map(|ext| dir.join(format!("{base}.{ext}"))).find(|p| p.is_file())
}

/// Eigen pictogram in de cache (voor de placeholder), als dat er is.
pub fn cached_icon(dir: &Path, id: &str) -> Option<PathBuf> {
    let p = dir.join(format!("{}.icon.png", safe_name(id)));
    p.is_file().then_some(p)
}

/// Minecraft Java: het grasblok uit de eigen bestanden van de game (.minecraft\assets).
/// De assets-index koppelt "icons/icon_256x256.png" aan een bestand in objects\<xx>\<hash>.
fn minecraft_java_icon() -> Option<PathBuf> {
    let assets = PathBuf::from(std::env::var_os("APPDATA")?).join(".minecraft").join("assets");
    let mut indexes: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(assets.join("indexes"))
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
        .map(|p| (std::fs::metadata(&p).and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH), p))
        .collect();
    indexes.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, index) in indexes {
        let Ok(text) = std::fs::read_to_string(&index) else { continue };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
        for key in ["icons/icon_256x256.png", "icons/icon_128x128.png"] {
            let Some(hash) = v["objects"][key]["hash"].as_str() else { continue };
            if hash.len() < 3 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
                continue;
            }
            let file = assets.join("objects").join(&hash[..2]).join(hash);
            let is_png = std::fs::read(&file).map(|b| b.starts_with(b"\x89PNG")).unwrap_or(false);
            if is_png {
                return Some(file);
            }
        }
    }
    None
}

/// Zet eigen pictogrammen in de cache voor games waarvan het programma-pictogram niet klopt
/// (Minecraft Java draait via javaw.exe en zou anders het Java-logo tonen).
fn ensure_icons(dir: &Path, games: &[Game]) -> usize {
    let mut added = 0;
    for g in games.iter().filter(|g| g.id == "minecraft:java") {
        if cached_icon(dir, &g.id).is_some() {
            continue;
        }
        if let Some(src) = minecraft_java_icon() {
            let _ = std::fs::create_dir_all(dir);
            if std::fs::copy(&src, dir.join(format!("{}.icon.png", safe_name(&g.id)))).is_ok() {
                added += 1;
            }
        }
    }
    added
}

fn none_marker(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{}.none", safe_name(id)))
}

/// Wist de "niets gevonden"-markeringen, zodat een nieuwe poging alles opnieuw probeert.
pub fn clear_misses(dir: &Path) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            if e.path().extension().map(|x| x == "none").unwrap_or(false) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

fn ext_for(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("png")
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

fn store(dir: &Path, id: &str, bytes: &[u8]) -> Option<PathBuf> {
    let ext = ext_for(bytes)?;
    std::fs::create_dir_all(dir).ok()?;
    let path = dir.join(format!("{}.{ext}", safe_name(id)));
    std::fs::write(&path, bytes).ok()?;
    Some(path)
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .tls_config(ureq::tls::TlsConfig::builder().provider(ureq::tls::TlsProvider::NativeTls).build())
        .timeout_global(Some(Duration::from_secs(20)))
        .http_status_as_error(true)
        .build()
        .into()
}

fn download(agent: &ureq::Agent, url: &str, bearer: Option<&str>) -> Option<Vec<u8>> {
    let mut req = agent.get(url).header("User-Agent", "nezty/1.0 (game cover cache)");
    if let Some(b) = bearer {
        req = req.header("Authorization", &format!("Bearer {b}"));
    }
    let mut resp = req.call().ok()?;
    resp.body_mut().with_config().limit(15 * 1024 * 1024).read_to_vec().ok()
}

// ---------- Lokale bronnen ----------

fn steam_path() -> Option<PathBuf> {
    if let Ok(RegData::Str(p)) = registry::read(Hive::Hkcu, "Software\\Valve\\Steam", "SteamPath") {
        if !p.is_empty() {
            return Some(PathBuf::from(p.replace('/', "\\")));
        }
    }
    None
}

/// Steam bewaart afbeeldingen in appcache\librarycache\<appid>\ (soms met een submap per versie).
fn steam_local(appid: u32) -> Option<Vec<u8>> {
    let base = steam_path()?.join("appcache").join("librarycache");
    let dir = base.join(appid.to_string());
    let wanted = ["header.jpg", "library_header.jpg", "library_hero.jpg", "library_600x900.jpg"];
    for name in wanted {
        if let Some(p) = find_file(&dir, name, 2) {
            if let Ok(b) = std::fs::read(&p) {
                return Some(b);
            }
        }
    }
    // Oude indeling: <appid>_header.jpg naast elkaar.
    for name in ["header.jpg", "library_hero.jpg", "library_600x900.jpg"] {
        let p = base.join(format!("{appid}_{name}"));
        if let Ok(b) = std::fs::read(&p) {
            return Some(b);
        }
    }
    None
}

fn find_file(dir: &Path, name: &str, depth: u32) -> Option<PathBuf> {
    let direct = dir.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    if depth == 0 {
        return None;
    }
    let rd = std::fs::read_dir(dir).ok()?;
    for e in rd.flatten() {
        if e.path().is_dir() {
            if let Some(p) = find_file(&e.path(), name, depth - 1) {
                return Some(p);
            }
        }
    }
    None
}

/// Xbox-app en Store: het opstartscherm of het grote logo uit de app-bestanden.
fn xbox_local(game: &Game) -> Option<Vec<u8>> {
    let content = PathBuf::from(game.install_path.as_ref()?);
    let config = std::fs::read_to_string(content.join("MicrosoftGame.config"))
        .or_else(|_| std::fs::read_to_string(content.join("AppxManifest.xml")))
        .ok()?;
    let attrs = ["SplashScreenImage", "Square480x480Logo", "Image", "Square150x150Logo"];
    for attr in attrs {
        let re = Regex::new(&format!(r#"{attr}="([^"]+)""#)).ok()?;
        for cap in re.captures_iter(&config) {
            let rel = cap[1].replace('/', "\\");
            if let Some(bytes) = read_scaled_asset(&content, &rel) {
                return Some(bytes);
            }
        }
    }
    None
}

/// Store-apps hebben vaak varianten zoals SplashScreen.scale-200.png; kies de grootste.
fn read_scaled_asset(root: &Path, rel: &str) -> Option<Vec<u8>> {
    let direct = root.join(rel);
    if direct.is_file() {
        return std::fs::read(direct).ok();
    }
    let full = root.join(rel);
    let dir = full.parent()?;
    let stem = full.file_stem()?.to_string_lossy().to_lowercase();
    let ext = full.extension()?.to_string_lossy().to_lowercase();
    let mut best: Option<(u64, PathBuf)> = None;
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let n = e.file_name().to_string_lossy().to_lowercase();
        if n.starts_with(&format!("{stem}.")) && n.ends_with(&format!(".{ext}")) {
            let size = e.metadata().map(|m| m.len()).unwrap_or(0);
            if best.as_ref().map(|(s, _)| size > *s).unwrap_or(true) {
                best = Some((size, e.path()));
            }
        }
    }
    std::fs::read(best?.1).ok()
}

/// Epic bewaart de catalogus (met afbeeldings-URL's) lokaal als base64-JSON.
fn epic_image_url(game: &Game) -> Option<String> {
    let uri = game.launch_uri.as_deref()?;
    // com.epicgames.launcher://apps/<namespace>%3A<itemid>%3A<appname>?...
    let ids = uri.split("apps/").nth(1)?.split('?').next()?;
    let parts: Vec<&str> = ids.split("%3A").collect();
    if parts.len() < 2 {
        return None;
    }
    let (ns, item) = (parts[0], parts[1]);
    let path = PathBuf::from(std::env::var("PROGRAMDATA").unwrap_or_else(|_| "C:\\ProgramData".into()))
        .join("Epic\\EpicGamesLauncher\\Data\\Catalog\\catcache.bin");
    let raw = std::fs::read_to_string(path).ok()?;
    use base64::Engine;
    let decoded = base64::engine::general_purpose::STANDARD.decode(raw.trim()).ok()?;
    let items: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    let item_v = items.as_array()?.iter().find(|i| i["namespace"] == ns && i["id"] == item)?;
    let images = item_v["keyImages"].as_array()?;
    for kind in ["DieselGameBox", "DieselGameBoxWide", "Thumbnail", "DieselGameBoxTall"] {
        if let Some(url) = images.iter().find(|k| k["type"] == kind).and_then(|k| k["url"].as_str()) {
            return Some(url.to_string());
        }
    }
    None
}

// ---------- Online bronnen ----------

fn steam_cdn(agent: &ureq::Agent, appid: u32) -> Option<Vec<u8>> {
    let urls = [
        format!("https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/{appid}/header.jpg"),
        format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{appid}/header.jpg"),
    ];
    urls.iter().find_map(|u| download(agent, u, None))
}

fn steamgriddb(agent: &ureq::Agent, key: &str, name: &str) -> Option<Vec<u8>> {
    let term: String = name.chars().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect();
    let term = term.split_whitespace().collect::<Vec<_>>().join("%20");
    let search = download(agent, &format!("https://www.steamgriddb.com/api/v2/search/autocomplete/{term}"), Some(key))?;
    let v: serde_json::Value = serde_json::from_slice(&search).ok()?;
    let id = v["data"].as_array()?.first()?["id"].as_u64()?;
    let grids = download(agent, &format!("https://www.steamgriddb.com/api/v2/grids/game/{id}?dimensions=920x430,460x215&types=static"), Some(key))?;
    let g: serde_json::Value = serde_json::from_slice(&grids).ok()?;
    let url = g["data"].as_array()?.first()?["url"].as_str()?.to_string();
    download(agent, &url, None)
}

/// Zoekt één afbeelding voor een game. Geeft het pad in de cache terug.
fn fetch_one(agent: &ureq::Agent, dir: &Path, game: &Game, sgdb_key: &str, online: bool) -> Option<PathBuf> {
    // 1. Lokaal
    let local = match game.launcher {
        Launcher::Steam => game.steam_app_id.and_then(steam_local),
        Launcher::Xbox => xbox_local(game),
        _ => None,
    }
    .or_else(|| game.steam_app_id.and_then(steam_local));
    if let Some(b) = local {
        if let Some(p) = store(dir, &game.id, &b) {
            return Some(p);
        }
    }
    // Zonder toestemming voor online ophalen stopt het hier.
    if !online {
        return None;
    }
    // 2. Epic-catalogus (URL lokaal, afbeelding van Epic's CDN)
    if game.launcher == Launcher::Epic {
        if let Some(url) = epic_image_url(game) {
            let sized = format!("{url}?w=920&h=430&resize=1&quality=high");
            if let Some(b) = download(agent, &sized, None).or_else(|| download(agent, &url, None)) {
                if let Some(p) = store(dir, &game.id, &b) {
                    return Some(p);
                }
            }
        }
    }
    // 3. Steam-CDN voor bekende games
    if let Some(appid) = game.steam_app_id {
        if let Some(b) = steam_cdn(agent, appid) {
            if let Some(p) = store(dir, &game.id, &b) {
                return Some(p);
            }
        }
    }
    // 4. SteamGridDB met eigen sleutel
    if !sgdb_key.trim().is_empty() {
        if let Some(b) = steamgriddb(agent, sgdb_key.trim(), &game.name) {
            if let Some(p) = store(dir, &game.id, &b) {
                return Some(p);
            }
        }
    }
    None
}

/// Haalt op de achtergrond ontbrekende afbeeldingen op en meldt elke nieuwe via "covers-updated".
pub fn fetch_missing(app: AppHandle, state: Arc<AppState>) {
    if state.covers_busy.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        let dir = state.covers_dir();
        let settings = state.settings_clone();
        let key = settings.steam_grid_db_key.clone();
        let online = settings.online_covers;
        let agent = agent();
        let mut changed = 0;
        let games = state.all_games();
        if ensure_icons(&dir, &games) > 0 {
            for g in games.iter().filter(|g| g.id == "minecraft:java") {
                let _ = app.emit("covers-updated", &g.id);
            }
        }
        for game in state.all_games() {
            if cached(&dir, &game.id).is_some() || none_marker(&dir, &game.id).is_file() {
                continue;
            }
            match fetch_one(&agent, &dir, &game, &key, online) {
                Some(_) => {
                    changed += 1;
                    let _ = app.emit("covers-updated", &game.id);
                }
                None => {
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = std::fs::write(none_marker(&dir, &game.id), b"");
                }
            }
        }
        log::info!("afbeeldingen opgehaald: {changed}");
        state.covers_busy.store(false, Ordering::SeqCst);
    });
}
