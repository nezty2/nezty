//! Met welke server praat je game echt? nezty leest via ETW (Event Tracing for Windows, provider
//! Microsoft-Windows-Kernel-Network) welke UDP-pakketten het gameproces verstuurt en ontvangt: alleen adres, poort
//! en aantal, nooit de inhoud. Dezelfde bron die Broncontrole gebruikt. Het gameproces zelf wordt niet aangeraakt
//! (geen handle, geen injectie), dus voor anti-cheat verandert er niets. Vereist beheerdersrechten.
//! De sessie stopt vanzelf als er een minuut niemand om vraagt, en altijd bij het afsluiten van nezty.
//!
//! Indeling van de events (Get-WinEvent -ListProvider Microsoft-Windows-Kernel-Network):
//! 42/43 = UDPv4 verzonden/ontvangen, 58/59 = UDPv6. Velden: PID u32, size u32, daddr, saddr, dport u16, sport u16
//! (poorten in netwerkvolgorde). Verzonden: daddr is de server. Ontvangen: saddr is de server.

use crate::error::{AppError, AppResult};
use crate::state::lock;
use crate::winapi::wide;
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::{LazyLock, Mutex, RwLock};
use std::time::{Duration, Instant};
use windows_sys::core::GUID;
use windows_sys::Win32::System::Diagnostics::Etw::{
    CloseTrace, ControlTraceW, EnableTraceEx2, OpenTraceW, ProcessTrace, StartTraceW, CONTROLTRACE_HANDLE, EVENT_CONTROL_CODE_ENABLE_PROVIDER, EVENT_RECORD,
    EVENT_TRACE_CONTROL_STOP, EVENT_TRACE_LOGFILEW, EVENT_TRACE_PROPERTIES, EVENT_TRACE_REAL_TIME_MODE, PROCESS_TRACE_MODE_EVENT_RECORD, PROCESS_TRACE_MODE_REAL_TIME,
    TRACE_LEVEL_INFORMATION, WNODE_FLAG_TRACED_GUID,
};

const SESSION: &str = "nezty-gamenet";
const KERNEL_NETWORK: GUID = GUID::from_u128(0x7dd42a49_5329_4832_8dfd_43d979153a88);
/// KERNEL_NETWORK_KEYWORD_IPV4 | KERNEL_NETWORK_KEYWORD_IPV6
const KEYWORDS: u64 = 0x10 | 0x20;
/// Zo lang niemand om de server vraagt: dan stopt de sessie.
const IDLE_STOP: Duration = Duration::from_secs(60);
/// Een verbinding die zo lang stil is, telt niet meer mee (bv. de server van het vorige potje).
const FORGET: Duration = Duration::from_secs(15);
/// Alleen verbindingen die nu nog actief zijn.
const RECENT: Duration = Duration::from_secs(3);

struct Flow {
    packets: u64,
    first: Instant,
    last: Instant,
}

/// De drukste verbinding van de game: bij een game in een potje is dat de gameserver.
pub struct Peer {
    pub ip: IpAddr,
    pub port: u16,
    pub packets_per_sec: f32,
}

struct Session {
    control: u64,
    props: Vec<u64>,
    started: Instant,
}

static SESSION_STATE: Mutex<Option<Session>> = Mutex::new(None);
static LAST_ASK: Mutex<Option<Instant>> = Mutex::new(None);
static WATCH: LazyLock<RwLock<HashSet<u32>>> = LazyLock::new(|| RwLock::new(HashSet::new()));
static FLOWS: LazyLock<Mutex<HashMap<(u32, IpAddr, u16), Flow>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// EVENT_TRACE_PROPERTIES met ruimte voor de sessienaam erachter, goed uitgelijnd.
fn new_props() -> Vec<u64> {
    let base = std::mem::size_of::<EVENT_TRACE_PROPERTIES>();
    let mut buf = vec![0u64; (base + 2 * (SESSION.len() + 1) + 16).div_ceil(8)];
    let total = (buf.len() * 8) as u32;
    unsafe {
        let p = buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;
        (*p).Wnode.BufferSize = total;
        (*p).Wnode.Flags = WNODE_FLAG_TRACED_GUID;
        // Tijdstempels via QueryPerformanceCounter.
        (*p).Wnode.ClientContext = 1;
        (*p).LogFileMode = EVENT_TRACE_REAL_TIME_MODE;
        (*p).BufferSize = 64;
        (*p).MinimumBuffers = 4;
        (*p).MaximumBuffers = 16;
        (*p).FlushTimer = 1;
        (*p).LoggerNameOffset = base as u32;
    }
    buf
}

/// Een sessie met dezelfde naam die nog loopt (bv. na een crash van nezty) stoppen. Ook bij het opstarten.
pub fn stop_by_name() {
    let mut props = new_props();
    let name = wide(SESSION);
    unsafe {
        ControlTraceW(CONTROLTRACE_HANDLE { Value: 0 }, name.as_ptr(), props.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES, EVENT_TRACE_CONTROL_STOP);
    }
}

/// Start de sessie als die nog niet loopt.
pub fn ensure_running() -> AppResult<()> {
    *lock(&LAST_ASK) = Some(Instant::now());
    let mut current = lock(&SESSION_STATE);
    if current.is_some() {
        return Ok(());
    }
    stop_by_name();
    let name = wide(SESSION);
    let mut props = new_props();
    let mut handle = CONTROLTRACE_HANDLE { Value: 0 };
    let rc = unsafe { StartTraceW(&mut handle, name.as_ptr(), props.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES) };
    if rc == 5 {
        return Err(AppError::NeedsAdmin("ETW".into()));
    }
    if rc != 0 {
        return Err(AppError::msg(format!("StartTrace: {rc}")));
    }
    let rc = unsafe { EnableTraceEx2(handle, &KERNEL_NETWORK, EVENT_CONTROL_CODE_ENABLE_PROVIDER, TRACE_LEVEL_INFORMATION as u8, KEYWORDS, 0, 0, std::ptr::null()) };
    if rc != 0 {
        unsafe { ControlTraceW(handle, std::ptr::null(), props.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES, EVENT_TRACE_CONTROL_STOP) };
        return Err(AppError::msg(format!("EnableTraceEx2: {rc}")));
    }
    // Verwerken op een eigen thread: ProcessTrace blokkeert tot de sessie stopt.
    std::thread::Builder::new().name("etw-gamenet".into()).spawn(|| unsafe {
        let mut name = wide(SESSION);
        let mut log: EVENT_TRACE_LOGFILEW = std::mem::zeroed();
        log.LoggerName = name.as_mut_ptr();
        log.Anonymous1.ProcessTraceMode = PROCESS_TRACE_MODE_REAL_TIME | PROCESS_TRACE_MODE_EVENT_RECORD;
        log.Anonymous2.EventRecordCallback = Some(on_event);
        let trace = OpenTraceW(&mut log);
        if trace.Value == u64::MAX {
            log::warn!("ETW: OpenTrace mislukt");
            stop();
            return;
        }
        let rc = ProcessTrace(&trace, 1, std::ptr::null(), std::ptr::null());
        log::info!("ETW-sessie {SESSION} gestopt ({rc})");
        CloseTrace(trace);
    })?;
    *current = Some(Session { control: handle.Value, props, started: Instant::now() });
    drop(current);
    // Opruimen en vanzelf stoppen als niemand meer kijkt.
    std::thread::spawn(|| loop {
        std::thread::sleep(Duration::from_secs(5));
        if lock(&SESSION_STATE).is_none() {
            break;
        }
        let idle = lock(&LAST_ASK).map_or(true, |t| t.elapsed() > IDLE_STOP);
        if idle {
            stop();
            break;
        }
        let now = Instant::now();
        lock(&FLOWS).retain(|_, f| now.duration_since(f.last) < FORGET);
    });
    Ok(())
}

/// Stopt de sessie (ook bij het afsluiten van nezty).
pub fn stop() {
    let Some(mut s) = lock(&SESSION_STATE).take() else { return };
    unsafe {
        ControlTraceW(CONTROLTRACE_HANDLE { Value: s.control }, std::ptr::null(), s.props.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES, EVENT_TRACE_CONTROL_STOP);
    }
    lock(&FLOWS).clear();
    if let Ok(mut w) = WATCH.write() {
        w.clear();
    }
}

/// Welke processen bekeken worden. Elke vraag houdt de sessie een minuut langer in leven.
pub fn watch(pids: &[u32]) {
    *lock(&LAST_ASK) = Some(Instant::now());
    if let Ok(mut w) = WATCH.write() {
        if w.len() != pids.len() || !pids.iter().all(|p| w.contains(p)) {
            w.clear();
            w.extend(pids.iter().copied());
        }
    }
}

/// Hoe lang de sessie al loopt (in seconden), 0 als hij niet loopt.
pub fn watching_secs() -> u64 {
    lock(&SESSION_STATE).as_ref().map(|s| s.started.elapsed().as_secs()).unwrap_or(0)
}

fn is_public(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => {
            let o = a.octets();
            !(a.is_private() || a.is_loopback() || a.is_link_local() || a.is_multicast() || a.is_broadcast() || a.is_unspecified() || (o[0] == 100 && (64..128).contains(&o[1])))
        }
        IpAddr::V6(a) => {
            let s = a.segments();
            !(a.is_loopback() || a.is_multicast() || a.is_unspecified() || (s[0] & 0xffc0) == 0xfe80 || (s[0] & 0xfe00) == 0xfc00)
        }
    }
}

/// De drukste UDP-verbinding van deze processen in de laatste seconden. DNS, web (QUIC) en tijd tellen niet mee.
pub fn busiest_peer(pids: &[u32]) -> Option<Peer> {
    let now = Instant::now();
    let flows = lock(&FLOWS);
    let mut per_peer: HashMap<(IpAddr, u16), (u64, Instant, Instant)> = HashMap::new();
    for ((pid, ip, port), f) in flows.iter() {
        if !pids.contains(pid) || now.duration_since(f.last) > RECENT || !is_public(ip) || matches!(port, 53 | 80 | 123 | 443) {
            continue;
        }
        let e = per_peer.entry((*ip, *port)).or_insert((0, f.first, f.last));
        e.0 += f.packets;
        e.1 = e.1.min(f.first);
        e.2 = e.2.max(f.last);
    }
    per_peer
        .into_iter()
        .map(|((ip, port), (n, first, last))| Peer { ip, port, packets_per_sec: n as f32 / last.duration_since(first).as_secs_f32().max(1.0) })
        .max_by(|a, b| a.packets_per_sec.total_cmp(&b.packets_per_sec))
}

unsafe extern "system" fn on_event(rec: *mut EVENT_RECORD) {
    let Some(r) = rec.as_ref() else { return };
    let (v6, send) = match r.EventHeader.EventDescriptor.Id {
        42 => (false, true),
        43 => (false, false),
        58 => (true, true),
        59 => (true, false),
        _ => return,
    };
    let len = r.UserDataLength as usize;
    if r.UserData.is_null() || len < if v6 { 44 } else { 20 } {
        return;
    }
    let d = std::slice::from_raw_parts(r.UserData as *const u8, len);
    let pid = u32::from_le_bytes([d[0], d[1], d[2], d[3]]);
    // Alleen de processen van de game; de rest van het verkeer wordt meteen vergeten.
    if !WATCH.read().map(|w| w.contains(&pid)).unwrap_or(false) {
        return;
    }
    let port = |o: usize| u16::from_be_bytes([d[o], d[o + 1]]);
    let (remote, remote_port) = if v6 {
        let addr = |o: usize| {
            let mut a = [0u8; 16];
            a.copy_from_slice(&d[o..o + 16]);
            IpAddr::V6(Ipv6Addr::from(a))
        };
        if send {
            (addr(8), port(40))
        } else {
            (addr(24), port(42))
        }
    } else {
        let addr = |o: usize| IpAddr::V4(Ipv4Addr::new(d[o], d[o + 1], d[o + 2], d[o + 3]));
        if send {
            (addr(8), port(16))
        } else {
            (addr(12), port(18))
        }
    };
    let now = Instant::now();
    let mut flows = lock(&FLOWS);
    let f = flows.entry((pid, remote, remote_port)).or_insert(Flow { packets: 0, first: now, last: now });
    f.packets += 1;
    f.last = now;
}
