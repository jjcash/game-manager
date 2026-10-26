// SPDX-License-Identifier: GPL-3.0-or-later

use serde::Serialize;

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
        error.message =
            "Database unavailable; inspect local diagnostics. The database was preserved.".into();
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
