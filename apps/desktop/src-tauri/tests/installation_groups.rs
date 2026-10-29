// SPDX-License-Identifier: GPL-3.0-or-later
use game_manager_lib::storage::{Database, ErrorCode, MIGRATOR, manual::*, query::*};
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
use tempfile::tempdir;

fn create() -> CreateRequest {
    CreateRequest {
        metadata: Metadata {
            title: "Same title".into(),
            release_date: None,
            genre: None,
            developer: None,
            favorite: false,
        },
        source_name: "Manual".into(),
        installation: None,
    }
}
fn install(location: &str) -> InstallationInput {
    InstallationInput {
        root_path_ref: location.into(),
        platform: "windows".into(),
        version: None,
        detection_state: "unknown".into(),
    }
}
async fn mutate(db: &Database, game: &str, action: Action) {
    let revision = db.manual_details(game).await.unwrap().revision;
    db.manual_mutate(&MutationRequest {
        game_id: game.into(),
        expected_revision: revision,
        action,
    })
    .await
    .unwrap();
}
async fn add_group(db: &Database, game: &str) -> String {
    assert!(!"日本語 Café 🎮".is_ascii());
    let before = db.manual_details(game).await.unwrap();
    mutate(
        db,
        game,
        Action::SaveGroup {
            group_id: None,
            display_name: "日本語 Café 🎮".into(),
        },
    )
    .await;
    db.manual_details(game)
        .await
        .unwrap()
        .groups
        .into_iter()
        .find(|g| !before.groups.iter().any(|b| b.id == g.id))
        .unwrap()
        .id
}

#[tokio::test]
async fn grouping_preserves_canonical_membership_presence_overrides_queries_and_restart() {
    let root = tempdir().unwrap();
    let path = root.path().join("library.sqlite3");
    let db = Database::initialize(&path).await.unwrap();
    let game = db.manual_create(&create()).await.unwrap();
    let other = db.manual_create(&create()).await.unwrap();
    assert_ne!(game, other);
    mutate(
        &db,
        &game,
        Action::AddEntry {
            source_name: "Second".into(),
        },
    )
    .await;
    let d = db.manual_details(&game).await.unwrap();
    for (entry, location) in [
        (0, "metadata://one"),
        (0, "metadata://two"),
        (1, "metadata://three"),
    ] {
        mutate(
            &db,
            &game,
            Action::SaveInstallation {
                entry_id: d.entries[entry].id.clone(),
                installation_id: None,
                installation: install(location),
            },
        )
        .await;
    }
    let first = add_group(&db, &game).await;
    let second = add_group(&db, &game).await;
    assert_ne!(first, second);
    let before = db.manual_details(&game).await.unwrap();
    let criteria = Criteria {
        search: "Same title".into(),
        source_id: Some(
            before
                .entries
                .iter()
                .find(|e| e.source_name == "Manual")
                .unwrap()
                .source_id
                .clone(),
        ),
        ..Default::default()
    };
    let request = QueryRequest {
        limit: 100,
        offset: 0,
        criteria: criteria.clone(),
    };
    let rows = db.query_window(&request).await.unwrap().items;
    for i in &before.installations {
        mutate(
            &db,
            &game,
            Action::AssignGroup {
                installation_id: i.id.clone(),
                group_id: Some(first.clone()),
            },
        )
        .await;
    }
    mutate(
        &db,
        &game,
        Action::SaveGroup {
            group_id: Some(first.clone()),
            display_name: "Renamed 🎮".into(),
        },
    )
    .await;
    assert_eq!(db.query_window(&request).await.unwrap().items, rows);
    assert_eq!(db.query_count(&criteria).await.unwrap(), 2);
    let d = db.manual_details(&game).await.unwrap();
    assert_eq!(d.installations.len(), 3);
    assert_eq!(d.entries.len(), 2);
    assert!(
        d.installations
            .iter()
            .all(|i| i.group_id.as_deref() == Some(&first)
                && i.detection_state == "unknown"
                && i.state_origin == "user")
    );
    assert_eq!(d.overridden_fields, before.overridden_fields);
    assert!(d.metadata.genre.is_none());
    assert!(db.manual_details(&other).await.unwrap().groups.is_empty());
    // Location edits preserve every identity and the organizational relationship.
    let i = &d.installations[0];
    mutate(
        &db,
        &game,
        Action::SaveInstallation {
            entry_id: i.entry_id.clone(),
            installation_id: Some(i.id.clone()),
            installation: install("metadata://changed-location"),
        },
    )
    .await;
    mutate(
        &db,
        &game,
        Action::AssignGroup {
            installation_id: i.id.clone(),
            group_id: None,
        },
    )
    .await;
    let expected = serde_json::to_value(db.manual_details(&game).await.unwrap()).unwrap();
    db.close().await;
    let db = Database::initialize(&path).await.unwrap();
    assert_eq!(
        serde_json::to_value(db.manual_details(&game).await.unwrap()).unwrap(),
        expected
    );
    mutate(&db, &game, Action::RemoveGroup { group_id: first }).await;
    let d = db.manual_details(&game).await.unwrap();
    assert_eq!(d.groups[0].id, second);
    assert!(d.installations.iter().all(|i| i.group_id.is_none()));
    assert_eq!(db.query_window(&request).await.unwrap().items, rows);
    // Empty groups survive membership removal; only explicit group/game removal deletes them.
    for i in &d.installations {
        mutate(
            &db,
            &game,
            Action::AssignGroup {
                installation_id: i.id.clone(),
                group_id: Some(second.clone()),
            },
        )
        .await;
    }
    mutate(
        &db,
        &game,
        Action::RemoveInstallation {
            installation_id: d.installations[0].id.clone(),
        },
    )
    .await;
    mutate(
        &db,
        &game,
        Action::RemoveEntry {
            entry_id: d.entries[0].id.clone(),
        },
    )
    .await;
    assert_eq!(db.manual_details(&game).await.unwrap().groups.len(), 1);
    mutate(&db, &game, Action::RemoveGame).await;
    db.validate().await.unwrap();
    assert_eq!(db.counts().await.unwrap().games, 1);
    db.close().await;
    let mut c = SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(path))
        .await
        .unwrap();
    for table in ["installation_groups", "installation_group_members"] {
        let n: i64 =
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
                .fetch_one(&mut c)
                .await
                .unwrap();
        assert_eq!(n, 0);
    }
    c.close().await.unwrap();
}

#[tokio::test]
async fn group_conflicts_validation_bounds_and_cross_game_ids_are_atomic() {
    let root = tempdir().unwrap();
    let path = root.path().join("library.sqlite3");
    let db = Database::initialize(&path).await.unwrap();
    let game = db.manual_create(&create()).await.unwrap();
    let other = db.manual_create(&create()).await.unwrap();
    let group = add_group(&db, &game).await;
    let foreign = add_group(&db, &other).await;
    let d = db.manual_details(&game).await.unwrap();
    mutate(
        &db,
        &game,
        Action::SaveInstallation {
            entry_id: d.entries[0].id.clone(),
            installation_id: None,
            installation: install("never-accessed"),
        },
    )
    .await;
    let d = db.manual_details(&game).await.unwrap();
    for action in [
        Action::AssignGroup {
            installation_id: d.installations[0].id.clone(),
            group_id: Some(foreign.clone()),
        },
        Action::RemoveGroup { group_id: foreign },
        Action::SaveGroup {
            group_id: Some("missing".into()),
            display_name: "Name".into(),
        },
        Action::AssignGroup {
            installation_id: "missing".into(),
            group_id: Some(group.clone()),
        },
    ] {
        assert_eq!(
            db.manual_mutate(&MutationRequest {
                game_id: game.clone(),
                expected_revision: d.revision,
                action
            })
            .await
            .unwrap_err()
            .code,
            ErrorCode::NotFound
        );
        assert_eq!(db.manual_details(&game).await.unwrap().revision, d.revision);
    }
    for name in ["".into(), " ".into(), "bad\0name".into(), "x".repeat(81)] {
        assert_eq!(
            db.manual_mutate(&MutationRequest {
                game_id: game.clone(),
                expected_revision: d.revision,
                action: Action::SaveGroup {
                    group_id: None,
                    display_name: name
                }
            })
            .await
            .unwrap_err()
            .code,
            ErrorCode::InvalidRequest
        );
    }
    let a = MutationRequest {
        game_id: game.clone(),
        expected_revision: d.revision,
        action: Action::SaveGroup {
            group_id: Some(group.clone()),
            display_name: "A".into(),
        },
    };
    let b = MutationRequest {
        game_id: game.clone(),
        expected_revision: d.revision,
        action: Action::RemoveGroup { group_id: group },
    };
    let (a, b) = tokio::join!(db.manual_mutate(&a), db.manual_mutate(&b));
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert_eq!(
        a.err().or_else(|| b.err()).unwrap().code,
        ErrorCode::Conflict
    );
    for _ in db.manual_details(&game).await.unwrap().groups.len()..100 {
        add_group(&db, &game).await;
    }
    let revision = db.manual_details(&game).await.unwrap().revision;
    assert_eq!(
        db.manual_mutate(&MutationRequest {
            game_id: game.clone(),
            expected_revision: revision,
            action: Action::SaveGroup {
                group_id: None,
                display_name: "Rolled back".into()
            }
        })
        .await
        .unwrap_err()
        .code,
        ErrorCode::InvalidRequest
    );
    assert_eq!(db.manual_details(&game).await.unwrap().groups.len(), 100);
    assert_eq!(db.manual_details(&game).await.unwrap().revision, revision);
    db.close().await;
}

#[tokio::test]
async fn populated_v2_upgrade_failure_rolls_back_then_preserves_legacy_and_provider_records() {
    let root = tempdir().unwrap();
    let path = root.path().join("library.sqlite3");
    let mut c = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .foreign_keys(true),
    )
    .await
    .unwrap();
    let old = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Borrowed(&MIGRATOR.migrations[..2]),
        ..sqlx::migrate::Migrator::DEFAULT
    };
    old.run_direct(None, &mut c, false).await.unwrap();
    sqlx::raw_sql("INSERT INTO games VALUES ('legacy','Title','title',NULL,NULL,NULL,0,12,34,'legacy',7); INSERT INTO library_sources VALUES ('s','steam','Steam'); INSERT INTO library_entries VALUES ('e','legacy','s','123','playing',12); INSERT INTO game_installations VALUES ('i','e','opaque-only','windows',NULL,'present','legacy'); INSERT INTO game_field_overrides VALUES ('legacy','genre',NULL); CREATE TABLE installation_group_members(block_upgrade TEXT);").execute(&mut c).await.unwrap();
    c.close().await.unwrap();
    assert_eq!(
        Database::initialize(&path).await.unwrap_err().code,
        ErrorCode::Migration
    );
    let mut c = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&path)
            .foreign_keys(true),
    )
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT max(version) FROM _sqlx_migrations")
            .fetch_one(&mut c)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM sqlite_schema WHERE name='installation_groups'"
        )
        .fetch_one(&mut c)
        .await
        .unwrap(),
        0
    );
    sqlx::query("DROP TABLE installation_group_members")
        .execute(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    let db = Database::initialize(&path).await.unwrap();
    let d = db.manual_details("legacy").await.unwrap();
    assert_eq!(d.revision, 7);
    assert_eq!(d.origin, "legacy");
    assert_eq!(d.entries[0].source_type, "steam");
    assert_eq!(d.installations[0].id, "i");
    assert_eq!(d.installations[0].state_origin, "legacy");
    assert!(d.groups.is_empty());
    let gid = add_group(&db, "legacy").await;
    mutate(
        &db,
        "legacy",
        Action::AssignGroup {
            installation_id: "i".into(),
            group_id: Some(gid.clone()),
        },
    )
    .await;
    let d = db.manual_details("legacy").await.unwrap();
    assert_eq!(d.installations[0].group_id.as_deref(), Some(gid.as_str()));
    assert_eq!(d.installations[0].detection_state, "present");
    assert_eq!(d.installations[0].state_origin, "legacy");
    assert_eq!(d.overridden_fields, vec!["genre"]);
    assert!(d.metadata.genre.is_none());
    assert_eq!(
        db.manual_mutate(&MutationRequest {
            game_id: "legacy".into(),
            expected_revision: d.revision,
            action: Action::RemoveInstallation {
                installation_id: "i".into()
            }
        })
        .await
        .unwrap_err()
        .code,
        ErrorCode::Conflict
    );
    db.validate().await.unwrap();
    db.close().await;
}

#[tokio::test]
async fn sqlite_constraints_reject_cross_game_assignment_and_group_reparenting() {
    let root = tempdir().unwrap();
    let path = root.path().join("library.sqlite3");
    let db = Database::initialize(&path).await.unwrap();
    let game = db.manual_create(&create()).await.unwrap();
    let other = db.manual_create(&create()).await.unwrap();
    let group = add_group(&db, &game).await;
    let foreign = add_group(&db, &other).await;
    let d = db.manual_details(&game).await.unwrap();
    mutate(
        &db,
        &game,
        Action::SaveInstallation {
            entry_id: d.entries[0].id.clone(),
            installation_id: None,
            installation: install("opaque"),
        },
    )
    .await;
    let iid = db.manual_details(&game).await.unwrap().installations[0]
        .id
        .clone();
    db.close().await;
    let mut c = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&path)
            .foreign_keys(true),
    )
    .await
    .unwrap();
    assert!(
        sqlx::query("INSERT INTO installation_group_members VALUES (?,?)")
            .bind(&iid)
            .bind(&foreign)
            .execute(&mut c)
            .await
            .is_err()
    );
    sqlx::query("INSERT INTO installation_group_members VALUES (?,?)")
        .bind(&iid)
        .bind(&group)
        .execute(&mut c)
        .await
        .unwrap();
    assert!(
        sqlx::query("UPDATE installation_group_members SET group_id=? WHERE installation_id=?")
            .bind(&foreign)
            .bind(&iid)
            .execute(&mut c)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE installation_groups SET game_id=? WHERE id=?")
            .bind(&other)
            .bind(&group)
            .execute(&mut c)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM library_entries WHERE game_id=?")
            .bind(&game)
            .execute(&mut c)
            .await
            .is_err()
    );
    sqlx::query("DELETE FROM game_installations WHERE id=?")
        .bind(&iid)
        .execute(&mut c)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM installation_group_members")
            .fetch_one(&mut c)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM installation_groups")
            .fetch_one(&mut c)
            .await
            .unwrap(),
        2
    );
    c.close().await.unwrap();
}
