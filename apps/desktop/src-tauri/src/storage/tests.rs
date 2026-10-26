// SPDX-License-Identifier: GPL-3.0-or-later

use super::*;
use tempfile::tempdir;

#[tokio::test]
async fn every_simultaneously_acquired_pool_connection_enforces_foreign_keys() {
    let dir = tempdir().unwrap();
    let db = Database::initialize(&dir.path().join("pool.sqlite3"))
        .await
        .unwrap();
    let mut connections = Vec::new();
    for _ in 0..POOL_SIZE {
        let mut c = db.pool.acquire().await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("PRAGMA foreign_keys")
                .fetch_one(&mut *c)
                .await
                .unwrap(),
            1
        );
        assert!(sqlx::query("INSERT INTO library_entries VALUES ('invalid', 'absent', 'absent', NULL, 'playing', 0)").execute(&mut *c).await.is_err());
        connections.push(c);
    }
    assert_eq!(db.pool.size(), POOL_SIZE);
    drop(connections);
    db.close().await;
}

#[tokio::test]
async fn generation_failure_keeps_committed_batches_and_rolls_back_failing_batch() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("failure.sqlite3");
    let db = Database::open(&path, "benchmark", true).await.unwrap();
    sqlx::raw_sql("CREATE TRIGGER inject_failure BEFORE INSERT ON games WHEN (SELECT count(*) FROM games) = 260 BEGIN SELECT RAISE(ABORT, 'injected failure'); END;").execute(&db.pool).await.unwrap();
    assert!(
        synthetic::populate(
            &db,
            synthetic::DatasetConfig {
                games: 500,
                seed: 42
            }
        )
        .await
        .is_err()
    );
    assert_eq!(db.counts().await.unwrap().games, 250);
    assert!(
        !sqlx::query_scalar::<_, bool>("SELECT generation_complete FROM database_metadata")
            .fetch_one(&db.pool)
            .await
            .unwrap()
    );
    db.validate().await.unwrap();
    db.close().await;
    assert!(path.exists());
    let db = synthetic::open_benchmark(&path).await.unwrap();
    assert_eq!(
        synthetic::dataset_config(&db).await.unwrap_err().code,
        ErrorCode::InvalidDatabase
    );
    assert!(
        synthetic::populate(
            &db,
            synthetic::DatasetConfig {
                games: 500,
                seed: 42
            }
        )
        .await
        .is_err()
    );
    db.close().await;
}

#[cfg(windows)]
#[tokio::test]
async fn exclusively_locked_database_reports_failure_and_preserves_bytes() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = tempdir().unwrap();
    let path = dir.path().join("locked.sqlite3");
    let db = Database::initialize(&path).await.unwrap();
    db.close().await;
    let before = std::fs::read(&path).unwrap();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(&path)
        .unwrap();
    assert!(Database::initialize(&path).await.is_err());
    drop(lock);
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
