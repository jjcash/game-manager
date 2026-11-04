// SPDX-License-Identifier: GPL-3.0-or-later
//! M2.1 experiment only. Never imported by src/; no launch, attach, kill or DB API.
use std::{
    io,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    sync::Arc,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{FILETIME, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{
        CreateEventW, GetExitCodeProcess, GetProcessId, GetProcessTimes, SetEvent,
        WaitForMultipleObjects, WaitForSingleObject,
    },
};

pub const LIMIT: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    pub pid: u32,
    pub created_100ns: u64,
}

fn ticks(time: FILETIME) -> u64 {
    (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime)
}

fn times(handle: &OwnedHandle) -> io::Result<(u64, u64)> {
    let (mut created, mut exited, mut kernel, mut user) = Default::default();
    // SAFETY: owned live handle and four correctly sized outputs. Exit time is
    // consumed only after the process object has been observed signaled.
    if unsafe {
        GetProcessTimes(
            handle.as_raw_handle(),
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok((ticks(created), ticks(exited)))
}

#[derive(Clone)]
pub struct Stop(Arc<OwnedHandle>);
impl Stop {
    pub fn signal(&self) -> io::Result<()> {
        // SAFETY: Arc keeps this event live even while another thread waits.
        if unsafe { SetEvent(self.0.as_raw_handle()) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Reason {
    Exited { code: u32, exited_100ns: u64 },
    Cancelled,
    Shutdown,
    NativeFailure(Option<i32>),
}

#[derive(Debug)]
pub struct Completion {
    pub id: u64,
    pub identity: Identity,
    pub reason: Reason,
    pub saw_alive: bool,
    /// Time between adoption and terminal observation, including detection lag.
    pub observation_window: Duration,
    /// Last confirmed alive sample relative to adoption. Not gameplay.
    pub confirmed_alive_span: Duration,
}

struct Entry {
    id: u64,
    handle: OwnedHandle,
    identity: Identity,
    started: Instant,
    last_alive: Duration,
    saw_alive: bool,
}
impl Entry {
    fn complete(self, reason: Reason) -> Completion {
        Completion {
            id: self.id,
            identity: self.identity,
            reason,
            saw_alive: self.saw_alive,
            observation_window: self.started.elapsed(),
            confirmed_alive_span: self.last_alive,
        }
        // OwnedHandle drops here; closing a process handle does not kill it.
    }
}

pub struct Observer {
    stop: Stop,
    entries: Vec<Entry>,
    closed: bool,
    next_id: u64,
    pub waits: u64,
}
impl Observer {
    pub fn new() -> io::Result<Self> {
        // SAFETY: unnamed, non-inheritable, manual-reset event; owned on success.
        let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
        if event.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            stop: Stop(Arc::new(unsafe { OwnedHandle::from_raw_handle(event) })),
            entries: Vec::with_capacity(LIMIT),
            closed: false,
            next_id: 0,
            waits: 0,
        })
    }
    pub fn stop(&self) -> Stop {
        self.stop.clone()
    }
    pub fn active(&self) -> usize {
        self.entries.len()
    }
    /// Ownership transfer only. No PID-based reopening. On rejection, releases
    /// the supplied handle without terminating the already created process.
    pub fn adopt(&mut self, handle: OwnedHandle) -> io::Result<(u64, Identity)> {
        if self.closed || self.entries.len() == LIMIT {
            return Err(io::Error::other("observation unavailable"));
        }
        let created_100ns = times(&handle)?.0;
        // SAFETY: borrowed live process handle; zero reports failure.
        let pid = unsafe { GetProcessId(handle.as_raw_handle()) };
        if pid == 0 {
            return Err(io::Error::last_os_error());
        }
        let started = Instant::now();
        let state = unsafe { WaitForSingleObject(handle.as_raw_handle(), 0) };
        if state != WAIT_OBJECT_0 && state != WAIT_TIMEOUT {
            return Err(io::Error::last_os_error());
        }
        let identity = Identity { pid, created_100ns };
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push(Entry {
            id,
            handle,
            identity,
            started,
            last_alive: Duration::ZERO,
            saw_alive: state == WAIT_TIMEOUT,
        });
        Ok((id, identity))
    }
    /// Call only on the owning thread between waits. No close-during-wait race.
    pub fn cancel(&mut self, id: u64) -> Option<Completion> {
        let index = self.entries.iter().position(|entry| entry.id == id)?;
        Some(self.entries.remove(index).complete(Reason::Cancelled))
    }
    fn interrupt(&mut self, reason: impl Fn() -> Reason) -> Vec<Completion> {
        self.closed = true;
        self.entries
            .drain(..)
            .map(|e| e.complete(reason()))
            .collect()
    }
    /// One shared blocking wait, not one polling thread per process. The owner
    /// alone mutates entries; Stop only signals an independently owned event.
    pub fn step(&mut self, timeout_ms: u32) -> Vec<Completion> {
        if self.closed {
            return Vec::new();
        }
        let mut handles = [std::ptr::null_mut(); LIMIT + 1];
        handles[0] = self.stop.0.as_raw_handle();
        for (slot, entry) in handles[1..].iter_mut().zip(&self.entries) {
            *slot = entry.handle.as_raw_handle();
        }
        self.waits += 1;
        // SAFETY: distinct live handles, fixed bound, none closed during wait.
        // Stop wins a simultaneous signal by occupying the lowest index.
        let state = unsafe {
            WaitForMultipleObjects(
                (self.entries.len() + 1) as u32,
                handles.as_ptr(),
                0,
                timeout_ms,
            )
        };
        if state == WAIT_OBJECT_0 {
            return self.interrupt(|| Reason::Shutdown);
        }
        if state == WAIT_FAILED {
            let code = io::Error::last_os_error().raw_os_error();
            return self.interrupt(|| Reason::NativeFailure(code));
        }
        if state != WAIT_TIMEOUT
            && !(WAIT_OBJECT_0 + 1..WAIT_OBJECT_0 + 1 + self.entries.len() as u32).contains(&state)
        {
            return self.interrupt(|| Reason::NativeFailure(None));
        }
        let mut completed = Vec::with_capacity(LIMIT);
        let mut index = 0;
        while index < self.entries.len() {
            let entry = &mut self.entries[index];
            let state = unsafe { WaitForSingleObject(entry.handle.as_raw_handle(), 0) };
            if state == WAIT_TIMEOUT {
                entry.saw_alive = true;
                entry.last_alive = entry.started.elapsed();
                index += 1;
                continue;
            }
            let reason = if state == WAIT_OBJECT_0 {
                let mut code = 0;
                // Test signaled state first: exit code 259 is a valid terminal
                // code here and must not be mistaken for STILL_ACTIVE.
                if unsafe { GetExitCodeProcess(entry.handle.as_raw_handle(), &mut code) } == 0 {
                    Reason::NativeFailure(io::Error::last_os_error().raw_os_error())
                } else {
                    match times(&entry.handle) {
                        Ok((_, exited_100ns)) => Reason::Exited { code, exited_100ns },
                        Err(e) => Reason::NativeFailure(e.raw_os_error()),
                    }
                }
            } else {
                Reason::NativeFailure(io::Error::last_os_error().raw_os_error())
            };
            completed.push(self.entries.remove(index).complete(reason));
        }
        completed
    }
}
