fn main() {
    // Eigen manifest: nezty start altijd met administratorrechten (zie app.manifest).
    let windows = tauri_build::WindowsAttributes::new().app_manifest(include_str!("app.manifest"));
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows)).expect("tauri build failed");
}
