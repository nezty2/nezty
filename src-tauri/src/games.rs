//! Detectie van geïnstalleerde games via Epic, Steam, Riot, Battle.net/EA (Uninstall-register),
//! de Xbox-app (XboxGames-map) en Minecraft. Plus een catalogus met bekende games.

use crate::error::{AppError, AppResult};
use crate::models::{Game, Launcher};
use crate::powershell::{run_ps, spawn_detached};
use crate::registry::{self, Hive, RegData};
use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

struct Known {
    key: &'static str,
    name: &'static str,
    procs: &'static [&'static str],
    epic_app: &'static [&'static str],
    epic_exe: Option<&'static str>,
    steam_appids: &'static [u32],
    steam_exe: Option<&'static str>,
    uninstall_names: &'static [&'static str],
    uninstall_exe: Option<&'static str>,
    uninstall_launcher: Launcher,
}

const fn known(key: &'static str, name: &'static str, procs: &'static [&'static str]) -> Known {
    Known {
        key,
        name,
        procs,
        epic_app: &[],
        epic_exe: None,
        steam_appids: &[],
        steam_exe: None,
        uninstall_names: &[],
        uninstall_exe: None,
        uninstall_launcher: Launcher::Custom,
    }
}

const KNOWN: &[Known] = &[
    Known {
        epic_app: &["Fortnite"],
        epic_exe: Some("FortniteGame\\Binaries\\Win64\\FortniteClient-Win64-Shipping.exe"),
        uninstall_launcher: Launcher::Epic,
        ..known("fortnite", "Fortnite", &["FortniteClient-Win64-Shipping.exe"])
    },
    Known { uninstall_launcher: Launcher::Riot, ..known("valorant", "VALORANT", &["VALORANT-Win64-Shipping.exe"]) },
    Known {
        steam_appids: &[730],
        steam_exe: Some("game\\bin\\win64\\cs2.exe"),
        uninstall_launcher: Launcher::Steam,
        ..known("cs2", "Counter-Strike 2", &["cs2.exe"])
    },
    Known {
        steam_appids: &[1172470],
        steam_exe: Some("r5apex_dx12.exe"),
        uninstall_names: &["Apex Legends"],
        uninstall_exe: Some("r5apex_dx12.exe"),
        uninstall_launcher: Launcher::Ea,
        ..known("apex", "Apex Legends", &["r5apex_dx12.exe", "r5apex.exe"])
    },
    Known {
        steam_appids: &[1938090],
        steam_exe: Some("cod.exe"),
        uninstall_names: &["Call of Duty"],
        uninstall_exe: Some("cod.exe"),
        uninstall_launcher: Launcher::Battlenet,
        ..known("cod", "Call of Duty", &["cod.exe", "cod23-cod.exe", "cod24-cod.exe", "cod25-cod.exe", "cod26-cod.exe", "ModernWarfare.exe"])
    },
    Known {
        epic_app: &["Sugar", "Rocket League"],
        epic_exe: Some("Binaries\\Win64\\RocketLeague.exe"),
        steam_appids: &[252950],
        steam_exe: Some("Binaries\\Win64\\RocketLeague.exe"),
        uninstall_launcher: Launcher::Epic,
        ..known("rocketleague", "Rocket League", &["RocketLeague.exe"])
    },
    Known {
        steam_appids: &[359550],
        steam_exe: Some("RainbowSix.exe"),
        uninstall_launcher: Launcher::Steam,
        ..known("r6", "Rainbow Six Siege", &["RainbowSix.exe", "RainbowSix_Vulkan.exe", "RainbowSix_DX11.exe", "RainbowSix_BE.exe"])
    },
    Known {
        steam_appids: &[3240220, 271590],
        uninstall_launcher: Launcher::Steam,
        ..known("gta5", "Grand Theft Auto V", &["GTA5_Enhanced.exe", "GTA5.exe"])
    },
];

fn known_by_epic(app_name: &str, display: &str) -> Option<&'static Known> {
    KNOWN.iter().find(|k| k.epic_app.iter().any(|a| a.eq_ignore_ascii_case(app_name) || a.eq_ignore_ascii_case(display)))
}

fn known_by_steam(appid: u32) -> Option<&'static Known> {
    KNOWN.iter().find(|k| k.steam_appids.contains(&appid))
}

fn known_by_uninstall(display: &str) -> Option<&'static Known> {
    let lower = display.to_lowercase();
    KNOWN.iter().find(|k| k.uninstall_names.iter().any(|n| lower.contains(&n.to_lowercase())))
}

fn file_name(p: &str) -> String {
    Path::new(p).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()
}

fn join(base: &str, rel: &str) -> String {
    Path::new(base).join(rel).to_string_lossy().to_string()
}

fn existing(path: String) -> Option<String> {
    if Path::new(&path).is_file() {
        Some(path)
    } else {
        None
    }
}

const BAD_EXE_WORDS: &[&str] = &["unins", "crash", "setup", "redist", "launcher", "helper", "report", "install", "update", "_be.exe", "eac", "easyanticheat", "battleye", "vc_", "dxsetup", "dotnet", "ue4prereq", "cefprocess", "webhelper"];

fn is_bad_exe(name_lower: &str) -> bool {
    BAD_EXE_WORDS.iter().any(|w| name_lower.contains(w))
}

/// Zoekt een exe met deze naam in de installatiemap (tot een paar mappen diep).
fn find_exe(dir: &Path, name: &str, depth: u32) -> Option<String> {
    let direct = dir.join(name);
    if direct.is_file() {
        return Some(direct.to_string_lossy().to_string());
    }
    if depth == 0 {
        return None;
    }
    let rd = std::fs::read_dir(dir).ok()?;
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if let Some(f) = find_exe(&p, name, depth - 1) {
                return Some(f);
            }
        }
    }
    None
}

/// Schatting van de hoofd-exe: de grootste exe in de map (of één niveau dieper), zonder installers en helpers.
fn guess_main_exe(dir: &Path) -> Option<String> {
    let mut best: Option<(u64, PathBuf)> = None;
    let mut consider = |p: PathBuf| {
        let n = p.file_name().map(|f| f.to_string_lossy().to_lowercase()).unwrap_or_default();
        if !n.ends_with(".exe") || is_bad_exe(&n) {
            return;
        }
        let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
        if best.as_ref().map(|(s, _)| size > *s).unwrap_or(true) {
            best = Some((size, p));
        }
    };
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_file() {
                consider(p);
            } else if p.is_dir() {
                if let Ok(rd2) = std::fs::read_dir(&p) {
                    for e2 in rd2.flatten() {
                        if e2.path().is_file() {
                            consider(e2.path());
                        }
                    }
                }
            }
        }
    }
    best.map(|(_, p)| p.to_string_lossy().to_string())
}

/// Exe van een bekende game: eerst het bekende pad, dan de bekende procesnamen, dan een schatting.
fn resolve_exe(install: &str, known: Option<&Known>, rel: Option<&str>) -> Option<String> {
    if let Some(r) = rel {
        if let Some(p) = existing(join(install, r)) {
            return Some(p);
        }
    }
    if let Some(k) = known {
        for proc_name in k.procs {
            if let Some(p) = find_exe(Path::new(install), proc_name, 4) {
                return Some(p);
            }
        }
    }
    guess_main_exe(Path::new(install))
}

fn procs_for(known: Option<&Known>, exe: Option<&str>) -> Vec<String> {
    match known {
        Some(k) => k.procs.iter().map(|p| p.to_string()).collect(),
        None => exe.map(|e| vec![file_name(e)]).unwrap_or_default(),
    }
}

// ---------- Epic Games ----------

fn detect_epic() -> Vec<Game> {
    let dir = std::env::var_os("PROGRAMDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\ProgramData"))
        .join("Epic\\EpicGamesLauncher\\Data\\Manifests");
    let mut games = vec![];
    let Ok(rd) = std::fs::read_dir(&dir) else { return games };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e != "item").unwrap_or(true) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let app = s("AppName");
        let display = s("DisplayName");
        let install = s("InstallLocation");
        let launch_exe = s("LaunchExecutable");
        let main = s("MainGameAppName");
        if app.is_empty() || install.is_empty() || launch_exe.is_empty() {
            continue;
        }
        if v.get("bIsIncompleteInstall").and_then(|x| x.as_bool()).unwrap_or(false) {
            continue;
        }
        if !main.is_empty() && main != app {
            continue; // DLC of onderdeel
        }
        // Geen games: Unreal Engine, RealityScan en dergelijke.
        let cats = text.to_lowercase();
        if cats.contains("\"applications\"") && !cats.contains("\"games\"") {
            continue;
        }
        let ns = s("CatalogNamespace");
        let item = s("CatalogItemId");
        let launch_uri = if !ns.is_empty() && !item.is_empty() {
            Some(format!("com.epicgames.launcher://apps/{ns}%3A{item}%3A{app}?action=launch&silent=true"))
        } else {
            Some(format!("com.epicgames.launcher://apps/{app}?action=launch&silent=true"))
        };
        let k = known_by_epic(&app, &display);
        let exe_path = resolve_exe(&install, k, k.and_then(|k| k.epic_exe)).or_else(|| existing(join(&install, &launch_exe.replace('/', "\\"))));
        games.push(Game {
            id: k.map(|k| format!("epic:{}", k.key)).unwrap_or_else(|| format!("epic:{app}")),
            name: k.map(|k| k.name.to_string()).unwrap_or(display),
            launcher: Launcher::Epic,
            install_path: Some(install),
            process_names: k.map(|k| k.procs.iter().map(|p| p.to_string()).collect()).unwrap_or_else(|| vec![file_name(&launch_exe)]),
            exe_path,
            launch_uri,
            known: k.is_some(),
            icon_key: k.map(|k| k.key.to_string()),
            steam_app_id: k.and_then(|k| k.steam_appids.first().copied()),
            cover: None,
            icon: None,
        });
    }
    games
}

// ---------- Steam ----------

fn steam_path() -> Option<String> {
    if let Ok(RegData::Str(p)) = registry::read(Hive::Hkcu, "Software\\Valve\\Steam", "SteamPath") {
        if !p.is_empty() {
            return Some(p.replace('/', "\\"));
        }
    }
    if let Ok(RegData::Str(p)) = registry::read(Hive::Hklm, "SOFTWARE\\WOW6432Node\\Valve\\Steam", "InstallPath") {
        if !p.is_empty() {
            return Some(p);
        }
    }
    None
}

fn detect_steam() -> Vec<Game> {
    let mut games = vec![];
    let Some(steam) = steam_path() else { return games };
    let mut libraries = vec![PathBuf::from(&steam)];
    if let Ok(text) = std::fs::read_to_string(Path::new(&steam).join("steamapps").join("libraryfolders.vdf")) {
        let re = Regex::new(r#""path"\s+"([^"]+)""#).unwrap();
        for cap in re.captures_iter(&text) {
            let p = cap[1].replace("\\\\", "\\");
            if !libraries.iter().any(|l| l.to_string_lossy().eq_ignore_ascii_case(&p)) {
                libraries.push(PathBuf::from(p));
            }
        }
    }
    let re_appid = Regex::new(r#""appid"\s+"(\d+)""#).unwrap();
    let re_name = Regex::new(r#""name"\s+"([^"]+)""#).unwrap();
    let re_dir = Regex::new(r#""installdir"\s+"([^"]+)""#).unwrap();
    let mut seen = HashSet::new();
    for lib in libraries {
        let apps = lib.join("steamapps");
        let Ok(rd) = std::fs::read_dir(&apps) else { continue };
        for entry in rd.flatten() {
            let p = entry.path();
            let fname = p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
            if !fname.starts_with("appmanifest_") || !fname.ends_with(".acf") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&p) else { continue };
            let Some(appid) = re_appid.captures(&text).and_then(|c| c[1].parse::<u32>().ok()) else { continue };
            if !seen.insert(appid) {
                continue;
            }
            let name = re_name.captures(&text).map(|c| c[1].to_string()).unwrap_or_else(|| format!("Steam app {appid}"));
            let installdir = re_dir.captures(&text).map(|c| c[1].to_string()).unwrap_or_default();
            let lower = name.to_lowercase();
            if appid == 228980 || lower.contains("redistributable") || lower.contains("runtime") || lower.contains("proton") || lower.contains("steamworks") || lower.contains("dedicated server") {
                continue;
            }
            let install = apps.join("common").join(&installdir);
            if !install.is_dir() {
                continue;
            }
            let k = known_by_steam(appid);
            let install_s = install.to_string_lossy().to_string();
            let exe_path = resolve_exe(&install_s, k, k.and_then(|k| k.steam_exe));
            games.push(Game {
                id: k.map(|k| format!("steam:{}", k.key)).unwrap_or_else(|| format!("steam:{appid}")),
                name: k.map(|k| k.name.to_string()).unwrap_or(name),
                launcher: Launcher::Steam,
                install_path: Some(install_s),
                process_names: procs_for(k, exe_path.as_deref()),
                exe_path,
                launch_uri: Some(format!("steam://rungameid/{appid}")),
                known: k.is_some(),
                icon_key: k.map(|k| k.key.to_string()),
                steam_app_id: Some(appid),
                cover: None,
                icon: None,
            });
        }
    }
    games
}

// ---------- Riot ----------

fn riot_client() -> Option<String> {
    let p = Path::new("C:\\ProgramData\\Riot Games\\RiotClientInstalls.json");
    let text = std::fs::read_to_string(p).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    for key in ["rc_default", "rc_live", "rc_beta"] {
        if let Some(s) = v.get(key).and_then(|x| x.as_str()) {
            if Path::new(s).is_file() {
                return Some(s.to_string());
            }
        }
    }
    None
}

fn detect_riot() -> Vec<Game> {
    let yaml = Path::new("C:\\ProgramData\\Riot Games\\Metadata\\valorant.live\\valorant.live.product_settings.yaml");
    let Ok(text) = std::fs::read_to_string(yaml) else { return vec![] };
    let Some(line) = text.lines().find(|l| l.trim_start().starts_with("product_install_full_path:")) else { return vec![] };
    let raw = line.split_once(':').map(|(_, v)| v.trim().trim_matches('"').to_string()).unwrap_or_default();
    let install = raw.replace('/', "\\");
    if install.is_empty() || !Path::new(&install).is_dir() {
        return vec![];
    }
    let exe = existing(join(&install, "ShooterGame\\Binaries\\Win64\\VALORANT-Win64-Shipping.exe"));
    vec![Game {
        id: "riot:valorant".into(),
        name: "VALORANT".into(),
        launcher: Launcher::Riot,
        install_path: Some(install),
        exe_path: exe,
        process_names: vec!["VALORANT-Win64-Shipping.exe".into()],
        launch_uri: None,
        known: true,
        icon_key: Some("valorant".into()),
        steam_app_id: None,
        cover: None,
        icon: None,
    }]
}

// ---------- Battle.net / EA via Uninstall-register ----------

fn launcher_slug(l: Launcher) -> &'static str {
    match l {
        Launcher::Epic => "epic",
        Launcher::Steam => "steam",
        Launcher::Battlenet => "battlenet",
        Launcher::Riot => "riot",
        Launcher::Ea => "ea",
        Launcher::Xbox => "xbox",
        Launcher::Minecraft => "minecraft",
        Launcher::Custom => "custom",
    }
}

fn detect_uninstall() -> Vec<Game> {
    let mut games = vec![];
    let roots = [
        (Hive::Hklm, "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall"),
        (Hive::Hklm, "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall"),
        (Hive::Hkcu, "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall"),
    ];
    let mut seen = HashSet::new();
    for (hive, root) in roots {
        let Ok(keys) = registry::subkeys(hive, root) else { continue };
        for key in keys {
            if key.starts_with("Steam App") {
                continue;
            }
            let path = format!("{root}\\{key}");
            let display = match registry::read(hive, &path, "DisplayName") {
                Ok(RegData::Str(s)) => s,
                _ => continue,
            };
            let Some(k) = known_by_uninstall(&display) else { continue };
            if !seen.insert(k.key) {
                continue;
            }
            let install = match registry::read(hive, &path, "InstallLocation") {
                Ok(RegData::Str(s)) if !s.is_empty() => s.trim_matches('"').to_string(),
                _ => continue,
            };
            if !Path::new(&install).is_dir() {
                continue;
            }
            let exe_path = resolve_exe(&install, Some(k), k.uninstall_exe);
            games.push(Game {
                id: format!("{}:{}", launcher_slug(k.uninstall_launcher), k.key),
                name: k.name.to_string(),
                launcher: k.uninstall_launcher,
                install_path: Some(install),
                exe_path,
                process_names: k.procs.iter().map(|p| p.to_string()).collect(),
                launch_uri: None,
                known: true,
                icon_key: Some(k.key.to_string()),
                steam_app_id: k.steam_appids.first().copied(),
                cover: None,
                icon: None,
            });
        }
    }
    games
}

// ---------- Xbox-app (XboxGames-map) ----------

fn detect_xbox() -> Vec<Game> {
    let mut games = vec![];
    let re_exe = Regex::new(r#"<Executable[^>]*\sName="([^"]+)""#).unwrap();
    let re_name = Regex::new(r#"DefaultDisplayName="([^"]+)""#).unwrap();
    for letter in b'C'..=b'Z' {
        let root = PathBuf::from(format!("{}:\\XboxGames", letter as char));
        let Ok(rd) = std::fs::read_dir(&root) else { continue };
        for entry in rd.flatten() {
            let content = entry.path().join("Content");
            let Ok(text) = std::fs::read_to_string(content.join("MicrosoftGame.config")) else { continue };
            let folder = entry.file_name().to_string_lossy().to_string();
            let mut name = re_name.captures(&text).map(|c| c[1].to_string()).unwrap_or_else(|| folder.clone());
            if name.starts_with("ms-resource") {
                name = folder.clone();
            }
            // Launchers zijn geen games.
            if name.to_lowercase().contains("launcher") {
                continue;
            }
            let exe_rel = re_exe.captures(&text).map(|c| c[1].to_string());
            let exe_path = exe_rel.as_deref().and_then(|rel| existing(content.join(rel).to_string_lossy().to_string()));
            let lower = name.to_lowercase();
            let is_bedrock = exe_rel.as_deref().map(|e| e.eq_ignore_ascii_case("Minecraft.Windows.exe")).unwrap_or(false);
            let k = KNOWN.iter().find(|k| lower.contains(&k.name.to_lowercase()));
            games.push(Game {
                id: if is_bedrock { "minecraft:bedrock".into() } else { format!("xbox:{}", folder.to_lowercase().replace(' ', "-")) },
                name: if is_bedrock { "Minecraft (Bedrock)".into() } else { k.map(|k| k.name.to_string()).unwrap_or(name) },
                launcher: Launcher::Xbox,
                install_path: Some(content.to_string_lossy().to_string()),
                exe_path: exe_path.clone(),
                process_names: k
                    .map(|k| k.procs.iter().map(|p| p.to_string()).collect())
                    .unwrap_or_else(|| exe_rel.as_deref().map(|e| vec![file_name(e)]).unwrap_or_default()),
                launch_uri: if is_bedrock { Some("minecraft://".into()) } else { None },
                known: k.is_some() || is_bedrock,
                icon_key: if is_bedrock { Some("minecraft".into()) } else { k.map(|k| k.key.to_string()) },
                steam_app_id: k.and_then(|k| k.steam_appids.first().copied()),
                cover: None,
                icon: None,
            });
        }
    }
    games
}

// ---------- Minecraft ----------

fn detect_minecraft(already_bedrock: bool) -> Vec<Game> {
    let mut games = vec![];
    if let Some(appdata) = std::env::var_os("APPDATA").map(PathBuf::from) {
        let mc = appdata.join(".minecraft");
        if mc.is_dir() {
            // De Java-runtime staat in .minecraft\runtime of (Store-launcher) in de pakketmap.
            let mut roots = vec![mc.join("runtime")];
            if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
                roots.push(local.join("Packages\\Microsoft.4297127D64EC6_8wekyb3d8bbwe\\LocalCache\\Local\\runtime"));
            }
            let exe = roots.iter().find_map(|r| find_exe(r, "javaw.exe", 6));
            games.push(Game {
                id: "minecraft:java".into(),
                name: "Minecraft (Java Edition)".into(),
                launcher: Launcher::Minecraft,
                install_path: Some(mc.to_string_lossy().to_string()),
                exe_path: exe,
                process_names: vec!["javaw.exe".into()],
                launch_uri: None,
                known: true,
                icon_key: Some("minecraft".into()),
                steam_app_id: None,
                cover: None,
                icon: None,
            });
        }
    }
    if !already_bedrock {
        if let Ok(out) = run_ps("(Get-AppxPackage -Name Microsoft.MinecraftUWP -ErrorAction SilentlyContinue | Select-Object -First 1).InstallLocation") {
            let loc = out.trim().to_string();
            if !loc.is_empty() && Path::new(&loc).is_dir() {
                games.push(Game {
                    id: "minecraft:bedrock".into(),
                    name: "Minecraft (Bedrock)".into(),
                    launcher: Launcher::Xbox,
                    install_path: Some(loc.clone()),
                    exe_path: existing(join(&loc, "Minecraft.Windows.exe")),
                    process_names: vec!["Minecraft.Windows.exe".into()],
                    launch_uri: Some("minecraft://".into()),
                    known: true,
                    icon_key: Some("minecraft".into()),
                    steam_app_id: None,
                    cover: None,
                    icon: None,
                });
            }
        }
    }
    games
}

/// Alle detectoren samen, ontdubbeld op id en exe. Bekende games eerst.
pub fn detect_all() -> Vec<Game> {
    let mut all = vec![];
    all.extend(detect_epic());
    all.extend(detect_steam());
    all.extend(detect_riot());
    all.extend(detect_uninstall());
    let xbox = detect_xbox();
    let has_bedrock = xbox.iter().any(|g| g.id == "minecraft:bedrock");
    all.extend(xbox);
    all.extend(detect_minecraft(has_bedrock));
    let mut seen_id = HashSet::new();
    let mut seen_exe = HashSet::new();
    all.retain(|g| {
        if !seen_id.insert(g.id.clone()) {
            return false;
        }
        match &g.exe_path {
            Some(e) => seen_exe.insert(e.to_lowercase()),
            None => true,
        }
    });
    all.sort_by(|a, b| b.known.cmp(&a.known).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    all
}

/// Maakt een Game voor een door de gebruiker gekozen .exe.
pub fn custom_game(exe_path: &str, name: Option<&str>) -> AppResult<Game> {
    let p = Path::new(exe_path);
    if !p.is_file() {
        return Err(AppError::NotFound(exe_path.to_string()));
    }
    let file = file_name(exe_path);
    let lower = file.to_lowercase();
    let k = KNOWN.iter().find(|k| k.procs.iter().any(|pn| pn.to_lowercase() == lower));
    let title = crate::icons::file_description(exe_path).unwrap_or_else(|| p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| file.clone()));
    Ok(Game {
        id: format!("custom:{}", uuid::Uuid::new_v4()),
        name: name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).unwrap_or_else(|| k.map(|k| k.name.to_string()).unwrap_or(title)),
        launcher: Launcher::Custom,
        install_path: p.parent().map(|d| d.to_string_lossy().to_string()),
        exe_path: Some(exe_path.to_string()),
        process_names: vec![file],
        launch_uri: None,
        known: false,
        icon_key: k.map(|k| k.key.to_string()),
        steam_app_id: k.and_then(|k| k.steam_appids.first().copied()),
        cover: None,
        icon: None,
    })
}

/// Start een game via de launcher-URI of direct via de exe.
pub fn launch(game: &Game) -> AppResult<()> {
    match game.launcher {
        Launcher::Riot => {
            let client = riot_client().ok_or_else(|| AppError::NotFound("Riot Client".into()))?;
            spawn_detached(&client, &["--launch-product=valorant", "--launch-patchline=live"], None)
        }
        Launcher::Minecraft => {
            let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
            let candidates = [
                "C:\\Program Files (x86)\\Minecraft Launcher\\MinecraftLauncher.exe".to_string(),
                "C:\\Program Files\\Minecraft Launcher\\MinecraftLauncher.exe".to_string(),
                format!("{local}\\Programs\\Minecraft Launcher\\MinecraftLauncher.exe"),
            ];
            match candidates.iter().find(|c| Path::new(c).is_file()) {
                Some(c) => spawn_detached(c, &[], None),
                None => open_uri("minecraft-launcher://"),
            }
        }
        Launcher::Xbox => {
            if let Some(uri) = &game.launch_uri {
                return open_uri(uri);
            }
            let helper = game.install_path.as_deref().map(|d| join(d, "gamelaunchhelper.exe"));
            match helper.filter(|h| Path::new(h).is_file()) {
                Some(h) => spawn_detached(&h, &[], game.install_path.as_deref()),
                None => launch_exe(game),
            }
        }
        _ => match &game.launch_uri {
            Some(uri) => open_uri(uri),
            None => launch_exe(game),
        },
    }
}

fn launch_exe(game: &Game) -> AppResult<()> {
    let exe = game.exe_path.as_deref().ok_or_else(|| AppError::coded("no_exe").with("game", &game.name))?;
    spawn_detached(exe, &[], Path::new(exe).parent().map(|p| p.to_string_lossy().to_string()).as_deref())
}

fn open_uri(uri: &str) -> AppResult<()> {
    // `start` via cmd laat Windows de protocol-handler kiezen.
    spawn_detached("cmd.exe", &["/C", "start", "", uri], None)
}
