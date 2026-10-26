// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Connection, FromRow, SqliteConnection, SqlitePool, migrate::Migrator};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

pub mod query;
#[cfg(feature = "benchmarks")]
mod search_evaluation;
#[cfg(any(feature = "benchmarks", test))]
pub mod synthetic;

pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");
pub const MAX_PAGE_SIZE: u32 = 200;
pub const POOL_SIZE: u32 = 4;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    InvalidPath,
    Io,
    Database,
    Migration,
    InvalidDatabase,
    WrongPurpose,
    AlreadyExists,
    InvalidRequest,
    NotReady,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageError {
    pub code: ErrorCode,
    pub operation: &'static str,
    pub message: String,
}

impl StorageError {
    pub fn new(code: ErrorCode, operation: &'static str, message: impl ToString) -> Self {
        Self {
            code,
            operation,
            message: message.to_string(),
        }
    }
}
impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} ({}): {}", self.code, self.operation, self.message)
    }
}
impl std::error::Error for StorageError {}
impl From<sqlx::Error> for StorageError {
    fn from(error: sqlx::Error) -> Self {
        Self::new(ErrorCode::Database, "query", error)
    }
}

pub type Result<T> = std::result::Result<T, StorageError>;

/// Centralized installed-data path; never uses the checkout or current directory.
pub fn application_database_path(
    default_root: &Path,
    override_root: Option<&Path>,
) -> Result<PathBuf> {
    let root = override_root.unwrap_or(default_root);
    validate_path(root)?;
    Ok(root.join("database").join("library.sqlite3"))
}

pub fn validate_path(path: &Path) -> Result<()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(StorageError::new(
            ErrorCode::InvalidPath,
            "path",
            "Use an absolute local path without parent traversal",
        ));
    }
    #[cfg(windows)]
    if !matches!(path.components().next(), Some(std::path::Component::Prefix(p)) if matches!(p.kind(), std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)))
    {
        return Err(StorageError::new(
            ErrorCode::InvalidPath,
            "path",
            "UNC and device paths are unsupported; use a local drive",
        ));
    }
    Ok(())
}

#[derive(Debug)]
pub struct Database {
    pub(crate) pool: SqlitePool,
}

#[derive(Debug, Serialize, FromRow, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub games: i64,
    pub library_sources: i64,
    pub library_entries: i64,
    pub installations: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseStatus {
    pub schema_version: i64,
    pub sqlite_version: String,
    pub purpose: String,
    pub journal_mode: String,
    pub foreign_keys: bool,
    pub pool_limit: u32,
    pub counts: Counts,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SummarySort {
    Title,
    ReleaseDate,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PageRequest {
    pub limit: u32,
    pub offset: u32,
    pub sort: SummarySort,
}

#[derive(Debug, Serialize, FromRow, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GameSummary {
    pub id: String,
    pub title: String,
    pub release_date: Option<String>,
    pub genre: Option<String>,
    pub developer: Option<String>,
    pub favorite: bool,
    pub source_names: String,
    pub entry_count: i64,
    pub installation_count: i64,
    pub installed: bool,
}

/// Details View projection: no descriptions, artwork, or installation paths.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryRow {
    pub id: String,
    pub title: String,
    pub release_date: Option<String>,
    pub source_names: String,
    pub entry_count: i64,
    pub installation_count: i64,
    pub installed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryWindow {
    pub offset: u32,
    pub items: Vec<LibraryRow>,
    pub query_ms: f64,
}

impl From<GameSummary> for LibraryRow {
    fn from(row: GameSummary) -> Self {
        Self {
            id: row.id,
            title: row.title,
            release_date: row.release_date,
            source_names: row.source_names,
            entry_count: row.entry_count,
            installation_count: row.installation_count,
            installed: row.installed,
        }
    }
}

// Limit before correlated aggregates: work and response size follow the page, not the library.
const TITLE_PAGE: &str = "SELECT id FROM games ORDER BY sort_title, id LIMIT ? OFFSET ?";
const RELEASE_PAGE: &str = "SELECT id FROM games ORDER BY release_date, id LIMIT ? OFFSET ?";

impl Database {
    /// Benchmark UI never creates, migrates, checkpoints, or writes the selected file.
    #[cfg(feature = "benchmarks")]
    pub async fn benchmark_view(path: &Path) -> Result<Self> {
        validate_path(path)?;
        let pool = SqlitePoolOptions::new()
            .max_connections(POOL_SIZE)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(path)
                    .read_only(true)
                    .foreign_keys(true),
            )
            .await?;
        let db = Self { pool };
        let purpose: String =
            sqlx::query_scalar("SELECT purpose FROM database_metadata WHERE singleton = 1")
                .fetch_one(&db.pool)
                .await?;
        if purpose != "benchmark" {
            return Err(StorageError::new(
                ErrorCode::WrongPurpose,
                "benchmark_view",
                "Expected a synthetic benchmark database",
            ));
        }
        let applied: Vec<(i64, bool, Vec<u8>)> = sqlx::query_as(
            "SELECT version, success, checksum FROM _sqlx_migrations ORDER BY version",
        )
        .fetch_all(&db.pool)
        .await?;
        if applied.len() != MIGRATOR.iter().count()
            || applied.iter().zip(MIGRATOR.iter()).any(
                |((version, success, checksum), migration)| {
                    *version != migration.version
                        || !success
                        || checksum.as_slice() != migration.checksum.as_ref()
                },
            )
        {
            return Err(StorageError::new(
                ErrorCode::Migration,
                "benchmark_view",
                "Benchmark migration history does not match this build; file preserved",
            ));
        }
        let config = synthetic::dataset_config(&db).await?;
        if db.counts().await?.games != i64::from(config.games) {
            return Err(StorageError::new(
                ErrorCode::InvalidDatabase,
                "benchmark_view",
                "Benchmark count does not match metadata",
            ));
        }
        Ok(db)
    }

    pub async fn window(&self, request: &PageRequest) -> Result<LibraryWindow> {
        let start = std::time::Instant::now();
        let items = self
            .page(request)
            .await?
            .into_iter()
            .map(LibraryRow::from)
            .collect();
        Ok(LibraryWindow {
            offset: request.offset,
            items,
            query_ms: start.elapsed().as_secs_f64() * 1000.0,
        })
    }
    pub async fn initialize(path: &Path) -> Result<Self> {
        Self::open(path, "application", true).await
    }

    pub(crate) async fn open(path: &Path, purpose: &str, allow_empty: bool) -> Result<Self> {
        validate_path(path)?;
        let parent = path.parent().ok_or_else(|| {
            StorageError::new(ErrorCode::InvalidPath, "path", "Database has no parent")
        })?;
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| StorageError::new(ErrorCode::Io, "create_directory", e))?;
        // Inspect before migrations or changing journal mode. Never adopt an unrelated DB.
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(allow_empty)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        let mut connection = SqliteConnection::connect_with(&options)
            .await
            .map_err(|e| StorageError::new(ErrorCode::Database, "connect", e))?;
        let tables: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        )
        .fetch_all(&mut connection)
        .await?;
        if tables.is_empty() && !allow_empty {
            connection.close().await?;
            return Err(StorageError::new(
                ErrorCode::InvalidDatabase,
                "validate_identity",
                "Expected an existing benchmark schema; file preserved",
            ));
        }
        if !tables.is_empty() {
            if !tables.iter().any(|s| s == "database_metadata")
                || !tables.iter().any(|s| s == "_sqlx_migrations")
            {
                return Err(StorageError::new(
                    ErrorCode::InvalidDatabase,
                    "validate_identity",
                    "Unrecognized database; file preserved",
                ));
            }
            let actual: String =
                sqlx::query_scalar("SELECT purpose FROM database_metadata WHERE singleton = 1")
                    .fetch_one(&mut connection)
                    .await?;
            if actual != purpose {
                return Err(StorageError::new(
                    ErrorCode::WrongPurpose,
                    "validate_identity",
                    "Application and benchmark databases cannot be interchanged",
                ));
            }
        }
        // SQLx validates migration checksums and rejects versions absent from this binary.
        MIGRATOR
            // SQLx's direct entry point avoids its generic Acquire lifetime limitation
            // when this initialization future is spawned on Tauri's Send runtime.
            .run_direct(None, &mut connection, false)
            .await
            .map_err(|e| StorageError::new(ErrorCode::Migration, "migrate", e))?;
        if tables.is_empty() && purpose == "benchmark" {
            sqlx::query("UPDATE database_metadata SET purpose = 'benchmark' WHERE singleton = 1")
                .execute(&mut connection)
                .await?;
        }
        connection.close().await?;
        let pool = SqlitePoolOptions::new()
            .max_connections(POOL_SIZE)
            .min_connections(1)
            .acquire_timeout(Duration::from_secs(10))
            .connect_with(
                options
                    .journal_mode(SqliteJournalMode::Wal)
                    .synchronous(SqliteSynchronous::Full)
                    .pragma("cache_size", "-2048")
                    .pragma("wal_autocheckpoint", "1000")
                    .statement_cache_capacity(64),
            )
            .await?;
        Ok(Self { pool })
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }

    pub async fn counts(&self) -> Result<Counts> {
        Ok(sqlx::query_as("SELECT (SELECT count(*) FROM games) AS games, (SELECT count(*) FROM library_sources) AS library_sources, (SELECT count(*) FROM library_entries) AS library_entries, (SELECT count(*) FROM game_installations) AS installations").fetch_one(&self.pool).await?)
    }

    pub async fn status(&self) -> Result<DatabaseStatus> {
        Ok(DatabaseStatus {
            schema_version: sqlx::query_scalar(
                "SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations WHERE success = 1",
            )
            .fetch_one(&self.pool)
            .await?,
            sqlite_version: sqlx::query_scalar("SELECT sqlite_version()")
                .fetch_one(&self.pool)
                .await?,
            purpose: sqlx::query_scalar(
                "SELECT purpose FROM database_metadata WHERE singleton = 1",
            )
            .fetch_one(&self.pool)
            .await?,
            journal_mode: sqlx::query_scalar("PRAGMA journal_mode")
                .fetch_one(&self.pool)
                .await?,
            foreign_keys: sqlx::query_scalar("PRAGMA foreign_keys")
                .fetch_one(&self.pool)
                .await?,
            pool_limit: POOL_SIZE,
            counts: self.counts().await?,
        })
    }

    /// Explicit full validation for tests/benchmarks; no mandatory startup library scan.
    pub async fn validate(&self) -> Result<()> {
        let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
            .fetch_one(&self.pool)
            .await?;
        let violations = sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&self.pool)
            .await?;
        if integrity != "ok" || !violations.is_empty() {
            return Err(StorageError::new(
                ErrorCode::InvalidDatabase,
                "integrity_check",
                integrity,
            ));
        }
        Ok(())
    }

    pub async fn page(&self, request: &PageRequest) -> Result<Vec<GameSummary>> {
        if request.limit == 0 || request.limit > MAX_PAGE_SIZE || request.offset > 1_000_000 {
            return Err(StorageError::new(
                ErrorCode::InvalidRequest,
                "page",
                "Limit must be 1..=200 and offset <= 1000000",
            ));
        }
        Ok(sqlx::query_as(sqlx::AssertSqlSafe(page_sql(request.sort)))
            .bind(request.limit)
            .bind(request.offset)
            .fetch_all(&self.pool)
            .await?)
    }

    #[cfg(feature = "benchmarks")]
    pub async fn page_query_plan(&self, request: &PageRequest) -> Result<Vec<String>> {
        use sqlx::Row;
        let sql = format!("EXPLAIN QUERY PLAN {}", page_sql(request.sort));
        Ok(sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(request.limit)
            .bind(request.offset)
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(|r| r.get::<String, _>(3))
            .collect())
    }
}

fn page_sql(sort: SummarySort) -> String {
    // Only trusted static SQL fragments are interpolated. All values are bound.
    let (page, order) = match sort {
        SummarySort::Title => (TITLE_PAGE, "g.sort_title, g.id"),
        SummarySort::ReleaseDate => (RELEASE_PAGE, "g.release_date, g.id"),
    };
    format!(
        "WITH page AS MATERIALIZED ({page}) {}",
        summary_projection(order)
    )
}

fn summary_projection(order: &str) -> String {
    // Order comes only from private allowlisted sort fragments. Values remain bound.
    format!("SELECT g.id, g.title, g.release_date, g.genre, g.developer, g.favorite,
            COALESCE((SELECT group_concat(display_name, ', ') FROM (SELECT DISTINCT s.display_name FROM library_entries e JOIN library_sources s ON s.id = e.source_id WHERE e.game_id = g.id ORDER BY s.display_name)), '') AS source_names,
            (SELECT count(*) FROM library_entries e WHERE e.game_id = g.id) AS entry_count,
            (SELECT count(*) FROM library_entries e JOIN game_installations i ON i.entry_id = e.id WHERE e.game_id = g.id) AS installation_count,
            EXISTS(SELECT 1 FROM library_entries e JOIN game_installations i ON i.entry_id = e.id WHERE e.game_id = g.id AND i.detection_state = 'present') AS installed
            FROM page JOIN games g ON g.id = page.id ORDER BY {order}")
}

#[cfg(test)]
mod tests;
