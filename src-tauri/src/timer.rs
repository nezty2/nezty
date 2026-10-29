//! Hogere timerresolutie (experimenteel): nezty vraagt 0,5 ms aan zolang de tweak aan staat en er een
//! game draait. Sinds Windows 11 telt zo'n verzoek alleen voor alle programma's als
//! GlobalTimerResolutionRequests = 1 staat (dat zet de tweak zelf, na een herstart actief).

use std::sync::atomic::{AtomicBool, Ordering};

#[link(name = "ntdll")]
extern "system" {
    fn NtSetTimerResolution(desired: u32, set: u8, current: *mut u32) -> i32;
}

static HELD: AtomicBool = AtomicBool::new(false);

/// 5000 eenheden van 100 ns = 0,5 ms. Doet niets als de gevraagde stand al geldt.
pub fn hold(on: bool) {
    if HELD.swap(on, Ordering::SeqCst) == on {
        return;
    }
    let mut current = 0u32;
    unsafe {
        NtSetTimerResolution(5000, u8::from(on), &mut current);
    }
}
