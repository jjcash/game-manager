// SPDX-License-Identifier: GPL-3.0-or-later

use serde::Serialize;

#[tauri::command]
pub async fn artwork_get_thumbnail(
    state: tauri::State<'_, crate::ArtworkState>,
    id: String,
) -> Result<crate::artwork::Reply, String> {
    Ok(if let Some(service) = &state.0 {
        service.request(&id).await
    } else {
        crate::artwork::Reply::empty("missing")
    })
}

#[tauri::command]
pub fn artwork_get_stats(state: tauri::State<'_, crate::ArtworkState>) -> crate::artwork::Stats {
    state
        .0
        .as_ref()
        .map_or_else(Default::default, |s| s.stats())
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
    if !matches!(error.code, ErrorCode::NotReady | ErrorCode::InvalidRequest) {
        eprintln!("{}", serde_json::to_string(&error).unwrap_or_default());
        error.message =
            "Database unavailable. Check the configured data location and local diagnostics, then restart Game Manager. The database was preserved.".into();
    }
    error
}

#[tauri::command]
pub async fn database_get_status(
    state: tauri::State<'_, crate::DatabaseState>,
) -> crate::storage::Result<crate::storage::DatabaseStatus> {
    async { state.ready()?.status().await }
        .await
        .map_err(public_error)
}

#[tauri::command]
pub async fn library_get_page(
    state: tauri::State<'_, crate::DatabaseState>,
    request: crate::storage::PageRequest,
) -> crate::storage::Result<Vec<crate::storage::GameSummary>> {
    async { state.ready()?.page(&request).await }
        .await
        .map_err(public_error)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryInfo {
    total: i64,
    source: String,
    diagnostics: bool,
    library_total: i64,
    sources: Vec<crate::storage::query::SourceOption>,
    query_ms: f64,
}

#[tauri::command]
pub async fn library_get_info(
    state: tauri::State<'_, crate::DatabaseState>,
    criteria: Option<crate::storage::query::Criteria>,
) -> crate::storage::Result<LibraryInfo> {
    async {
        let criteria = criteria.unwrap_or_default();
        let start = std::time::Instant::now();
        let total = state.ready()?.query_count(&criteria).await?;
        let query_ms = start.elapsed().as_secs_f64() * 1000.0;
        Ok(LibraryInfo {
            total,
            library_total: state.ready()?.counts().await?.games,
            sources: state.ready()?.source_options().await?,
            query_ms,
            source: state.source.clone(),
            diagnostics: cfg!(any(debug_assertions, feature = "benchmarks")),
        })
    }
    .await
    .map_err(public_error)
}

#[tauri::command]
pub async fn library_get_window(
    state: tauri::State<'_, crate::DatabaseState>,
    request: crate::storage::PageRequest,
) -> crate::storage::Result<crate::storage::LibraryWindow> {
    async { state.ready()?.window(&request).await }
        .await
        .map_err(public_error)
}

#[tauri::command]
pub async fn library_query(
    state: tauri::State<'_, crate::DatabaseState>,
    request: crate::storage::query::QueryRequest,
) -> crate::storage::Result<crate::storage::LibraryWindow> {
    async { state.ready()?.query_window(&request).await }
        .await
        .map_err(public_error)
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
