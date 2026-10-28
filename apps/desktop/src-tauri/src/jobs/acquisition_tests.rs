// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded M0.8 controls. These force occupancy, not the historical failure's cause.
use super::*;
use crate::storage::{query::Criteria, synthetic::DatasetConfig};
use std::time::Duration;

async fn fixture() -> (tempfile::TempDir, Arc<Scheduler>, Database) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("benchmarks/data/jobs-acquisition");
    fixtures::generate(
        &root,
        DatasetConfig {
            games: 500,
            seed: 42,
        },
    )
    .await
    .unwrap();
    let (scheduler, db) = Scheduler::open(&root, Budget::new(false)).await.unwrap();
    assert_eq!(
        db.pool.options().get_acquire_timeout(),
        Duration::from_secs(10)
    );
    (temp, scheduler, db)
}

fn request() -> Request {
    Request {
        kind: Kind::Write,
        records: 128,
        seed: 20261026,
        intensity: Intensity::Saturated,
        fault: Fault::None,
    }
}

async fn wait(s: &Scheduler, id: u64, predicate: impl Fn(&Snapshot) -> bool) -> Snapshot {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let snapshot = s.report().jobs.into_iter().find(|j| j.id == id).unwrap();
            if predicate(&snapshot) {
                return snapshot;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .expect("bounded acquisition control stalled")
}

async fn audit(db: &Database, id: u64, committed: u32) {
    let (ledger, items): (i64, i64) = sqlx::query_as(
        "SELECT committed,(SELECT count(*) FROM simulation_items WHERE job_id=?) FROM simulation_runs WHERE id=?",
    )
    .bind(id as i64)
    .bind(id as i64)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        (ledger, items),
        (i64::from(committed), i64::from(committed))
    );
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(integrity, "ok");
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&db.pool)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn occupied_pool_guard_failure_and_cancellation_recover_without_capacity_change() {
    let (_temp, s, db) = fixture().await;
    for cancel in [false, true, false, true] {
        // Prevent processing until the running ledger checkpoint is durable.
        let token = AtomicBool::new(false);
        let a = s.budget.acquire(&token).await.unwrap();
        let b = s.budget.acquire(&token).await.unwrap();
        let id = s.start(request()).await.unwrap().id;
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let state: String =
                    sqlx::query_scalar("SELECT state FROM simulation_runs WHERE id=?")
                        .bind(id as i64)
                        .fetch_one(&db.pool)
                        .await
                        .unwrap();
                if state == "running" {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let mut held = Vec::new();
        for _ in 0..storage::POOL_SIZE {
            let mut connection = db.pool.acquire().await.unwrap();
            // A real read on each retained connection; holding is deliberate.
            let _: i64 = sqlx::query_scalar("SELECT count(*) FROM games")
                .fetch_one(&mut *connection)
                .await
                .unwrap();
            held.push(connection);
        }
        assert_eq!(db.pool.size(), 4);
        assert_eq!(db.pool.num_idle(), 0);
        drop((a, b));
        wait(&s, id, |j| j.processed == 64).await;
        let started = Instant::now();
        let mut ticks = Vec::new();
        if cancel {
            // Leave acquisition pending before requesting cancellation; release
            // the readers well before the unchanged 250-ms failure guard.
            tokio::time::sleep(Duration::from_millis(20)).await;
            s.cancel(id).unwrap();
        } else {
            // Bounded heartbeat distinguishes observed executor delay from occupancy.
            while s.report().jobs.iter().find(|j| j.id == id).unwrap().state != State::Finalizing {
                assert!(
                    ticks.len() < 500,
                    "terminal checkpoint did not become pending"
                );
                let at = Instant::now();
                tokio::time::sleep(Duration::from_millis(2)).await;
                ticks.push(at.elapsed().as_secs_f64() * 1000.0);
            }
        }
        println!(
            "M0.8 forced occupancy cancel={cancel} held={} size={} idle={} observed_ms={:.3} heartbeat_n={} heartbeat_max_ms={:.3}",
            held.len(),
            db.pool.size(),
            db.pool.num_idle(),
            started.elapsed().as_secs_f64() * 1000.0,
            ticks.len(),
            ticks.iter().copied().fold(0.0, f64::max)
        );
        drop(held);
        let terminal = wait(&s, id, |j| j.state.terminal()).await;
        assert_eq!(terminal.processed, 64);
        assert_eq!(terminal.committed, 0);
        if cancel {
            assert_eq!(terminal.state, State::Cancelled);
            assert!(terminal.error.is_none());
        } else {
            assert_eq!(terminal.state, State::Failed);
            assert_eq!(terminal.error.as_deref(), Some("POOL_EXHAUSTED"));
            // Failed acquisition samples are absent from the existing success-only ring.
            assert!(terminal.pool_wait_ms.is_empty());
        }
        audit(&db, id, 0).await;
        let recovered = s.start(request()).await.unwrap();
        let recovered = wait(&s, recovered.id, |j| j.state.terminal()).await;
        assert_eq!(recovered.state, State::Completed);
        assert_eq!(recovered.committed, 128);
        audit(&db, recovered.id, 128).await;
    }
    s.shutdown().await.unwrap();
    assert_eq!(s.report().active, 0);
    assert_eq!(s.budget.stats().workers_active, 0);
    let root = s.root.clone();
    tokio::time::timeout(Duration::from_secs(5), db.close())
        .await
        .unwrap();
    drop(s);
    let checked = fixtures::audit(&root).await.unwrap();
    println!(
        "M0.8 closed audit {}",
        serde_json::to_string(&checked).unwrap()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_reads_and_repeated_writes_release_pool_and_scheduler_resources() {
    let (_temp, s, db) = fixture().await;
    let db = Arc::new(db);
    let stop = Arc::new(AtomicBool::new(false));
    let mut readers = Vec::new();
    for _ in 0..4 {
        let db = Arc::clone(&db);
        let stop = Arc::clone(&stop);
        readers.push(tokio::spawn(async move {
            let mut n = 0;
            while !stop.load(Ordering::SeqCst) && n < 4096 {
                assert_eq!(db.query_count(&Criteria::default()).await.unwrap(), 500);
                n += 1;
                tokio::task::yield_now().await;
            }
            n
        }));
    }
    let mut waits = Vec::new();
    for _ in 0..16 {
        let id = s.start(request()).await.unwrap().id;
        let job = wait(&s, id, |j| j.state.terminal()).await;
        assert_eq!(job.state, State::Completed, "{:?}", job.error);
        assert_eq!((job.processed, job.committed), (128, 128));
        waits.extend(job.pool_wait_ms);
        audit(&db, id, 128).await;
    }
    stop.store(true, Ordering::SeqCst);
    let mut read_count = 0;
    for reader in readers {
        read_count += reader.await.unwrap();
    }
    s.shutdown().await.unwrap();
    assert_eq!(s.report().active, 0);
    assert_eq!(s.budget.stats().workers_active, 0);
    println!(
        "M0.8 natural reads={read_count} writes=2048 acquisitions={} max_ms={:.3} pool_size={} idle={}",
        waits.len(),
        waits.iter().copied().fold(0.0, f64::max),
        db.pool.size(),
        db.pool.num_idle()
    );
    // Drop returns may still be queued (num_idle is only an instantaneous
    // sample). Reacquire every slot to test recovery, then close with a bound.
    let mut recovered = Vec::new();
    for _ in 0..storage::POOL_SIZE {
        recovered.push(
            tokio::time::timeout(Duration::from_secs(5), db.pool.acquire())
                .await
                .unwrap()
                .unwrap(),
        );
    }
    drop(recovered);
    tokio::time::timeout(Duration::from_secs(5), db.pool.close())
        .await
        .unwrap();
}
