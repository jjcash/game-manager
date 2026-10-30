// SPDX-License-Identifier: GPL-3.0-or-later
//! M1.8 native boundary adapted from the verified M1.7 candidate.
use super::{Failure, Request};
use crate::storage::launch_profiles::{Configuration, PathRef};
use sqlx::{Connection, FromRow, SqliteConnection};
use std::{
    ffi::OsStr,
    io,
    os::windows::{ffi::OsStrExt, io::FromRawHandle, io::OwnedHandle},
    path::{Path, PathBuf},
    ptr,
};
use windows_sys::Win32::{
    Foundation::{HANDLE_FLAG_INHERIT, SetHandleInformation},
    Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation},
    Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE,
    },
    System::Threading::{
        CREATE_NO_WINDOW, CREATE_UNICODE_ENVIRONMENT, CreateProcessW,
        DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT, GetCurrentProcess,
        InitializeProcThreadAttributeList, OpenProcessToken, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
        PROCESS_INFORMATION, STARTF_USESTDHANDLES, STARTUPINFOEXW, UpdateProcThreadAttribute,
    },
};

type Result<T> = std::result::Result<T, Failure>;
use super::{native, reject};

// A single indexed join, not the editor's list API or a library scan. Defensive
// snapshot comparison supplements (does not replace) existing revisions.
#[derive(Debug, PartialEq, Eq, FromRow)]
struct Context {
    game_revision: i64,
    profile_revision: i64,
    installation_id: String,
    entry_id: String,
    source_id: String,
    source_type: String,
    external_item_id: Option<String>,
    root_path_ref: String,
    platform: String,
    version: Option<String>,
    detection_state: String,
    state_origin: String,
    provenance: String,
    launcher_kind: String,
    display_name: String,
    executable_kind: String,
    executable_value: String,
    arguments_json: String,
    working_directory_kind: Option<String>,
    working_directory_value: Option<String>,
    default_profile_id: Option<String>,
}
const LOOKUP: &str = "SELECT g.revision game_revision,p.revision profile_revision,p.installation_id,i.entry_id,e.source_id,s.source_type,e.external_item_id,i.root_path_ref,i.platform,i.version,i.detection_state,i.state_origin,p.provenance,p.launcher_kind,p.display_name,p.executable_kind,p.executable_value,p.arguments_json,p.working_directory_kind,p.working_directory_value,d.profile_id default_profile_id FROM launch_profiles p JOIN game_installations i ON i.id=p.installation_id JOIN library_entries e ON e.id=i.entry_id JOIN library_sources s ON s.id=e.source_id JOIN games g ON g.id=e.game_id LEFT JOIN game_launch_defaults d ON d.game_id=g.id WHERE p.id=? AND g.id=?";
async fn context(c: &mut SqliteConnection, r: &Request) -> Result<Context> {
    if r.expected_game_revision < 0
        || r.expected_profile_revision < 0
        || [&r.game_id, &r.profile_id].iter().any(|id| {
            id.len() != 32
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
    {
        return Err(reject("request", "invalidRequest"));
    }
    let purpose: String =
        sqlx::query_scalar("SELECT purpose FROM database_metadata WHERE singleton=1")
            .fetch_one(&mut *c)
            .await?;
    if purpose != "application" {
        return Err(reject("request", "wrongPurpose"));
    }
    let row: Context = sqlx::query_as(LOOKUP)
        .bind(&r.profile_id)
        .bind(&r.game_id)
        .fetch_optional(c)
        .await?
        .ok_or_else(|| reject("request", "missingOrUnrelated"))?;
    if row.game_revision != r.expected_game_revision
        || row.profile_revision != r.expected_profile_revision
    {
        return Err(reject("request", "staleContext"));
    }
    if row.provenance != "user"
        || row.launcher_kind != "standalone"
        || row.source_type != "manual"
        || row.platform != "windows"
    {
        return Err(reject("request", "unsupportedContext"));
    }
    Ok(row)
}
fn reference(kind: &str, value: &str) -> Result<PathRef> {
    match kind {
        "absolutePath" => Ok(PathRef::AbsolutePath(value.into())),
        "installationRelative" => Ok(PathRef::InstallationRelative(value.into())),
        _ => Err(reject("resolution", "unsupportedReference")),
    }
}
impl Context {
    fn configuration(&self) -> Result<Configuration> {
        let wd = match (&self.working_directory_kind, &self.working_directory_value) {
            (None, None) => None,
            (Some(k), Some(v)) => Some(reference(k, v)?),
            _ => return Err(reject("resolution", "invalidReference")),
        };
        if self.arguments_json.len() > 131072 {
            return Err(reject("request", "invalidConfiguration"));
        }
        let config = Configuration {
            display_name: self.display_name.clone(),
            launcher_kind: self.launcher_kind.clone(),
            executable_ref: reference(&self.executable_kind, &self.executable_value)?,
            arguments: serde_json::from_str(&self.arguments_json)
                .map_err(|_| reject("request", "invalidArguments"))?,
            working_directory_ref: wd,
        };
        config
            .validate()
            .map_err(|_| reject("request", "invalidConfiguration"))?;
        Ok(config)
    }
}
fn resolve(reference: &PathRef, root: &str) -> Result<PathBuf> {
    reference
        .validate()
        .map_err(|_| reject("resolution", "invalidReference"))?;
    let path = match reference {
        PathRef::AbsolutePath(value) => PathBuf::from(value),
        PathRef::InstallationRelative(value) => {
            PathRef::AbsolutePath(root.into())
                .validate()
                .map_err(|_| reject("resolution", "invalidInstallationLocation"))?;
            let mut path = PathBuf::from(root);
            // Validate before join: rooted/prefix/parent components cannot replace root.
            for component in Path::new(value).components() {
                if !matches!(component, std::path::Component::Normal(_)) {
                    return Err(reject("resolution", "invalidReference"));
                }
                path.push(component);
            }
            path
        }
    };
    if !path.is_absolute() {
        return Err(reject("resolution", "invalidReference"));
    }
    Ok(path)
}
#[derive(Debug)]
struct Specification {
    executable: PathBuf,
    directory: PathBuf,
    arguments: Vec<String>,
}
fn specification(row: &Context) -> Result<Specification> {
    let config = row.configuration()?;
    let executable = resolve(&config.executable_ref, &row.root_path_ref)?;
    if !executable
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
    {
        return Err(reject("resolution", "unsupportedExecutable"));
    }
    let directory = match &config.working_directory_ref {
        Some(r) => resolve(r, &row.root_path_ref)?,
        None => executable
            .parent()
            .ok_or_else(|| reject("resolution", "invalidParent"))?
            .to_path_buf(),
    };
    let spec = Specification {
        executable,
        directory,
        arguments: config.arguments,
    };
    command_line(&spec)?;
    Ok(spec)
}
#[derive(Debug)]
pub(crate) struct Prepared {
    request: Request,
    context: Context,
}
pub(crate) async fn prepare(c: &mut SqliteConnection, request: Request) -> Result<Prepared> {
    let mut tx = c.begin().await?;
    let row = context(&mut tx, &request).await?;
    specification(&row)?;
    // Prepared snapshot instant: all joined records and lexical config validated.
    tx.commit().await?;
    Ok(Prepared {
        request,
        context: row,
    })
}

// Windows CRT/Rust convention: always quote, double backslashes before a quote
// or closing delimiter, escape literal quotes. Never a shell command encoder.
fn quote(out: &mut Vec<u16>, value: &OsStr) {
    out.push(b'"' as u16);
    let mut slashes = 0;
    for unit in value.encode_wide() {
        if unit == b'\\' as u16 {
            slashes += 1;
            continue;
        }
        out.extend(std::iter::repeat_n(
            b'\\' as u16,
            if unit == b'"' as u16 {
                slashes * 2 + 1
            } else {
                slashes
            },
        ));
        slashes = 0;
        out.push(unit);
    }
    out.extend(std::iter::repeat_n(b'\\' as u16, slashes * 2));
    out.push(b'"' as u16);
}
fn command_line(spec: &Specification) -> Result<Vec<u16>> {
    let mut line = Vec::new();
    quote(&mut line, spec.executable.as_os_str());
    for argument in &spec.arguments {
        line.push(b' ' as u16);
        quote(&mut line, OsStr::new(argument));
    }
    line.push(0);
    if line.len() > 32767 {
        return Err(reject("resolution", "encodedArgumentsTooLong"));
    }
    Ok(line)
}
fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}
pub(super) fn unelevated() -> Result<()> {
    let mut token = ptr::null_mut();
    // SAFETY: valid current-process pseudo handle and output; owned on success.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(native("request", io::Error::last_os_error()));
    }
    let token = unsafe { OwnedHandle::from_raw_handle(token) };
    use std::os::windows::io::AsRawHandle;
    let mut elevation = TOKEN_ELEVATION::default();
    let mut returned = 0;
    // SAFETY: live token, correctly sized writable TOKEN_ELEVATION and length.
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenElevation,
            (&mut elevation as *mut TOKEN_ELEVATION).cast(),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        )
    } == 0
    {
        return Err(native("request", io::Error::last_os_error()));
    }
    if elevation.TokenIsElevated != 0 {
        return Err(reject("request", "elevatedParent"));
    }
    Ok(())
}
pub(super) struct Created {
    pub pid: u32,
    pub handle: OwnedHandle,
}

// Native opaque attribute storage aligned to pointer width. Keeps the backing
// allocation stable and deletes initialized native state on every exit path.
struct Attributes(Vec<usize>);
impl Drop for Attributes {
    fn drop(&mut self) {
        unsafe { DeleteProcThreadAttributeList(self.0.as_mut_ptr().cast()) };
    }
}
fn spawn(spec: &Specification) -> Result<Created> {
    unelevated()?;
    use std::os::windows::fs::OpenOptionsExt;
    let file = std::fs::metadata(&spec.executable).map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            reject("resolution", "missingExecutable")
        } else {
            native("resolution", e)
        }
    })?;
    // Attribute-only directory open: catches denied access without enumeration,
    // content reads, exclusive locking or a claim of reparse/object confinement.
    let directory_handle = std::fs::OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(&spec.directory)
        .map_err(|e| {
            if e.kind() == io::ErrorKind::PermissionDenied {
                native("resolution", e)
            } else {
                reject("resolution", "invalidWorkingDirectory")
            }
        })?;
    let directory = directory_handle.metadata().map_err(|e| {
        if e.kind() == io::ErrorKind::PermissionDenied {
            native("resolution", e)
        } else {
            reject("resolution", "invalidWorkingDirectory")
        }
    })?;
    if !file.is_file() {
        return Err(reject("resolution", "missingExecutable"));
    }
    if !directory.is_dir() {
        return Err(reject("resolution", "invalidWorkingDirectory"));
    }
    let exe = wide(spec.executable.as_os_str());
    let cwd = wide(spec.directory.as_os_str());
    let mut line = command_line(spec)?;
    use std::os::windows::io::AsRawHandle;
    let null = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("NUL")
        .map_err(|e| native("creation", e))?;
    if unsafe {
        SetHandleInformation(
            null.as_raw_handle(),
            HANDLE_FLAG_INHERIT,
            HANDLE_FLAG_INHERIT,
        )
    } == 0
    {
        return Err(native("creation", io::Error::last_os_error()));
    }
    let mut bytes = 0;
    // SAFETY: first call asks Windows for required opaque storage size.
    unsafe { InitializeProcThreadAttributeList(ptr::null_mut(), 1, 0, &mut bytes) };
    if bytes == 0 {
        return Err(native("creation", io::Error::last_os_error()));
    }
    let mut buffer = vec![0usize; bytes.div_ceil(size_of::<usize>())];
    if unsafe { InitializeProcThreadAttributeList(buffer.as_mut_ptr().cast(), 1, 0, &mut bytes) }
        == 0
    {
        return Err(native("creation", io::Error::last_os_error()));
    }
    let handles = [null.as_raw_handle()];
    // Drop the attribute list before its borrowed handle array (Win32 requires
    // lpValue storage to remain alive through DeleteProcThreadAttributeList).
    let mut attributes = Attributes(buffer);
    // SAFETY: initialized attribute list, one valid inheritable NUL handle;
    // handles array lives through list destruction; backing storage never moves.
    if unsafe {
        UpdateProcThreadAttribute(
            attributes.0.as_mut_ptr().cast(),
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            handles.as_ptr().cast(),
            size_of_val(&handles),
            ptr::null_mut(),
            ptr::null(),
        )
    } == 0
    {
        return Err(native("creation", io::Error::last_os_error()));
    }
    let mut startup = STARTUPINFOEXW {
        lpAttributeList: attributes.0.as_mut_ptr().cast(),
        ..Default::default()
    };
    startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = null.as_raw_handle();
    startup.StartupInfo.hStdOutput = null.as_raw_handle();
    startup.StartupInfo.hStdError = null.as_raw_handle();
    let mut info = PROCESS_INFORMATION::default();
    // SAFETY: NUL-terminated absolute executable/directory, mutable bounded line;
    // only NUL inherited, default token/environment, no callback or retained ptr.
    if unsafe {
        CreateProcessW(
            exe.as_ptr(),
            line.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            1,
            CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT | EXTENDED_STARTUPINFO_PRESENT,
            ptr::null(),
            cwd.as_ptr(),
            &startup.StartupInfo,
            &mut info,
        )
    } == 0
    {
        return Err(native("creation", io::Error::last_os_error()));
    }
    // SAFETY: success transfers exactly these two native handles to this scope.
    let handle = unsafe { OwnedHandle::from_raw_handle(info.hProcess) };
    let thread = unsafe { OwnedHandle::from_raw_handle(info.hThread) };
    drop(thread);
    Ok(Created {
        pid: info.dwProcessId,
        handle,
    })
}
// The caller owns BEGIN IMMEDIATE and launch admission through this synchronous
// boundary. A successful result owns a process handle; production drops it immediately.
pub(super) fn create(row: Prepared) -> Result<Created> {
    spawn(&specification(&row.context)?)
}
pub(crate) async fn revalidate(c: &mut SqliteConnection, prepared: &Prepared) -> Result<()> {
    let row = context(c, &prepared.request).await?;
    if row != prepared.context {
        return Err(reject("request", "staleContext"));
    }
    specification(&row)?;
    Ok(())
}
