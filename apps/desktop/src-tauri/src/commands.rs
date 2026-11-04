// SPDX-License-Identifier: GPL-3.0-or-later

use serde::Serialize;

#[tauri::command]
pub async fn artwork_get_thumbnail(
    state: tauri::State<'_, crate::ArtworkState>,
    database: tauri::State<'_, crate::DatabaseState>,
    budget: tauri::State<'_, std::sync::Arc<crate::scheduling::Budget>>,
    id: String,
) -> Result<crate::artwork::Reply, String> {
    Ok(if let Some(service) = &state.0 {
        service.request(&id).await
    } else {
        if let Ok(db) = database.ready()
            && let Some(lifecycle) = &db.lifecycle
        {
            lifecycle
                .artwork
                .thumbnail(db.pool.clone(), budget.inner().clone(), id)
                .await
        } else {
            crate::artwork::Reply::empty("missing")
        }
    })
}

#[tauri::command]
pub fn artwork_get_stats(
    state: tauri::State<'_, crate::ArtworkState>,
    database: tauri::State<'_, crate::DatabaseState>,
) -> crate::artwork::Stats {
    #[cfg(feature = "benchmarks")]
    crate::startup_flush();
    state.0.as_ref().map_or_else(
        || {
            database
                .ready()
                .ok()
                .and_then(|db| db.lifecycle.as_ref())
                .map_or_else(Default::default, |l| l.artwork.stats())
        },
        |s| s.stats(),
    )
}

/// A small, read-only IPC probe. This does not imply database readiness.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    app_name: &'static str,
    version: &'static str,
    backend: &'static str,
}

#[tauri::command]
pub fn app_get_status() -> AppStatus {
    AppStatus {
        app_name: "Game Manager",
        version: env!("CARGO_PKG_VERSION"),
        backend: "rust",
    }
}

// Keep local SQLite/OS diagnostics out of the frontend response.
fn public_error(mut error: crate::storage::StorageError) -> crate::storage::StorageError {
    use crate::storage::ErrorCode;
    if !matches!(
        error.code,
        ErrorCode::NotReady | ErrorCode::InvalidRequest | ErrorCode::NotFound | ErrorCode::Conflict
    ) {
        eprintln!("{}", serde_json::to_string(&error).unwrap_or_default());
        error.message =
            "Database unavailable. Check the configured data location and local diagnostics, then restart Game Manager. The database was preserved.".into();
    }
    error
}

#[tauri::command]
pub async fn manual_get(
    budget: tauri::State<'_, std::sync::Arc<crate::scheduling::Budget>>,
    state: tauri::State<'_, crate::DatabaseState>,
    game_id: String,
) -> crate::storage::Result<crate::storage::manual::Details> {
    let _foreground = budget.enter();
    async { state.ready()?.manual_details(&game_id).await }
        .await
        .map_err(public_error)
}
#[tauri::command]
pub async fn manual_create(
    budget: tauri::State<'_, std::sync::Arc<crate::scheduling::Budget>>,
    state: tauri::State<'_, crate::DatabaseState>,
    request: crate::storage::manual::CreateRequest,
) -> crate::storage::Result<String> {
    let _foreground = budget.enter();
    async { state.ready()?.manual_create(&request).await }
        .await
        .map_err(public_error)
}
#[tauri::command]
pub async fn manual_mutate(
    budget: tauri::State<'_, std::sync::Arc<crate::scheduling::Budget>>,
    state: tauri::State<'_, crate::DatabaseState>,
    request: crate::storage::manual::MutationRequest,
) -> crate::storage::Result<()> {
    let _foreground = budget.enter();
    async { state.ready()?.manual_mutate(&request).await }
        .await
        .map_err(public_error)
}

#[tauri::command]
pub async fn database_get_status(
    budget: tauri::State<'_, std::sync::Arc<crate::scheduling::Budget>>,
    state: tauri::State<'_, crate::DatabaseState>,
) -> crate::storage::Result<crate::storage::DatabaseStatus> {
    let _foreground = budget.enter();
    async { state.ready()?.status().await }
        .await
        .map_err(public_error)
}

#[tauri::command]
pub async fn library_get_page(
    budget: tauri::State<'_, std::sync::Arc<crate::scheduling::Budget>>,
    state: tauri::State<'_, crate::DatabaseState>,
    request: crate::storage::PageRequest,
) -> crate::storage::Result<Vec<crate::storage::GameSummary>> {
    let _foreground = budget.enter();
    async { state.ready()?.page(&request).await }
        .await
        .map_err(public_error)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryInfo {
    pool_wait_ms: f64,
    synthetic_jobs: bool,
    artwork_workers: u8,
    total: i64,
    source: String,
    diagnostics: bool,
    library_total: i64,
    sources: Vec<crate::storage::query::SourceOption>,
    query_ms: f64,
}

#[tauri::command]
pub async fn library_get_info(
    budget: tauri::State<'_, std::sync::Arc<crate::scheduling::Budget>>,
    state: tauri::State<'_, crate::DatabaseState>,
    criteria: Option<crate::storage::query::Criteria>,
) -> crate::storage::Result<LibraryInfo> {
    #[cfg(feature = "benchmarks")]
    crate::startup_mark("metadata-request-entry");
    let _foreground = budget.enter();
    let result = async {
        let criteria = criteria.unwrap_or_default();
        let start = std::time::Instant::now();
        let (total, pool_wait_ms) = state.ready()?.query_count_timed(&criteria).await?;
        let query_ms = start.elapsed().as_secs_f64() * 1000.0;
        Ok(LibraryInfo {
            pool_wait_ms,
            synthetic_jobs: state.synthetic_jobs,
            artwork_workers: state.artwork_workers,
            total,
            library_total: state.ready()?.counts().await?.games,
            sources: state.ready()?.source_options().await?,
            query_ms,
            source: state.source.clone(),
            diagnostics: cfg!(any(debug_assertions, feature = "benchmarks")),
        })
    }
    .await;
    #[cfg(feature = "benchmarks")]
    crate::startup_mark("metadata-request-complete");
    result.map_err(public_error)
}

#[tauri::command]
pub async fn library_get_window(
    budget: tauri::State<'_, std::sync::Arc<crate::scheduling::Budget>>,
    state: tauri::State<'_, crate::DatabaseState>,
    request: crate::storage::PageRequest,
) -> crate::storage::Result<crate::storage::LibraryWindow> {
    let _foreground = budget.enter();
    async { state.ready()?.window(&request).await }
        .await
        .map_err(public_error)
}

#[tauri::command]
pub async fn library_query(
    budget: tauri::State<'_, std::sync::Arc<crate::scheduling::Budget>>,
    state: tauri::State<'_, crate::DatabaseState>,
    request: crate::storage::query::QueryRequest,
) -> crate::storage::Result<crate::storage::LibraryWindow> {
    let _foreground = budget.enter();
    async {
        #[cfg(feature = "benchmarks")]
        crate::startup_mark("window-request-entry");
        let result = state.ready()?.query_window(&request).await;
        #[cfg(feature = "benchmarks")]
        crate::startup_mark("window-request-complete");
        result
    }
    .await
    .map_err(public_error)
}

#[cfg(feature = "benchmarks")]
#[tauri::command]
pub async fn synthetic_start(
    state: tauri::State<'_, crate::JobState>,
    request: crate::jobs::Request,
) -> Result<crate::jobs::Snapshot, String> {
    state
        .0
        .get()
        .ok_or("Synthetic jobs unavailable")?
        .start(request)
        .await
}
#[cfg(feature = "benchmarks")]
#[tauri::command]
pub fn synthetic_list(
    state: tauri::State<'_, crate::JobState>,
    detailed: Option<bool>,
) -> Result<crate::jobs::Report, String> {
    let mut report = state.0.get().ok_or("Synthetic jobs unavailable")?.report();
    if !detailed.unwrap_or(false) {
        report.scheduling.foreground_ms.clear();
        report.scheduling.worker_wait_ms.clear();
        for job in &mut report.jobs {
            job.pool_wait_ms.clear();
            job.transaction_ms.clear();
        }
    }
    Ok(report)
}
#[cfg(feature = "benchmarks")]
#[tauri::command]
pub fn synthetic_cancel(
    state: tauri::State<'_, crate::JobState>,
    id: u64,
) -> Result<crate::jobs::Snapshot, String> {
    state
        .0
        .get()
        .ok_or("Synthetic jobs unavailable")?
        .cancel(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_matches_the_shared_frontend_contract_fixture() {
        let expected: serde_json::Value =
            serde_json::from_str(include_str!("../../src/ipc/app-status.fixture.json"))
                .expect("valid contract fixture");
        assert_eq!(serde_json::to_value(app_get_status()).unwrap(), expected);
    }

    #[test]
    fn database_requests_are_narrow_and_errors_hide_local_diagnostics() {
        use crate::storage::{ErrorCode, PageRequest, StorageError};
        for input in [
            r#"{"limit":100,"offset":0,"sort":"arbitrarySql"}"#,
            r#"{"limit":-1,"offset":0,"sort":"title"}"#,
            r#"{"limit":100,"offset":0,"sort":"title","sql":"SELECT *"}"#,
        ] {
            assert!(serde_json::from_str::<PageRequest>(input).is_err());
        }
        let error = public_error(StorageError::new(
            ErrorCode::Migration,
            "migrate",
            "private/path and SQL details",
        ));
        assert_eq!(error.code, ErrorCode::Migration);
        assert!(!error.message.contains("private/path"));
    }
}
