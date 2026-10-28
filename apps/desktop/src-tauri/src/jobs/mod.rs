// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic, disposable job prototype. Not a production importer or durable queue.
#[cfg(test)]
mod acquisition_tests;
pub mod fixtures;
use crate::{
    scheduling::Budget,
    storage::{self, Database},
};
use serde::{Deserialize, Serialize};
use sqlx::{Acquire, Row};
use std::{
    collections::VecDeque,
    fs,
    io::Read,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::Instant,
};
use tokio::sync::{Notify, Semaphore};

pub const BATCH: u32 = 64;
pub const HISTORY: usize = 8;
pub const MAX_RECORDS: u32 = 100_000;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum State {
    Queued,
    Running,
    CancelRequested,
    Finalizing,
    Cancelled,
    Completed,
    Failed,
    Interrupted,
}
impl State {
    fn terminal(self) -> bool {
        matches!(
            self,
            Self::Cancelled | Self::Completed | Self::Failed | Self::Interrupted
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Cpu,
    Hash,
    Write,
    Mixed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Intensity {
    Gentle,
    Saturated,
    Slow,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Fault {
    #[default]
    None,
    Parse,
    Write,
    Resource,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub kind: Kind,
    pub records: u32,
    pub seed: u32,
    pub intensity: Intensity,
    #[serde(default)]
    pub fault: Fault,
}
impl Request {
    pub fn validate(&self) -> Result<(), String> {
        if self.records == 0
            || self.records > MAX_RECORDS
            || (self.fault == Fault::Write && !matches!(self.kind, Kind::Write | Kind::Mixed))
        {
            return Err("Invalid synthetic workload bounds or fault".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub id: u64,
    pub state: State,
    pub request: Request,
    pub processed: u32,
    pub committed: u32,
    pub batches: u32,
    pub digest: String,
    pub elapsed_ms: f64,
    pub cancel_latency_ms: Option<f64>,
    pub error: Option<String>,
    pub busy_retries: u32,
    pub transaction_ms: Vec<f64>,
    pub pool_wait_ms: Vec<f64>,
    pub progress_revision: u64,
}
struct Entry {
    snapshot: Mutex<Snapshot>,
    cancel: Arc<AtomicBool>,
    notify: Notify,
    start: Instant,
    cancel_at: Mutex<Option<Instant>>,
}
pub struct Scheduler {
    db: Database,
    root: PathBuf,
    config: fixtures::Marker,
    budget: Arc<Budget>,
    entries: Mutex<VecDeque<Arc<Entry>>>,
    next: AtomicU64,
    slots: Arc<Semaphore>,
    writer: tokio::sync::Mutex<()>,
    stopped: AtomicBool,
    active: AtomicUsize,
    peak: AtomicUsize,
    tasks: Mutex<VecDeque<tokio::task::JoinHandle<()>>>,
    _lease: fs::File,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub jobs: Vec<Snapshot>,
    pub active: usize,
    pub queued: usize,
    pub max_active: usize,
    pub queue_limit: usize,
    pub cpu_limit: usize,
    pub scheduling: crate::scheduling::Stats,
}
fn error(e: impl ToString) -> storage::StorageError {
    storage::StorageError::new(storage::ErrorCode::InvalidDatabase, "synthetic_jobs", e)
}
impl Scheduler {
    pub async fn open(
        root: &std::path::Path,
        budget: Arc<Budget>,
    ) -> storage::Result<(Arc<Self>, Database)> {
        let root = fixtures::validate_root(root)?;
        let lease = fixtures::lease(&root)?;
        let (db, config, root) = fixtures::open(&root).await?;
        let mismatch: i64 = sqlx::query_scalar("SELECT count(*) FROM simulation_runs r WHERE committed != (SELECT count(*) FROM simulation_items i WHERE i.job_id=r.id)").fetch_one(&db.pool).await?;
        if mismatch != 0 {
            return Err(error("Synthetic checkpoint mismatch; preserved"));
        }
        let next: i64 = sqlx::query_scalar("SELECT COALESCE(max(id),0)+1 FROM simulation_runs")
            .fetch_one(&db.pool)
            .await?;
        let mut entries = VecDeque::new();
        for row in sqlx::query("SELECT * FROM simulation_runs ORDER BY id DESC LIMIT 8")
            .fetch_all(&db.pool)
            .await?
            .into_iter()
            .rev()
        {
            let request: Request = serde_json::from_str(row.get("request")).map_err(error)?;
            request.validate().map_err(error)?;
            let state: State =
                serde_json::from_str(&format!("\"{}\"", row.get::<&str, _>("state")))
                    .map_err(error)?;
            let snapshot = Snapshot {
                id: row.get::<i64, _>("id") as u64,
                state,
                request,
                processed: row.get::<u32, _>("processed"),
                committed: row.get::<u32, _>("committed"),
                batches: 0,
                digest: row.get("digest"),
                elapsed_ms: row.get("elapsed_ms"),
                cancel_latency_ms: None,
                error: (state == State::Interrupted)
                    .then(|| "INTERRUPTED_AT_DURABLE_CHECKPOINT".into()),
                busy_retries: 0,
                transaction_ms: vec![],
                pool_wait_ms: vec![],
                progress_revision: 0,
            };
            entries.push_back(Arc::new(Entry {
                snapshot: Mutex::new(snapshot),
                cancel: Arc::new(AtomicBool::new(false)),
                notify: Notify::new(),
                start: Instant::now(),
                cancel_at: Mutex::new(None),
            }));
        }
        let view = Database {
            pool: db.pool.clone(),
        };
        Ok((
            Arc::new(Self {
                db,
                config,
                root,
                budget,
                entries: Mutex::new(entries),
                next: AtomicU64::new(next as u64),
                slots: Arc::new(Semaphore::new(1)),
                writer: tokio::sync::Mutex::new(()),
                stopped: AtomicBool::new(false),
                active: AtomicUsize::new(0),
                peak: AtomicUsize::new(0),
                tasks: Mutex::new(VecDeque::new()),
                _lease: lease,
            }),
            view,
        ))
    }
    pub fn report(&self) -> Report {
        let jobs: Vec<_> = self
            .entries
            .lock()
            .unwrap()
            .iter()
            .map(|entry| {
                let mut s = entry.snapshot.lock().unwrap().clone();
                if !s.state.terminal() {
                    s.elapsed_ms = entry.start.elapsed().as_secs_f64() * 1000.0;
                }
                s
            })
            .collect();
        Report {
            active: self.active.load(Ordering::SeqCst),
            queued: jobs.iter().filter(|j| j.state == State::Queued).count(),
            jobs,
            max_active: self.peak.load(Ordering::SeqCst),
            queue_limit: 2,
            cpu_limit: 2,
            scheduling: self.budget.stats(),
        }
    }
    pub async fn start(self: &Arc<Self>, request: Request) -> Result<Snapshot, String> {
        request.validate()?;
        let entry = {
            let mut entries = self.entries.lock().unwrap();
            if self.stopped.load(Ordering::SeqCst) {
                return Err("Scheduler is shutting down".into());
            }
            if entries
                .iter()
                .filter(|e| !e.snapshot.lock().unwrap().state.terminal())
                .count()
                >= 3
                || entries
                    .iter()
                    .filter(|e| e.snapshot.lock().unwrap().state == State::Queued)
                    .count()
                    >= 2
            {
                return Err("Synthetic queue full (one active plus two queued)".into());
            }
            while entries.len() >= HISTORY {
                let index = entries
                    .iter()
                    .position(|e| e.snapshot.lock().unwrap().state.terminal())
                    .ok_or("History full")?;
                entries.remove(index);
            }
            let snapshot = Snapshot {
                id: self.next.fetch_add(1, Ordering::SeqCst),
                state: State::Queued,
                request,
                processed: 0,
                committed: 0,
                batches: 0,
                digest: "0000000000000000".into(),
                elapsed_ms: 0.0,
                cancel_latency_ms: None,
                error: None,
                busy_retries: 0,
                transaction_ms: vec![],
                pool_wait_ms: vec![],
                progress_revision: 0,
            };
            let entry = Arc::new(Entry {
                snapshot: Mutex::new(snapshot),
                cancel: Arc::new(AtomicBool::new(false)),
                notify: Notify::new(),
                start: Instant::now(),
                cancel_at: Mutex::new(None),
            });
            entries.push_back(Arc::clone(&entry));
            entry
        };
        let snapshot = entry.snapshot.lock().unwrap().clone();
        let inserted=async { let _writer=self.writer.lock().await;
            sqlx::query("INSERT INTO simulation_runs(id,state,request,seed,total,digest) VALUES(?,'queued',?,?,?,?)").bind(snapshot.id as i64).bind(serde_json::to_string(&snapshot.request).map_err(|_| "Invalid request")?).bind(snapshot.request.seed.to_string()).bind(snapshot.request.records).bind(&snapshot.digest).execute(&self.db.pool).await.map_err(|_| "Could not reserve synthetic ledger")?;
            sqlx::query("DELETE FROM simulation_runs WHERE id NOT IN (SELECT id FROM simulation_runs ORDER BY id DESC LIMIT 8) AND state IN ('completed','cancelled','failed','interrupted')").execute(&self.db.pool).await.map_err(|_| "Could not prune synthetic ledger")?;
            Ok::<(),String>(()) }.await;
        if let Err(message) = inserted {
            let mut s = entry.snapshot.lock().unwrap();
            s.state = State::Failed;
            s.error = Some(message.clone());
            return Err(message);
        }
        let scheduler = Arc::clone(self);
        let task = tokio::spawn(async move {
            scheduler.run(entry).await;
        });
        let mut tasks = self.tasks.lock().unwrap();
        tasks.retain(|h| !h.is_finished());
        tasks.push_back(task);
        Ok(snapshot)
    }
    pub fn cancel(&self, id: u64) -> Result<Snapshot, String> {
        let entries = self.entries.lock().unwrap();
        let entry = entries
            .iter()
            .find(|e| e.snapshot.lock().unwrap().id == id)
            .ok_or("Unknown synthetic job")?;
        let mut s = entry.snapshot.lock().unwrap();
        if !s.state.terminal() && s.state != State::Finalizing {
            s.state = State::CancelRequested;
            entry.cancel.store(true, Ordering::SeqCst);
            let mut at = entry.cancel_at.lock().unwrap();
            if at.is_none() {
                *at = Some(Instant::now());
            }
            entry.notify.notify_waiters();
        }
        Ok(s.clone())
    }
    pub async fn shutdown(&self) -> Result<(), String> {
        self.stopped.store(true, Ordering::SeqCst);
        let ids: Vec<_> = self
            .report()
            .jobs
            .into_iter()
            .filter(|j| !j.state.terminal())
            .map(|j| j.id)
            .collect();
        for id in ids {
            self.cancel(id)?;
        }
        for _ in 0..1000 {
            if self.report().jobs.iter().all(|j| j.state.terminal()) {
                let tasks: Vec<_> = self.tasks.lock().unwrap().drain(..).collect();
                for task in tasks {
                    task.await
                        .map_err(|_| "Synthetic task failed during shutdown")?;
                }
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        Err("Shutdown still awaiting safe job boundary".into())
    }
    async fn run(self: Arc<Self>, entry: Arc<Entry>) {
        let permit = loop {
            if entry.cancel.load(Ordering::SeqCst) {
                break None;
            }
            tokio::select! {
                _=entry.notify.notified()=>{},
                _=tokio::time::sleep(std::time::Duration::from_millis(5))=>{},
                permit=Arc::clone(&self.slots).acquire_owned()=>{break permit.ok();}
            }
        };
        if permit.is_some() {
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.peak.fetch_max(active, Ordering::SeqCst);
        }
        let result = if permit.is_some() && !entry.cancel.load(Ordering::SeqCst) {
            {
                let mut s = entry.snapshot.lock().unwrap();
                if s.state == State::Queued {
                    s.state = State::Running;
                }
            }
            let ready = {
                let _writer = self.writer.lock().await;
                let id = entry.snapshot.lock().unwrap().id;
                sqlx::query("UPDATE simulation_runs SET state='running' WHERE id=?")
                    .bind(id as i64)
                    .execute(&self.db.pool)
                    .await
            };
            if ready.is_err() {
                Err("RUN_CHECKPOINT_FAILED".into())
            } else {
                self.execute(&entry).await
            }
        } else {
            Ok(())
        };
        let mut terminal = {
            let mut snapshot = entry.snapshot.lock().unwrap();
            let terminal = snapshot.clone();
            snapshot.state = State::Finalizing;
            terminal
        };
        terminal.state = if result.is_err() {
            State::Failed
        } else if entry.cancel.load(Ordering::SeqCst) {
            State::Cancelled
        } else {
            State::Completed
        };
        terminal.error = result.err();
        terminal.elapsed_ms = entry.start.elapsed().as_secs_f64() * 1000.0;
        terminal.cancel_latency_ms = entry
            .cancel_at
            .lock()
            .unwrap()
            .map(|at| at.elapsed().as_secs_f64() * 1000.0);
        {
            let _writer = self.writer.lock().await;
            let state = serde_json::to_value(terminal.state).unwrap();
            let saved=sqlx::query("UPDATE simulation_runs SET state=?,processed=?,committed=?,digest=?,elapsed_ms=? WHERE id=?").bind(state.as_str().unwrap()).bind(terminal.processed).bind(terminal.committed).bind(&terminal.digest).bind(terminal.elapsed_ms).bind(terminal.id as i64).execute(&self.db.pool).await;
            if saved.is_err() {
                terminal.state = State::Failed;
                terminal.error = Some(format!(
                    "{};TERMINAL_CHECKPOINT_IO",
                    terminal.error.as_deref().unwrap_or("CHECKPOINT_FAILED")
                ));
            }
        }
        if permit.is_some() {
            self.active.fetch_sub(1, Ordering::SeqCst);
        }
        drop(permit);
        // Publish a terminal snapshot only after worker, transaction and slot cleanup.
        terminal.elapsed_ms = entry.start.elapsed().as_secs_f64() * 1000.0;
        terminal.cancel_latency_ms = entry
            .cancel_at
            .lock()
            .unwrap()
            .map(|at| at.elapsed().as_secs_f64() * 1000.0);
        *entry.snapshot.lock().unwrap() = terminal;
    }
    async fn execute(&self, entry: &Arc<Entry>) -> Result<(), String> {
        let request = entry.snapshot.lock().unwrap().request.clone();
        while entry.snapshot.lock().unwrap().processed < request.records {
            if entry.cancel.load(Ordering::SeqCst) {
                return Ok(());
            }
            let start = entry.snapshot.lock().unwrap().processed;
            let count = BATCH.min(request.records - start);
            let Some(worker) = self.budget.acquire(&entry.cancel).await else {
                return Ok(());
            };
            let cancel = Arc::clone(&entry.cancel);
            let root = self.root.clone();
            let config = self.config.clone();
            let req = request.clone();
            let outcome = tokio::task::spawn_blocking(move || {
                let _worker = worker;
                process(&root, &config, &req, start, count, &cancel)
            })
            .await
            .map_err(|_| "SYNTHETIC_WORKER_FAILED")?;
            let records = outcome.records;
            {
                let mut s = entry.snapshot.lock().unwrap();
                s.processed += records.len() as u32;
                s.digest = format!(
                    "{:016x}",
                    records
                        .iter()
                        .fold(u64::from_str_radix(&s.digest, 16).unwrap_or(0), |h, r| h
                            .rotate_left(1)
                            ^ r.digest)
                );
                s.progress_revision += 1;
            }
            if let Some(error) = outcome.failure {
                return Err(error);
            }
            if entry.cancel.load(Ordering::SeqCst) {
                return Ok(());
            }
            if matches!(request.kind, Kind::Write | Kind::Mixed) {
                self.commit(entry, &records).await?;
            }
            if request.intensity == Intensity::Gentle {
                tokio::time::sleep(std::time::Duration::from_millis(4)).await;
            } else {
                tokio::task::yield_now().await;
            }
        }
        Ok(())
    }
    async fn commit(&self, entry: &Entry, records: &[Record]) -> Result<(), String> {
        let _writer = self.writer.lock().await;
        for attempt in 0..4 {
            if entry.cancel.load(Ordering::SeqCst) {
                return Ok(());
            }
            let waiting = Instant::now();
            let mut connection = tokio::time::timeout(
                std::time::Duration::from_millis(250),
                self.db.pool.acquire(),
            )
            .await
            .map_err(|_| "POOL_EXHAUSTED")?
            .map_err(|_| "POOL_CLOSED")?;
            {
                let mut s = entry.snapshot.lock().unwrap();
                sample(
                    &mut s.pool_wait_ms,
                    waiting.elapsed().as_secs_f64() * 1000.0,
                );
            }
            let started = Instant::now();
            let mut tx = connection.begin().await.map_err(|_| "BEGIN_FAILED")?;
            let snapshot = entry.snapshot.lock().unwrap().clone();
            let result=async {
                for record in records {
                    sqlx::query("INSERT INTO simulation_items(job_id,ordinal,game_id,digest) VALUES(?,?,?,?)").bind(snapshot.id as i64).bind(record.ordinal).bind(&record.game_id).bind(format!("{:016x}",record.digest)).execute(&mut *tx).await?;
                    sqlx::query("UPDATE games SET developer=?,updated_at_ms=? WHERE id=?").bind(format!("Synthetic job studio {}",record.digest%32)).bind(i64::from(record.ordinal)).bind(&record.game_id).execute(&mut *tx).await?;
                }
                sqlx::query("UPDATE simulation_runs SET committed=?,processed=?,digest=?,elapsed_ms=? WHERE id=?").bind(snapshot.committed+records.len() as u32).bind(snapshot.processed).bind(&snapshot.digest).bind(entry.start.elapsed().as_secs_f64()*1000.0).bind(snapshot.id as i64).execute(&mut *tx).await?;
                Ok::<(),sqlx::Error>(())
            }.await;
            if result.is_ok() && snapshot.request.fault == Fault::Write && snapshot.batches == 1 {
                tx.rollback().await.map_err(|_| "ROLLBACK_FAILED")?;
                return Err("SIMULATED_WRITE_FAILURE_ROLLED_BACK".into());
            }
            if entry.cancel.load(Ordering::SeqCst) {
                tx.rollback().await.map_err(|_| "ROLLBACK_FAILED")?;
                return Ok(());
            }
            match result {
                Ok(()) => {
                    tx.commit().await.map_err(|_| "COMMIT_FAILED")?;
                    let mut s = entry.snapshot.lock().unwrap();
                    s.committed += records.len() as u32;
                    s.batches += 1;
                    sample(
                        &mut s.transaction_ms,
                        started.elapsed().as_secs_f64() * 1000.0,
                    );
                    return Ok(());
                }
                Err(e) => {
                    let busy = e.as_database_error().is_some_and(|e| {
                        e.code()
                            .is_some_and(|c| matches!(c.as_ref(), "5" | "6" | "261" | "517"))
                    });
                    tx.rollback().await.map_err(|_| "ROLLBACK_FAILED")?;
                    if !busy || attempt == 3 {
                        return Err(if busy {
                            "SQLITE_BUSY_RETRY_LIMIT"
                        } else {
                            "SYNTHETIC_WRITE_FAILED"
                        }
                        .into());
                    }
                    entry.snapshot.lock().unwrap().busy_retries += 1;
                    drop(connection);
                    tokio::time::sleep(std::time::Duration::from_millis(2)).await;
                }
            }
        }
        Err("WRITE_RETRY_LIMIT".into())
    }
}
fn sample(v: &mut Vec<f64>, n: f64) {
    if v.len() == 64 {
        v.remove(0);
    }
    v.push(n);
}
struct Record {
    ordinal: u32,
    game_id: String,
    digest: u64,
}
struct BatchOutcome {
    records: Vec<Record>,
    failure: Option<String>,
}
fn process(
    root: &std::path::Path,
    config: &fixtures::Marker,
    request: &Request,
    start: u32,
    count: u32,
    cancel: &AtomicBool,
) -> BatchOutcome {
    let mut records = Vec::with_capacity(BATCH as usize);
    let result = (|| -> Result<(), String> {
        if request.fault == Fault::Resource {
            return Err("SIMULATED_RESOURCE_LIMIT".into());
        }
        let path = root.join("hash.bin");
        if path
            .canonicalize()
            .map_err(|_| "HASH_FIXTURE_UNAVAILABLE")?
            != path
            || fs::metadata(&path)
                .map_err(|_| "HASH_FIXTURE_UNAVAILABLE")?
                .len()
                != 256 * 1024
        {
            return Err("HASH_FIXTURE_INVALID".into());
        }
        for ordinal in start..start + count {
            if cancel.load(Ordering::SeqCst) {
                break;
            }
            if request.fault == Fault::Parse && ordinal >= BATCH {
                return Err("SIMULATED_METADATA_PARSE_FAILURE".into());
            }
            let game_id = format!("syn-{:016x}-g-{:05}", config.seed, ordinal % config.games);
            let input = format!(
                "{{\"game\":\"{game_id}\",\"value\":{},\"title\":\"Synthetic discovery record\"}}",
                u64::from(request.seed) + u64::from(ordinal)
            );
            let mut digest = u64::from(request.seed) ^ u64::from(ordinal);
            let rounds = if request.intensity == Intensity::Saturated {
                64
            } else {
                16
            };
            for _ in 0..rounds {
                let parsed: serde_json::Value =
                    serde_json::from_str(&input).map_err(|_| "METADATA_PARSE_FAILED")?;
                digest = digest.wrapping_add(std::hint::black_box(
                    parsed["value"].as_u64().ok_or("INVALID_METADATA")?,
                ));
            }
            if matches!(request.kind, Kind::Hash | Kind::Mixed) {
                let mut file = fs::File::open(&path).map_err(|_| "HASH_READ_FAILED")?;
                let mut buffer = [0u8; 16 * 1024];
                for _ in 0..16 {
                    if cancel.load(Ordering::SeqCst) {
                        return Ok(());
                    }
                    file.read_exact(&mut buffer)
                        .map_err(|_| "HASH_READ_FAILED")?;
                    digest = digest.rotate_left(1)
                        ^ crate::artwork::fingerprint(std::hint::black_box(&buffer));
                    if request.intensity == Intensity::Slow {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                }
            }
            records.push(Record {
                ordinal,
                game_id,
                digest,
            });
        }
        if request.kind == Kind::Mixed && !cancel.load(Ordering::SeqCst) {
            let image = image::RgbImage::from_fn(320, 180, |x, y| {
                image::Rgb([x as u8, y as u8, request.seed as u8])
            });
            std::hint::black_box(image::imageops::resize(
                &image,
                64,
                36,
                image::imageops::FilterType::Triangle,
            ));
        }
        Ok(())
    })();
    BatchOutcome {
        records,
        failure: result.err(),
    }
}
