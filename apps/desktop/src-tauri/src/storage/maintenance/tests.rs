// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use tempfile::tempdir;

fn cancel() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}
async fn connection(path: &Path) -> SqliteConnection {
    SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .foreign_keys(true),
    )
    .await
    .unwrap()
}
async fn populate_v2(c: &mut SqliteConnection) {
    sqlx::raw_sql("INSERT INTO games(id,title,sort_title,release_date,genre,developer,favorite,created_at_ms,updated_at_ms,origin,revision) VALUES
        ('g1','ゲーム Café','ゲーム café',NULL,NULL,NULL,0,123,456,'manual',17),
        ('g2','ゲーム Café','ゲーム café','2000-02-29','冒険','Développeur',1,234,567,'legacy',99),
        ('g3','Owned, uninstalled','owned, uninstalled',NULL,NULL,NULL,0,1,2,'legacy',7);
        INSERT INTO library_sources VALUES ('s1','manual','Manual 日本語'),('s2','steam','Legacy source');
        INSERT INTO library_entries VALUES ('e1','g1','s1',NULL,'playing',123),('e2','g1','s2','100','completed',124),('e3','g2','s1',NULL,'abandoned',125),('e4','g3','s2','200','not_started',126);
        INSERT INTO game_installations VALUES ('i1','e1','metadata://one','windows','v1','present','user'),('i2','e1','metadata://two','windows',NULL,'missing','legacy'),('i3','e2','metadata://three','windows','β','unknown','user');
        INSERT INTO game_field_overrides VALUES ('g1','title','ゲーム Café'),('g1','release_date',NULL),('g1','genre',NULL),('g1','developer',NULL),('g1','favorite','0');")
        .execute(c).await.unwrap();
}
async fn populate(db: &Database) {
    populate_v2(&mut db.pool.acquire().await.unwrap()).await;
    sqlx::raw_sql("INSERT INTO launch_profiles(id,installation_id,provenance,display_name,launcher_kind,executable_kind,executable_value,arguments_json,revision,created_at_ms,updated_at_ms) VALUES ('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa','i1','user','Café','standalone','installationRelative','game.exe','[\"two words\",\"\",\"日本語\"]',7,123,456); INSERT INTO game_launch_defaults VALUES ('g1','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa');").execute(&db.pool).await.unwrap();
    sqlx::raw_sql("INSERT INTO installation_groups VALUES ('group-a','g1','日本語 copies'),('group-b','g1','日本語 copies'); INSERT INTO installation_group_members VALUES ('i1','group-a'),('i3','group-a');").execute(&db.pool).await.unwrap();
}
async fn dump(db: &Database) -> Vec<String> {
    let mut result = vec![];
    for sql in [
        "SELECT json_group_array(json_array(singleton,purpose,generator_version,dataset_seed,dataset_games,generation_complete)) FROM (SELECT * FROM database_metadata ORDER BY singleton)",
        "SELECT json_group_array(json_array(version,description,installed_on,success,checksum,execution_time)) FROM (SELECT version,description,installed_on,success,hex(checksum) checksum,execution_time FROM _sqlx_migrations ORDER BY version)",
        "SELECT json_group_array(json_array(id,title,sort_title,release_date,genre,developer,favorite,created_at_ms,updated_at_ms,origin,revision)) FROM (SELECT * FROM games ORDER BY id)",
        "SELECT json_group_array(json_array(id,source_type,display_name)) FROM (SELECT * FROM library_sources ORDER BY id)",
        "SELECT json_group_array(json_array(id,game_id,source_id,external_item_id,completion_status,added_at_ms)) FROM (SELECT * FROM library_entries ORDER BY id)",
        "SELECT json_group_array(json_array(id,entry_id,root_path_ref,platform,version,detection_state,state_origin)) FROM (SELECT * FROM game_installations ORDER BY id)",
        "SELECT json_group_array(json_array(game_id,field,value)) FROM (SELECT * FROM game_field_overrides ORDER BY game_id,field)",
        "SELECT json_group_array(json_array(id,game_id,display_name)) FROM (SELECT * FROM installation_groups ORDER BY id)",
        "SELECT json_group_array(json_array(installation_id,group_id)) FROM (SELECT * FROM installation_group_members ORDER BY installation_id)",
        "SELECT json_group_array(json_array(id,installation_id,provenance,display_name,launcher_kind,executable_kind,executable_value,arguments_json,working_directory_kind,working_directory_value,revision,created_at_ms,updated_at_ms)) FROM (SELECT * FROM launch_profiles ORDER BY id)",
        "SELECT json_group_array(json_array(game_id,profile_id)) FROM (SELECT * FROM game_launch_defaults ORDER BY game_id)",
    ] {
        result.push(sqlx::query_scalar(sql).fetch_one(&db.pool).await.unwrap());
    }
    result
}
async fn prepared(root: &Path) -> (Database, PreparedRestore, Vec<String>) {
    let db = Database::initialize(&root.join("library.sqlite3"))
        .await
        .unwrap();
    populate(&db).await;
    let expected = dump(&db).await;
    let backup = root.join("selected.sqlite3");
    create_backup(&db, &backup, &cancel()).await.unwrap();
    sqlx::query("UPDATE games SET revision=revision+1,title='Current changed title' WHERE id='g2'")
        .execute(&db.pool)
        .await
        .unwrap();
    let candidate = stage_restore(&db, &backup).await.unwrap();
    (db, candidate, expected)
}
fn assert_no_temporary(root: &Path) {
    for entry in std::fs::read_dir(root).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().into_owned();
        assert!(
            !name.starts_with(".gm-backup-") && !name.starts_with(".gm-install-"),
            "lingering {name}"
        );
    }
}

#[tokio::test]
async fn empty_and_exact_identity_round_trip_survive_two_restarts_and_repeated_restore() {
    let root = tempdir().unwrap();
    let path = root.path().join("library.sqlite3");
    let db = Database::initialize(&path).await.unwrap();
    let empty = root.path().join("empty.sqlite3");
    assert_eq!(
        create_backup(&db, &empty, &cancel())
            .await
            .unwrap()
            .counts
            .games,
        0
    );
    populate(&db).await;
    let expected = dump(&db).await;
    let backup = root.path().join("library.backup");
    create_backup(&db, &backup, &cancel()).await.unwrap();
    let bytes = std::fs::read(&backup).unwrap();
    sqlx::query("UPDATE games SET revision=1000 WHERE id='g2'")
        .execute(&db.pool)
        .await
        .unwrap();
    let prior = dump(&db).await;
    let candidate = stage_restore(&db, &backup).await.unwrap();
    assert_eq!(dump(&db).await, prior);
    assert_eq!(std::fs::read(&backup).unwrap(), bytes);
    candidate.confirm(&db).await.unwrap();
    assert!(db.is_stopping());
    assert_eq!(
        Database::initialize(&path).await.unwrap_err().code,
        ErrorCode::Conflict
    );
    assert!(db.counts().await.is_err());
    drop(db); // actual connection close + process-lock lifetime boundary
    let db = Database::initialize(&path).await.unwrap();
    assert_eq!(outcome(&path).unwrap(), Some("restored"));
    assert_eq!(dump(&db).await, expected);
    let first = journal(&path).unwrap().unwrap();
    assert!(first.join("previous.sqlite3").is_file());
    assert!(first.join("candidate.sqlite3").is_file());
    db.close().await;
    let db = Database::initialize(&path).await.unwrap();
    assert_eq!(dump(&db).await, expected);
    let second = stage_restore(&db, &empty).await.unwrap();
    second.confirm(&db).await.unwrap();
    drop(db);
    let db = Database::initialize(&path).await.unwrap();
    assert_eq!(db.counts().await.unwrap().games, 0);
    assert!(first.join("archived-intent").is_file());
    assert!(first.join("previous.sqlite3").is_file());
    db.close().await;
    assert_no_temporary(root.path());
}

#[tokio::test]
async fn cancellation_collisions_partial_output_and_failed_publication_preserve_active_data() {
    let root = tempdir().unwrap();
    let db = Database::initialize(&root.path().join("library.sqlite3"))
        .await
        .unwrap();
    populate(&db).await;
    let expected = dump(&db).await;
    let target = root.path().join("backup.sqlite3");
    std::fs::write(&target, b"keep existing file").unwrap();
    assert_eq!(
        create_backup(&db, &target, &cancel())
            .await
            .unwrap_err()
            .code,
        ErrorCode::AlreadyExists
    );
    assert_eq!(std::fs::read(&target).unwrap(), b"keep existing file");
    let flag = cancel();
    flag.store(true, Ordering::SeqCst);
    assert!(
        create_backup(&db, &root.path().join("cancelled"), &flag)
            .await
            .is_err()
    );
    for point in [
        "backup-partial-output",
        "backup-snapshot",
        "backup-validated",
        "backup-publication",
    ] {
        let target = root.path().join(point);
        assert!(
            create_backup_with(
                &db,
                &target,
                &cancel(),
                Faults {
                    fail: Some(point),
                    ..Default::default()
                }
            )
            .await
            .is_err()
        );
        assert!(!target.exists());
        assert_no_temporary(root.path());
    }
    assert!(
        create_backup(&db, &root.path().join("missing-parent/file"), &cancel())
            .await
            .is_err()
    );
    let valid = root.path().join("valid.sqlite3");
    create_backup(&db, &valid, &cancel()).await.unwrap();
    let staged = stage_restore(&db, &valid).await.unwrap();
    let staging = staged.directory.path().to_owned();
    drop(staged);
    assert!(!staging.exists());
    assert_eq!(dump(&db).await, expected);
    db.close().await;
}

#[tokio::test]
async fn snapshots_include_committed_wal_and_exclude_uncommitted_edits_during_reads_and_writes() {
    let root = tempdir().unwrap();
    let path = root.path().join("library.sqlite3");
    let db = Database::initialize(&path).await.unwrap();
    populate(&db).await;
    sqlx::query("PRAGMA wal_autocheckpoint=0")
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE games SET revision=1234 WHERE id='g2'")
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(std::fs::metadata(component(&path, "-wal")).unwrap().len() > 0);
    let mut tx = db.pool.begin().await.unwrap();
    sqlx::query("UPDATE games SET revision=9999 WHERE id='g2'")
        .execute(&mut *tx)
        .await
        .unwrap();
    let target = root.path().join("backup.sqlite3");
    let flag = cancel();
    let (snapshot, ()) = tokio::join!(create_backup(&db, &target, &flag), async {
        for _ in 0..20 {
            assert_eq!(db.counts().await.unwrap().games, 3);
            tokio::task::yield_now().await;
        }
    });
    snapshot.unwrap();
    tx.rollback().await.unwrap();
    let c = stage_restore(&db, &target).await.unwrap();
    c.confirm(&db).await.unwrap();
    drop(db);
    let db = Database::initialize(&path).await.unwrap();
    assert_eq!(db.manual_details("g2").await.unwrap().revision, 1234);
    db.close().await;
}

#[tokio::test]
async fn invalid_backups_never_touch_active_or_selected_files() {
    let root = tempdir().unwrap();
    let path = root.path().join("library.sqlite3");
    let db = Database::initialize(&path).await.unwrap();
    populate(&db).await;
    let expected = dump(&db).await;
    let valid = root.path().join("valid.sqlite3");
    create_backup(&db, &valid, &cancel()).await.unwrap();
    for (name, sql) in [
        (
            "benchmark",
            "UPDATE database_metadata SET purpose='benchmark'",
        ),
        (
            "newer",
            "INSERT INTO _sqlx_migrations(version,description,success,checksum,execution_time) VALUES (999,'newer',1,x'00',0)",
        ),
        ("checksum", "UPDATE _sqlx_migrations SET checksum=x'00'"),
        ("dirty", "UPDATE _sqlx_migrations SET success=0"),
        (
            "invalid-success-flag",
            "UPDATE _sqlx_migrations SET success=2",
        ),
        ("missing", "DROP TABLE game_field_overrides"),
        (
            "cross-game-group",
            "UPDATE library_entries SET game_id='g2' WHERE id='e2'",
        ),
        ("extra", "CREATE TABLE unrelated(secret TEXT)"),
        (
            "profile-traversal",
            "UPDATE launch_profiles SET executable_value='..\\escape.exe'",
        ),
        (
            "profile-arguments",
            "UPDATE launch_profiles SET arguments_json='[1]'",
        ),
        (
            "profile-working-directory",
            "UPDATE launch_profiles SET working_directory_kind='absolutePath',working_directory_value='C:relative'",
        ),
        (
            "profile-unsupported",
            "PRAGMA ignore_check_constraints=ON; UPDATE launch_profiles SET launcher_kind='shell'",
        ),
        (
            "profile-revision",
            "PRAGMA ignore_check_constraints=ON; UPDATE launch_profiles SET revision=-1",
        ),
        (
            "override",
            "UPDATE game_field_overrides SET value='1' WHERE field='favorite'",
        ),
        (
            "foreign-key",
            "PRAGMA foreign_keys=OFF; UPDATE library_entries SET game_id='absent' WHERE id='e4'",
        ),
        (
            "metadata",
            "UPDATE database_metadata SET dataset_games=15000",
        ),
    ] {
        let file = root.path().join(name);
        std::fs::copy(&valid, &file).unwrap();
        let mut c = connection(&file).await;
        sqlx::raw_sql(sql).execute(&mut c).await.unwrap();
        c.close().await.unwrap();
        let before = std::fs::read(&file).unwrap();
        assert!(stage_restore(&db, &file).await.is_err(), "accepted {name}");
        assert_eq!(std::fs::read(&file).unwrap(), before);
    }
    // Corrupt the data while restoring the exact trigger SQL: this exercises
    // relationship validation rather than merely the schema-mismatch gate.
    let file = root.path().join("cross-game-default");
    std::fs::copy(&valid, &file).unwrap();
    let mut c = connection(&file).await;
    let trigger: String = sqlx::query_scalar(
        "SELECT sql FROM sqlite_schema WHERE name='launch_default_same_game_update'",
    )
    .fetch_one(&mut c)
    .await
    .unwrap();
    sqlx::raw_sql("DROP TRIGGER launch_default_same_game_update; UPDATE game_launch_defaults SET game_id='g2';")
        .execute(&mut c).await.unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(trigger.as_str()))
        .execute(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    let before = std::fs::read(&file).unwrap();
    assert!(stage_restore(&db, &file).await.is_err());
    assert_eq!(std::fs::read(&file).unwrap(), before);
    for (name, data) in [
        ("unrecognized", b"not SQLite".as_slice()),
        ("truncated", &std::fs::read(&valid).unwrap()[..1024]),
    ] {
        let file = root.path().join(name);
        std::fs::write(&file, data).unwrap();
        assert!(stage_restore(&db, &file).await.is_err());
        assert_eq!(std::fs::read(file).unwrap(), data);
    }
    std::fs::write(component(&valid, "-wal"), b"live sidecar").unwrap();
    assert!(stage_restore(&db, &valid).await.is_err());
    assert_eq!(dump(&db).await, expected);
    db.close().await;
}

#[tokio::test]
async fn transition_failures_recover_original_and_preserve_all_recovery_sets() {
    for point in [
        "attempted",
        "library.sqlite3",
        "library.sqlite3-wal",
        "library.sqlite3-shm",
        "installed",
        "reopened",
        "verified",
        "native-open-denied",
    ] {
        let root = tempdir().unwrap();
        let path = root.path().join("library.sqlite3");
        let (db, candidate, _) = prepared(root.path()).await;
        let expected_original = dump(&db).await;
        candidate.confirm(&db).await.unwrap();
        drop(db);
        let owner = Lifecycle::acquire(&path).unwrap();
        let recovered = resume_with(
            &path,
            Faults {
                fail: Some(point),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(dump(&recovered).await, expected_original, "failed {point}");
        assert_eq!(outcome(&path).unwrap(), Some("recovered"));
        recovered.close().await;
        drop(owner);
        let reopened = Database::initialize(&path).await.unwrap();
        assert_eq!(dump(&reopened).await, expected_original);
        reopened.close().await;
        assert_no_temporary(root.path());
    }
}

#[tokio::test]
async fn interrupted_apply_and_interrupted_recovery_are_repeatable() {
    for point in ["attempted", "installed", "recovery-installed"] {
        let root = tempdir().unwrap();
        let path = root.path().join("library.sqlite3");
        let (db, candidate, _) = prepared(root.path()).await;
        let original = dump(&db).await;
        candidate.confirm(&db).await.unwrap();
        drop(db);
        if point == "recovery-installed" {
            mark(&journal(&path).unwrap().unwrap(), "attempted").unwrap();
        }
        let task_path = path.clone();
        let task = tokio::spawn(async move {
            let _owner = Lifecycle::acquire(&task_path).unwrap();
            resume_with(
                &task_path,
                Faults {
                    interrupt: Some(point),
                    ..Default::default()
                },
            )
            .await
        });
        assert!(task.await.unwrap_err().is_panic());
        let db = Database::initialize(&path).await.unwrap();
        assert_eq!(dump(&db).await, original);
        assert_eq!(outcome(&path).unwrap(), Some("recovered"));
        db.close().await;
    }
}

#[tokio::test]
async fn confirmation_waits_for_existing_transaction_and_closes_all_connections() {
    let root = tempdir().unwrap();
    let path = root.path().join("library.sqlite3");
    let (db, candidate, _) = prepared(root.path()).await;
    let db = Arc::new(db);
    let mut tx = db.pool.begin().await.unwrap();
    sqlx::query("UPDATE games SET revision=777 WHERE id='g2'")
        .execute(&mut *tx)
        .await
        .unwrap();
    let clone = Arc::clone(&db);
    let mut task = tokio::spawn(async move { candidate.confirm(&clone).await });
    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut task)
            .await
            .is_err()
    );
    tx.commit().await.unwrap();
    task.await.unwrap().unwrap();
    let directory = journal(&path).unwrap().unwrap();
    let report = validate_snapshot(&directory.join("previous.sqlite3"))
        .await
        .unwrap();
    assert_eq!(report.counts.games, 3);
    let mut c = connection(&directory.join("previous.sqlite3")).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT revision FROM games WHERE id='g2'")
            .fetch_one(&mut c)
            .await
            .unwrap(),
        777
    );
    c.close().await.unwrap();
    assert!(db.pool.is_closed());
    assert_eq!(db.pool.size(), 0);
    drop(db);
    let db = Database::initialize(&path).await.unwrap();
    db.close().await;
    assert_no_temporary(root.path());
}

#[cfg(windows)]
#[tokio::test]
async fn native_sharing_denial_preserves_selected_and_active_files() {
    use std::os::windows::fs::OpenOptionsExt;
    let root = tempdir().unwrap();
    let db = Database::initialize(&root.path().join("library.sqlite3"))
        .await
        .unwrap();
    let target = root.path().join("locked.sqlite3");
    create_backup(&db, &target, &cancel()).await.unwrap();
    let lock = OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&target)
        .unwrap();
    assert_eq!(
        stage_restore(&db, &target).await.unwrap_err().code,
        ErrorCode::Io
    );
    drop(lock);
    assert_eq!(validate_snapshot(&target).await.unwrap().counts.games, 0);
    db.close().await;
}

#[tokio::test]
async fn native_process_interruptions_recover_without_rust_destructors() {
    const NAME: &str = "storage::maintenance::tests::native_process_interruptions_recover_without_rust_destructors";
    const POINTS: [&str; 10] = [
        "attempted",
        "library.sqlite3",
        "library.sqlite3-wal",
        "library.sqlite3-shm",
        "installed",
        "reopened",
        "verified",
        "recovery-displaced",
        "recovery-installed",
        "library.sqlite3-journal",
    ];
    if let (Some(root), Ok(point)) = (
        std::env::var_os("GM_RESTORE_TEST_ROOT"),
        std::env::var("GM_RESTORE_TEST_POINT"),
    ) {
        let point = *POINTS
            .iter()
            .find(|p| **p == point)
            .expect("test transition");
        let path = PathBuf::from(root).join("library.sqlite3");
        let _owner = Lifecycle::acquire(&path).unwrap();
        let _ = resume_with(
            &path,
            Faults {
                terminate: Some(point),
                ..Default::default()
            },
        )
        .await;
        panic!("test transition was not reached");
    }
    for point in POINTS {
        let root = tempdir().unwrap();
        let path = root.path().join("library.sqlite3");
        let (db, candidate, _) = prepared(root.path()).await;
        let original = dump(&db).await;
        candidate.confirm(&db).await.unwrap();
        drop(db);
        if point.starts_with("recovery-") {
            mark(&journal(&path).unwrap().unwrap(), "attempted").unwrap();
        }
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", NAME, "--nocapture"])
            .env("GM_RESTORE_TEST_ROOT", root.path())
            .env("GM_RESTORE_TEST_POINT", point)
            .output()
            .unwrap();
        assert_eq!(
            status.status.code(),
            Some(86),
            "{point}: {}",
            String::from_utf8_lossy(&status.stderr)
        );
        let db = Database::initialize(&path).await.unwrap();
        assert_eq!(dump(&db).await, original, "interrupted {point}");
        assert_eq!(outcome(&path).unwrap(), Some("recovered"));
        db.close().await;
        assert_no_temporary(root.path());
    }
}

#[tokio::test]
async fn interrupted_unconfirmed_staging_is_cleaned_without_changing_library() {
    const NAME: &str = "storage::maintenance::tests::interrupted_unconfirmed_staging_is_cleaned_without_changing_library";
    if let Some(root) = std::env::var_os("GM_STAGING_TEST_ROOT") {
        let root = PathBuf::from(root);
        let db = Database::initialize(&root.join("library.sqlite3"))
            .await
            .unwrap();
        let _ = stage_restore_with(
            &db,
            &root.join("selected.sqlite3"),
            Faults {
                terminate: Some("staging-copied"),
                ..Default::default()
            },
        )
        .await;
        panic!("staging interruption was not reached");
    }
    let root = tempdir().unwrap();
    let path = root.path().join("library.sqlite3");
    let db = Database::initialize(&path).await.unwrap();
    populate(&db).await;
    let original = dump(&db).await;
    create_backup(&db, &root.path().join("selected.sqlite3"), &cancel())
        .await
        .unwrap();
    db.close().await;
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", NAME])
        .env("GM_STAGING_TEST_ROOT", root.path())
        .output()
        .unwrap();
    assert_eq!(child.status.code(), Some(86));
    let db = Database::initialize(&path).await.unwrap();
    assert_eq!(dump(&db).await, original);
    assert!(journal(&path).unwrap().is_none());
    let entries: Vec<_> = std::fs::read_dir(staging_root(root.path()))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(entries, [std::ffi::OsString::from("owner")]);
    db.close().await;
}

#[tokio::test]
async fn interrupted_confirmation_preserves_original_copy_outside_staging_cleanup() {
    const NAME: &str = "storage::maintenance::tests::interrupted_confirmation_preserves_original_copy_outside_staging_cleanup";
    if let Some(root) = std::env::var_os("GM_CONFIRMATION_TEST_ROOT") {
        let root = PathBuf::from(root);
        let db = Database::initialize(&root.join("library.sqlite3"))
            .await
            .unwrap();
        let candidate = stage_restore(&db, &root.join("selected.sqlite3"))
            .await
            .unwrap();
        let _ = candidate
            .confirm_with(
                &db,
                Faults {
                    terminate: Some("previous-validated"),
                    ..Default::default()
                },
            )
            .await;
        panic!("confirmation interruption was not reached");
    }
    let root = tempdir().unwrap();
    let path = root.path().join("library.sqlite3");
    let db = Database::initialize(&path).await.unwrap();
    populate(&db).await;
    let original = dump(&db).await;
    create_backup(&db, &root.path().join("selected.sqlite3"), &cancel())
        .await
        .unwrap();
    db.close().await;
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", NAME])
        .env("GM_CONFIRMATION_TEST_ROOT", root.path())
        .output()
        .unwrap();
    assert_eq!(child.status.code(), Some(86));
    let db = Database::initialize(&path).await.unwrap();
    assert_eq!(dump(&db).await, original);
    assert!(journal(&path).unwrap().is_none());
    let preserved: Vec<_> = std::fs::read_dir(root.path())
        .unwrap()
        .filter_map(|e| {
            let e = e.unwrap();
            e.file_name()
                .to_str()
                .filter(|n| n.starts_with("gm-restore-"))
                .map(|_| e.path())
        })
        .collect();
    assert_eq!(preserved.len(), 1);
    assert_eq!(
        validate_snapshot(&preserved[0].join("previous.sqlite3"))
            .await
            .unwrap()
            .counts
            .games,
        3
    );
    assert!(preserved[0].join("candidate.sqlite3").is_file());
    db.close().await;
}

#[tokio::test]
async fn controlled_restart_handoff_requires_draining_and_leaves_old_services_closed() {
    let root = tempdir().unwrap();
    let path = root.path().join("library.sqlite3");
    let (old, candidate, expected) = prepared(root.path()).await;
    assert!(old.release_for_restart().is_err());
    candidate.confirm(&old).await.unwrap();
    assert_eq!(
        Database::initialize(&path).await.unwrap_err().code,
        ErrorCode::Conflict
    );
    old.release_for_restart().unwrap();
    let current = Database::initialize(&path).await.unwrap();
    assert_eq!(dump(&current).await, expected);
    assert!(old.maintenance_path().is_err());
    assert!(old.counts().await.is_err());
    drop(old);
    assert_eq!(
        Database::initialize(&path).await.unwrap_err().code,
        ErrorCode::Conflict
    );
    current.close().await;
}

#[tokio::test]
async fn partial_success_markers_and_interrupted_recovery_never_suppress_original_recovery() {
    for complete_marker in [false, true] {
        let root = tempdir().unwrap();
        let path = root.path().join("library.sqlite3");
        let (db, candidate, _) = prepared(root.path()).await;
        let original = dump(&db).await;
        candidate.confirm(&db).await.unwrap();
        drop(db);
        let directory = journal(&path).unwrap().unwrap();
        mark(&directory, "attempted").unwrap();
        if complete_marker {
            mark(&directory, "completed").unwrap();
        } else {
            File::create_new(directory.join("completed")).unwrap();
            File::create_new(directory.join("recovered")).unwrap();
        }
        mark(&directory, "recovering").unwrap();
        displace(&path, &directory, Faults::default()).unwrap();
        assert!(!path.exists());
        let db = Database::initialize(&path).await.unwrap();
        assert_eq!(dump(&db).await, original);
        assert_eq!(outcome(&path).unwrap(), Some("recovered"));
        assert!(owned_marker(&directory, "recovered"));
        db.close().await;
    }
}

#[tokio::test]
async fn damaged_history_after_success_never_replays_restore_over_later_edits() {
    let root = tempdir().unwrap();
    let path = root.path().join("library.sqlite3");
    let (db, candidate, _) = prepared(root.path()).await;
    candidate.confirm(&db).await.unwrap();
    drop(db);
    let current = Database::initialize(&path).await.unwrap();
    let directory = journal(&path).unwrap().unwrap();
    sqlx::query("UPDATE games SET revision=888 WHERE id='g2'")
        .execute(&current.pool)
        .await
        .unwrap();
    let edited = dump(&current).await;
    current.close().await;
    drop(current);
    std::fs::write(directory.join("completed"), b"damaged history marker").unwrap();
    let reopened = Database::initialize(&path).await.unwrap();
    assert_eq!(
        dump(&reopened).await,
        edited,
        "a completed operation must never replay over later library edits"
    );
    reopened.close().await;
}

#[tokio::test]
async fn completion_retirement_interruptions_keep_committed_candidate() {
    const NAME: &str =
        "storage::maintenance::tests::completion_retirement_interruptions_keep_committed_candidate";
    const POINTS: [&str; 3] = ["last-reference", "intent-archived", "pending-retired"];
    if let (Some(root), Ok(point)) = (
        std::env::var_os("GM_RETIRE_TEST_ROOT"),
        std::env::var("GM_RETIRE_TEST_POINT"),
    ) {
        let point = *POINTS.iter().find(|p| **p == point).unwrap();
        let path = PathBuf::from(root).join("library.sqlite3");
        let _owner = Lifecycle::acquire(&path).unwrap();
        let _ = resume_with(
            &path,
            Faults {
                terminate: Some(point),
                ..Default::default()
            },
        )
        .await;
        panic!("retirement transition was not reached");
    }
    for point in POINTS {
        let root = tempdir().unwrap();
        let path = root.path().join("library.sqlite3");
        let (old, candidate, expected) = prepared(root.path()).await;
        candidate.confirm(&old).await.unwrap();
        drop(old);
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", NAME])
            .env("GM_RETIRE_TEST_ROOT", root.path())
            .env("GM_RETIRE_TEST_POINT", point)
            .output()
            .unwrap();
        assert_eq!(child.status.code(), Some(86), "{point}");
        let current = Database::initialize(&path).await.unwrap();
        assert_eq!(dump(&current).await, expected);
        assert!(!pending_path(&path).exists());
        assert_eq!(outcome(&path).unwrap(), Some("restored"));
        current.close().await;
    }
    for point in POINTS {
        let root = tempdir().unwrap();
        let path = root.path().join("library.sqlite3");
        let (old, candidate, expected) = prepared(root.path()).await;
        candidate.confirm(&old).await.unwrap();
        drop(old);
        let owner = Lifecycle::acquire(&path).unwrap();
        assert!(
            resume_with(
                &path,
                Faults {
                    fail: Some(point),
                    ..Default::default()
                }
            )
            .await
            .is_err()
        );
        drop(owner);
        let current = Database::initialize(&path).await.unwrap();
        assert_eq!(
            dump(&current).await,
            expected,
            "finalization failure {point} must not roll committed data back"
        );
        current.close().await;
    }
}

#[tokio::test]
async fn reappearing_completed_reference_cannot_overwrite_later_library_changes() {
    let root = tempdir().unwrap();
    let path = root.path().join("library.sqlite3");
    let (old, candidate, _) = prepared(root.path()).await;
    candidate.confirm(&old).await.unwrap();
    drop(old);
    let current = Database::initialize(&path).await.unwrap();
    let directory = journal(&path).unwrap().unwrap();
    sqlx::query("INSERT INTO games(id,title,sort_title,created_at_ms,updated_at_ms) VALUES ('later','Later record','later record',1,1)").execute(&current.pool).await.unwrap();
    let edited = dump(&current).await;
    current.close().await;
    drop(current);
    // Model an old reference reappearing independently of the committed data.
    std::fs::hard_link(directory.join("archived-intent"), pending_path(&path)).unwrap();
    let reopened = Database::initialize(&path).await.unwrap();
    assert_eq!(dump(&reopened).await, edited);
    assert!(!pending_path(&path).exists());
    reopened.close().await;
    drop(reopened);
    std::fs::write(directory.join("completed"), b"damaged committed marker").unwrap();
    std::fs::hard_link(directory.join("archived-intent"), pending_path(&path)).unwrap();
    assert_eq!(
        Database::initialize(&path).await.unwrap_err().code,
        ErrorCode::InvalidDatabase
    );
    // An ambiguous control record fails closed without changing healthy data.
    let inspected = Database::open(&path, "application", false).await.unwrap();
    assert_eq!(dump(&inspected).await, edited);
    inspected.close().await;
}

#[cfg(windows)]
#[tokio::test]
async fn native_destination_permission_denial_cleans_failed_output() {
    struct Access(std::path::PathBuf);
    impl Drop for Access {
        fn drop(&mut self) {
            let _ = std::process::Command::new("icacls")
                .arg(&self.0)
                .args(["/remove:d", "*S-1-1-0"])
                .output();
        }
    }
    let root = tempdir().unwrap();
    let db = Database::initialize(&root.path().join("library.sqlite3"))
        .await
        .unwrap();
    let destination = root.path().join("denied");
    std::fs::create_dir(&destination).unwrap();
    let guard = Access(destination.clone());
    let result = std::process::Command::new("icacls")
        .arg(&destination)
        .args(["/deny", "*S-1-1-0:(OI)(CI)(W)"])
        .output()
        .unwrap();
    assert!(result.status.success(), "fixture ACL setup failed");
    assert_eq!(
        create_backup(&db, &destination.join("backup.sqlite3"), &cancel())
            .await
            .unwrap_err()
            .code,
        ErrorCode::Io
    );
    drop(guard);
    assert!(std::fs::read_dir(destination).unwrap().next().is_none());
    db.close().await;
}

#[tokio::test]
async fn release_and_stress_scale_maintenance_probe() {
    for size in [15_000i64, 25_000] {
        let root = tempdir().unwrap();
        let path = root.path().join("library.sqlite3");
        let db = Database::initialize(&path).await.unwrap();
        sqlx::query("WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<?) INSERT INTO games(id,title,sort_title,created_at_ms,updated_at_ms,revision) SELECT printf('g%08d',x),printf('Synthetic %08d',x),printf('synthetic %08d',x),x,x,x FROM n")
            .bind(size).execute(&db.pool).await.unwrap();
        sqlx::query(
            "INSERT INTO library_sources VALUES ('source','manual','Synthetic test source')",
        )
        .execute(&db.pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO library_entries(id,game_id,source_id,added_at_ms) SELECT 'e'||id,id,'source',created_at_ms FROM games").execute(&db.pool).await.unwrap();
        sqlx::query("INSERT INTO game_installations(id,entry_id,root_path_ref,platform,detection_state,state_origin) SELECT 'i'||id,id,'synthetic://'||id,'windows','present','user' FROM library_entries WHERE added_at_ms%2=0").execute(&db.pool).await.unwrap();
        let target = root.path().join("snapshot.sqlite3");
        let start = std::time::Instant::now();
        let report = create_backup(&db, &target, &cancel()).await.unwrap();
        let backup_ms = start.elapsed().as_secs_f64() * 1000.;
        let start = std::time::Instant::now();
        let candidate = stage_restore(&db, &target).await.unwrap();
        let stage_ms = start.elapsed().as_secs_f64() * 1000.;
        let start = std::time::Instant::now();
        candidate.confirm(&db).await.unwrap();
        let prepare_ms = start.elapsed().as_secs_f64() * 1000.;
        assert!(db.pool.is_closed());
        assert_eq!(db.pool.size(), 0);
        drop(db);
        let start = std::time::Instant::now();
        let db = Database::initialize(&path).await.unwrap();
        let reopen_ms = start.elapsed().as_secs_f64() * 1000.;
        assert_eq!(db.counts().await.unwrap(), report.counts);
        assert_eq!(db.manual_details("g00000001").await.unwrap().revision, 1);
        println!(
            "MAINTENANCE_SCALE {}",
            serde_json::json!({"games":size,"bytes":report.bytes,"backupMs":backup_ms,"stageMs":stage_ms,"prepareMs":prepare_ms,"restoreStartupMs":reopen_ms,"poolLimit":super::super::POOL_SIZE,"copyComparisonBuffersBytes":131072})
        );
        db.close().await;
        assert_no_temporary(root.path());
    }
}

#[tokio::test]
async fn exact_v2_v3_backups_upgrade_only_active_copies_and_recovery_is_repeatable() {
    for version in [2, 3] {
        let root = tempdir().unwrap();
        let backup = root.path().join(format!("v{version}.sqlite3"));
        let mut c = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(&backup)
                .create_if_missing(true)
                .foreign_keys(true),
        )
        .await
        .unwrap();
        let old = sqlx::migrate::Migrator {
            migrations: std::borrow::Cow::Borrowed(&MIGRATOR.migrations[..version]),
            ..sqlx::migrate::Migrator::DEFAULT
        };
        old.run_direct(None, &mut c, false).await.unwrap();
        populate_v2(&mut c).await;
        if version == 3 {
            sqlx::query("INSERT INTO installation_groups VALUES ('old-group','g1','Preserved v3')")
                .execute(&mut c)
                .await
                .unwrap();
        }
        c.close().await.unwrap();
        let bytes = std::fs::read(&backup).unwrap();
        assert_eq!(
            validate_snapshot(&backup).await.unwrap().schema_version,
            version as i64
        );
        let path = root.path().join("library.sqlite3");
        let db = Database::initialize(&path).await.unwrap();
        populate(&db).await;
        let original = dump(&db).await;
        let staged = stage_restore(&db, &backup).await.unwrap();
        assert_eq!(staged.snapshot.schema_version, version as i64);
        staged.confirm(&db).await.unwrap();
        drop(db);
        let db = Database::initialize(&path).await.unwrap();
        assert_eq!(db.status().await.unwrap().schema_version, 4);
        let d = db.manual_details("g1").await.unwrap();
        assert_eq!(d.revision, 17);
        assert_eq!(d.entries.len(), 2);
        assert_eq!(d.installations.len(), 3);
        assert_eq!(d.groups.len(), if version == 3 { 1 } else { 0 });
        assert!(d.profiles.is_empty());
        assert!(d.default_profile_id.is_none());
        assert!(d.installations.iter().all(|i| i.group_id.is_none()));
        assert_eq!(d.overridden_fields.len(), 5);
        assert!(d.metadata.genre.is_none());
        let directory = journal(&path).unwrap().unwrap();
        assert_eq!(
            std::fs::read(directory.join("candidate.sqlite3")).unwrap(),
            bytes
        );
        assert_eq!(std::fs::read(&backup).unwrap(), bytes);
        let expected = dump(&db).await;
        // V3 backup after a v2 restore is itself restorable with exact migration data.
        let v3 = root.path().join("upgraded.sqlite3");
        assert_eq!(
            create_backup(&db, &v3, &cancel())
                .await
                .unwrap()
                .schema_version,
            4
        );
        let staged = stage_restore(&db, &v3).await.unwrap();
        staged.confirm(&db).await.unwrap();
        drop(db);
        let db = Database::initialize(&path).await.unwrap();
        assert_eq!(dump(&db).await, expected);
        db.close().await;
        let db = Database::initialize(&path).await.unwrap();
        assert_eq!(dump(&db).await, expected);
        db.close().await;
        // A v2 candidate that fails after active-copy migration recovers a grouped v3 original.
        let other = root.path().join("recovery/library.sqlite3");
        let db = Database::initialize(&other).await.unwrap();
        populate(&db).await;
        let prior = dump(&db).await;
        assert_eq!(&prior[2..], &original[2..]);
        let staged = stage_restore(&db, &backup).await.unwrap();
        staged.confirm(&db).await.unwrap();
        drop(db);
        let db = resume_with(
            &other,
            Faults {
                fail: Some("reopened"),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(dump(&db).await, prior);
        assert_eq!(outcome(&other).unwrap(), Some("recovered"));
        db.close().await;
        assert_eq!(std::fs::read(&backup).unwrap(), bytes);
        // Synthetic historical M1.4 journal: its original is still exact v2 and
        // attempted intent must recover it by copying, never upgrading the preserved file.
        let historical = root.path().join("historical/library.sqlite3");
        let db = Database::initialize(&historical).await.unwrap();
        populate(&db).await;
        let staged = stage_restore(&db, &backup).await.unwrap();
        staged.confirm(&db).await.unwrap();
        let directory = pending_journal(&historical).unwrap().unwrap();
        drop(db);
        std::fs::copy(&backup, directory.join("previous.sqlite3")).unwrap();
        mark(&directory, "attempted").unwrap();
        let db = Database::initialize(&historical).await.unwrap();
        assert_eq!(outcome(&historical).unwrap(), Some("recovered"));
        assert_eq!(db.status().await.unwrap().schema_version, 4);
        let d = db.manual_details("g1").await.unwrap();
        assert_eq!(d.revision, 17);
        assert_eq!(d.installations.len(), 3);
        assert_eq!(d.overridden_fields.len(), 5);
        assert_eq!(d.groups.len(), if version == 3 { 1 } else { 0 });
        assert!(d.profiles.is_empty());
        assert!(d.default_profile_id.is_none());
        assert_eq!(
            std::fs::read(directory.join("previous.sqlite3")).unwrap(),
            bytes
        );
        db.close().await;
    }
}
