//! Fijnere timer tijdens het gamen (terug sinds 0.6.1, zoals in 0.3.16): nezty vraagt 0,5 ms aan zolang de tweak
//! aan staat en er een game draait. Sinds Windows 11 negeert de kernel zo'n verzoek van een geminimaliseerd of
//! bedekt proces tenzij `PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION` voor dit proces uit staat én
//! `GlobalTimerResolutionRequests = 1` in het register staat (na een herstart actief).

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Once;
use windows_sys::Win32::System::Threading::{GetCurrentProcess, ProcessPowerThrottling, SetProcessInformation};

#[link(name = "ntdll")]
extern "system" {
    fn NtSetTimerResolution(desired: u32, set: u8, current: *mut u32) -> i32;
    fn NtQueryTimerResolution(minimum: *mut u32, maximum: *mut u32, current: *mut u32) -> i32;
}

#[repr(C)]
struct PowerThrottlingState {
    version: u32,
    control_mask: u32,
    state_mask: u32,
}

const POWER_THROTTLING_VERSION: u32 = 1;
const EXECUTION_SPEED: u32 = 0x1;
const IGNORE_TIMER_RESOLUTION: u32 = 0x4;

static INIT: Once = Once::new();
static HELD: AtomicBool = AtomicBool::new(false);
static REQUESTED: AtomicU32 = AtomicU32::new(5000);

/// Voorkomt dat Windows 11 het timerverzoek van nezty negeert zodra het venster geminimaliseerd is of achter een
/// fullscreen game zit.
fn disable_timer_throttling() {
    INIT.call_once(|| unsafe {
        let info = PowerThrottlingState {
            version: POWER_THROTTLING_VERSION,
            control_mask: EXECUTION_SPEED | IGNORE_TIMER_RESOLUTION,
            state_mask: 0,
        };
        let _ = SetProcessInformation(
            GetCurrentProcess(),
            ProcessPowerThrottling,
            &info as *const PowerThrottlingState as *const c_void,
            std::mem::size_of::<PowerThrottlingState>() as u32,
        );
    });
}

/// Fijnst ondersteunde timerresolutie (in eenheden van 100 ns, doorgaans 5000 = 0,5 ms).
fn finest_100ns() -> u32 {
    let (mut min_res, mut max_res, mut cur_res) = (0u32, 0u32, 0u32);
    let ok = unsafe { NtQueryTimerResolution(&mut min_res, &mut max_res, &mut cur_res) } == 0;
    if ok && max_res > 0 {
        max_res.clamp(5000, 10000)
    } else {
        5000
    }
}

/// Huidige systeemtimer in eenheden van 100 ns (5000 = 0,5 ms, 156250 = 15,625 ms).
#[allow(dead_code)]
pub fn current_100ns() -> Option<u32> {
    let (mut min_res, mut max_res, mut cur_res) = (0u32, 0u32, 0u32);
    let ok = unsafe { NtQueryTimerResolution(&mut min_res, &mut max_res, &mut cur_res) } == 0;
    (ok && cur_res > 0).then_some(cur_res)
}

/// Vraagt 0,5 ms (of het fijnst mogelijke tussen 0,5 en 1,0 ms) aan zolang `on` waar is.
pub fn hold(on: bool) {
    if HELD.swap(on, Ordering::SeqCst) == on {
        return;
    }
    if on {
        disable_timer_throttling();
        let desired = finest_100ns();
        REQUESTED.store(desired, Ordering::SeqCst);
        let mut current = 0u32;
        unsafe {
            NtSetTimerResolution(desired, 1, &mut current);
        }
    } else {
        let desired = REQUESTED.load(Ordering::SeqCst);
        let mut current = 0u32;
        unsafe {
            NtSetTimerResolution(desired, 0, &mut current);
        }
    }
}
