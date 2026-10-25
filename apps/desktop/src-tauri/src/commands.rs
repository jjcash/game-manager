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
}
