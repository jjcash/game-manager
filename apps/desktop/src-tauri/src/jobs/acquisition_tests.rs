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
    wait_observed(s, id, predicate, None).await
}

async fn wait_observed(
    s: &Scheduler,
    id: u64,
    predicate: impl Fn(&Snapshot) -> bool,
    mut diagnostics: Option<&mut AcquisitionDiagnostics<'_>>,
) -> Snapshot {
    if let Some(diagnostics) = diagnostics.as_deref_mut() {
        diagnostics.wait_started = Some(Instant::now());
    }
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let report = s.report();
            let snapshot = report.jobs.into_iter().find(|j| j.id == id).unwrap();
            if let Some(diagnostics) = diagnostics.as_deref_mut() {
                let now = Instant::now();
                if let Some(previous) = diagnostics.last_poll.replace(now) {
                    diagnostics.max_poll_gap = diagnostics.max_poll_gap.max(now - previous);
                }
                diagnostics.observed_active = Some(report.active);
                diagnostics.scheduling = Some(report.scheduling);
            }
            if predicate(&snapshot) {
                if let Some(diagnostics) = diagnostics.as_deref_mut() {
                    // One terminal copy; nonterminal observations move the existing
                    // report snapshot, with no additional per-poll allocation.
                    diagnostics.last_snapshot = Some(snapshot.clone());
                }
                return snapshot;
            }
            if let Some(diagnostics) = diagnostics.as_deref_mut() {
                diagnostics.last_snapshot = Some(snapshot);
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await;
    if let Some(diagnostics) = diagnostics {
        diagnostics.last_wait = diagnostics.wait_started.take().unwrap().elapsed();
        diagnostics.max_wait = diagnostics.max_wait.max(diagnostics.last_wait);
        if let Some(previous) = diagnostics.last_poll {
            // Include a delayed timeout delivery after the final completed poll.
            diagnostics.max_poll_gap = diagnostics.max_poll_gap.max(previous.elapsed());
        }
    }
    result.expect("bounded acquisition control stalled")
}

// Test-local, constant-count observations. Formatting and additional pool samples
// occur only on failure; the polling loop reuses its existing scheduler report.
struct AcquisitionDiagnostics<'a> {
    scheduler: &'a Scheduler,
    db: &'a Database,
    iteration: usize,
    id: Option<u64>,
    phase: &'static str,
    phase_started: Instant,
    wait_started: Option<Instant>,
    last_wait: Duration,
    max_wait: Duration,
    last_poll: Option<Instant>,
    max_poll_gap: Duration,
    last_snapshot: Option<Snapshot>,
    observed_active: Option<usize>,
    scheduling: Option<crate::scheduling::Stats>,
}

impl<'a> AcquisitionDiagnostics<'a> {
    fn new(scheduler: &'a Scheduler, db: &'a Database) -> Self {
        Self {
            scheduler,
            db,
            iteration: 0,
            id: None,
            phase: "readers_start",
            phase_started: Instant::now(),
            wait_started: None,
            last_wait: Duration::ZERO,
            max_wait: Duration::ZERO,
            last_poll: None,
            max_poll_gap: Duration::ZERO,
            last_snapshot: None,
            observed_active: None,
            scheduling: None,
        }
    }

    fn phase(&mut self, phase: &'static str) {
        self.phase = phase;
        self.phase_started = Instant::now();
    }
}

impl Drop for AcquisitionDiagnostics<'_> {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            return;
        }
        let size = self.db.pool.size();
        let idle = self.db.pool.num_idle();
        let wait = self.wait_started.map_or(self.last_wait, |at| at.elapsed());
        let poll_age = self.last_poll.map(|at| at.elapsed());
        let gap = if self.phase == "terminal_wait" {
            self.max_poll_gap.max(poll_age.unwrap_or_default())
        } else {
            self.max_poll_gap
        };
        // Pool samples are non-atomic. Non-idle includes opening/returning work,
        // not necessarily acquired connections or executing queries. Poll gaps
        // include report work, the existing sleep and task/timer delivery.
        eprintln!(
            "M0.8 acquisition failure iteration={}/16 job={:?} phase={} phase_ms={:.3} terminal_wait_ms={:.3} last_snapshot(state,processed,committed,error)={:?} last_poll_age_ms={:?} max_poll_gap_ms={:.3} observed_active={:?} active_now={} slot_available_now={} observed_workers_active={:?} observed_workers_max={:?} observed_foreground_active={:?} observed_backoffs={:?} pool_size={} pool_idle={} pool_non_idle_estimate={} pool_limit={}",
            self.iteration,
            self.id,
            self.phase,
            self.phase_started.elapsed().as_secs_f64() * 1000.0,
            wait.as_secs_f64() * 1000.0,
            self.last_snapshot
                .as_ref()
                .map(|j| (j.state, j.processed, j.committed, &j.error)),
            poll_age.map(|age| age.as_secs_f64() * 1000.0),
            gap.as_secs_f64() * 1000.0,
            self.observed_active,
            self.scheduler.active.load(Ordering::SeqCst),
            self.scheduler.slots.available_permits(),
            self.scheduling.as_ref().map(|s| s.workers_active),
            self.scheduling.as_ref().map(|s| s.workers_max),
            self.scheduling.as_ref().map(|s| s.foreground_active),
            self.scheduling.as_ref().map(|s| s.backoffs),
            size,
            idle,
            (size as usize).saturating_sub(idle),
            self.db.pool.options().get_max_connections(),
        );
    }
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
    let mut diagnostics = AcquisitionDiagnostics::new(&s, &db);
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
    for iteration in 0..16 {
        diagnostics.iteration = iteration + 1;
        diagnostics.id = None;
        diagnostics.last_snapshot = None;
        diagnostics.observed_active = None;
        diagnostics.scheduling = None;
        diagnostics.last_poll = None;
        diagnostics.last_wait = Duration::ZERO;
        diagnostics.phase("start");
        let id = s.start(request()).await.unwrap().id;
        diagnostics.id = Some(id);
        diagnostics.phase("terminal_wait");
        let job = wait_observed(&s, id, |j| j.state.terminal(), Some(&mut diagnostics)).await;
        diagnostics.phase("terminal_assertions");
        assert_eq!(job.state, State::Completed, "{:?}", job.error);
        assert_eq!((job.processed, job.committed), (128, 128));
        waits.extend(job.pool_wait_ms);
        diagnostics.phase("audit");
        audit(&db, id, 128).await;
    }
    stop.store(true, Ordering::SeqCst);
    diagnostics.phase("readers_join");
    let mut read_count = 0;
    for reader in readers {
        read_count += reader.await.unwrap();
    }
    diagnostics.phase("shutdown");
    s.shutdown().await.unwrap();
    diagnostics.phase("resource_assertions");
    assert_eq!(s.report().active, 0);
    assert_eq!(s.budget.stats().workers_active, 0);
    println!(
        "M0.8 natural reads={read_count} writes=2048 acquisitions={} max_ms={:.3} pool_size={} idle={} max_terminal_wait_ms={:.3} max_poll_gap_ms={:.3}",
        waits.len(),
        waits.iter().copied().fold(0.0, f64::max),
        db.pool.size(),
        db.pool.num_idle(),
        diagnostics.max_wait.as_secs_f64() * 1000.0,
        diagnostics.max_poll_gap.as_secs_f64() * 1000.0,
    );
    // Drop returns may still be queued (num_idle is only an instantaneous
    // sample). Reacquire every slot to test recovery, then close with a bound.
    let mut recovered = Vec::new();
    diagnostics.phase("pool_reacquire");
    for _ in 0..storage::POOL_SIZE {
        recovered.push(
            tokio::time::timeout(Duration::from_secs(5), db.pool.acquire())
                .await
                .unwrap()
                .unwrap(),
        );
    }
    drop(recovered);
    diagnostics.phase("pool_close");
    tokio::time::timeout(Duration::from_secs(5), db.pool.close())
        .await
        .unwrap();
}
