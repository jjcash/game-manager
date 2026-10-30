// SPDX-License-Identifier: GPL-3.0-or-later
//! Persisted preparation and final revalidation without executing a process.
use super::{
    windows::{prepare, revalidate},
    *,
};
use crate::storage::{
    Database,
    launch_profiles::{Action as ProfileAction, Configuration, PathRef},
    manual::{self, Action, MutationRequest},
};
use sqlx::Connection;

async fn fixture() -> (tempfile::TempDir, Database, Request) {
    let root = tempfile::tempdir().unwrap();
    let db = Database::initialize(&root.path().join("library.sqlite3"))
        .await
        .unwrap();
    let game = db
        .manual_create(&manual::CreateRequest {
            metadata: manual::Metadata {
                title: "Synthetic".into(),
                release_date: None,
                genre: None,
                developer: None,
                favorite: false,
            },
            source_name: "Manual".into(),
            installation: Some(manual::InstallationInput {
                root_path_ref: "Z:\\synthetic".into(),
                platform: "windows".into(),
                version: None,
                detection_state: "unknown".into(),
            }),
        })
        .await
        .unwrap();
    let details = db.manual_details(&game).await.unwrap();
    db.manual_mutate(&MutationRequest {
        game_id: game.clone(),
        expected_revision: details.revision,
        action: Action::LaunchProfile {
            action: ProfileAction::Create {
                installation_id: details.installations[0].id.clone(),
                configuration: Configuration {
                    display_name: "Synthetic".into(),
                    launcher_kind: "standalone".into(),
                    executable_ref: PathRef::InstallationRelative("fixture.exe".into()),
                    arguments: vec!["".into(), "日本語".into()],
                    working_directory_ref: None,
                },
            },
        },
    })
    .await
    .unwrap();
    let details = db.manual_details(&game).await.unwrap();
    let request = Request {
        game_id: game,
        profile_id: details.profiles[0].id.clone(),
        expected_profile_revision: details.profiles[0].revision,
        expected_game_revision: details.revision,
    };
    (root, db, request)
}

#[tokio::test]
async fn persisted_preparation_checks_identity_revisions_purpose_and_sources() {
    let (_root, db, request) = fixture().await;
    let mut c = db.pool.acquire().await.unwrap();
    let prepared = prepare(&mut c, request.clone()).await.unwrap();
    let mut tx = c.begin_with("BEGIN IMMEDIATE").await.unwrap();
    revalidate(&mut tx, &prepared).await.unwrap();
    tx.rollback().await.unwrap();
    // A real second canonical game cannot borrow the first game's profile.
    sqlx::query("INSERT INTO games SELECT ?,title,sort_title,release_date,genre,developer,favorite,created_at_ms,updated_at_ms,origin,revision FROM games LIMIT 1")
        .bind("f".repeat(32)).execute(&mut *c).await.unwrap();
    for (mut r, code) in [
        (request.clone(), "invalidRequest"),
        (request.clone(), "missingOrUnrelated"),
        (request.clone(), "missingOrUnrelated"),
        (request.clone(), "staleContext"),
        (request.clone(), "staleContext"),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (mut r, code))| {
        match i {
            0 => r.game_id = "invalid".into(),
            1 => r.game_id = "f".repeat(32),
            2 => r.profile_id = "f".repeat(32),
            3 => r.expected_game_revision += 1,
            _ => r.expected_profile_revision += 1,
        }
        (r, code)
    }) {
        assert_eq!(prepare(&mut c, r.clone()).await.unwrap_err().code, code);
        r.expected_profile_revision = -1;
        assert_eq!(prepare(&mut c, r).await.unwrap_err().code, "invalidRequest");
    }
    sqlx::query("UPDATE library_sources SET source_type='steam'")
        .execute(&mut *c)
        .await
        .unwrap();
    assert_eq!(
        prepare(&mut c, request.clone()).await.unwrap_err().code,
        "unsupportedContext"
    );
    sqlx::query("UPDATE library_sources SET source_type='manual'")
        .execute(&mut *c)
        .await
        .unwrap();
    sqlx::query("PRAGMA ignore_check_constraints=ON")
        .execute(&mut *c)
        .await
        .unwrap();
    sqlx::query("DROP TRIGGER launch_profile_fixed_identity")
        .execute(&mut *c)
        .await
        .unwrap();
    for sql in [
        "UPDATE launch_profiles SET launcher_kind='shell'",
        "UPDATE launch_profiles SET launcher_kind='standalone', provenance='provider'",
    ] {
        sqlx::query(sql).execute(&mut *c).await.unwrap();
        assert_eq!(
            prepare(&mut c, request.clone()).await.unwrap_err().code,
            "unsupportedContext"
        );
    }
    sqlx::query("UPDATE launch_profiles SET provenance='user'")
        .execute(&mut *c)
        .await
        .unwrap();
    sqlx::query("UPDATE database_metadata SET purpose='benchmark'")
        .execute(&mut *c)
        .await
        .unwrap();
    assert_eq!(
        prepare(&mut c, request).await.unwrap_err().code,
        "wrongPurpose"
    );
    drop(c);
    db.close().await;
}

#[tokio::test]
async fn final_snapshot_rejects_detectable_edits_defaults_and_deleted_context() {
    for sql in [
        "UPDATE launch_profiles SET revision=revision+1",
        "UPDATE games SET revision=revision+1",
        "UPDATE game_installations SET version='changed without revision'",
        "UPDATE library_sources SET display_name='irrelevant name'", // source name is not execution context
        "INSERT INTO game_launch_defaults SELECT g.id,p.id FROM games g,launch_profiles p",
        "DELETE FROM launch_profiles",
    ] {
        let (_root, db, request) = fixture().await;
        let mut c = db.pool.acquire().await.unwrap();
        let prepared = prepare(&mut c, request).await.unwrap();
        sqlx::query(sql).execute(&mut *c).await.unwrap();
        let mut tx = c.begin_with("BEGIN IMMEDIATE").await.unwrap();
        let result = revalidate(&mut tx, &prepared).await;
        if sql.contains("irrelevant name") {
            result.unwrap();
        } else {
            assert!(result.is_err(), "{sql}");
        }
        tx.rollback().await.unwrap();
        drop(c);
        db.close().await;
    }
    let (_root, db, request) = fixture().await;
    let mut c = db.pool.acquire().await.unwrap();
    let prepared = prepare(&mut c, request).await.unwrap();
    sqlx::query("PRAGMA foreign_keys=OFF")
        .execute(&mut *c)
        .await
        .unwrap();
    sqlx::query("DELETE FROM game_installations")
        .execute(&mut *c)
        .await
        .unwrap();
    assert_eq!(
        revalidate(&mut c, &prepared).await.unwrap_err().code,
        "missingOrUnrelated"
    );
    drop(c);
    db.close().await;
}

#[tokio::test]
async fn restore_and_shutdown_drain_admitted_preparation_before_pool_close() {
    let (root, db, request) = fixture().await;
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let snapshot = root.path().join("backup.sqlite3");
    crate::storage::maintenance::create_backup(&db, &snapshot, &cancel)
        .await
        .unwrap();
    let candidate = crate::storage::maintenance::stage_restore(&db, &snapshot)
        .await
        .unwrap();
    let permit = db.lifecycle.as_ref().unwrap().launches.enter().unwrap();
    let mut confirm = Box::pin(candidate.confirm(&db));
    assert!(
        std::future::poll_fn(|cx| std::task::Poll::Ready(confirm.as_mut().poll(cx).is_pending()))
            .await
    );
    assert!(db.is_stopping());
    assert!(!db.pool.is_closed());
    assert_eq!(
        db.launch_standalone(request.clone())
            .await
            .unwrap_err()
            .code,
        "stopping"
    );
    drop(permit);
    tokio::time::timeout(std::time::Duration::from_secs(10), confirm)
        .await
        .unwrap()
        .unwrap();
    assert!(db.pool.is_closed());
    assert_eq!(
        db.launch_standalone(request).await.unwrap_err().code,
        "stopping"
    );
    db.close().await;
    let (_root, db, _) = fixture().await;
    let permit = db.lifecycle.as_ref().unwrap().launches.enter().unwrap();
    let mut close = Box::pin(db.close());
    assert!(
        std::future::poll_fn(|cx| std::task::Poll::Ready(close.as_mut().poll(cx).is_pending()))
            .await
    );
    assert!(!db.pool.is_closed());
    drop(permit);
    close.await;
    assert!(db.pool.is_closed());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_ipc_waiter_cannot_release_blocking_creation_or_retry_lost_success() {
    let (_root, db, request) = fixture().await;
    let (entered, started) = tokio::sync::oneshot::channel();
    let (release, blocked) = std::sync::mpsc::channel();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let permit = db.lifecycle.as_ref().unwrap().launches.enter().unwrap();
    let tx = db.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let task = tokio::spawn(create_with_reservation(tx, permit, move || {
        entered.send(()).unwrap();
        blocked
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        observed.fetch_add(1, Ordering::SeqCst);
        Ok(Created { pid: 42 })
    }));
    started.await.unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    let mut close = Box::pin(db.close());
    assert!(
        std::future::poll_fn(|cx| std::task::Poll::Ready(close.as_mut().poll(cx).is_pending()))
            .await
    );
    assert!(!db.pool.is_closed());
    assert_eq!(
        db.launch_standalone(request).await.unwrap_err().code,
        "stopping"
    );
    release.send(()).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), close)
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(db.pool.is_closed());
}
