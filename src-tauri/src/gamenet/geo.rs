//! Waar staat een server? Via de officiële, openbare IP-lijsten van Amazon AWS (ip-ranges.json), Google Cloud
//! (cloud.json) en Valve (de Steam Datagram Relay-lijst van CS2). Geen gok en geen externe locatiedienst: staat een
//! adres er niet in, dan zegt nezty dat eerlijk. De lijsten worden een week bewaard in de datamap.

use crate::models::{now_ms, NetRegion, ServerLocation};
use crate::state::{lock, AppState};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const AWS_URL: &str = "https://ip-ranges.amazonaws.com/ip-ranges.json";
const GCP_URL: &str = "https://www.gstatic.com/ipranges/cloud.json";
const VALVE_URL: &str = "https://api.steampowered.com/ISteamApps/GetSDRConfig/v1/?appid=730";
const CACHE_FILE: &str = "net-ranges.json";
const MAX_AGE_MS: i64 = 7 * 24 * 3600 * 1000;
/// Lukte het ophalen maar half (bv. even geen internet), dan na een uur opnieuw proberen.
const RETRY_MS: i64 = 3600 * 1000;

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct Ranges {
    /// Wanneer opnieuw ophalen (ms).
    refresh_after_ms: i64,
    v4: Vec<Range>,
    valve: Vec<ValvePop>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Range {
    net: u32,
    len: u8,
    provider: String,
    region: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct ValvePop {
    code: String,
    desc: String,
    lat: f64,
    lon: f64,
    relays: Vec<String>,
}

static CACHE: Mutex<Option<Arc<Ranges>>> = Mutex::new(None);
static FETCHING: Mutex<()> = Mutex::new(());

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .tls_config(ureq::tls::TlsConfig::builder().provider(ureq::tls::TlsProvider::NativeTls).root_certs(ureq::tls::RootCerts::PlatformVerifier).build())
        .timeout_global(Some(Duration::from_secs(20)))
        .http_status_as_error(true)
        .build()
        .into()
}

fn get_json(url: &str) -> Option<serde_json::Value> {
    let mut r = agent().get(url).header("User-Agent", "nezty").call().ok()?;
    let text = r.body_mut().with_config().limit(16 * 1024 * 1024).read_to_string().ok()?;
    serde_json::from_str(&text).ok()
}

fn mask(len: u8) -> u32 {
    if len == 0 {
        0
    } else {
        u32::MAX << (32 - len as u32)
    }
}

fn parse_v4_prefix(s: &str) -> Option<(u32, u8)> {
    let (a, l) = s.split_once('/')?;
    let ip: Ipv4Addr = a.parse().ok()?;
    let len: u8 = l.parse().ok()?;
    (len <= 32).then(|| (u32::from(ip) & mask(len), len))
}

fn fetch() -> Ranges {
    let mut out = Ranges::default();
    let mut complete = true;
    match get_json(AWS_URL) {
        // Gameservers draaien op EC2; de rest van AWS (opslag, CloudFront) is geen gameserver.
        Some(v) => {
            for p in v["prefixes"].as_array().into_iter().flatten().filter(|p| p["service"] == "EC2") {
                let region = p["region"].as_str().unwrap_or_default();
                if let (false, Some((net, len))) = (region.is_empty() || region == "GLOBAL", p["ip_prefix"].as_str().and_then(parse_v4_prefix)) {
                    out.v4.push(Range { net, len, provider: "aws".into(), region: region.into() });
                }
            }
        }
        None => complete = false,
    }
    match get_json(GCP_URL) {
        Some(v) => {
            for p in v["prefixes"].as_array().into_iter().flatten() {
                let scope = p["scope"].as_str().unwrap_or_default();
                if let (false, Some((net, len))) = (scope.is_empty() || scope == "global", p["ipv4Prefix"].as_str().and_then(parse_v4_prefix)) {
                    out.v4.push(Range { net, len, provider: "gcp".into(), region: scope.into() });
                }
            }
        }
        None => complete = false,
    }
    match get_json(VALVE_URL).as_ref().and_then(|v| v["pops"].as_object()) {
        Some(pops) => {
            for (code, p) in pops {
                let relays: Vec<String> = p["relays"].as_array().into_iter().flatten().filter_map(|r| r["ipv4"].as_str().map(String::from)).collect();
                if relays.is_empty() {
                    continue;
                }
                // "geo" is [lengtegraad, breedtegraad].
                let geo = p["geo"].as_array();
                let lon = geo.and_then(|g| g.first()).and_then(|x| x.as_f64()).unwrap_or(0.0);
                let lat = geo.and_then(|g| g.get(1)).and_then(|x| x.as_f64()).unwrap_or(0.0);
                out.valve.push(ValvePop { code: code.clone(), desc: p["desc"].as_str().unwrap_or(code).to_string(), lat, lon, relays });
            }
            out.valve.sort_by(|a, b| a.desc.cmp(&b.desc));
        }
        None => complete = false,
    }
    out.refresh_after_ms = now_ms() + if complete { MAX_AGE_MS } else { RETRY_MS };
    out
}

/// De lijsten: uit het geheugen, van schijf, of vers opgehaald als ze ouder zijn dan een week.
fn ranges(state: &AppState) -> Arc<Ranges> {
    let fresh = |r: &Ranges| now_ms() < r.refresh_after_ms;
    if let Some(r) = lock(&CACHE).as_ref().filter(|r| fresh(r)) {
        return r.clone();
    }
    let _one_at_a_time = lock(&FETCHING);
    if let Some(r) = lock(&CACHE).as_ref().filter(|r| fresh(r)) {
        return r.clone();
    }
    let path = state.data_dir.join(CACHE_FILE);
    let disk: Option<Ranges> = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok());
    let r = match disk {
        Some(d) if fresh(&d) => d,
        old => {
            let new = fetch();
            if new.v4.is_empty() && new.valve.is_empty() {
                // Geen internet: de oude lijst blijft bruikbaar, over een uur opnieuw proberen.
                let mut keep = old.unwrap_or_default();
                keep.refresh_after_ms = now_ms() + RETRY_MS;
                keep
            } else {
                if let Ok(text) = serde_json::to_string(&new) {
                    let _ = crate::state::write_atomic(&path, text.as_bytes());
                }
                new
            }
        }
    };
    let r = Arc::new(r);
    *lock(&CACHE) = Some(r.clone());
    r
}

/// "Amsterdam (Netherlands)" wordt ("Amsterdam", "Netherlands").
fn split_desc(desc: &str) -> (String, Option<String>) {
    match desc.rsplit_once(" (") {
        Some((city, rest)) => (city.trim().to_string(), Some(rest.trim_end_matches(')').trim().to_string())),
        None => (desc.trim().to_string(), None),
    }
}

/// Spelregio (zoals games ze noemen) bij een plek op de kaart.
fn group_of(lat: f64, lon: f64) -> &'static str {
    if lat > 34.0 && (-30.0..=60.0).contains(&lon) {
        "eu"
    } else if lon < -30.0 && lat > 15.0 {
        if lon > -85.0 {
            "na_east"
        } else if lon > -105.0 {
            "na_central"
        } else {
            "na_west"
        }
    } else if lon < -30.0 {
        "brazil"
    } else if lat < 0.0 && lon > 110.0 {
        "oceania"
    } else if lat < 10.0 && lon < 55.0 {
        "africa"
    } else if (30.0..=63.0).contains(&lon) {
        "middle_east"
    } else {
        "asia"
    }
}

/// Stad en land van een AWS- of Google Cloud-regio.
fn place(provider: &str, region: &str) -> Option<(&'static str, &'static str)> {
    Some(match (provider, region) {
        ("aws", "us-east-1") => ("Virginia", "United States"),
        ("aws", "us-east-2") => ("Ohio", "United States"),
        ("aws", "us-west-1") => ("California", "United States"),
        ("aws", "us-west-2") => ("Oregon", "United States"),
        ("aws", "ca-central-1") => ("Montreal", "Canada"),
        ("aws", "ca-west-1") => ("Calgary", "Canada"),
        ("aws", "sa-east-1") => ("São Paulo", "Brazil"),
        ("aws", "eu-central-1") => ("Frankfurt", "Germany"),
        ("aws", "eu-central-2") => ("Zurich", "Switzerland"),
        ("aws", "eu-west-1") => ("Dublin", "Ireland"),
        ("aws", "eu-west-2") => ("London", "United Kingdom"),
        ("aws", "eu-west-3") => ("Paris", "France"),
        ("aws", "eu-north-1") => ("Stockholm", "Sweden"),
        ("aws", "eu-south-1") => ("Milan", "Italy"),
        ("aws", "eu-south-2") => ("Aragón", "Spain"),
        ("aws", "me-south-1") => ("Bahrain", "Bahrain"),
        ("aws", "me-central-1") => ("UAE", "United Arab Emirates"),
        ("aws", "il-central-1") => ("Tel Aviv", "Israel"),
        ("aws", "af-south-1") => ("Cape Town", "South Africa"),
        ("aws", "ap-east-1") => ("Hong Kong", "China"),
        ("aws", "ap-south-1") => ("Mumbai", "India"),
        ("aws", "ap-northeast-1") => ("Tokyo", "Japan"),
        ("aws", "ap-northeast-2") => ("Seoul", "South Korea"),
        ("aws", "ap-northeast-3") => ("Osaka", "Japan"),
        ("aws", "ap-southeast-1") => ("Singapore", "Singapore"),
        ("aws", "ap-southeast-2") => ("Sydney", "Australia"),
        ("aws", "ap-southeast-3") => ("Jakarta", "Indonesia"),
        ("gcp", "europe-west1") => ("St. Ghislain", "Belgium"),
        ("gcp", "europe-west2") => ("London", "United Kingdom"),
        ("gcp", "europe-west3") => ("Frankfurt", "Germany"),
        ("gcp", "europe-west4") => ("Eemshaven", "Netherlands"),
        ("gcp", "europe-west6") => ("Zurich", "Switzerland"),
        ("gcp", "europe-west8") => ("Milan", "Italy"),
        ("gcp", "europe-west9") => ("Paris", "France"),
        ("gcp", "europe-west10") => ("Berlin", "Germany"),
        ("gcp", "europe-west12") => ("Turin", "Italy"),
        ("gcp", "europe-north1") => ("Hamina", "Finland"),
        ("gcp", "europe-north2") => ("Stockholm", "Sweden"),
        ("gcp", "europe-central2") => ("Warsaw", "Poland"),
        ("gcp", "europe-southwest1") => ("Madrid", "Spain"),
        ("gcp", "us-central1") => ("Iowa", "United States"),
        ("gcp", "us-east1") => ("South Carolina", "United States"),
        ("gcp", "us-east4") => ("Virginia", "United States"),
        ("gcp", "us-east5") => ("Columbus", "United States"),
        ("gcp", "us-south1") => ("Dallas", "United States"),
        ("gcp", "us-west1") => ("Oregon", "United States"),
        ("gcp", "us-west2") => ("Los Angeles", "United States"),
        ("gcp", "us-west3") => ("Salt Lake City", "United States"),
        ("gcp", "us-west4") => ("Las Vegas", "United States"),
        ("gcp", "northamerica-northeast1") => ("Montreal", "Canada"),
        ("gcp", "northamerica-northeast2") => ("Toronto", "Canada"),
        ("gcp", "southamerica-east1") => ("São Paulo", "Brazil"),
        ("gcp", "asia-east1") => ("Changhua", "Taiwan"),
        ("gcp", "asia-east2") => ("Hong Kong", "China"),
        ("gcp", "asia-northeast1") => ("Tokyo", "Japan"),
        ("gcp", "asia-northeast2") => ("Osaka", "Japan"),
        ("gcp", "asia-northeast3") => ("Seoul", "South Korea"),
        ("gcp", "asia-south1") => ("Mumbai", "India"),
        ("gcp", "asia-southeast1") => ("Singapore", "Singapore"),
        ("gcp", "australia-southeast1") => ("Sydney", "Australia"),
        ("gcp", "me-west1") => ("Tel Aviv", "Israel"),
        ("gcp", "me-central1") => ("Doha", "Qatar"),
        ("gcp", "africa-south1") => ("Johannesburg", "South Africa"),
        _ => return None,
    })
}

/// Waar staat dit adres? None als het in geen van de officiële lijsten staat.
pub fn locate(state: &AppState, ip: IpAddr) -> Option<ServerLocation> {
    let IpAddr::V4(v4) = ip else { return None };
    let r = ranges(state);
    let s = v4.to_string();
    // Valve: precies een relay-adres, of hetzelfde /24-blok als een relay van die locatie.
    let block = |a: &str| a.rsplit_once('.').map(|(b, _)| b.to_string());
    let own = block(&s);
    let pop = r.valve.iter().find(|p| p.relays.contains(&s)).or_else(|| r.valve.iter().find(|p| p.relays.iter().any(|x| block(x) == own)));
    if let Some(p) = pop {
        let (city, country) = split_desc(&p.desc);
        return Some(ServerLocation { provider: "valve".into(), region: p.code.clone(), city: Some(city), country });
    }
    let n = u32::from(v4);
    let best = r.v4.iter().filter(|x| n & mask(x.len) == x.net).max_by_key(|x| x.len)?;
    let (city, country) = match place(&best.provider, &best.region) {
        Some((c, k)) => (Some(c.to_string()), Some(k.to_string())),
        None => (None, None),
    };
    Some(ServerLocation { provider: best.provider.clone(), region: best.region.clone(), city, country })
}

/// Datacenters om naartoe te meten: de AWS-regio's (waar veel shooters draaien) en alle Valve-locaties.
pub fn regions(state: &AppState) -> Vec<NetRegion> {
    let mut out: Vec<NetRegion> = crate::netinfo::game_regions()
        .into_iter()
        .map(|g| NetRegion { id: format!("aws-{}", g.id), provider: "aws".into(), group: g.region, city: g.city, country: None, host: g.host })
        .collect();
    for p in &ranges(state).valve {
        let (city, country) = split_desc(&p.desc);
        out.push(NetRegion { id: format!("valve-{}", p.code), provider: "valve".into(), group: group_of(p.lat, p.lon).into(), city, country, host: p.relays[0].clone() });
    }
    out
}
