// SPDX-License-Identifier: GPL-3.0-or-later
//! Disposable, ID-only thumbnail service. No filesystem path crosses IPC.
pub mod fixtures;

use base64::{Engine, engine::general_purpose::STANDARD};
use image::{ImageFormat, ImageReader};
use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};

pub const WIDTH: u32 = 64;
pub const HEIGHT: u32 = 36;
pub const SOURCE_BYTES: u64 = 8 * 1024 * 1024;
pub const SOURCE_PIXELS: u64 = 4_194_304;
pub const MEMORY_ENTRIES: usize = 128;
pub const DISK_ENTRIES: usize = 256;
pub const THUMB_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reply {
    pub status: String,
    pub data_url: Option<String>,
    pub cache: String,
    pub work_ms: f64,
    pub decode_ms: f64,
}
impl Reply {
    pub(crate) fn empty(status: &str) -> Self {
        Self {
            status: status.into(),
            data_url: None,
            cache: "none".into(),
            work_ms: 0.0,
            decode_ms: 0.0,
        }
    }
}
#[derive(Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub active: usize,
    pub max_active: usize,
    pub active_decodes: usize,
    pub max_active_decodes: usize,
    pub deduplicated: u64,
    pub busy: u64,
    pub memory_hits: u64,
    pub disk_hits: u64,
    pub decodes: u64,
    pub failures: u64,
    pub evictions: u64,
    pub memory_entries: usize,
    pub disk_entries: usize,
    pub disk_bytes: u64,
}
#[derive(Default)]
struct Cache {
    memory: VecDeque<(String, String)>,
    disk: VecDeque<(String, u64)>,
    stats: Stats,
}
pub struct Service {
    root: PathBuf,
    seed: u64,
    games: u32,
    cache: Mutex<Cache>,
    flights: Mutex<HashMap<u32, Flight>>,
}
struct Flight {
    receiver: tokio::sync::watch::Receiver<Option<Reply>>,
    waiters: usize,
}
struct DecodeGuard<'a>(&'a Service);
impl Drop for DecodeGuard<'_> {
    fn drop(&mut self) {
        self.0.cache.lock().unwrap().stats.active_decodes -= 1;
    }
}
impl Service {
    /// Opens only explicitly generated fixture roots. Never creates source assets.
    pub fn open(root: &Path) -> Result<Arc<Self>, String> {
        let root = root.canonicalize().map_err(|_| "Artwork unavailable")?;
        let config = fixtures::read_config(&root)?;
        for child in ["originals", "thumbnails", "temporary"] {
            let path = root.join(child);
            let meta =
                fs::symlink_metadata(&path).map_err(|_| "Artwork directories unavailable")?;
            if !meta.is_dir()
                || meta.file_type().is_symlink()
                || path
                    .canonicalize()
                    .map_err(|_| "Artwork directory unavailable")?
                    != path
            {
                return Err("Artwork directory must be owned and local".into());
            }
        }
        let service = Arc::new(Self {
            root,
            seed: config.seed,
            games: config.games,
            cache: Mutex::new(Cache::default()),
            flights: Mutex::new(HashMap::new()),
        });
        for entry in fs::read_dir(service.root.join("temporary"))
            .map_err(|_| "Temporary cache unavailable")?
        {
            let entry = entry.map_err(|_| "Temporary cache unavailable")?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.is_ascii()
                || name.len() != 8
                || !name.ends_with(".tmp")
                || !name[..4].bytes().all(|b| b.is_ascii_digit())
            {
                return Err("Unknown temporary file preserved".into());
            }
            read_owned(&entry.path(), THUMB_BYTES as u64)
                .map_err(|_| "Unsafe temporary file preserved")?;
            fs::remove_file(entry.path()).map_err(|_| "Temporary recovery failed")?;
        }
        service.trim_disk()?;
        Ok(service)
    }
    fn asset(&self, id: &str) -> Option<u32> {
        let prefix = format!("syn-{:016x}-g-", self.seed);
        let suffix = id.strip_prefix(&prefix)?;
        if suffix.len() != 5 || !suffix.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let index: u32 = suffix.parse().ok()?;
        (index < self.games).then_some(fixtures::asset_for_index(index))
    }
    pub fn stats(&self) -> Stats {
        self.cache.lock().unwrap().stats.clone()
    }
    /// Admission happens before spawn_blocking: at most two native jobs, no native queue.
    /// Duplicate callers share the admitted job; detached work always releases admission.
    pub async fn request(self: &Arc<Self>, id: &str) -> Reply {
        let Some(asset) = self.asset(id) else {
            return Reply::empty("invalid");
        };
        let mut receiver = {
            let mut flights = self.flights.lock().unwrap();
            if let Some(flight) = flights.get_mut(&asset) {
                if flight.waiters >= 8 {
                    self.cache.lock().unwrap().stats.busy += 1;
                    return Reply::empty("busy");
                }
                flight.waiters += 1;
                self.cache.lock().unwrap().stats.deduplicated += 1;
                flight.receiver.clone()
            } else {
                if flights.len() >= 2 {
                    self.cache.lock().unwrap().stats.busy += 1;
                    return Reply::empty("busy");
                }
                let (sender, receiver) = tokio::sync::watch::channel(None);
                flights.insert(
                    asset,
                    Flight {
                        receiver: receiver.clone(),
                        waiters: 1,
                    },
                );
                {
                    let mut cache = self.cache.lock().unwrap();
                    cache.stats.active = flights.len();
                    cache.stats.max_active = cache.stats.max_active.max(flights.len());
                }
                let service = Arc::clone(self);
                tokio::spawn(async move {
                    let worker = Arc::clone(&service);
                    let reply = tokio::task::spawn_blocking(move || worker.load(asset))
                        .await
                        .unwrap_or_else(|_| Reply::empty("failed"));
                    sender.send_replace(Some(reply));
                    let mut flights = service.flights.lock().unwrap();
                    flights.remove(&asset);
                    service.cache.lock().unwrap().stats.active = flights.len();
                });
                receiver
            }
        };
        loop {
            if let Some(reply) = receiver.borrow_and_update().clone() {
                return reply;
            }
            if receiver.changed().await.is_err() {
                return Reply::empty("failed");
            }
        }
    }
    fn load(&self, asset: u32) -> Reply {
        let start = Instant::now();
        let result = self.load_inner(asset);
        let mut reply = match result {
            Ok(reply) => reply,
            Err(status) => {
                self.cache.lock().unwrap().stats.failures += 1;
                Reply::empty(status)
            }
        };
        reply.work_ms = start.elapsed().as_secs_f64() * 1000.0;
        reply
    }
    fn load_inner(&self, asset: u32) -> Result<Reply, &'static str> {
        let source = self.root.join("originals").join(format!("{asset:04}.img"));
        let bytes = read_owned(&source, SOURCE_BYTES).map_err(|e| {
            if e.kind() == std::io::ErrorKind::InvalidData {
                "oversized"
            } else {
                "unavailable"
            }
        })?;
        // Hash the bounded bytes, not just mtime/length, so same-size replacements invalidate.
        let fingerprint = fingerprint(&bytes);
        let key = format!("v1-{asset:04}-{fingerprint:016x}.png");
        {
            let mut cache = self.cache.lock().unwrap();
            if let Some(index) = cache.memory.iter().position(|(k, _)| k == &key) {
                let entry = cache.memory.remove(index).unwrap();
                let data_url = entry.1.clone();
                cache.memory.push_back(entry);
                cache.stats.memory_hits += 1;
                return Ok(Reply {
                    data_url: Some(data_url),
                    cache: "memory".into(),
                    ..Reply::empty("ready")
                });
            }
        }
        let path = self.root.join("thumbnails").join(&key);
        let disk = read_owned(&path, THUMB_BYTES as u64)
            .ok()
            .filter(|b| valid_thumbnail(b));
        let (png, hit, decode_ms) = if let Some(bytes) = disk {
            self.cache.lock().unwrap().stats.disk_hits += 1;
            (bytes, "disk", 0.0)
        } else {
            let format = image::guess_format(&bytes).map_err(|_| "unsupported")?;
            if !matches!(format, ImageFormat::Png | ImageFormat::Jpeg) {
                return Err("unsupported");
            }
            let (width, height) = ImageReader::with_format(Cursor::new(&bytes), format)
                .into_dimensions()
                .map_err(|_| "corrupt")?;
            if width == 0
                || height == 0
                || width > 4096
                || height > 4096
                || u64::from(width) * u64::from(height) > SOURCE_PIXELS
            {
                return Err("oversized");
            }
            let mut reader = ImageReader::with_format(Cursor::new(&bytes), format);
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(4096);
            limits.max_image_height = Some(4096);
            limits.max_alloc = Some(64 * 1024 * 1024);
            reader.limits(limits);
            let decode_start = Instant::now();
            {
                let mut cache = self.cache.lock().unwrap();
                cache.stats.active_decodes += 1;
                cache.stats.max_active_decodes = cache
                    .stats
                    .max_active_decodes
                    .max(cache.stats.active_decodes);
            }
            let decode_guard = DecodeGuard(self);
            let decoded = reader.decode().map_err(|_| "corrupt")?;
            let thumb = decoded
                .resize_exact(WIDTH, HEIGHT, image::imageops::FilterType::Triangle)
                .to_rgb8();
            let mut png = Cursor::new(Vec::new());
            thumb
                .write_to(&mut png, ImageFormat::Png)
                .map_err(|_| "failed")?;
            let png = png.into_inner();
            if png.len() > THUMB_BYTES {
                return Err("failed");
            }
            let elapsed = decode_start.elapsed().as_secs_f64() * 1000.0;
            drop(decode_guard);
            self.cache.lock().unwrap().stats.decodes += 1;
            // Only one worker per asset. No arbitrary temp files or source writes.

            let temporary = self.root.join("temporary").join(format!("{asset:04}.tmp"));
            let written = (|| -> std::io::Result<()> {
                use std::io::Write;
                for directory in ["temporary", "thumbnails"] {
                    let owned = self.root.join(directory);
                    if owned.canonicalize()? != owned {
                        return Err(std::io::Error::other(
                            "Cache directory redirected; preserved",
                        ));
                    }
                }
                let mut file = fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(&temporary)?;
                file.write_all(&png)?;
                drop(file);
                let mut cache = self.cache.lock().unwrap();
                // Reserve capacity before publishing; both writers share this index.
                if path.exists() {
                    remove_derived(&path)?;
                }
                cache.disk.retain(|(k, _)| k != &key);
                while cache.disk.len() >= DISK_ENTRIES
                    || cache.disk.iter().map(|e| e.1).sum::<u64>() + png.len() as u64
                        > (DISK_ENTRIES * THUMB_BYTES) as u64
                {
                    let (old, _) = cache
                        .disk
                        .front()
                        .cloned()
                        .ok_or_else(|| std::io::Error::other("Cache capacity"))?;
                    remove_derived(&self.root.join("thumbnails").join(old))?;
                    cache.disk.pop_front();
                    cache.stats.evictions += 1;
                }
                fs::rename(&temporary, &path)?;
                cache.disk.push_back((key.clone(), png.len() as u64));
                cache.stats.disk_entries = cache.disk.len();
                cache.stats.disk_bytes = cache.disk.iter().map(|e| e.1).sum();
                Ok(())
            })();
            if written.is_err() {
                if read_owned(&temporary, THUMB_BYTES as u64).is_ok() {
                    let _ = fs::remove_file(&temporary);
                }
                return Err("unavailable");
            }

            (png, "miss", elapsed)
        };
        let data_url = format!("data:image/png;base64,{}", STANDARD.encode(png));
        let mut cache = self.cache.lock().unwrap();
        // Discard older transformations of this asset; originals are never deleted.
        let prefix = format!("v1-{asset:04}-");
        cache.memory.retain(|(k, _)| !k.starts_with(&prefix));
        cache.memory.push_back((key, data_url.clone()));
        while cache.memory.len() > MEMORY_ENTRIES {
            cache.memory.pop_front();
            cache.stats.evictions += 1;
        }
        cache.stats.memory_entries = cache.memory.len();
        Ok(Reply {
            data_url: Some(data_url),
            cache: hit.into(),
            decode_ms,
            ..Reply::empty("ready")
        })
    }
    fn trim_disk(&self) -> Result<(), String> {
        // Startup-only bounded directory index. Runtime never rescans the directory.
        let mut cache = self.cache.lock().unwrap();
        for entry in fs::read_dir(self.root.join("thumbnails")).map_err(|_| "Cache unavailable")? {
            let entry = entry.map_err(|_| "Cache unavailable")?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let valid = name.is_ascii()
                && name.len() == 28
                && name.starts_with("v1-")
                && name.ends_with(".png")
                && name[3..7].bytes().all(|b| b.is_ascii_digit())
                && name.as_bytes()[7] == b'-'
                && name[8..24].bytes().all(|b| b.is_ascii_hexdigit());
            if !valid {
                return Err("Unexpected file in disposable thumbnail cache; preserved".into());
            }
            let meta = fs::symlink_metadata(entry.path()).map_err(|_| "Cache unavailable")?;
            if !meta.is_file()
                || meta.file_type().is_symlink()
                || entry
                    .path()
                    .canonicalize()
                    .map_err(|_| "Cache unavailable")?
                    != entry.path()
            {
                return Err("Unsafe cache entry; preserved".into());
            }
            cache.disk.push_back((name, meta.len()));
            while cache.disk.len() > DISK_ENTRIES
                || cache.disk.iter().map(|e| e.1).sum::<u64>() > (DISK_ENTRIES * THUMB_BYTES) as u64
            {
                let (name, _) = cache.disk.front().cloned().ok_or("Cache accounting")?;
                remove_derived(&self.root.join("thumbnails").join(name))
                    .map_err(|_| "Cache eviction failed")?;
                cache.disk.pop_front();
                cache.stats.evictions += 1;
            }
        }
        cache.stats.disk_entries = cache.disk.len();
        cache.stats.disk_bytes = cache.disk.iter().map(|e| e.1).sum();
        Ok(())
    }
}
fn remove_derived(path: &Path) -> std::io::Result<()> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
        Ok(meta) => {
            if !meta.is_file() || meta.file_type().is_symlink() || path.canonicalize()? != path {
                return Err(std::io::Error::other(
                    "Unsafe derived cache file; preserved",
                ));
            }
            fs::remove_file(path)
        }
    }
}
fn read_owned(path: &Path, limit: u64) -> std::io::Result<Vec<u8>> {
    let meta = fs::symlink_metadata(path)?;
    if meta.len() > limit {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Image too large",
        ));
    }
    if !meta.is_file() || meta.file_type().is_symlink() || path.canonicalize()? != path {
        return Err(std::io::Error::other("Invalid owned image"));
    }
    let file = fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(std::io::Error::other("Image too large"));
    }
    Ok(bytes)
}
fn valid_thumbnail(bytes: &[u8]) -> bool {
    ImageReader::with_format(Cursor::new(bytes), ImageFormat::Png)
        .into_dimensions()
        .is_ok_and(|d| d == (WIDTH, HEIGHT))
        && image::load_from_memory_with_format(bytes, ImageFormat::Png).is_ok()
}
pub fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}
