// SPDX-License-Identifier: GPL-3.0-or-later
//! One-attempt standalone launcher. No executable authority crosses IPC.
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[cfg(windows)]
mod windows;

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub stage: &'static str,
    pub code: &'static str,
    pub os_code: Option<i32>,
}
pub(crate) fn reject(stage: &'static str, code: &'static str) -> Failure {
    Failure {
        stage,
        code,
        os_code: None,
    }
}
#[cfg(windows)]
fn native(stage: &'static str, error: std::io::Error) -> Failure {
    Failure {
        stage,
        code: if error.kind() == std::io::ErrorKind::PermissionDenied {
            "permissionDenied"
        } else {
            "osFailure"
        },
        os_code: error.raw_os_error(),
    }
}
impl From<sqlx::Error> for Failure {
    fn from(_: sqlx::Error) -> Self {
        reject("request", "databaseUnavailable")
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub game_id: String,
    pub profile_id: String,
    pub expected_profile_revision: i64,
    pub expected_game_revision: i64,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Created {
    pub pid: u32,
}

/// Single active launch, no waiting launch queue. Closure is permanent.
#[derive(Debug)]
pub(crate) struct Admission {
    closed: AtomicBool,
    slot: Arc<Semaphore>,
}
impl Default for Admission {
    fn default() -> Self {
        Self {
            closed: AtomicBool::new(false),
            slot: Arc::new(Semaphore::new(1)),
        }
    }
}
impl Admission {
    pub(crate) fn enter(&self) -> Result<OwnedSemaphorePermit, Failure> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(reject("request", "stopping"));
        }
        let permit = Arc::clone(&self.slot)
            .try_acquire_owned()
            .map_err(|_| reject("request", "busy"))?;
        // Linearizes admission against close; an admitted operation may finish.
        if self.closed.load(Ordering::SeqCst) {
            return Err(reject("request", "stopping"));
        }
        Ok(permit)
    }
    pub(crate) fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
    }
    pub(crate) async fn drain(&self) {
        self.close();
        let _drained = self
            .slot
            .acquire()
            .await
            .expect("admission semaphore never closed");
    }
}

impl crate::storage::Database {
    pub async fn launch_standalone(&self, request: Request) -> Result<Created, Failure> {
        #[cfg(not(windows))]
        {
            let _ = request;
            Err(reject("request", "unsupportedPlatform"))
        }
        #[cfg(windows)]
        {
            let lifecycle = self
                .lifecycle
                .as_ref()
                .ok_or_else(|| reject("request", "wrongPurpose"))?;
            let permit = lifecycle.launches.enter()?;
            if self.is_stopping() || self.pool.is_closed() {
                return Err(reject("request", "stopping"));
            }
            let prepared = {
                let mut connection = self.pool.acquire().await?;
                windows::prepare(&mut connection, request).await?
            };
            let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
            windows::revalidate(&mut tx, &prepared).await?;
            create_with_reservation(tx, permit, move || {
                windows::create(prepared).map(|created| {
                    let pid = created.pid;
                    drop(created.handle);
                    Created { pid }
                })
            })
            .await
        }
    }
}

// Private handoff seam permits deterministic cancellation/lifecycle tests without
// replacing every native test. Production supplies exactly one Win32 operation.
#[cfg(windows)]
async fn create_with_reservation(
    tx: sqlx::Transaction<'static, sqlx::Sqlite>,
    permit: OwnedSemaphorePermit,
    create: impl FnOnce() -> Result<Created, Failure> + Send + 'static,
) -> Result<Created, Failure> {
    let runtime = tokio::runtime::Handle::current();
    // Ownership moves before blocking: IPC cancellation cannot let restore
    // destroy the pool beneath native creation. Admission bounds workers to 1.
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let result = create();
        // Read-only reservation; no acknowledgement commit after spawn.
        // SQLx queues another rollback on failure before pool reuse;
        // cleanup cannot turn irreversible creation into a retryable failure.
        let _ = runtime.block_on(tx.rollback());
        result
    })
    .await
    .map_err(|_| reject("creation", "outcomeUnknown"))?
}

#[tauri::command]
pub async fn standalone_launch(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, crate::DatabaseState>,
    request: Request,
) -> Result<Created, Failure> {
    admit_caller(window.label())?;
    if state.shutting_down.load(Ordering::SeqCst) {
        return Err(reject("request", "stopping"));
    }
    let db = state.ready().map_err(|e| {
        reject(
            "request",
            if e.operation == "restore" {
                "stopping"
            } else {
                "databaseUnavailable"
            },
        )
    })?;
    db.launch_standalone(request).await
}

fn admit_caller(label: &str) -> Result<(), Failure> {
    if label != "main" {
        return Err(reject("request", "unsupportedCaller"));
    }
    Ok(())
}

#[cfg(all(test, windows))]
#[path = "tests.rs"]
mod context_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn admission_closes_before_drain_and_never_queues() {
        let gate = Arc::new(Admission::default());
        let permit = gate.enter().unwrap();
        assert_eq!(gate.enter().unwrap_err().code, "busy");
        gate.close();
        assert_eq!(gate.enter().unwrap_err().code, "stopping");
        let drain = gate.drain();
        tokio::pin!(drain);
        assert!(
            std::future::poll_fn(|cx| std::task::Poll::Ready(drain.as_mut().poll(cx).is_pending()))
                .await
        );
        drop(permit);
        drain.await;
        assert_eq!(gate.enter().unwrap_err().code, "stopping");
    }
    #[test]
    fn ipc_schema_and_errors_do_not_expose_configuration() {
        let value = serde_json::json!({"gameId":"a".repeat(32),"profileId":"b".repeat(32),"expectedProfileRevision":0,"expectedGameRevision":0});
        assert!(serde_json::from_value::<Request>(value.clone()).is_ok());
        for field in [
            "executable",
            "arguments",
            "workingDirectory",
            "environment",
            "authorized",
        ] {
            let mut extra = value.clone();
            extra[field] = serde_json::json!("untrusted");
            assert!(serde_json::from_value::<Request>(extra).is_err());
        }
        assert_eq!(
            serde_json::to_value(reject("request", "staleContext")).unwrap(),
            serde_json::json!({"stage":"request","code":"staleContext","osCode":null})
        );
    }

    #[test]
    fn caller_and_capability_are_local_main_only() {
        admit_caller("main").unwrap();
        for label in ["", "settings", "main2", "remote"] {
            assert_eq!(admit_caller(label).unwrap_err().code, "unsupportedCaller");
        }
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../../capabilities/default.json")).unwrap();
        assert_eq!(capability["windows"], serde_json::json!(["main"]));
        assert!(capability.get("remote").is_none());
        assert_ne!(capability["local"], false);
        let permissions = capability["permissions"].as_array().unwrap();
        assert_eq!(
            permissions
                .iter()
                .filter(|p| **p == "allow-standalone-launch")
                .count(),
            1
        );
        assert!(!permissions.iter().any(
            |p| p.as_str().unwrap().contains("shell") || p.as_str().unwrap().contains("opener")
        ));
        let native = include_str!("windows.rs");
        for forbidden in ["ShellExecute", "cmd /c", "PowerShell", "Command::new"] {
            assert!(!native.contains(forbidden));
        }
    }
}
