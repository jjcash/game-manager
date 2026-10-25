fn main() {
    // Opt application commands into Tauri's capability checks.
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["app_get_status"])),
    )
    .expect("could not generate Game Manager's Tauri build context");
}
