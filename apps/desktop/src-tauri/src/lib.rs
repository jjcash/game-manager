// SPDX-License-Identifier: GPL-3.0-or-later

pub mod artwork;
mod commands;
#[cfg(feature = "benchmarks")]
pub mod jobs;
pub mod scheduling;
pub mod storage;

#[cfg(feature = "benchmarks")]
#[derive(Default)]
pub struct JobState(Arc<tokio::sync::OnceCell<Arc<jobs::Scheduler>>>);

pub struct ArtworkState(Option<Arc<artwork::Service>>);

use std::sync::Arc;
use tauri::Manager;

#[derive(Default)]
pub struct DatabaseState {
    database: Arc<tokio::sync::OnceCell<storage::Result<storage::Database>>>,
    source: String,
    synthetic_jobs: bool,
    artwork_workers: u8,
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
    let builder = tauri::Builder::default().setup(|app| {
        #[cfg(feature = "benchmarks")]
        let job_root =
            std::env::var_os("GAME_MANAGER_SYNTHETIC_ROOT").map(std::path::PathBuf::from);
        #[cfg(feature = "benchmarks")]
        let jobs_enabled = job_root.is_some();
        #[cfg(not(feature = "benchmarks"))]
        let jobs_enabled = false;
        #[cfg(feature = "benchmarks")]
        let foreground = match std::env::var("GAME_MANAGER_BACKGROUND_POLICY")
            .unwrap_or_else(|_| "unthrottled".into())
            .as_str()
        {
            "foreground" => true,
            "unthrottled" => false,
            _ => return Err("Invalid background policy".into()),
        };
        #[cfg(not(feature = "benchmarks"))]
        let foreground = false;
        let budget = scheduling::Budget::new(foreground);
        app.manage(Arc::clone(&budget));
        #[cfg(feature = "benchmarks")]
        let artwork_workers = std::env::var("GAME_MANAGER_ARTWORK_WORKERS")
            .unwrap_or_else(|_| "2".into())
            .parse::<u8>()
            .ok()
            .filter(|n| *n <= 2)
            .ok_or("Invalid artwork worker count")?;
        #[cfg(not(feature = "benchmarks"))]
        let artwork_workers = 2;
        #[cfg(feature = "benchmarks")]
        let job_state = JobState::default();
        #[cfg(feature = "benchmarks")]
        let job_ready = Arc::clone(&job_state.0);
        #[cfg(feature = "benchmarks")]
        {
            if jobs_enabled {
                app.add_capability(
                    tauri::ipc::CapabilityBuilder::new("synthetic-jobs")
                        .window("main")
                        .permission("allow-synthetic-start")
                        .permission("allow-synthetic-list")
                        .permission("allow-synthetic-cancel"),
                )?;
            }
            app.manage(job_state);
        }
        #[cfg(feature = "benchmarks")]
        let benchmark_path =
            std::env::var_os("GAME_MANAGER_BENCHMARK_DB").map(std::path::PathBuf::from);
        #[cfg(not(feature = "benchmarks"))]
        let benchmark_path: Option<std::path::PathBuf> = None;
        let state = DatabaseState {
            synthetic_jobs: jobs_enabled,
            artwork_workers,
            source: if jobs_enabled {
                "Disposable synthetic job fixture".into()
            } else {
                benchmark_path.as_ref().map_or_else(
                    || "Local application library".into(),
                    |p| format!("Synthetic benchmark: {}", p.display()),
                )
            },
            ..Default::default()
        };
        let ready = Arc::clone(&state.database);
        // Fixtures are selected only by an explicitly enabled benchmark binary.
        // Normal startup never generates artwork or scans a personal library.
        #[cfg(feature = "benchmarks")]
        let artwork = std::env::var_os("GAME_MANAGER_BENCHMARK_ARTWORK").and_then(|root| {
            match artwork::Service::open_with_policy(
                std::path::Path::new(&root),
                artwork_workers,
                Arc::clone(&budget),
            ) {
                Ok(service) => Some(service),
                Err(error) => {
                    eprintln!("Artwork unavailable: {error}");
                    None
                }
            }
        });
        #[cfg(not(feature = "benchmarks"))]
        let artwork = None;
        app.manage(ArtworkState(artwork));
        let default_root = app.path().app_local_data_dir();
        let override_root = std::env::var_os("GAME_MANAGER_DATA_DIR").map(std::path::PathBuf::from);
        app.manage(state);
        tauri::async_runtime::spawn(async move {
            #[cfg(feature = "benchmarks")]
            if let Some(root) = job_root {
                let result = match jobs::Scheduler::open(&root, budget).await {
                    Ok((scheduler, db)) => {
                        let _ = job_ready.set(scheduler);
                        Ok(db)
                    }
                    Err(error) => Err(error),
                };
                let _ = ready.set(result);
                return;
            }
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
                        match storage::application_database_path(&root, override_root.as_deref()) {
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
    });
    #[cfg(not(feature = "benchmarks"))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        commands::app_get_status,
        commands::database_get_status,
        commands::library_get_page,
        commands::library_get_info,
        commands::library_get_window,
        commands::library_query,
        commands::artwork_get_thumbnail,
        commands::artwork_get_stats
    ]);
    #[cfg(feature = "benchmarks")]
    let builder = builder.invoke_handler(tauri::generate_handler![
        commands::app_get_status,
        commands::database_get_status,
        commands::library_get_page,
        commands::library_get_info,
        commands::library_get_window,
        commands::library_query,
        commands::artwork_get_thumbnail,
        commands::artwork_get_stats,
        commands::synthetic_start,
        commands::synthetic_list,
        commands::synthetic_cancel
    ]);
    let app = builder
        .build(tauri::generate_context!())
        .expect("Game Manager could not start its desktop runtime");
    app.run(|app, event| {
        #[cfg(feature = "benchmarks")]
        if let tauri::RunEvent::ExitRequested { api, code, .. } = event
            && code.is_none()
            && let Some(scheduler) = app.state::<JobState>().0.get().cloned()
        {
            api.prevent_exit();
            let handle = app.clone();
            tauri::async_runtime::spawn(async move {
                let code = if let Err(error) = scheduler.shutdown().await {
                    eprintln!("Synthetic shutdown: {error}");
                    1
                } else {
                    0
                };
                handle.exit(code);
            });
        }
        #[cfg(not(feature = "benchmarks"))]
        let _ = (app, event);
    });
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
