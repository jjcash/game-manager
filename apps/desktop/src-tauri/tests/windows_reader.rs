// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(windows)]
#[path = "support/windows_reader.rs"]
mod reader;
use reader::*;
use std::{
    fs,
    sync::{Arc, atomic::Ordering},
};
use std::{
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
    time::Duration,
};
static SERIAL: Mutex<()> = Mutex::new(());
const BODY: &[u8] = b"synthetic metadata";
fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|p| p.into_inner())
}
struct Fixture {
    root: PathBuf,
    outside: PathBuf,
    audit: Arc<Audit>,
    _temp: tempfile::TempDir,
    _serial: MutexGuard<'static, ()>,
}
impl Fixture {
    fn new() -> Self {
        let serial = serial();
        let temp = tempfile::Builder::new()
            .prefix("gm-m13c-")
            .tempdir()
            .unwrap();
        let root = temp.path().join("root");
        let outside = temp.path().join("outside");
        fs::create_dir_all(root.join("steamapps")).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(
            outside.join("sentinel.txt"),
            b"NOT STEAM: SYNTHETIC EXTERNAL SENTINEL",
        )
        .unwrap();
        Self {
            root,
            outside,
            audit: Arc::new(Audit::default()),
            _temp: temp,
            _serial: serial,
        }
    }
    fn leaf(&self) -> PathBuf {
        self.root.join("steamapps/appmanifest_10.acf")
    }
    fn acquire(&self, cancel: Cancel) -> Acquired {
        Acquired::acquire(self.root.to_str().unwrap(), Arc::clone(&self.audit), cancel).unwrap()
    }
    fn approved(&self, cancel: Cancel) -> Approved {
        let acquired = self.acquire(cancel);
        let id = acquired.identity().clone();
        acquired.approve(&id).unwrap()
    }
    fn scan(&self) -> Result<Summary> {
        self.approved(Cancel::default())
            .scan(Limits::default(), |_, b| b == BODY, |_, _| {})
    }
    fn released(&self) {
        assert_eq!(self.audit.handles.load(Ordering::SeqCst), 0);
        assert_eq!(self.audit.workers.load(Ordering::SeqCst), 0);
    }
}
fn junction(path: &Path, target: &Path) {
    // Structured arguments to a fixed script; fixture paths are not shell source.
    let output = std::process::Command::new("powershell.exe").args(["-NoProfile", "-Command",
        "& { param($link, $target) New-Item -ItemType Junction -Path $link -Target $target -ErrorAction Stop | Out-Null }",
        path.to_str().unwrap(), target.to_str().unwrap()]).output().unwrap();
    assert!(
        output.status.success(),
        "junction fixture failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn blocked_native_open_cancellation_gate() {
    let _serial = serial();
    use std::{
        os::windows::{
            fs::OpenOptionsExt,
            io::{AsRawHandle, FromRawHandle, OwnedHandle},
        },
        ptr::{null, null_mut},
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::*,
        Storage::FileSystem::*,
        System::{
            IO::{CancelSynchronousIo, DeviceIoControl, OVERLAPPED},
            Ioctl::FSCTL_REQUEST_OPLOCK_LEVEL_1,
            Threading::{CreateEventW, WaitForSingleObject},
        },
    };
    let temp = tempfile::Builder::new()
        .prefix("gm-m13c-cancel-")
        .tempdir()
        .unwrap();
    let root = temp.path().join("root");
    fs::create_dir_all(root.join("steamapps")).unwrap();
    let leaf = root.join("steamapps/appmanifest_10.acf");
    fs::write(&leaf, b"synthetic metadata").unwrap();
    let owner = fs::OpenOptions::new()
        .access_mode(GENERIC_READ | GENERIC_WRITE)
        .share_mode(7)
        .custom_flags(FILE_FLAG_OVERLAPPED)
        .open(&leaf)
        .unwrap();
    let event_raw = unsafe { CreateEventW(null(), 1, 0, null()) };
    assert!(!event_raw.is_null());
    let event = unsafe { OwnedHandle::from_raw_handle(event_raw) };
    let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
    overlapped.hEvent = event.as_raw_handle();
    let result = unsafe {
        DeviceIoControl(
            owner.as_raw_handle(),
            FSCTL_REQUEST_OPLOCK_LEVEL_1,
            null(),
            0,
            null_mut(),
            0,
            null_mut(),
            &mut overlapped,
        )
    };
    assert_eq!(result, 0);
    assert_eq!(unsafe { GetLastError() }, ERROR_IO_PENDING);
    let audit = Arc::new(Audit::default());
    let worker_audit = Arc::clone(&audit);
    let cancel = Cancel::default();
    let worker_cancel = cancel.clone();
    let worker = std::thread::spawn(move || {
        let acquired = Acquired::acquire(root.to_str().unwrap(), worker_audit, worker_cancel)?;
        let id = acquired.identity().clone();
        let approved = acquired.approve(&id)?;
        approved.inspect_leaf("appmanifest_10.acf", true).map(drop)
    });
    let broke = unsafe { WaitForSingleObject(event.as_raw_handle(), 3000) };
    if broke != WAIT_OBJECT_0 {
        // Fixture teardown always releases the oplock and joins the actual worker.
        drop(owner);
        let outcome = worker.join().unwrap();
        panic!("oplock did not block candidate: wait={broke}, result={outcome:?}");
    }
    assert!(
        !worker.is_finished(),
        "fixture must demonstrate outstanding native I/O"
    );
    cancel.cancel();
    let start = Instant::now();
    let mut attempts = 0;
    let mut accepted = 0;
    while !worker.is_finished() && start.elapsed() < Duration::from_secs(2) {
        attempts += 1;
        accepted += usize::from(unsafe { CancelSynchronousIo(worker.as_raw_handle()) } != 0);
        std::thread::sleep(Duration::from_millis(5));
    }
    let completed_before_release = worker.is_finished();
    // Release only after measuring completion. A failed cancel must not leak the
    // deliberately blocked test worker or its kernel-owned OVERLAPPED storage.
    drop(owner);
    let outcome = worker.join().unwrap();
    eprintln!(
        "blocked NtCreateFile cancellation: completed_before_oplock_release={completed_before_release}, attempts={attempts}, accepted={accepted}, elapsed={:?}, result={outcome:?}, audit={audit:?}",
        start.elapsed()
    );
    assert!(
        completed_before_release,
        "STOP: cancellation did not complete blocked native open"
    );
    assert!(outcome.is_err());
    assert_eq!(audit.content_reads.load(Ordering::SeqCst), 0);
    assert_eq!(audit.handles.load(Ordering::SeqCst), 0);
}

#[test]
fn native_acquisition_gate() {
    let _serial = serial();
    let temp = tempfile::Builder::new()
        .prefix("gm-m13c-")
        .tempdir()
        .unwrap();
    let root = temp.path().join("root");
    fs::create_dir_all(root.join("steamapps")).unwrap();
    fs::write(
        root.join("steamapps/appmanifest_10.acf"),
        b"synthetic metadata",
    )
    .unwrap();
    let audit = Arc::new(Audit::default());
    let cancel = Cancel::default();
    eprintln!("disposable root request={}", root.display());
    let acquired =
        Acquired::acquire(root.to_str().unwrap(), Arc::clone(&audit), cancel.clone()).unwrap();
    let id = acquired.identity().clone();
    assert_eq!(audit.content_open_attempts.load(Ordering::SeqCst), 0);
    let approved = acquired.approve(&id).unwrap();
    let handle = approved.inspect_leaf("appmanifest_10.acf", false).unwrap();
    assert_eq!(stamp(&handle, false).unwrap().size, 18);
    assert!(approved.inspect_leaf("../outside", true).is_err());
    cancel.cancel();
    assert!(matches!(
        approved.inspect_leaf("appmanifest_10.acf", true),
        Err(Error::Cancelled)
    ));
    drop(handle);
    drop(approved);
    assert_eq!(audit.handles.load(Ordering::SeqCst), 0);
    eprintln!("native acquisition and explicit handle-identity approval: {audit:?}");
}

#[test]
fn native_exact_enumeration_and_namespace_gate() {
    let f = Fixture::new();
    fs::write(f.leaf(), BODY).unwrap();
    for name in [
        "common", "workshop", "music", "userdata", "config", "appcache",
    ] {
        fs::create_dir(f.root.join("steamapps").join(name)).unwrap();
        fs::write(
            f.root
                .join("steamapps")
                .join(name)
                .join("appmanifest_20.acf"),
            BODY,
        )
        .unwrap();
    }
    for name in [
        "libraryfolders.vdf",
        "appmanifest_01.acf",
        "appmanifest_0.acf",
        "appmanifest_4294967296.acf",
        "appmanifest_+1.acf",
        "appmanifest_1.acf.bak",
        "appmanifest_１.acf",
        "APPMan~1.ACF",
        "Appmanifest_30.acf",
    ] {
        fs::write(f.root.join("steamapps").join(name), BODY).unwrap();
    }
    fs::write(
        f.root.join("steamapps/libraryfolders.vdf"),
        format!("unapproved {}", f.outside.display()),
    )
    .unwrap();
    // An ADS is never enumerated as a leaf or opened; only the default stream is eligible.
    fs::write(
        format!("{}:secret", f.leaf().display()),
        b"SYNTHETIC ADS SENTINEL",
    )
    .unwrap();
    let s = f.scan().unwrap();
    assert_eq!(s.accepted, 1);
    let names = f.audit.names.lock().unwrap();
    for (name, values) in names.iter().filter(|(_, n)| n[1] > 0 || n[3] > 0) {
        assert_eq!(name, "appmanifest_10.acf");
        assert!(values[1] <= 4);
    }
    for denied in [
        "common",
        "workshop",
        "music",
        "userdata",
        "config",
        "appcache",
        "libraryfolders.vdf",
        "sentinel.txt",
        "APPMan~1.ACF",
    ] {
        assert!(
            !names.contains_key(denied),
            "unauthorized native open: {denied}"
        );
    }
    assert_eq!(f.audit.bytes.load(Ordering::SeqCst), BODY.len() * 2);
    drop(names);
    f.released();
}

#[test]
fn rejected_requests_never_reach_native_leaf_open() {
    let f = Fixture::new();
    let approved = f.approved(Cancel::default());
    let before = f.audit.attribute_attempts.load(Ordering::SeqCst);
    for name in [
        "../appmanifest_10.acf",
        "..\\appmanifest_10.acf",
        "C:\\appmanifest_10.acf",
        "\\\\server\\share",
        "appmanifest_10.acf:secret",
        "appmanifest_10.acf.",
        "appmanifest_10.acf ",
        "appmanifest_0001.acf",
        "APPManifest_10.acf",
        "appmanifest_1０.acf",
        "appmanifest_4294967296.acf",
        "appmanifest_-1.acf",
    ] {
        assert!(
            matches!(approved.inspect_leaf(name, true), Err(Error::Policy)),
            "{name}"
        );
    }
    assert_eq!(f.audit.attribute_attempts.load(Ordering::SeqCst), before);
    assert_eq!(f.audit.content_open_attempts.load(Ordering::SeqCst), 0);
    drop(approved);
    for path in [
        "..\\root",
        "C:root",
        "\\\\server\\share\\root",
        "\\\\?\\C:\\root",
        "\\\\.\\PhysicalDrive0",
        "C:\\root\\..\\other",
        "C:\\root:stream",
        "C:/root",
        "C:\\root\\",
    ] {
        assert!(Acquired::acquire(path, Arc::clone(&f.audit), Cancel::default()).is_err());
    }
    f.released();
}

#[test]
fn junction_ancestor_root_directory_and_leaf_fail_closed() {
    let f = Fixture::new();
    let linked = f._temp.path().join("linked");
    junction(&linked, &f.root);
    for path in [linked.clone(), linked.join("steamapps")] {
        assert!(
            Acquired::acquire(
                path.to_str().unwrap(),
                Arc::clone(&f.audit),
                Cancel::default()
            )
            .is_err()
        );
    }
    fs::remove_dir(&linked).unwrap();
    let apps = f.root.join("steamapps");
    fs::remove_dir(&apps).unwrap();
    junction(&apps, &f.outside);
    let a = f.acquire(Cancel::default());
    let id = a.identity().clone();
    assert!(a.approve(&id).is_err());
    fs::remove_dir(&apps).unwrap();
    fs::create_dir(&apps).unwrap();
    junction(&f.leaf(), &f.outside);
    let s = f.scan().unwrap();
    assert_eq!(s.accepted, 0);
    assert_eq!(s.warnings, 1);
    assert_eq!(f.audit.content_reads.load(Ordering::SeqCst), 0);
    fs::remove_dir(f.leaf()).unwrap();
    f.released();
}

#[test]
fn hardlink_sentinels_locked_and_unlocked_are_never_read() {
    use std::os::windows::fs::OpenOptionsExt;
    let f = Fixture::new();
    fs::hard_link(f.outside.join("sentinel.txt"), f.leaf()).unwrap();
    for locked in [false, true] {
        let owner = locked.then(|| {
            fs::OpenOptions::new()
                .read(true)
                .write(true)
                .share_mode(0)
                .open(f.outside.join("sentinel.txt"))
                .unwrap()
        });
        let before = f.audit.content_open_attempts.load(Ordering::SeqCst);
        let s = f.scan().unwrap();
        assert_eq!(s.accepted, 0);
        assert_eq!(s.warnings, 1);
        assert_eq!(f.audit.content_reads.load(Ordering::SeqCst), 0);
        assert_eq!(f.audit.content_open_attempts.load(Ordering::SeqCst), before);
        eprintln!(
            "hardlink sentinel locked={locked}: attempts/success/reads={:?}",
            f.audit.names.lock().unwrap().get("appmanifest_10.acf")
        );
        drop(owner);
        f.released();
    }
}

#[test]
fn approval_rejection_and_root_replacement_invalidate_capabilities() {
    let f = Fixture::new();
    let cancel = Cancel::default();
    let a = f.acquire(cancel.clone());
    let id = a.identity().clone();
    assert!(matches!(
        Acquired::acquire(
            f.root.to_str().unwrap(),
            Arc::clone(&f.audit),
            Cancel::default()
        ),
        Err(Error::Busy)
    ));
    cancel.cancel();
    assert!(matches!(a.approve(&id), Err(Error::Cancelled)));
    f.released();
    let a = f.acquire(Cancel::default());
    let mut wrong = a.identity().clone();
    wrong.file[0] ^= 1;
    assert!(matches!(a.approve(&wrong), Err(Error::Policy)));
    f.released();
    for after in [false, true] {
        let a = f.acquire(Cancel::default());
        let id = a.identity().clone();
        let (a, approved) = if after {
            (None, Some(a.approve(&id).unwrap()))
        } else {
            (Some(a), None)
        };
        // Same-parent rename models replacement; cross-parent directory moves with
        // retained child handles are denied by Windows (retained first-run evidence).
        let saved = f._temp.path().join("saved-root");
        eprintln!("root replacement fixture: after_approval={after}");
        fs::rename(&f.root, &saved).unwrap();
        fs::create_dir_all(f.root.join("steamapps")).unwrap();
        if let Some(a) = a {
            assert!(matches!(a.approve(&id), Err(Error::Changed)));
        }
        if let Some(approved) = approved {
            assert!(matches!(approved.revalidate(), Err(Error::Changed)));
            assert!(matches!(approved.revalidate(), Err(Error::Cancelled)));
        }
        fs::remove_dir(f.root.join("steamapps")).unwrap();
        fs::remove_dir(&f.root).unwrap();
        fs::rename(saved, &f.root).unwrap();
        f.released();
    }
}

#[test]
fn steamapps_replacement_during_enumeration_invalidates_session() {
    let f = Fixture::new();
    fs::write(f.leaf(), BODY).unwrap();
    let mut replaced = false;
    let s = f.approved(Cancel::default()).scan(
        Limits::default(),
        |_, _| true,
        |stage, _| {
            if stage == Stage::EnumerationBatch && !replaced {
                replaced = true;
                fs::rename(f.root.join("steamapps"), f.outside.join("saved-apps")).unwrap();
                junction(&f.root.join("steamapps"), &f.outside);
            }
        },
    );
    assert!(s.is_err());
    assert_eq!(f.audit.content_reads.load(Ordering::SeqCst), 0);
    fs::remove_dir(f.root.join("steamapps")).unwrap();
    f.released();
}

#[test]
fn observed_link_count_change_before_read_is_rejected() {
    let f = Fixture::new();
    fs::write(f.leaf(), BODY).unwrap();
    let mut changed = false;
    let s = f
        .approved(Cancel::default())
        .scan(
            Limits::default(),
            |_, _| true,
            |stage, _| {
                if stage == Stage::BeforeRead && !changed {
                    changed = true;
                    fs::hard_link(f.leaf(), f.outside.join("second-name")).unwrap();
                }
            },
        )
        .unwrap();
    assert_eq!(s.warnings, 1);
    assert_eq!(s.accepted, 0);
    assert_eq!(f.audit.content_open_attempts.load(Ordering::SeqCst), 1);
    assert_eq!(f.audit.content_reads.load(Ordering::SeqCst), 0);
    f.released();
    fs::remove_file(f.outside.join("second-name")).unwrap();
    assert_eq!(f.scan().unwrap().accepted, 1);
    f.released();
}

#[test]
#[ignore = "symlink fixture requires Developer Mode or SeCreateSymbolicLinkPrivilege; execute separately"]
fn symbolic_links_at_ancestor_root_directory_and_leaf() {
    use std::os::windows::fs::{symlink_dir, symlink_file};
    let f = Fixture::new();
    let link = f._temp.path().join("symbolic");
    symlink_dir(&f.root, &link).expect("UNAVAILABLE symlink fixture; this is not a security pass");
    for path in [link.clone(), link.join("steamapps")] {
        assert!(
            Acquired::acquire(
                path.to_str().unwrap(),
                Arc::clone(&f.audit),
                Cancel::default()
            )
            .is_err()
        );
    }
    fs::remove_dir(link).unwrap();
    fs::remove_dir(f.root.join("steamapps")).unwrap();
    symlink_dir(&f.outside, f.root.join("steamapps")).unwrap();
    let a = f.acquire(Cancel::default());
    let id = a.identity().clone();
    assert!(a.approve(&id).is_err());
    fs::remove_dir(f.root.join("steamapps")).unwrap();
    fs::create_dir(f.root.join("steamapps")).unwrap();
    symlink_file(f.outside.join("sentinel.txt"), f.leaf()).unwrap();
    assert_eq!(f.scan().unwrap().accepted, 0);
    assert_eq!(f.audit.content_reads.load(Ordering::SeqCst), 0);
    f.released();
}

#[test]
fn file_replacement_and_mutation_are_incomplete_bounded_observations() {
    let f = Fixture::new();
    for stage in [
        Stage::Enumerated,
        Stage::AfterFirstRead,
        Stage::AfterSecondRead,
        Stage::BeforeRead,
    ] {
        fs::write(f.leaf(), BODY).unwrap();
        let mut changed = false;
        let s = f
            .approved(Cancel::default())
            .scan(
                Limits::default(),
                |_, b| b == BODY,
                |s, _| {
                    if s == stage && !changed {
                        changed = true;
                        if stage == Stage::BeforeRead {
                            fs::write(f.leaf(), vec![b'x'; 65537]).unwrap();
                        } else {
                            fs::rename(f.leaf(), f.outside.join("old.acf")).unwrap();
                            fs::write(f.leaf(), BODY).unwrap();
                        }
                    }
                },
            )
            .unwrap();
        assert_eq!(s.accepted, 0, "stage {stage:?}");
        assert_eq!(s.warnings, 1);
        assert!(s.retries <= 1);
        f.released();
        let _ = fs::remove_file(f.outside.join("old.acf"));
    }
    // An ordinary in-place rewrite may recover with the one permitted retry.
    fs::write(f.leaf(), BODY).unwrap();
    let mut changed = false;
    let s = f
        .approved(Cancel::default())
        .scan(
            Limits::default(),
            |_, b| b == b"updated",
            |stage, _| {
                if stage == Stage::AfterFirstRead && !changed {
                    changed = true;
                    fs::write(f.leaf(), b"updated").unwrap();
                }
            },
        )
        .unwrap();
    assert_eq!(s.accepted, 1);
    assert_eq!(s.retries, 1);
    f.released();
}

#[test]
fn limits_missing_special_oversized_and_failure_recovery() {
    let f = Fixture::new();
    assert_eq!(f.scan().unwrap().accepted, 0);
    fs::create_dir(f.leaf()).unwrap();
    assert_eq!(f.scan().unwrap().warnings, 1);
    fs::remove_dir(f.leaf()).unwrap();
    fs::write(f.leaf(), vec![b'x'; 65537]).unwrap();
    assert_eq!(f.scan().unwrap().warnings, 1);
    assert_eq!(f.audit.content_reads.load(Ordering::SeqCst), 0);
    fs::write(f.leaf(), BODY).unwrap();
    for limits in [
        Limits {
            bytes: 10,
            ..Limits::default()
        },
        Limits {
            records: 0,
            ..Limits::default()
        },
        Limits {
            elapsed: Duration::ZERO,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            f.approved(Cancel::default())
                .scan(limits, |_, _| true, |_, _| {}),
            Err(Error::Limit)
        ));
        f.released();
    }
    let s = f
        .approved(Cancel::default())
        .scan(
            Limits::default(),
            |_, _| true,
            |stage, _| {
                if stage == Stage::Enumerated {
                    fs::remove_file(f.leaf()).unwrap();
                }
            },
        )
        .unwrap();
    assert_eq!(s.warnings, 1);
    f.released();
    for i in 0..1000 {
        fs::write(f.root.join("steamapps").join(format!("unrelated-{i}")), []).unwrap();
    }
    assert!(matches!(
        f.approved(Cancel::default()).scan(
            Limits {
                entries: 100,
                ..Limits::default()
            },
            |_, _| true,
            |_, _| {}
        ),
        Err(Error::Limit)
    ));
    f.released();
    fs::write(f.leaf(), BODY).unwrap();
    assert_eq!(f.scan().unwrap().accepted, 1);
    f.released();
}

#[test]
fn repeated_cancellation_workers_and_drop_release_native_resources() {
    let f = Fixture::new();
    fs::write(f.leaf(), BODY).unwrap();
    for stage in [
        Stage::EnumerationBatch,
        Stage::Enumerated,
        Stage::BeforeOpen,
        Stage::BeforeRead,
        Stage::AfterFirstRead,
        Stage::AfterSecondRead,
    ] {
        let cancel = Cancel::default();
        let s = f.approved(cancel.clone()).scan(
            Limits::default(),
            |_, _| true,
            |s, _| {
                if s == stage {
                    cancel.cancel();
                }
            },
        );
        assert!(matches!(s, Err(Error::Cancelled)));
        f.released();
    }
    for _ in 0..20 {
        let path = f.root.clone();
        let audit = Arc::clone(&f.audit);
        let worker = Worker::start(Arc::clone(&f.audit), move |cancel| {
            let a = Acquired::acquire(path.to_str().unwrap(), audit, cancel)?;
            let id = a.identity().clone();
            a.approve(&id)?
                .scan(Limits::default(), |_, _| true, |_, _| {})
        })
        .unwrap();
        assert!(matches!(
            Worker::start(Arc::clone(&f.audit), |_| ()),
            Err(Error::Busy)
        ));
        worker.cancel();
        let _ = worker.finish();
        f.released();
    }
    let path = f.root.clone();
    let audit = Arc::clone(&f.audit);
    let worker = Worker::start(Arc::clone(&f.audit), move |cancel| {
        let a = Acquired::acquire(path.to_str().unwrap(), audit, cancel)?;
        let id = a.identity().clone();
        a.approve(&id)?
            .scan(Limits::default(), |_, _| true, |_, _| {})
    })
    .unwrap();
    drop(worker);
    f.released();
    assert_eq!(f.audit.peak_workers.load(Ordering::SeqCst), 1);
}
