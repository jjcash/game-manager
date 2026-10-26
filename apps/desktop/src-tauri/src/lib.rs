// SPDX-License-Identifier: GPL-3.0-or-later

mod commands;
pub mod storage;

use std::sync::Arc;
use tauri::Manager;

#[derive(Default)]
pub struct DatabaseState {
    database: Arc<tokio::sync::OnceCell<storage::Result<storage::Database>>>,
    source: String,
}

impl DatabaseState {
    fn ready(&self) -> storage::Result<&storage::Database> {
        self.database
            .get()
            .ok_or_else(|| {
                storage::StorageError::new(
                    storage::ErrorCode::NotReady,
                    "startup",
                    "Database initialization is still running",
                )
            })?
            .as_ref()
            .map_err(Clone::clone)
    }
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(feature = "benchmarks")]
            let benchmark_path =
                std::env::var_os("GAME_MANAGER_BENCHMARK_DB").map(std::path::PathBuf::from);
            #[cfg(not(feature = "benchmarks"))]
            let benchmark_path: Option<std::path::PathBuf> = None;
            let state = DatabaseState {
                source: benchmark_path.as_ref().map_or_else(
                    || "Local application library".into(),
                    |p| format!("Synthetic benchmark: {}", p.display()),
                ),
                ..Default::default()
            };
            let ready = Arc::clone(&state.database);
            let default_root = app.path().app_local_data_dir();
            let override_root =
                std::env::var_os("GAME_MANAGER_DATA_DIR").map(std::path::PathBuf::from);
            app.manage(state);
            tauri::async_runtime::spawn(async move {
                let result = if let Some(path) = benchmark_path {
                    #[cfg(feature = "benchmarks")]
                    {
                        storage::Database::benchmark_view(&path).await
                    }
                    #[cfg(not(feature = "benchmarks"))]
                    {
                        let _ = path;
                        unreachable!()
                    }
                } else {
                    match default_root {
                        Ok(root) => {
                            match storage::application_database_path(
                                &root,
                                override_root.as_deref(),
                            ) {
                                Ok(path) => storage::Database::initialize(&path).await,
                                Err(error) => Err(error),
                            }
                        }
                        Err(error) => Err(storage::StorageError::new(
                            storage::ErrorCode::InvalidPath,
                            "resolve_data_root",
                            error,
                        )),
                    }
                };
                if let Err(error) = &result {
                    // Structured diagnostic; detailed local errors are not sent over IPC.
                    eprintln!(
                        "{}",
                        serde_json::to_string(error)
                            .unwrap_or_else(|_| "database initialization failed".into())
                    );
                }
                let _ = ready.set(result);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_get_status,
            commands::database_get_status,
            commands::library_get_page,
            commands::library_get_info,
            commands::library_get_window
        ])
        .run(tauri::generate_context!())
        .expect("Game Manager could not start its desktop runtime");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn database_startup_exposes_pending_and_persistent_failure_states() {
        let state = DatabaseState::default();
        assert_eq!(
            state.ready().unwrap_err().code,
            storage::ErrorCode::NotReady
        );
        state
            .database
            .set(Err(storage::StorageError::new(
                storage::ErrorCode::Migration,
                "migrate",
                "failed",
            )))
            .unwrap();
        assert_eq!(
            state.ready().unwrap_err().code,
            storage::ErrorCode::Migration
        );
        assert_eq!(
            state.ready().unwrap_err().code,
            storage::ErrorCode::Migration
        );
    }
}
