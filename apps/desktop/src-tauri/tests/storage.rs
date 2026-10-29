// SPDX-License-Identifier: GPL-3.0-or-later

use game_manager_lib::storage::{self, Database, ErrorCode, PageRequest, SummarySort};
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
use std::path::Path;
use tempfile::tempdir;

async fn raw(path: &Path) -> SqliteConnection {
    SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .foreign_keys(true),
    )
    .await
    .unwrap()
}
fn page(limit: u32, offset: u32) -> PageRequest {
    PageRequest {
        limit,
        offset,
        sort: SummarySort::Title,
    }
}

#[tokio::test]
async fn fresh_migrations_empty_behavior_and_reopen_preserve_records() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("library.sqlite3");
    let db = Database::initialize(&path).await.unwrap();
    let status = db.status().await.unwrap();
    assert_eq!(status.schema_version, 4);
    assert_eq!(status.journal_mode, "wal");
    assert!(status.foreign_keys);
    assert_eq!(status.pool_limit, 4);
    assert_eq!(status.counts.games, 0);
    assert!(db.page(&page(100, 0)).await.unwrap().is_empty());
    db.validate().await.unwrap();
    db.close().await;
    let mut conn = raw(&path).await;
    sqlx::query("INSERT INTO games(id, title, sort_title, created_at_ms, updated_at_ms) VALUES ('g', 'Test', 'test', 0, 0)").execute(&mut conn).await.unwrap();
    conn.close().await.unwrap();
    let db = Database::initialize(&path).await.unwrap();
    assert_eq!(db.counts().await.unwrap().games, 1);
    let rows = db.page(&page(10, 0)).await.unwrap();
    assert_eq!(rows[0].id, "g");
    assert_eq!(rows[0].source_names, "");
    assert!(!rows[0].installed);
    db.validate().await.unwrap();
    db.close().await;
    let mut conn = raw(&path).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM _sqlx_migrations")
            .fetch_one(&mut conn)
            .await
            .unwrap(),
        4
    );
    conn.close().await.unwrap();
}

#[tokio::test]
async fn identity_constraints_foreign_keys_and_multiple_relationships() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("library.sqlite3");
    let db = Database::initialize(&path).await.unwrap();
    let mut c = raw(&path).await;
    sqlx::raw_sql("INSERT INTO games(id,title,sort_title,release_date,genre,developer,favorite,created_at_ms,updated_at_ms) VALUES ('g', 'Title', 'title', NULL, NULL, NULL, 0, 0, 0);
        INSERT INTO library_sources VALUES ('a', 'manual', 'A'), ('b', 'manual', 'B');
        INSERT INTO library_entries VALUES ('e1', 'g', 'a', '1', 'playing', 0), ('e2', 'g', 'b', '1', 'completed', 0);
        INSERT INTO game_installations(id,entry_id,root_path_ref,platform,version,detection_state) VALUES ('i1', 'e1', 'synthetic://one', 'windows', '1', 'present'), ('i2', 'e1', 'synthetic://two', 'windows', '2', 'missing'), ('i3', 'e2', 'synthetic://three', 'windows', '1', 'present');")
        .execute(&mut c).await.unwrap();
    for sql in [
        "INSERT INTO games(id,title,sort_title,release_date,genre,developer,favorite,created_at_ms,updated_at_ms) VALUES ('g', 'Duplicate', 'duplicate', NULL, NULL, NULL, 0, 0, 0)",
        "INSERT INTO library_entries VALUES ('bad', 'absent', 'a', '2', 'playing', 0)",
        "INSERT INTO library_entries VALUES ('bad', 'g', 'absent', '2', 'playing', 0)",
        "INSERT INTO library_entries VALUES ('bad', 'g', 'a', '1', 'playing', 0)",
        "INSERT INTO game_installations(id,entry_id,root_path_ref,platform,version,detection_state) VALUES ('bad', 'absent', 'synthetic://bad', 'windows', '1', 'present')",
        "INSERT INTO game_installations(id,entry_id,root_path_ref,platform,version,detection_state) VALUES ('bad', 'e1', 'synthetic://one', 'windows', '1', 'present')",
        "UPDATE game_installations SET detection_state = 'invalid'",
        "UPDATE games SET favorite = 2",
        "DELETE FROM games WHERE id = 'g'",
        "DELETE FROM library_sources WHERE id = 'a'",
        "DELETE FROM library_entries WHERE id = 'e1'",
    ] {
        assert!(
            sqlx::query(sql).execute(&mut c).await.is_err(),
            "constraint should reject {sql}"
        );
    }
    let rows = db.page(&page(100, 0)).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].entry_count, 2);
    assert_eq!(rows[0].installation_count, 3);
    assert_eq!(rows[0].source_names, "A, B");
    assert!(rows[0].installed);
    c.close().await.unwrap();
    db.validate().await.unwrap();
    db.close().await;
}

#[tokio::test]
async fn bounded_pagination_and_tied_order_are_stable() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("library.sqlite3");
    let db = Database::initialize(&path).await.unwrap();
    let mut c = raw(&path).await;
    for id in ["c", "a", "b"] {
        sqlx::query("INSERT INTO games(id,title,sort_title,release_date,genre,developer,favorite,created_at_ms,updated_at_ms) VALUES (?, 'Same title', 'same title', '2000-01-01', NULL, NULL, 0, 0, 0)").bind(id).execute(&mut c).await.unwrap();
    }
    for sort in [SummarySort::Title, SummarySort::ReleaseDate] {
        let first = db
            .page(&PageRequest {
                limit: 2,
                offset: 0,
                sort,
            })
            .await
            .unwrap();
        assert_eq!(
            first.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(
            db.page(&PageRequest {
                limit: 2,
                offset: 2,
                sort
            })
            .await
            .unwrap()[0]
                .id,
            "c"
        );
        assert_eq!(
            db.page(&PageRequest {
                limit: 2,
                offset: 0,
                sort
            })
            .await
            .unwrap(),
            first
        );
    }
    assert!(db.page(&page(1, 4)).await.unwrap().is_empty());
    for request in [page(0, 0), page(201, 0), page(100, 1_000_001)] {
        assert_eq!(
            db.page(&request).await.unwrap_err().code,
            ErrorCode::InvalidRequest
        );
    }
    c.close().await.unwrap();
    db.close().await;
}

#[tokio::test]
async fn invalid_inaccessible_and_unrelated_paths_are_preserved() {
    let dir = tempdir().unwrap();
    assert_eq!(
        Database::initialize(Path::new("relative.sqlite3"))
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidPath
    );
    assert_eq!(
        Database::initialize(&dir.path().join("..").join("bad.sqlite3"))
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidPath
    );
    assert!(Database::initialize(dir.path()).await.is_err());
    let blocker = dir.path().join("not-a-directory");
    std::fs::write(&blocker, b"keep me").unwrap();
    assert_eq!(
        Database::initialize(&blocker.join("db.sqlite3"))
            .await
            .unwrap_err()
            .code,
        ErrorCode::Io
    );
    assert_eq!(std::fs::read(&blocker).unwrap(), b"keep me");
    let corrupt = dir.path().join("corrupt.sqlite3");
    std::fs::write(&corrupt, b"not a database").unwrap();
    assert!(Database::initialize(&corrupt).await.is_err());
    assert_eq!(std::fs::read(&corrupt).unwrap(), b"not a database");
    let unrelated = dir.path().join("unrelated.sqlite3");
    let mut c = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&unrelated)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TABLE personal_data(value TEXT)")
        .execute(&mut c)
        .await
        .unwrap();
    sqlx::query("INSERT INTO personal_data VALUES ('preserve')")
        .execute(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    let before = std::fs::read(&unrelated).unwrap();
    assert_eq!(
        Database::initialize(&unrelated).await.unwrap_err().code,
        ErrorCode::InvalidDatabase
    );
    assert_eq!(std::fs::read(&unrelated).unwrap(), before);
    #[cfg(windows)]
    assert_eq!(
        Database::initialize(Path::new(r"\\server\share\db.sqlite3"))
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidPath
    );
}

#[test]
fn installed_paths_are_centralized_and_independent_of_checkout() {
    let dir = tempdir().unwrap();
    let custom = dir.path().join("custom");
    assert_eq!(
        storage::application_database_path(dir.path(), None).unwrap(),
        dir.path().join("database/library.sqlite3")
    );
    assert_eq!(
        storage::application_database_path(dir.path(), Some(&custom)).unwrap(),
        custom.join("database/library.sqlite3")
    );
    assert!(storage::application_database_path(dir.path(), Some(Path::new("relative"))).is_err());
}

#[tokio::test]
async fn checksum_and_newer_schema_fail_without_recreating_database() {
    for failure in ["checksum", "newer", "dirty"] {
        let dir = tempdir().unwrap();
        let path = dir.path().join("library.sqlite3");
        let db = Database::initialize(&path).await.unwrap();
        db.close().await;
        let mut c = raw(&path).await;
        sqlx::query("INSERT INTO games(id,title,sort_title,release_date,genre,developer,favorite,created_at_ms,updated_at_ms) VALUES ('keep', 'Keep', 'keep', NULL, NULL, NULL, 0, 0, 0)")
            .execute(&mut c)
            .await
            .unwrap();
        if failure == "newer" {
            sqlx::query("INSERT INTO _sqlx_migrations(version, description, success, checksum, execution_time) VALUES (999, 'future', 1, X'00', 0)").execute(&mut c).await.unwrap();
        } else if failure == "dirty" {
            sqlx::query("UPDATE _sqlx_migrations SET success = 0 WHERE version = 1")
                .execute(&mut c)
                .await
                .unwrap();
        } else {
            sqlx::query("UPDATE _sqlx_migrations SET checksum = X'00' WHERE version = 1")
                .execute(&mut c)
                .await
                .unwrap();
        }
        c.close().await.unwrap();
        assert_eq!(
            Database::initialize(&path).await.unwrap_err().code,
            ErrorCode::Migration
        );
        let mut c = raw(&path).await;
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT title FROM games WHERE id = 'keep'")
                .fetch_one(&mut c)
                .await
                .unwrap(),
            "Keep"
        );
        c.close().await.unwrap();
    }
}

#[tokio::test]
async fn failing_migration_rolls_back_ddl_and_preserves_existing_records() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("library.sqlite3");
    let db = Database::initialize(&path).await.unwrap();
    db.close().await;
    let mut c = raw(&path).await;
    sqlx::query("INSERT INTO games(id,title,sort_title,release_date,genre,developer,favorite,created_at_ms,updated_at_ms) VALUES ('keep', 'Keep', 'keep', NULL, NULL, NULL, 0, 0, 0)")
        .execute(&mut c)
        .await
        .unwrap();
    let migration_dir = dir.path().join("migrations");
    std::fs::create_dir(&migration_dir).unwrap();
    std::fs::write(
        migration_dir.join("0001_library.sql"),
        include_str!("../migrations/0001_library.sql"),
    )
    .unwrap();
    std::fs::write(
        migration_dir.join("0002_manual_library.sql"),
        include_str!("../migrations/0002_manual_library.sql"),
    )
    .unwrap();
    std::fs::write(
        migration_dir.join("0003_installation_groups.sql"),
        include_str!("../migrations/0003_installation_groups.sql"),
    )
    .unwrap();
    std::fs::write(
        migration_dir.join("0004_launch_profiles.sql"),
        include_str!("../migrations/0004_launch_profiles.sql"),
    )
    .unwrap();
    std::fs::write(
        migration_dir.join("0005_failure.sql"),
        "CREATE TABLE should_rollback(id TEXT); INSERT INTO nonexistent VALUES (1);",
    )
    .unwrap();
    let migrator = sqlx::migrate::Migrator::new(migration_dir.as_path())
        .await
        .unwrap();
    assert!(migrator.run(&mut c).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM sqlite_master WHERE name = 'should_rollback'"
        )
        .fetch_one(&mut c)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM games WHERE id = 'keep'")
            .fetch_one(&mut c)
            .await
            .unwrap(),
        1
    );
    c.close().await.unwrap();
    let db = Database::initialize(&path).await.unwrap();
    assert_eq!(db.status().await.unwrap().schema_version, 4);
    db.close().await;
}

#[cfg(feature = "benchmarks")]
mod synthetic_tests {
    use super::*;
    use storage::synthetic::{self, DatasetConfig};
    async fn snapshot(path: &Path) -> Vec<String> {
        let mut c = raw(path).await;
        let mut output = Vec::new();
        for sql in [
            "SELECT json_group_array(json_array(id,title,sort_title,release_date,genre,developer,favorite,created_at_ms,updated_at_ms)) FROM (SELECT * FROM games ORDER BY id)",
            "SELECT json_group_array(json_array(id,source_type,display_name)) FROM (SELECT * FROM library_sources ORDER BY id)",
            "SELECT json_group_array(json_array(id,game_id,source_id,external_item_id,completion_status,added_at_ms)) FROM (SELECT * FROM library_entries ORDER BY id)",
            "SELECT json_group_array(json_array(id,entry_id,root_path_ref,platform,version,detection_state)) FROM (SELECT * FROM game_installations ORDER BY id)",
        ] {
            output.push(sqlx::query_scalar(sql).fetch_one(&mut c).await.unwrap());
        }
        c.close().await.unwrap();
        output
    }

    #[tokio::test]
    async fn generation_is_deterministic_and_seed_changes_content() {
        let dir = tempdir().unwrap();
        let mut snapshots = Vec::new();
        for (name, seed) in [("a", 42), ("b", 42), ("c", 43)] {
            let path = dir.path().join(format!("{name}.sqlite3"));
            synthetic::generate(&path, DatasetConfig { games: 500, seed })
                .await
                .unwrap();
            snapshots.push(snapshot(&path).await);
        }
        assert_eq!(snapshots[0], snapshots[1]);
        assert_ne!(snapshots[0], snapshots[2]);
    }

    #[tokio::test]
    async fn all_six_sizes_counts_integrity_distribution_and_index_plans() {
        let dir = tempdir().unwrap();
        for n in synthetic::DATASET_SIZES {
            let path = dir.path().join(format!("{n}.sqlite3"));
            let report = synthetic::generate(&path, DatasetConfig { games: n, seed: 42 })
                .await
                .unwrap();
            assert_eq!(report.counts.games, i64::from(n));
            assert_eq!(report.counts.library_sources, 5);
            assert_eq!(report.counts.library_entries, i64::from(n + n.div_ceil(7)));
            let expected_installations: u32 = (0..n)
                .map(|i| {
                    if i % 100 < 62 {
                        1 + u32::from(i % 19 == 0) + u32::from(i % 7 == 0 && i % 3 == 0)
                    } else {
                        0
                    }
                })
                .sum();
            assert_eq!(
                report.counts.installations,
                i64::from(expected_installations)
            );
            assert!(report.database_bytes > 0);
            let db = synthetic::open_benchmark(&path).await.unwrap();
            db.validate().await.unwrap();
            assert_eq!(synthetic::dataset_config(&db).await.unwrap().games, n);
            for (sort, index) in [
                (SummarySort::Title, "ix_games_sort"),
                (SummarySort::ReleaseDate, "ix_games_release"),
            ] {
                let request = PageRequest {
                    limit: 100,
                    offset: n / 2,
                    sort,
                };
                assert_eq!(db.page(&request).await.unwrap().len(), 100);
                assert!(
                    db.page_query_plan(&request)
                        .await
                        .unwrap()
                        .iter()
                        .any(|line| line.contains(index))
                );
            }
            db.close().await;
            let mut c = raw(&path).await;
            assert_eq!(
                sqlx::query_scalar::<_, i64>("SELECT count(DISTINCT title) FROM games")
                    .fetch_one(&mut c)
                    .await
                    .unwrap(),
                i64::from(n)
            );
            assert_eq!(
                sqlx::query_scalar::<_, i64>(
                    "SELECT count(DISTINCT source_id) FROM library_entries"
                )
                .fetch_one(&mut c)
                .await
                .unwrap(),
                5
            );
            assert!(
                sqlx::query_scalar::<_, i64>(
                    "SELECT count(*) FROM games WHERE release_date IS NULL"
                )
                .fetch_one(&mut c)
                .await
                .unwrap()
                    > 0
            );
            assert!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM games g WHERE NOT EXISTS (SELECT 1 FROM library_entries e JOIN game_installations i ON i.entry_id=e.id WHERE e.game_id=g.id)").fetch_one(&mut c).await.unwrap() > 0);
            c.close().await.unwrap();
        }
    }

    #[tokio::test]
    async fn generator_refuses_existing_normal_and_benchmark_files_and_orphan_sidecars() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("library.sqlite3");
        let config = DatasetConfig {
            games: 500,
            seed: 42,
        };
        let db = Database::initialize(&path).await.unwrap();
        db.close().await;
        let before = std::fs::read(&path).unwrap();
        assert_eq!(
            synthetic::generate(&path, config).await.unwrap_err().code,
            ErrorCode::AlreadyExists
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(
            synthetic::open_benchmark(&path).await.unwrap_err().code,
            ErrorCode::WrongPurpose
        );
        let benchmark = dir.path().join("benchmark.sqlite3");
        synthetic::generate(&benchmark, config).await.unwrap();
        assert_eq!(
            synthetic::generate(&benchmark, config)
                .await
                .unwrap_err()
                .code,
            ErrorCode::AlreadyExists
        );
        assert_eq!(
            Database::initialize(&benchmark).await.unwrap_err().code,
            ErrorCode::WrongPurpose
        );
        let orphan = dir.path().join("orphan.sqlite3");
        std::fs::write(dir.path().join("orphan.sqlite3-wal"), b"preserve").unwrap();
        assert_eq!(
            synthetic::generate(&orphan, config).await.unwrap_err().code,
            ErrorCode::AlreadyExists
        );
        assert!(!orphan.exists());
        assert!(
            synthetic::generate(
                &dir.path().join("unsupported.sqlite3"),
                DatasetConfig { games: 1, seed: 0 }
            )
            .await
            .is_err()
        );
        assert!(!dir.path().join("unsupported.sqlite3").exists());
        let empty = dir.path().join("empty.sqlite3");
        std::fs::write(&empty, []).unwrap();
        assert_eq!(
            synthetic::open_benchmark(&empty).await.unwrap_err().code,
            ErrorCode::InvalidDatabase
        );
        assert_eq!(std::fs::metadata(&empty).unwrap().len(), 0);
    }
}

#[tokio::test]
async fn details_window_is_bounded_narrow_stable_and_empty_is_success() {
    let dir = tempdir().unwrap();
    let db = Database::initialize(&dir.path().join("window.sqlite3"))
        .await
        .unwrap();
    let empty = db.window(&page(100, 0)).await.unwrap();
    assert!(empty.items.is_empty());
    for request in [page(0, 0), page(201, 0), page(100, 1_000_001)] {
        assert_eq!(
            db.window(&request).await.unwrap_err().code,
            ErrorCode::InvalidRequest
        );
    }
    db.close().await;
}

#[cfg(feature = "benchmarks")]
#[tokio::test]
async fn benchmark_view_is_read_only_and_refuses_missing_application_and_mismatched_schema() {
    use storage::synthetic::{DatasetConfig, generate};
    let dir = tempdir().unwrap();
    let missing = dir.path().join("missing.sqlite3");
    assert!(Database::benchmark_view(&missing).await.is_err());
    assert!(!missing.exists());
    let normal = dir.path().join("normal.sqlite3");
    Database::initialize(&normal).await.unwrap().close().await;
    let before = std::fs::read(&normal).unwrap();
    assert_eq!(
        Database::benchmark_view(&normal).await.unwrap_err().code,
        ErrorCode::WrongPurpose
    );
    assert_eq!(std::fs::read(&normal).unwrap(), before);
    let path = dir.path().join("synthetic.sqlite3");
    generate(
        &path,
        DatasetConfig {
            games: 500,
            seed: 42,
        },
    )
    .await
    .unwrap();
    let before = std::fs::read(&path).unwrap();
    let db = Database::benchmark_view(&path).await.unwrap();
    let first = db.window(&page(200, 0)).await.unwrap();
    let second = db.window(&page(200, 200)).await.unwrap();
    let last = db.window(&page(200, 400)).await.unwrap();
    assert_eq!(
        (first.items.len(), second.items.len(), last.items.len()),
        (200, 200, 100)
    );
    let ids: std::collections::HashSet<_> = first
        .items
        .iter()
        .chain(&second.items)
        .chain(&last.items)
        .map(|r| &r.id)
        .collect();
    assert_eq!(ids.len(), 500);
    let wire = serde_json::to_value(&first).unwrap();
    assert_eq!(wire["items"][0].as_object().unwrap().len(), 7);
    assert!(first.query_ms >= 0.0);
    db.close().await;
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let mut c = raw(&path).await;
    sqlx::query("UPDATE _sqlx_migrations SET checksum=X'00'")
        .execute(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    let before = std::fs::read(&path).unwrap();
    assert_eq!(
        Database::benchmark_view(&path).await.unwrap_err().code,
        ErrorCode::Migration
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
