// SPDX-License-Identifier: GPL-3.0-or-later
//! Small resource budget, not an OS priority or deadline guarantee.
use serde::Serialize;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[derive(Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub foreground_active: usize,
    pub foreground_max: usize,
    pub workers_active: usize,
    pub workers_max: usize,
    pub backoffs: u64,
    pub foreground_ms: Vec<f64>,
    pub worker_wait_ms: Vec<f64>,
}
pub struct Budget {
    semaphore: Arc<Semaphore>,
    active: AtomicUsize,
    foreground: bool,
    last_request: Mutex<Instant>,
    stats: Mutex<Stats>,
}
impl Budget {
    pub fn new(foreground: bool) -> Arc<Self> {
        Arc::new(Self {
            semaphore: Arc::new(Semaphore::new(2)),
            active: AtomicUsize::new(0),
            foreground,
            last_request: Mutex::new(Instant::now() - Duration::from_secs(1)),
            stats: Mutex::new(Stats::default()),
        })
    }
    pub fn stats(&self) -> Stats {
        self.stats.lock().unwrap().clone()
    }
    pub fn enter(self: &Arc<Self>) -> Foreground {
        *self.last_request.lock().unwrap() = Instant::now();
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        let mut stats = self.stats.lock().unwrap();
        stats.foreground_active = active;
        stats.foreground_max = stats.foreground_max.max(active);
        Foreground {
            budget: Arc::clone(self),
            start: Instant::now(),
        }
    }
    pub async fn acquire(self: &Arc<Self>, cancel: &AtomicBool) -> Option<Worker> {
        let start = Instant::now();
        loop {
            if cancel.load(Ordering::SeqCst) {
                return None;
            }
            if self.foreground
                && (self.active.load(Ordering::SeqCst) > 0
                    || self.last_request.lock().unwrap().elapsed() < Duration::from_millis(8))
            {
                self.stats.lock().unwrap().backoffs += 1;
                tokio::time::sleep(Duration::from_millis(2)).await;
                continue;
            }
            let permit = match tokio::time::timeout(
                Duration::from_millis(5),
                Arc::clone(&self.semaphore).acquire_owned(),
            )
            .await
            {
                Ok(Ok(permit)) => permit,
                Ok(Err(_)) => return None,
                Err(_) => continue,
            };
            if cancel.load(Ordering::SeqCst) {
                return None;
            }
            let mut stats = self.stats.lock().unwrap();
            stats.workers_active += 1;
            stats.workers_max = stats.workers_max.max(stats.workers_active);
            bounded(
                &mut stats.worker_wait_ms,
                start.elapsed().as_secs_f64() * 1000.0,
            );
            return Some(Worker {
                budget: Arc::clone(self),
                _permit: permit,
            });
        }
    }
}
pub struct Foreground {
    budget: Arc<Budget>,
    start: Instant,
}
impl Drop for Foreground {
    fn drop(&mut self) {
        let active = self.budget.active.fetch_sub(1, Ordering::SeqCst) - 1;
        let mut stats = self.budget.stats.lock().unwrap();
        stats.foreground_active = active;
        bounded(
            &mut stats.foreground_ms,
            self.start.elapsed().as_secs_f64() * 1000.0,
        );
    }
}
pub struct Worker {
    budget: Arc<Budget>,
    _permit: OwnedSemaphorePermit,
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.budget.stats.lock().unwrap().workers_active -= 1;
    }
}
fn bounded(samples: &mut Vec<f64>, value: f64) {
    if samples.len() == 1024 {
        samples.remove(0);
    }
    samples.push(value);
}
