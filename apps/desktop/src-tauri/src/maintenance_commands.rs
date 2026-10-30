// SPDX-License-Identifier: GPL-3.0-or-later
//! Main-window maintenance IPC accepts no filesystem paths.
use crate::storage::{
    self, ErrorCode, StorageError,
    maintenance::{self, PreparedRestore, Snapshot},
};
use serde::Serialize;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Default)]
pub(crate) struct Session {
    candidate: Option<PreparedRestore>,
    token: u32,
}
#[derive(Default)]
pub struct MaintenanceState {
    session: tokio::sync::Mutex<Session>,
    cancel: Arc<AtomicBool>,
    closing: AtomicBool,
}
impl MaintenanceState {
    pub(crate) async fn drain_for_shutdown(&self) -> tokio::sync::MutexGuard<'_, Session> {
        self.closing.store(true, Ordering::SeqCst);
        self.cancel.store(true, Ordering::SeqCst);
        self.session.lock().await
    }
    fn admit(&self) -> storage::Result<tokio::sync::MutexGuard<'_, Session>> {
        let session = self.session.try_lock().map_err(|_| busy())?;
        if self.closing.load(Ordering::SeqCst) {
            return Err(busy());
        }
        Ok(session)
    }
}
fn busy() -> StorageError {
    StorageError::new(
        ErrorCode::Conflict,
        "library_maintenance",
        "Another backup or restore request is active. Finish or cancel it before continuing.",
    )
}
fn public(error: StorageError) -> StorageError {
    if error.operation == "library_maintenance" {
        error
    } else {
        StorageError::new(
            error.code,
            "library_maintenance",
            "Library maintenance failed. Existing library and recovery files were preserved. Check the selected drive, permissions and available space.",
        )
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    token: u32,
    snapshot: Snapshot,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    available: bool,
    restart_required: bool,
    outcome: Option<&'static str>,
    candidate: Option<Candidate>,
}

#[tauri::command]
pub async fn backup_get_status(
    state: tauri::State<'_, crate::DatabaseState>,
    maintenance: tauri::State<'_, MaintenanceState>,
) -> storage::Result<Status> {
    let session = maintenance.admit()?;
    let db = state
        .database
        .get()
        .ok_or_else(|| {
            StorageError::new(
                ErrorCode::NotReady,
                "library_maintenance",
                "Library is still initializing.",
            )
        })?
        .as_ref()
        .map_err(|e| public(e.clone()))?;
    Ok(Status {
        available: db.lifecycle.is_some(),
        restart_required: db.is_stopping(),
        outcome: if let Some(l) = &db.lifecycle {
            maintenance::outcome(&l.path).map_err(public)?
        } else {
            None
        },
        candidate: session.candidate.as_ref().map(|c| Candidate {
            token: session.token,
            snapshot: c.snapshot.clone(),
        }),
    })
}
#[tauri::command]
pub async fn backup_create(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, crate::DatabaseState>,
    maintenance: tauri::State<'_, MaintenanceState>,
) -> storage::Result<Option<Snapshot>> {
    let session = maintenance.admit()?;
    if session.candidate.is_some() {
        return Err(busy());
    }
    let db = state.ready().map_err(public)?;
    db.maintenance_path()?;
    maintenance.cancel.store(false, Ordering::SeqCst);
    let Some(path) = choose(&window, true).await? else {
        return Ok(None);
    };
    maintenance::create_backup(db, &path, &maintenance.cancel)
        .await
        .map(Some)
        .map_err(public)
}
#[tauri::command]
pub fn backup_cancel(maintenance: tauri::State<'_, MaintenanceState>) {
    maintenance.cancel.store(true, Ordering::SeqCst);
}
#[tauri::command]
pub async fn backup_select_restore(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, crate::DatabaseState>,
    maintenance: tauri::State<'_, MaintenanceState>,
) -> storage::Result<Option<Candidate>> {
    let mut session = maintenance.admit()?;
    if session.candidate.is_some() {
        return Err(busy());
    }
    let db = state.ready().map_err(public)?;
    db.maintenance_path()?;
    let Some(path) = choose(&window, false).await? else {
        return Ok(None);
    };
    let candidate = maintenance::stage_restore(db, &path)
        .await
        .map_err(public)?;
    session.token = session.token.checked_add(1).ok_or_else(busy)?;
    let report = Candidate {
        token: session.token,
        snapshot: candidate.snapshot.clone(),
    };
    session.candidate = Some(candidate);
    Ok(Some(report))
}
#[tauri::command]
pub async fn backup_cancel_restore(
    maintenance: tauri::State<'_, MaintenanceState>,
) -> storage::Result<()> {
    let mut session = maintenance.admit()?;
    session.candidate.take();
    Ok(())
}
#[tauri::command]
pub async fn backup_confirm_restore(
    state: tauri::State<'_, crate::DatabaseState>,
    maintenance: tauri::State<'_, MaintenanceState>,
    token: u32,
) -> storage::Result<()> {
    let mut session = maintenance.admit()?;
    if session.token != token {
        return Err(busy());
    }
    let db = state.ready().map_err(public)?;
    let candidate = session.candidate.take().ok_or_else(busy)?;
    candidate.confirm(db).await.map_err(public)
}
#[tauri::command]
pub fn backup_restart(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::DatabaseState>,
    maintenance: tauri::State<'_, MaintenanceState>,
) -> storage::Result<()> {
    // Pool closure alone is insufficient: confirmation may still be publishing
    // preserved snapshots/intent. Serialize with the complete confirmation.
    let session = maintenance.admit()?;
    let db = state
        .database
        .get()
        .and_then(|r| r.as_ref().ok())
        .ok_or_else(busy)?;
    if !db.is_stopping() {
        return Err(busy());
    }
    // Tauri spawns before exiting the old process. Only a fully drained, gated
    // database may release its ownership lock for that controlled handoff.
    db.release_for_restart().map_err(public)?;
    maintenance.closing.store(true, Ordering::SeqCst);
    // restart() may wait for ExitRequested; never retain the session while the
    // exit handler drains it. This closed state rejects any later maintenance.
    drop(session);
    app.restart();
}

async fn choose(
    window: &tauri::WebviewWindow,
    save: bool,
) -> storage::Result<Option<std::path::PathBuf>> {
    #[cfg(windows)]
    {
        let owner = window
            .hwnd()
            .map_err(|_| public(StorageError::new(ErrorCode::Io, "dialog", "owner")))?
            .0 as usize;
        tokio::task::spawn_blocking(move || native_dialog(owner, save))
            .await
            .map_err(|_| busy())?
    }
    #[cfg(not(windows))]
    {
        let _ = (window, save);
        Err(StorageError::new(
            ErrorCode::InvalidRequest,
            "library_maintenance",
            "Native backup dialogs require Windows.",
        ))
    }
}

#[cfg(windows)]
fn native_dialog(owner: usize, save: bool) -> storage::Result<Option<std::path::PathBuf>> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::UI::Controls::Dialogs::*;
    let mut filename = vec![0u16; 32768];
    if save {
        for (a, b) in filename
            .iter_mut()
            .zip("game-manager-backup.sqlite3".encode_utf16())
        {
            *a = b;
        }
    }
    let filter: Vec<u16> = "SQLite library backup\0*.sqlite3\0All files\0*.*\0\0"
        .encode_utf16()
        .collect();
    let title: Vec<u16> = (if save {
        "Create library backup — choose a new filename"
    } else {
        "Select library backup to validate"
    })
    .encode_utf16()
    .chain(Some(0))
    .collect();
    let extension: Vec<u16> = "sqlite3\0".encode_utf16().collect();
    // SAFETY: all buffers live across the synchronous native call; lengths are in
    // WCHARs; no callbacks/hooks; the parent HWND belongs to this Tauri window.
    let mut options: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    options.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    options.hwndOwner = owner as _;
    options.lpstrFile = filename.as_mut_ptr();
    options.nMaxFile = filename.len() as u32;
    options.lpstrFilter = filter.as_ptr();
    options.lpstrTitle = title.as_ptr();
    options.lpstrDefExt = extension.as_ptr();
    options.Flags = OFN_EXPLORER
        | OFN_NOCHANGEDIR
        | OFN_PATHMUSTEXIST
        | OFN_DONTADDTORECENT
        | if save { 0 } else { OFN_FILEMUSTEXIST };
    let accepted = unsafe {
        if save {
            GetSaveFileNameW(&mut options)
        } else {
            GetOpenFileNameW(&mut options)
        }
    };
    if accepted == 0 {
        // SAFETY: reads the extended error for the immediately preceding dialog.
        if unsafe { CommDlgExtendedError() } == 0 {
            return Ok(None);
        }
        return Err(StorageError::new(
            ErrorCode::Io,
            "library_maintenance",
            "Native file dialog failed. No file was changed.",
        ));
    }
    let end = filename.iter().position(|v| *v == 0).ok_or_else(busy)?;
    Ok(Some(std::ffi::OsString::from_wide(&filename[..end]).into()))
}

#[cfg(test)]
mod shutdown_tests {
    use super::*;
    #[tokio::test]
    async fn shutdown_closes_maintenance_admission_and_waits_for_active_session() {
        let maintenance = MaintenanceState::default();
        let active = maintenance.admit().unwrap();
        let mut shutdown = Box::pin(maintenance.drain_for_shutdown());
        assert!(
            std::future::poll_fn(|cx| std::task::Poll::Ready(
                shutdown.as_mut().poll(cx).is_pending()
            ))
            .await
        );
        assert!(maintenance.closing.load(Ordering::SeqCst));
        assert!(maintenance.cancel.load(Ordering::SeqCst));
        drop(active);
        drop(shutdown.await);
        assert!(maintenance.admit().is_err());
    }
}
