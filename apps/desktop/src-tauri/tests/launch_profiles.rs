// SPDX-License-Identifier: GPL-3.0-or-later
use game_manager_lib::storage::{
    Database, ErrorCode, MIGRATOR,
    launch_profiles::*,
    manual::{self, MutationRequest},
};
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
use tempfile::tempdir;

fn config() -> Configuration {
    Configuration {
        display_name: "日本語 Café 🎮".into(),
        launcher_kind: "standalone".into(),
        executable_ref: PathRef::InstallationRelative("bin\\ゲーム.exe".into()),
        arguments: vec![
            "two words".into(),
            "\"quoted\"".into(),
            "".into(),
            "日本語 🎮".into(),
            "%TEMP% $HOME & |".into(),
        ],
        working_directory_ref: None,
    }
}

#[tokio::test]
async fn several_installations_and_provider_memberships_keep_user_profile_ownership() {
    let root = tempdir().unwrap();
    let path = root.path().join("db");
    let db = Database::initialize(&path).await.unwrap();
    let (game, first) = setup(&db).await;
    let mut c = raw(&path).await;
    sqlx::query("INSERT INTO library_sources VALUES ('provider','steam','Read-only fixture')")
        .execute(&mut c)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO library_entries VALUES ('provider-entry',?,'provider','123','not_started',1)",
    )
    .bind(&game)
    .execute(&mut c)
    .await
    .unwrap();
    sqlx::query("INSERT INTO game_installations VALUES ('provider-install','provider-entry','X:\\synthetic-provider','windows',NULL,'unknown','legacy')").execute(&mut c).await.unwrap();
    c.close().await.unwrap();
    for installation_id in [first, "provider-install".into()] {
        let mut configuration = config();
        if installation_id == "provider-install" {
            configuration.working_directory_ref = Some(PathRef::InstallationRelative("bin".into()));
        }
        act(
            &db,
            &game,
            Action::Create {
                installation_id,
                configuration,
            },
        )
        .await
        .unwrap();
    }
    let d = db.manual_details(&game).await.unwrap();
    assert_eq!(d.profiles.len(), 2);
    assert_eq!(
        d.profiles
            .iter()
            .find(|p| p.installation_id == "provider-install")
            .unwrap()
            .configuration
            .working_directory_ref,
        Some(PathRef::InstallationRelative("bin".into()))
    );
    assert_ne!(d.profiles[0].installation_id, d.profiles[1].installation_id);
    assert!(
        d.profiles
            .iter()
            .all(|p| p.provenance == "user" && p.game_id == game)
    );
    // Competing defaults using one game revision have one winner; the other
    // must not overwrite it even though both selected profile revisions match.
    let request = |p: &Profile| MutationRequest {
        game_id: game.clone(),
        expected_revision: d.revision,
        action: manual::Action::LaunchProfile {
            action: Action::SetDefault {
                profile_id: Some(p.id.clone()),
                expected_profile_revision: Some(p.revision),
            },
        },
    };
    let a = request(&d.profiles[0]);
    let b = request(&d.profiles[1]);
    let (a, b) = tokio::join!(db.manual_mutate(&a), db.manual_mutate(&b));
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert_eq!(a.err().or(b.err()).unwrap().code, ErrorCode::Conflict);
    let mut c = raw(&path).await;
    assert!(
        sqlx::query("UPDATE game_installations SET entry_id='provider-entry' WHERE id=?")
            .bind(
                &d.profiles
                    .iter()
                    .find(|p| p.installation_id != "provider-install")
                    .unwrap()
                    .installation_id
            )
            .execute(&mut c)
            .await
            .is_err()
    );
    c.close().await.unwrap();
    db.close().await;
}
async fn setup(db: &Database) -> (String, String) {
    let game = db
        .manual_create(&manual::CreateRequest {
            metadata: manual::Metadata {
                title: "Same".into(),
                release_date: None,
                genre: None,
                developer: None,
                favorite: false,
            },
            source_name: "Manual".into(),
            installation: Some(manual::InstallationInput {
                root_path_ref: "Z:\\synthetic-unopened".into(),
                platform: "windows".into(),
                version: None,
                detection_state: "unknown".into(),
            }),
        })
        .await
        .unwrap();
    let i = db.manual_details(&game).await.unwrap().installations[0]
        .id
        .clone();
    (game, i)
}
async fn act(db: &Database, game: &str, action: Action) -> game_manager_lib::storage::Result<()> {
    let d = db.manual_details(game).await?;
    db.manual_mutate(&MutationRequest {
        game_id: game.into(),
        expected_revision: d.revision,
        action: manual::Action::LaunchProfile { action },
    })
    .await
}
async fn raw(path: &std::path::Path) -> SqliteConnection {
    SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn identities_literal_configuration_defaults_groups_deletion_and_restart() {
    let root = tempdir().unwrap();
    let path = root.path().join("db");
    let db = Database::initialize(&path).await.unwrap();
    let (game, i) = setup(&db).await;
    let (other, j) = setup(&db).await;
    for _ in 0..2 {
        act(
            &db,
            &game,
            Action::Create {
                installation_id: i.clone(),
                configuration: config(),
            },
        )
        .await
        .unwrap();
    }
    let d = db.manual_details(&game).await.unwrap();
    let p = &d.profiles[0];
    let q = &d.profiles[1];
    assert_ne!(p.id, q.id);
    assert_eq!(p.configuration, q.configuration);
    assert_eq!(p.configuration, config());
    assert_eq!(p.provenance, "user");
    assert_eq!(p.game_id, game);
    assert_eq!(d.revision, 0);
    act(
        &db,
        &game,
        Action::SetDefault {
            profile_id: Some(p.id.clone()),
            expected_profile_revision: Some(0),
        },
    )
    .await
    .unwrap();
    act(
        &db,
        &game,
        Action::Duplicate {
            profile_id: p.id.clone(),
            expected_profile_revision: 0,
        },
    )
    .await
    .unwrap();
    assert_eq!(db.manual_details(&game).await.unwrap().profiles.len(), 3);
    let mut c = config();
    c.executable_ref = PathRef::AbsolutePath("X:\\synthetic\\ゲーム.exe".into());
    c.working_directory_ref = Some(PathRef::AbsolutePath("X:\\".into()));
    act(
        &db,
        &game,
        Action::Save {
            profile_id: p.id.clone(),
            expected_profile_revision: 0,
            configuration: c.clone(),
        },
    )
    .await
    .unwrap();
    let after = db.manual_details(&game).await.unwrap();
    assert_eq!(after.revision, 1);
    assert_eq!(after.default_profile_id, Some(p.id.clone()));
    let updated = after.profiles.iter().find(|x| x.id == p.id).unwrap();
    assert_eq!(updated.revision, 1);
    assert_eq!(updated.installation_id, i);
    assert_eq!(updated.configuration, c);
    let eid = d.installations[0].entry_id.clone();
    for action in [
        manual::Action::RemoveInstallation {
            installation_id: i.clone(),
        },
        manual::Action::RemoveGame,
    ] {
        assert_eq!(
            db.manual_mutate(&MutationRequest {
                game_id: game.clone(),
                expected_revision: 1,
                action
            })
            .await
            .unwrap_err()
            .code,
            ErrorCode::Conflict
        );
    }
    db.manual_mutate(&MutationRequest {
        game_id: game.clone(),
        expected_revision: 1,
        action: manual::Action::AddEntry {
            source_name: "Second".into(),
        },
    })
    .await
    .unwrap();
    assert_eq!(
        db.manual_mutate(&MutationRequest {
            game_id: game.clone(),
            expected_revision: 2,
            action: manual::Action::RemoveEntry {
                entry_id: eid.clone()
            }
        })
        .await
        .unwrap_err()
        .code,
        ErrorCode::Conflict
    );
    for action in [
        manual::Action::SaveInstallation {
            entry_id: eid,
            installation_id: Some(i.clone()),
            installation: manual::InstallationInput {
                root_path_ref: "Q:\\changed-location".into(),
                platform: "other".into(),
                version: Some("2".into()),
                detection_state: "missing".into(),
            },
        },
        manual::Action::SaveGroup {
            group_id: None,
            display_name: "Group".into(),
        },
    ] {
        let r = db.manual_details(&game).await.unwrap().revision;
        db.manual_mutate(&MutationRequest {
            game_id: game.clone(),
            expected_revision: r,
            action,
        })
        .await
        .unwrap();
    }
    let d = db.manual_details(&game).await.unwrap();
    db.manual_mutate(&MutationRequest {
        game_id: game.clone(),
        expected_revision: d.revision,
        action: manual::Action::RemoveGroup {
            group_id: d.groups[0].id.clone(),
        },
    })
    .await
    .unwrap();
    let d = db.manual_details(&game).await.unwrap();
    let retained = d.profiles.iter().find(|x| x.id == p.id).unwrap();
    assert_eq!(retained.configuration, c);
    assert_eq!(retained.revision, 1);
    assert_eq!(d.default_profile_id, Some(p.id.clone()));
    assert!(
        act(
            &db,
            &game,
            Action::Create {
                installation_id: j,
                configuration: config()
            }
        )
        .await
        .is_err()
    );
    act(
        &db,
        &other,
        Action::Create {
            installation_id: db.manual_details(&other).await.unwrap().installations[0]
                .id
                .clone(),
            configuration: config(),
        },
    )
    .await
    .unwrap();
    let foreign = db.manual_details(&other).await.unwrap().profiles[0]
        .id
        .clone();
    assert!(
        act(
            &db,
            &game,
            Action::SetDefault {
                profile_id: Some(foreign.clone()),
                expected_profile_revision: Some(0)
            }
        )
        .await
        .is_err()
    );
    let mut sql = raw(&path).await;
    for statement in [
        format!(
            "INSERT INTO game_launch_defaults VALUES ('{other}','{}')",
            p.id
        ),
        format!("UPDATE game_launch_defaults SET profile_id='{foreign}' WHERE game_id='{game}'"),
        format!(
            "UPDATE launch_profiles SET installation_id='{}' WHERE id='{}'",
            db.manual_details(&other).await.unwrap().installations[0].id,
            p.id
        ),
        format!(
            "UPDATE library_entries SET game_id='{other}' WHERE id='{}'",
            d.installations[0].entry_id
        ),
    ] {
        assert!(
            sqlx::query(sqlx::AssertSqlSafe(statement.as_str()))
                .execute(&mut sql)
                .await
                .is_err(),
            "{statement}"
        );
    }
    sql.close().await.unwrap();
    let ids: Vec<_> = d
        .profiles
        .iter()
        .map(|p| (p.id.clone(), p.revision))
        .collect();
    db.close().await;
    let db = Database::initialize(&path).await.unwrap();
    let d = db.manual_details(&game).await.unwrap();
    assert_eq!(
        d.profiles
            .iter()
            .map(|p| (p.id.clone(), p.revision))
            .collect::<Vec<_>>(),
        ids
    );
    act(
        &db,
        &game,
        Action::SetDefault {
            profile_id: Some(q.id.clone()),
            expected_profile_revision: Some(0),
        },
    )
    .await
    .unwrap();
    act(
        &db,
        &game,
        Action::SetDefault {
            profile_id: None,
            expected_profile_revision: None,
        },
    )
    .await
    .unwrap();
    assert!(
        db.manual_details(&game)
            .await
            .unwrap()
            .default_profile_id
            .is_none()
    );
    act(
        &db,
        &game,
        Action::SetDefault {
            profile_id: Some(p.id.clone()),
            expected_profile_revision: Some(1),
        },
    )
    .await
    .unwrap();
    let before = db.manual_details(&game).await.unwrap().revision;
    act(
        &db,
        &game,
        Action::Remove {
            profile_id: p.id.clone(),
            expected_profile_revision: 1,
        },
    )
    .await
    .unwrap();
    let after = db.manual_details(&game).await.unwrap();
    assert!(after.default_profile_id.is_none());
    assert_eq!(after.revision, before + 1);
    for p in after.profiles {
        act(
            &db,
            &game,
            Action::Remove {
                profile_id: p.id,
                expected_profile_revision: p.revision,
            },
        )
        .await
        .unwrap();
    }
    let d = db.manual_details(&game).await.unwrap();
    db.manual_mutate(&MutationRequest {
        game_id: game,
        expected_revision: d.revision,
        action: manual::Action::RemoveGame,
    })
    .await
    .unwrap();
    db.close().await;
}

#[test]
fn lexical_windows_contract_and_bounds_never_resolve_paths() {
    assert!(!config().display_name.is_ascii());
    for value in [
        "",
        "..\\escape.exe",
        "a\\..\\b",
        "C:game.exe",
        "C:\\game.exe",
        "\\root",
        "\\\\host\\game",
        "\\\\?\\C:\\game",
        "a/b",
        "a\\\\b",
        ".",
        "a.",
        "a ",
        "NUL.txt",
        "COM¹.exe",
        "x:stream",
        "x\0",
    ] {
        assert!(
            PathRef::InstallationRelative(value.into())
                .validate()
                .is_err(),
            "{value:?}"
        );
    }
    for value in [
        "C:relative",
        "\\root",
        "\\\\host\\a",
        "\\\\?\\C:\\a",
        "C:\\a\\..",
        "C:/game",
        "C:\\game\\",
        "C:\\a:stream",
        "C:\\CON.exe",
    ] {
        assert!(
            PathRef::AbsolutePath(value.into()).validate().is_err(),
            "{value:?}"
        );
    }
    for value in ["C:\\", "Z:\\日本語\\🎮.exe"] {
        PathRef::AbsolutePath(value.into()).validate().unwrap();
    }
    let mut c = config();
    c.arguments = vec!["".into(); 128];
    c.validate().unwrap();
    c.arguments.push("".into());
    assert!(c.validate().is_err());
    for args in [
        vec!["x".repeat(4097)],
        vec!["x".repeat(4096); 5],
        vec!["\0".into()],
    ] {
        let mut c = config();
        c.arguments = args;
        assert!(c.validate().is_err());
    }
    let mut c = config();
    c.launcher_kind = "steam".into();
    assert!(c.validate().is_err());
    c = config();
    c.display_name = "🎮".repeat(81);
    assert!(c.validate().is_err());
    assert!(serde_json::from_value::<Configuration>(serde_json::json!({"displayName":"x","launcherKind":"standalone","executableRef":{"kind":"shell","value":"x"},"arguments":[],"workingDirectoryRef":null})).is_err());
}

#[tokio::test]
async fn concurrent_revisions_stale_defaults_failed_writes_and_bounded_graph() {
    let root = tempdir().unwrap();
    let db = Database::initialize(&root.path().join("db")).await.unwrap();
    let (game, i) = setup(&db).await;
    act(
        &db,
        &game,
        Action::Create {
            installation_id: i.clone(),
            configuration: config(),
        },
    )
    .await
    .unwrap();
    let d = db.manual_details(&game).await.unwrap();
    let p = &d.profiles[0];
    let request = || MutationRequest {
        game_id: game.clone(),
        expected_revision: 0,
        action: manual::Action::LaunchProfile {
            action: Action::Save {
                profile_id: p.id.clone(),
                expected_profile_revision: 0,
                configuration: config(),
            },
        },
    };
    let a = request();
    let b = request();
    let (a, b) = tokio::join!(db.manual_mutate(&a), db.manual_mutate(&b));
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert_eq!(a.err().or(b.err()).unwrap().code, ErrorCode::Conflict);
    for action in [
        Action::Remove {
            profile_id: p.id.clone(),
            expected_profile_revision: 0,
        },
        Action::SetDefault {
            profile_id: Some(p.id.clone()),
            expected_profile_revision: Some(0),
        },
    ] {
        assert_eq!(
            act(&db, &game, action).await.unwrap_err().code,
            ErrorCode::Conflict
        );
    }
    let mut bad = config();
    bad.arguments = vec!["x".repeat(4097)];
    let mut unsupported = config();
    unsupported.launcher_kind = "shell".into();
    let mut traversal = config();
    traversal.executable_ref = PathRef::InstallationRelative("..\\escape.exe".into());
    for configuration in [bad, unsupported, traversal] {
        assert!(
            act(
                &db,
                &game,
                Action::Save {
                    profile_id: p.id.clone(),
                    expected_profile_revision: 1,
                    configuration
                }
            )
            .await
            .is_err()
        );
    }
    assert_eq!(
        db.manual_details(&game).await.unwrap().profiles[0].revision,
        1
    );
    let mut sql = raw(&root.path().join("db")).await;
    sqlx::query("UPDATE games SET revision=1")
        .execute(&mut sql)
        .await
        .unwrap();
    assert_eq!(
        db.manual_mutate(&MutationRequest {
            game_id: game.clone(),
            expected_revision: 0,
            action: manual::Action::LaunchProfile {
                action: Action::SetDefault {
                    profile_id: Some(p.id.clone()),
                    expected_profile_revision: Some(1)
                }
            }
        })
        .await
        .unwrap_err()
        .code,
        ErrorCode::Conflict
    );
    sql.close().await.unwrap();
    for _ in 1..100 {
        act(
            &db,
            &game,
            Action::Create {
                installation_id: i.clone(),
                configuration: config(),
            },
        )
        .await
        .unwrap();
    }
    assert!(
        act(
            &db,
            &game,
            Action::Duplicate {
                profile_id: p.id.clone(),
                expected_profile_revision: 1
            }
        )
        .await
        .is_err()
    );
    assert_eq!(db.manual_details(&game).await.unwrap().profiles.len(), 100);
    db.close().await;
}

#[tokio::test]
async fn populated_v3_migration_and_failed_v4_rollback_recover_without_identity_changes() {
    let root = tempdir().unwrap();
    let path = root.path().join("db");
    let mut c = raw(&path).await;
    let old = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Borrowed(&MIGRATOR.migrations[..3]),
        ..sqlx::migrate::Migrator::DEFAULT
    };
    old.run_direct(None, &mut c, false).await.unwrap();
    sqlx::raw_sql("INSERT INTO games(id,title,sort_title,created_at_ms,updated_at_ms,origin,revision) VALUES ('g','Same','same',123,456,'manual',17); INSERT INTO library_sources VALUES ('s','manual','Manual'); INSERT INTO library_entries VALUES ('e','g','s',NULL,'playing',123); INSERT INTO game_installations VALUES ('i','e','Z:\\synthetic','windows',NULL,'unknown','user'); INSERT INTO installation_groups VALUES ('group','g','Group'); INSERT INTO installation_group_members VALUES ('i','group'); CREATE TABLE game_launch_defaults(blocker TEXT);").execute(&mut c).await.unwrap();
    c.close().await.unwrap();
    assert_eq!(
        Database::initialize(&path).await.unwrap_err().code,
        ErrorCode::Migration
    );
    let mut c = raw(&path).await;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='launch_profiles')",
    )
    .fetch_one(&mut c)
    .await
    .unwrap();
    assert!(!exists);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM _sqlx_migrations")
            .fetch_one(&mut c)
            .await
            .unwrap(),
        3
    );
    sqlx::query("DROP TABLE game_launch_defaults")
        .execute(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    let db = Database::initialize(&path).await.unwrap();
    let d = db.manual_details("g").await.unwrap();
    assert_eq!(d.revision, 17);
    assert_eq!(d.groups[0].id, "group");
    assert_eq!(d.installations[0].id, "i");
    assert_eq!(d.installations[0].group_id.as_deref(), Some("group"));
    assert!(d.profiles.is_empty());
    db.close().await;
}
