// SPDX-License-Identifier: GPL-3.0-or-later
//! Feature-specific manual-library artwork. Originals are never garbage-collected.
mod files;
use crate::{
    artwork::{self, Reply},
    scheduling::Budget,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use image::{ImageFormat, ImageReader};
use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};
use std::{
    collections::{HashMap, VecDeque},
    io::Cursor,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[derive(Debug, Serialize)]
pub struct Failure {
    pub code: &'static str,
}
type Result<T> = std::result::Result<T, Failure>;
fn failure(code: &'static str) -> Failure {
    Failure { code }
}
impl From<sqlx::Error> for Failure {
    fn from(_: sqlx::Error) -> Self {
        failure("database")
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub game_id: String,
    pub expected_revision: i64,
}
impl Request {
    fn validate(&self) -> Result<()> {
        if self.game_id.is_empty()
            || self.game_id.len() > 128
            || !self
                .game_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || self.expected_revision < 0
        {
            return Err(failure("invalidRequest"));
        }
        Ok(())
    }
}
#[derive(Debug)]
pub struct Service {
    root: PathBuf,
    closed: AtomicBool,
    all: Arc<Semaphore>,
    imports: Arc<Semaphore>,
    thumbnails: Arc<Semaphore>,
    memory: Mutex<VecDeque<(String, String)>>,
    stats: Mutex<artwork::Stats>,
    disk: Mutex<HashMap<u8, u64>>,
}
struct Operation {
    _all: OwnedSemaphorePermit,
    _kind: OwnedSemaphorePermit,
}
struct DecodeActivity<'a>(&'a Mutex<artwork::Stats>);
impl Drop for DecodeActivity<'_> {
    fn drop(&mut self) {
        self.0.lock().unwrap().active_decodes -= 1;
    }
}
impl Service {
    pub(crate) fn new(root: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            root,
            closed: AtomicBool::new(false),
            all: Arc::new(Semaphore::new(3)),
            imports: Arc::new(Semaphore::new(1)),
            thumbnails: Arc::new(Semaphore::new(2)),
            memory: Mutex::new(VecDeque::new()),
            stats: Mutex::new(artwork::Stats::default()),
            disk: Mutex::new(HashMap::new()),
        })
    }
    fn admit(&self, import: bool) -> Result<Operation> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(failure("stopping"));
        }
        let all = self
            .all
            .clone()
            .try_acquire_owned()
            .map_err(|_| failure("busy"))?;
        let kind = if import {
            &self.imports
        } else {
            &self.thumbnails
        }
        .clone()
        .try_acquire_owned()
        .map_err(|_| failure("busy"))?;
        if self.closed.load(Ordering::SeqCst) {
            return Err(failure("stopping"));
        }
        Ok(Operation {
            _all: all,
            _kind: kind,
        })
    }
    pub(crate) async fn drain(&self) {
        self.closed.store(true, Ordering::SeqCst);
        let _guard = self
            .all
            .acquire_many(3)
            .await
            .expect("artwork semaphore never closed");
    }
    pub(crate) fn stats(&self) -> artwork::Stats {
        let mut stats = self.stats.lock().unwrap().clone();
        stats.active = 3 - self.all.available_permits();
        stats.memory_entries = self.memory.lock().unwrap().len();
        let disk = self.disk.lock().unwrap();
        stats.disk_entries = disk.len();
        stats.disk_bytes = disk.values().sum();
        stats
    }
    fn process(&self, bytes: &[u8]) -> Result<(&'static str, u32, u32, Vec<u8>)> {
        {
            let mut stats = self.stats.lock().unwrap();
            stats.active_decodes += 1;
            stats.max_active_decodes = stats.max_active_decodes.max(stats.active_decodes);
            stats.decodes += 1;
        }
        let _activity = DecodeActivity(&self.stats);
        decode(bytes)
    }
    async fn context(c: &mut SqliteConnection, request: &Request) -> Result<()> {
        request.validate()?;
        let row: Option<(i64, String)> =
            sqlx::query_as("SELECT revision,origin FROM games WHERE id=?")
                .bind(&request.game_id)
                .fetch_optional(c)
                .await?;
        match row {
            None => Err(failure("notFound")),
            Some((revision, origin)) => {
                if origin != "manual" {
                    return Err(failure("unsupportedGame"));
                }
                if revision != request.expected_revision {
                    return Err(failure("stale"));
                }
                Ok(())
            }
        }
    }
    async fn assign(pool: &SqlitePool, request: &Request, asset: Option<Asset>) -> Result<()> {
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        Self::context(&mut tx, request).await?;
        checkpoint("writer");
        if let Some(a) = asset {
            sqlx::query("INSERT INTO artwork_assets(id,format,bytes,width,height,fingerprint) VALUES (?,?,?,?,?,?)")
                .bind(&a.id).bind(a.format).bind(a.bytes).bind(a.width).bind(a.height).bind(a.fingerprint).execute(&mut *tx).await?;
            checkpoint("metadata");
            sqlx::query("INSERT INTO game_artwork(game_id,asset_id) VALUES (?,?) ON CONFLICT(game_id) DO UPDATE SET asset_id=excluded.asset_id").bind(&request.game_id).bind(a.id).execute(&mut *tx).await?;
        } else {
            sqlx::query("DELETE FROM game_artwork WHERE game_id=?")
                .bind(&request.game_id)
                .execute(&mut *tx)
                .await?;
        }
        checkpoint("assignment");
        sqlx::query("UPDATE games SET revision=revision+1,updated_at_ms=CAST((julianday('now')-2440587.5)*86400000 AS INTEGER) WHERE id=?").bind(&request.game_id).execute(&mut *tx).await?;
        checkpoint("before-commit");
        tx.commit().await?;
        checkpoint("committed");
        Ok(())
    }
    async fn import(
        self: &Arc<Self>,
        pool: SqlitePool,
        budget: Arc<Budget>,
        request: Request,
        path: PathBuf,
        operation: Operation,
    ) -> Result<()> {
        // The detached owner survives cancellation of its IPC waiter, including
        // the blocking decode/publication and the final database transaction.
        let service = self.clone();
        tokio::spawn(async move {
            let _operation = operation;
            Self::context(&mut *pool.acquire().await?, &request).await?;
            let id: String = sqlx::query_scalar("SELECT lower(hex(randomblob(16)))")
                .fetch_one(&pool)
                .await?;
            let worker = budget
                .acquire(&AtomicBool::new(false))
                .await
                .ok_or_else(|| failure("stopping"))?;
            let asset = tokio::task::spawn_blocking(move || {
                let _worker = worker;
                service.publish(&path, id, None)
            })
            .await
            .map_err(|_| failure("storage"))??;
            Self::assign(&pool, &request, Some(asset)).await
        })
        .await
        .map_err(|_| failure("storage"))?
    }
    fn publish(&self, path: &std::path::Path, id: String, fault: Option<&str>) -> Result<Asset> {
        let bytes = files::selected(path)?;
        let (format, width, height, _) = self.process(&bytes)?;
        checkpoint("validated");
        if fault == Some("validated") {
            return Err(failure("interrupted"));
        }
        let originals = files::Directory::open(&self.root.join("originals"), true)?;
        originals.publish(&format!("{id}.img"), &bytes)?;
        checkpoint("verified");
        if fault == Some("published") {
            return Err(failure("interrupted"));
        }
        Ok(Asset {
            id,
            format,
            bytes: bytes.len() as i64,
            width,
            height,
            fingerprint: format!("{:016x}", artwork::fingerprint(&bytes)),
        })
    }
    pub(crate) async fn thumbnail(
        self: &Arc<Self>,
        pool: SqlitePool,
        budget: Arc<Budget>,
        id: String,
    ) -> Reply {
        if id.is_empty()
            || id.len() > 128
            || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Reply::empty("invalid");
        }
        let operation = match self.admit(false) {
            Ok(op) => op,
            Err(e) => return Reply::empty(e.code),
        };
        {
            let mut stats = self.stats.lock().unwrap();
            stats.max_active = stats
                .max_active
                .max(2 - self.thumbnails.available_permits());
        }
        let service = self.clone();
        tokio::spawn(async move {
            let _operation=operation;
            let asset:Option<(String,i64,String)>=sqlx::query_as("SELECT a.id,a.bytes,a.fingerprint FROM game_artwork g JOIN artwork_assets a ON a.id=g.asset_id WHERE g.game_id=?").bind(id).fetch_optional(&pool).await.map_err(|_|failure("database"))?;
            let Some((id,size,fingerprint))=asset else {return Ok(Reply::empty("missing"));};
            if !asset_id(&id) {return Err(failure("invalidAsset"));}
            let worker=budget.acquire(&AtomicBool::new(false)).await.ok_or_else(||failure("stopping"))?;
            tokio::task::spawn_blocking(move || {let _worker=worker;service.load(&id,size,&fingerprint)}).await.map_err(|_|failure("unavailable"))?
        }).await.unwrap_or_else(|_|Err(failure("unavailable"))).unwrap_or_else(|e|Reply::empty(e.code))
    }
    fn load(&self, id: &str, size: i64, fingerprint: &str) -> Result<Reply> {
        let start = std::time::Instant::now();
        // Re-read bounded originals before cache hits: removed, changed or corrupt
        // durable files must not be hidden by a surviving derived cache.
        let originals = files::Directory::open(&self.root.join("originals"), false)?;
        let bytes = originals.read(&format!("{id}.img"), artwork::SOURCE_BYTES)?;
        if bytes.len() as i64 != size
            || format!("{:016x}", artwork::fingerprint(&bytes)) != fingerprint
        {
            return Err(failure("corrupt"));
        }
        let mut memory = self.memory.lock().unwrap();
        if let Some(index) = memory.iter().position(|(key, _)| key == id) {
            let entry = memory.remove(index).unwrap();
            let url = entry.1.clone();
            memory.push_back(entry);
            self.stats.lock().unwrap().memory_hits += 1;
            return Ok(Reply {
                data_url: Some(url),
                cache: "memory".into(),
                work_ms: start.elapsed().as_secs_f64() * 1000.0,
                ..Reply::empty("ready")
            });
        }
        drop(memory);
        let cache = files::Directory::open(&self.root.join("thumbnails"), true).ok();
        // Fixed 256 names bound disk use across restarts, without a directory scan.
        let slot = u8::from_str_radix(&id[..2], 16).map_err(|_| failure("invalidAsset"))?;
        let name = format!("thumb-v1-{slot:03}.bin");
        let old = cache
            .as_ref()
            .and_then(|c| c.read(&name, artwork::THUMB_BYTES as u64 + 33).ok());
        let hit = old
            .as_ref()
            .and_then(|v| cached(v))
            .filter(|(key, _)| *key == id);
        if let Some(record) = &old
            && cached(record).is_some()
        {
            self.disk.lock().unwrap().insert(slot, record.len() as u64);
        }
        let (png, cache_hit, decode_ms) = if let Some((_, png)) = hit {
            self.stats.lock().unwrap().disk_hits += 1;
            (png.to_vec(), "disk", 0.0)
        } else {
            let decode_start = std::time::Instant::now();
            let (_, _, _, png) = self.process(&bytes)?;
            if let Some(cache) = &cache {
                // Unknown files/unsafe objects in a cache slot are preserved.
                let may_publish = if old.as_ref().is_some_and(|b| cached(b).is_some()) {
                    cache.remove_derived(&name, old.as_ref().unwrap()).is_ok()
                } else {
                    !cache.path.join(&name).exists()
                };
                if may_publish {
                    let mut record = format!("{id}\n").into_bytes();
                    record.extend(&png);
                    if cache.publish(&name, &record).is_ok() {
                        self.disk.lock().unwrap().insert(slot, record.len() as u64);
                    }
                }
            }
            (png, "miss", decode_start.elapsed().as_secs_f64() * 1000.0)
        };
        let url = format!("data:image/png;base64,{}", STANDARD.encode(png));
        let mut memory = self.memory.lock().unwrap();
        memory.retain(|(key, _)| key != id);
        memory.push_back((id.into(), url.clone()));
        while memory.len() > artwork::MEMORY_ENTRIES {
            memory.pop_front();
        }
        Ok(Reply {
            data_url: Some(url),
            cache: cache_hit.into(),
            decode_ms,
            work_ms: start.elapsed().as_secs_f64() * 1000.0,
            ..Reply::empty("ready")
        })
    }
}
fn asset_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn cached(record: &[u8]) -> Option<(&str, &[u8])> {
    if record.len() < 34 || record[32] != b'\n' {
        return None;
    }
    let id = std::str::from_utf8(&record[..32]).ok()?;
    let png = &record[33..];
    (asset_id(id) && png.len() <= artwork::THUMB_BYTES && artwork::valid_thumbnail(png))
        .then_some((id, png))
}
struct Asset {
    id: String,
    format: &'static str,
    bytes: i64,
    width: u32,
    height: u32,
    fingerprint: String,
}
fn decode(bytes: &[u8]) -> Result<(&'static str, u32, u32, Vec<u8>)> {
    if bytes.is_empty() || bytes.len() as u64 > artwork::SOURCE_BYTES {
        return Err(failure("oversized"));
    }
    let format = image::guess_format(bytes).map_err(|_| failure("unsupported"))?;
    let label = match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpeg",
        _ => return Err(failure("unsupported")),
    };
    let (width, height) = ImageReader::with_format(Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(|_| failure("corrupt"))?;
    if width == 0
        || height == 0
        || width > 4096
        || height > 4096
        || u64::from(width) * u64::from(height) > artwork::SOURCE_PIXELS
    {
        return Err(failure("oversized"));
    }
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader.decode().map_err(|_| failure("corrupt"))?;
    let thumb = decoded
        .resize_exact(
            artwork::WIDTH,
            artwork::HEIGHT,
            image::imageops::FilterType::Triangle,
        )
        .to_rgb8();
    let mut png = Cursor::new(Vec::new());
    thumb
        .write_to(&mut png, ImageFormat::Png)
        .map_err(|_| failure("corrupt"))?;
    let png = png.into_inner();
    if png.len() > artwork::THUMB_BYTES {
        return Err(failure("oversized"));
    }
    Ok((label, width, height, png))
}

fn caller(label: &str) -> Result<()> {
    if label != "main" {
        Err(failure("unsupportedCaller"))
    } else {
        Ok(())
    }
}
#[tauri::command]
pub async fn artwork_choose(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, crate::DatabaseState>,
    budget: tauri::State<'_, Arc<Budget>>,
    request: Request,
) -> Result<bool> {
    caller(window.label())?;
    request.validate()?;
    if state.shutting_down.load(Ordering::SeqCst) {
        return Err(failure("stopping"));
    }
    let db = state.ready().map_err(|_| failure("stopping"))?;
    let service = db
        .lifecycle
        .as_ref()
        .ok_or_else(|| failure("wrongPurpose"))?
        .artwork
        .clone();
    let operation = service.admit(true)?;
    let pool = db.pool.clone();
    let budget = budget.inner().clone();
    #[cfg(windows)]
    let owner = window.hwnd().map_err(|_| failure("unavailable"))?.0 as usize;
    #[cfg(not(windows))]
    let owner = 0;
    tokio::spawn(async move {
        Service::context(&mut *pool.acquire().await?, &request).await?;
        let path = tokio::task::spawn_blocking(move || dialog(owner))
            .await
            .map_err(|_| failure("unavailable"))??;
        if let Some(path) = path {
            service
                .import(pool, budget, request, path, operation)
                .await?;
            Ok(true)
        } else {
            drop(operation);
            Ok(false)
        }
    })
    .await
    .map_err(|_| failure("unavailable"))?
}
#[tauri::command]
pub async fn artwork_clear(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, crate::DatabaseState>,
    request: Request,
) -> Result<()> {
    caller(window.label())?;
    request.validate()?;
    if state.shutting_down.load(Ordering::SeqCst) {
        return Err(failure("stopping"));
    }
    let db = state.ready().map_err(|_| failure("stopping"))?;
    let service = db
        .lifecycle
        .as_ref()
        .ok_or_else(|| failure("wrongPurpose"))?
        .artwork
        .clone();
    let operation = service.admit(true)?;
    let pool = db.pool.clone();
    tokio::spawn(async move {
        let _operation = operation;
        Service::assign(&pool, &request, None).await
    })
    .await
    .map_err(|_| failure("database"))?
}
#[cfg(windows)]
fn dialog(owner: usize) -> Result<Option<PathBuf>> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::UI::Controls::Dialogs::*;
    let mut filename = vec![0u16; 32768];
    let filter: Vec<u16> = "PNG or JPEG artwork\0*.png;*.jpg;*.jpeg\0\0"
        .encode_utf16()
        .collect();
    let title: Vec<u16> = "Choose local artwork\0".encode_utf16().collect();
    let mut options: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    options.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    options.hwndOwner = owner as _;
    options.lpstrFile = filename.as_mut_ptr();
    options.nMaxFile = filename.len() as u32;
    options.lpstrFilter = filter.as_ptr();
    options.lpstrTitle = title.as_ptr();
    options.Flags = OFN_EXPLORER
        | OFN_NOCHANGEDIR
        | OFN_PATHMUSTEXIST
        | OFN_FILEMUSTEXIST
        | OFN_DONTADDTORECENT;
    // SAFETY: buffers remain alive through synchronous native selection.
    if unsafe { GetOpenFileNameW(&mut options) } == 0 {
        return if unsafe { CommDlgExtendedError() } == 0 {
            Ok(None)
        } else {
            Err(failure("unavailable"))
        };
    }
    let end = filename
        .iter()
        .position(|c| *c == 0)
        .ok_or_else(|| failure("unsafePath"))?;
    Ok(Some(std::ffi::OsString::from_wide(&filename[..end]).into()))
}
#[cfg(not(windows))]
fn dialog(_: usize) -> Result<Option<PathBuf>> {
    Err(failure("unsupportedPlatform"))
}

#[cfg(test)]
mod tests;

// Compiled out of ordinary and benchmark artifacts. Only disposable subprocess
// tests set this hook; it cannot be selected through production IPC.
#[cfg(test)]
fn checkpoint(name: &str) {
    if std::env::var("GM_ARTWORK_CRASH_AT").as_deref() == Ok(name) {
        std::process::exit(89);
    }
}
#[cfg(not(test))]
#[inline]
fn checkpoint(_: &str) {}
