use super::*;
use crate::{journal::RevertPayload, models::*, tweaks};
use std::path::PathBuf;

// The benchmark runs as a normal user. Only this bounded worker needs UAC:
// it accepts two predefined NVIDIA values and restores the initial value.
#[test]
#[ignore = "opt-in hardware benchmark; run only in its dedicated test binary"]
fn elevated_nvidia_benchmark_worker() {
    use crate::tweaks::smoothness as nv;
    let exe = std::env::current_exe().unwrap();
    if exe.file_name().unwrap() != "measurement-control.exe" { return; }
    assert!(crate::winapi::is_elevated());
    let dir = exe.parent().unwrap();
    let path = dir.join("nvidia-request.txt");
    let id = nv::PREFERRED_PSTATE_ID;
    let original = nv::nvdrs_read_dword(id).unwrap();
    let start = Instant::now();
    let run = || -> AppResult<()> {
        let mut last_applied = None;
        while start.elapsed().as_secs() < 1800 {
            let fresh = std::fs::metadata(&path)?.modified()?.elapsed().is_ok_and(|t| t.as_secs() < 120);
            if !fresh { break; }
            let wanted = match std::fs::read_to_string(&path)?.trim() {
                "default" => None,
                "max" => Some(nv::PREFERRED_PSTATE_PREFER_MAX),
                "stop" => break,
                _ => return Err(AppError::msg("Unknown benchmark request")),
            };
            if last_applied != Some(wanted) && nv::nvdrs_read_dword(id)? != wanted {
                nv::nvdrs_write_dword(id,wanted)?;
                if nv::nvdrs_read_dword(id)? != wanted { return Err(AppError::msg("NVIDIA verification failed")); }
            }
            last_applied = Some(wanted);
            std::thread::sleep(Duration::from_secs(1));
        }
        Ok(())
    };
    let result = run();
    if nv::nvdrs_read_dword(id).unwrap() != original { nv::nvdrs_write_dword(id,original).unwrap(); }
    assert_eq!(nv::nvdrs_read_dword(id).unwrap(),original);
    result.unwrap();
}

// An isolated test binary, never shipped in the app. An explicit benchmark
// directory and operation are required before changing any host settings.
#[test]
#[ignore = "manual hardware benchmark, explicit opt-in"]
fn hardware_benchmark_control() {
    use crate::tweaks::{smoothness as nv, system};
    let Some(dir) = std::env::var_os("NEZTY_BENCH_DIR").map(PathBuf::from) else { return };
    let op = std::env::var("NEZTY_BENCH_OPERATION").unwrap();
    let state = AppState::new(dir.join("journal"), crate::winapi::is_elevated());
    let now = now_ms();
    let token = TokenPayload { v:1, u:None, n:None, l:"fixture".into(), t:"month".into(), s:"active".into(),
        a:Some(now), e:Some(now+60000), h:String::new(), d:"fixture".into(), i:now, c:now+60000 };
    {
        let mut runtime = lock(&state.license);
        runtime.last = Some(token.clone());
        runtime.payload = Some(token);
    }
    let read = || -> AppResult<serde_json::Value> {
        Ok(serde_json::json!({"plan":system::windows_active_scheme()?.0,
            "cache":nv::nvdrs_read_dword(nv::PS_SHADERDISKCACHE_MAX_SIZE_ID)?,
            "perf":nv::nvdrs_read_dword(nv::PREFERRED_PSTATE_ID)?}))
    };
    let snapshot = dir.join("original-settings.json");
    if op == "snapshot" {
        let data = read().unwrap();
        let mut file = std::fs::File::create_new(snapshot).unwrap();
        serde_json::to_writer_pretty(&mut file, &data).unwrap();
        file.sync_all().unwrap();
        return;
    }
    let original: serde_json::Value = serde_json::from_slice(&std::fs::read(snapshot).unwrap()).unwrap();
    match op.as_str() {
        "capabilities" => {
            let mut rows = Vec::new();
            for id in ["wifi_power_max","wifi_gaming_mode","power_mode_best","cpu_prefer_pcores","touchpad_off_with_mouse"] {
                let d = tweaks::get(id).unwrap();
                let st = tweaks::status(&state, id, None);
                let locked = tweaks::platform_locked(&state,d);
                // Only exercise the rejection branch on this host.
                let error = if locked || st.state == TweakState::Unavailable {
                    Some(tweaks::apply(&state,id,None,None,None).unwrap_err().to_string())
                } else { None };
                rows.push(serde_json::json!({"id":id,"status":st.state,"locked":locked,"detail":st.detail,"error":error}));
            }
            std::fs::write(dir.join("capabilities.json"),serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
        }
        "cache_default" | "perf_default" => {
            let id = if op == "cache_default" { nv::PS_SHADERDISKCACHE_MAX_SIZE_ID } else { nv::PREFERRED_PSTATE_ID };
            if nv::nvdrs_read_dword(id).unwrap().is_some() { nv::nvdrs_write_dword(id,None).unwrap(); }
            assert_eq!(nv::nvdrs_read_dword(id).unwrap(),None);
        }
        "power_plan" | "nvidia_shader_cache" | "nvidia_max_perf" => {
            tweaks::apply(&state,&op,None,Some("benchmark"),None).unwrap();
            tweaks::verify(&state,&op,true,None).unwrap();
        }
        "undo" => {
            let r = lock(&state.journal).revert_all();
            assert!(r.failed.is_empty(), "{:?}", r.failed);
        }
        "restore" => {
            for (name,id) in [("cache",nv::PS_SHADERDISKCACHE_MAX_SIZE_ID),("perf",nv::PREFERRED_PSTATE_ID)] {
                let expected = original[name].as_u64().map(|x|x as u32);
                if nv::nvdrs_read_dword(id).unwrap() != expected { nv::nvdrs_write_dword(id,expected).unwrap(); }
            }
            let guid = original["plan"].as_str().unwrap();
            if system::windows_active_scheme().unwrap().0 != guid {
                assert!(crate::powershell::run_cmd("powercfg",&["/setactive",guid]).unwrap().0);
            }
            assert_eq!(read().unwrap(),original);
        }
        "read" => {}
        _ => panic!("unknown operation"),
    }
    std::fs::write(dir.join("current-settings.json"),serde_json::to_vec_pretty(&read().unwrap()).unwrap()).unwrap();
}

struct Account { dir: PathBuf, target: String, last: String }
impl Account {
    fn new() -> Self {
        let id = uuid::Uuid::new_v4();
        Self { dir: std::env::temp_dir().join(format!("nezty-license-{id}")), target: format!("nezty/test/{id}"), last: format!("nezty/test-last/{id}") }
    }
    fn open(&self) -> AppState {
        let state = AppState::new(self.dir.clone(), true);
        {
            let mut rt = lock(&state.license);
            rt.cred_target = self.target.clone();
            rt.last_target = self.last.clone();
        }
        init(&state);
        state
    }
}
impl Drop for Account {
    fn drop(&mut self) {
        cred::delete(&self.target);
        cred::delete(&self.last);
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn all_catalogue_entries_block_without_a_license() {
    let account = Account::new();
    let state = account.open();
    for d in tweaks::CATALOG {
        assert!(matches!(tweaks::apply(&state, d.id, None, None, None), Err(AppError::Coded {key, ..}) if key == "license_locked"), "{}", d.id);
    }
    assert_eq!(revert_reason(&state), Some("no_token"));
    assert!(lock(&state.journal).entries.is_empty());
}

#[test]
fn missing_or_tampered_token_restores_a_persistent_change() {
    let account = Account::new();
    let state = account.open();
    let path = format!(r"Software\Nezty-Test-{}", uuid::Uuid::new_v4());
    let mut entry = crate::journal::JournalEntry::new("game_mode","fixture",JournalKind::Registry,&path);
    entry.revertable = true;
    entry.payload = RevertPayload::Registry { hive: Hive::Hkcu, path: path.clone(), name: "probe".into(),
        old: RegData::Absent, post: None };
    lock(&state.journal).write_registry(entry, &RegData::Dword(1)).unwrap();
    drop(state);
    assert!(cred::write(&account.last, r#"{"token":"v1.bad.fake","gone":false}"#));
    let reopened = account.open();
    let report = check_headless(&reopened);
    assert_eq!(report["verdict"], "no_token");
    assert_eq!(report["reverted"], 1);
    assert_eq!(report["failed"], serde_json::json!([]));
    assert_eq!(registry::read(Hive::Hkcu,&path,"probe").unwrap(), RegData::Absent);
    Hive::Hkcu.root().delete_subkey_all(&path).unwrap();
}

#[test]
fn read_live_catalogue_without_applying_changes() {
    let account = Account::new();
    let list = tweaks::list(&account.open());
    assert_eq!(list.len(), tweaks::CATALOG.len());
    if let Some(path) = std::env::var_os("NEZTY_TEST_CATALOGUE") {
        std::fs::write(path, serde_json::to_vec_pretty(&list).unwrap()).unwrap();
    }
}

fn fixture(path: &str, mut body: serde_json::Value) -> serde_json::Value {
    // This test may only administer the ephemeral loopback server started by windows-client.ts.
    let base = URL.unwrap();
    assert!(base.starts_with("http://127.0.0.1:"));
    let admin = std::env::var("NEZTY_FIXTURE_ADMIN").unwrap();
    body["admin"] = admin.clone().into();
    let reply = agent().post(format!("{base}{path}")).header("x-admin-key", &admin)
        .header("content-type", "application/json").send(body.to_string()).unwrap().body_mut().read_to_string().unwrap();
    let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["ok"], true, "{path}: {v}");
    v
}

fn register_fixture(state: &AppState, user: &str) -> String {
    let made = fixture("/admin/create", serde_json::json!({"type":"month"}));
    let key = made["keys"][0].as_str().unwrap();
    assert_eq!(register(state, user, "Geldig12345", key, None, true).unwrap().mode, "active");
    assert!(require(state).is_ok());
    let token = lock(&state.license).token.clone().unwrap();
    assert!(verify_token(&token, "another-device").is_none());
    assert!(verify_token(&(token + "x"), &device_id()).is_none());
    key.into()
}

#[test]
#[ignore = "run node server/test/windows-client.ts for the isolated HTTP server"]
fn server_to_windows_expiry_revocation_deletion_and_offline() {
    for (user, endpoint, verdict) in [("revoke_test","/admin/revoke","revoked"),
        ("delete_test","/admin/delete","revoked"), ("move_test","/admin/reset-device","no_token"),
        ("expire_test","/fixture/clock","expired")] {
        let account = Account::new();
        let state = account.open();
        let key = register_fixture(&state, user);
        assert_eq!(refresh(&state).unwrap().mode, "active");
        let game: Game = serde_json::from_value(serde_json::json!({"id":"fixture","name":"Test","launcher":"custom",
            "processNames":[],"known":false,"exePath":account.dir.join("fixture.exe")})).unwrap();
        let path = r"Software\Microsoft\DirectX\UserGpuPreferences";
        let name = game.exe_path.as_deref().unwrap();
        assert_eq!(registry::read(Hive::Hkcu, path, name).unwrap(), RegData::Absent);
        tweaks::apply(&state, "gpu_high_perf", None, None, Some(&game)).unwrap();
        tweaks::verify(&state, "gpu_high_perf", true, Some(&game)).unwrap();
        // Local deadlines also apply between two server checks.
        {
            let mut rt = lock(&state.license);
            let p = rt.payload.clone().unwrap();
            rt.clock = Some((Instant::now(), p.c));
            assert_eq!(rt.status().mode, "offline");
            assert_eq!(rt.verdict(), Some("offline"));
            rt.clock = Some((Instant::now(), p.e.unwrap()));
            assert_eq!(rt.status().mode, "expired");
        }
        assert!(require(&state).is_err());
        drop(state);
        fixture(endpoint, serde_json::json!({"key":key}));
        let reopened = account.open();
        let outcome = check_headless(&reopened);
        assert_eq!(outcome["verdict"], verdict, "{outcome}");
        assert_eq!(outcome["reverted"], 1);
        assert_eq!(outcome["failed"], serde_json::json!([]));
        assert_eq!(outcome["remaining"], false);
        assert!(require(&reopened).is_err());
        assert_eq!(registry::read(Hive::Hkcu, path, name).unwrap(), RegData::Absent);
    }
}

#[test]
#[ignore = "requires isolated license fixture"]
fn registered_tweaks_apply_verify_and_restore_original_registry_data() {
    let account = Account::new();
    let state = account.open();
    register_fixture(&state, "registry_test");
    registry::with_test_hives(|| {
        // Windows already has a nonvolatile HKLM\SOFTWARE before the restart marker is created.
        Hive::Hklm.root().create_subkey("SOFTWARE").unwrap();
        let nic = r"SYSTEM\CurrentControlSet\Control\Class\{4d36e972-e325-11ce-bfc1-08002be10318}\0000";
        registry::write(Hive::Hklm, nic, "*IfType", &RegData::Dword(6)).unwrap();
        registry::write(Hive::Hklm, nic, "ComponentId", &RegData::Str(r"pci\fixture".into())).unwrap();
        registry::write(Hive::Hkcu, r"Software\Microsoft\GameBar", "AutoGameModeEnabled", &RegData::Dword(0)).unwrap();
        for id in ["game_mode", "game_bar_dvr_off", "windowed_opt", "timer_resolution", "foreground_priority",
            "windows_bloat_off", "background_apps_off", "background_tamer", "driver_paging_off",
            "game_bar_presence_off", "notifications_off", "mmcss_gaming", "ethernet_power_off",
            "windows_snappy", "mouse_precision_off", "keyboard_response_fast", "accessibility_hotkeys_off",
            "lang_hotkey_off", "hags"] {
            tweaks::apply(&state, id, None, Some("test"), None).unwrap_or_else(|e| panic!("{id}: {e}"));
            tweaks::verify(&state, id, true, None).unwrap_or_else(|e| panic!("{id}: {e}"));
            let entries: Vec<_> = lock(&state.journal).entries.iter().filter(|e| e.tweak_id == id && !e.reverted).cloned().collect();
            assert!(!entries.is_empty(), "{id}");
            assert!(tweaks::revert_via_journal(&state, id, None).unwrap());
            for e in entries {
                let RevertPayload::Registry {hive, path, name, old, ..} = e.payload else { panic!("{id}") };
                assert!(registry::matches_exact(hive, &path, &name, &old).unwrap(), "{id}: {name}");
            }
        }
    });
}

#[test]
#[ignore = "requires isolated license fixture and temporary Windows power plan"]
fn energy_tweaks_restore_ac_and_dc_on_an_inactive_plan() {
    use crate::{powershell::run_cmd, tweaks::{input, system}};
    let account = Account::new();
    let state = account.open();
    register_fixture(&state, "energy_test");
    let active = system::windows_active_scheme().unwrap().0;
    let id = uuid::Uuid::new_v4().to_string();
    let (ok, _, err) = run_cmd("powercfg", &["/duplicatescheme", "SCHEME_BALANCED", &id]).unwrap();
    assert!(ok, "{err}");
    struct Plan(String);
    impl Drop for Plan {
        fn drop(&mut self) {
            system::TEST_SCHEME.with(|s| s.replace(None));
            let result = crate::powershell::run_cmd("powercfg", &["/delete", &self.0]);
            assert!(result.is_ok_and(|v| v.0), "Could not remove temporary plan {}", self.0);
        }
    }
    let _plan = Plan(id.clone());
    system::TEST_SCHEME.with(|s| s.replace(Some(id.clone())));
    let usb = ("2a737441-1930-4402-8d77-b2bebba308a3","48e6b7a6-50f5-4782-a5d4-53bb8f07e226");
    let cpu = "54533251-82be-4824-96c1-47b60b740d00";
    for (tweak, (sub, setting), ac, dc, device) in [
        ("usb_suspend_off", usb, 1, 1, DeviceSetting::Pc),
        ("usb_suspend_ac_off", usb, 1, 1, DeviceSetting::Laptop),
        ("cpu_desktop_tuning", (cpu, "893dee8e-2bef-41e0-89c6-b55d0929964c"), 5, 5, DeviceSetting::Pc),
        ("cpu_full_speed", (cpu, "bc5038f7-23e0-4960-96da-33abaf5935ec"), 90, 85, DeviceSetting::Laptop),
        ("cpu_epp_performance", (cpu, "36687f9e-e3a5-4dbf-b1dc-15eb381c6863"), 50, 65, DeviceSetting::Laptop),
        ("battery_saver_off", ("de830923-a562-41af-a086-e3a2c6bad2da", "e69653ca-cf7f-4f05-aa73-cb833fa90ad4"), 25, 35, DeviceSetting::Laptop),
    ] {
        lock(&state.settings).device_type = device;
        input::write_power_setting(&id, sub, setting, ac, dc).unwrap_or_else(|e| panic!("{tweak}: {e}"));
        tweaks::apply(&state, tweak, None, Some("energy"), None).unwrap_or_else(|e| panic!("{tweak}: {e}"));
        tweaks::verify(&state, tweak, true, None).unwrap_or_else(|e| panic!("{tweak}: {e}"));
        assert!(tweaks::revert_via_journal(&state, tweak, None).unwrap(), "{tweak}");
        assert_eq!(input::read_power_setting(&id, sub, setting).unwrap(), Some((ac, dc)), "{tweak}");
        assert_eq!(system::windows_active_scheme().unwrap().0, active);
    }
}
