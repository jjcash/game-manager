// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use crate::storage::{Database, maintenance};
use sqlx::{Connection, SqliteConnection};
use tempfile::tempdir;

fn image(path: &std::path::Path, format: ImageFormat, color: u8) {
    image::RgbImage::from_pixel(80, 45, image::Rgb([color, 20, 30]))
        .save_with_format(path, format)
        .unwrap();
}
async fn setup(root: &std::path::Path) -> (Database, String) {
    let db = Database::initialize(&root.join("library.sqlite3"))
        .await
        .unwrap();
    let id = db
        .manual_create(&crate::storage::manual::CreateRequest {
            metadata: crate::storage::manual::Metadata {
                title: "Artwork fixture".into(),
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
    (db, id)
}
fn service(db: &Database) -> Arc<Service> {
    db.lifecycle.as_ref().unwrap().artwork.clone()
}
async fn import(db: &Database, id: &str, path: PathBuf) -> Result<()> {
    let s = service(db);
    let revision = db.manual_details(id).await.unwrap().revision;
    s.import(
        db.pool.clone(),
        Budget::new(false),
        Request {
            game_id: id.into(),
            expected_revision: revision,
        },
        path,
        s.admit(true)?,
    )
    .await
}
async fn thumb(db: &Database, id: &str) -> Reply {
    service(db)
        .thumbnail(db.pool.clone(), Budget::new(false), id.into())
        .await
}

#[tokio::test]
async fn durable_replacement_clear_missing_corrupt_restart_and_cache_loss() {
    let root = tempdir().unwrap();
    let source = root.path().join("source.png");
    image(&source, ImageFormat::Png, 10);
    let external = std::fs::read(&source).unwrap();
    let (db, id) = setup(root.path()).await;
    import(&db, &id, source.clone()).await.unwrap();
    let first = db.manual_details(&id).await.unwrap();
    assert_eq!(first.revision, 1);
    let first_asset = first.artwork_asset_id.unwrap();
    assert_eq!(std::fs::read(&source).unwrap(), external);
    std::fs::remove_file(&source).unwrap();
    assert_eq!(thumb(&db, &id).await.status, "ready");
    image(&source, ImageFormat::Jpeg, 90);
    import(&db, &id, source.clone()).await.unwrap();
    let details = db.manual_details(&id).await.unwrap();
    let second = details.artwork_asset_id.unwrap();
    assert_ne!(first_asset, second);
    let s = service(&db);
    assert!(
        s.root
            .join("originals")
            .join(format!("{first_asset}.img"))
            .exists()
    );
    assert_eq!(thumb(&db, &id).await.status, "ready");
    db.close().await;
    let db = Database::initialize(&root.path().join("library.sqlite3"))
        .await
        .unwrap();
    assert_eq!(thumb(&db, &id).await.cache, "disk");
    // Delete only the test's known derived slot; restart must regenerate it.
    let cache = service(&db).root.join("thumbnails").join(format!(
        "thumb-v1-{:03}.bin",
        u8::from_str_radix(&second[..2], 16).unwrap()
    ));
    std::fs::remove_file(cache).unwrap();
    db.close().await;
    let db = Database::initialize(&root.path().join("library.sqlite3"))
        .await
        .unwrap();
    assert_eq!(thumb(&db, &id).await.cache, "miss");
    let original = service(&db)
        .root
        .join("originals")
        .join(format!("{second}.img"));
    std::fs::write(&original, b"corrupt").unwrap();
    assert!(thumb(&db, &id).await.data_url.is_none());
    std::fs::remove_file(&original).unwrap();
    assert!(thumb(&db, &id).await.data_url.is_none());
    Service::assign(
        &db.pool,
        &Request {
            game_id: id.clone(),
            expected_revision: 2,
        },
        None,
    )
    .await
    .unwrap();
    assert_eq!(db.manual_details(&id).await.unwrap().revision, 3);
    assert_eq!(thumb(&db, &id).await.status, "missing");
    assert!(
        service(&db)
            .root
            .join("originals")
            .join(format!("{first_asset}.img"))
            .exists()
    );
    db.close().await;
}

#[tokio::test]
async fn failures_stale_deleted_legacy_and_rollback_preserve_old_assignment_and_sources() {
    let root = tempdir().unwrap();
    let source = root.path().join("source.png");
    image(&source, ImageFormat::Png, 15);
    let (db, id) = setup(root.path()).await;
    import(&db, &id, source.clone()).await.unwrap();
    let old = db.manual_details(&id).await.unwrap().artwork_asset_id;
    let s = service(&db);
    let asset = s.publish(&source, "a".repeat(32), None).unwrap();
    assert_eq!(
        Service::assign(
            &db.pool,
            &Request {
                game_id: id.clone(),
                expected_revision: 0
            },
            Some(asset)
        )
        .await
        .unwrap_err()
        .code,
        "stale"
    );
    std::fs::write(&source, b"not an image").unwrap();
    assert!(import(&db, &id, source.clone()).await.is_err());
    assert_eq!(db.manual_details(&id).await.unwrap().artwork_asset_id, old);
    assert_eq!(std::fs::read(&source).unwrap(), b"not an image");
    image(&source, ImageFormat::Png, 25);
    sqlx::raw_sql("CREATE TRIGGER artwork_test_failure BEFORE UPDATE ON games BEGIN SELECT RAISE(ABORT,'fixture commit failure'); END;").execute(&db.pool).await.unwrap();
    assert!(import(&db, &id, source.clone()).await.is_err());
    assert_eq!(db.manual_details(&id).await.unwrap().artwork_asset_id, old);
    sqlx::query("DROP TRIGGER artwork_test_failure")
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE games SET origin='legacy' WHERE id=?")
        .bind(&id)
        .execute(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        import(&db, &id, source.clone()).await.unwrap_err().code,
        "unsupportedGame"
    );
    sqlx::query("DELETE FROM library_entries WHERE game_id=?")
        .bind(&id)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM game_field_overrides WHERE game_id=?")
        .bind(&id)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM games WHERE id=?")
        .bind(&id)
        .execute(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        Service::assign(
            &db.pool,
            &Request {
                game_id: id,
                expected_revision: 1
            },
            None
        )
        .await
        .unwrap_err()
        .code,
        "notFound"
    );
    db.close().await;
}

#[test]
fn image_limits_and_narrow_ipc_authority() {
    assert!(caller("other").is_err());
    assert!(caller("main").is_ok());
    for json in [
        r#"{"gameId":"g","expectedRevision":0,"path":"C:\\x"}"#,
        r#"{"gameId":"../x","expectedRevision":0}"#,
        r#"{"gameId":"g","expectedRevision":-1}"#,
    ] {
        assert!(serde_json::from_str::<Request>(json).map_or(true, |r| r.validate().is_err()));
    }
    assert!(decode(b"GIF89a").is_err());
    assert!(decode(&vec![0; artwork::SOURCE_BYTES as usize + 1]).is_err());
    for (w, h) in [(4097, 1), (1, 4097), (2049, 2048)] {
        let image = image::RgbImage::new(w, h);
        let mut png = Cursor::new(Vec::new());
        image.write_to(&mut png, ImageFormat::Png).unwrap();
        assert_eq!(decode(png.get_ref()).unwrap_err().code, "oversized");
    }
    let image = image::RgbImage::new(64, 36);
    let mut png = Cursor::new(Vec::new());
    image.write_to(&mut png, ImageFormat::Png).unwrap();
    let mut bytes = png.into_inner();
    bytes.truncate(bytes.len() / 2);
    assert!(decode(&bytes).is_err());
}

#[tokio::test]
async fn native_objects_locked_files_storage_failures_and_unknown_cache_are_preserved() {
    let root = tempdir().unwrap();
    let source = root.path().join("source.png");
    image(&source, ImageFormat::Png, 12);
    let bytes = std::fs::read(&source).unwrap();
    let (db, id) = setup(root.path()).await;
    assert!(files::selected(root.path()).is_err());
    assert!(files::selected(&root.path().join("..\\source.png")).is_err());
    let link = root.path().join("hard.png");
    std::fs::hard_link(&source, &link).unwrap();
    assert!(import(&db, &id, source.clone()).await.is_err());
    std::fs::remove_file(link).unwrap();
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        let alternate = PathBuf::from(source.to_str().unwrap().replace('\\', "/"));
        assert_eq!(files::selected(&alternate).unwrap(), bytes);
        let locked = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&source)
            .unwrap();
        assert!(import(&db, &id, source.clone()).await.is_err());
        drop(locked);
        assert!(files::selected(&root.path().join("source.png:stream")).is_err());
        // Disposable junction points outside the owned artwork root; it must not
        // permit a read/write or change the external sentinel.
        let external = root.path().join("external");
        std::fs::create_dir(&external).unwrap();
        std::fs::write(external.join("sentinel"), b"keep").unwrap();
        let junction = root.path().join("junction");
        let status = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&junction)
            .arg(&external)
            .output()
            .unwrap();
        assert!(status.status.success());
        assert!(files::Directory::open(&junction, true).is_err());
        assert_eq!(std::fs::read(external.join("sentinel")).unwrap(), b"keep");
        std::fs::remove_dir(junction).unwrap();
    }
    let s = service(&db);
    std::fs::create_dir_all(&s.root).unwrap();
    std::fs::write(s.root.join("originals"), b"unknown").unwrap();
    assert!(import(&db, &id, source.clone()).await.is_err());
    assert_eq!(std::fs::read(&source).unwrap(), bytes);
    std::fs::remove_file(s.root.join("originals")).unwrap();
    import(&db, &id, source).await.unwrap();
    let a = db
        .manual_details(&id)
        .await
        .unwrap()
        .artwork_asset_id
        .unwrap();
    let dir = files::Directory::open(&s.root.join("thumbnails"), true).unwrap();
    let name = format!(
        "thumb-v1-{:03}.bin",
        u8::from_str_radix(&a[..2], 16).unwrap()
    );
    std::fs::write(dir.path.join(&name), b"unknown").unwrap();
    assert_eq!(thumb(&db, &id).await.status, "ready");
    assert_eq!(std::fs::read(dir.path.join(name)).unwrap(), b"unknown");
    db.close().await;
}

#[tokio::test]
async fn admission_and_lost_waiter_retain_native_resources_until_drain() {
    let root = tempdir().unwrap();
    let (db, id) = setup(root.path()).await;
    let s = service(&db);
    let import = s.admit(true).unwrap();
    assert!(s.admit(true).is_err());
    let t1 = s.admit(false).unwrap();
    let t2 = s.admit(false).unwrap();
    assert!(s.admit(false).is_err());
    drop(t1);
    drop(t2);
    let (send, receive) = std::sync::mpsc::channel();
    let (started, ready) = tokio::sync::oneshot::channel();
    let owner = tokio::spawn(async move {
        tokio::task::spawn_blocking(move || {
            let _operation = import;
            let _ = started.send(());
            receive.recv().unwrap();
        })
        .await
        .unwrap();
    });
    ready.await.unwrap();
    owner.abort();
    let drain = s.drain();
    tokio::pin!(drain);
    assert!(
        std::future::poll_fn(|cx| std::task::Poll::Ready(drain.as_mut().poll(cx).is_pending()))
            .await
    );
    assert!(s.admit(true).is_err());
    send.send(()).unwrap();
    drain.await;
    assert!(thumb(&db, &id).await.data_url.is_none());
    db.close().await;
}

#[tokio::test]
async fn database_only_backup_restore_with_present_and_missing_originals() {
    let root = tempdir().unwrap();
    let source = root.path().join("source.png");
    image(&source, ImageFormat::Png, 20);
    let (db, id) = setup(root.path()).await;
    import(&db, &id, source).await.unwrap();
    let backup = root.path().join("backup.sqlite3");
    let cancel = Arc::new(AtomicBool::new(false));
    let snapshot = maintenance::create_backup(&db, &backup, &cancel)
        .await
        .unwrap();
    assert_eq!(snapshot.schema_version, 5);
    let original = service(&db).root.join("originals").join(format!(
        "{}.img",
        db.manual_details(&id)
            .await
            .unwrap()
            .artwork_asset_id
            .unwrap()
    ));
    let candidate = maintenance::stage_restore(&db, &backup).await.unwrap();
    candidate.confirm(&db).await.unwrap();
    drop(db);
    let db = Database::initialize(&root.path().join("library.sqlite3"))
        .await
        .unwrap();
    assert_eq!(thumb(&db, &id).await.status, "ready");
    std::fs::remove_file(original).unwrap();
    let candidate = maintenance::stage_restore(&db, &backup).await.unwrap();
    candidate.confirm(&db).await.unwrap();
    drop(db);
    let db = Database::initialize(&root.path().join("library.sqlite3"))
        .await
        .unwrap();
    assert!(thumb(&db, &id).await.data_url.is_none());
    assert!(
        db.manual_details(&id)
            .await
            .unwrap()
            .artwork_asset_id
            .is_some()
    );
    db.close().await;
}

#[tokio::test]
async fn actual_restore_closes_artwork_admission_and_waits_for_its_native_owner() {
    let root = tempdir().unwrap();
    let (db, _) = setup(root.path()).await;
    let backup = root.path().join("backup.sqlite3");
    maintenance::create_backup(&db, &backup, &Arc::new(AtomicBool::new(false)))
        .await
        .unwrap();
    let candidate = maintenance::stage_restore(&db, &backup).await.unwrap();
    let s = service(&db);
    let operation = s.admit(true).unwrap();
    let (send, receive) = std::sync::mpsc::channel();
    let owner = tokio::task::spawn_blocking(move || {
        let _operation = operation;
        receive.recv().unwrap();
    });
    {
        let restore = candidate.confirm(&db);
        tokio::pin!(restore);
        assert!(
            std::future::poll_fn(|cx| std::task::Poll::Ready(
                restore.as_mut().poll(cx).is_pending()
            ))
            .await
        );
        assert!(db.is_stopping());
        assert!(s.admit(false).is_err());
        assert!(!db.pool.is_closed());
        send.send(()).unwrap();
        owner.await.unwrap();
        restore.await.unwrap();
    }
    assert!(db.pool.is_closed());
    drop(db);
    let db = Database::initialize(&root.path().join("library.sqlite3"))
        .await
        .unwrap();
    assert_eq!(db.counts().await.unwrap().games, 1);
    db.close().await;
}

#[tokio::test]
async fn process_interruption_at_every_publication_and_transaction_boundary() {
    if let Ok(root) = std::env::var("GM_ARTWORK_CRASH_ROOT") {
        let root = std::path::Path::new(&root);
        let db = Database::initialize(&root.join("library.sqlite3"))
            .await
            .unwrap();
        let id: String = sqlx::query_scalar("SELECT id FROM games LIMIT 1")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        let source = root.join("source.png");
        image(&source, ImageFormat::Png, 90);
        import(&db, &id, source).await.unwrap();
        panic!("checkpoint did not terminate child");
    }
    for at in [
        "validated",
        "temporary-synced",
        "published",
        "verified",
        "writer",
        "metadata",
        "assignment",
        "before-commit",
        "committed",
    ] {
        let root = tempdir().unwrap();
        let (db, id) = setup(root.path()).await;
        let source = root.path().join("source.png");
        image(&source, ImageFormat::Png, 10);
        import(&db, &id, source).await.unwrap();
        let old = db
            .manual_details(&id)
            .await
            .unwrap()
            .artwork_asset_id
            .unwrap();
        let original = root
            .path()
            .join("artwork/originals")
            .join(format!("{old}.img"));
        let original_bytes = std::fs::read(&original).unwrap();
        db.close().await;
        drop(db);
        let output=std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact","local_artwork::tests::process_interruption_at_every_publication_and_transaction_boundary","--nocapture"]).env("GM_ARTWORK_CRASH_ROOT",root.path()).env("GM_ARTWORK_CRASH_AT",at).output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(89),
            "{at}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let db = Database::initialize(&root.path().join("library.sqlite3"))
            .await
            .unwrap();
        let assignments: i64 = sqlx::query_scalar("SELECT count(*) FROM game_artwork")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(assignments, 1, "{at}");
        let current = db.manual_details(&id).await.unwrap();
        assert_eq!(
            current.artwork_asset_id.as_deref() == Some(old.as_str()),
            at != "committed",
            "{at}"
        );
        assert_eq!(
            current.revision,
            if at == "committed" { 2 } else { 1 },
            "{at}"
        );
        assert_eq!(
            std::fs::read(&original).unwrap(),
            original_bytes,
            "prior original: {at}"
        );
        db.validate().await.unwrap();
        let prior = std::fs::read_dir(root.path().join("artwork/originals"))
            .map(|entries| entries.count())
            .unwrap_or(0);
        db.close().await;
        let db = Database::initialize(&root.path().join("library.sqlite3"))
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_dir(root.path().join("artwork/originals"))
                .map(|e| e.count())
                .unwrap_or(0),
            prior,
            "startup must preserve orphans: {at}"
        );
        db.close().await;
    }
}

#[tokio::test]
async fn populated_historical_upgrades_and_exact_v5_validation() {
    for count in [2, 3, 4] {
        let root = tempdir().unwrap();
        let path = root.path().join("library.sqlite3");
        let mut c = SqliteConnection::connect_with(
            &sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true),
        )
        .await
        .unwrap();
        let migrator = sqlx::migrate::Migrator {
            migrations: std::borrow::Cow::Borrowed(&crate::storage::MIGRATOR.migrations[..count]),
            ..sqlx::migrate::Migrator::DEFAULT
        };
        migrator.run_direct(None, &mut c, false).await.unwrap();
        sqlx::query("INSERT INTO games(id,title,sort_title,created_at_ms,updated_at_ms,origin,revision) VALUES ('g','Retained','retained',1,2,'manual',9)").execute(&mut c).await.unwrap();
        c.close().await.unwrap();
        let selected = std::fs::read(&path).unwrap();
        assert_eq!(
            maintenance::validate_snapshot(&path)
                .await
                .unwrap()
                .schema_version,
            count as i64
        );
        assert_eq!(std::fs::read(&path).unwrap(), selected);
        let db = Database::initialize(&path).await.unwrap();
        assert_eq!(db.status().await.unwrap().schema_version, 5);
        let details = db.manual_details("g").await.unwrap();
        assert_eq!(details.revision, 9);
        assert!(details.artwork_asset_id.is_none());
        let backup = root.path().join("backup.sqlite3");
        maintenance::create_backup(&db, &backup, &Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();
        db.close().await;
        let mut c = SqliteConnection::connect_with(
            &sqlx::sqlite::SqliteConnectOptions::new().filename(&backup),
        )
        .await
        .unwrap();
        sqlx::query("UPDATE _sqlx_migrations SET checksum=X'00' WHERE version=?")
            .bind(count as i64)
            .execute(&mut c)
            .await
            .unwrap();
        c.close().await.unwrap();
        assert!(maintenance::validate_snapshot(&backup).await.is_err());
    }
}

#[test]
fn bounded_memory_and_disk_slots_preserve_every_original_and_immutable_publication() {
    let root = tempdir().unwrap();
    let source = root.path().join("source.png");
    image(&source, ImageFormat::Png, 30);
    let s = Service::new(root.path().join("artwork"));
    for i in 0..270u32 {
        let id = format!("{:02x}{:030x}", i % 256, i);
        let asset = s.publish(&source, id.clone(), None).unwrap();
        let reply = s.load(&id, asset.bytes, &asset.fingerprint).unwrap();
        assert_eq!(reply.status, "ready");
        assert!(reply.data_url.unwrap().len() <= 22000);
        assert!(s.stats().memory_entries <= 128);
        assert!(s.stats().disk_entries <= 256);
    }
    assert_eq!(s.stats().memory_entries, 128);
    assert_eq!(
        std::fs::read_dir(s.root.join("thumbnails"))
            .unwrap()
            .count(),
        256
    );
    assert_eq!(
        std::fs::read_dir(s.root.join("originals")).unwrap().count(),
        270
    );
    let before = std::fs::read(
        s.root
            .join("originals")
            .join(format!("{}.img", "0".repeat(32))),
    )
    .unwrap();
    image(&source, ImageFormat::Jpeg, 99);
    assert!(s.publish(&source, "0".repeat(32), None).is_err());
    assert_eq!(
        std::fs::read(
            s.root
                .join("originals")
                .join(format!("{}.img", "0".repeat(32)))
        )
        .unwrap(),
        before
    );
}
