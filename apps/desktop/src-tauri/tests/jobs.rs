// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(feature = "benchmarks")]
use game_manager_lib::{
    jobs::{self, Fault, Intensity, Kind, Request, Scheduler, State},
    scheduling::Budget,
    storage::{self, synthetic::DatasetConfig},
};
use sqlx::{Connection, sqlite::SqliteConnectOptions};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
}
impl Fixture {
    async fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("benchmarks/data/jobs-test");
        jobs::fixtures::generate(
            &root,
            DatasetConfig {
                games: 500,
                seed: 42,
            },
        )
        .await
        .unwrap();
        Self { _temp: temp, root }
    }
    async fn connection(&self) -> sqlx::SqliteConnection {
        sqlx::SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(self.root.join("library.sqlite3"))
                .foreign_keys(true),
        )
        .await
        .unwrap()
    }
    async fn integrity(&self) {
        let mut c = self.connection().await;
        let result: String = sqlx::query_scalar("PRAGMA integrity_check")
            .fetch_one(&mut c)
            .await
            .unwrap();
        assert_eq!(result, "ok");
        assert!(
            sqlx::query("PRAGMA foreign_key_check")
                .fetch_all(&mut c)
                .await
                .unwrap()
                .is_empty()
        );
    }
}
fn request(kind: Kind, records: u32) -> Request {
    Request {
        kind,
        records,
        seed: 123,
        intensity: Intensity::Gentle,
        fault: Fault::None,
    }
}
async fn until(check: impl Fn() -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while !check() {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .expect("state transition deadline");
}
async fn done(s: &Scheduler, id: u64) -> jobs::Snapshot {
    until(|| {
        s.report().jobs.iter().any(|j| {
            j.id == id && matches!(j.state, State::Completed | State::Cancelled | State::Failed)
        })
    })
    .await;
    s.report().jobs.into_iter().find(|j| j.id == id).unwrap()
}

#[tokio::test]
async fn fixture_hash_is_deterministic_for_maximum_seed() {
    let temp = tempfile::tempdir().unwrap();
    let roots = ["jobs-max-a", "jobs-max-b", "jobs-other"]
        .map(|name| temp.path().join("benchmarks/data").join(name));
    for (index, root) in roots.iter().enumerate() {
        jobs::fixtures::generate(
            root,
            DatasetConfig {
                games: 500,
                seed: if index < 2 { u64::MAX } else { 42 },
            },
        )
        .await
        .unwrap();
    }
    let bytes = std::fs::read(roots[0].join("hash.bin")).unwrap();
    assert_eq!(bytes.len(), 256 * 1024);
    assert_eq!(bytes, std::fs::read(roots[1].join("hash.bin")).unwrap());
    assert_ne!(bytes, std::fs::read(roots[2].join("hash.bin")).unwrap());
    assert_eq!(bytes[0], (u64::MAX % 251) as u8);
    assert_eq!(bytes[1], (7919u64.wrapping_add(u64::MAX) % 251) as u8);
}

#[tokio::test]
async fn deterministic_discovery_hashing_and_atomic_short_writes() {
    let f = Fixture::new().await;
    let (s, db) = Scheduler::open(&f.root, Budget::new(false)).await.unwrap();
    for kind in [Kind::Cpu, Kind::Hash, Kind::Write, Kind::Mixed] {
        let first = s.start(request(kind, 128)).await.unwrap();
        let a = done(&s, first.id).await;
        let second = s.start(request(kind, 128)).await.unwrap();
        let b = done(&s, second.id).await;
        assert_eq!(a.state, State::Completed);
        assert_eq!(a.processed, 128);
        assert_eq!(a.digest, b.digest);
        assert_eq!(
            a.committed,
            if matches!(kind, Kind::Write | Kind::Mixed) {
                128
            } else {
                0
            }
        );
        assert!(a.transaction_ms.len() <= 64);
        assert!(a.pool_wait_ms.len() <= 64);
    }
    assert_eq!(db.counts().await.unwrap().games, 500);
    assert_eq!(db.status().await.unwrap().pool_limit, 4);
    f.integrity().await;
    s.shutdown().await.unwrap();
}

#[tokio::test]
async fn queue_limits_cancel_before_start_during_hash_and_cleanup() {
    let f = Fixture::new().await;
    let budget = Budget::new(false);
    let (s, _) = Scheduler::open(&f.root, Arc::clone(&budget)).await.unwrap();
    let slow = Request {
        intensity: Intensity::Slow,
        ..request(Kind::Hash, 100000)
    };
    let a = s.start(slow).await.unwrap();
    until(|| s.report().active == 1 && budget.stats().workers_active == 1).await;
    let b = s.start(request(Kind::Cpu, 1000)).await.unwrap();
    let c = s.start(request(Kind::Cpu, 1000)).await.unwrap();
    assert!(s.start(request(Kind::Cpu, 1)).await.is_err());
    assert_eq!(s.cancel(b.id).unwrap().state, State::CancelRequested);
    let b = done(&s, b.id).await;
    assert_eq!(b.state, State::Cancelled);
    assert_eq!(b.processed, 0);
    assert_eq!(b.committed, 0);
    s.cancel(c.id).unwrap();
    s.cancel(a.id).unwrap();
    let a = done(&s, a.id).await;
    assert_eq!(a.state, State::Cancelled);
    assert!(a.processed < 100000);
    assert!(a.cancel_latency_ms.is_some());
    s.shutdown().await.unwrap();
    assert_eq!(s.report().active, 0);
    assert_eq!(budget.stats().workers_active, 0);
    assert_eq!(s.report().max_active, 1);
    f.integrity().await;
}

#[tokio::test]
async fn simultaneous_admission_never_exceeds_waiting_or_active_limits() {
    let f = Fixture::new().await;
    let (s, _) = Scheduler::open(&f.root, Budget::new(false)).await.unwrap();
    let mut starts = Vec::new();
    for _ in 0..16 {
        let scheduler = Arc::clone(&s);
        starts.push(tokio::spawn(async move {
            scheduler
                .start(Request {
                    intensity: Intensity::Slow,
                    ..request(Kind::Hash, 100000)
                })
                .await
        }));
    }
    let mut accepted = Vec::new();
    for start in starts {
        if let Ok(job) = start.await.unwrap() {
            accepted.push(job.id);
        }
        let report = s.report();
        assert!(report.queued <= 2);
        assert!(report.active <= 1);
    }
    assert!((1..=3).contains(&accepted.len()));
    for id in accepted {
        s.cancel(id).unwrap();
        assert_eq!(done(&s, id).await.state, State::Cancelled);
    }
    s.shutdown().await.unwrap();
    f.integrity().await;
}

#[tokio::test]
async fn monotonic_progress_cancellation_between_batches_and_restart() {
    let f = Fixture::new().await;
    let (s, _) = Scheduler::open(&f.root, Budget::new(false)).await.unwrap();
    let a = s.start(request(Kind::Write, 10000)).await.unwrap();
    let mut processed = 0;
    let mut committed = 0;
    loop {
        let j = s.report().jobs.into_iter().find(|j| j.id == a.id).unwrap();
        assert!(j.processed >= processed);
        assert!(j.committed >= committed);
        assert!(j.committed <= j.processed);
        processed = j.processed;
        committed = j.committed;
        if committed >= 64 {
            s.cancel(a.id).unwrap();
            break;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    let cancelled = done(&s, a.id).await;
    assert_eq!(cancelled.state, State::Cancelled);
    assert!(cancelled.committed < 10000);
    let mut c = f.connection().await;
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM simulation_items WHERE job_id=?")
        .bind(a.id as i64)
        .fetch_one(&mut c)
        .await
        .unwrap();
    assert_eq!(rows, i64::from(cancelled.committed));
    let fresh = s.start(request(Kind::Write, 128)).await.unwrap();
    assert_eq!(done(&s, fresh.id).await.state, State::Completed);
    s.shutdown().await.unwrap();
    f.integrity().await;
}

#[tokio::test]
async fn failed_transaction_preserves_committed_prefix_and_library_navigation() {
    let f = Fixture::new().await;
    let (s, db) = Scheduler::open(&f.root, Budget::new(false)).await.unwrap();
    let before = db
        .window(&storage::PageRequest {
            limit: 100,
            offset: 0,
            sort: storage::SummarySort::Title,
        })
        .await
        .unwrap();
    let a = s
        .start(Request {
            fault: Fault::Write,
            ..request(Kind::Write, 128)
        })
        .await
        .unwrap();
    let j = done(&s, a.id).await;
    assert_eq!(j.state, State::Failed);
    assert_eq!(j.processed, 128);
    assert_eq!(j.committed, 64);
    assert!(j.error.unwrap().contains("ROLLED_BACK"));
    let after = db
        .window(&storage::PageRequest {
            limit: 100,
            offset: 0,
            sort: storage::SummarySort::Title,
        })
        .await
        .unwrap();
    assert_eq!(before.items, after.items);
    let mut c = f.connection().await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM simulation_items WHERE job_id=?")
        .bind(a.id as i64)
        .fetch_one(&mut c)
        .await
        .unwrap();
    assert_eq!(count, 64);
    f.integrity().await;
    s.shutdown().await.unwrap();
}

#[tokio::test]
async fn invalid_input_resource_failure_and_repeated_start_cancel_are_bounded() {
    let f = Fixture::new().await;
    let (s, _) = Scheduler::open(&f.root, Budget::new(false)).await.unwrap();
    assert!(s.start(request(Kind::Cpu, 0)).await.is_err());
    assert!(s.start(request(Kind::Write, 100001)).await.is_err());
    assert!(s.cancel(u64::MAX).is_err());
    for input in [
        r#"{"kind":"shell","records":1,"seed":1,"intensity":"gentle"}"#,
        r#"{"kind":"cpu","records":1,"seed":1,"intensity":"gentle","path":"C:/personal"}"#,
        r#"{"kind":"write","records":1,"seed":1,"intensity":"gentle","sql":"DELETE FROM games"}"#,
    ] {
        assert!(serde_json::from_str::<Request>(input).is_err());
    }
    for _ in 0..12 {
        let a = s
            .start(Request {
                fault: Fault::Resource,
                ..request(Kind::Cpu, 64)
            })
            .await
            .unwrap();
        assert_eq!(done(&s, a.id).await.state, State::Failed);
    }
    assert_eq!(s.report().jobs.len(), 8);
    let mut c = f.connection().await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM simulation_runs")
        .fetch_one(&mut c)
        .await
        .unwrap();
    assert_eq!(count, 8);
    for _ in 0..5 {
        let a = s.start(request(Kind::Hash, 100000)).await.unwrap();
        s.cancel(a.id).unwrap();
        assert_eq!(done(&s, a.id).await.state, State::Cancelled);
    }
    s.shutdown().await.unwrap();
    assert!(s.start(request(Kind::Cpu, 1)).await.is_err());
    f.integrity().await;
}

#[tokio::test]
async fn restart_reconciles_interrupted_prefix_and_rolls_back_incomplete_transaction() {
    let f = Fixture::new().await;
    let mut c = f.connection().await;
    let req = serde_json::to_string(&request(Kind::Write, 128)).unwrap();
    sqlx::query(
        "INSERT INTO simulation_runs VALUES(1,'running',?,1,1,'123',128,'0000000000000001',1.0)",
    )
    .bind(req)
    .execute(&mut c)
    .await
    .unwrap();
    sqlx::query("INSERT INTO simulation_items VALUES(1,0,'syn-000000000000002a-g-00000','one')")
        .execute(&mut c)
        .await
        .unwrap();
    {
        let mut tx = c.begin().await.unwrap();
        sqlx::query(
            "INSERT INTO simulation_items VALUES(1,1,'syn-000000000000002a-g-00001','two')",
        )
        .execute(&mut *tx)
        .await
        .unwrap();
        sqlx::query("UPDATE simulation_runs SET committed=2,processed=2 WHERE id=1")
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    c.close().await.unwrap();
    let (s, _) = Scheduler::open(&f.root, Budget::new(false)).await.unwrap();
    let j = &s.report().jobs[0];
    assert_eq!(j.state, State::Interrupted);
    assert_eq!(j.committed, 1);
    assert_eq!(s.report().active, 0);
    let a = s.start(request(Kind::Write, 128)).await.unwrap();
    assert_eq!(a.id, 2);
    assert_eq!(done(&s, a.id).await.state, State::Completed);
    s.shutdown().await.unwrap();
    f.integrity().await;
}

#[tokio::test]
async fn lease_cleanup_and_nonbenchmark_paths_are_protected() {
    let f = Fixture::new().await;
    let (s, db) = Scheduler::open(&f.root, Budget::new(false)).await.unwrap();
    assert!(Scheduler::open(&f.root, Budget::new(false)).await.is_err());
    assert!(jobs::fixtures::clean(&f.root).await.is_err());
    assert!(jobs::fixtures::validate_root(Path::new("C:/games")).is_err());
    s.shutdown().await.unwrap();
    db.close().await;
    drop(s);
    let audit = jobs::fixtures::audit(&f.root).await.unwrap();
    assert_eq!(audit.integrity, "ok");
    assert_eq!(audit.games, 500);
    assert_eq!(audit.items, audit.committed);
    jobs::fixtures::clean(&f.root).await.unwrap();
    assert!(!f.root.exists());
}

#[tokio::test]
async fn busy_writer_retries_do_not_prevent_wal_readers() {
    let f = Fixture::new().await;
    let budget = Budget::new(true);
    let (s, db) = Scheduler::open(&f.root, Arc::clone(&budget)).await.unwrap();
    let foreground = budget.enter();
    let a = s.start(request(Kind::Write, 128)).await.unwrap();
    until(|| budget.stats().backoffs > 0).await;
    let mut lock = f.connection().await;
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut lock)
        .await
        .unwrap();
    drop(foreground);
    assert_eq!(db.counts().await.unwrap().games, 500);
    let j = done(&s, a.id).await;
    assert_eq!(j.state, State::Failed);
    assert_eq!(j.committed, 0);
    assert!(j.busy_retries > 0);
    sqlx::query("ROLLBACK").execute(&mut lock).await.unwrap();
    s.shutdown().await.unwrap();
    f.integrity().await;
}

#[tokio::test]
async fn shared_resource_budget_and_cancellable_backoff_preserve_foreground_access() {
    let budget = Budget::new(false);
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let a = budget.acquire(&cancel).await.unwrap();
    let b = budget.acquire(&cancel).await.unwrap();
    let other = Arc::clone(&budget);
    let token = Arc::clone(&cancel);
    let waiter = tokio::spawn(async move { other.acquire(&token).await });
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(!waiter.is_finished());
    cancel.store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(waiter.await.unwrap().is_none());
    let entered = Instant::now();
    let foreground = budget.enter();
    assert!(entered.elapsed() < Duration::from_secs(1));
    drop(foreground);
    drop(a);
    drop(b);
    assert_eq!(budget.stats().workers_max, 2);
    assert_eq!(budget.stats().workers_active, 0);
}

#[tokio::test]
async fn parsing_cancellation_and_invalid_metadata_failure_are_distinct() {
    let f = Fixture::new().await;
    let budget = Budget::new(false);
    let (s, _) = Scheduler::open(&f.root, Arc::clone(&budget)).await.unwrap();
    let a = s
        .start(Request {
            intensity: Intensity::Saturated,
            ..request(Kind::Cpu, 100000)
        })
        .await
        .unwrap();
    until(|| budget.stats().workers_active == 1).await;
    s.cancel(a.id).unwrap();
    let cancelled = done(&s, a.id).await;
    assert_eq!(cancelled.state, State::Cancelled);
    assert!(cancelled.processed < 100000);
    let bad = s
        .start(Request {
            fault: Fault::Parse,
            ..request(Kind::Cpu, 128)
        })
        .await
        .unwrap();
    let failed = done(&s, bad.id).await;
    assert_eq!(failed.state, State::Failed);
    assert_eq!(failed.processed, 64);
    assert!(failed.error.unwrap().contains("METADATA_PARSE"));
    s.shutdown().await.unwrap();
    f.integrity().await;
}
