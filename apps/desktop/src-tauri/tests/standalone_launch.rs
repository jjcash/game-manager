// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(windows)]

#[path = "support/standalone_launch.rs"]
mod candidate;

use candidate::{Admission::*, Authorization::*, *};
use game_manager_lib::storage::{
    Database,
    launch_profiles::{Action as ProfileAction, Configuration, PathRef},
    manual::{self, Action, MutationRequest},
};
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
use std::{
    fs,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};
use tempfile::{TempDir, tempdir};
use windows_sys::Win32::{
    Foundation::{HANDLE_FLAG_INHERIT, SetHandleInformation, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{
        CreateEventW, GetCurrentProcess, GetExitCodeProcess, GetProcessHandleCount,
        WaitForSingleObject,
    },
};

// Native resource observations must not race this test binary's other fixtures.
static NATIVE_CASE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct Fixture {
    _serial: tokio::sync::MutexGuard<'static, ()>,
    _root: TempDir,
    db: Database,
    connection: SqliteConnection,
    game: String,
    installation: String,
    entry: String,
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
        Self::with_root(tempdir().unwrap()).await
    }
    async fn with_root(root: TempDir) -> Self {
        let serial = NATIVE_CASE.lock().await;
        unelevated().expect("Run M1.7 from an ordinary unelevated desktop session");
        let directory = root.path().join("bin space 日本語");
        control(&directory, 0, 0);
        let exe = directory.join("fixture target.exe");
        compile(&exe, None);
        let database = root.path().join("library.sqlite3");
        let db = Database::initialize(&database).await.unwrap();
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
        let details = db.manual_details(&game).await.unwrap();
        let installation = details.installations[0].id.clone();
        let entry = details.entries[0].id.clone();
        let connection = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(&database)
                .foreign_keys(true),
        )
        .await
        .unwrap();
        Self {
            _serial: serial,
            _root: root,
            db,
            connection,
            game,
            installation,
            entry,
            exe,
            configuration: Configuration {
                display_name: "Same name".into(),
                launcher_kind: "standalone".into(),
                executable_ref: PathRef::InstallationRelative(
                    "bin space 日本語\\fixture target.exe".into(),
                ),
                arguments: vec![],
                working_directory_ref: None,
            },
        }
    }
    async fn action(&self, action: Action) {
        let revision = self.db.manual_details(&self.game).await.unwrap().revision;
        self.db
            .manual_mutate(&MutationRequest {
                game_id: self.game.clone(),
                expected_revision: revision,
                action,
            })
            .await
            .unwrap();
    }
    async fn add(&self) -> Request {
        let before = self
            .db
            .manual_details(&self.game)
            .await
            .unwrap()
            .profiles
            .into_iter()
            .map(|p| p.id)
            .collect::<Vec<_>>();
        self.action(Action::LaunchProfile {
            action: ProfileAction::Create {
                installation_id: self.installation.clone(),
                configuration: self.configuration.clone(),
            },
        })
        .await;
        let details = self.db.manual_details(&self.game).await.unwrap();
        let profile = details
            .profiles
            .iter()
            .find(|p| !before.contains(&p.id))
            .unwrap();
        Request {
            game_id: self.game.clone(),
            profile_id: profile.id.clone(),
            expected_profile_revision: profile.revision,
            expected_game_revision: details.revision,
        }
    }
    async fn prepared(&mut self, request: Request) -> Prepared {
        prepare(&mut self.connection, request, Ready).await.unwrap()
    }
    async fn run(&mut self, request: Request) -> Created {
        let p = self.prepared(request).await;
        execute(&mut self.connection, p, ExplicitUserAttempt, Ready)
            .await
            .unwrap()
    }
    async fn close(self) {
        self.connection.close().await.unwrap();
        self.db.close().await;
    }
}
fn wait(created: Created) -> (u32, u32) {
    let pid = created.pid;
    // SAFETY: owned live process handle; fixture lifetime itself is <=3 seconds.
    assert_eq!(
        unsafe { WaitForSingleObject(created.handle.as_raw_handle(), 6000) },
        WAIT_OBJECT_0
    );
    let mut code = 0;
    assert_ne!(
        unsafe { GetExitCodeProcess(created.handle.as_raw_handle(), &mut code) },
        0
    );
    (pid, code)
}
fn report(directory: &Path) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !directory.join("exited.txt").exists() {
        assert!(
            Instant::now() < deadline,
            "synthetic child did not exit in its bound"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let bytes = fs::read(directory.join("received.bin")).unwrap();
    let mut rest = bytes.as_slice();
    let mut values = Vec::new();
    while !rest.is_empty() {
        let n = u32::from_le_bytes(rest[..4].try_into().unwrap()) as usize;
        values.push(String::from_utf8(rest[4..4 + n].to_vec()).unwrap());
        rest = &rest[4 + n..];
    }
    values
}
fn failure<T>(result: Result<T, Failure>, stage: &str, code: &str) {
    let err = result.err().expect("must fail closed");
    assert_eq!((err.stage, err.code), (stage, code));
    assert!(!format!("{err:?}").contains("Synthetic only"));
}

#[tokio::test]
async fn native_literal_arguments_and_deterministic_default_directory() {
    let mut f = Fixture::new().await;
    let mut combinations = vec![
        vec![],
        vec!["one".into()],
        vec!["first".into(), "second".into()],
        vec![
            "".into(),
            "two words".into(),
            "\"embedded quotes\"".into(),
            "a\\\\\"b".into(),
            "trailing\\\\".into(),
            " \\".into(),
            "日本語 café 🎮".into(),
            "\t\n".into(),
            "& | > < ^ %TEMP% !name! $(no) ; `no` * ?".into(),
            "& echo unsafe > shell-sentinel.txt".into(),
        ],
        vec![String::new(); 128],
        vec!["x".repeat(4096); 4],
    ];
    let mut quoted = Vec::new();
    for slashes in 0..16 {
        quoted.push(format!("before{}\"after", "\\".repeat(slashes)));
        quoted.push(format!(" {}", "\\".repeat(slashes)));
        quoted.push(format!("{}\"", "\\".repeat(slashes)));
    }
    combinations.push(quoted);
    for arguments in combinations {
        f.configuration.arguments = arguments.clone();
        let r = f.add().await;
        let (pid, code) = wait(f.run(r).await);
        assert_eq!(code, 0);
        let received = report(f.exe.parent().unwrap());
        assert_eq!(received[0], pid.to_string());
        assert_eq!(Path::new(&received[1]), f.exe.parent().unwrap());
        assert_eq!(Path::new(&received[2]), f.exe);
        assert_eq!(received[3], std::env::var("SystemRoot").unwrap());
        assert_eq!(received[4], "noInput");
        assert_eq!(&received[5..], arguments);
        assert!(!f.exe.parent().unwrap().join("shell-sentinel.txt").exists());
    }
    f.close().await;
}

#[tokio::test]
async fn absolute_relative_directories_location_change_and_no_rebasing() {
    let mut f = Fixture::new().await;
    for directory_ref in [
        PathRef::InstallationRelative("work space 日本語".into()),
        PathRef::AbsolutePath(
            f._root
                .path()
                .join("work space 日本語")
                .to_str()
                .unwrap()
                .into(),
        ),
    ] {
        let wd = f._root.path().join("work space 日本語");
        control(&wd, 17, 0);
        f.configuration.working_directory_ref = Some(directory_ref);
        let r = f.add().await;
        assert_eq!(wait(f.run(r).await).1, 17);
        assert_eq!(Path::new(&report(&wd)[1]), wd);
    }
    f.configuration.working_directory_ref = None;
    let request = f.add().await;
    let prepared = f.prepared(request.clone()).await;
    let relocated = f._root.path().join("other root");
    let bin = relocated.join("bin space 日本語");
    control(&bin, 23, 0);
    fs::copy(&f.exe, bin.join("fixture target.exe")).unwrap();
    f.action(Action::SaveInstallation {
        entry_id: f.entry.clone(),
        installation_id: Some(f.installation.clone()),
        installation: manual::InstallationInput {
            root_path_ref: relocated.to_str().unwrap().into(),
            platform: "windows".into(),
            version: Some("changed".into()),
            detection_state: "present".into(),
        },
    })
    .await;
    failure(
        execute(&mut f.connection, prepared, ExplicitUserAttempt, Ready).await,
        "request",
        "staleContext",
    );
    let mut fresh = request;
    fresh.expected_game_revision = f.db.manual_details(&f.game).await.unwrap().revision;
    assert_eq!(wait(f.run(fresh).await).1, 23);
    // Same saved relative value, new recorded root. No rewriting persisted refs.
    assert_eq!(
        f.db.manual_details(&f.game).await.unwrap().profiles[0]
            .configuration
            .executable_ref,
        f.configuration.executable_ref
    );
    f.configuration.executable_ref = PathRef::AbsolutePath(f.exe.to_str().unwrap().into());
    let r = f.add().await;
    assert_eq!(wait(f.run(r).await).1, 0); // absolute target ignores new installation root
    f.close().await;
}

#[tokio::test]
async fn malformed_paths_unsupported_kinds_and_arguments_fail_before_execution() {
    let mut f = Fixture::new().await;
    let request = f.add().await;
    // Deliberately corrupted test-owned SQL fixtures: production persistence
    // rejects these already. Never relax schema CHECKs for execution itself.
    for value in [
        "..\\fixture.exe",
        "bin\\..\\fixture.exe",
        "C:fixture.exe",
        "\\\\server\\fixture.exe",
        "\\\\?\\C:\\fixture.exe",
        "\\root.exe",
        "bin/fixture.exe",
        "%TEMP%\\fixture.exe",
    ] {
        sqlx::query("UPDATE launch_profiles SET executable_value=? WHERE id=?")
            .bind(value)
            .bind(&request.profile_id)
            .execute(&mut f.connection)
            .await
            .unwrap();
        let result = prepare(&mut f.connection, request.clone(), Ready).await;
        if value.starts_with('%') {
            // Percent is literal, never expanded; path exists only if configured.
            let p = result.unwrap();
            assert_eq!(
                execute(&mut f.connection, p, ExplicitUserAttempt, Ready)
                    .await
                    .err()
                    .unwrap()
                    .stage,
                "resolution"
            );
        } else {
            assert!(result.is_err(), "{value}");
        }
    }
    for value in [
        "test.bat",
        "test.cmd",
        "test.ps1",
        "test.sh",
        "test.lnk",
        "test.dll",
        "test.url",
        "test.txt",
        "test",
        "steam:123",
    ] {
        sqlx::query("UPDATE launch_profiles SET executable_value=? WHERE id=?")
            .bind(value)
            .bind(&request.profile_id)
            .execute(&mut f.connection)
            .await
            .unwrap();
        assert!(
            prepare(&mut f.connection, request.clone(), Ready)
                .await
                .is_err(),
            "{value}"
        );
    }
    sqlx::query(
        "UPDATE launch_profiles SET executable_kind='absolutePath',executable_value=? WHERE id=?",
    )
    .bind(f.exe.to_str().unwrap())
    .bind(&request.profile_id)
    .execute(&mut f.connection)
    .await
    .unwrap();
    for args in [
        vec!["a".into(); 129],
        vec!["a".repeat(4097)],
        vec!["a".repeat(4096); 5],
        vec!["nul\0".into()],
    ] {
        sqlx::query("PRAGMA ignore_check_constraints=ON")
            .execute(&mut f.connection)
            .await
            .unwrap();
        sqlx::query("UPDATE launch_profiles SET arguments_json=? WHERE id=?")
            .bind(serde_json::to_string(&args).unwrap())
            .bind(&request.profile_id)
            .execute(&mut f.connection)
            .await
            .unwrap();
        assert!(
            prepare(&mut f.connection, request.clone(), Ready)
                .await
                .is_err()
        );
    }
    sqlx::query("UPDATE launch_profiles SET arguments_json=? WHERE id=?")
        .bind(serde_json::to_string(&vec!["\\".repeat(4096); 4]).unwrap())
        .bind(&request.profile_id)
        .execute(&mut f.connection)
        .await
        .unwrap();
    failure(
        prepare(&mut f.connection, request.clone(), Ready).await,
        "resolution",
        "encodedArgumentsTooLong",
    );
    sqlx::query("UPDATE launch_profiles SET arguments_json='[]',working_directory_kind='unsupported',working_directory_value='work' WHERE id=?").bind(&request.profile_id).execute(&mut f.connection).await.unwrap();
    failure(
        prepare(&mut f.connection, request.clone(), Ready).await,
        "resolution",
        "unsupportedReference",
    );
    sqlx::query("UPDATE launch_profiles SET working_directory_kind='installationRelative',working_directory_value='..\\work' WHERE id=?").bind(&request.profile_id).execute(&mut f.connection).await.unwrap();
    assert!(
        prepare(&mut f.connection, request.clone(), Ready)
            .await
            .is_err()
    );
    sqlx::query("UPDATE launch_profiles SET working_directory_kind=NULL,working_directory_value=NULL WHERE id=?").bind(&request.profile_id).execute(&mut f.connection).await.unwrap();
    // Empty/unsupported locations cannot resolve a relative target, even when
    // another executable with that basename exists in a different directory.
    sqlx::query("UPDATE launch_profiles SET executable_kind='installationRelative',executable_value='fixture.exe' WHERE id=?").bind(&request.profile_id).execute(&mut f.connection).await.unwrap();
    for root in [
        "",
        "relative",
        "C:relative",
        "\\\\server\\share",
        "\\\\?\\C:\\root",
    ] {
        sqlx::query("UPDATE game_installations SET root_path_ref=? WHERE id=?")
            .bind(root)
            .bind(&f.installation)
            .execute(&mut f.connection)
            .await
            .unwrap();
        failure(
            prepare(&mut f.connection, request.clone(), Ready).await,
            "resolution",
            "invalidInstallationLocation",
        );
    }
    assert!(!f.exe.parent().unwrap().join("exited.txt").exists());
    f.close().await;
}

#[tokio::test]
async fn identity_revisions_authorization_and_default_changes() {
    let mut f = Fixture::new().await;
    let r = f.add().await;
    let duplicate = f.add().await;
    assert_ne!(r.profile_id, duplicate.profile_id);
    for (game, profile) in [
        ("0".repeat(32), r.profile_id.clone()),
        (f.game.clone(), "0".repeat(32)),
    ] {
        let mut wrong = r.clone();
        wrong.game_id = game;
        wrong.profile_id = profile;
        failure(
            prepare(&mut f.connection, wrong, Ready).await,
            "request",
            "missingOrUnrelated",
        );
    }
    let other =
        f.db.manual_create(&manual::CreateRequest {
            metadata: manual::Metadata {
                title: "Other".into(),
                release_date: None,
                genre: None,
                developer: None,
                favorite: false,
            },
            source_name: "Manual".into(),
            installation: None,
        })
        .await
        .unwrap();
    let mut cross = r.clone();
    cross.game_id = other;
    failure(
        prepare(&mut f.connection, cross, Ready).await,
        "request",
        "missingOrUnrelated",
    );
    for admission in [NotReady, Stopping] {
        failure(
            prepare(&mut f.connection, r.clone(), admission).await,
            "request",
            "notReady",
        );
        let p = f.prepared(r.clone()).await;
        failure(
            execute(&mut f.connection, p, ExplicitUserAttempt, admission).await,
            "request",
            "notReady",
        );
    }
    let p = f.prepared(r.clone()).await;
    failure(
        execute(&mut f.connection, p, Cancelled, Ready).await,
        "request",
        "notAuthorized",
    );
    let p = f.prepared(r.clone()).await;
    f.action(Action::LaunchProfile {
        action: ProfileAction::Save {
            profile_id: r.profile_id.clone(),
            expected_profile_revision: 0,
            configuration: f.configuration.clone(),
        },
    })
    .await;
    failure(
        execute(&mut f.connection, p, ExplicitUserAttempt, Ready).await,
        "request",
        "staleContext",
    );
    let mut fresh = r;
    fresh.expected_profile_revision = 1;
    let p = f.prepared(fresh.clone()).await;
    f.action(Action::LaunchProfile {
        action: ProfileAction::SetDefault {
            profile_id: Some(duplicate.profile_id.clone()),
            expected_profile_revision: Some(0),
        },
    })
    .await;
    failure(
        execute(&mut f.connection, p, ExplicitUserAttempt, Ready).await,
        "request",
        "staleContext",
    );
    fresh.expected_game_revision = f.db.manual_details(&f.game).await.unwrap().revision;
    let p = f.prepared(fresh.clone()).await;
    f.action(Action::LaunchProfile {
        action: ProfileAction::Remove {
            profile_id: fresh.profile_id.clone(),
            expected_profile_revision: 1,
        },
    })
    .await;
    failure(
        execute(&mut f.connection, p, ExplicitUserAttempt, Ready).await,
        "request",
        "missingOrUnrelated",
    );
    assert!(!f.exe.parent().unwrap().join("exited.txt").exists());
    f.close().await;
}

#[tokio::test]
async fn unsupported_provenance_sources_and_defensive_context_comparison() {
    let mut f = Fixture::new().await;
    let r = f.add().await;
    let prepared = f.prepared(r.clone()).await;
    // Simulates a broken writer that failed to advance the game revision.
    sqlx::query("UPDATE game_installations SET version='changed' WHERE id=?")
        .bind(&f.installation)
        .execute(&mut f.connection)
        .await
        .unwrap();
    failure(
        execute(&mut f.connection, prepared, ExplicitUserAttempt, Ready).await,
        "request",
        "staleContext",
    );
    sqlx::query("PRAGMA ignore_check_constraints=ON")
        .execute(&mut f.connection)
        .await
        .unwrap();
    sqlx::query("UPDATE launch_profiles SET launcher_kind='steam' WHERE id=?")
        .bind(&r.profile_id)
        .execute(&mut f.connection)
        .await
        .unwrap();
    failure(
        prepare(&mut f.connection, r.clone(), Ready).await,
        "request",
        "unsupportedContext",
    );
    sqlx::query("UPDATE launch_profiles SET launcher_kind='standalone' WHERE id=?")
        .bind(&r.profile_id)
        .execute(&mut f.connection)
        .await
        .unwrap();
    // Immutable-identity trigger must be removed only in this corrupt fixture.
    sqlx::query("DROP TRIGGER launch_profile_fixed_identity")
        .execute(&mut f.connection)
        .await
        .unwrap();
    sqlx::query("UPDATE launch_profiles SET provenance='provider' WHERE id=?")
        .bind(&r.profile_id)
        .execute(&mut f.connection)
        .await
        .unwrap();
    failure(
        prepare(&mut f.connection, r.clone(), Ready).await,
        "request",
        "unsupportedContext",
    );
    sqlx::query("UPDATE launch_profiles SET provenance='user' WHERE id=?")
        .bind(&r.profile_id)
        .execute(&mut f.connection)
        .await
        .unwrap();
    sqlx::query("UPDATE library_sources SET source_type='steam'")
        .execute(&mut f.connection)
        .await
        .unwrap();
    failure(
        prepare(&mut f.connection, r.clone(), Ready).await,
        "request",
        "unsupportedContext",
    );
    sqlx::query("UPDATE library_sources SET source_type='manual'")
        .execute(&mut f.connection)
        .await
        .unwrap();
    sqlx::query("UPDATE database_metadata SET purpose='benchmark'")
        .execute(&mut f.connection)
        .await
        .unwrap();
    failure(
        prepare(&mut f.connection, r, Ready).await,
        "request",
        "wrongPurpose",
    );
    assert!(!f.exe.parent().unwrap().join("exited.txt").exists());
    f.close().await;
}

#[tokio::test]
async fn native_missing_wrong_types_invalid_format_and_sharing_denied() {
    use std::os::windows::fs::OpenOptionsExt;
    let mut f = Fixture::new().await;
    f.configuration.executable_ref =
        PathRef::AbsolutePath(f._root.path().join("missing.exe").to_str().unwrap().into());
    let r = f.add().await;
    let p = f.prepared(r).await;
    let e = execute(&mut f.connection, p, ExplicitUserAttempt, Ready)
        .await
        .err()
        .unwrap();
    assert_eq!((e.stage, e.os_code), ("resolution", Some(2)));
    let invalid = f._root.path().join("invalid.exe");
    fs::write(&invalid, b"not a PE executable & echo no shell").unwrap();
    f.configuration.executable_ref = PathRef::AbsolutePath(invalid.to_str().unwrap().into());
    let r = f.add().await;
    let p = f.prepared(r).await;
    let e = execute(&mut f.connection, p, ExplicitUserAttempt, Ready)
        .await
        .err()
        .unwrap();
    assert_eq!(e.stage, "creation");
    assert!(
        matches!(e.os_code, Some(193 | 216)),
        "loader must reject invalid format: {e:?}"
    );
    let dir_exe = f._root.path().join("directory.exe");
    fs::create_dir(&dir_exe).unwrap();
    f.configuration.executable_ref = PathRef::AbsolutePath(dir_exe.to_str().unwrap().into());
    let r = f.add().await;
    let p = f.prepared(r).await;
    failure(
        execute(&mut f.connection, p, ExplicitUserAttempt, Ready).await,
        "resolution",
        "wrongObjectType",
    );
    f.configuration.executable_ref = PathRef::AbsolutePath(f.exe.to_str().unwrap().into());
    for wd in [f._root.path().join("missing-directory"), invalid] {
        f.configuration.working_directory_ref =
            Some(PathRef::AbsolutePath(wd.to_str().unwrap().into()));
        let r = f.add().await;
        let p = f.prepared(r).await;
        assert_eq!(
            execute(&mut f.connection, p, ExplicitUserAttempt, Ready)
                .await
                .err()
                .unwrap()
                .stage,
            "resolution"
        );
    }
    f.configuration.working_directory_ref = None;
    let r = f.add().await;
    let p = f.prepared(r).await;
    let lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&f.exe)
        .unwrap();
    let e = execute(&mut f.connection, p, ExplicitUserAttempt, Ready)
        .await
        .err()
        .unwrap();
    assert_eq!(e.stage, "creation");
    assert_eq!(e.os_code, Some(32));
    drop(lock);
    f.close().await;
}

#[tokio::test]
async fn no_privileged_handle_inheritance_and_bounded_cleanup() {
    const RESOURCE_CHILD: &str = "GM_M17_SYNTHETIC_RESOURCE_CHILD";
    if std::env::var(RESOURCE_CHILD).as_deref() != Ok("1") {
        let _serial = NATIVE_CASE.lock().await;
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "no_privileged_handle_inheritance_and_bounded_cleanup",
                "--test-threads=1",
                "--nocapture",
            ])
            .env(RESOURCE_CHILD, "1")
            .output()
            .unwrap();
        println!("{}", String::from_utf8_lossy(&output.stdout));
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let mut f = Fixture::new().await;
    // SAFETY: unnamed manual-reset event. Parent owns it until all children exit.
    let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
    assert!(!event.is_null());
    let event = unsafe { OwnedHandle::from_raw_handle(event) };
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
    let directory = f.exe.parent().unwrap().to_path_buf();
    fs::write(
        directory.join("fixture-control.txt"),
        format!("0 0 {}", event.as_raw_handle() as usize),
    )
    .unwrap();
    let r = f.add().await;
    wait(f.run(r).await);
    assert_eq!(
        unsafe { WaitForSingleObject(event.as_raw_handle(), 0) },
        WAIT_TIMEOUT,
        "unrelated inheritable event leaked"
    );
    // Positive sensor/control for the API investigation: stable Command really
    // does inherit this event, even with all standard streams sent to NUL.
    let status = Command::new(&f.exe)
        .current_dir(&directory)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(
        unsafe { WaitForSingleObject(event.as_raw_handle(), 0) },
        WAIT_OBJECT_0
    );
    // Profile creation uses a separate SQLx pool that lazily opens connections.
    // Its three-file connection growth is fixture work, not launcher ownership.
    // Finish configuration and close that pool before measuring the full launch
    // path on the already-open dedicated connection. No launcher warmup is added.
    let request = f.add().await;
    f.db.close().await;
    // Exact single-test subprocess excludes other test runtimes. Every launch
    // must retain exactly its returned process handle and release it after wait.
    let count = || {
        let mut n = 0;
        assert_ne!(
            unsafe { GetProcessHandleCount(GetCurrentProcess(), &mut n) },
            0
        );
        n
    };
    let before = count();
    for iteration in 0..20 {
        let created = f.run(request.clone()).await;
        assert_eq!(
            count(),
            before + 1,
            "only the returned process handle: {iteration}"
        );
        wait(created);
        assert_eq!(count(), before, "retained handles after launch {iteration}");
    }
    let after = count();
    println!("M17 handles before={before} after={after}; 20 completed repeat launches");
    assert_eq!(
        after, before,
        "unexpected retained handles: {before} -> {after}"
    );
    f.close().await;
}

#[tokio::test]
async fn simultaneous_children_and_cancellation_after_creation_release_handles() {
    let mut f = Fixture::new().await;
    let mut children = Vec::new();
    for i in 0..3 {
        let wd = f._root.path().join(format!("parallel {i}"));
        control(&wd, i, 300);
        f.configuration.working_directory_ref =
            Some(PathRef::AbsolutePath(wd.to_str().unwrap().into()));
        let r = f.add().await;
        children.push((f.run(r).await, wd));
    }
    assert!(children.iter().all(|(child, _)| unsafe {
        WaitForSingleObject(child.handle.as_raw_handle(), 0)
    } == WAIT_TIMEOUT));
    let (released, wd) = children.pop().unwrap();
    drop(released); // Model cancellation/app shutdown AFTER creation: no kill.
    for (child, wd) in children {
        let (pid, _) = wait(child);
        assert_eq!(report(&wd)[0], pid.to_string());
    }
    assert_eq!(
        fs::read_to_string({
            report(&wd);
            wd.join("exited.txt")
        })
        .unwrap(),
        "2"
    );
    f.close().await;
}

#[tokio::test]
async fn isolated_preparation_and_creation_samples() {
    let mut f = Fixture::new().await;
    let request = f.add().await;
    wait(f.run(request.clone()).await); // One excluded warmup, compile excluded.
    let mut prepare_us = Vec::new();
    let mut creation_us = Vec::new();
    for _ in 0..12 {
        let start = Instant::now();
        let p = f.prepared(request.clone()).await;
        prepare_us.push(start.elapsed().as_micros());
        let start = Instant::now();
        let child = execute(&mut f.connection, p, ExplicitUserAttempt, Ready)
            .await
            .unwrap();
        creation_us.push(start.elapsed().as_micros());
        wait(child);
    }
    println!("M17 preparation_us={prepare_us:?} revalidation_plus_creation_us={creation_us:?}");
    f.close().await;
}

#[tokio::test]
async fn native_access_denied_and_elevation_required_fail_without_fallback() {
    struct Acl(PathBuf);
    impl Drop for Acl {
        fn drop(&mut self) {
            let result = Command::new("icacls")
                .arg(&self.0)
                .args(["/remove:d", "*S-1-1-0"])
                .output()
                .unwrap();
            assert!(result.status.success(), "restore disposable fixture ACL");
        }
    }
    let mut f = Fixture::new().await;
    let r = f.add().await;
    let p = f.prepared(r.clone()).await;
    let guard = Acl(f.exe.clone());
    let result = Command::new("icacls")
        .arg(&f.exe)
        .args(["/deny", "*S-1-1-0:(X)"])
        .output()
        .unwrap();
    assert!(result.status.success(), "fixture execute ACL setup");
    let err = execute(&mut f.connection, p, ExplicitUserAttempt, Ready)
        .await
        .err()
        .unwrap();
    assert_eq!((err.stage, err.os_code), ("creation", Some(5)));
    drop(guard);
    let wd = f._root.path().join("denied directory");
    control(&wd, 0, 0);
    f.configuration.working_directory_ref =
        Some(PathRef::AbsolutePath(wd.to_str().unwrap().into()));
    let r = f.add().await;
    let p = f.prepared(r).await;
    let guard = Acl(wd.clone());
    let result = Command::new("icacls")
        .arg(&wd)
        .args(["/deny", "*S-1-1-0:(F)"])
        .output()
        .unwrap();
    assert!(result.status.success(), "fixture directory ACL setup");
    let err = execute(&mut f.connection, p, ExplicitUserAttempt, Ready)
        .await
        .err()
        .unwrap();
    assert_eq!(err.os_code, Some(5));
    assert!(matches!(err.stage, "resolution" | "creation"));
    drop(guard);
    assert!(!wd.join("exited.txt").exists());
    let manifest = f._root.path().join("administrator.manifest");
    fs::write(
        &manifest,
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
<trustInfo xmlns="urn:schemas-microsoft-com:asm.v3"><security><requestedPrivileges>
<requestedExecutionLevel level="requireAdministrator" uiAccess="false"/>
</requestedPrivileges></security></trustInfo></assembly>"#,
    )
    .unwrap();
    let admin = f.exe.with_file_name("requires administrator.exe");
    compile(&admin, Some(&manifest));
    f.configuration.executable_ref = PathRef::AbsolutePath(admin.to_str().unwrap().into());
    f.configuration.working_directory_ref = None;
    let r = f.add().await;
    let p = f.prepared(r).await;
    let err = execute(&mut f.connection, p, ExplicitUserAttempt, Ready)
        .await
        .err()
        .unwrap();
    assert_eq!((err.stage, err.os_code), ("creation", Some(740)));
    assert!(!f.exe.parent().unwrap().join("exited.txt").exists());
    f.close().await;
}

#[tokio::test]
async fn parent_exit_does_not_kill_synthetic_child() {
    const DRIVER_ROOT: &str = "GM_M17_SYNTHETIC_PARENT_ROOT";
    if let Some(directory) = std::env::var_os(DRIVER_ROOT) {
        let root = PathBuf::from(directory);
        assert_eq!(
            fs::read(root.join("authorized.txt")).unwrap(),
            b"M1.7 synthetic parent only"
        );
        let mut f = Fixture::with_root(tempfile::tempdir_in(&root).unwrap()).await;
        control(f.exe.parent().unwrap(), 0, 700);
        let r = f.add().await;
        let child = f.run(r).await;
        fs::write(
            root.join("child.json"),
            serde_json::to_vec(
                &serde_json::json!({"directory":f.exe.parent().unwrap(),"pid":child.pid}),
            )
            .unwrap(),
        )
        .unwrap();
        drop(child);
        // Intentional real parent exit: no destructors/worker or kill-on-close job.
        std::process::exit(0);
    }
    let _serial = NATIVE_CASE.lock().await;
    let root = tempdir().unwrap();
    fs::write(
        root.path().join("authorized.txt"),
        b"M1.7 synthetic parent only",
    )
    .unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "parent_exit_does_not_kill_synthetic_child",
            "--test-threads=1",
        ])
        .env(DRIVER_ROOT, root.path())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "synthetic launcher parent failed");
    let data: serde_json::Value =
        serde_json::from_slice(&fs::read(root.path().join("child.json")).unwrap()).unwrap();
    let directory = Path::new(data["directory"].as_str().unwrap());
    assert!(directory.starts_with(root.path()));
    let received = report(directory);
    assert_eq!(received[0], data["pid"].as_u64().unwrap().to_string());
    assert_eq!(
        fs::read_to_string(directory.join("exited.txt")).unwrap(),
        "0"
    );
}

#[test]
fn request_and_production_surface_are_identifier_only() {
    let request = serde_json::json!({"gameId":"0".repeat(32),"profileId":"1".repeat(32),"expectedProfileRevision":0,"expectedGameRevision":0});
    assert!(serde_json::from_value::<Request>(request.clone()).is_ok());
    for field in [
        "executablePath",
        "commandLine",
        "workingDirectory",
        "environment",
        "installationId",
        "authorized",
    ] {
        let mut invalid = request.clone();
        invalid[field] = serde_json::json!("untrusted");
        assert!(serde_json::from_value::<Request>(invalid).is_err());
    }
    // M1.8 intentionally registers production IPC. The historical candidate
    // remains test-only; production must not import its test authority enums.
    let production = include_str!("../src/launch/mod.rs");
    assert!(!production.contains("tests/support"));
    assert!(!production.contains("Authorization::"));
    assert!(include_str!("../build.rs").contains("\"standalone_launch\""));
    assert!(!include_str!("../capabilities/default.json").contains("shell:"));
}
