// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(windows)]
//! Complete local M2.1 native target. No production session code is exercised.
#[path = "support/session_observation.rs"]
mod candidate;

use candidate::{Completion, Identity, LIMIT, Observer, Reason};
use std::{
    cell::OnceCell,
    fs,
    os::windows::{
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};
use tempfile::{TempDir, tempdir};
use windows_sys::Win32::{
    Foundation::{DuplicateHandle, FILETIME, WAIT_OBJECT_0, WAIT_TIMEOUT},
    Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation},
    System::Threading::{
        CreateEventW, GetCurrentProcess, GetProcessHandleCount, GetProcessTimes, OpenProcess,
        OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
        WaitForSingleObject,
    },
};

// Warm fixture compilation once, then serialize all resource measurements.
static NATIVE: Mutex<()> = Mutex::new(());
thread_local! {
    // libtest owns each test thread; its destructor removes the compiled binary.
    static BINARY: OnceCell<(TempDir, PathBuf)> = const { OnceCell::new() };
}
fn binary() -> PathBuf {
    BINARY.with(|cell| {
        cell.get_or_init(|| {
            unelevated();
            let root = tempfile::Builder::new()
                .prefix("gm-m21-binary-")
                .tempdir()
                .unwrap();
            let exe = root.path().join("synthetic-session.exe");
            let output = Command::new("rustc")
                .args(["--edition=2024", "-Dwarnings"])
                .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/session/child.rs"))
                .arg("-o")
                .arg(&exe)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            (root, exe)
        })
        .1
        .clone()
    })
}
fn unelevated() {
    let mut raw = std::ptr::null_mut();
    assert_ne!(
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw) },
        0
    );
    let token = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut info = TOKEN_ELEVATION::default();
    let mut size = 0;
    assert_ne!(
        unsafe {
            GetTokenInformation(
                token.as_raw_handle(),
                TokenElevation,
                (&mut info as *mut TOKEN_ELEVATION).cast(),
                size_of::<TOKEN_ELEVATION>() as u32,
                &mut size,
            )
        },
        0
    );
    assert_eq!(
        info.TokenIsElevated, 0,
        "Use an ordinary unelevated Windows desktop"
    );
}
fn handles() -> u32 {
    let mut count = 0;
    assert_ne!(
        unsafe { GetProcessHandleCount(GetCurrentProcess(), &mut count) },
        0
    );
    count
}
fn cpu_100ns() -> u64 {
    let (mut created, mut exited, mut kernel, mut user): (FILETIME, FILETIME, FILETIME, FILETIME) =
        Default::default();
    assert_ne!(
        unsafe {
            GetProcessTimes(
                GetCurrentProcess(),
                &mut created,
                &mut exited,
                &mut kernel,
                &mut user,
            )
        },
        0
    );
    let ticks = |t: FILETIME| (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime);
    ticks(kernel) + ticks(user)
}
fn observation_handle(child: &Child) -> OwnedHandle {
    let mut raw = std::ptr::null_mut();
    // SAFETY: live source/target process and child. Restrict observer access:
    // no terminate, VM read, write, debug or child enumeration permissions.
    assert_ne!(
        unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                child.as_raw_handle(),
                GetCurrentProcess(),
                &mut raw,
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                0,
                0,
            )
        },
        0
    );
    unsafe { OwnedHandle::from_raw_handle(raw) }
}
fn wait_file(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(12);
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "fixture timed out: {}",
            path.display()
        );
        thread::sleep(Duration::from_millis(5));
    }
}
struct Fixture {
    root: TempDir,
    child: Child,
}
impl Fixture {
    fn new(mode: &str, code: i32) -> Self {
        let root = tempfile::Builder::new()
            .prefix("gm-m21-case-")
            .tempdir()
            .unwrap();
        let child = Command::new(binary())
            .arg(mode)
            .arg(root.path())
            .arg(code.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(0x08000000)
            .spawn()
            .unwrap();
        Self { root, child }
    }
    fn ready(&self) {
        wait_file(&self.root.path().join("ready"));
    }
    fn release(&self) {
        fs::write(self.root.path().join("release"), b"release").unwrap();
    }
    fn alive(&self) {
        assert_eq!(
            unsafe { WaitForSingleObject(self.child.as_raw_handle(), 0) },
            WAIT_TIMEOUT
        );
    }
    fn finish(&mut self) {
        self.release();
        assert_eq!(
            unsafe { WaitForSingleObject(self.child.as_raw_handle(), 12000) },
            WAIT_OBJECT_0
        );
        self.child.wait().unwrap();
        assert!(!self.root.path().join("watchdog").exists());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Failure cleanup is cooperative too. Never Child::kill/TerminateProcess.
        let _ = fs::write(self.root.path().join("release"), b"release");
        let descendant = self.root.path().join("descendant");
        if descendant.is_dir() {
            let _ = fs::write(descendant.join("release"), b"release");
        }
        let _ = self.child.wait();
    }
}
fn complete(observer: &mut Observer) -> Completion {
    let mut result = observer.step(12000);
    assert_eq!(result.len(), 1);
    assert_eq!(observer.active(), 0);
    assert!(
        observer.step(0).is_empty(),
        "completion must not be emitted twice"
    );
    result.remove(0)
}
fn exited(result: &Completion, code: u32) {
    let Reason::Exited {
        code: actual,
        exited_100ns,
    } = result.reason
    else {
        panic!("{result:?}");
    };
    assert_eq!(actual, code);
    assert!(exited_100ns >= result.identity.created_100ns);
    assert!(result.confirmed_alive_span <= result.observation_window);
}

#[test]
fn a_direct_child_alive_then_normal_exit() {
    let _serial = NATIVE.lock().unwrap();
    let mut f = Fixture::new("hold", 0);
    f.ready();
    let mut observer = Observer::new().unwrap();
    let (_, identity) = observer.adopt(observation_handle(&f.child)).unwrap();
    assert_eq!(identity.pid, f.child.id());
    assert!(observer.step(20).is_empty());
    f.release();
    let result = complete(&mut observer);
    exited(&result, 0);
    assert!(result.saw_alive);
    assert!(result.confirmed_alive_span > Duration::ZERO);
    eprintln!("M2.1 normal observation: {result:?}");
    f.finish();
}
#[test]
fn b_immediate_exit_before_adoption_does_not_invent_alive() {
    let _serial = NATIVE.lock().unwrap();
    let mut f = Fixture::new("immediate", 0);
    assert_eq!(
        unsafe { WaitForSingleObject(f.child.as_raw_handle(), 12000) },
        WAIT_OBJECT_0
    );
    let mut observer = Observer::new().unwrap();
    observer.adopt(observation_handle(&f.child)).unwrap();
    let result = complete(&mut observer);
    exited(&result, 0);
    assert!(!result.saw_alive);
    assert_eq!(result.confirmed_alive_span, Duration::ZERO);
    f.finish();
}
#[test]
fn c_nonzero_and_still_active_numeric_exit_are_terminal() {
    let _serial = NATIVE.lock().unwrap();
    for code in [17, 259] {
        let mut f = Fixture::new("immediate", code);
        let mut observer = Observer::new().unwrap();
        observer.adopt(observation_handle(&f.child)).unwrap();
        exited(&complete(&mut observer), code as u32);
        f.finish();
    }
}
#[test]
fn d_handoff_direct_exit_does_not_mean_descendant_exit() {
    let _serial = NATIVE.lock().unwrap();
    let mut f = Fixture::new("handoff", 0);
    let mut observer = Observer::new().unwrap();
    observer.adopt(observation_handle(&f.child)).unwrap();
    exited(&complete(&mut observer), 0);
    let next = f.root.path().join("descendant");
    wait_file(&next.join("ready"));
    assert!(!next.join("done").exists());
    // Only test-owned provenance permits this verification open, never candidate
    // inference or attachment. Hold it before release and wait for actual exit.
    let pid: u32 = fs::read_to_string(next.join("ready"))
        .unwrap()
        .parse()
        .unwrap();
    let raw = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    assert!(!raw.is_null());
    let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
    assert_eq!(
        unsafe { WaitForSingleObject(handle.as_raw_handle(), 0) },
        WAIT_TIMEOUT
    );
    fs::write(next.join("release"), b"release").unwrap();
    assert_eq!(
        unsafe { WaitForSingleObject(handle.as_raw_handle(), 12000) },
        WAIT_OBJECT_0
    );
    assert!(!next.join("watchdog").exists());
    eprintln!(
        "M2.1 handoff: direct exited; test-owned descendant was still alive; no automatic attachment"
    );
    f.finish();
}
#[test]
fn e_drop_observer_leaves_child_alive() {
    let _serial = NATIVE.lock().unwrap();
    let mut f = Fixture::new("hold", 0);
    f.ready();
    let before = handles();
    let mut observer = Observer::new().unwrap();
    observer.adopt(observation_handle(&f.child)).unwrap();
    drop(observer);
    assert_eq!(handles(), before);
    f.alive();
    f.finish();
}

// Same exact outer test acts as a synthetic manager helper. Its fixture root
// belongs to the outer process, so it survives both clean exit and hard exit.
fn parent_exit(abrupt: bool) {
    if let Some(root) = std::env::var_os("GM_M21_PARENT_ROOT") {
        unelevated();
        let root = PathBuf::from(root);
        let child = Command::new(std::env::var_os("GM_M21_BINARY").unwrap())
            .arg("hold")
            .arg(&root)
            .arg("0")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(0x08000000)
            .spawn()
            .unwrap();
        let mut observer = Observer::new().unwrap();
        let (_, identity) = observer.adopt(observation_handle(&child)).unwrap();
        wait_file(&root.join("ready"));
        fs::write(
            root.join("identity"),
            format!("{} {}", identity.pid, identity.created_100ns),
        )
        .unwrap();
        if !abrupt {
            drop(observer);
            drop(child);
        }
        std::process::exit(if abrupt { 73 } else { 0 });
    }
    let root = tempdir().unwrap();
    let name = if abrupt {
        "h_unexpected_observer_failure"
    } else {
        "f_child_survives_manager_parent_exit"
    };
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture"])
        .env("GM_M21_PARENT_ROOT", root.path())
        .env("GM_M21_BINARY", binary())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(if abrupt { 73 } else { 0 }));
    let identity: Vec<u64> = fs::read_to_string(root.path().join("identity"))
        .unwrap()
        .split_whitespace()
        .map(|v| v.parse().unwrap())
        .collect();
    assert!(!root.path().join("done").exists());
    // Verification of a known test-owned process with creation-time matching.
    let raw = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            0,
            identity[0] as u32,
        )
    };
    assert!(!raw.is_null());
    let mut verifier = Observer::new().unwrap();
    let (_, actual) = verifier
        .adopt(unsafe { OwnedHandle::from_raw_handle(raw) })
        .unwrap();
    assert_eq!(
        actual,
        Identity {
            pid: identity[0] as u32,
            created_100ns: identity[1]
        }
    );
    assert!(verifier.step(0).is_empty());
    fs::write(root.path().join("release"), b"release").unwrap();
    exited(&complete(&mut verifier), 0);
    assert!(!root.path().join("watchdog").exists());
    eprintln!(
        "M2.1 parent exited code {:?}; child survived and exited cooperatively",
        status.code()
    );
}
#[test]
fn f_child_survives_manager_parent_exit() {
    let _serial = NATIVE.lock().unwrap();
    parent_exit(false);
}
#[test]
fn g_cancel_and_stop_during_active_wait() {
    let _serial = NATIVE.lock().unwrap();
    let mut f = Fixture::new("hold", 0);
    f.ready();
    let mut observer = Observer::new().unwrap();
    let (id, _) = observer.adopt(observation_handle(&f.child)).unwrap();
    assert_eq!(observer.cancel(id).unwrap().reason, Reason::Cancelled);
    assert!(observer.cancel(id).is_none());
    f.alive();
    observer.adopt(observation_handle(&f.child)).unwrap();
    let stop = observer.stop();
    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(0);
    let worker = thread::spawn(move || {
        entered_tx.send(()).unwrap();
        let result = observer.step(u32::MAX);
        assert!(observer.adopt(observation_handle_for_event()).is_err());
        (observer, result)
    });
    entered_rx.recv().unwrap();
    thread::sleep(Duration::from_millis(30));
    let start = Instant::now();
    stop.signal().unwrap();
    let (observer, result) = worker.join().unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].reason, Reason::Shutdown);
    assert_eq!(observer.active(), 0);
    eprintln!("M2.1 active wait stop/join: {:?}", start.elapsed());
    drop(observer);
    drop(stop);
    f.alive();
    f.finish();
}
fn observation_handle_for_event() -> OwnedHandle {
    let raw = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
    assert!(!raw.is_null());
    unsafe { OwnedHandle::from_raw_handle(raw) }
}
#[test]
fn h_unexpected_observer_failure() {
    let _serial = NATIVE.lock().unwrap();
    parent_exit(true);
    let mut f = Fixture::new("hold", 0);
    f.ready();
    let before = handles();
    let panic = std::panic::catch_unwind(|| {
        let mut observer = Observer::new().unwrap();
        observer.adopt(observation_handle(&f.child)).unwrap();
        panic!("deliberately injected observer unwind");
    });
    assert!(panic.is_err());
    assert_eq!(handles(), before);
    f.alive();
    f.finish();
    let mut observer = Observer::new().unwrap();
    let before = handles();
    assert!(observer.adopt(observation_handle_for_event()).is_err());
    assert_eq!(
        handles(),
        before,
        "failed identity read releases input handle"
    );
}
#[test]
fn i_repeated_exact_handle_recovery_and_identity() {
    let _serial = NATIVE.lock().unwrap();
    // Initialize runtime/compiler/test allocations before measuring.
    let mut warm = Fixture::new("immediate", 0);
    warm.finish();
    drop(warm);
    let before = handles();
    let start = Instant::now();
    for _ in 0..20 {
        let mut f = Fixture::new("immediate", 0);
        let mut observer = Observer::new().unwrap();
        let (_, actual) = observer.adopt(observation_handle(&f.child)).unwrap();
        assert_eq!(actual.pid, f.child.id());
        // Deterministic PID-reuse defense: same PID plus another creation time
        // is a different identity. Real Windows PID reuse is not forced here.
        assert_ne!(
            actual,
            Identity {
                created_100ns: actual.created_100ns + 1,
                ..actual
            }
        );
        exited(&complete(&mut observer), 0);
        f.finish();
    }
    let after = handles();
    eprintln!(
        "M2.1 handles: {before} -> {after} over 20 observations; elapsed {:?}",
        start.elapsed()
    );
    assert_eq!(before, after);
}
#[test]
fn j_two_concurrent_observations_bound_and_idle_wait_cost() {
    let _serial = NATIVE.lock().unwrap();
    let mut first = Fixture::new("hold", 0);
    let mut second = Fixture::new("hold", 0);
    let mut third = Fixture::new("hold", 0);
    first.ready();
    second.ready();
    third.ready();
    let before = handles();
    let mut observer = Observer::new().unwrap();
    observer.adopt(observation_handle(&first.child)).unwrap();
    observer.adopt(observation_handle(&second.child)).unwrap();
    assert_eq!(observer.active(), LIMIT);
    assert_eq!(handles(), before + LIMIT as u32 + 1);
    assert!(observer.adopt(observation_handle(&third.child)).is_err());
    assert_eq!(handles(), before + LIMIT as u32 + 1);
    third.alive();
    let cpu_before = cpu_100ns();
    let start = Instant::now();
    for _ in 0..10 {
        assert!(observer.step(20).is_empty());
    }
    eprintln!(
        "M2.1 idle: {} waits / {:?}; process CPU delta {} x100ns; active {}; handle delta {}",
        observer.waits,
        start.elapsed(),
        cpu_100ns() - cpu_before,
        observer.active(),
        handles() - before
    );
    first.release();
    let result = observer.step(12000);
    assert_eq!(result.len(), 1);
    exited(&result[0], 0);
    assert_eq!(observer.active(), 1);
    let (third_id, _) = observer.adopt(observation_handle(&third.child)).unwrap();
    assert_eq!(observer.active(), LIMIT);
    observer.stop().signal().unwrap();
    let result = observer.step(0);
    assert_eq!(result.len(), 2);
    assert!(result.iter().any(|c| c.id == third_id));
    assert!(result.iter().all(|c| c.reason == Reason::Shutdown));
    second.alive();
    third.alive();
    drop(observer);
    assert_eq!(handles(), before);
    first.finish();
    second.finish();
    third.finish();
}
