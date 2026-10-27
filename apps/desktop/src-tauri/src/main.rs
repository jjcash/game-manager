// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(feature = "benchmarks")]
    game_manager_lib::startup_mark("main-entry");
    game_manager_lib::run()
}
