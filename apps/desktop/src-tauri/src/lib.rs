mod commands;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::app_get_status])
        .run(tauri::generate_context!())
        .expect("Game Manager could not start its desktop runtime");
}
