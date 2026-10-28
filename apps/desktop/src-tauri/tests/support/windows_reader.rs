// SPDX-License-Identifier: GPL-3.0-or-later
//! STOP: incomplete M1.3C feasibility candidate, NOT a verified safe reader.
//! Integration-test support ONLY, never application code. Required symlink and
//! broader native coverage are unavailable/incomplete; see M1.3C feasibility report.
//! Model B namespace authority; no exclusive historical object-provenance claim.
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::c_void,
    mem::{size_of, zeroed},
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr::{null, null_mut},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use windows_sys::{
    Wdk::{
        Foundation::OBJECT_ATTRIBUTES,
        Storage::FileSystem::{
            FILE_OPEN, FILE_OPEN_REPARSE_POINT, FILE_SYNCHRONOUS_IO_NONALERT,
            FileFsDeviceInformation, NtCreateFile, NtQueryVolumeInformationFile,
        },
    },
    Win32::{
        Foundation::*,
        Security::*,
        Storage::FileSystem::*,
        System::{
            IO::{CancelSynchronousIo, IO_STATUS_BLOCK},
            Threading::{GetCurrentProcess, OpenProcessToken},
        },
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Policy,
    Cancelled,
    Native(u32),
    Changed,
    Limit,
    Busy,
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Default, Debug)]
pub struct Audit {
    pub attribute_attempts: AtomicUsize,
    pub content_open_attempts: AtomicUsize,
    pub successful_opens: AtomicUsize,
    pub content_reads: AtomicUsize,
    pub bytes: AtomicUsize,
    pub handles: AtomicUsize,
    pub peak_handles: AtomicUsize,
    pub entries: AtomicUsize,
    pub peak_buffers: AtomicUsize,
    pub workers: AtomicUsize,
    pub peak_workers: AtomicUsize,
    /// Synthetic-only audit: [attribute attempts, content attempts, successes, reads].
    /// Bounded by the 25k leaf cap plus at most 40 acquired directory components.
    pub names: Mutex<BTreeMap<String, [usize; 4]>>,
}
impl Audit {
    fn note(&self, name: &str, column: usize) -> Result<()> {
        let mut names = self.names.lock().expect("audit lock");
        if !names.contains_key(name) && names.len() >= 25_040 {
            return Err(Error::Limit);
        }
        names.entry(name.into()).or_default()[column] += 1;
        Ok(())
    }
}

pub struct Handle {
    owned: OwnedHandle,
    audit: Arc<Audit>,
}
impl Handle {
    pub fn raw(&self) -> HANDLE {
        self.owned.as_raw_handle()
    }
    // SAFETY: caller transfers a unique, valid, non-pseudo native handle.
    unsafe fn own(raw: HANDLE, audit: &Arc<Audit>) -> Self {
        let n = audit.handles.fetch_add(1, Ordering::SeqCst) + 1;
        audit.peak_handles.fetch_max(n, Ordering::SeqCst);
        Self {
            owned: unsafe { OwnedHandle::from_raw_handle(raw) },
            audit: Arc::clone(audit),
        }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        self.audit.handles.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Clone, Default)]
pub struct Cancel(pub Arc<AtomicBool>);
impl Cancel {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn check(&self) -> Result<()> {
        if self.0.load(Ordering::SeqCst) {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
}

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}
fn win(ok: i32) -> Result<()> {
    if ok == 0 {
        Err(Error::Native(unsafe { GetLastError() }))
    } else {
        Ok(())
    }
}

/// Every relative call receives one already validated component, never metadata values.
/// No CASE_INSENSITIVE: aliases must not widen the exact namespace.
fn open(
    parent: Option<&Handle>,
    name: &str,
    content: bool,
    list: bool,
    audit: &Arc<Audit>,
    cancel: &Cancel,
) -> Result<Handle> {
    cancel.check()?;
    if parent.is_some() && !component(name) {
        return Err(Error::Policy);
    }
    audit.note(name, usize::from(content))?;
    if content {
        audit.content_open_attempts.fetch_add(1, Ordering::SeqCst);
    } else {
        audit.attribute_attempts.fetch_add(1, Ordering::SeqCst);
    }
    let mut text: Vec<u16> = name.encode_utf16().collect();
    let mut us = UNICODE_STRING {
        Length: (text.len() * 2) as u16,
        MaximumLength: (text.len() * 2) as u16,
        Buffer: text.as_mut_ptr(),
    };
    let oa = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: parent.map_or(null_mut(), Handle::raw),
        ObjectName: &mut us,
        Attributes: OBJ_DONT_REPARSE,
        SecurityDescriptor: null_mut(),
        SecurityQualityOfService: null_mut(),
    };
    let mut raw = null_mut();
    let mut io: IO_STATUS_BLOCK = unsafe { zeroed() };
    // SAFETY: all structures/buffers live through this synchronous call; no EAs or
    // creation. Single-component relative names plus no-reparse at every level.
    // Omit DIRECTORY_FILE (whose documented compatible flags omit OPEN_REPARSE_POINT).
    // Inspect the resulting type before using the handle as a directory or reading.
    let status = unsafe {
        NtCreateFile(
            &mut raw,
            FILE_READ_ATTRIBUTES | SYNCHRONIZE | if content || list { FILE_READ_DATA } else { 0 },
            &oa,
            &mut io,
            null(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            FILE_OPEN,
            FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
            null(),
            0,
        )
    };
    // Only STATUS_SUCCESS accepted, not informational success (e.g. oplock state).
    if status < 0 {
        return Err(Error::Native(status as u32));
    }
    let handle = unsafe { Handle::own(raw, audit) };
    audit.successful_opens.fetch_add(1, Ordering::SeqCst);
    audit.note(name, 2)?;
    if status != 0 {
        return Err(Error::Native(status as u32));
    }
    cancel.check()?;
    if parent.is_some() {
        exact_name(&handle, name)?;
    }
    Ok(handle)
}

fn exact_name(handle: &Handle, expected: &str) -> Result<()> {
    let mut buffer = [0u64; 512];
    win(unsafe {
        GetFileInformationByHandleEx(handle.raw(), FileNameInfo, buffer.as_mut_ptr().cast(), 4096)
    })?;
    let len = unsafe { *buffer.as_ptr().cast::<u32>() } as usize;
    if !len.is_multiple_of(2) || len > 4092 {
        return Err(Error::Policy);
    }
    let text = unsafe {
        std::slice::from_raw_parts(buffer.as_ptr().cast::<u8>().add(4).cast::<u16>(), len / 2)
    };
    let name = String::from_utf16(text).map_err(|_| Error::Policy)?;
    if name.rsplit('\\').next() != Some(expected) {
        return Err(Error::Policy);
    }
    Ok(())
}

fn component(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 120
        && s != "."
        && s != ".."
        && !s.ends_with(['.', ' '])
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-. ".contains(&b))
}
pub fn manifest_id(name: &str) -> Option<u32> {
    let id = name.strip_prefix("appmanifest_")?.strip_suffix(".acf")?;
    if id.is_empty()
        || id.len() > 10
        || id.starts_with('0')
        || !id.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    id.parse::<u32>().ok().filter(|id| *id != 0)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub volume: u64,
    pub file: [u8; 16],
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp {
    pub identity: Identity,
    pub index: u64,
    pub size: i64,
    pub written: i64,
    pub changed: i64,
    pub links: u32,
}

fn info<T: Default>(handle: &Handle, class: FILE_INFO_BY_HANDLE_CLASS) -> Result<T> {
    let mut out = T::default();
    // SAFETY: callers pair the exact SDK class and sized POD output structure.
    win(unsafe {
        GetFileInformationByHandleEx(
            handle.raw(),
            class,
            (&mut out as *mut T).cast(),
            size_of::<T>() as u32,
        )
    })?;
    Ok(out)
}
pub fn stamp(handle: &Handle, directory: bool) -> Result<Stamp> {
    if unsafe { GetFileType(handle.raw()) } != FILE_TYPE_DISK {
        return Err(Error::Policy);
    }
    let basic: FILE_BASIC_INFO = info(handle, FileBasicInfo)?;
    let standard: FILE_STANDARD_INFO = info(handle, FileStandardInfo)?;
    let id: FILE_ID_INFO = info(handle, FileIdInfo)?;
    let mut old: BY_HANDLE_FILE_INFORMATION = unsafe { zeroed() };
    win(unsafe { GetFileInformationByHandle(handle.raw(), &mut old) })?;
    // Ordinary NTFS directories and small regular metadata only. Refuse EFS,
    // sparse/offline/recall/reparse/device metadata rather than fetching payloads.
    let allowed = FILE_ATTRIBUTE_DIRECTORY
        | FILE_ATTRIBUTE_ARCHIVE
        | FILE_ATTRIBUTE_NORMAL
        | FILE_ATTRIBUTE_READONLY
        | FILE_ATTRIBUTE_HIDDEN
        | FILE_ATTRIBUTE_SYSTEM
        | FILE_ATTRIBUTE_NOT_CONTENT_INDEXED;
    if basic.FileAttributes & !allowed != 0
        || standard.DeletePending
        || standard.Directory != directory
        || (!directory && standard.NumberOfLinks != 1)
        || standard.EndOfFile < 0
        || (!directory && standard.EndOfFile > 65536)
    {
        return Err(Error::Policy);
    }
    Ok(Stamp {
        identity: Identity {
            volume: id.VolumeSerialNumber,
            file: id.FileId.Identifier,
        },
        index: (u64::from(old.nFileIndexHigh) << 32) | u64::from(old.nFileIndexLow),
        size: standard.EndOfFile,
        written: basic.LastWriteTime,
        changed: basic.ChangeTime,
        links: standard.NumberOfLinks,
    })
}

fn unelevated() -> Result<()> {
    let mut raw = null_mut();
    win(unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw) })?;
    let token = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut elevation: TOKEN_ELEVATION = unsafe { zeroed() };
    let mut len = 0;
    win(unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenElevation,
            (&mut elevation as *mut TOKEN_ELEVATION).cast(),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        )
    })?;
    if elevation.TokenIsElevated != 0 {
        Err(Error::Policy)
    } else {
        Ok(())
    }
}

fn anchor(path: &str, audit: &Arc<Audit>, cancel: &Cancel) -> Result<(Handle, Vec<String>)> {
    unelevated()?;
    let b = path.as_bytes();
    if b.len() < 4 || b.len() > 240 || !b[0].is_ascii_uppercase() || &b[1..3] != b":\\" {
        return Err(Error::Policy);
    }
    let parts: Vec<String> = path[3..].split('\\').map(str::to_owned).collect();
    if parts.len() > 32 || parts.iter().any(|p| !component(p)) {
        return Err(Error::Policy);
    }
    let drive = wide(&path[..2]);
    let mut target = [0u16; 512];
    let length =
        unsafe { QueryDosDeviceW(drive.as_ptr(), target.as_mut_ptr(), target.len() as u32) };
    if length == 0 {
        return Err(Error::Native(unsafe { GetLastError() }));
    }
    let end = target.iter().position(|c| *c == 0).ok_or(Error::Policy)?;
    let target = String::from_utf16(&target[..end]).map_err(|_| Error::Policy)?;
    let suffix = target
        .strip_prefix("\\Device\\HarddiskVolume")
        .ok_or(Error::Policy)?;
    if suffix.is_empty() || !suffix.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::Policy);
    }
    // The drive map is only a selection request. Open the validated direct kernel
    // device anchor, not a DOS alias. SUBST/redirectors fail the target grammar.
    let root = open(None, &format!("{target}\\"), false, false, audit, cancel)?;
    stamp(&root, true)?;
    #[repr(C)]
    struct Device {
        kind: u32,
        characteristics: u32,
    }
    let mut device = Device {
        kind: 0,
        characteristics: 0,
    };
    let mut io: IO_STATUS_BLOCK = unsafe { zeroed() };
    let status = unsafe {
        NtQueryVolumeInformationFile(
            root.raw(),
            &mut io,
            (&mut device as *mut Device).cast::<c_void>(),
            size_of::<Device>() as u32,
            FileFsDeviceInformation,
        )
    };
    if status != 0 {
        return Err(Error::Native(status as u32));
    }
    // FILE_DEVICE_DISK and no FILE_REMOVABLE_MEDIA (1), FILE_REMOTE_DEVICE (0x10)
    // or FILE_VIRTUAL_VOLUME (0x40). FILE_DEVICE_IS_MOUNTED (0x20) is expected.
    if device.kind != FILE_DEVICE_DISK || device.characteristics & (1 | 0x10 | 0x40) != 0 {
        return Err(Error::Policy);
    }
    let mut fs = [0u16; 32];
    win(unsafe {
        GetVolumeInformationByHandleW(
            root.raw(),
            null_mut(),
            0,
            null_mut(),
            null_mut(),
            null_mut(),
            fs.as_mut_ptr(),
            fs.len() as u32,
        )
    })?;
    if fs[..5] != [78, 84, 70, 83, 0] || unsafe { GetDriveTypeW(wide(&path[..3]).as_ptr()) } != 3 {
        return Err(Error::Policy);
    }
    Ok((root, parts))
}

struct Edge {
    handle: Handle,
    name: String,
    identity: Identity,
}
static ACTIVE: AtomicBool = AtomicBool::new(false);
struct Session;
impl Drop for Session {
    fn drop(&mut self) {
        ACTIVE.store(false, Ordering::SeqCst);
    }
}
pub struct Acquired {
    chain: Vec<Edge>,
    audit: Arc<Audit>,
    cancel: Cancel,
    _session: Session,
}
pub struct Approved {
    acquired: Acquired,
    apps: Handle,
    identity: Identity,
}
impl Acquired {
    pub fn acquire(path: &str, audit: Arc<Audit>, cancel: Cancel) -> Result<Self> {
        cancel.check()?;
        ACTIVE
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| Error::Busy)?;
        let session = Session;
        let (root, parts) = anchor(path, &audit, &cancel)?;
        let identity = stamp(&root, true)?.identity;
        let mut chain = vec![Edge {
            handle: root,
            name: String::new(),
            identity,
        }];
        for name in parts {
            let handle = open(
                Some(&chain.last().unwrap().handle),
                &name,
                false,
                false,
                &audit,
                &cancel,
            )?;
            let identity = stamp(&handle, true)?.identity;
            if identity.volume != chain[0].identity.volume {
                return Err(Error::Policy);
            }
            chain.push(Edge {
                handle,
                name,
                identity,
            });
        }
        Ok(Self {
            chain,
            audit,
            cancel,
            _session: session,
        })
    }
    pub fn identity(&self) -> &Identity {
        &self.chain.last().unwrap().identity
    }
    fn root(&self) -> &Handle {
        &self.chain.last().unwrap().handle
    }
    fn revalidate(&self) -> Result<()> {
        self.cancel.check()?;
        for pair in self.chain.windows(2) {
            let result = (|| {
                let next = open(
                    Some(&pair[0].handle),
                    &pair[1].name,
                    false,
                    false,
                    &self.audit,
                    &self.cancel,
                )?;
                if stamp(&next, true)?.identity != pair[1].identity {
                    return Err(Error::Changed);
                }
                Ok(())
            })();
            if result.is_err() {
                self.cancel.cancel();
                return result;
            }
        }
        Ok(())
    }
    /// Test-only explicit review: consuming the acquired capability binds approval
    /// to these owned handles. A path/boolean cannot construct Approved.
    pub fn approve(self, reviewed: &Identity) -> Result<Approved> {
        if reviewed != self.identity() {
            return Err(Error::Policy);
        }
        self.revalidate()?;
        let apps = open(
            Some(self.root()),
            "steamapps",
            false,
            true,
            &self.audit,
            &self.cancel,
        )?;
        let identity = stamp(&apps, true)?.identity;
        if identity.volume != self.identity().volume {
            return Err(Error::Policy);
        }
        Ok(Approved {
            acquired: self,
            apps,
            identity,
        })
    }
}
impl Approved {
    pub fn revalidate(&self) -> Result<()> {
        self.acquired.revalidate()?;
        let apps = open(
            Some(self.acquired.root()),
            "steamapps",
            false,
            false,
            &self.acquired.audit,
            &self.acquired.cancel,
        )?;
        if stamp(&apps, true)?.identity != self.identity {
            self.acquired.cancel.cancel();
            return Err(Error::Changed);
        }
        Ok(())
    }
    pub fn inspect_leaf(&self, name: &str, content_rights: bool) -> Result<Handle> {
        if manifest_id(name).is_none() {
            return Err(Error::Policy);
        }
        self.revalidate()?;
        let handle = open(
            Some(&self.apps),
            name,
            content_rights,
            false,
            &self.acquired.audit,
            &self.acquired.cancel,
        )?;
        let value = stamp(&handle, false)?;
        if value.identity.volume != self.identity.volume {
            return Err(Error::Policy);
        }
        Ok(handle)
    }
}

#[derive(Clone, Copy)]
pub struct Limits {
    pub entries: usize,
    pub records: usize,
    pub bytes: usize,
    pub elapsed: Duration,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            entries: 50_000,
            records: 25_000,
            bytes: 128 * 1024 * 1024,
            elapsed: Duration::from_secs(30),
        }
    }
}
#[derive(Debug, Default)]
pub struct Summary {
    pub accepted: usize,
    pub warnings: usize,
    pub retries: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    EnumerationBatch,
    Enumerated,
    BeforeOpen,
    BeforeRead,
    AfterFirstRead,
    AfterSecondRead,
}
struct Entry {
    name: String,
    index: u64,
}
struct Budget {
    limits: Limits,
    started: Instant,
    bytes: usize,
}
impl Budget {
    fn check(&self, cancel: &Cancel) -> Result<()> {
        cancel.check()?;
        if self.started.elapsed() >= self.limits.elapsed {
            Err(Error::Limit)
        } else {
            Ok(())
        }
    }
}

impl Approved {
    fn enumerate(
        &self,
        budget: &Budget,
        summary: &mut Summary,
        hook: &mut impl FnMut(Stage, &str),
    ) -> Result<Vec<Entry>> {
        // Alignment required by FILE_ID_BOTH_DIR_INFO. Fixed 64 KiB batch, no recursion.
        let mut buffer = vec![0u64; 8192];
        self.acquired
            .audit
            .peak_buffers
            .fetch_max(65536, Ordering::SeqCst);
        let mut entries = BTreeMap::new();
        let mut denied = BTreeSet::new();
        loop {
            budget.check(&self.acquired.cancel)?;
            hook(Stage::EnumerationBatch, "");
            budget.check(&self.acquired.cancel)?;
            self.revalidate()?;
            buffer.fill(0);
            let ok = unsafe {
                GetFileInformationByHandleEx(
                    self.apps.raw(),
                    FileIdBothDirectoryInfo,
                    buffer.as_mut_ptr().cast(),
                    65536,
                )
            };
            if ok == 0 {
                let error = unsafe { GetLastError() };
                if error == ERROR_NO_MORE_FILES {
                    break;
                }
                return Err(Error::Native(error));
            }
            let mut offset = 0usize;
            loop {
                budget.check(&self.acquired.cancel)?;
                let n = self.acquired.audit.entries.fetch_add(1, Ordering::SeqCst) + 1;
                if n > budget.limits.entries {
                    return Err(Error::Limit);
                }
                let name_offset = std::mem::offset_of!(FILE_ID_BOTH_DIR_INFO, FileName);
                if offset + size_of::<FILE_ID_BOTH_DIR_INFO>() > 65536 {
                    return Err(Error::Policy);
                }
                // SAFETY: native result is in aligned allocated storage; bounds checked
                // for the fixed header and flexible UTF-16 tail before accessing either.
                let record = unsafe {
                    &*buffer
                        .as_ptr()
                        .cast::<u8>()
                        .add(offset)
                        .cast::<FILE_ID_BOTH_DIR_INFO>()
                };
                let len = record.FileNameLength as usize;
                if !len.is_multiple_of(2) || len > 510 || offset + name_offset + len > 65536 {
                    return Err(Error::Policy);
                }
                let name = unsafe { std::slice::from_raw_parts(record.FileName.as_ptr(), len / 2) };
                // Ill-formed UTF-16 is unsupported, never a filesystem-open request.
                if let Ok(name) = String::from_utf16(name)
                    && let Some(id) = manifest_id(&name.to_ascii_lowercase())
                {
                    if manifest_id(&name).is_none() || entries.contains_key(&id) {
                        denied.insert(id);
                        summary.warnings += 1;
                    } else if entries.len() >= budget.limits.records {
                        return Err(Error::Limit);
                    } else {
                        entries.insert(
                            id,
                            Entry {
                                name,
                                index: record.FileId as u64,
                            },
                        );
                    }
                }
                let next = record.NextEntryOffset as usize;
                if next == 0 {
                    break;
                }
                if next < name_offset + len || !next.is_multiple_of(8) || offset + next >= 65536 {
                    return Err(Error::Policy);
                }
                offset += next;
            }
        }
        for id in denied {
            entries.remove(&id);
        }
        Ok(entries.into_values().collect())
    }

    fn observation(
        &self,
        entry: &Entry,
        budget: &mut Budget,
        hook: &mut impl FnMut(Stage, &str),
    ) -> Result<(Stamp, Vec<u8>)> {
        budget.check(&self.acquired.cancel)?;
        // Attribute-only inspection is separately audited. A later content-capable
        // open is not an atomic access upgrade; validate that handle too before ReadFile.
        let inspect = self.inspect_leaf(&entry.name, false)?;
        let before = stamp(&inspect, false)?;
        if before.index != entry.index {
            return Err(Error::Changed);
        }
        drop(inspect);
        budget.check(&self.acquired.cancel)?;
        let file = self.inspect_leaf(&entry.name, true)?;
        if stamp(&file, false)? != before {
            return Err(Error::Changed);
        }
        let mut bytes = Vec::with_capacity(65537);
        let mut chunk = [0u8; 16384];
        self.acquired
            .audit
            .peak_buffers
            .fetch_max(2 * 65537 + 16384, Ordering::SeqCst);
        loop {
            hook(Stage::BeforeRead, &entry.name);
            budget.check(&self.acquired.cancel)?;
            stamp(&file, false)?; // also reject newly observed multiple links before each read
            let remaining = budget.limits.bytes.saturating_sub(budget.bytes);
            if remaining == 0 {
                return Err(Error::Limit);
            }
            let count = chunk.len().min(65537 - bytes.len()).min(remaining);
            if count == 0 {
                return Err(Error::Policy);
            }
            let mut read = 0;
            self.acquired
                .audit
                .content_reads
                .fetch_add(1, Ordering::SeqCst);
            self.acquired.audit.note(&entry.name, 3)?;
            win(unsafe {
                ReadFile(
                    file.raw(),
                    chunk.as_mut_ptr(),
                    count as u32,
                    &mut read,
                    null_mut(),
                )
            })?;
            budget.bytes += read as usize;
            self.acquired
                .audit
                .bytes
                .fetch_add(read as usize, Ordering::SeqCst);
            bytes.extend_from_slice(&chunk[..read as usize]);
            budget.check(&self.acquired.cancel)?;
            if bytes.len() > 65536 {
                return Err(Error::Policy);
            }
            if read == 0 {
                break;
            }
        }
        let after = stamp(&file, false)?;
        if before != after || after.size as usize != bytes.len() {
            return Err(Error::Changed);
        }
        let current = self.inspect_leaf(&entry.name, false)?;
        if stamp(&current, false)? != after {
            return Err(Error::Changed);
        }
        Ok((after, bytes))
    }

    /// Consumes the authorization on every terminal outcome. Stable bodies are
    /// delivered one at a time; the reader never retains the aggregate input.
    pub fn scan(
        self,
        limits: Limits,
        mut consume: impl FnMut(&str, &[u8]) -> bool,
        mut hook: impl FnMut(Stage, &str),
    ) -> Result<Summary> {
        if limits.entries > 50_000
            || limits.records > 25_000
            || limits.bytes > 128 * 1024 * 1024
            || limits.elapsed > Duration::from_secs(30)
        {
            return Err(Error::Policy);
        }
        let mut budget = Budget {
            limits,
            started: Instant::now(),
            bytes: 0,
        };
        let mut summary = Summary::default();
        let entries = self.enumerate(&budget, &mut summary, &mut hook)?;
        hook(Stage::Enumerated, "");
        budget.check(&self.acquired.cancel)?;
        self.revalidate()?;
        for entry in entries {
            hook(Stage::BeforeOpen, &entry.name);
            budget.check(&self.acquired.cancel)?;
            self.revalidate()?;
            let mut accepted = false;
            for attempt in 0..2 {
                let result = (|| {
                    let a = self.observation(&entry, &mut budget, &mut hook)?;
                    hook(Stage::AfterFirstRead, &entry.name);
                    let b = self.observation(&entry, &mut budget, &mut hook)?;
                    hook(Stage::AfterSecondRead, &entry.name);
                    budget.check(&self.acquired.cancel)?;
                    self.revalidate()?;
                    let now = self.inspect_leaf(&entry.name, false)?;
                    if a != b || stamp(&now, false)? != b.0 {
                        return Err(Error::Changed);
                    }
                    Ok(b.1)
                })();
                match result {
                    Ok(body) => {
                        accepted = consume(&entry.name, &body);
                        break;
                    }
                    Err(Error::Changed) if attempt == 0 => {
                        summary.retries += 1;
                    }
                    Err(Error::Limit | Error::Cancelled) => {
                        return result.map(|_| summary);
                    }
                    Err(_) => {
                        self.acquired.cancel.check()?;
                        break;
                    }
                }
            }
            if accepted {
                summary.accepted += 1;
            } else {
                summary.warnings += 1;
            }
        }
        self.revalidate()?;
        Ok(summary)
    }
}

/// One dedicated worker; cancellation is repeatedly requested on that exact
/// thread until it finishes. Never detach or announce cleanup before join.
/// OS/driver failure can still delay completion: the oplock gate tests the selected
/// healthy local NTFS mechanism, not arbitrary driver behavior.
static WORKER_ACTIVE: AtomicBool = AtomicBool::new(false);
pub struct Worker<T> {
    thread: Option<std::thread::JoinHandle<T>>,
    cancel: Cancel,
    audit: Arc<Audit>,
    started: Instant,
}
impl<T: Send + 'static> Worker<T> {
    pub fn start(
        audit: Arc<Audit>,
        work: impl FnOnce(Cancel) -> T + Send + 'static,
    ) -> Result<Self> {
        if WORKER_ACTIVE
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(Error::Busy);
        }
        audit.workers.store(1, Ordering::SeqCst);
        audit.peak_workers.fetch_max(1, Ordering::SeqCst);
        let cancel = Cancel::default();
        let token = cancel.clone();
        let thread = match std::thread::Builder::new()
            .name("m13c-reader".into())
            .spawn(move || work(token))
        {
            Ok(thread) => thread,
            Err(_) => {
                audit.workers.store(0, Ordering::SeqCst);
                WORKER_ACTIVE.store(false, Ordering::SeqCst);
                return Err(Error::Policy);
            }
        };
        Ok(Self {
            thread: Some(thread),
            cancel,
            audit,
            started: Instant::now(),
        })
    }
    pub fn cancel(&self) {
        self.cancel.cancel();
    }
    pub fn finish(mut self) -> T {
        self.join().expect("isolated reader worker panicked")
    }
}
impl<T> Worker<T> {
    fn join(&mut self) -> std::thread::Result<T> {
        let thread = self.thread.take().expect("one terminal join");
        while !thread.is_finished() {
            if self.started.elapsed() >= Duration::from_secs(30) {
                self.cancel.cancel();
            }
            if self.cancel.check().is_err() {
                unsafe {
                    CancelSynchronousIo(thread.as_raw_handle());
                }
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let result = thread.join();
        self.audit.workers.store(0, Ordering::SeqCst);
        WORKER_ACTIVE.store(false, Ordering::SeqCst);
        result
    }
}
impl<T> Drop for Worker<T> {
    fn drop(&mut self) {
        if self.thread.is_some() {
            self.cancel.cancel();
            let _ = self.join();
        }
    }
}
