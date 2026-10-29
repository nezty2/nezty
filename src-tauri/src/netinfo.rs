//! Netwerk: game-regio's om te pingen, verbindingsoverzicht en DNS-cache legen.

use crate::error::AppResult;
use crate::models::{AdapterKind, ConnectionOverview, GameRegion, NetworkAdapter};
use crate::powershell::{run_cmd, run_ps};
use serde::Deserialize;
use std::collections::BTreeMap;

/// Datacenters waar de grote shooters (Fortnite, Apex, Rocket League, ook CoD en andere) op draaien.
/// Game-servers publiceren geen vaste IP's; dit zijn de cloud-regio's, gemeten via een TCP-handshake.
pub fn game_regions() -> Vec<GameRegion> {
    let r = |id: &str, region: &str, city: &str, aws: &str| GameRegion {
        id: id.into(),
        region: region.into(),
        city: city.into(),
        host: format!("ec2.{aws}.amazonaws.com:443"),
    };
    vec![
        r("eu-central", "eu", "Frankfurt", "eu-central-1"),
        r("eu-west-2", "eu", "London", "eu-west-2"),
        r("eu-west-1", "eu", "Dublin", "eu-west-1"),
        r("eu-west-3", "eu", "Paris", "eu-west-3"),
        r("us-east-1", "na_east", "Virginia", "us-east-1"),
        r("us-east-2", "na_central", "Ohio", "us-east-2"),
        r("us-west-1", "na_west", "California", "us-west-1"),
        r("sa-east-1", "brazil", "São Paulo", "sa-east-1"),
        r("me-south-1", "middle_east", "Bahrain", "me-south-1"),
        r("ap-northeast-1", "asia", "Tokyo", "ap-northeast-1"),
        r("ap-southeast-2", "oceania", "Sydney", "ap-southeast-2"),
    ]
}

#[derive(Deserialize)]
struct PsAdapter {
    name: Option<String>,
    description: Option<String>,
    media: Option<String>,
    status: Option<String>,
    speed: Option<String>,
    ipv4: Option<String>,
    gateway: Option<String>,
    #[serde(default)]
    dns: Vec<String>,
    mac: Option<String>,
}

fn parse_speed(s: &str) -> Option<f64> {
    let s = s.trim().to_lowercase();
    let (num, unit) = s.split_once(' ')?;
    let n: f64 = num.parse().ok()?;
    let mbps = match unit {
        "gbps" => n * 1000.0,
        "mbps" => n,
        "kbps" => n / 1000.0,
        "bps" => n / 1_000_000.0,
        _ => return None,
    };
    if mbps <= 0.0 {
        None
    } else {
        Some(mbps)
    }
}

pub fn connection_overview() -> AppResult<ConnectionOverview> {
    let script = r#"
$list = Get-NetAdapter -Physical -ErrorAction SilentlyContinue | ForEach-Object {
  $a = $_
  $ip = Get-NetIPConfiguration -InterfaceIndex $a.ifIndex -ErrorAction SilentlyContinue
  $dns = @()
  if ($ip -and $ip.DNSServer) { $dns = @($ip.DNSServer | Where-Object { $_.AddressFamily -eq 2 } | ForEach-Object { $_.ServerAddresses } | ForEach-Object { [string]$_ }) }
  [pscustomobject]@{
    name = [string]$a.Name; description = [string]$a.InterfaceDescription; media = [string]$a.PhysicalMediaType;
    status = [string]$a.Status; speed = [string]$a.LinkSpeed;
    ipv4 = if ($ip -and $ip.IPv4Address) { [string]($ip.IPv4Address | Select-Object -First 1).IPAddress } else { $null };
    gateway = if ($ip -and $ip.IPv4DefaultGateway) { [string]($ip.IPv4DefaultGateway | Select-Object -First 1).NextHop } else { $null };
    dns = $dns; mac = [string]$a.MacAddress
  }
}
ConvertTo-Json -InputObject @($list) -Compress -Depth 3
"#;
    let out = run_ps(script)?;
    let parsed: Vec<PsAdapter> = serde_json::from_str(out.trim()).unwrap_or_default();
    let mut adapters: Vec<NetworkAdapter> = parsed
        .into_iter()
        .map(|a| {
            let media = a.media.unwrap_or_default().to_lowercase();
            let kind = if media.contains("802.11") || media.contains("wireless") {
                AdapterKind::Wifi
            } else if media.contains("802.3") || media.contains("ethernet") {
                AdapterKind::Ethernet
            } else {
                AdapterKind::Other
            };
            NetworkAdapter {
                name: a.name.unwrap_or_default(),
                description: a.description.unwrap_or_default(),
                kind,
                status: a.status.unwrap_or_default(),
                link_speed_mbps: a.speed.as_deref().and_then(parse_speed),
                ipv4: a.ipv4.filter(|s| !s.is_empty()),
                gateway: a.gateway.filter(|s| !s.is_empty()),
                dns: a.dns.into_iter().filter(|s| !s.is_empty()).collect(),
                mac: a.mac.filter(|s| !s.is_empty()),
            }
        })
        .collect();
    adapters.sort_by_key(|a| if a.status.eq_ignore_ascii_case("Up") { 0 } else { 1 });

    let mut tcp_global = BTreeMap::new();
    if let Ok((ok, out, _)) = run_cmd("netsh", &["interface", "tcp", "show", "global"]) {
        if ok {
            for line in out.lines() {
                if let Some((k, v)) = line.split_once(':') {
                    let (k, v) = (k.trim(), v.trim());
                    if !k.is_empty() && !v.is_empty() && k.len() < 60 {
                        tcp_global.insert(k.to_string(), v.to_string());
                    }
                }
            }
        }
    }
    Ok(ConnectionOverview { adapters, tcp_global })
}

pub fn flush_dns() -> AppResult<()> {
    run_cmd("ipconfig", &["/flushdns"])?;
    Ok(())
}
