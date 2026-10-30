// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(windows)]
//! Complete native production-path gate; never run elevated on hosted CI.
use game_manager_lib::{
    launch::{Created, Request},
    storage::{
        Database,
        launch_profiles::{Action as ProfileAction, Configuration, PathRef},
        manual::{self, Action, MutationRequest},
    },
};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};
use tempfile::{TempDir, tempdir};
use windows_sys::Win32::{
    Foundation::{HANDLE_FLAG_INHERIT, SetHandleInformation, WAIT_TIMEOUT},
    System::Threading::{
        CreateEventW, GetCurrentProcess, GetProcessHandleCount, WaitForSingleObject,
    },
};
static NATIVE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
struct Fixture {
    _serial: tokio::sync::MutexGuard<'static, ()>,
    root: TempDir,
    db: Database,
    game: String,
    exe: PathBuf,
    configuration: Configuration,
}
fn compile(exe: &Path, manifest: Option<&Path>) {
    let mut build = Command::new("rustc");
    build
        .args(["--edition=2024", "-Dwarnings"])
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/standalone/child.rs"))
        .arg("-o")
        .arg(exe);
    if let Some(manifest) = manifest {
        build
            .args([
                "-C",
                "link-arg=/MANIFEST:EMBED",
                "-C",
                "link-arg=/MANIFESTUAC:NO",
                "-C",
            ])
            .arg(format!("link-arg=/MANIFESTINPUT:{}", manifest.display()));
    }
    let output = build.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn control(directory: &Path, exit: i32, milliseconds: u32) {
    fs::create_dir_all(directory).unwrap();
    fs::write(
        directory.join("fixture-control.txt"),
        format!("{exit} {milliseconds}"),
    )
    .unwrap();
}

impl Fixture {
    async fn new() -> Self {
        let serial = NATIVE.lock().await;
        let principal = Command::new("whoami").arg("/groups").output().unwrap();
        assert!(
            String::from_utf8_lossy(&principal.stdout).contains("S-1-16-8192"),
            "ordinary Medium integrity required"
        );
        let root = if let Some(parent) = std::env::var_os("M18_PARENT_FIXTURE_ROOT") {
            tempfile::Builder::new()
                .prefix("owned-child-")
                .tempdir_in(parent)
                .unwrap()
        } else {
            tempdir().unwrap()
        };
        let dir = root.path().join("bin space 日本語");
        control(&dir, 0, 0);
        let exe = dir.join("fixture target.exe");
        compile(&exe, None);
        let db = Database::initialize(&root.path().join("library.sqlite3"))
            .await
            .unwrap();
        let game = db
            .manual_create(&manual::CreateRequest {
                metadata: manual::Metadata {
                    title: "Synthetic only".into(),
                    release_date: None,
                    genre: None,
                    developer: None,
                    favorite: false,
                },
                source_name: "Manual".into(),
                installation: Some(manual::InstallationInput {
                    root_path_ref: root.path().to_str().unwrap().into(),
                    platform: "windows".into(),
                    version: None,
                    detection_state: "unknown".into(),
                }),
            })
            .await
            .unwrap();
        Self {
            _serial: serial,
            root,
            db,
            game,
            exe,
            configuration: Configuration {
                display_name: "Synthetic".into(),
                launcher_kind: "standalone".into(),
                executable_ref: PathRef::InstallationRelative(
                    "bin space 日本語\\fixture target.exe".into(),
                ),
                arguments: vec![],
                working_directory_ref: None,
            },
        }
    }
    async fn add(&self) -> Request {
        let d = self.db.manual_details(&self.game).await.unwrap();
        self.db
            .manual_mutate(&MutationRequest {
                game_id: self.game.clone(),
                expected_revision: d.revision,
                action: Action::LaunchProfile {
                    action: ProfileAction::Create {
                        installation_id: d.installations[0].id.clone(),
                        configuration: self.configuration.clone(),
                    },
                },
            })
            .await
            .unwrap();
        let d = self.db.manual_details(&self.game).await.unwrap();
        let p = d
            .profiles
            .iter()
            .find(|p| !d.profiles.is_empty() && p.configuration == self.configuration)
            .unwrap();
        Request {
            game_id: self.game.clone(),
            profile_id: p.id.clone(),
            expected_profile_revision: p.revision,
            expected_game_revision: d.revision,
        }
    }
    async fn close(self) {
        self.db.close().await;
    }
}
fn report(dir: &Path) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(6);
    while !dir.join("exited.txt").exists() {
        assert!(Instant::now() < deadline, "fixture timeout");
        std::thread::sleep(Duration::from_millis(10));
    }
    let bytes = fs::read(dir.join("received.bin")).unwrap();
    let mut rest = bytes.as_slice();
    let mut values = vec![];
    while !rest.is_empty() {
        let n = u32::from_le_bytes(rest[..4].try_into().unwrap()) as usize;
        rest = &rest[4..];
        values.push(String::from_utf8(rest[..n].to_vec()).unwrap());
        rest = &rest[n..];
    }
    assert!(!dir.join("watchdog.txt").exists());
    values
}
fn verify(created: Created, dir: &Path, args: &[String]) {
    let received = report(dir);
    assert_eq!(received[0], created.pid.to_string());
    assert_eq!(received[1], dir.to_str().unwrap());
    assert_eq!(received[3], std::env::var("SystemRoot").unwrap());
    assert_eq!(&received[5..], args);
    assert_eq!(received[4], "noInput");
}
#[tokio::test]
async fn production_literal_unicode_arguments_directory_and_handle_allowlist() {
    let mut f = Fixture::new().await;
    let event = unsafe {
        OwnedHandle::from_raw_handle(CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()))
    };
    assert_ne!(
        unsafe {
            SetHandleInformation(
                event.as_raw_handle(),
                HANDLE_FLAG_INHERIT,
                HANDLE_FLAG_INHERIT,
            )
        },
        0
    );
    let dir = f.root.path().join("work 日本語");
    control(&dir, 0, 0);
    fs::write(
        dir.join("fixture-control.txt"),
        format!("0 0 {}", event.as_raw_handle() as usize),
    )
    .unwrap();
    f.configuration.working_directory_ref =
        Some(PathRef::AbsolutePath(dir.to_str().unwrap().into()));
    f.configuration.arguments = vec![
        "".into(),
        "two words".into(),
        "日本語 🎮".into(),
        "a\\\"b".into(),
        "trailing\\".into(),
        "& | > %PATH% $(echo no) ;".into(),
    ];
    let r = f.add().await;
    let created = f.db.launch_standalone(r).await.unwrap();
    verify(created, &dir, &f.configuration.arguments);
    assert_eq!(
        unsafe { WaitForSingleObject(event.as_raw_handle(), 0) },
        WAIT_TIMEOUT
    );
    f.close().await;
}
#[tokio::test]
async fn production_missing_executable_and_working_directory_release_admission() {
    let mut f = Fixture::new().await;
    f.configuration.executable_ref =
        PathRef::AbsolutePath(f.root.path().join("missing.exe").to_str().unwrap().into());
    let r = f.add().await;
    assert_eq!(
        f.db.launch_standalone(r).await.unwrap_err().code,
        "missingExecutable"
    );
    f.configuration.executable_ref = PathRef::AbsolutePath(f.exe.to_str().unwrap().into());
    f.configuration.working_directory_ref =
        Some(PathRef::InstallationRelative("missing-directory".into()));
    let r = f.add().await;
    assert_eq!(
        f.db.launch_standalone(r).await.unwrap_err().code,
        "invalidWorkingDirectory"
    );
    f.configuration.working_directory_ref = None;
    let r = f.add().await;
    verify(
        f.db.launch_standalone(r).await.unwrap(),
        f.exe.parent().unwrap(),
        &[],
    );
    f.close().await;
}
#[tokio::test]
async fn production_native_failure_no_retry_and_writer_recovery() {
    let mut f = Fixture::new().await;
    let bad = f.root.path().join("invalid.exe");
    fs::write(&bad, "harmless invalid PE").unwrap();
    f.configuration.executable_ref = PathRef::AbsolutePath(bad.to_str().unwrap().into());
    let r = f.add().await;
    let e = f.db.launch_standalone(r).await.unwrap_err();
    assert_eq!(e.stage, "creation");
    assert!(matches!(e.os_code, Some(193 | 216)));
    // A fresh write and subsequent deliberate valid request prove reservation/slot recovery.
    f.configuration.executable_ref = PathRef::AbsolutePath(f.exe.to_str().unwrap().into());
    let r = f.add().await;
    verify(
        f.db.launch_standalone(r).await.unwrap(),
        f.exe.parent().unwrap(),
        &[],
    );
    f.close().await;
}
#[tokio::test]
async fn production_stale_profile_game_wrong_identity_and_closed_services() {
    let f = Fixture::new().await;
    let r = f.add().await;
    for i in 0..5 {
        let mut bad = r.clone();
        match i {
            0 => bad.expected_game_revision += 1,
            1 => bad.expected_profile_revision += 1,
            2 => bad.game_id = "f".repeat(32),
            3 => bad.profile_id = "f".repeat(32),
            _ => bad.game_id = "bad".into(),
        };
        assert!(f.db.launch_standalone(bad).await.is_err());
    }
    assert!(!f.exe.parent().unwrap().join("received.bin").exists());
    f.db.close().await;
    assert_eq!(
        f.db.launch_standalone(r).await.unwrap_err().code,
        "stopping"
    );
    f.close().await;
}
#[tokio::test]
async fn production_child_survives_handle_release_database_shutdown_and_lost_reporting() {
    let f = Fixture::new().await;
    control(f.exe.parent().unwrap(), 17, 500);
    let r = f.add().await;
    // Discard successful delivery exactly as a lost IPC response; no retry path.
    let pid = f.db.launch_standalone(r).await.unwrap().pid;
    f.db.close().await;
    let values = report(f.exe.parent().unwrap());
    assert_eq!(values[0], pid.to_string());
    assert_eq!(
        fs::read_to_string(f.exe.parent().unwrap().join("exited.txt")).unwrap(),
        "17"
    );
    f.close().await;
}
#[tokio::test]
async fn production_handle_recovery_over_twenty_deliberate_launches() {
    let f = Fixture::new().await;
    let r = f.add().await;
    // Warm pool/native/Tokio allocations before observing stable native handles.
    verify(
        f.db.launch_standalone(r.clone()).await.unwrap(),
        f.exe.parent().unwrap(),
        &[],
    );
    let handles = || {
        let mut n = 0;
        assert_ne!(
            unsafe { GetProcessHandleCount(GetCurrentProcess(), &mut n) },
            0
        );
        n
    };
    let before = handles();
    for _ in 0..20 {
        fs::remove_file(f.exe.parent().unwrap().join("exited.txt")).unwrap();
        verify(
            f.db.launch_standalone(r.clone()).await.unwrap(),
            f.exe.parent().unwrap(),
            &[],
        );
    }
    let after = handles();
    eprintln!("M1.8 production handles: {before} -> {after} over 20 launches");
    assert_eq!(before, after);
    f.close().await;
}
#[tokio::test]
async fn production_elevation_required_target_is_refused() {
    let mut f = Fixture::new().await;
    let manifest = f.root.path().join("requires-admin.manifest");
    fs::write(&manifest,r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0"><trustInfo xmlns="urn:schemas-microsoft-com:asm.v3"><security><requestedPrivileges><requestedExecutionLevel level="requireAdministrator" uiAccess="false"/></requestedPrivileges></security></trustInfo></assembly>"#).unwrap();
    let exe = f.root.path().join("requires-admin.exe");
    compile(&exe, Some(&manifest));
    f.configuration.executable_ref = PathRef::AbsolutePath(exe.to_str().unwrap().into());
    let r = f.add().await;
    let e = f.db.launch_standalone(r).await.unwrap_err();
    assert_eq!(e.stage, "creation");
    assert_eq!(e.os_code, Some(740));
    f.close().await;
}

#[tokio::test]
async fn production_writer_collision_fails_without_creation_and_recovers() {
    use sqlx::{Connection, SqliteConnection};
    let f = Fixture::new().await;
    let r = f.add().await;
    let mut c = SqliteConnection::connect(f.root.path().join("library.sqlite3").to_str().unwrap())
        .await
        .unwrap();
    let tx = c.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let result = tokio::time::timeout(Duration::from_secs(10), f.db.launch_standalone(r.clone()))
        .await
        .unwrap();
    assert_eq!(result.unwrap_err().code, "databaseUnavailable");
    assert!(!f.exe.parent().unwrap().join("received.bin").exists());
    tx.rollback().await.unwrap();
    c.close().await.unwrap();
    verify(
        f.db.launch_standalone(r).await.unwrap(),
        f.exe.parent().unwrap(),
        &[],
    );
    f.close().await;
}
#[tokio::test]
async fn production_child_survives_real_parent_exit() {
    if let Some(parent) = std::env::var_os("M18_PARENT_FIXTURE_ROOT") {
        let f = Fixture::new().await;
        control(f.exe.parent().unwrap(), 0, 700);
        let created = f.db.launch_standalone(f.add().await).await.unwrap();
        fs::write(
            Path::new(&parent).join("receipt.json"),
            serde_json::to_vec(
                &serde_json::json!({"pid":created.pid,"directory":f.exe.parent().unwrap()}),
            )
            .unwrap(),
        )
        .unwrap();
        std::process::exit(0);
    }
    let _serial = NATIVE.lock().await;
    let root = tempdir().unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "production_child_survives_real_parent_exit",
            "--nocapture",
        ])
        .env("M18_PARENT_FIXTURE_ROOT", root.path())
        .status()
        .unwrap();
    assert!(status.success());
    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(root.path().join("receipt.json")).unwrap()).unwrap();
    let dir = Path::new(receipt["directory"].as_str().unwrap());
    assert!(dir.starts_with(root.path()));
    assert_eq!(report(dir)[0], receipt["pid"].as_u64().unwrap().to_string());
}
