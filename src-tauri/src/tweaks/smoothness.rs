//! Smoothness- & anti-stutter-tweaks (v0.7.5):
//! 1. `background_tamer`: zet open achtergrond-slurpers (Discord, Spotify, browsers, launcher-helpers)
//!    tijdens het gamen in Windows EcoQoS + lage prioriteit, zonder ooit het gameproces zelf aan te raken
//!    (nul anti-cheat risico) en zonder aan CPU-affiniteit te zitten.
//! 2. `nvidia_shader_cache`: verruimt de NVIDIA Shader Cache via officiÃ«le NVAPI DRS naar 100 GB zodat
//!    games zoals Fortnite en Warzone (UE5) minder vaak shaders opnieuw hoeven te compileren.
//! 3. `cpu_desktop_tuning`: optionele losse tweak op vaste pc's die minimum processorstatus op 100% houdt
//!    en PCIe ASPM / NVMe-schijfslaapstand uitschakelt.

use crate::bigwins;
use crate::error::{AppError, AppResult};
use crate::journal::{JournalEntry, RevertPayload};
use crate::models::{JournalKind, Msg, TweakState};
use crate::registry::Hive;
use crate::state::{lock, AppState};
use crate::tweaks::input::{read_power_setting, write_power_setting};
use crate::tweaks::regtweak::{self, RegSpec};
use crate::tweaks::{get, revert_via_journal, system, Status};
use crate::winapi::wide;
use std::collections::{HashMap, HashSet};
use std::ffi::{c_void, CString};
use std::ptr::null_mut;
use std::sync::{Mutex, OnceLock};
use sysinfo::System;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows_sys::Win32::System::Threading::{
    GetPriorityClass, OpenProcess, ProcessPowerThrottling, SetPriorityClass, SetProcessInformation,
    BELOW_NORMAL_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS, PROCESS_QUERY_INFORMATION,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_INFORMATION, REALTIME_PRIORITY_CLASS,
};

const NEZTY_TWEAKS_KEY: &str = r"Software\nezty\Tweaks";

// ============================================================================
// 1. Achtergrond-Temmer (`background_tamer`) â€” veilig zonder gameproces/Core 0
// ============================================================================

const BG_TAME_PROCS: &[&str] = &[
    "discord.exe",
    "spotify.exe",
    "chrome.exe",
    "msedge.exe",
    "brave.exe",
    "opera.exe",
    "vivaldi.exe",
    "firefox.exe",
    "steamwebhelper.exe",
    "epicwebhelper.exe",
    "riotclientuxrender.exe",
    "battle.net.exe",
    "eadesktop.exe",
    "upc.exe",
    "onedrive.exe",
    "overwolfbrowser.exe",
];

#[repr(C)]
struct PowerThrottlingState {
    version: u32,
    control_mask: u32,
    state_mask: u32,
}

const POWER_THROTTLING_VERSION: u32 = 1;
const EXECUTION_SPEED: u32 = 0x1;

#[derive(Default)]
struct BgTamerRuntime {
    /// Achtergrond-PID -> oorspronkelijke prioriteitsklasse.
    tamed_bg: HashMap<u32, u32>,
}

fn tamer_runtime() -> &'static Mutex<BgTamerRuntime> {
    static RT: OnceLock<Mutex<BgTamerRuntime>> = OnceLock::new();
    RT.get_or_init(|| Mutex::new(BgTamerRuntime::default()))
}

unsafe fn set_eco_qos(handle: HANDLE, enable_eco: Option<bool>) {
    let info = match enable_eco {
        Some(true) => PowerThrottlingState {
            version: POWER_THROTTLING_VERSION,
            control_mask: EXECUTION_SPEED,
            state_mask: EXECUTION_SPEED,
        },
        Some(false) => PowerThrottlingState {
            version: POWER_THROTTLING_VERSION,
            control_mask: EXECUTION_SPEED,
            state_mask: 0,
        },
        None => PowerThrottlingState {
            version: POWER_THROTTLING_VERSION,
            control_mask: 0,
            state_mask: 0,
        },
    };
    let _ = SetProcessInformation(
        handle,
        ProcessPowerThrottling,
        &info as *const PowerThrottlingState as *const c_void,
        std::mem::size_of::<PowerThrottlingState>() as u32,
    );
}

unsafe fn open_bg_process(pid: u32) -> HANDLE {
    let h = OpenProcess(PROCESS_SET_INFORMATION | PROCESS_QUERY_INFORMATION, 0, pid);
    if !h.is_null() {
        return h;
    }
    OpenProcess(PROCESS_SET_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid)
}

/// Herstelt alle getemde achtergrondprocessen direct naar hun originele prioriteit en haalt EcoQoS weg.
pub fn release_all_bg_tamer() {
    let Ok(mut rt) = tamer_runtime().lock() else { return };
    if rt.tamed_bg.is_empty() {
        return;
    }
    for (pid, old_prio) in rt.tamed_bg.drain() {
        unsafe {
            let h = open_bg_process(pid);
            if !h.is_null() {
                if old_prio != 0 {
                    let _ = SetPriorityClass(h, old_prio);
                }
                set_eco_qos(h, None);
                CloseHandle(h);
            }
        }
    }
}

/// Wordt elke ronde door `watcher.rs` aangeroepen. Raakt NOOIT een gameproces aan.
pub fn sync_bg_tamer(active: bool, sys: &System, game_proc_names: &HashSet<String>) {
    if !active || game_proc_names.is_empty() {
        release_all_bg_tamer();
        return;
    }
    let Ok(mut rt) = tamer_runtime().lock() else { return };
    let alive: HashSet<u32> = sys.processes().keys().map(|p| p.as_u32()).collect();
    rt.tamed_bg.retain(|pid, _| alive.contains(pid));

    let self_pid = std::process::id();
    for (pid_obj, proc_info) in sys.processes() {
        let pid = pid_obj.as_u32();
        if pid <= 4 || pid == self_pid || rt.tamed_bg.contains_key(&pid) {
            continue;
        }
        let name = proc_info.name().to_string_lossy().to_lowercase();
        // Extra beveiliging: nooit een gameproces aanraken, zelfs niet als iemand een eigen procesnaam toevoegde.
        if game_proc_names.contains(&name) || !BG_TAME_PROCS.contains(&name.as_str()) {
            continue;
        }
        unsafe {
            let h = open_bg_process(pid);
            if h.is_null() {
                continue;
            }
            let old_prio = GetPriorityClass(h);
            if old_prio != 0 && old_prio != REALTIME_PRIORITY_CLASS {
                let _ = SetPriorityClass(h, BELOW_NORMAL_PRIORITY_CLASS);
            }
            set_eco_qos(h, Some(true));
            rt.tamed_bg
                .insert(pid, if old_prio != 0 { old_prio } else { NORMAL_PRIORITY_CLASS });
            CloseHandle(h);
        }
    }
}

fn bg_tamer_specs() -> Vec<RegSpec> {
    vec![RegSpec::dword(Hive::Hkcu, NEZTY_TWEAKS_KEY, "BackgroundTamer", 1, 0).off_absent()]
}

pub fn bg_tamer_status() -> AppResult<Status> {
    Ok(Status::simple(regtweak::state_of(&bg_tamer_specs())?))
}

pub fn bg_tamer_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    regtweak::apply(state, get("background_tamer")?, &bg_tamer_specs(), session, None, None)
}

pub fn bg_tamer_disable(state: &AppState) -> AppResult<()> {
    release_all_bg_tamer();
    regtweak::disable(state, get("background_tamer")?, &bg_tamer_specs(), None, None)
}

// ============================================================================
// 2. NVIDIA 100 GB Shader Cache (`nvidia_shader_cache`) via officiÃ«le NVAPI DRS
// ============================================================================

pub const PS_SHADERDISKCACHE_MAX_SIZE_ID: u32 = 0x00AC_8497;
const SHADER_CACHE_100GB_MB: u32 = 102_400;
const NVAPI_SETTING_NOT_FOUND: i32 = -160;

#[repr(C)]
#[derive(Clone, Copy)]
struct NvDrsBinaryData {
    value_length: u32,
    value_data: [u8; 4096],
}

#[repr(C)]
#[derive(Clone, Copy)]
union NvDrsSettingUnion {
    u32_value: u32,
    binary_value: NvDrsBinaryData,
    wsz_value: [u16; 2048],
}

#[repr(C)]
struct NvDrsSettingV1 {
    version: u32,
    setting_name: [u16; 2048],
    setting_id: u32,
    setting_type: u32,
    setting_location: u32,
    is_current_predefined: u32,
    is_predefined_valid: u32,
    predefined: NvDrsSettingUnion,
    current: NvDrsSettingUnion,
}

type QueryInterfaceFn = unsafe extern "C" fn(u32) -> *const c_void;
type InitFn = unsafe extern "C" fn() -> i32;
type CreateSessionFn = unsafe extern "C" fn(*mut *mut c_void) -> i32;
type DestroySessionFn = unsafe extern "C" fn(*mut c_void) -> i32;
type LoadSaveFn = unsafe extern "C" fn(*mut c_void) -> i32;
type GetBaseProfileFn = unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32;
type GetSettingFn = unsafe extern "C" fn(*mut c_void, *mut c_void, u32, *mut NvDrsSettingV1) -> i32;
type SetSettingFn = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut NvDrsSettingV1) -> i32;
type DeleteSettingFn = unsafe extern "C" fn(*mut c_void, *mut c_void, u32) -> i32;

struct NvDrsApi {
    create_session: CreateSessionFn,
    destroy_session: DestroySessionFn,
    load_settings: LoadSaveFn,
    save_settings: LoadSaveFn,
    get_base_profile: GetBaseProfileFn,
    get_setting: GetSettingFn,
    set_setting: SetSettingFn,
    delete_setting: DeleteSettingFn,
    lock: Mutex<()>,
}

unsafe impl Send for NvDrsApi {}
unsafe impl Sync for NvDrsApi {}

fn nvdrs() -> Option<&'static NvDrsApi> {
    static API: OnceLock<Option<NvDrsApi>> = OnceLock::new();
    API.get_or_init(|| unsafe {
        if !bigwins::gpu_layout().gpus.iter().any(|g| g.vendor == "nvidia") {
            return None;
        }
        let lib = LoadLibraryW(wide("nvapi64.dll").as_ptr());
        if lib.is_null() {
            return None;
        }
        let qi_name = CString::new("nvapi_QueryInterface").ok()?;
        let qi_ptr = GetProcAddress(lib, qi_name.as_ptr() as *const u8)?;
        let qi: QueryInterfaceFn = std::mem::transmute(qi_ptr);

        let init_ptr = qi(0x0150_E828);
        if init_ptr.is_null() {
            return None;
        }
        let init: InitFn = std::mem::transmute(init_ptr);
        if init() != 0 {
            return None;
        }

        let f = |id: u32| -> Option<*const c_void> {
            let p = qi(id);
            (!p.is_null()).then_some(p)
        };
        Some(NvDrsApi {
            create_session: std::mem::transmute(f(0x0694_D52E)?),
            destroy_session: std::mem::transmute(f(0xDAD9_CFF8)?),
            load_settings: std::mem::transmute(f(0x375D_BD6B)?),
            save_settings: std::mem::transmute(f(0xFCBC_7E14)?),
            get_base_profile: std::mem::transmute(f(0xDA84_66A0)?),
            get_setting: std::mem::transmute(f(0x73BF_8338)?),
            set_setting: std::mem::transmute(f(0x577D_D202)?),
            delete_setting: std::mem::transmute(f(0xE4A2_6362)?),
            lock: Mutex::new(()),
        })
    })
    .as_ref()
}

fn new_boxed_setting() -> Box<NvDrsSettingV1> {
    let mut b: Box<std::mem::MaybeUninit<NvDrsSettingV1>> = Box::new(std::mem::MaybeUninit::zeroed());
    unsafe {
        let ptr = b.as_mut_ptr();
        (*ptr).version = (std::mem::size_of::<NvDrsSettingV1>() as u32) | (1 << 16);
        Box::from_raw(Box::into_raw(b) as *mut NvDrsSettingV1)
    }
}

pub fn nvdrs_read_dword(setting_id: u32) -> AppResult<Option<u32>> {
    let api = nvdrs().ok_or_else(|| AppError::coded("nvidia_only"))?;
    let _guard = api.lock.lock().map_err(|_| AppError::msg("nvdrs lock"))?;
    unsafe {
        let mut session: *mut c_void = null_mut();
        let rc = (api.create_session)(&mut session);
        if rc != 0 || session.is_null() {
            return Err(AppError::msg(format!("NvAPI_DRS_CreateSession ({rc})")));
        }
        let res = (|| -> AppResult<Option<u32>> {
            let rc = (api.load_settings)(session);
            if rc != 0 {
                return Err(AppError::msg(format!("NvAPI_DRS_LoadSettings ({rc})")));
            }
            let mut profile: *mut c_void = null_mut();
            let rc = (api.get_base_profile)(session, &mut profile);
            if rc != 0 || profile.is_null() {
                return Err(AppError::msg(format!("NvAPI_DRS_GetBaseProfile ({rc})")));
            }
            let mut setting = new_boxed_setting();
            let rc = (api.get_setting)(session, profile, setting_id, &mut *setting);
            if rc == NVAPI_SETTING_NOT_FOUND {
                return Ok(None);
            }
            if rc != 0 {
                return Err(AppError::msg(format!("NvAPI_DRS_GetSetting ({rc})")));
            }
            if setting.is_current_predefined != 0 {
                return Ok(None);
            }
            Ok(Some(setting.current.u32_value))
        })();
        let _ = (api.destroy_session)(session);
        res
    }
}

pub fn nvdrs_write_dword(setting_id: u32, value: Option<u32>) -> AppResult<()> {
    let api = nvdrs().ok_or_else(|| AppError::coded("nvidia_only"))?;
    let _guard = api.lock.lock().map_err(|_| AppError::msg("nvdrs lock"))?;
    unsafe {
        let mut session: *mut c_void = null_mut();
        let rc = (api.create_session)(&mut session);
        if rc != 0 || session.is_null() {
            return Err(AppError::msg(format!("NvAPI_DRS_CreateSession ({rc})")));
        }
        let res = (|| -> AppResult<()> {
            let rc = (api.load_settings)(session);
            if rc != 0 {
                return Err(AppError::msg(format!("NvAPI_DRS_LoadSettings ({rc})")));
            }
            let mut profile: *mut c_void = null_mut();
            let rc = (api.get_base_profile)(session, &mut profile);
            if rc != 0 || profile.is_null() {
                return Err(AppError::msg(format!("NvAPI_DRS_GetBaseProfile ({rc})")));
            }
            match value {
                Some(v) => {
                    let mut setting = new_boxed_setting();
                    setting.setting_id = setting_id;
                    setting.setting_type = 0; // NVDRS_DWORD_TYPE
                    setting.setting_location = 0; // NVDRS_CURRENT_PROFILE_LOCATION
                    setting.current.u32_value = v;
                    let rc = (api.set_setting)(session, profile, &mut *setting);
                    if rc != 0 {
                        return Err(AppError::msg(format!("NvAPI_DRS_SetSetting ({rc})")));
                    }
                }
                None => {
                    let rc = (api.delete_setting)(session, profile, setting_id);
                    if rc != 0 && rc != NVAPI_SETTING_NOT_FOUND {
                        return Err(AppError::msg(format!("NvAPI_DRS_DeleteProfileSetting ({rc})")));
                    }
                }
            }
            let rc = (api.save_settings)(session);
            if rc != 0 {
                return Err(AppError::msg(format!("NvAPI_DRS_SaveSettings ({rc})")));
            }
            Ok(())
        })();
        let _ = (api.destroy_session)(session);
        res
    }
}

pub fn nvidia_shader_status() -> AppResult<Status> {
    if nvdrs().is_none() {
        return Ok(Status::with_detail(TweakState::Unavailable, Msg::new("nvidia_only")));
    }
    let cur = nvdrs_read_dword(PS_SHADERDISKCACHE_MAX_SIZE_ID)?;
    let on = cur.is_some_and(|mb| mb >= SHADER_CACHE_100GB_MB);
    Ok(Status::simple(if on { TweakState::On } else { TweakState::Off }))
}

pub fn nvidia_shader_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    let d = get("nvidia_shader_cache")?;
    let old = nvdrs_read_dword(PS_SHADERDISKCACHE_MAX_SIZE_ID)?;
    if old.is_some_and(|mb| mb >= SHADER_CACHE_100GB_MB) {
        return Ok(());
    }
    nvdrs_write_dword(PS_SHADERDISKCACHE_MAX_SIZE_ID, Some(SHADER_CACHE_100GB_MB))?;
    let mut e = JournalEntry::new(d.id, d.name, JournalKind::Setting, "NVIDIA Global Profile: Shader Cache Size");
    e.old_value = Some(old.map(|v| format!("{v} MB")).unwrap_or_else(|| "Driver default".into()));
    e.new_value = Some(format!("{SHADER_CACHE_100GB_MB} MB (100 GB)"));
    e.revertable = true;
    e.session_id = session.map(|s| s.to_string());
    e.payload = RevertPayload::NvDrsDword { setting_id: PS_SHADERDISKCACHE_MAX_SIZE_ID, old };
    lock(&state.journal).record(e)
}

pub fn nvidia_shader_disable(state: &AppState) -> AppResult<()> {
    if revert_via_journal(state, "nvidia_shader_cache", None)? {
        return Ok(());
    }
    if nvdrs().is_some() {
        nvdrs_write_dword(PS_SHADERDISKCACHE_MAX_SIZE_ID, None)?;
    }
    Ok(())
}

// ============================================================================
// 2b. NVIDIA Energiebeheer: Voorkeur voor maximale prestaties (`nvidia_max_perf`)
// ============================================================================

pub const PREFERRED_PSTATE_ID: u32 = 0x1057E38B;
pub const PREFERRED_PSTATE_PREFER_MAX: u32 = 1;

pub fn nvidia_max_perf_status() -> AppResult<Status> {
    if nvdrs().is_none() {
        return Ok(Status::with_detail(TweakState::Unavailable, Msg::new("nvidia_only")));
    }
    let cur = nvdrs_read_dword(PREFERRED_PSTATE_ID)?;
    let on = cur == Some(PREFERRED_PSTATE_PREFER_MAX);
    Ok(Status::simple(if on { TweakState::On } else { TweakState::Off }))
}

pub fn nvidia_max_perf_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    let d = get("nvidia_max_perf")?;
    let old = nvdrs_read_dword(PREFERRED_PSTATE_ID)?;
    if old == Some(PREFERRED_PSTATE_PREFER_MAX) {
        return Ok(());
    }
    nvdrs_write_dword(PREFERRED_PSTATE_ID, Some(PREFERRED_PSTATE_PREFER_MAX))?;
    let mut e = JournalEntry::new(d.id, d.name, JournalKind::Setting, "NVIDIA Global Profile: Power Management Mode (Prefer Maximum Performance)");
    e.old_value = Some(old.map(|v| if v == 0 { "Optimal Power / Adaptive".into() } else { format!("{v}") }).unwrap_or_else(|| "Driver default".into()));
    e.new_value = Some("Prefer maximum performance".into());
    e.revertable = true;
    e.session_id = session.map(|s| s.to_string());
    e.payload = RevertPayload::NvDrsDword { setting_id: PREFERRED_PSTATE_ID, old };
    lock(&state.journal).record(e)
}

pub fn nvidia_max_perf_disable(state: &AppState) -> AppResult<()> {
    if revert_via_journal(state, "nvidia_max_perf", None)? {
        return Ok(());
    }
    if nvdrs().is_some() {
        nvdrs_write_dword(PREFERRED_PSTATE_ID, None)?;
    }
    Ok(())
}

// ============================================================================
// 3. Optioneel: Desktop CPU minimum 100% & PCIe/NVMe slaapstand uit (`cpu_desktop_tuning`)
// ============================================================================

const SUB_PROC: &str = "54533251-82be-4824-96c1-47b60b740d00";
const SET_MIN_PROC: &str = "893dee8e-2bef-41e0-89c6-b55d0929964c";
const SET_MAX_PROC: &str = "bc5038f7-23e0-4960-96da-33abaf5935ec";

const SUB_PCIE: &str = "501a4d13-42af-4429-9fd1-a8218c268e20";
const SET_ASPM: &str = "ee12f906-d277-404b-b6da-e5fa1a576df5";

const SUB_DISK: &str = "0012ee47-9041-4b5d-9b77-535fba8b1442";
const SET_DISK_IDLE: &str = "6738e2c4-e8a5-4a42-b16a-e040e769756e";
const SET_NVME_LATENCY: &str = "fc95af4d-40e7-4b6d-835a-56d131dbc80e";

/// `(subgroep, instelling, gewenste_ac, fallback_standaard_ac, label)`
/// Bewust zonder aanpassing aan `PERFBOOSTMODE`: alleen minimum/maximum CPU en PCIe/NVMe-slaapstand.
const DESKTOP_POWER_SPECS: &[(&str, &str, u32, u32, &str)] = &[
    (SUB_PROC, SET_MIN_PROC, 100, 5, "minimum processor state"),
    (SUB_PROC, SET_MAX_PROC, 100, 100, "maximum processor state"),
    (SUB_PCIE, SET_ASPM, 0, 1, "PCIe link state power management"),
    (SUB_DISK, SET_DISK_IDLE, 0, 1200, "disk idle timeout"),
    (SUB_DISK, SET_NVME_LATENCY, 0, 0, "NVMe power transition latency"),
];

pub fn cpu_desktop_status() -> AppResult<Status> {
    let (scheme, _) = system::active_scheme()?;
    let mut supported = 0usize;
    for &(sub, setting, want_ac, _, _) in DESKTOP_POWER_SPECS {
        let Some((ac, _)) = read_power_setting(&scheme, sub, setting)? else {
            continue;
        };
        supported += 1;
        if ac != want_ac {
            return Ok(Status::simple(TweakState::Off));
        }
    }
    if supported == 0 {
        return Ok(Status::with_detail(TweakState::Unavailable, Msg::new("power_setting_missing")));
    }
    Ok(Status::simple(TweakState::On))
}

pub fn cpu_desktop_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    let d = get("cpu_desktop_tuning")?;
    let (scheme, scheme_name) = system::active_scheme()?;
    let mut changed_any = false;
    for &(sub, setting, want_ac, _, label) in DESKTOP_POWER_SPECS {
        let Some((old_ac, old_dc)) = read_power_setting(&scheme, sub, setting)? else {
            continue;
        };
        changed_any = true;
        if old_ac == want_ac {
            continue;
        }
        let mut e = JournalEntry::new(d.id, d.name, JournalKind::PowerSetting, &format!("{scheme_name}: {label}"));
        e.old_value = Some(format!("AC {old_ac}, DC {old_dc}"));
        e.new_value = Some(format!("AC {want_ac}, DC {old_dc}"));
        e.revertable = true;
        e.session_id = session.map(|s| s.to_string());
        e.payload = RevertPayload::PowerSetting {
            scheme: scheme.clone(),
            sub: sub.to_string(),
            setting: setting.to_string(),
            ac: old_ac,
            dc: old_dc,
        };
        lock(&state.journal).write_power(e, want_ac, old_dc)?;
    }
    if !changed_any {
        return Err(AppError::coded("power_setting_missing"));
    }
    Ok(())
}

pub fn cpu_desktop_disable(state: &AppState) -> AppResult<()> {
    if revert_via_journal(state, "cpu_desktop_tuning", None)? {
        return Ok(());
    }
    let (scheme, _) = system::active_scheme()?;
    for &(sub, setting, _, default_ac, _) in DESKTOP_POWER_SPECS {
        if let Some((_, dc)) = read_power_setting(&scheme, sub, setting)? {
            let _ = write_power_setting(&scheme, sub, setting, default_ac, dc);
        }
    }
    Ok(())
}
