// SPDX-License-Identifier: GPL-3.0-or-later
use game_manager_lib::storage::{
    Database, ErrorCode, SummarySort,
    query::{Criteria, Direction, InstallationFilter, QueryRequest},
};
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
use tempfile::tempdir;

fn request(criteria: Criteria, offset: u32, limit: u32) -> QueryRequest {
    QueryRequest {
        criteria,
        offset,
        limit,
    }
}
async fn fixture() -> (tempfile::TempDir, Database) {
    let dir = tempdir().unwrap();
    let path = dir.path().join("query.sqlite3");
    let db = Database::initialize(&path).await.unwrap();
    let mut c = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&path)
            .foreign_keys(true),
    )
    .await
    .unwrap();
    for (id, title, sort, date) in [
        ("a", r#"Alpha %_ \ " O'Brien"#, "same", Some("2000-01-01")),
        ("b", "Alpha twin", "same", Some("2000-01-01")),
        ("c", "ALPHA omega", "z", None),
        ("d", "\u{e1}rcade", "d", Some("2020-12-31")),
        ("e", "\u{c1}rcade", "e", Some("2022-01-01")),
        ("f", "\u{661f}\u{306e} Voyager", "f", Some("1999-12-31")),
    ] {
        sqlx::query("INSERT INTO games (id,title,sort_title,release_date,created_at_ms,updated_at_ms) VALUES (?,?,?,?,0,0)").bind(id).bind(title).bind(sort).bind(date).execute(&mut c).await.unwrap();
    }
    sqlx::raw_sql("INSERT INTO library_sources VALUES ('s1','manual','Source One'),('s2','manual','Source Two');
        INSERT INTO library_entries VALUES ('a1','a','s1','1','playing',0),('a2','a','s2','1','playing',0),('b1','b','s1','2','playing',0),('c1','c','s2','3','playing',0);
        INSERT INTO game_installations VALUES ('i1','a1','synthetic://missing','windows','1','missing'),('i2','a2','synthetic://present','windows','1','present');").execute(&mut c).await.unwrap();
    c.close().await.unwrap();
    (dir, db)
}

#[tokio::test]
async fn literal_case_whitespace_unicode_and_punctuation_search() {
    let (_dir, db) = fixture().await;
    for (term, expected) in [
        ("", 6),
        (" \t\n", 6),
        (" ALpha ", 3),
        ("pha tw", 1),
        ("%", 1),
        ("_", 1),
        ("\\", 1),
        ("\"", 1),
        ("O'Brien", 1),
        ("' OR 1=1 --", 0),
        ("\u{e1}r", 1),
        ("\u{c1}r", 1),
        ("a\u{301}rcade", 0),
        ("\u{661f}\u{306e}", 1),
        ("Alpha  twin", 0),
    ] {
        let criteria = Criteria {
            search: term.into(),
            ..Default::default()
        };
        assert_eq!(db.query_count(&criteria).await.unwrap(), expected, "{term}");
        assert_eq!(
            db.query_window(&request(criteria, 0, 100))
                .await
                .unwrap()
                .items
                .len() as i64,
            expected
        );
    }
    let lower = db
        .query_window(&request(
            Criteria {
                search: "\u{e1}r".into(),
                ..Default::default()
            },
            0,
            100,
        ))
        .await
        .unwrap();
    assert_eq!(lower.items[0].id, "d"); // non-ASCII case is exact, not silently folded
    db.close().await;
}

#[tokio::test]
async fn identical_display_titles_remain_distinct_stable_canonical_games() {
    let (dir, db) = fixture().await;
    let mut c = SqliteConnection::connect_with(
        &SqliteConnectOptions::new().filename(dir.path().join("query.sqlite3")),
    )
    .await
    .unwrap();
    sqlx::query("INSERT INTO games (id,title,sort_title,release_date,created_at_ms,updated_at_ms) SELECT 'duplicate',title,sort_title,release_date,0,0 FROM games WHERE id='b'").execute(&mut c).await.unwrap();
    c.close().await.unwrap();
    let criteria = Criteria {
        search: "Alpha twin".into(),
        ..Default::default()
    };
    assert_eq!(db.query_count(&criteria).await.unwrap(), 2);
    let first = db
        .query_window(&request(criteria.clone(), 0, 1))
        .await
        .unwrap();
    let second = db.query_window(&request(criteria, 1, 1)).await.unwrap();
    assert_eq!(first.items[0].title, second.items[0].title);
    assert_eq!(first.items[0].id, "b");
    assert_eq!(second.items[0].id, "duplicate");
    db.close().await;
}

#[tokio::test]
async fn composed_filters_are_canonical_and_installation_is_global_to_game() {
    let (_dir, db) = fixture().await;
    let installed = Criteria {
        installed: InstallationFilter::Installed,
        ..Default::default()
    };
    assert_eq!(db.query_count(&installed).await.unwrap(), 1);
    let combined = Criteria {
        search: "alpha".into(),
        source_id: Some("s1".into()),
        year_from: Some(2000),
        year_to: Some(2000),
        ..installed
    };
    let rows = db
        .query_window(&request(combined.clone(), 0, 100))
        .await
        .unwrap()
        .items;
    assert_eq!(db.query_count(&combined).await.unwrap(), 1);
    assert_eq!(rows[0].id, "a");
    assert_eq!(rows[0].entry_count, 2);
    assert_eq!(rows[0].installation_count, 2);
    assert_eq!(rows[0].source_names, "Source One, Source Two");
    let not_installed = Criteria {
        installed: InstallationFilter::NotInstalled,
        ..Default::default()
    };
    assert_eq!(db.query_count(&not_installed).await.unwrap(), 5);
    assert_eq!(
        db.query_count(&Criteria {
            source_id: Some("s1".into()),
            ..not_installed
        })
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        db.query_count(&Criteria {
            source_id: Some("absent".into()),
            ..Default::default()
        })
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_count(&Criteria {
            year_from: Some(2000),
            year_to: Some(2000),
            ..Default::default()
        })
        .await
        .unwrap(),
        2
    );
    db.close().await;
}

#[tokio::test]
async fn stable_paging_ties_nulls_and_reverse_order() {
    let (_dir, db) = fixture().await;
    for sort in [SummarySort::Title, SummarySort::ReleaseDate] {
        let mut orders = Vec::new();
        for direction in [Direction::Asc, Direction::Desc] {
            let criteria = Criteria {
                sort,
                direction,
                ..Default::default()
            };
            let full = db
                .query_window(&request(criteria.clone(), 0, 200))
                .await
                .unwrap()
                .items;
            let ids: Vec<_> = full.iter().map(|r| r.id.clone()).collect();
            let mut pages = Vec::new();
            for offset in [0, 2, 4, 6] {
                let first = db
                    .query_window(&request(criteria.clone(), offset, 2))
                    .await
                    .unwrap()
                    .items;
                let again = db
                    .query_window(&request(criteria.clone(), offset, 2))
                    .await
                    .unwrap()
                    .items;
                assert_eq!(first, again);
                pages.extend(first.into_iter().map(|r| r.id));
            }
            assert_eq!(pages, ids);
            assert_eq!(pages.len(), 6);
            orders.push(ids);
        }
        let reverse: Vec<_> = orders[0].iter().rev().cloned().collect();
        assert_eq!(orders[1], reverse);
        if matches!(sort, SummarySort::ReleaseDate) {
            assert_eq!(orders[0], vec!["c", "f", "a", "b", "d", "e"]);
        }
    }
    db.close().await;
}

#[tokio::test]
async fn empty_no_match_and_request_validation_do_not_hide_errors() {
    let dir = tempdir().unwrap();
    let db = Database::initialize(&dir.path().join("empty.sqlite3"))
        .await
        .unwrap();
    assert_eq!(db.query_count(&Criteria::default()).await.unwrap(), 0);
    assert!(
        db.query_window(&request(Criteria::default(), 0, 100))
            .await
            .unwrap()
            .items
            .is_empty()
    );
    for req in [
        request(Criteria::default(), 0, 0),
        request(Criteria::default(), 0, 201),
        request(Criteria::default(), 1_000_001, 100),
        request(
            Criteria {
                search: "a".repeat(201),
                ..Default::default()
            },
            0,
            100,
        ),
        request(
            Criteria {
                search: "bad\0".into(),
                ..Default::default()
            },
            0,
            100,
        ),
        request(
            Criteria {
                source_id: Some("".into()),
                ..Default::default()
            },
            0,
            100,
        ),
        request(
            Criteria {
                year_from: Some(0),
                ..Default::default()
            },
            0,
            100,
        ),
        request(
            Criteria {
                year_from: Some(2022),
                year_to: Some(2000),
                ..Default::default()
            },
            0,
            100,
        ),
    ] {
        assert_eq!(
            db.query_window(&req).await.unwrap_err().code,
            ErrorCode::InvalidRequest
        );
    }
    for value in [
        r#"{"limit":100,"offset":0,"criteria":{"sort":"title; DROP TABLE games"}}"#,
        r#"{"limit":100,"offset":0,"criteria":{"direction":"sideways"}}"#,
        r#"{"limit":100,"offset":0,"criteria":{"installed":"yes"}}"#,
        r#"{"limit":100,"offset":0,"criteria":{"sql":"SELECT *"}}"#,
        r#"{"limit":100,"offset":-1,"criteria":{}}"#,
    ] {
        assert!(serde_json::from_str::<QueryRequest>(value).is_err());
    }
    db.close().await;
    assert!(db.query_count(&Criteria::default()).await.is_err());
}
