// SPDX-License-Identifier: GPL-3.0-or-later
//! Literal user configuration only; lexical validation never accesses a target.
use super::{Database, ErrorCode, Result, StorageError, manual};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqliteConnection};

pub const MAX_PROFILES: usize = 100;
fn invalid(message: &str) -> StorageError {
    StorageError::new(ErrorCode::InvalidRequest, "launch_profiles", message)
}
fn conflict(message: &str) -> StorageError {
    StorageError::new(ErrorCode::Conflict, "launch_profiles", message)
}
fn valid_id(id: &str) -> Result<()> {
    if id.is_empty() || id.len() > 128 || id.chars().any(char::is_control) || id.trim() != id {
        return Err(invalid("Invalid record ID"));
    }
    Ok(())
}
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum PathRef {
    InstallationRelative(String),
    AbsolutePath(String),
}
impl PathRef {
    fn parts(&self) -> (&str, &str) {
        match self {
            Self::InstallationRelative(v) => ("installationRelative", v),
            Self::AbsolutePath(v) => ("absolutePath", v),
        }
    }
    fn from_parts(kind: &str, value: String) -> Result<Self> {
        match kind {
            "installationRelative" => Ok(Self::InstallationRelative(value)),
            "absolutePath" => Ok(Self::AbsolutePath(value)),
            _ => Err(invalid("Unsupported reference kind")),
        }
    }
    pub fn validate(&self) -> Result<()> {
        let (_, value) = self.parts();
        // Deliberately conservative Windows lexical grammar: backslash separators,
        // drive-rooted local absolute paths, no namespace/ADS/dot/device aliases.
        if value.is_empty() || value.chars().count() > 2048 || value.chars().any(char::is_control) {
            return Err(invalid(
                "References require 1–2048 characters without controls",
            ));
        }
        let tail = match self {
            Self::InstallationRelative(_) => value,
            Self::AbsolutePath(_) => {
                let b = value.as_bytes();
                if b.len() < 3 || !b[0].is_ascii_alphabetic() || b[1] != b':' || b[2] != b'\\' {
                    return Err(invalid(
                        "Absolute references require a local drive path such as C:\\Games\\game.exe",
                    ));
                }
                &value[3..]
            }
        };
        if tail.is_empty() && matches!(self, Self::AbsolutePath(_)) {
            return Ok(());
        }
        for component in tail.split('\\') {
            let stem = component.split('.').next().unwrap_or("").to_uppercase();
            let reserved = matches!(
                stem.as_str(),
                "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
            ) || ["COM", "LPT"].iter().any(|prefix| {
                stem.strip_prefix(prefix).is_some_and(|n| {
                    matches!(
                        n,
                        "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                    )
                })
            });
            if component.is_empty()
                || component == "."
                || component == ".."
                || component.ends_with([' ', '.'])
                || component.chars().any(|c| "<>:\"/|?*".contains(c))
                || reserved
            {
                return Err(invalid(
                    "Use backslash-separated Windows components without traversal, namespaces, device names, trailing dots/spaces or reserved punctuation",
                ));
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Configuration {
    pub display_name: String,
    pub launcher_kind: String,
    pub executable_ref: PathRef,
    pub arguments: Vec<String>,
    pub working_directory_ref: Option<PathRef>,
}
impl Configuration {
    pub fn validate(&self) -> Result<()> {
        if self.display_name.trim().is_empty()
            || self.display_name.chars().count() > 80
            || self.display_name.chars().any(char::is_control)
            || self.launcher_kind != "standalone"
        {
            return Err(invalid(
                "Enter a name of 1–80 characters without controls; only standalone is supported",
            ));
        }
        self.executable_ref.validate()?;
        if let Some(p) = &self.working_directory_ref {
            p.validate()?;
        }
        // UTF-16 units reflect Windows command-line limits without joining/quoting.
        if self.arguments.len() > 128
            || self
                .arguments
                .iter()
                .any(|a| a.contains('\0') || a.encode_utf16().count() > 4096)
            || self
                .arguments
                .iter()
                .map(|a| a.encode_utf16().count())
                .sum::<usize>()
                > 16384
        {
            return Err(invalid(
                "Arguments allow 128 literal strings, 4096 UTF-16 units each and 16384 total; NUL is unsupported",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub game_id: String,
    pub installation_id: String,
    pub provenance: String,
    pub revision: i64,
    pub configuration: Configuration,
}
#[derive(Debug, Deserialize)]
#[serde(
    tag = "operation",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Action {
    Create {
        installation_id: String,
        configuration: Configuration,
    },
    Save {
        profile_id: String,
        expected_profile_revision: i64,
        configuration: Configuration,
    },
    Duplicate {
        profile_id: String,
        expected_profile_revision: i64,
    },
    Remove {
        profile_id: String,
        expected_profile_revision: i64,
    },
    SetDefault {
        profile_id: Option<String>,
        expected_profile_revision: Option<i64>,
    },
}
#[derive(FromRow)]
struct Row {
    id: String,
    game_id: String,
    installation_id: String,
    provenance: String,
    display_name: String,
    launcher_kind: String,
    executable_kind: String,
    executable_value: String,
    arguments_json: String,
    working_directory_kind: Option<String>,
    working_directory_value: Option<String>,
    revision: i64,
}
impl Row {
    fn decode(self) -> Result<Profile> {
        let working_directory_ref =
            match (self.working_directory_kind, self.working_directory_value) {
                (None, None) => None,
                (Some(k), Some(v)) => Some(PathRef::from_parts(&k, v)?),
                _ => return Err(invalid("Invalid working-directory pair")),
            };
        let configuration = Configuration {
            display_name: self.display_name,
            launcher_kind: self.launcher_kind,
            executable_ref: PathRef::from_parts(&self.executable_kind, self.executable_value)?,
            arguments: serde_json::from_str(&self.arguments_json)
                .map_err(|_| invalid("Invalid literal argument array"))?,
            working_directory_ref,
        };
        configuration.validate()?;
        if self.provenance != "user" || self.revision < 0 {
            return Err(invalid("Invalid provenance or revision"));
        }
        Ok(Profile {
            id: self.id,
            game_id: self.game_id,
            installation_id: self.installation_id,
            provenance: self.provenance,
            revision: self.revision,
            configuration,
        })
    }
}
pub(super) async fn list(c: &mut SqliteConnection, game: &str) -> Result<Vec<Profile>> {
    let rows: Vec<Row> = sqlx::query_as("SELECT p.*,e.game_id FROM launch_profiles p JOIN game_installations i ON i.id=p.installation_id JOIN library_entries e ON e.id=i.entry_id WHERE e.game_id=? ORDER BY p.id LIMIT 101")
    .bind(game)
    .fetch_all(c)
    .await?;
    if rows.len() > MAX_PROFILES {
        return Err(invalid(
            "Editor supports at most 100 LaunchProfiles per game",
        ));
    }
    rows.into_iter().map(Row::decode).collect()
}
async fn profile(c: &mut SqliteConnection, game: &str, id: &str, revision: i64) -> Result<Profile> {
    valid_id(id)?;
    if revision < 0 {
        return Err(invalid("Profile revision must be nonnegative"));
    }
    let row: Row = sqlx::query_as("SELECT p.*,e.game_id FROM launch_profiles p JOIN game_installations i ON i.id=p.installation_id JOIN library_entries e ON e.id=i.entry_id WHERE p.id=? AND e.game_id=?")
        .bind(id)
        .bind(game)
        .fetch_optional(c)
        .await?
        .ok_or_else(|| {
            StorageError::new(
                ErrorCode::NotFound,
                "launch_profiles",
                "Profile no longer belongs to this game. Reopen the editor.",
            )
        })?;
    let p = row.decode()?;
    if p.revision != revision {
        return Err(conflict(
            "Profile changed. Copy your draft, then reopen the editor before saving.",
        ));
    }
    Ok(p)
}
async fn write(
    c: &mut SqliteConnection,
    id: &str,
    installation: &str,
    config: &Configuration,
    create: bool,
) -> Result<()> {
    config.validate()?;
    let (ek, ev) = config.executable_ref.parts();
    let (wk, wv) = config
        .working_directory_ref
        .as_ref()
        .map(|p| {
            let (k, v) = p.parts();
            (Some(k), Some(v))
        })
        .unwrap_or((None, None));
    let arguments =
        serde_json::to_string(&config.arguments).map_err(|_| invalid("Invalid arguments"))?;
    let time = manual::now(c).await?;
    if create {
        sqlx::query("INSERT INTO launch_profiles(id,installation_id,provenance,display_name,launcher_kind,executable_kind,executable_value,arguments_json,working_directory_kind,working_directory_value,created_at_ms,updated_at_ms) VALUES (?,?,'user',?,?,?,?,?,?,?,?,?)")
            .bind(id).bind(installation).bind(&config.display_name).bind(&config.launcher_kind).bind(ek).bind(ev).bind(arguments).bind(wk).bind(wv).bind(time).bind(time).execute(c).await?;
    } else {
        sqlx::query("UPDATE launch_profiles SET display_name=?,launcher_kind=?,executable_kind=?,executable_value=?,arguments_json=?,working_directory_kind=?,working_directory_value=?,revision=revision+1,updated_at_ms=? WHERE id=?")
            .bind(&config.display_name).bind(&config.launcher_kind).bind(ek).bind(ev).bind(arguments).bind(wk).bind(wv).bind(time).bind(id).execute(c).await?;
    }
    Ok(())
}
pub(super) async fn mutate(
    db: &Database,
    game: &str,
    expected_game_revision: i64,
    action: &Action,
) -> Result<()> {
    valid_id(game)?;
    manual::application(&mut *db.pool.acquire().await?).await?;
    let mut tx = db.pool.begin_with("BEGIN IMMEDIATE").await?;
    let revision: i64 = sqlx::query_scalar("SELECT revision FROM games WHERE id=?")
        .bind(game)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| invalid("Game no longer exists"))?;
    let mut advance_game = false;
    match action {
        Action::Create {
            installation_id,
            configuration,
        } => {
            valid_id(installation_id)?;
            let own: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_installations i JOIN library_entries e ON e.id=i.entry_id WHERE i.id=? AND e.game_id=?)").bind(installation_id).bind(game).fetch_one(&mut *tx).await?;
            if !own {
                return Err(invalid("Choose an installation belonging to this game"));
            }
            let id = manual::id(&mut tx).await?;
            write(&mut tx, &id, installation_id, configuration, true).await?;
        }
        Action::Save {
            profile_id,
            expected_profile_revision,
            configuration,
        } => {
            let p = profile(&mut tx, game, profile_id, *expected_profile_revision).await?;
            write(&mut tx, &p.id, &p.installation_id, configuration, false).await?;
        }
        Action::Duplicate {
            profile_id,
            expected_profile_revision,
        } => {
            let p = profile(&mut tx, game, profile_id, *expected_profile_revision).await?;
            let id = manual::id(&mut tx).await?;
            write(&mut tx, &id, &p.installation_id, &p.configuration, true).await?;
        }
        Action::Remove {
            profile_id,
            expected_profile_revision,
        } => {
            profile(&mut tx, game, profile_id, *expected_profile_revision).await?;
            let removed =
                sqlx::query("DELETE FROM game_launch_defaults WHERE game_id=? AND profile_id=?")
                    .bind(game)
                    .bind(profile_id)
                    .execute(&mut *tx)
                    .await?;
            advance_game = removed.rows_affected() != 0;
            sqlx::query("DELETE FROM launch_profiles WHERE id=?")
                .bind(profile_id)
                .execute(&mut *tx)
                .await?;
        }
        Action::SetDefault {
            profile_id,
            expected_profile_revision,
        } => {
            if revision != expected_game_revision || expected_game_revision < 0 {
                return Err(conflict(
                    "Game changed. Reopen the editor before changing its default.",
                ));
            }
            match (profile_id, expected_profile_revision) {
                (Some(id), Some(r)) => {
                    profile(&mut tx, game, id, *r).await?;
                    sqlx::query("INSERT INTO game_launch_defaults(game_id,profile_id) VALUES (?,?) ON CONFLICT(game_id) DO UPDATE SET profile_id=excluded.profile_id").bind(game).bind(id).execute(&mut *tx).await?;
                }
                (None, None) => {
                    sqlx::query("DELETE FROM game_launch_defaults WHERE game_id=?")
                        .bind(game)
                        .execute(&mut *tx)
                        .await?;
                }
                _ => {
                    return Err(invalid(
                        "Default selection requires the selected profile revision",
                    ));
                }
            }
            advance_game = true;
        }
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM launch_profiles p JOIN game_installations i ON i.id=p.installation_id JOIN library_entries e ON e.id=i.entry_id WHERE e.game_id=?").bind(game).fetch_one(&mut *tx).await?;
    if count > MAX_PROFILES as i64 {
        return Err(invalid("At most 100 LaunchProfiles per game"));
    }
    if advance_game {
        let time = manual::now(&mut tx).await?;
        sqlx::query("UPDATE games SET revision=revision+1,updated_at_ms=? WHERE id=?")
            .bind(time)
            .bind(game)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Bounded pages, no library-wide configuration allocation. SQL first bounds text
/// before decoding; this also rejects modified databases bypassing CHECK rules.
pub(super) async fn validate_all(c: &mut SqliteConnection) -> Result<()> {
    let bad: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM launch_profiles WHERE length(display_name)>80 OR length(executable_value)>2048 OR length(arguments_json)>131072 OR length(working_directory_value)>2048 OR revision<0 OR provenance<>'user' OR length(id)<>32 OR id GLOB '*[^0-9a-f]*') OR EXISTS(SELECT 1 FROM game_launch_defaults d JOIN launch_profiles p ON p.id=d.profile_id JOIN game_installations i ON i.id=p.installation_id JOIN library_entries e ON e.id=i.entry_id WHERE e.game_id<>d.game_id)").fetch_one(&mut *c).await?;
    if bad {
        return Err(invalid("Invalid profile or default configuration"));
    }
    let mut after = String::new();
    loop {
        let rows: Vec<Row> =
            sqlx::query_as("SELECT p.*,e.game_id FROM launch_profiles p JOIN game_installations i ON i.id=p.installation_id JOIN library_entries e ON e.id=i.entry_id WHERE p.id>? ORDER BY p.id LIMIT 100")
                .bind(&after)
                .fetch_all(&mut *c)
                .await?;
        if rows.is_empty() {
            break;
        }
        for row in rows {
            after.clone_from(&row.id);
            row.decode()?;
        }
    }
    Ok(())
}
