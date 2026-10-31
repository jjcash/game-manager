// SPDX-License-Identifier: GPL-3.0-or-later
//! Retained no-reparse parent handles prevent rename/substitution while accessing
//! a selected or owned file. Leaves reject observed hard links before reads.
use super::{Result, failure};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub(super) struct Directory {
    pub path: PathBuf,
    _handles: Vec<File>,
}
impl Directory {
    pub fn open(path: &Path, create: bool) -> Result<Self> {
        crate::storage::validate_path(path).map_err(|_| failure("unsafePath"))?;
        // Preserve the central path resolver's accepted separator spellings.
        // Components normalize separators without following links; validate the
        // original first so parent traversal cannot disappear during normalization.
        let normalized: PathBuf = path.components().collect();
        let path = normalized.as_path();
        #[cfg(windows)]
        {
            let value = path.to_str().ok_or_else(|| failure("unsafePath"))?;
            // Ordinary drive paths only. No extended/device syntax, ADS, aliases,
            // trailing dots/spaces or reserved DOS names, including parent names.
            if value.len() < 3
                || value.encode_utf16().count() > 2048
                || path.components().count() > 256
                || value.as_bytes()[1] != b':'
                || !value.as_bytes()[0].is_ascii_alphabetic()
                || value.as_bytes()[2] != b'\\'
                || value[2..].contains([':', '/'])
            {
                return Err(failure("unsafePath"));
            }
            use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
            let drive: Vec<u16> = value[..3].encode_utf16().chain(Some(0)).collect();
            // SAFETY: terminated drive-root buffer valid for this synchronous call.
            if !matches!(
                unsafe { GetDriveTypeW(drive.as_ptr()) },
                2 | 3 // DRIVE_REMOVABLE | DRIVE_FIXED
            ) {
                return Err(failure("unsafePath"));
            }
            for part in value[3..].split('\\').filter(|p| !p.is_empty()) {
                if !ordinary_component(part) {
                    return Err(failure("unsafePath"));
                }
            }
        }
        let mut current = PathBuf::new();
        let mut handles = Vec::new();
        for component in path.components() {
            current.push(component);
            if !current.is_absolute() {
                continue;
            }
            if create && !current.exists() {
                std::fs::create_dir(&current).map_err(|_| failure("storage"))?;
            }
            let mut options = OpenOptions::new();
            options.read(true);
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt;
                use windows_sys::Win32::Storage::FileSystem::*;
                options
                    .access_mode(FILE_READ_ATTRIBUTES)
                    .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
                    .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT);
            }
            let handle = options.open(&current).map_err(|_| failure("unavailable"))?;
            check(&handle, true)?;
            handles.push(handle);
        }
        Ok(Self {
            path: path.into(),
            _handles: handles,
        })
    }
    pub fn read(&self, name: &str, limit: u64) -> Result<Vec<u8>> {
        let file = open_leaf(&self.path.join(name), false)?;
        let size = file.metadata().map_err(|_| failure("unavailable"))?.len();
        if size == 0 || size > limit {
            return Err(failure("oversized"));
        }
        let mut bytes = Vec::with_capacity(size as usize);
        file.take(limit + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| failure("unavailable"))?;
        if bytes.len() as u64 != size {
            return Err(failure("unavailable"));
        }
        Ok(bytes)
    }
    pub fn publish(&self, name: &str, bytes: &[u8]) -> Result<()> {
        let mut temporary = tempfile::Builder::new()
            .prefix(".gm-artwork-")
            .tempfile_in(&self.path)
            .map_err(|_| failure("storage"))?;
        temporary
            .write_all(bytes)
            .and_then(|_| temporary.as_file().sync_all())
            .map_err(|_| failure("storage"))?;
        super::checkpoint("temporary-synced");
        // Hard-link publication is create-if-absent, never replacement. Tempfile
        // removes only its own temporary name; crash leftovers are preserved.
        std::fs::hard_link(temporary.path(), self.path.join(name))
            .map_err(|_| failure("storage"))?;
        super::checkpoint("published");
        drop(temporary);
        if self.read(name, bytes.len() as u64)? != bytes {
            return Err(failure("storage"));
        }
        Ok(())
    }
    pub fn remove_derived(&self, name: &str, expected: &[u8]) -> Result<()> {
        let mut file = open_leaf(&self.path.join(name), true)?;
        let mut actual = Vec::new();
        (&mut file)
            .take(crate::artwork::THUMB_BYTES as u64 + 34)
            .read_to_end(&mut actual)
            .map_err(|_| failure("storage"))?;
        if actual != expected || super::cached(&actual).is_none() {
            return Err(failure("unsafeObject"));
        }
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::Storage::FileSystem::*;
            let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
            // SAFETY: valid exclusively held, verified regular single-link file.
            if unsafe {
                SetFileInformationByHandle(
                    file.as_raw_handle() as _,
                    FileDispositionInfo,
                    &disposition as *const _ as _,
                    std::mem::size_of_val(&disposition) as u32,
                )
            } == 0
            {
                return Err(failure("storage"));
            }
        }
        #[cfg(not(windows))]
        {
            drop(file);
            std::fs::remove_file(self.path.join(name)).map_err(|_| failure("storage"))?;
        }
        Ok(())
    }
}
fn check(file: &File, directory: bool) -> Result<()> {
    let metadata = file.metadata().map_err(|_| failure("unavailable"))?;
    if metadata.is_dir() != directory || (!directory && !metadata.is_file()) {
        return Err(failure("unsafeObject"));
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::*;
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        // SAFETY: valid live file handle, correctly sized output.
        if unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, &mut info) } == 0
            || info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
            || (!directory && info.nNumberOfLinks != 1)
        {
            return Err(failure("unsafeObject"));
        }
    }
    Ok(())
}
fn open_leaf(path: &Path, delete: bool) -> Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::*;
        options
            .share_mode(if delete { 0 } else { FILE_SHARE_READ })
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
        if delete {
            options.access_mode(FILE_GENERIC_READ | 0x00010000);
        }
    }
    let file = options.open(path).map_err(|_| failure("unavailable"))?;
    check(&file, false)?;
    Ok(file)
}
pub(super) fn selected(path: &Path) -> Result<Vec<u8>> {
    let parent = Directory::open(path.parent().ok_or_else(|| failure("unsafePath"))?, false)?;
    // Validate the leaf with the same lexical checks, without opening it as a directory.
    #[cfg(windows)]
    {
        let leaf = path
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or_else(|| failure("unsafePath"))?;
        if !ordinary_component(leaf) {
            return Err(failure("unsafePath"));
        }
    }
    parent.read(
        path.file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| failure("unsafePath"))?,
        crate::artwork::SOURCE_BYTES,
    )
}

#[cfg(windows)]
fn ordinary_component(part: &str) -> bool {
    let stem = part
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    !part.is_empty()
        && !part.ends_with(['.', ' '])
        && part != "."
        && part != ".."
        && !part
            .chars()
            .any(|c| c.is_control() || "<>\"|?*:/\\".contains(c))
        && !matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        )
        && !(stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit())
}
