// SPDX-License-Identifier: GPL-3.0-or-later
//! Deterministic, test-only executable association model for ADR-010. No production
//! launch schema, profile service, executable discovery or process execution.
use game_manager_lib::storage::MIGRATOR;
use sqlx::{Connection, SqliteConnection};

async fn model() -> SqliteConnection {
    let mut c = SqliteConnection::connect("sqlite::memory:").await.unwrap();
    sqlx::query("PRAGMA foreign_keys=ON")
        .execute(&mut c)
        .await
        .unwrap();
    MIGRATOR.run_direct(None, &mut c, false).await.unwrap();
    sqlx::raw_sql("INSERT INTO games(id,title,sort_title,created_at_ms,updated_at_ms) VALUES ('g','Same','same',1,1),('other','Same','same',1,1);
        INSERT INTO library_sources VALUES ('s','manual','Manual'),('provider','steam','Steam');
        INSERT INTO library_entries VALUES ('e','g','s',NULL,'not_started',1),('p','g','provider','123','not_started',1),('other-e','other','s',NULL,'not_started',1);
        INSERT INTO game_installations VALUES ('i','e','opaque-one','windows',NULL,'unknown','user'),('i2','e','opaque-two','windows',NULL,'missing','user'),('pi','p','opaque-provider','windows',NULL,'unknown','legacy'),('oi','other-e','opaque-other','windows',NULL,'unknown','user');
        CREATE TABLE contract_profiles (
            id TEXT PRIMARY KEY, game_id TEXT NOT NULL REFERENCES games(id) ON DELETE RESTRICT,
            installation_id TEXT NOT NULL REFERENCES game_installations(id) ON DELETE RESTRICT,
            display_name TEXT NOT NULL, origin TEXT NOT NULL CHECK(origin IN ('user','provider')),
            source_id TEXT REFERENCES library_sources(id) ON DELETE RESTRICT, external_profile_id TEXT,
            executable_ref TEXT, arguments_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(arguments_json) AND json_type(arguments_json)='array'),
            working_directory_ref TEXT, revision INTEGER NOT NULL DEFAULT 0 CHECK(revision>=0),
            UNIQUE(id,game_id), UNIQUE(source_id,external_profile_id),
            CHECK((origin='user' AND source_id IS NULL AND external_profile_id IS NULL) OR (origin='provider' AND source_id IS NOT NULL AND external_profile_id IS NOT NULL AND length(external_profile_id)>0))
        ) STRICT;
        CREATE TRIGGER contract_profile_binding BEFORE INSERT ON contract_profiles
        WHEN NOT EXISTS(SELECT 1 FROM game_installations i JOIN library_entries e ON e.id=i.entry_id WHERE i.id=NEW.installation_id AND e.game_id=NEW.game_id AND (NEW.origin='user' OR e.source_id=NEW.source_id))
        BEGIN SELECT RAISE(ABORT,'Profile binding mismatch'); END;
        CREATE TRIGGER contract_profile_fixed_binding BEFORE UPDATE OF id,game_id,installation_id,origin,source_id,external_profile_id ON contract_profiles
        BEGIN SELECT RAISE(ABORT,'Profile binding is immutable'); END;
        CREATE TABLE contract_defaults(game_id TEXT PRIMARY KEY REFERENCES games(id) ON DELETE RESTRICT, profile_id TEXT NOT NULL, FOREIGN KEY(profile_id,game_id) REFERENCES contract_profiles(id,game_id) ON DELETE CASCADE) STRICT;")
        .execute(&mut c).await.unwrap();
    c
}

#[tokio::test]
async fn profiles_have_independent_ids_required_installations_and_provenance() {
    let mut c = model().await;
    sqlx::raw_sql("INSERT INTO contract_profiles(id,game_id,installation_id,display_name,origin,executable_ref) VALUES ('a','g','i','Vanilla','user','opaque-target'),('b','g','i','Vanilla','user','opaque-target'),('c','g','i2','Vanilla','user','opaque-target'); INSERT INTO contract_profiles(id,game_id,installation_id,display_name,origin,source_id,external_profile_id) VALUES ('provider-profile','g','pi','Vanilla','provider','provider','default');").execute(&mut c).await.unwrap();
    for sql in [
        "INSERT INTO contract_profiles(id,game_id,installation_id,display_name,origin) VALUES ('bad','other','i','Same','user')",
        "INSERT INTO contract_profiles(id,game_id,installation_id,display_name,origin) VALUES ('bad','g',NULL,'Same','user')",
        "INSERT INTO contract_profiles(id,game_id,installation_id,display_name,origin) VALUES ('bad','g','i','Same','provider')",
        "INSERT INTO contract_profiles(id,game_id,installation_id,display_name,origin,source_id,external_profile_id) VALUES ('bad','g','i','Same','provider','provider','bad')",
        "UPDATE contract_profiles SET installation_id='i2' WHERE id='a'",
    ] {
        assert!(sqlx::query(sql).execute(&mut c).await.is_err(), "{sql}");
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM contract_profiles WHERE game_id='g'")
            .fetch_one(&mut c)
            .await
            .unwrap(),
        4
    );
    sqlx::query("UPDATE game_installations SET root_path_ref='changed',detection_state='present' WHERE id='i'").execute(&mut c).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT executable_ref FROM contract_profiles WHERE id='a'"
        )
        .fetch_one(&mut c)
        .await
        .unwrap(),
        "opaque-target"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT installation_id FROM contract_profiles WHERE id='a'"
        )
        .fetch_one(&mut c)
        .await
        .unwrap(),
        "i"
    );
    let update = "UPDATE contract_profiles SET display_name=?,revision=revision+1 WHERE id='a' AND revision=?";
    assert_eq!(
        sqlx::query(update)
            .bind("Renamed")
            .bind(0)
            .execute(&mut c)
            .await
            .unwrap()
            .rows_affected(),
        1
    );
    assert_eq!(
        sqlx::query(update)
            .bind("Stale")
            .bind(0)
            .execute(&mut c)
            .await
            .unwrap()
            .rows_affected(),
        0
    );
    c.close().await.unwrap();
}

#[tokio::test]
async fn reviewed_deletion_clears_explicit_default_without_retargeting_or_orphans() {
    let mut c = model().await;
    sqlx::raw_sql("INSERT INTO contract_profiles(id,game_id,installation_id,display_name,origin) VALUES ('a','g','i','Same','user'),('b','g','i','Same','user'),('o','other','oi','Same','user');").execute(&mut c).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM contract_defaults")
            .fetch_one(&mut c)
            .await
            .unwrap(),
        0
    );
    assert!(
        sqlx::query("INSERT INTO contract_defaults VALUES ('g','o')")
            .execute(&mut c)
            .await
            .is_err()
    );
    let mut choice = c.begin().await.unwrap();
    let choose = "INSERT INTO contract_defaults(game_id,profile_id) SELECT g.id,p.id FROM games g JOIN contract_profiles p ON p.game_id=g.id WHERE g.id='g' AND p.id='a' AND g.revision=? AND p.revision=?";
    assert_eq!(
        sqlx::query(choose)
            .bind(9)
            .bind(0)
            .execute(&mut *choice)
            .await
            .unwrap()
            .rows_affected(),
        0
    );
    assert_eq!(
        sqlx::query(choose)
            .bind(0)
            .bind(9)
            .execute(&mut *choice)
            .await
            .unwrap()
            .rows_affected(),
        0
    );
    assert_eq!(
        sqlx::query(choose)
            .bind(0)
            .bind(0)
            .execute(&mut *choice)
            .await
            .unwrap()
            .rows_affected(),
        1
    );
    // The successful choice consumes the game revision in the same transaction
    // along with the default; another writer with the old token cannot replace it.
    assert_eq!(
        sqlx::query("UPDATE games SET revision=revision+1 WHERE id='g' AND revision=0")
            .execute(&mut *choice)
            .await
            .unwrap()
            .rows_affected(),
        1
    );
    assert_eq!(sqlx::query("UPDATE contract_defaults SET profile_id='b' WHERE game_id='g' AND EXISTS(SELECT 1 FROM games WHERE id='g' AND revision=0)")
        .execute(&mut *choice).await.unwrap().rows_affected(),0);
    choice.commit().await.unwrap();
    sqlx::raw_sql("INSERT INTO installation_groups VALUES ('group','g','Copies'); INSERT INTO installation_group_members VALUES ('i','group'); DELETE FROM installation_groups WHERE id='group';").execute(&mut c).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT profile_id FROM contract_defaults WHERE game_id='g'"
        )
        .fetch_one(&mut c)
        .await
        .unwrap(),
        "a"
    );
    for sql in [
        "DELETE FROM game_installations WHERE id='i'",
        "DELETE FROM library_entries WHERE id='e'",
        "DELETE FROM games WHERE id='g'",
    ] {
        assert!(sqlx::query(sql).execute(&mut c).await.is_err());
    }
    // Simulate the later explicitly reviewed metadata-deletion transaction.
    let mut tx = c.begin().await.unwrap();
    assert_eq!(
        sqlx::query("DELETE FROM contract_profiles WHERE id='a' AND revision=9")
            .execute(&mut *tx)
            .await
            .unwrap()
            .rows_affected(),
        0
    );
    assert_eq!(
        sqlx::query("DELETE FROM contract_profiles WHERE id='a' AND revision=0")
            .execute(&mut *tx)
            .await
            .unwrap()
            .rows_affected(),
        1
    );
    sqlx::query("DELETE FROM contract_profiles WHERE id='b' AND revision=0")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("DELETE FROM game_installations WHERE entry_id='e'")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("DELETE FROM library_entries WHERE id='e'")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM contract_defaults")
            .fetch_one(&mut c)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM contract_profiles WHERE id='o'")
            .fetch_one(&mut c)
            .await
            .unwrap(),
        1
    );
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_optional(&mut c)
            .await
            .unwrap()
            .is_none()
    );
    c.close().await.unwrap();
}
