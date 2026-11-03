//! Netwerk: game-regio's om te pingen, verbindingsoverzicht en DNS-cache legen.

use crate::error::AppResult;
use crate::models::{AdapterKind, ConnectionOverview, GameRegion, NetworkAdapter};
use crate::powershell::run_cmd;
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

/// Tekst uit een Windows-tekenreeks die met 0 eindigt.
fn pwstr(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut len = 0;
    unsafe {
        while *p.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
    }
}

/// IPv4-adres uit een SOCKET_ADDRESS, of None voor IPv6 en lege adressen.
fn ipv4(a: &windows_sys::Win32::Networking::WinSock::SOCKET_ADDRESS) -> Option<String> {
    use windows_sys::Win32::Networking::WinSock::AF_INET;
    if a.lpSockaddr.is_null() {
        return None;
    }
    let sa = unsafe { &*a.lpSockaddr };
    if sa.sa_family != AF_INET {
        return None;
    }
    // SOCKADDR_IN: 2 bytes poort, daarna de 4 bytes van het adres.
    let b: Vec<u8> = sa.sa_data[2..6].iter().map(|x| *x as u8).collect();
    Some(format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3]))
}

/// Fysieke netwerkadapters (zoals Get-NetAdapter -Physical) met snelheid, IPv4, gateway en DNS.
pub fn adapters() -> Vec<NetworkAdapter> {
    scan().into_iter().map(|s| s.adapter).collect()
}

/// Een gevonden adapter.
pub struct ScannedAdapter {
    pub adapter: NetworkAdapter,
}

/// Rechtstreeks via GetAdaptersAddresses en GetIfEntry2, zonder PowerShell.
pub fn scan() -> Vec<ScannedAdapter> {
    use windows_sys::Win32::Foundation::ERROR_BUFFER_OVERFLOW;
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetAdaptersAddresses, GetIfEntry2, GAA_FLAG_INCLUDE_GATEWAYS, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_MULTICAST, IF_TYPE_ETHERNET_CSMACD, IF_TYPE_IEEE80211, IP_ADAPTER_ADDRESSES_LH, MIB_IF_ROW2,
    };
    use windows_sys::Win32::NetworkManagement::Ndis::{IfOperStatusUp, NdisPhysicalMedium802_3, NdisPhysicalMediumNative802_11, NdisPhysicalMediumWirelessLan};
    use windows_sys::Win32::Networking::WinSock::AF_UNSPEC;

    let flags = GAA_FLAG_INCLUDE_GATEWAYS | GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST;
    let mut size: u32 = 16 * 1024;
    let mut buf: Vec<u64> = Vec::new();
    let mut ret = ERROR_BUFFER_OVERFLOW;
    for _ in 0..3 {
        // u64 zodat de structuren goed uitgelijnd liggen.
        buf = vec![0u64; (size as usize).div_ceil(8)];
        ret = unsafe { GetAdaptersAddresses(AF_UNSPEC as u32, flags, std::ptr::null(), buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH, &mut size) };
        if ret != ERROR_BUFFER_OVERFLOW {
            break;
        }
    }
    if ret != 0 {
        log::warn!("netwerkadapters lezen: fout {ret}");
        return vec![];
    }
    let mut out = vec![];
    let mut p = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
    while !p.is_null() {
        let a = unsafe { &*p };
        p = a.Next;
        // Alleen echte hardware met een aansluiting: geen loopback, VPN, Hyper-V of wifi-direct.
        let mut row: MIB_IF_ROW2 = unsafe { std::mem::zeroed() };
        row.InterfaceLuid = a.Luid;
        if unsafe { GetIfEntry2(&mut row) } != 0 {
            continue;
        }
        let bits = row.InterfaceAndOperStatusFlags._bitfield;
        let (hardware, connector, not_connected) = (bits & 1 != 0, bits & 4 != 0, bits & 16 != 0);
        if !hardware || !connector {
            continue;
        }
        let medium = row.PhysicalMediumType;
        let kind = if medium == NdisPhysicalMediumNative802_11 || medium == NdisPhysicalMediumWirelessLan || a.IfType == IF_TYPE_IEEE80211 {
            AdapterKind::Wifi
        } else if medium == NdisPhysicalMedium802_3 || a.IfType == IF_TYPE_ETHERNET_CSMACD {
            AdapterKind::Ethernet
        } else {
            AdapterKind::Other
        };
        let up = a.OperStatus == IfOperStatusUp && !not_connected;
        let speed = a.TransmitLinkSpeed;
        let link_speed_mbps = (up && speed > 0 && speed != u64::MAX).then(|| (speed as f64 / 100_000.0).round() / 10.0);
        let mut ip = None;
        let mut u = a.FirstUnicastAddress;
        while ip.is_none() && !u.is_null() {
            let ua = unsafe { &*u };
            ip = ipv4(&ua.Address);
            u = ua.Next;
        }
        let mut gateway = None;
        let mut g = a.FirstGatewayAddress;
        while gateway.is_none() && !g.is_null() {
            let ga = unsafe { &*g };
            gateway = ipv4(&ga.Address);
            g = ga.Next;
        }
        let mut dns = vec![];
        let mut d = a.FirstDnsServerAddress;
        while !d.is_null() {
            let da = unsafe { &*d };
            if let Some(x) = ipv4(&da.Address) {
                dns.push(x);
            }
            d = da.Next;
        }
        let mac_len = (a.PhysicalAddressLength as usize).min(a.PhysicalAddress.len());
        let mac = (mac_len > 0).then(|| a.PhysicalAddress[..mac_len].iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join("-"));
        out.push(ScannedAdapter {
            adapter: NetworkAdapter {
                name: pwstr(a.FriendlyName),
                description: pwstr(a.Description),
                kind,
                status: if up { "Up".into() } else { "Disconnected".into() },
                link_speed_mbps,
                ipv4: ip,
                gateway,
                dns,
                mac,
            },
        });
    }
    out
}

pub fn connection_overview() -> AppResult<ConnectionOverview> {
    let mut adapters = adapters();
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
