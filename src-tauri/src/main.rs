// De desktopapp opent ook in testbuilds zonder apart consolevenster.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() {
    nezty_lib::run()
}
