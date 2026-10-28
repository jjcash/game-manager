// SPDX-License-Identifier: GPL-3.0-or-later
//! M1.2 executable contract, compiled ONLY by the integration test target.
//! No filesystem, registry, network, credentials, Tauri or database access.
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
};

pub const MANIFEST_BYTES: usize = 64 * 1024;
pub const INDEX_BYTES: usize = 2 * 1024 * 1024;
const MAX_DEPTH: usize = 16;
const MAX_PAIRS: usize = 32_768;
const MAX_TOKEN: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Limit,
    Encoding,
    Syntax,
    DuplicateKey,
    InvalidId,
    InvalidPath,
    Changed,
    UnsafeSnapshot,
    SelectionRequired,
    Missing,
    Cancelled,
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Text(String),
    Object(BTreeMap<String, Value>),
}
impl Value {
    fn object(&self) -> Result<&BTreeMap<String, Value>> {
        match self {
            Self::Object(v) => Ok(v),
            _ => Err(Error::Syntax),
        }
    }
    fn text(&self) -> Result<&str> {
        match self {
            Self::Text(v) => Ok(v),
            _ => Err(Error::Syntax),
        }
    }
}

struct Parser<'a> {
    input: &'a [u8],
    position: usize,
    pairs: usize,
}
impl Parser<'_> {
    fn whitespace(&mut self) {
        loop {
            while self
                .input
                .get(self.position)
                .is_some_and(u8::is_ascii_whitespace)
            {
                self.position += 1;
            }
            if self.input.get(self.position..self.position + 2) != Some(b"//") {
                return;
            }
            while self.input.get(self.position).is_some_and(|b| *b != b'\n') {
                self.position += 1;
            }
        }
    }
    fn token(&mut self) -> Result<String> {
        self.whitespace();
        if self.input.get(self.position) != Some(&b'"') {
            return Err(Error::Syntax);
        }
        self.position += 1;
        let mut bytes = Vec::new();
        loop {
            let mut byte = *self.input.get(self.position).ok_or(Error::Syntax)?;
            self.position += 1;
            if byte == b'"' {
                break;
            }
            if byte == b'\\' {
                byte = *self.input.get(self.position).ok_or(Error::Syntax)?;
                self.position += 1;
                byte = match byte {
                    b'\\' | b'"' => byte,
                    b'n' => b'\n',
                    b'r' => b'\r',
                    b't' => b'\t',
                    _ => return Err(Error::Syntax),
                };
            } else if byte < 32 {
                return Err(Error::Syntax);
            }
            if bytes.len() >= MAX_TOKEN {
                return Err(Error::Limit);
            }
            bytes.push(byte);
        }
        String::from_utf8(bytes).map_err(|_| Error::Encoding)
    }
    fn members(&mut self, depth: usize, nested: bool) -> Result<BTreeMap<String, Value>> {
        if depth > MAX_DEPTH {
            return Err(Error::Limit);
        }
        let mut result = BTreeMap::new();
        loop {
            self.whitespace();
            match self.input.get(self.position) {
                None if !nested => return Ok(result),
                Some(b'}') if nested => {
                    self.position += 1;
                    return Ok(result);
                }
                None | Some(b'}') => return Err(Error::Syntax),
                _ => {}
            }
            if self.pairs >= MAX_PAIRS {
                return Err(Error::Limit);
            }
            self.pairs += 1;
            let key = self.token()?.to_ascii_lowercase();
            if key.is_empty() || key.starts_with('#') || key.chars().any(char::is_control) {
                return Err(Error::Syntax);
            }
            self.whitespace();
            let value = if self.input.get(self.position) == Some(&b'{') {
                self.position += 1;
                Value::Object(self.members(depth + 1, true)?)
            } else {
                Value::Text(self.token()?)
            };
            if result.insert(key, value).is_some() {
                return Err(Error::DuplicateKey);
            }
        }
    }
}
pub fn parse(input: &[u8], limit: usize) -> Result<BTreeMap<String, Value>> {
    if input.len() > limit {
        return Err(Error::Limit);
    }
    std::str::from_utf8(input).map_err(|_| Error::Encoding)?;
    if input.contains(&0) {
        return Err(Error::Encoding);
    }
    let input = input.strip_prefix(b"\xef\xbb\xbf").unwrap_or(input);
    Parser {
        input,
        position: 0,
        pairs: 0,
    }
    .members(0, false)
}
pub fn app_id(text: &str) -> Result<u32> {
    if text.is_empty() || text.len() > 10 || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::InvalidId);
    }
    let id = text.parse::<u32>().map_err(|_| Error::InvalidId)?;
    if id == 0 || text != id.to_string() {
        return Err(Error::InvalidId);
    }
    Ok(id)
}
pub fn manifest_id(filename: &str) -> Result<u32> {
    if filename.len() > 64 {
        return Err(Error::InvalidId);
    }
    let name = filename.to_ascii_lowercase();
    app_id(
        name.strip_prefix("appmanifest_")
            .and_then(|s| s.strip_suffix(".acf"))
            .ok_or(Error::InvalidId)?,
    )
}

pub fn component(value: &str) -> Result<()> {
    if value.is_empty()
        || value.trim() != value
        || value.encode_utf16().count() > 255
        || value.ends_with('.')
        || value
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
    {
        return Err(Error::InvalidPath);
    }
    let stem = value.split('.').next().unwrap_or("").to_uppercase();
    if matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|p| {
        stem.strip_prefix(p).is_some_and(|n| {
            matches!(
                n,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    }) {
        return Err(Error::InvalidPath);
    }
    Ok(())
}
/// Lexical candidate validation ONLY. This grants no read authority or confinement proof.
pub fn library_path(value: &str) -> Result<String> {
    if value.len() > 4096 {
        return Err(Error::InvalidPath);
    }
    let normalized = value.replace('/', "\\");
    let bytes = normalized.as_bytes();
    if bytes.len() < 4 || !bytes[0].is_ascii_alphabetic() || bytes[1..3] != *b":\\" {
        return Err(Error::InvalidPath);
    }
    let result = normalized.trim_end_matches('\\');
    if result.len() <= 3 {
        return Err(Error::InvalidPath);
    }
    for part in result[3..].split('\\') {
        component(part)?;
    }
    Ok(result.into())
}
/// Exactly the two metadata kinds proposed for the M1.3 installed-only preview.
pub fn allowed_relative_metadata(value: &str) -> bool {
    let Some(leaf) = value.strip_prefix("steamapps/") else {
        return false;
    };
    leaf == "libraryfolders.vdf" || manifest_id(leaf).is_ok()
}
#[derive(Debug)]
pub struct Folders {
    pub candidates: Vec<String>,
    pub rejected: usize,
}
pub fn folders(input: &[u8]) -> Result<Folders> {
    let document = parse(input, INDEX_BYTES)?;
    if document.len() != 1 {
        return Err(Error::Syntax);
    }
    let root = document
        .get("libraryfolders")
        .ok_or(Error::Syntax)?
        .object()?;
    let mut candidates = Vec::new();
    let mut seen = BTreeSet::new();
    let mut rejected = 0;
    let mut roots = 0;
    for (key, value) in root {
        if !key.bytes().all(|b| b.is_ascii_digit()) || key.parse::<u32>().is_err() {
            continue;
        }
        roots += 1;
        if roots > 32 {
            return Err(Error::Limit);
        }
        let path = match value {
            Value::Text(v) => Ok(v.as_str()),
            Value::Object(v) => v.get("path").ok_or(Error::Syntax).and_then(Value::text),
        };
        match path.and_then(library_path) {
            Ok(path) if seen.insert(path.to_ascii_lowercase()) => candidates.push(path),
            Ok(_) => {}
            Err(_) => rejected += 1,
        }
    }
    Ok(Folders {
        candidates,
        rejected,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Installation {
    ClientReportedPresent,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    ManifestRecord,
    CacheCandidate,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ownership {
    Unknown,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Observation {
    pub app_id: u32,
    pub title: String,
    pub install_dir: String,
    pub installation: Installation,
    pub visibility: Visibility,
    pub ownership: Ownership,
    pub potentially_stale: bool,
    pub fallback_title: bool,
}
pub fn manifest(filename: &str, input: &[u8]) -> Result<Observation> {
    let id = manifest_id(filename)?;
    let document = parse(input, MANIFEST_BYTES)?;
    if document.len() != 1 {
        return Err(Error::Syntax);
    }
    let values = document.get("appstate").ok_or(Error::Syntax)?.object()?;
    let actual = app_id(values.get("appid").ok_or(Error::InvalidId)?.text()?)?;
    if actual != id {
        return Err(Error::InvalidId);
    }
    let install_dir = values.get("installdir").ok_or(Error::InvalidPath)?.text()?;
    component(install_dir)?;
    let flags = values
        .get("stateflags")
        .and_then(|v| v.text().ok())
        .and_then(|s| {
            if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) {
                s.parse::<u64>().ok()
            } else {
                None
            }
        });
    // Conservative subset: FullyInstalled plus UpdateRequired/AppRunning only.
    // Other combinations retain the record without asserting present/absent.
    let installation = if flags.is_some_and(|f| f & 4 != 0 && f & !0x46 == 0) {
        Installation::ClientReportedPresent
    } else {
        Installation::Unknown
    };
    let name = values
        .get("name")
        .and_then(|v| v.text().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty() && v.chars().count() <= 200 && !v.chars().any(char::is_control));
    Ok(Observation {
        app_id: id,
        title: name
            .map(str::to_owned)
            .unwrap_or_else(|| format!("Steam app {id}")),
        install_dir: install_dir.into(),
        installation,
        visibility: Visibility::ManifestRecord,
        ownership: Ownership::Unknown,
        potentially_stale: true,
        fallback_title: name.is_none(),
    })
}
/// A numeric cache directory is an AppID hint, never proof of visible/owned/shared status.
pub fn cache_candidate(name: &str, directory: bool, reparse: bool) -> Option<u32> {
    if !directory || reparse {
        return None;
    }
    app_id(name).ok()
}
pub fn cache_classification() -> (Installation, Visibility, Ownership, bool) {
    (
        Installation::Unknown,
        Visibility::CacheCandidate,
        Ownership::Unknown,
        true,
    )
}
pub fn groups(rows: &[(String, Observation)]) -> Result<BTreeMap<u32, BTreeSet<(String, String)>>> {
    if rows.len() > 25_000 {
        return Err(Error::Limit);
    }
    let mut result: BTreeMap<u32, BTreeSet<(String, String)>> = BTreeMap::new();
    for (library_slot, observation) in rows {
        external_identity(library_slot, observation.app_id)?;
        result
            .entry(observation.app_id)
            .or_default()
            .insert((library_slot.clone(), observation.install_dir.clone()));
    }
    Ok(result)
}
#[derive(Debug, PartialEq, Eq)]
pub enum Association<'a> {
    Existing(&'a str),
    NewRequired,
    ReviewConflict,
}
/// The input is established Steam AppID mappings, never title matches.
pub fn association<'a>(existing_steam_mappings: &[&'a str]) -> Association<'a> {
    let unique: BTreeSet<_> = existing_steam_mappings.iter().copied().collect();
    match unique.len() {
        0 => Association::NewRequired,
        1 => Association::Existing(unique.first().copied().expect("one mapping")),
        _ => Association::ReviewConflict,
    }
}
#[derive(Debug)]
pub struct Preview {
    pub items: Vec<Observation>,
    pub warnings: Vec<(usize, Error)>,
}
pub fn preview(
    files: &[(&str, Option<&[u8]>)],
    cancelled: impl Fn(usize) -> bool,
) -> Result<Preview> {
    if files.len() > 25_000
        || files
            .iter()
            .map(|(_, b)| b.map_or(0, <[u8]>::len) as u64)
            .sum::<u64>()
            > 128 * 1024 * 1024
    {
        return Err(Error::Limit);
    }
    let mut result = Preview {
        items: Vec::new(),
        warnings: Vec::new(),
    };
    for (index, (name, bytes)) in files.iter().enumerate() {
        if cancelled(index) {
            return Err(Error::Cancelled);
        }
        let parsed = bytes
            .ok_or(Error::Missing)
            .and_then(|data| manifest(name, data));
        match parsed {
            Ok(v) => result.items.push(v),
            Err(e) => result.warnings.push((index, e)),
        }
    }
    if cancelled(files.len()) {
        return Err(Error::Cancelled);
    }
    Ok(result)
}

#[derive(Debug, PartialEq, Eq)]
pub enum AccountScope<'a> {
    UnattributedClient,
    UserSelected(&'a str),
}
pub fn account_scope<'a>(
    known: &[&str],
    explicit_choice: Option<&'a str>,
) -> Result<AccountScope<'a>> {
    match explicit_choice {
        Some("continue-unattributed") => Ok(AccountScope::UnattributedClient),
        Some(id) if known.contains(&id) => Ok(AccountScope::UserSelected(id)),
        _ => Err(Error::SelectionRequired),
    }
}
pub fn external_identity(client_scope: &str, id: u32) -> Result<(String, String)> {
    if client_scope.is_empty()
        || client_scope.len() > 64
        || !client_scope
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        || id == 0
    {
        return Err(Error::InvalidId);
    }
    Ok((
        format!("steam.local.v1/client/{client_scope}"),
        id.to_string(),
    ))
}
/// EXISTENCE of a user row wins even when its value is NULL (Some(None)).
pub fn effective_value(user_row: Option<Option<&str>>, provider: Option<&str>) -> Option<String> {
    match user_row {
        Some(v) => v.map(str::to_owned),
        None => provider.map(str::to_owned),
    }
}
/// No local absence ever revokes a membership or canonical identity.
pub fn may_propose_installation_missing(
    complete_approved_scan: bool,
    manifest_missing: bool,
) -> bool {
    complete_approved_scan && manifest_missing
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub file_id: u64,
    pub modified: u64,
    pub length: u64,
    pub regular: bool,
    pub reparse: bool,
}
pub fn read_bounded(reader: impl Read, limit: usize) -> Result<Vec<u8>> {
    let mut result = Vec::new();
    reader
        .take(limit as u64 + 1)
        .read_to_end(&mut result)
        .map_err(|_| Error::Changed)?;
    if result.len() > limit {
        return Err(Error::Limit);
    }
    Ok(result)
}
/// Models reader-supplied identity/metadata proofs and two bounded reads.
/// It does NOT open paths, establish Windows handle confinement, or prove a vendor transaction.
pub fn stable_snapshot(
    before: Stamp,
    after: Stamp,
    first: &[u8],
    second: &[u8],
    limit: usize,
) -> Result<Vec<u8>> {
    if !before.regular || !after.regular || before.reparse || after.reparse {
        return Err(Error::UnsafeSnapshot);
    }
    if before.length > limit as u64 || after.length > limit as u64 {
        return Err(Error::Limit);
    }
    if before != after || first.len() as u64 != before.length || first != second {
        return Err(Error::Changed);
    }
    read_bounded(first, limit)
}
#[derive(Debug, PartialEq, Eq)]
pub enum BinaryGate {
    KnownButDeferred(u8),
    Unsupported,
    Truncated,
}
pub fn binary_gate(prefix: &[u8], package: bool) -> BinaryGate {
    let Some(header) = prefix.get(..4) else {
        return BinaryGate::Truncated;
    };
    let magic = u32::from_le_bytes(header.try_into().expect("four-byte header"));
    match (package, magic) {
        (false, 0x07564427) => BinaryGate::KnownButDeferred(39),
        (false, 0x07564428) => BinaryGate::KnownButDeferred(40),
        (false, 0x07564429) => BinaryGate::KnownButDeferred(41),
        (true, 0x06565527) => BinaryGate::KnownButDeferred(39),
        (true, 0x06565528) => BinaryGate::KnownButDeferred(40),
        _ => BinaryGate::Unsupported,
    }
}
