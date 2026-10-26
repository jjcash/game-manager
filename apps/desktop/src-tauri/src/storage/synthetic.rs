// SPDX-License-Identifier: GPL-3.0-or-later

use super::{Counts, Database, ErrorCode, Result, StorageError, validate_path};
use serde::Serialize;
use sqlx::{QueryBuilder, Sqlite};
use std::{path::Path, time::Instant};

pub const DATASET_SIZES: [u32; 6] = [500, 2_000, 5_000, 12_320, 15_000, 25_000];
pub const GENERATOR_VERSION: u32 = 1;
const BATCH_SIZE: u32 = 250;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetConfig {
    pub games: u32,
    pub seed: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationReport {
    pub generator_version: u32,
    pub config: DatasetConfig,
    pub initialization_ms: f64,
    pub generation_ms: f64,
    pub database_bytes: u64,
    pub counts: Counts,
}

/// Creates a new file exclusively. There is intentionally no overwrite/reset mode.
pub async fn generate(path: &Path, config: DatasetConfig) -> Result<GenerationReport> {
    if !DATASET_SIZES.contains(&config.games) {
        return Err(StorageError::new(
            ErrorCode::InvalidRequest,
            "generate",
            "Unsupported dataset size",
        ));
    }
    validate_path(path)?;
    let parent = path.parent().ok_or_else(|| {
        StorageError::new(
            ErrorCode::InvalidPath,
            "generate",
            "Missing parent directory",
        )
    })?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|e| StorageError::new(ErrorCode::Io, "create_benchmark_directory", e))?;
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut sidecar = path.as_os_str().to_owned();
        sidecar.push(suffix);
        if tokio::fs::try_exists(Path::new(&sidecar))
            .await
            .map_err(|e| StorageError::new(ErrorCode::Io, "check_sidecar", e))?
        {
            return Err(StorageError::new(
                ErrorCode::AlreadyExists,
                "generate",
                "Existing SQLite sidecar; choose a new filename",
            ));
        }
    }
    tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .await
        .map_err(|e| {
            StorageError::new(
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    ErrorCode::AlreadyExists
                } else {
                    ErrorCode::Io
                },
                "reserve_benchmark",
                e,
            )
        })?;
    let start = Instant::now();
    let db = Database::open(path, "benchmark", true).await?;
    let initialization_ms = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    let result = populate(&db, config).await;
    let generation_ms = start.elapsed().as_secs_f64() * 1000.0;
    if let Err(error) = result {
        db.close().await;
        return Err(error);
    }
    let validation = async {
        db.validate().await?;
        let counts = db.counts().await?;
        if counts.games != i64::from(config.games) {
            return Err(StorageError::new(
                ErrorCode::InvalidDatabase,
                "generation_counts",
                "Generated game count does not match the request",
            ));
        }
        sqlx::query("UPDATE database_metadata SET generation_complete = 1 WHERE singleton = 1")
            .execute(&db.pool)
            .await?;
        Ok(counts)
    }
    .await;
    db.close().await; // Last connection checkpoints WAL; measure the closed main file.
    let counts = validation?;
    let database_bytes = tokio::fs::metadata(path)
        .await
        .map_err(|e| StorageError::new(ErrorCode::Io, "database_size", e))?
        .len();
    Ok(GenerationReport {
        generator_version: GENERATOR_VERSION,
        config,
        initialization_ms,
        generation_ms,
        database_bytes,
        counts,
    })
}

pub async fn open_benchmark(path: &Path) -> Result<Database> {
    if !tokio::fs::try_exists(path)
        .await
        .map_err(|e| StorageError::new(ErrorCode::Io, "open_benchmark", e))?
    {
        return Err(StorageError::new(
            ErrorCode::InvalidPath,
            "open_benchmark",
            "Benchmark database does not exist",
        ));
    }
    Database::open(path, "benchmark", false).await
}

pub async fn dataset_config(db: &Database) -> Result<DatasetConfig> {
    let (seed, games, complete, version): (String, u32, bool, u32) = sqlx::query_as("SELECT dataset_seed, dataset_games, generation_complete, generator_version FROM database_metadata WHERE singleton = 1").fetch_one(&db.pool).await?;
    if !complete || version != GENERATOR_VERSION || !DATASET_SIZES.contains(&games) {
        return Err(StorageError::new(
            ErrorCode::InvalidDatabase,
            "benchmark_metadata",
            "Incomplete or unsupported synthetic dataset; file preserved",
        ));
    }
    Ok(DatasetConfig {
        games,
        seed: seed
            .parse()
            .map_err(|e| StorageError::new(ErrorCode::InvalidDatabase, "seed", e))?,
    })
}

struct Rng(u64);
impl Rng {
    // SplitMix64 with explicit wrapping arithmetic: stable across Rust versions/platforms.
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
}
struct Game {
    id: String,
    title: String,
    sort_title: String,
    release: Option<String>,
    genre: &'static str,
    developer: String,
    favorite: bool,
    timestamp: i64,
}
struct Entry {
    id: String,
    game_id: String,
    source_id: String,
    external_id: String,
    completion: &'static str,
    added: i64,
}
struct Installation {
    id: String,
    entry_id: String,
    root: String,
    version: String,
    state: &'static str,
}

pub(super) async fn populate(db: &Database, config: DatasetConfig) -> Result<()> {
    let purpose: String =
        sqlx::query_scalar("SELECT purpose FROM database_metadata WHERE singleton = 1")
            .fetch_one(&db.pool)
            .await?;
    if purpose != "benchmark" || db.counts().await?.games != 0 {
        return Err(StorageError::new(
            ErrorCode::WrongPurpose,
            "seed",
            "Only a new empty benchmark database can be seeded",
        ));
    }
    let mut tx = db.pool.begin().await?;
    sqlx::query("UPDATE database_metadata SET generator_version = ?, dataset_seed = ?, dataset_games = ?, generation_complete = 0 WHERE singleton = 1")
        .bind(GENERATOR_VERSION).bind(config.seed.to_string()).bind(config.games).execute(&mut *tx).await?;
    for (id, kind, label) in [
        ("a", "synthetic_store", "Synthetic Store A"),
        ("b", "synthetic_store", "Synthetic Store B"),
        ("c", "synthetic_store", "Synthetic Store C"),
        ("manual", "manual", "Synthetic Manual"),
        ("archive", "synthetic_archive", "Synthetic Archive"),
    ] {
        sqlx::query("INSERT INTO library_sources VALUES (?, ?, ?)")
            .bind(id)
            .bind(kind)
            .bind(label)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    let mut rng = Rng(config.seed);
    let prefixes = [
        "Amber", "Silent", "Crimson", "Lunar", "Hidden", "Ancient", "Neon", "Frozen", "Silver",
        "Broken", "Emerald", "Golden", "Distant", "Wild", "Café", "星の",
    ];
    let nouns = [
        "Harbor",
        "Kingdom",
        "Voyager",
        "Forest",
        "Citadel",
        "Horizons",
        "Engine",
        "Chronicles",
        "Labyrinth",
        "Skies",
        "Frontier",
        "Islands",
        "Odyssey",
        "Valley",
        "Dreams",
        "Expedition",
    ];
    let subtitles = [
        "Awakening",
        "Beyond the Gate",
        "Echoes",
        "Rebirth",
        "Lost Worlds",
        "Origins",
        "Afterlight",
        "The Return",
    ];
    let genres = [
        "Action",
        "RPG",
        "Strategy",
        "Simulation",
        "Adventure",
        "Puzzle",
        "Racing",
        "Platformer",
    ];
    for begin in (0..config.games).step_by(BATCH_SIZE as usize) {
        let mut games = Vec::with_capacity(BATCH_SIZE as usize);
        let mut entries = Vec::new();
        let mut installations = Vec::new();
        for i in begin..(begin + BATCH_SIZE).min(config.games) {
            let id = format!("syn-{:016x}-g-{i:05}", config.seed);
            let key = (u64::from(i) * 7919 + config.seed % 32768) % 32768;
            let title = format!(
                "{} {}: {} {}",
                prefixes[(key % 16) as usize],
                nouns[((key / 16) % 16) as usize],
                subtitles[((key / 256) % 8) as usize],
                key / 2048 + 1
            );
            let release = if rng.next().is_multiple_of(20) {
                None
            } else {
                Some(format!(
                    "{:04}-{:02}-{:02}",
                    1985 + rng.next() % 41,
                    1 + rng.next() % 12,
                    1 + rng.next() % 28
                ))
            };
            let source = match rng.next() % 100 {
                0..50 => "a",
                50..70 => "b",
                70..85 => "c",
                85..95 => "manual",
                _ => "archive",
            };
            let timestamp = 1_600_000_000_000 + (rng.next() % 150_000_000_000) as i64;
            games.push(Game {
                id: id.clone(),
                sort_title: title.to_lowercase(),
                title,
                release,
                genre: genres[(rng.next() % 8) as usize],
                developer: format!("Synthetic Studio {}", rng.next() % 60 + 1),
                favorite: rng.next().is_multiple_of(10),
                timestamp,
            });
            for entry_index in 0..if i % 7 == 0 { 2 } else { 1 } {
                let entry_id = format!("{id}-e-{entry_index}");
                let entry_source = if entry_index == 0 {
                    source
                } else if source == "a" {
                    "b"
                } else {
                    "a"
                };
                entries.push(Entry {
                    id: entry_id.clone(),
                    game_id: id.clone(),
                    source_id: entry_source.into(),
                    external_id: format!("synthetic-item-{i}"),
                    completion: match rng.next() % 100 {
                        0..55 => "not_started",
                        55..70 => "playing",
                        70..92 => "completed",
                        _ => "abandoned",
                    },
                    added: timestamp,
                });
                if i % 100 < 62 && (entry_index == 0 || i % 3 == 0) {
                    for copy in 0..if entry_index == 0 && i % 19 == 0 {
                        2
                    } else {
                        1
                    } {
                        installations.push(Installation {
                            id: format!("{entry_id}-i-{copy}"),
                            entry_id: entry_id.clone(),
                            root: format!("synthetic://volume-{copy}/{i}/{entry_index}"),
                            version: format!("{}.{}.0", 1 + rng.next() % 4, rng.next() % 10),
                            state: if i % 13 == 0 { "missing" } else { "present" },
                        });
                    }
                }
            }
        }
        let mut tx = db.pool.begin().await?;
        let mut query = QueryBuilder::<Sqlite>::new(
            "INSERT INTO games (id, title, sort_title, release_date, genre, developer, favorite, created_at_ms, updated_at_ms) ",
        );
        query.push_values(&games, |mut b, g| {
            b.push_bind(&g.id)
                .push_bind(&g.title)
                .push_bind(&g.sort_title)
                .push_bind(&g.release)
                .push_bind(g.genre)
                .push_bind(&g.developer)
                .push_bind(g.favorite)
                .push_bind(g.timestamp)
                .push_bind(g.timestamp);
        });
        query.build().execute(&mut *tx).await?;
        let mut query = QueryBuilder::<Sqlite>::new(
            "INSERT INTO library_entries (id, game_id, source_id, external_item_id, completion_status, added_at_ms) ",
        );
        query.push_values(&entries, |mut b, e| {
            b.push_bind(&e.id)
                .push_bind(&e.game_id)
                .push_bind(&e.source_id)
                .push_bind(&e.external_id)
                .push_bind(e.completion)
                .push_bind(e.added);
        });
        query.build().execute(&mut *tx).await?;
        if !installations.is_empty() {
            let mut query = QueryBuilder::<Sqlite>::new(
                "INSERT INTO game_installations (id, entry_id, root_path_ref, platform, version, detection_state) ",
            );
            query.push_values(&installations, |mut b, i| {
                b.push_bind(&i.id)
                    .push_bind(&i.entry_id)
                    .push_bind(&i.root)
                    .push_bind("windows")
                    .push_bind(&i.version)
                    .push_bind(i.state);
            });
            query.build().execute(&mut *tx).await?;
        }
        tx.commit().await?;
    }
    Ok(())
}
