// SPDX-License-Identifier: GPL-3.0-or-later
use game_manager_lib::storage::{Database, ErrorCode, manual::*, query::*};
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
use tempfile::tempdir;

fn values(title: &str) -> Metadata {
    Metadata {
        title: title.into(),
        release_date: None,
        genre: None,
        developer: None,
        favorite: false,
    }
}
fn create(title: &str) -> CreateRequest {
    CreateRequest {
        metadata: values(title),
        source_name: "Manual".into(),
        installation: None,
    }
}

#[tokio::test]
async fn bounded_relationships_duplicate_locations_and_concurrent_edits() {
    let root = tempdir().unwrap();
    let path = root.path().join("app.sqlite3");
    let db = Database::initialize(&path).await.unwrap();
    let game = db.manual_create(&create("Bounded")).await.unwrap();
    let d = db.manual_details(&game).await.unwrap();
    let eid = d.entries[0].id.clone();
    mutate(
        &db,
        &game,
        Action::SaveInstallation {
            entry_id: eid.clone(),
            installation_id: None,
            installation: install("same", "unknown"),
        },
    )
    .await;
    let d = db.manual_details(&game).await.unwrap();
    assert_eq!(
        db.manual_mutate(&MutationRequest {
            game_id: game.clone(),
            expected_revision: d.revision,
            action: Action::SaveInstallation {
                entry_id: eid,
                installation_id: None,
                installation: install("same", "present")
            }
        })
        .await
        .unwrap_err()
        .code,
        ErrorCode::Conflict
    );
    let a = MutationRequest {
        game_id: game.clone(),
        expected_revision: d.revision,
        action: Action::Metadata {
            metadata: values("Concurrent A"),
        },
    };
    let b = MutationRequest {
        game_id: game.clone(),
        expected_revision: d.revision,
        action: Action::Metadata {
            metadata: values("Concurrent B"),
        },
    };
    let (a, b) = tokio::join!(db.manual_mutate(&a), db.manual_mutate(&b));
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert_eq!(
        a.err().or_else(|| b.err()).unwrap().code,
        ErrorCode::Conflict
    );
    for _ in 1..100 {
        mutate(
            &db,
            &game,
            Action::AddEntry {
                source_name: "Manual".into(),
            },
        )
        .await;
    }
    let d = db.manual_details(&game).await.unwrap();
    assert_eq!(d.entries.len(), 100);
    assert!(
        db.manual_mutate(&MutationRequest {
            game_id: game.clone(),
            expected_revision: d.revision,
            action: Action::AddEntry {
                source_name: "Not committed".into()
            }
        })
        .await
        .is_err()
    );
    assert_eq!(db.manual_details(&game).await.unwrap().entries.len(), 100);
    assert_eq!(db.counts().await.unwrap().library_sources, 1);
    db.validate().await.unwrap();
    db.close().await;
}

#[cfg(feature = "benchmarks")]
#[tokio::test]
async fn manual_commands_refuse_frozen_benchmark_databases() {
    use game_manager_lib::storage::synthetic::{self, DatasetConfig};
    let root = tempdir().unwrap();
    let path = root.path().join("benchmark.sqlite3");
    synthetic::generate(
        &path,
        DatasetConfig {
            games: 500,
            seed: 42,
        },
    )
    .await
    .unwrap();
    let before = std::fs::read(&path).unwrap();
    let db = Database::benchmark_view(&path).await.unwrap();
    assert_eq!(
        db.manual_create(&create("Forbidden"))
            .await
            .unwrap_err()
            .code,
        ErrorCode::WrongPurpose
    );
    assert_eq!(
        db.manual_details("any").await.unwrap_err().code,
        ErrorCode::WrongPurpose
    );
    assert_eq!(
        db.manual_mutate(&MutationRequest {
            game_id: "any".into(),
            expected_revision: 0,
            action: Action::RemoveGame
        })
        .await
        .unwrap_err()
        .code,
        ErrorCode::WrongPurpose
    );
    db.close().await;
    assert_eq!(before, std::fs::read(&path).unwrap());
}
fn install(path: &str, state: &str) -> InstallationInput {
    InstallationInput {
        root_path_ref: path.into(),
        platform: "windows".into(),
        version: None,
        detection_state: state.into(),
    }
}
async fn mutate(db: &Database, game: &str, action: Action) {
    let d = db.manual_details(game).await.unwrap();
    db.manual_mutate(&MutationRequest {
        game_id: game.into(),
        expected_revision: d.revision,
        action,
    })
    .await
    .unwrap();
}
async fn raw(path: &std::path::Path) -> SqliteConnection {
    SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .foreign_keys(true),
    )
    .await
    .unwrap()
}
#[tokio::test]
async fn validation_duplicate_unicode_nulls_and_atomic_creation() {
    let root = tempdir().unwrap();
    let path = root.path().join("app.sqlite3");
    let db = Database::initialize(&path).await.unwrap();
    for title in ["", " \t\n", "bad\0title", &"x".repeat(201)] {
        assert_eq!(
            db.manual_create(&create(title)).await.unwrap_err().code,
            ErrorCode::InvalidRequest
        );
    }
    for date in [
        "2023-02-29",
        "2024-02-30",
        "0000-01-01",
        "2024-13-01",
        "2024-01-00",
        "😃-02-01",
        "2024-2-01",
    ] {
        let mut r = create("date");
        r.metadata.release_date = Some(date.into());
        assert!(db.manual_create(&r).await.is_err());
    }
    let mut bad = create("atomic");
    bad.installation = Some(install("", "present"));
    assert!(db.manual_create(&bad).await.is_err());
    assert_eq!(db.counts().await.unwrap().games, 0);
    let a = db
        .manual_create(&create("日本語 • Café %_ ' 🎮"))
        .await
        .unwrap();
    let b = db
        .manual_create(&create("日本語 • Café %_ ' 🎮"))
        .await
        .unwrap();
    assert_ne!(a, b);
    let d = db.manual_details(&a).await.unwrap();
    assert_eq!(d.entries.len(), 1);
    assert!(d.installations.is_empty());
    assert_eq!(d.overridden_fields.len(), 5);
    assert_eq!(d.origin, "manual");
    let mut good = create("leap");
    good.metadata.release_date = Some("2024-02-29".into());
    db.manual_create(&good).await.unwrap();
    db.validate().await.unwrap();
    db.close().await;
}
#[tokio::test]
async fn memberships_global_presence_overrides_and_restart_removals() {
    let root = tempdir().unwrap();
    let path = root.path().join("app.sqlite3");
    let db = Database::initialize(&path).await.unwrap();
    let game = db.manual_create(&create("Original")).await.unwrap();
    mutate(
        &db,
        &game,
        Action::AddEntry {
            source_name: "Collection B".into(),
        },
    )
    .await;
    let d = db.manual_details(&game).await.unwrap();
    let a = &d.entries[0];
    let b = &d.entries[1];
    let a_id = a.id.clone();
    let b_id = b.id.clone();
    let source = a.source_id.clone();
    mutate(
        &db,
        &game,
        Action::SaveInstallation {
            entry_id: b_id.clone(),
            installation_id: None,
            installation: install("Z:\\not-accessed\\one", "present"),
        },
    )
    .await;
    mutate(
        &db,
        &game,
        Action::SaveInstallation {
            entry_id: b_id.clone(),
            installation_id: None,
            installation: install("Z:\\not-accessed\\two", "unknown"),
        },
    )
    .await;
    let c = Criteria {
        source_id: Some(source),
        installed: InstallationFilter::Installed,
        ..Default::default()
    };
    assert_eq!(db.query_count(&c).await.unwrap(), 1);
    let mut metadata = values("User title");
    metadata.favorite = true;
    metadata.genre = Some("RPG".into());
    mutate(&db, &game, Action::Metadata { metadata }).await;
    mutate(
        &db,
        &game,
        Action::UpdateEntry {
            entry_id: a_id.clone(),
            completion_status: "completed".into(),
        },
    )
    .await;
    assert_eq!(
        db.manual_details(&game).await.unwrap().metadata.title,
        "User title"
    );
    db.close().await;
    let db = Database::initialize(&path).await.unwrap();
    let d = db.manual_details(&game).await.unwrap();
    assert_eq!(d.id, game);
    assert_eq!(d.metadata.genre.as_deref(), Some("RPG"));
    assert_eq!(d.installations.len(), 2);
    assert!(d.installations.iter().all(|i| i.state_origin == "user"));
    let i = d
        .installations
        .iter()
        .find(|i| i.detection_state == "present")
        .unwrap();
    mutate(
        &db,
        &game,
        Action::SaveInstallation {
            entry_id: b_id.clone(),
            installation_id: Some(i.id.clone()),
            installation: install(&i.root_path_ref, "missing"),
        },
    )
    .await;
    assert_eq!(db.query_count(&c).await.unwrap(), 0);
    mutate(&db, &game, Action::RemoveEntry { entry_id: b_id }).await;
    let d = db.manual_details(&game).await.unwrap();
    assert!(d.installations.is_empty());
    assert_eq!(d.entries[0].id, a_id);
    assert!(
        db.manual_mutate(&MutationRequest {
            game_id: game.clone(),
            expected_revision: d.revision,
            action: Action::RemoveEntry { entry_id: a_id }
        })
        .await
        .is_err()
    );
    mutate(&db, &game, Action::RemoveGame).await;
    db.close().await;
    let db = Database::initialize(&path).await.unwrap();
    assert_eq!(db.counts().await.unwrap().games, 0);
    assert_eq!(
        db.manual_details(&game).await.unwrap_err().code,
        ErrorCode::NotFound
    );
    db.validate().await.unwrap();
    db.close().await;
}
#[tokio::test]
async fn provider_relationships_are_protected_and_do_not_erase_user_overrides() {
    let root = tempdir().unwrap();
    let path = root.path().join("app.sqlite3");
    let db = Database::initialize(&path).await.unwrap();
    let game = db.manual_create(&create("User title")).await.unwrap();
    let mut c = raw(&path).await;
    sqlx::query(
        "INSERT INTO library_sources VALUES ('provider','future-provider','Future provider')",
    )
    .execute(&mut c)
    .await
    .unwrap();
    sqlx::query("INSERT INTO library_entries(id,game_id,source_id,external_item_id,added_at_ms) VALUES ('external',?,'provider','external-id',0)").bind(&game).execute(&mut c).await.unwrap();
    sqlx::query("INSERT INTO game_installations(id,entry_id,root_path_ref,platform,detection_state) VALUES ('external-copy','external','never-accessed','windows','present')").execute(&mut c).await.unwrap();
    c.close().await.unwrap();
    for action in [
        Action::RemoveGame,
        Action::RemoveEntry {
            entry_id: "external".into(),
        },
        Action::UpdateEntry {
            entry_id: "external".into(),
            completion_status: "completed".into(),
        },
        Action::RemoveInstallation {
            installation_id: "external-copy".into(),
        },
    ] {
        let d = db.manual_details(&game).await.unwrap();
        assert_eq!(
            db.manual_mutate(&MutationRequest {
                game_id: game.clone(),
                expected_revision: d.revision,
                action
            })
            .await
            .unwrap_err()
            .code,
            ErrorCode::Conflict
        );
    }
    mutate(
        &db,
        &game,
        Action::AddEntry {
            source_name: "Another manual membership".into(),
        },
    )
    .await;
    let d = db.manual_details(&game).await.unwrap();
    assert_eq!(d.metadata.title, "User title");
    assert_eq!(d.overridden_fields.len(), 5);
    assert_eq!(d.installations[0].id, "external-copy");
    assert_eq!(d.entries.len(), 3);
    db.validate().await.unwrap();
    db.close().await;
}
#[tokio::test]
async fn stale_edits_failed_transactions_and_missing_records_preserve_state() {
    let root = tempdir().unwrap();
    let path = root.path().join("app.sqlite3");
    let db = Database::initialize(&path).await.unwrap();
    let mut request = create("Keep");
    request.installation = Some(install("never-accessed", "present"));
    let game = db.manual_create(&request).await.unwrap();
    mutate(
        &db,
        &game,
        Action::Metadata {
            metadata: values("Saved"),
        },
    )
    .await;
    assert_eq!(
        db.manual_mutate(&MutationRequest {
            game_id: game.clone(),
            expected_revision: 0,
            action: Action::Metadata {
                metadata: values("Stale")
            }
        })
        .await
        .unwrap_err()
        .code,
        ErrorCode::Conflict
    );
    let mut c = raw(&path).await;
    sqlx::raw_sql("CREATE TRIGGER fail_entry BEFORE INSERT ON library_entries BEGIN SELECT RAISE(ABORT,'injected failure'); END;").execute(&mut c).await.unwrap();
    assert!(
        db.manual_create(&create("Rollback whole creation"))
            .await
            .is_err()
    );
    let d = db.manual_details(&game).await.unwrap();
    assert!(
        db.manual_mutate(&MutationRequest {
            game_id: game.clone(),
            expected_revision: d.revision,
            action: Action::AddEntry {
                source_name: "Rolled back source".into()
            }
        })
        .await
        .is_err()
    );
    assert_eq!(db.manual_details(&game).await.unwrap().revision, d.revision);
    assert_eq!(db.counts().await.unwrap().library_sources, 1);
    sqlx::raw_sql("DROP TRIGGER fail_entry; CREATE TRIGGER fail_delete BEFORE DELETE ON games BEGIN SELECT RAISE(ABORT,'injected delete failure'); END;").execute(&mut c).await.unwrap();
    assert!(
        db.manual_mutate(&MutationRequest {
            game_id: game.clone(),
            expected_revision: d.revision,
            action: Action::RemoveGame
        })
        .await
        .is_err()
    );
    assert_eq!(db.manual_details(&game).await.unwrap().entries.len(), 1);
    assert_eq!(
        db.manual_details(&game)
            .await
            .unwrap()
            .overridden_fields
            .len(),
        5
    );
    assert_eq!(
        db.manual_details(&game).await.unwrap().installations.len(),
        1
    );
    sqlx::raw_sql("DROP TRIGGER fail_delete")
        .execute(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    let err = db
        .manual_mutate(&MutationRequest {
            game_id: game.clone(),
            expected_revision: d.revision,
            action: Action::RemoveInstallation {
                installation_id: "gone".into(),
            },
        })
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);
    assert_eq!(db.counts().await.unwrap().games, 1);
    assert_eq!(
        db.manual_details(&game).await.unwrap().metadata.title,
        "Saved"
    );
    db.validate().await.unwrap();
    db.close().await;
}
#[tokio::test]
async fn populated_v1_upgrade_preserves_identity_and_failed_upgrade_rolls_back() {
    let root = tempdir().unwrap();
    let path = root.path().join("app.sqlite3");
    let mut c = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    let old = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Borrowed(
            &game_manager_lib::storage::MIGRATOR.migrations[..1],
        ),
        ..sqlx::migrate::Migrator::DEFAULT
    };
    old.run(&mut c).await.unwrap();
    sqlx::raw_sql("INSERT INTO games VALUES ('old','Old','old',NULL,NULL,NULL,0,1,2); INSERT INTO library_sources VALUES ('manual','manual','Manual'); INSERT INTO library_entries VALUES ('entry','old','manual',NULL,'not_started',1); CREATE TABLE game_field_overrides(block_upgrade TEXT);").execute(&mut c).await.unwrap();
    c.close().await.unwrap();
    assert_eq!(
        Database::initialize(&path).await.unwrap_err().code,
        ErrorCode::Migration
    );
    let mut c = raw(&path).await;
    let columns: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info('games')")
        .fetch_all(&mut c)
        .await
        .unwrap();
    assert!(!columns.contains(&"origin".into()));
    sqlx::query("DROP TABLE game_field_overrides")
        .execute(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    let db = Database::initialize(&path).await.unwrap();
    let d = db.manual_details("old").await.unwrap();
    assert_eq!(d.origin, "legacy");
    assert_eq!(d.entries[0].id, "entry");
    assert!(d.overridden_fields.is_empty());
    mutate(
        &db,
        "old",
        Action::Metadata {
            metadata: values("Explicit override"),
        },
    )
    .await;
    db.close().await;
    let db = Database::initialize(&path).await.unwrap();
    assert_eq!(
        db.manual_details("old").await.unwrap().metadata.title,
        "Explicit override"
    );
    db.validate().await.unwrap();
    db.close().await;
}
#[tokio::test]
async fn pagination_counts_sort_ties_and_concurrent_bounded_reads_after_mutations() {
    let root = tempdir().unwrap();
    let path = root.path().join("app.sqlite3");
    let db = Database::initialize(&path).await.unwrap();
    let mut ids = vec![];
    for _ in 0..102 {
        ids.push(db.manual_create(&create("Tie")).await.unwrap());
    }
    ids.sort();
    let req = |offset| QueryRequest {
        limit: 100,
        offset,
        criteria: Criteria::default(),
    };
    let first = db.query_window(&req(0)).await.unwrap();
    assert_eq!(
        first.items.iter().map(|r| r.id.clone()).collect::<Vec<_>>(),
        ids[..100]
    );
    let read_request = req(100);
    let mutation = MutationRequest {
        game_id: ids[0].clone(),
        expected_revision: 0,
        action: Action::RemoveGame,
    };
    let (read, write) = tokio::join!(db.query_window(&read_request), db.manual_mutate(&mutation));
    assert!(read.unwrap().items.len() <= 2);
    write.unwrap();
    assert_eq!(db.query_count(&Criteria::default()).await.unwrap(), 101);
    assert_eq!(db.query_window(&req(100)).await.unwrap().items.len(), 1);
    mutate(
        &db,
        &ids[1],
        Action::Metadata {
            metadata: values("ZZ edited"),
        },
    )
    .await;
    assert_eq!(
        db.query_window(&req(100)).await.unwrap().items[0].id,
        ids[1]
    );
    assert_eq!(
        db.query_count(&Criteria {
            search: "Tie".into(),
            ..Default::default()
        })
        .await
        .unwrap(),
        100
    );
    db.validate().await.unwrap();
    db.close().await;
}
