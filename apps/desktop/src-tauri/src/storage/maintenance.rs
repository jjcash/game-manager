// SPDX-License-Identifier: GPL-3.0-or-later
//! Explicit database maintenance. Replacement runs only before startup publishes a pool.
use super::{Counts, Database, ErrorCode, MIGRATOR, Result, StorageError, validate_path};
use serde::Serialize;
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tempfile::TempDir;

const MAX_SNAPSHOT_BYTES: u64 = 1024 * 1024 * 1024;

fn error(code: ErrorCode, message: &str) -> StorageError {
    StorageError::new(code, "library_maintenance", message)
}
fn io(_: std::io::Error) -> StorageError {
    error(
        ErrorCode::Io,
        "Local file operation failed. Check permissions, available space and the selected drive. Existing library and recovery files were preserved.",
    )
}
fn local_path(path: &Path) -> Result<()> {
    validate_path(path)?;
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
        let Some(Component::Prefix(prefix)) = path.components().next() else {
            unreachable!("validated Windows prefix");
        };
        let drive = match prefix.kind() {
            Prefix::Disk(d) | Prefix::VerbatimDisk(d) => d,
            _ => unreachable!("validated local prefix"),
        };
        let root = [u16::from(drive), u16::from(b':'), u16::from(b'\\'), 0];
        // SAFETY: a complete, NUL-terminated drive root with no borrowed pointers.
        // GetDriveTypeW: DRIVE_REMOVABLE = 2, DRIVE_FIXED = 3.
        if !matches!(unsafe { GetDriveTypeW(root.as_ptr()) }, 2 | 3)
            || path
                .components()
                .any(|c| matches!(c,Component::Normal(n) if n.to_string_lossy().contains(':')))
        {
            return Err(error(
                ErrorCode::InvalidPath,
                "Use a local fixed or removable drive and an ordinary filename. Network, device and alternate-stream paths are unsupported.",
            ));
        }
    }
    Ok(())
}
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(f).await.map_err(|_| {
        error(
            ErrorCode::Io,
            "Local file task failed; existing data was preserved.",
        )
    })?
}

#[derive(Debug)]
pub(crate) struct Lifecycle {
    pub(crate) path: PathBuf,
    owner: File,
    pub stopping: AtomicBool,
    pub(crate) launches: crate::launch::Admission,
    pub(crate) artwork: std::sync::Arc<crate::local_artwork::Service>,
}
impl Lifecycle {
    pub fn acquire(path: &Path) -> Result<Self> {
        validate_path(path)?;
        let parent = path
            .parent()
            .ok_or_else(|| error(ErrorCode::InvalidPath, "Database parent is missing."))?;
        std::fs::create_dir_all(parent).map_err(io)?;
        let owner = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(component(path, ".gm-lock"))
            .map_err(io)?;
        owner.try_lock().map_err(|_| error(ErrorCode::Conflict, "This library is already open in another Game Manager process. Close it before continuing."))?;
        Ok(Self {
            path: path.into(),
            owner,
            stopping: AtomicBool::new(false),
            launches: Default::default(),
            artwork: crate::local_artwork::Service::new(parent.join("artwork")),
        })
    }
    pub fn release(&self) {
        self.stopping.store(true, Ordering::SeqCst);
        let _ = self.owner.unlock();
    }
}
impl Database {
    pub(crate) fn maintenance_path(&self) -> Result<&Path> {
        let lifecycle = self.lifecycle.as_ref().ok_or_else(|| error(ErrorCode::WrongPurpose, "Maintenance is available only for an application library, never benchmark or job fixtures."))?;
        if lifecycle.stopping.load(Ordering::SeqCst) {
            return Err(error(
                ErrorCode::NotReady,
                "Library is closed for a confirmed restore. Restart Game Manager to continue.",
            ));
        }
        local_path(&lifecycle.path)?;
        Ok(&lifecycle.path)
    }
    pub(crate) fn is_stopping(&self) -> bool {
        self.lifecycle
            .as_ref()
            .is_some_and(|l| l.stopping.load(Ordering::SeqCst))
    }
    pub(crate) fn release_for_restart(&self) -> Result<()> {
        if !self.is_stopping() || !self.pool.is_closed() || self.pool.size() != 0 {
            return Err(error(
                ErrorCode::Conflict,
                "The library must finish closing before restart.",
            ));
        }
        self.lifecycle
            .as_ref()
            .expect("stopped application lifecycle")
            .owner
            .unlock()
            .map_err(io)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub format_version: u32,
    pub application_version: &'static str,
    pub schema_version: i64,
    pub bytes: u64,
    pub counts: Counts,
}
type Schema = Vec<(String, String, String, String)>;
async fn schema(c: &mut SqliteConnection) -> Result<Schema> {
    Ok(sqlx::query_as(
        "SELECT substr(type,1,32),substr(name,1,128),substr(tbl_name,1,128),substr(sql,1,16384) FROM sqlite_schema WHERE sql IS NOT NULL ORDER BY type,name LIMIT 64",
    )
    .fetch_all(c)
    .await?)
}
async fn expected_schema(version: usize) -> Result<&'static Schema> {
    // Preserve the exact historical v2 schema gate; do not migrate a selected file
    // or its preserved candidate merely to determine compatibility.
    static SCHEMAS: tokio::sync::OnceCell<Vec<Schema>> = tokio::sync::OnceCell::const_new();
    let schemas = SCHEMAS
        .get_or_try_init(|| async {
            let mut result = Vec::new();
            for count in [2, 3, 4, 5] {
                let mut c = SqliteConnection::connect("sqlite::memory:").await?;
                let migrator = sqlx::migrate::Migrator {
                    migrations: std::borrow::Cow::Borrowed(&MIGRATOR.migrations[..count]),
                    ..sqlx::migrate::Migrator::DEFAULT
                };
                migrator
                    .run_direct(None, &mut c, false)
                    .await
                    .map_err(|_| {
                        error(
                            ErrorCode::Migration,
                            "Embedded application schema could not be initialized.",
                        )
                    })?;
                result.push(schema(&mut c).await?);
                c.close().await?;
            }
            Ok::<_, StorageError>(result)
        })
        .await?;
    Ok(&schemas[version - 2])
}
/// Standalone exact application schemas v2, v3, v4 and v5 are supported. Validation is
/// read-only; only an installed active copy is upgraded by ordinary initialization.
pub async fn validate_snapshot(path: &Path) -> Result<Snapshot> {
    if tokio::fs::metadata(path).await.map_err(io)?.len() > MAX_SNAPSHOT_BYTES {
        return Err(error(
            ErrorCode::InvalidRequest,
            "This build supports database snapshots up to 1 GiB. No replacement or backup publication was performed.",
        ));
    }
    let mut c = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .read_only(true)
            .immutable(true)
            .foreign_keys(true)
            .pragma("trusted_schema", "OFF")
            .pragma("cache_size", "-2048")
            .statement_cache_capacity(0),
    )
    .await
    .map_err(|_| {
        error(
            ErrorCode::InvalidDatabase,
            "Not a readable standalone SQLite backup.",
        )
    })?;
    let result = validate_connection(&mut c, path).await;
    let closed = c.close().await;
    let report = result.map_err(|e| match e.code {
        ErrorCode::Database => error(
            ErrorCode::InvalidDatabase,
            "Backup is corrupt, truncated or has an invalid application structure.",
        ),
        _ => e,
    })?;
    closed.map_err(|_| {
        error(
            ErrorCode::Database,
            "Backup validation connection did not close cleanly.",
        )
    })?;
    Ok(report)
}
async fn validate_connection(c: &mut SqliteConnection, path: &Path) -> Result<Snapshot> {
    let purpose: String =
        sqlx::query_scalar("SELECT substr(purpose,1,32) FROM database_metadata WHERE singleton=1")
            .fetch_one(&mut *c)
            .await?;
    if purpose != "application" {
        return Err(error(
            ErrorCode::WrongPurpose,
            "This is not an application library backup. Synthetic benchmark databases cannot be restored.",
        ));
    }
    let history: Vec<(i64, bool, Vec<u8>)> =
        sqlx::query_as("SELECT version,success=1,substr(checksum,1,64) FROM _sqlx_migrations ORDER BY version LIMIT 6")
            .fetch_all(&mut *c)
            .await?;
    let version = history.len();
    if !matches!(version, 2..=5)
        || history
            .iter()
            .zip(MIGRATOR.iter())
            .any(|((v, s, h), m)| *v != m.version || !s || h.as_slice() != m.checksum.as_ref())
    {
        return Err(error(
            ErrorCode::Migration,
            "Backup schema or migration history is incompatible with this build. Supported application schemas are v2, v3, v4 and v5; keep the backup for a compatible version.",
        ));
    }
    if schema(c).await? != *expected_schema(version).await? {
        return Err(error(
            ErrorCode::InvalidDatabase,
            "Backup tables, indexes or schema do not match the application. File preserved.",
        ));
    }
    // Bounded result: do not collect potentially unbounded integrity/FK error rows.
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check(1)")
        .fetch_one(&mut *c)
        .await?;
    let foreign_key_error = sqlx::query("PRAGMA foreign_key_check")
        .fetch_optional(&mut *c)
        .await?
        .is_some();
    let invalid_metadata: i64 = sqlx::query_scalar("SELECT count(*) FROM database_metadata WHERE generator_version IS NOT NULL OR dataset_seed IS NOT NULL OR dataset_games IS NOT NULL OR generation_complete<>0").fetch_one(&mut *c).await?;
    let invalid_overrides: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_field_overrides o JOIN games g ON g.id=o.game_id WHERE CASE o.field WHEN 'title' THEN o.value IS NOT g.title WHEN 'release_date' THEN o.value IS NOT g.release_date WHEN 'genre' THEN o.value IS NOT g.genre WHEN 'developer' THEN o.value IS NOT g.developer WHEN 'favorite' THEN o.value IS NOT CAST(g.favorite AS TEXT) END)").fetch_one(&mut *c).await?;
    if integrity != "ok" || foreign_key_error || invalid_metadata != 0 || invalid_overrides {
        return Err(error(
            ErrorCode::InvalidDatabase,
            "Backup integrity, relationships or explicit metadata overrides are invalid. File preserved.",
        ));
    }
    if version >= 3 {
        let invalid_group: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM installation_group_members m JOIN installation_groups g ON g.id=m.group_id JOIN game_installations i ON i.id=m.installation_id JOIN library_entries e ON e.id=i.entry_id WHERE g.game_id<>e.game_id)")
            .fetch_one(&mut *c).await?;
        if invalid_group {
            return Err(error(
                ErrorCode::InvalidDatabase,
                "Installation group relationships are invalid. File preserved.",
            ));
        }
    }
    if version >= 4 {
        super::launch_profiles::validate_all(c).await.map_err(|_| {
            error(
                ErrorCode::InvalidDatabase,
                "LaunchProfile configurations or defaults are invalid. File preserved.",
            )
        })?;
    }
    if version >= 5 {
        let invalid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_artwork a JOIN games g ON g.id=a.game_id WHERE g.origin<>'manual')").fetch_one(&mut *c).await?;
        if invalid {
            return Err(error(
                ErrorCode::InvalidDatabase,
                "Artwork assignment requires a manual game.",
            ));
        }
    }
    let counts = sqlx::query_as("SELECT (SELECT count(*) FROM games) AS games,(SELECT count(*) FROM library_sources) AS library_sources,(SELECT count(*) FROM library_entries) AS library_entries,(SELECT count(*) FROM game_installations) AS installations").fetch_one(&mut *c).await?;
    Ok(Snapshot {
        format_version: 1,
        application_version: env!("CARGO_PKG_VERSION"),
        schema_version: version as i64,
        bytes: tokio::fs::metadata(path).await.map_err(io)?.len(),
        counts,
    })
}

fn cancelled(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::SeqCst) {
        Err(error(
            ErrorCode::InvalidRequest,
            "Backup cancelled before publication. No backup was published.",
        ))
    } else {
        Ok(())
    }
}
/// VACUUM INTO is SQLite's documented consistent live snapshot alternative. All
/// current identities are explicit primary keys; no application identity uses rowid.
async fn vacuum(c: &mut SqliteConnection, destination: &Path) -> Result<()> {
    let name = destination.to_str().ok_or_else(|| {
        error(
            ErrorCode::InvalidPath,
            "Destination is not a valid Unicode path.",
        )
    })?;
    sqlx::query("VACUUM main INTO ?").bind(name).execute(c).await
        .map_err(|_| error(ErrorCode::Database, "Could not create a consistent snapshot. Check available space, permissions and concurrent database activity; current library is unchanged."))?;
    let path = destination.to_owned();
    blocking(move || {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(io)?
            .sync_all()
            .map_err(io)
    })
    .await
}
pub async fn create_backup(
    db: &Database,
    destination: &Path,
    cancel: &Arc<AtomicBool>,
) -> Result<Snapshot> {
    create_backup_with(db, destination, cancel, Faults::default()).await
}
async fn create_backup_with(
    db: &Database,
    destination: &Path,
    cancel: &Arc<AtomicBool>,
    faults: Faults,
) -> Result<Snapshot> {
    db.maintenance_path()?;
    local_path(destination)?;
    cancelled(cancel)?;
    let parent = destination
        .parent()
        .ok_or_else(|| error(ErrorCode::InvalidPath, "Destination has no parent."))?;
    let temporary = tempfile::Builder::new()
        .prefix(".gm-backup-")
        .tempdir_in(parent)
        .map_err(io)?;
    let snapshot = temporary.path().join("snapshot.sqlite3");
    let mut c = db.pool.acquire().await?;
    let result = vacuum(&mut c, &snapshot).await;
    // Explicitly return the source connection before validation/filesystem work.
    drop(c);
    result?;
    #[cfg(test)]
    if faults.fail == Some("backup-partial-output") {
        let mut output = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&snapshot)
            .unwrap();
        output.write_all(&[0; 1024]).unwrap();
        return Err(io(std::io::Error::from(std::io::ErrorKind::StorageFull)));
    }
    faults.at("backup-snapshot")?;
    cancelled(cancel)?;
    let report = validate_snapshot(&snapshot).await?;
    faults.at("backup-validated")?;
    cancelled(cancel)?;
    let destination = destination.to_owned();
    let cancel = Arc::clone(cancel);
    blocking(move || {
        faults.at("backup-publication")?;
        cancelled(&cancel)?;
        // A same-directory hard link publishes only the fully flushed inode and
        // fails if the destination exists. No check-then-rename overwrite race.
        std::fs::hard_link(&snapshot, &destination).map_err(|e| if e.kind() == std::io::ErrorKind::AlreadyExists {
            error(ErrorCode::AlreadyExists, "A file already exists at this destination. Choose a new name; overwriting is disabled.")
        } else { io(e) })?;
        // Preserve complete published output if its final OS flush reports failure.
        OpenOptions::new().read(true).write(true).open(&destination).map_err(io)?.sync_all().map_err(io)?;
        drop(temporary);
        Ok(())
    }).await?;
    Ok(report)
}

#[derive(Debug)]
pub struct PreparedRestore {
    directory: TempDir,
    pub snapshot: Snapshot,
}
fn read_selected(source: &Path, destination: &Path) -> Result<()> {
    local_path(source)?;
    if !std::fs::symlink_metadata(source)
        .map_err(io)?
        .file_type()
        .is_file()
    {
        return Err(error(
            ErrorCode::InvalidPath,
            "Select an ordinary standalone SQLite backup file.",
        ));
    }
    for suffix in ["-wal", "-shm", "-journal"] {
        match std::fs::symlink_metadata(component(source, suffix)) {
            Ok(_) => {
                return Err(error(
                    ErrorCode::InvalidDatabase,
                    "The selected file has SQLite sidecars and may be a live database. Select a standalone Game Manager backup instead.",
                ));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(io(e)),
        }
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Prevent ordinary concurrent writers/deletion for the duration of copy.
        options.share_mode(1);
    }
    let mut input = options.open(source).map_err(io)?;
    let mut header = [0u8; 16];
    if input.read_exact(&mut header).is_err() || &header != b"SQLite format 3\0" {
        return Err(error(
            ErrorCode::InvalidDatabase,
            "Selected file is not a SQLite database backup.",
        ));
    }
    input.rewind().map_err(io)?;
    if input.metadata().map_err(io)?.len() > MAX_SNAPSHOT_BYTES {
        return Err(error(
            ErrorCode::InvalidRequest,
            "This build supports database backups up to 1 GiB. Selected file was preserved.",
        ));
    }
    let mut output = File::create_new(destination).map_err(io)?;
    std::io::copy(&mut input, &mut output).map_err(io)?;
    output.sync_all().map_err(io)
}
pub async fn stage_restore(db: &Database, selected: &Path) -> Result<PreparedRestore> {
    stage_restore_with(db, selected, Faults::default()).await
}
fn staging_root(parent: &Path) -> PathBuf {
    parent.join(".gm-restore-staging")
}
fn ensure_staging(parent: &Path) -> Result<PathBuf> {
    let root = staging_root(parent);
    match std::fs::create_dir(&root) {
        Ok(()) => mark(&root, "owner")?,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            if !owned_marker(&root, "owner") {
                return Err(error(
                    ErrorCode::Conflict,
                    "Restore staging directory is unrecognized. Existing files were preserved.",
                ));
            }
        }
        Err(e) => return Err(io(e)),
    }
    Ok(root)
}
fn ordinary_directory(path: &Path) -> bool {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return false;
    };
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    metadata.file_type().is_dir()
}
fn owned_marker(directory: &Path, name: &str) -> bool {
    ordinary_directory(directory)
        && std::fs::symlink_metadata(directory.join(name))
            .is_ok_and(|m| m.file_type().is_file() && m.len() == 24)
        && std::fs::read(directory.join(name)).is_ok_and(|v| v == b"game-manager-restore-v1\n")
}
fn cleanup_staging(parent: &Path) -> Result<()> {
    let root = staging_root(parent);
    if !owned_marker(&root, "owner") {
        return Ok(());
    }
    for entry in std::fs::read_dir(root).map_err(io)? {
        let entry = entry.map_err(io)?;
        if entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with("gm-restore-"))
            && owned_marker(&entry.path(), "unconfirmed")
        {
            // This directory only contains unconfirmed disposable copies. Confirm
            // promotes the entire directory outside this cleanup scope first.
            std::fs::remove_dir_all(entry.path()).map_err(io)?;
        }
    }
    Ok(())
}
async fn stage_restore_with(
    db: &Database,
    selected: &Path,
    faults: Faults,
) -> Result<PreparedRestore> {
    let parent = db.maintenance_path()?.parent().expect("database parent");
    let root = ensure_staging(parent)?;
    let directory = tempfile::Builder::new()
        .prefix("gm-restore-")
        .tempdir_in(root)
        .map_err(io)?;
    mark(directory.path(), "unconfirmed")?;
    let source = selected.to_owned();
    let destination = directory.path().join("candidate.sqlite3");
    let candidate = destination.clone();
    blocking(move || read_selected(&source, &destination)).await?;
    faults.at("staging-copied")?;
    let snapshot = validate_snapshot(&candidate).await?;
    Ok(PreparedRestore {
        directory,
        snapshot,
    })
}
fn component(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_owned();
    value.push(suffix);
    value.into()
}
fn pending_path(path: &Path) -> PathBuf {
    component(path, ".restore-pending")
}
fn last_path(path: &Path) -> PathBuf {
    component(path, ".restore-last")
}
fn pending_journal(path: &Path) -> Result<Option<PathBuf>> {
    journal_at(path, &pending_path(path))
}
#[cfg(test)]
fn journal(path: &Path) -> Result<Option<PathBuf>> {
    if let Some(directory) = pending_journal(path)? {
        return Ok(Some(directory));
    }
    journal_at(path, &last_path(path))
}
fn journal_at(path: &Path, pointer: &Path) -> Result<Option<PathBuf>> {
    let file = match File::open(pointer) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io(e)),
    };
    let mut bytes = Vec::with_capacity(128);
    file.take(129).read_to_end(&mut bytes).map_err(io)?;
    let name = std::str::from_utf8(&bytes).map_err(|_| {
        error(
            ErrorCode::InvalidDatabase,
            "Restore intent is unreadable. Recovery files were preserved.",
        )
    })?;
    if !name.starts_with("gm-restore-")
        || name.len() > 128
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(error(
            ErrorCode::InvalidDatabase,
            "Restore intent is invalid. Recovery files were preserved.",
        ));
    }
    let directory = path.parent().expect("database parent").join(name);
    if !owned_marker(&directory, "prepared") {
        return Err(error(
            ErrorCode::InvalidDatabase,
            "Restore intent has no prepared recovery set. Files were preserved; manual recovery is required.",
        ));
    }
    Ok(Some(directory))
}
fn mark(directory: &Path, name: &str) -> Result<()> {
    if owned_marker(directory, name) {
        return Ok(());
    }
    let target = directory.join(name);
    match std::fs::symlink_metadata(&target) {
        Ok(_) => {
            // Preserve incomplete/unknown control data rather than overwriting it.
            let remnant = tempfile::Builder::new()
                .prefix("marker-remnant-")
                .tempdir_in(directory)
                .map_err(io)?
                .keep();
            std::fs::rename(&target, remnant.join(name)).map_err(io)?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(io(e)),
    }
    let mut file = tempfile::Builder::new()
        .prefix(".gm-marker-")
        .tempfile_in(directory)
        .map_err(io)?;
    file.write_all(b"game-manager-restore-v1\n").map_err(io)?;
    file.as_file().sync_all().map_err(io)?;
    let published = file.persist_noclobber(target).map_err(|e| io(e.error))?;
    published.sync_all().map_err(io)
}
fn terminal(directory: &Path) -> Result<bool> {
    Ok(owned_marker(directory, "recovered")
        || (!directory.join("recovering").try_exists().map_err(io)?
            && owned_marker(directory, "completed")))
}
pub(crate) fn outcome(path: &Path) -> Result<Option<&'static str>> {
    let pending = pending_journal(path)?;
    // Historical status is not recovery authority. Corrupt history can suppress
    // a status message, but must never replay an old restore over later edits.
    let Some(directory) = pending
        .clone()
        .or_else(|| journal_at(path, &last_path(path)).ok().flatten())
    else {
        return Ok(None);
    };
    if pending.is_none() && !terminal(&directory)? {
        return Ok(None);
    }
    Ok(Some(if owned_marker(&directory, "recovered") {
        "recovered"
    } else if terminal(&directory)? {
        "restored"
    } else {
        "pendingRestart"
    }))
}
impl PreparedRestore {
    /// After explicit confirmation only. No filesystem replacement here.
    pub async fn confirm(self, db: &Database) -> Result<()> {
        self.confirm_with(db, Faults::default()).await
    }
    async fn confirm_with(self, db: &Database, faults: Faults) -> Result<()> {
        let path = db.maintenance_path()?.to_owned();
        // Promotion precedes creation of any original recovery copy. Even an
        // interrupted preparation must never put recovery data in cleanup scope.
        let parent = path.parent().expect("database parent");
        let directory = parent.join(
            self.directory
                .path()
                .file_name()
                .expect("owned staging name"),
        );
        if directory.try_exists().map_err(io)? {
            return Err(error(
                ErrorCode::AlreadyExists,
                "Recovery directory already exists. Current library was preserved.",
            ));
        }
        let staging = self.directory.keep();
        std::fs::rename(&staging, &directory).map_err(io)?;
        let lifecycle = db.lifecycle.as_ref().expect("application lifecycle");
        lifecycle.stopping.store(true, Ordering::SeqCst);
        lifecycle.launches.drain().await;
        lifecycle.artwork.drain().await;
        db.pool.close().await;
        // Retain the process lock even though SQLx is closed. No other application
        // process can open this root until this process exits.
        let previous = directory.join("previous.sqlite3");
        let mut c = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(&path)
                .read_only(true)
                .busy_timeout(Duration::from_secs(5)),
        )
        .await?;
        let captured = vacuum(&mut c, &previous).await;
        let closed = c.close().await;
        captured?;
        closed?;
        validate_snapshot(&previous).await?;
        validate_snapshot(&directory.join("candidate.sqlite3")).await?;
        faults.at("previous-validated")?;
        blocking(move || {
            mark(&directory, "prepared")?;
            if pending_journal(&path)?.is_some() {
                return Err(error(ErrorCode::Conflict, "An earlier restore still requires recovery. All recovery snapshots were preserved."));
            }
            let pointer = directory.join("intent");
            let mut intent = File::create_new(&pointer).map_err(io)?;
            intent.write_all(directory.file_name().expect("owned directory").to_str().expect("ASCII temporary name").as_bytes()).map_err(io)?;
            intent.sync_all().map_err(io)?;
            drop(intent);
            std::fs::hard_link(pointer, pending_path(&path)).map_err(io)?;
            OpenOptions::new().read(true).write(true).open(pending_path(&path)).map_err(io)?.sync_all().map_err(io)
        }).await
    }
}

/// Copy with bounded buffers; publish without replacing any existing name. A byte
/// comparison catches short/corrupt copies before normal SQLite initialization.
fn install(source: &Path, path: &Path) -> Result<()> {
    let directory = tempfile::Builder::new()
        .prefix(".gm-install-")
        .tempdir_in(path.parent().expect("database parent"))
        .map_err(io)?;
    let target = directory.path().join("database.sqlite3");
    let mut input = File::open(source).map_err(io)?;
    let mut output = File::create_new(&target).map_err(io)?;
    std::io::copy(&mut input, &mut output).map_err(io)?;
    output.sync_all().map_err(io)?;
    drop(output);
    drop(input);
    let mut a = File::open(source).map_err(io)?;
    let mut b = File::open(&target).map_err(io)?;
    let (mut x, mut y) = ([0u8; 65536], [0u8; 65536]);
    loop {
        let n = a.read(&mut x).map_err(io)?;
        let m = b.read(&mut y).map_err(io)?;
        if n != m || x[..n] != y[..m] {
            return Err(error(
                ErrorCode::Io,
                "Replacement copy did not verify; recovery snapshots were preserved.",
            ));
        }
        if n == 0 {
            break;
        }
    }
    drop((a, b));
    std::fs::hard_link(&target, path).map_err(io)
}
fn displace(path: &Path, recovery: &Path, faults: Faults) -> Result<()> {
    // Keep before moving anything: failure must never drop displaced originals.
    let directory = tempfile::Builder::new()
        .prefix("displaced-")
        .tempdir_in(recovery)
        .map_err(io)?
        .keep();
    for (suffix, name) in [
        ("", "library.sqlite3"),
        ("-wal", "library.sqlite3-wal"),
        ("-shm", "library.sqlite3-shm"),
        ("-journal", "library.sqlite3-journal"),
    ] {
        let source = component(path, suffix);
        match std::fs::symlink_metadata(&source) {
            Ok(_) => std::fs::rename(source, directory.join(name)).map_err(io)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(io(e)),
        }
        faults.at(name)?;
    }
    Ok(())
}
#[derive(Clone, Copy, Default)]
struct Faults {
    #[cfg(test)]
    fail: Option<&'static str>,
    #[cfg(test)]
    interrupt: Option<&'static str>,
    #[cfg(test)]
    terminate: Option<&'static str>,
}
impl Faults {
    fn at(self, name: &str) -> Result<()> {
        #[cfg(test)]
        {
            if self.terminate == Some(name) {
                std::process::exit(86);
            }
            assert!(
                self.interrupt != Some(name),
                "simulated process interruption at {name}"
            );
            if self.fail == Some(name) {
                return Err(error(ErrorCode::Io, "Injected transition failure"));
            }
        }
        let _ = name;
        Ok(())
    }
}
pub(crate) async fn resume(path: &Path) -> Result<Database> {
    let parent = path.parent().expect("validated database parent").to_owned();
    blocking(move || cleanup_staging(&parent)).await?;
    resume_with(path, Faults::default()).await
}
async fn resume_with(path: &Path, faults: Faults) -> Result<Database> {
    let Some(directory) = pending_journal(path)? else {
        return Database::open(path, "application", true).await;
    };
    if terminal(&directory)? {
        // A complete verified marker commits the data change. Only metadata
        // finalization may follow; never roll committed data back over edits.
        let db = Database::open(path, "application", false).await?;
        if let Err(error) = db.validate().await {
            db.close().await;
            return Err(error);
        }
        return finish_pending(db, path, &directory, faults).await;
    }
    let last = last_path(path);
    let expected = directory
        .file_name()
        .expect("owned journal name")
        .to_str()
        .expect("ASCII journal name")
        .as_bytes();
    if directory.join("archived-intent").try_exists().map_err(io)?
        || (last.try_exists().map_err(io)? && reference_bytes(&last)? == expected)
    {
        return Err(error(
            ErrorCode::InvalidDatabase,
            "Completion history is ambiguous. Active data and all recovery copies were preserved; inspect recovery metadata before continuing.",
        ));
    }
    // Validate the recovery copy before any mutation, even for interrupted attempts.
    validate_snapshot(&directory.join("previous.sqlite3")).await?;
    if !directory.join("attempted").try_exists().map_err(io)?
        && !directory.join("recovering").try_exists().map_err(io)?
    {
        let result = apply(path, &directory, faults).await;
        match result {
            Ok(db) => return finish_pending(db, path, &directory, faults).await,
            // Completion metadata failure after verified installation must not
            // start rollback after intent retirement. Retry finalization instead.
            Err(error) if terminal(&directory)? => return Err(error),
            Err(_) => (),
        }
        // The failed candidate/partial components remain in the recovery directory.
    }
    let recovered = recover(path, &directory, faults).await?;
    finish_pending(recovered, path, &directory, faults).await
}
fn reference_bytes(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(128);
    File::open(path)
        .map_err(io)?
        .take(129)
        .read_to_end(&mut bytes)
        .map_err(io)?;
    Ok(bytes)
}
fn retire_pending(path: &Path, directory: &Path, faults: Faults) -> Result<()> {
    let expected = directory
        .file_name()
        .expect("owned journal name")
        .to_str()
        .expect("ASCII journal name")
        .as_bytes();
    let last = last_path(path);
    if last.try_exists().map_err(io)? && reference_bytes(&last)? != expected {
        let archived = directory.join("previous-status-reference");
        if archived.try_exists().map_err(io)? {
            return Err(error(
                ErrorCode::Conflict,
                "A prior status reference already exists. Recovery files were preserved.",
            ));
        }
        std::fs::rename(&last, archived).map_err(io)?;
    }
    if !last.try_exists().map_err(io)? {
        std::fs::hard_link(directory.join("intent"), &last).map_err(io)?;
    }
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(&last)
        .map_err(io)?
        .sync_all()
        .map_err(io)?;
    faults.at("last-reference")?;
    let pending = pending_path(path);
    let archived = directory.join("archived-intent");
    if !archived.try_exists().map_err(io)? {
        std::fs::hard_link(&pending, &archived).map_err(io)?;
    }
    if reference_bytes(&archived)? != expected {
        return Err(error(
            ErrorCode::InvalidDatabase,
            "Archived restore intent is inconsistent. Files were preserved.",
        ));
    }
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(&archived)
        .map_err(io)?
        .sync_all()
        .map_err(io)?;
    faults.at("intent-archived")?;
    if pending.try_exists().map_err(io)? {
        if reference_bytes(&pending)? != expected {
            return Err(error(
                ErrorCode::InvalidDatabase,
                "Pending restore intent changed. Files were preserved.",
            ));
        }
        // Remove only this fully verified, redundantly preserved control reference.
        // No database or recovery snapshot is deleted. Services remain gated.
        std::fs::remove_file(&pending).map_err(io)?;
    }
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(&archived)
        .map_err(io)?
        .sync_all()
        .map_err(io)?;
    faults.at("pending-retired")
}
async fn finish_pending(
    db: Database,
    path: &Path,
    directory: &Path,
    faults: Faults,
) -> Result<Database> {
    let (path, directory) = (path.to_owned(), directory.to_owned());
    if let Err(error) = blocking(move || retire_pending(&path, &directory, faults)).await {
        db.close().await;
        return Err(error);
    }
    Ok(db)
}
async fn apply(path: &Path, directory: &Path, faults: Faults) -> Result<Database> {
    let candidate = directory.join("candidate.sqlite3");
    let report = validate_snapshot(&candidate).await?;
    mark(directory, "attempted")?;
    faults.at("attempted")?;
    let (p, d) = (path.to_owned(), directory.to_owned());
    blocking(move || {
        displace(&p, &d, faults)?;
        install(&d.join("candidate.sqlite3"), &p)
    })
    .await?;
    faults.at("installed")?;
    #[cfg(all(test, windows))]
    let _denied_reopen = if faults.fail == Some("native-open-denied") {
        use std::os::windows::fs::OpenOptionsExt;
        Some(
            OpenOptions::new()
                .read(true)
                .share_mode(0)
                .open(path)
                .unwrap(),
        )
    } else {
        None
    };
    let db = Database::open(path, "application", false).await?;
    let verified = async {
        faults.at("reopened")?;
        db.validate().await?;
        if db.counts().await? != report.counts {
            return Err(error(
                ErrorCode::InvalidDatabase,
                "Restored library counts did not verify.",
            ));
        }
        faults.at("verified")?;
        mark(directory, "completed")
    }
    .await;
    if let Err(e) = verified {
        db.close().await;
        return Err(e);
    }
    Ok(db)
}
async fn recover(path: &Path, directory: &Path, faults: Faults) -> Result<Database> {
    let report = validate_snapshot(&directory.join("previous.sqlite3")).await?;
    // Recovery intent takes precedence over an earlier completion marker if
    // completion publication/flush failed and rollback was then interrupted.
    mark(directory, "recovering")?;
    let (p, d) = (path.to_owned(), directory.to_owned());
    blocking(move || {
        displace(&p, &d, Faults::default())?;
        faults.at("recovery-displaced")?;
        install(&d.join("previous.sqlite3"), &p)
    })
    .await?;
    faults.at("recovery-installed")?;
    let db = Database::open(path, "application", false).await?;
    let verified = async {
        db.validate().await?;
        if db.counts().await? != report.counts {
            return Err(error(
                ErrorCode::InvalidDatabase,
                "Recovered library counts did not verify.",
            ));
        }
        mark(directory, "recovered")
    }
    .await;
    if let Err(e) = verified {
        db.close().await;
        return Err(e);
    }
    Ok(db)
}

#[cfg(test)]
mod tests;
