// SPDX-License-Identifier: GPL-3.0-or-later

pub mod artwork;
mod commands;
#[cfg(feature = "benchmarks")]
pub mod jobs;
mod maintenance_commands;
pub mod scheduling;
pub mod storage;

#[cfg(feature = "benchmarks")]
#[derive(Default)]
pub struct JobState(Arc<tokio::sync::OnceCell<Arc<jobs::Scheduler>>>);

pub struct ArtworkState(Option<Arc<artwork::Service>>);

use std::sync::Arc;
use tauri::Manager;

// Buffer detailed marks until a post-startup stats request or normal exit.
// Matched timing runs retain M0.7C's two synchronous legacy writes. The
// separate window-profile control buffers those too, avoiding per-phase I/O.
#[cfg(feature = "benchmarks")]
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct StartupMark {
    startup_stage: &'static str,
    epoch_ms: f64,
    elapsed_ms: f64,
}

#[cfg(feature = "benchmarks")]
static STARTUP_MARKS: std::sync::Mutex<Vec<StartupMark>> = std::sync::Mutex::new(Vec::new());

#[cfg(feature = "benchmarks")]
fn sample_startup(stage: &'static str) -> StartupMark {
    static ORIGIN: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    StartupMark {
        startup_stage: stage,
        elapsed_ms: ORIGIN
            .get_or_init(std::time::Instant::now)
            .elapsed()
            .as_secs_f64()
            * 1000.0,
        epoch_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_secs_f64()
            * 1000.0,
    }
}

#[cfg(feature = "benchmarks")]
fn buffer_startup(mark: StartupMark) {
    let mut marks = STARTUP_MARKS.lock().expect("startup marks lock");
    // Startup-only observations stay bounded even if a diagnostic consumer
    // never flushes. The captured cohorts remain far below this ceiling.
    if marks.len() < 128 {
        marks.push(mark);
    }
}

#[cfg(feature = "benchmarks")]
pub fn startup_mark(stage: &'static str) {
    if std::env::var_os("GAME_MANAGER_STARTUP_TIMING").is_none() {
        return;
    }
    let mark = sample_startup(stage);
    if std::env::var_os("GAME_MANAGER_STARTUP_PROFILE_WINDOW").is_none()
        && std::env::var("GAME_MANAGER_STARTUP_TIMING").as_deref() != Ok("buffered")
        && matches!(stage, "run-entry" | "backend-published")
    {
        // Preserve the historical writer/JSON shape in matched legacy runs.
        eprintln!(
            "{}",
            serde_json::json!({"startupStage":stage,"epochMs":mark.epoch_ms})
        );
        buffer_startup(mark);
        buffer_startup(sample_startup(if stage == "run-entry" {
            "run-entry-log-complete"
        } else {
            "backend-published-log-complete"
        }));
    } else {
        buffer_startup(mark);
    }
}

#[cfg(feature = "benchmarks")]
pub(crate) fn startup_flush() {
    let marks = std::mem::take(&mut *STARTUP_MARKS.lock().expect("startup marks lock"));
    if !marks.is_empty() {
        let lines: Vec<_> = marks
            .iter()
            .map(|mark| serde_json::to_string(mark).expect("startup mark JSON"))
            .collect();
        eprintln!("{}", lines.join("\n"));
    }
}

#[derive(Default)]
pub struct DatabaseState {
    database: Arc<tokio::sync::OnceCell<storage::Result<storage::Database>>>,
    source: String,
    synthetic_jobs: bool,
    artwork_workers: u8,
}

impl DatabaseState {
    fn ready(&self) -> storage::Result<&storage::Database> {
        let db = self
            .database
            .get()
            .ok_or_else(|| {
                storage::StorageError::new(
                    storage::ErrorCode::NotReady,
                    "startup",
                    "Database initialization is still running",
                )
            })?
            .as_ref()
            .map_err(Clone::clone)?;
        if db.is_stopping() {
            return Err(storage::StorageError::new(
                storage::ErrorCode::NotReady,
                "restore",
                "Library is closed for restore. Restart Game Manager to continue.",
            ));
        }
        Ok(db)
    }
}

pub fn run() {
    #[cfg(feature = "benchmarks")]
    startup_mark("run-entry");
    let builder = tauri::Builder::default().setup(|app| {
        app.manage(maintenance_commands::MaintenanceState::default());
        #[cfg(feature = "benchmarks")]
        startup_mark("setup-entry");
        // A separate profiling mode brackets the otherwise Tauri-owned window
        // construction. Keep ordinary timing runs on the configured-window path.
        #[cfg(feature = "benchmarks")]
        if std::env::var_os("GAME_MANAGER_STARTUP_PROFILE_WINDOW").is_some() {
            startup_mark("window-build-entry");
            tauri::WebviewWindowBuilder::from_config(app.handle(), &app.config().app.windows[0])?
                .build()?;
            startup_mark("window-build-complete");
        }
        #[cfg(feature = "benchmarks")]
        startup_mark("application-setup-entry");
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
        #[cfg(feature = "benchmarks")]
        startup_mark("artwork-open-complete");
        app.manage(ArtworkState(artwork));
        let default_root = app.path().app_local_data_dir();
        let override_root = std::env::var_os("GAME_MANAGER_DATA_DIR").map(std::path::PathBuf::from);
        #[cfg(feature = "benchmarks")]
        startup_mark("directories-resolved");
        app.manage(state);
        tauri::async_runtime::spawn(async move {
            #[cfg(feature = "benchmarks")]
            startup_mark("database-task-entry");
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
                #[cfg(feature = "benchmarks")]
                startup_mark("backend-published");
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
            #[cfg(feature = "benchmarks")]
            startup_mark("backend-published");
        });
        #[cfg(feature = "benchmarks")]
        startup_mark("setup-complete");
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
        commands::manual_get,
        commands::manual_create,
        commands::manual_mutate,
        maintenance_commands::backup_get_status,
        maintenance_commands::backup_create,
        maintenance_commands::backup_cancel,
        maintenance_commands::backup_select_restore,
        maintenance_commands::backup_cancel_restore,
        maintenance_commands::backup_confirm_restore,
        maintenance_commands::backup_restart,
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
        commands::manual_get,
        commands::manual_create,
        commands::manual_mutate,
        maintenance_commands::backup_get_status,
        maintenance_commands::backup_create,
        maintenance_commands::backup_cancel,
        maintenance_commands::backup_select_restore,
        maintenance_commands::backup_cancel_restore,
        maintenance_commands::backup_confirm_restore,
        maintenance_commands::backup_restart,
        commands::artwork_get_thumbnail,
        commands::artwork_get_stats,
        commands::synthetic_start,
        commands::synthetic_list,
        commands::synthetic_cancel
    ]);
    #[cfg(feature = "benchmarks")]
    startup_mark("context-entry");
    let context = tauri::generate_context!();
    #[cfg(feature = "benchmarks")]
    let context = {
        let mut context = context;
        if std::env::var_os("GAME_MANAGER_STARTUP_PROFILE_WINDOW").is_some() {
            context.config_mut().app.windows[0].create = false;
        }
        context
    };
    #[cfg(feature = "benchmarks")]
    startup_mark("context-complete");
    let builder = builder.on_page_load(|_, payload| {
        #[cfg(feature = "benchmarks")]
        startup_mark(match payload.event() {
            tauri::webview::PageLoadEvent::Started => "navigation-started",
            tauri::webview::PageLoadEvent::Finished => "navigation-finished",
        });
        #[cfg(not(feature = "benchmarks"))]
        let _ = payload;
    });
    let app = builder
        .build(context)
        .expect("Game Manager could not start its desktop runtime");
    #[cfg(feature = "benchmarks")]
    startup_mark("runtime-built");
    app.run(|app, event| {
        #[cfg(feature = "benchmarks")]
        if matches!(event, tauri::RunEvent::Exit) {
            startup_flush();
        }
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
    #[cfg(feature = "benchmarks")]
    #[test]
    fn startup_buffer_stays_bounded_without_a_consumer() {
        for _ in 0..140 {
            buffer_startup(sample_startup("test"));
        }
        let marks = std::mem::take(&mut *STARTUP_MARKS.lock().unwrap());
        assert_eq!(marks.len(), 128);
        assert!(STARTUP_MARKS.lock().unwrap().is_empty());
    }

    #[cfg(feature = "benchmarks")]
    #[test]
    fn startup_sample_has_monotonic_and_utc_parser_fields() {
        let first = sample_startup("test-first");
        let second = sample_startup("test-second");
        assert!(second.elapsed_ms >= first.elapsed_ms);
        let json = serde_json::to_value(second).unwrap();
        assert_eq!(json["startupStage"], "test-second");
        assert!(json["elapsedMs"].is_number());
        assert!(json["epochMs"].as_f64().unwrap() > 0.0);
    }

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
