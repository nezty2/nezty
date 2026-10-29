// Geen consolevenster in release-builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    nezty_lib::run()
}
