// SPDX-License-Identifier: GPL-3.0-or-later

fn main() {
    println!("cargo:rerun-if-changed=migrations");
    // Opt application commands into Tauri's capability checks.
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "app_get_status",
            "database_get_status",
            "library_get_page",
            "library_get_info",
            "library_get_window",
            "library_query",
            "manual_get",
            "manual_create",
            "manual_mutate",
            "standalone_launch",
            "backup_get_status",
            "backup_create",
            "backup_cancel",
            "backup_select_restore",
            "backup_cancel_restore",
            "backup_confirm_restore",
            "backup_restart",
            "artwork_choose",
            "artwork_clear",
            "artwork_get_thumbnail",
            "artwork_get_stats",
            "synthetic_start",
            "synthetic_list",
            "synthetic_cancel",
        ]),
    ))
    .expect("could not generate Game Manager's Tauri build context");
}
