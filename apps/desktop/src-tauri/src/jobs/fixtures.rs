// SPDX-License-Identifier: GPL-3.0-or-later
use crate::storage::{
    self, Database,
    synthetic::{self, DatasetConfig},
};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Marker {
    pub version: u32,
    pub games: u32,
    pub seed: u64,
}
const MARKER: &str = "synthetic-jobs-v1.json";
const SCHEMA: &str = "CREATE TABLE simulation_runs (id INTEGER PRIMARY KEY, state TEXT NOT NULL, request TEXT NOT NULL, committed INTEGER NOT NULL DEFAULT 0, processed INTEGER NOT NULL DEFAULT 0, seed TEXT NOT NULL, total INTEGER NOT NULL, digest TEXT NOT NULL, elapsed_ms REAL NOT NULL DEFAULT 0); CREATE TABLE simulation_items (job_id INTEGER NOT NULL REFERENCES simulation_runs(id) ON DELETE CASCADE, ordinal INTEGER NOT NULL, game_id TEXT NOT NULL REFERENCES games(id), digest TEXT NOT NULL, PRIMARY KEY(job_id,ordinal)) STRICT;";
fn error(e: impl ToString) -> storage::StorageError {
    storage::StorageError::new(storage::ErrorCode::InvalidDatabase, "synthetic_jobs", e)
}
pub fn validate_root(root: &Path) -> storage::Result<PathBuf> {
    storage::validate_path(root)?;
    if !root
        .file_name()
        .is_some_and(|n| n.to_string_lossy().starts_with("jobs-"))
        || root.parent().and_then(Path::file_name) != Some(std::ffi::OsStr::new("data"))
        || root
            .parent()
            .and_then(Path::parent)
            .and_then(Path::file_name)
            != Some(std::ffi::OsStr::new("benchmarks"))
    {
        return Err(error("Use a disposable benchmarks/data/jobs-NAME root"));
    }
    let canonical = root.canonicalize().map_err(error)?;
    // Accept Windows verbatim prefixes, reject redirected final components.
    let parent = root.parent().unwrap().canonicalize().map_err(error)?;
    if canonical.parent() != Some(parent.as_path())
        || canonical.file_name() != root.file_name()
        || fs::symlink_metadata(root)
            .map_err(error)?
            .file_type()
            .is_symlink()
    {
        return Err(error("Linked job roots refused"));
    }
    Ok(canonical)
}
pub async fn generate(root: &Path, config: DatasetConfig) -> storage::Result<()> {
    storage::validate_path(root)?;
    if !root
        .file_name()
        .is_some_and(|n| n.to_string_lossy().starts_with("jobs-"))
        || root.parent().and_then(Path::file_name) != Some(std::ffi::OsStr::new("data"))
        || root
            .parent()
            .and_then(Path::parent)
            .and_then(Path::file_name)
            != Some(std::ffi::OsStr::new("benchmarks"))
    {
        return Err(error("Invalid disposable root"));
    }
    fs::create_dir_all(root.parent().unwrap()).map_err(error)?;
    fs::create_dir(root).map_err(error)?;
    synthetic::generate(&root.join("library.sqlite3"), config).await?;
    let db = Database::open(&root.join("library.sqlite3"), "benchmark", false).await?;
    sqlx::raw_sql(SCHEMA).execute(&db.pool).await?;
    db.close().await;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join("hash.bin"))
        .map_err(error)?;
    let bytes: Vec<u8> = (0..256 * 1024u32)
        .map(|index| ((u64::from(index) * 7919).wrapping_add(config.seed) % 251) as u8)
        .collect();
    file.write_all(&bytes).map_err(error)?;
    file.flush().map_err(error)?;
    fs::write(
        root.join(MARKER),
        serde_json::to_vec(&Marker {
            version: 1,
            games: config.games,
            seed: config.seed,
        })
        .map_err(error)?,
    )
    .map_err(error)?;
    Ok(())
}
pub async fn open(root: &Path) -> storage::Result<(Database, Marker, PathBuf)> {
    let root = validate_root(root)?;
    let marker_path = root.join(MARKER);
    if fs::metadata(&marker_path).map_err(error)?.len() > 512 {
        return Err(error("Invalid marker"));
    }
    let marker: Marker =
        serde_json::from_slice(&fs::read(&marker_path).map_err(error)?).map_err(error)?;
    if marker.version != 1 {
        return Err(error("Unsupported job fixture"));
    }
    for file in [MARKER, "library.sqlite3", "hash.bin"] {
        let path = root.join(file);
        if !fs::symlink_metadata(&path).map_err(error)?.is_file()
            || path.canonicalize().map_err(error)? != path
        {
            return Err(error("Job fixture redirected"));
        }
    }
    if fs::metadata(root.join("hash.bin")).map_err(error)?.len() != 256 * 1024 {
        return Err(error("Invalid bounded hash fixture"));
    }
    let path = root.join("library.sqlite3");
    let checked = Database::benchmark_view(&path).await?;
    let config = synthetic::dataset_config(&checked).await?;
    checked.close().await;
    if marker.games != config.games || marker.seed != config.seed {
        return Err(error("Job marker/database mismatch"));
    }
    let pool = SqlitePoolOptions::new()
        .max_connections(storage::POOL_SIZE)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(false)
                .foreign_keys(true)
                .journal_mode(SqliteJournalMode::Wal)
                .synchronous(SqliteSynchronous::Full)
                .busy_timeout(Duration::from_millis(25))
                .pragma("cache_size", "-2048")
                .statement_cache_capacity(64),
        )
        .await?;
    let db = Database {
        pool,
        lifecycle: None,
    };
    let _: i64 = sqlx::query_scalar("SELECT count(*) FROM simulation_runs")
        .fetch_one(&db.pool)
        .await?;
    // No automatic replay: committed ledger is authoritative; in-flight batch rolled back by SQLite.
    sqlx::query("UPDATE simulation_runs SET state='interrupted' WHERE state IN ('queued','running','cancelRequested')").execute(&db.pool).await?;
    Ok((db, marker, root))
}

pub(crate) fn lease(root: &Path) -> storage::Result<fs::File> {
    let path = root.join(".job-owner.lock");
    if path.exists()
        && (!fs::symlink_metadata(&path).map_err(error)?.is_file()
            || path.canonicalize().map_err(error)? != path)
    {
        return Err(error("Unsafe lease file"));
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(error)?;
    file.try_lock().map_err(error)?;
    Ok(file)
}
pub async fn clean(root: &Path) -> storage::Result<()> {
    let root = validate_root(root)?;
    let _lease = lease(&root)?;
    let (db, _, _) = open(&root).await?;
    db.close().await;
    for entry in fs::read_dir(&root).map_err(error)? {
        let entry = entry.map_err(error)?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if ![
            MARKER,
            "hash.bin",
            ".job-owner.lock",
            "library.sqlite3",
            "library.sqlite3-wal",
            "library.sqlite3-shm",
            "library.sqlite3-journal",
        ]
        .contains(&name.as_str())
            || !fs::symlink_metadata(&path).map_err(error)?.is_file()
            || path.canonicalize().map_err(error)? != path
        {
            return Err(error("Unknown fixture content preserved"));
        }
    }
    fs::remove_dir_all(&root).map_err(error)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Audit {
    pub games: i64,
    pub runs: i64,
    pub items: i64,
    pub committed: i64,
    pub interrupted: i64,
    pub integrity: &'static str,
}
pub async fn audit(root: &Path) -> storage::Result<Audit> {
    let root = validate_root(root)?;
    let _lease = lease(&root)?;
    let (db, _, _) = open(&root).await?;
    db.validate().await?;
    let runs = sqlx::query_scalar("SELECT count(*) FROM simulation_runs")
        .fetch_one(&db.pool)
        .await?;
    let items = sqlx::query_scalar("SELECT count(*) FROM simulation_items")
        .fetch_one(&db.pool)
        .await?;
    let committed = sqlx::query_scalar("SELECT COALESCE(sum(committed),0) FROM simulation_runs")
        .fetch_one(&db.pool)
        .await?;
    let interrupted =
        sqlx::query_scalar("SELECT count(*) FROM simulation_runs WHERE state='interrupted'")
            .fetch_one(&db.pool)
            .await?;
    let mismatch:i64=sqlx::query_scalar("SELECT count(*) FROM simulation_runs r WHERE committed!=(SELECT count(*) FROM simulation_items i WHERE i.job_id=r.id) OR committed>processed OR processed>total").fetch_one(&db.pool).await?;
    if mismatch != 0 || runs > 8 {
        return Err(error("Synthetic audit failed"));
    }
    let games = db.counts().await?.games;
    db.close().await;
    Ok(Audit {
        games,
        runs,
        items,
        committed,
        interrupted,
        integrity: "ok",
    })
}
