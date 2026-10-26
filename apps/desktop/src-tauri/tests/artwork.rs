// SPDX-License-Identifier: GPL-3.0-or-later
use game_manager_lib::artwork::{self, Service, fixtures::Config};
use std::{fs, path::Path, sync::Arc};
fn root() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}
fn prepare(root: &Path) {
    for dir in ["originals", "thumbnails", "temporary"] {
        fs::create_dir(root.join(dir)).unwrap();
    }
    fs::write(
        root.join("synthetic-artwork-v1.json"),
        serde_json::to_vec(&Config {
            version: 1,
            games: 500,
            seed: 42,
        })
        .unwrap(),
    )
    .unwrap();
}
fn image(root: &Path, index: u32, color: u8) {
    image::RgbImage::from_pixel(120, 80, image::Rgb([color, 64, 128]))
        .save_with_format(
            root.join("originals").join(format!("{index:04}.img")),
            image::ImageFormat::Png,
        )
        .unwrap();
}
fn id(index: u32) -> String {
    format!("syn-000000000000002a-g-{index:05}")
}

#[tokio::test]
async fn cache_hit_disk_reopen_and_same_length_source_change_invalidate() {
    let temp = root();
    prepare(temp.path());
    image(temp.path(), 1, 10);
    let source = temp.path().join("originals/0001.img");
    let mut original = fs::read(&source).unwrap();
    original.resize(4096, 0);
    fs::write(&source, original).unwrap();
    let service = Service::open(temp.path()).unwrap();
    let first = service.request(&id(1)).await;
    assert_eq!(first.cache, "miss");
    assert_eq!(first.status, "ready");
    assert_eq!(service.request(&id(1)).await.cache, "memory");
    let reopened = Service::open(temp.path()).unwrap();
    assert_eq!(reopened.request(&id(1)).await.cache, "disk");
    image(temp.path(), 1, 200);
    let mut replacement = fs::read(&source).unwrap();
    replacement.resize(4096, 0);
    fs::write(&source, replacement).unwrap();
    assert_eq!(fs::metadata(&source).unwrap().len(), 4096);
    let changed = service.request(&id(1)).await;
    assert_eq!(changed.cache, "miss");
    assert_ne!(first.data_url, changed.data_url);
    assert_eq!(service.stats().memory_entries, 1);
}

#[cfg(windows)]
#[tokio::test]
async fn inaccessible_source_preserved_and_retried_after_lock_release() {
    use std::os::windows::fs::OpenOptionsExt;
    let temp = root();
    prepare(temp.path());
    image(temp.path(), 1, 10);
    let path = temp.path().join("originals/0001.img");
    let locked = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&path)
        .unwrap();
    let service = Service::open(temp.path()).unwrap();
    assert_eq!(service.request(&id(1)).await.status, "unavailable");
    drop(locked);
    assert_eq!(service.request(&id(1)).await.status, "ready");
}

#[tokio::test]
async fn missing_corrupt_unsupported_oversized_invalid_ids_and_recovery() {
    let temp = root();
    prepare(temp.path());
    let service = Service::open(temp.path()).unwrap();
    assert_eq!(service.request(&id(1)).await.status, "unavailable");
    fs::write(
        temp.path().join("originals/0001.img"),
        b"\x89PNG\r\n\x1a\ncorrupt",
    )
    .unwrap();
    assert_eq!(service.request(&id(1)).await.status, "corrupt");
    fs::write(
        temp.path().join("originals/0001.img"),
        b"GIF89a unsupported",
    )
    .unwrap();
    assert_eq!(service.request(&id(1)).await.status, "unsupported");
    image::RgbImage::new(4097, 1024)
        .save_with_format(
            temp.path().join("originals/0001.img"),
            image::ImageFormat::Png,
        )
        .unwrap();
    assert_eq!(service.request(&id(1)).await.status, "oversized");
    fs::write(
        temp.path().join("originals/0001.img"),
        vec![0; artwork::SOURCE_BYTES as usize + 1],
    )
    .unwrap();
    assert_eq!(service.request(&id(1)).await.status, "oversized");
    for invalid in [
        "../0001",
        "C:\\secret.png",
        "file:///x",
        "syn-000000000000002a-g-00500",
        "syn-000000000000002a-g-00001/..",
        "syn-000000000000002b-g-00001",
        "syn-000000000000002a-g-0000é",
    ] {
        assert_eq!(service.request(invalid).await.status, "invalid");
    }
    image(temp.path(), 1, 10);
    assert_eq!(service.request(&id(1)).await.status, "ready");
    let owned = temp.path().join("originals/0001.img");
    fs::remove_file(&owned).unwrap();
    fs::create_dir(&owned).unwrap();
    assert_eq!(service.request(&id(1)).await.status, "unavailable");
}

#[tokio::test]
async fn duplicate_requests_share_work_and_native_admission_has_no_queue() {
    let temp = root();
    prepare(temp.path());
    for index in 1..=3 {
        image(temp.path(), index, index as u8);
    }
    let service = Service::open(temp.path()).unwrap();
    let (one, two, three) = (id(1), id(2), id(3));
    let (a, b, c, d) = tokio::join!(
        service.request(&one),
        service.request(&one),
        service.request(&two),
        service.request(&three)
    );
    assert_eq!(a.data_url, b.data_url);
    assert_eq!(c.status, "ready");
    assert_eq!(d.status, "busy");
    assert_eq!(service.stats().deduplicated, 1);
    assert_eq!(service.stats().max_active, 2);
    assert_eq!(service.stats().decodes, 2);
}

#[tokio::test]
async fn caches_evict_by_capacity_and_preserve_originals() {
    let temp = root();
    prepare(temp.path());
    let service = Service::open(temp.path()).unwrap();
    for index in 0..350 {
        image(temp.path(), index, index as u8);
        // Populate repeated-source slots before requesting their aliases.
        image(
            temp.path(),
            game_manager_lib::artwork::fixtures::asset_for_index(index),
            index as u8,
        );
        assert_eq!(service.request(&id(index)).await.status, "ready");
    }
    let stats = service.stats();
    assert_eq!(stats.memory_entries, artwork::MEMORY_ENTRIES);
    assert!(stats.disk_entries <= artwork::DISK_ENTRIES);
    assert!(stats.disk_bytes <= (artwork::DISK_ENTRIES * artwork::THUMB_BYTES) as u64);
    assert!(stats.evictions > 0);
    assert_eq!(
        fs::read_dir(temp.path().join("originals")).unwrap().count(),
        350
    );
    assert_ne!(service.request(&id(34)).await.cache, "memory");
}

#[tokio::test]
async fn interrupted_temporary_write_and_corrupt_derived_cache_are_recoverable() {
    let temp = root();
    prepare(temp.path());
    image(temp.path(), 1, 10);
    fs::write(temp.path().join("temporary/0001.tmp"), b"partial").unwrap();
    let service = Service::open(temp.path()).unwrap();
    assert!(!temp.path().join("temporary/0001.tmp").exists());
    service.request(&id(1)).await;
    drop(service);
    let cached = fs::read_dir(temp.path().join("thumbnails"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::write(&cached, b"corrupt").unwrap();
    let service = Service::open(temp.path()).unwrap();
    assert_eq!(service.request(&id(1)).await.cache, "miss");
    fs::write(temp.path().join("thumbnails/personal.txt"), b"preserve").unwrap();
    assert!(Service::open(temp.path()).is_err());
    assert!(temp.path().join("thumbnails/personal.txt").exists());
}

#[tokio::test]
async fn cancellation_of_a_waiter_does_not_leak_admission() {
    let temp = root();
    prepare(temp.path());
    image(temp.path(), 1, 10);
    let service = Service::open(temp.path()).unwrap();
    let other = Arc::clone(&service);
    let waiter = tokio::spawn(async move { other.request(&id(1)).await });
    tokio::task::yield_now().await;
    waiter.abort();
    for _ in 0..10000 {
        if service.stats().active == 0 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(service.request(&id(1)).await.status, "ready");
}
