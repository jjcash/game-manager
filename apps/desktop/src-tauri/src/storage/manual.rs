// SPDX-License-Identifier: GPL-3.0-or-later
//! Manual writes only. Effective `games` values mirror explicit overrides atomically.
//! Future provider writers must resolve overrides before updating that projection.
use super::{Database, ErrorCode, Result, StorageError};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqliteConnection};

const MAX_RELATED: i64 = 100;
fn invalid(message: &str) -> StorageError {
    StorageError::new(ErrorCode::InvalidRequest, "manual_library", message)
}
fn missing() -> StorageError {
    StorageError::new(
        ErrorCode::NotFound,
        "manual_library",
        "Record no longer exists. Close the editor and refresh the library.",
    )
}
fn conflict(message: &str) -> StorageError {
    StorageError::new(ErrorCode::Conflict, "manual_library", message)
}
fn text(value: &str, max: usize, required: bool) -> Result<String> {
    if value.chars().any(char::is_control) {
        return Err(invalid("Text must not contain control characters"));
    }
    let value = value.trim();
    if (required && value.is_empty())
        || value.chars().count() > max
        || value.chars().any(char::is_control)
    {
        return Err(invalid(
            "Text is blank, too long, or contains control characters",
        ));
    }
    Ok(value.into())
}
fn optional(value: &Option<String>) -> Result<Option<String>> {
    Ok(value
        .as_ref()
        .map(|v| text(v, 200, false))
        .transpose()?
        .filter(|v| !v.is_empty()))
}
pub fn valid_date(value: &str) -> bool {
    if value.len() != 10 || !value.is_ascii() || &value[4..5] != "-" || &value[7..8] != "-" {
        return false;
    }
    if !value.bytes().enumerate().all(|(i, b)| {
        if i == 4 || i == 7 {
            b == b'-'
        } else {
            b.is_ascii_digit()
        }
    }) {
        return false;
    }
    let (Ok(y), Ok(m), Ok(d)) = (
        value[..4].parse::<u32>(),
        value[5..7].parse::<u32>(),
        value[8..].parse::<u32>(),
    ) else {
        return false;
    };
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if y.is_multiple_of(400) || (y.is_multiple_of(4) && !y.is_multiple_of(100)) => 29,
        2 => 28,
        _ => 0,
    };
    y > 0 && d > 0 && d <= days
}
#[derive(Debug, Clone, Deserialize, Serialize, FromRow)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Metadata {
    pub title: String,
    pub release_date: Option<String>,
    pub genre: Option<String>,
    pub developer: Option<String>,
    pub favorite: bool,
}
impl Metadata {
    fn validate(&self) -> Result<Self> {
        let release_date = optional(&self.release_date)?;
        if release_date.as_ref().is_some_and(|d| !valid_date(d)) {
            return Err(invalid("Release date must be a valid YYYY-MM-DD date"));
        }
        Ok(Self {
            title: text(&self.title, 200, true)?,
            release_date,
            genre: optional(&self.genre)?,
            developer: optional(&self.developer)?,
            favorite: self.favorite,
        })
    }
}
#[derive(Debug, Clone, Deserialize, Serialize, FromRow)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstallationInput {
    pub root_path_ref: String,
    pub platform: String,
    pub version: Option<String>,
    pub detection_state: String,
}
impl InstallationInput {
    fn validate(&self) -> Result<Self> {
        if !matches!(
            self.detection_state.as_str(),
            "present" | "missing" | "unknown"
        ) {
            return Err(invalid("Invalid installation presence"));
        }
        Ok(Self {
            root_path_ref: text(&self.root_path_ref, 2048, true)?,
            platform: text(&self.platform, 80, true)?,
            version: optional(&self.version)?,
            detection_state: self.detection_state.clone(),
        })
    }
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateRequest {
    pub metadata: Metadata,
    pub source_name: String,
    pub installation: Option<InstallationInput>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MutationRequest {
    pub game_id: String,
    pub expected_revision: i64,
    pub action: Action,
}
#[derive(Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Action {
    Metadata {
        metadata: Metadata,
    },
    AddEntry {
        source_name: String,
    },
    UpdateEntry {
        entry_id: String,
        completion_status: String,
    },
    RemoveEntry {
        entry_id: String,
    },
    SaveInstallation {
        entry_id: String,
        installation_id: Option<String>,
        installation: InstallationInput,
    },
    RemoveInstallation {
        installation_id: String,
    },
    RemoveGame,
}
#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: String,
    pub source_id: String,
    pub source_name: String,
    pub source_type: String,
    pub completion_status: String,
}
#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Installation {
    pub id: String,
    pub entry_id: String,
    pub root_path_ref: String,
    pub platform: String,
    pub version: Option<String>,
    pub detection_state: String,
    pub state_origin: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Details {
    pub id: String,
    pub revision: i64,
    pub origin: String,
    pub metadata: Metadata,
    pub overridden_fields: Vec<String>,
    pub entries: Vec<Entry>,
    pub installations: Vec<Installation>,
}
async fn id(c: &mut SqliteConnection) -> Result<String> {
    Ok(sqlx::query_scalar("SELECT lower(hex(randomblob(16)))")
        .fetch_one(c)
        .await?)
}
async fn now(c: &mut SqliteConnection) -> Result<i64> {
    Ok(
        sqlx::query_scalar("SELECT CAST(unixepoch('subsec') * 1000 AS INTEGER)")
            .fetch_one(c)
            .await?,
    )
}
async fn application(c: &mut SqliteConnection) -> Result<()> {
    let purpose: String =
        sqlx::query_scalar("SELECT purpose FROM database_metadata WHERE singleton=1")
            .fetch_one(c)
            .await?;
    if purpose != "application" {
        return Err(StorageError::new(
            ErrorCode::WrongPurpose,
            "manual_library",
            "Manual editing requires an application database",
        ));
    }
    Ok(())
}
async fn bounded(c: &mut SqliteConnection, game: &str) -> Result<()> {
    let (entries, installs): (i64,i64) = sqlx::query_as("SELECT (SELECT count(*) FROM library_entries WHERE game_id=?), (SELECT count(*) FROM game_installations i JOIN library_entries e ON e.id=i.entry_id WHERE e.game_id=?)").bind(game).bind(game).fetch_one(c).await?;
    if entries > MAX_RELATED || installs > MAX_RELATED {
        return Err(invalid(
            "Manual editor supports at most 100 memberships and 100 installation records per game",
        ));
    }
    Ok(())
}
async fn entry(c: &mut SqliteConnection, game: &str, source: &str) -> Result<()> {
    let name = text(source, 80, true)?;
    let existing: Option<String> = sqlx::query_scalar("SELECT id FROM library_sources WHERE source_type='manual' AND display_name=? ORDER BY id LIMIT 1").bind(&name).fetch_optional(&mut *c).await?;
    let source_id = if let Some(s) = existing {
        s
    } else {
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM library_sources")
            .fetch_one(&mut *c)
            .await?;
        if count >= 100 {
            return Err(invalid("Source limit reached (100)"));
        }
        let s = id(c).await?;
        sqlx::query(
            "INSERT INTO library_sources(id,source_type,display_name) VALUES (?,'manual',?)",
        )
        .bind(&s)
        .bind(name)
        .execute(&mut *c)
        .await?;
        s
    };
    let eid = id(c).await?;
    let time = now(c).await?;
    sqlx::query("INSERT INTO library_entries(id,game_id,source_id,added_at_ms) VALUES (?,?,?,?)")
        .bind(eid)
        .bind(game)
        .bind(source_id)
        .bind(time)
        .execute(c)
        .await?;
    Ok(())
}
async fn own_entry(c: &mut SqliteConnection, game: &str, eid: &str) -> Result<()> {
    text(eid, 128, true)?;
    let source: Option<(String,Option<String>)> = sqlx::query_as("SELECT s.source_type,e.external_item_id FROM library_entries e JOIN library_sources s ON s.id=e.source_id WHERE e.id=? AND e.game_id=?").bind(eid).bind(game).fetch_optional(c).await?;
    match source {
        None => Err(missing()),
        Some((s, external)) if s == "manual" && external.is_none() => Ok(()),
        _ => Err(conflict(
            "Only manual memberships without provider identity can be changed here",
        )),
    }
}
async fn metadata(c: &mut SqliteConnection, game: &str, value: &Metadata) -> Result<()> {
    let v = value.validate()?;
    sqlx::query("UPDATE games SET title=?,sort_title=?,release_date=?,genre=?,developer=?,favorite=? WHERE id=?").bind(&v.title).bind(v.title.to_lowercase()).bind(&v.release_date).bind(&v.genre).bind(&v.developer).bind(v.favorite).bind(game).execute(&mut *c).await?;
    for (field, value) in [
        ("title", Some(v.title)),
        ("release_date", v.release_date),
        ("genre", v.genre),
        ("developer", v.developer),
        ("favorite", Some(if v.favorite { "1" } else { "0" }.into())),
    ] {
        sqlx::query("INSERT INTO game_field_overrides(game_id,field,value) VALUES (?,?,?) ON CONFLICT(game_id,field) DO UPDATE SET value=excluded.value").bind(game).bind(field).bind(value).execute(&mut *c).await?;
    }
    Ok(())
}
async fn installation(
    c: &mut SqliteConnection,
    game: &str,
    eid: &str,
    iid: Option<&str>,
    value: &InstallationInput,
) -> Result<()> {
    if let Some(iid) = iid {
        text(iid, 128, true)?;
    }
    own_entry(c, game, eid).await?;
    let v = value.validate()?;
    let duplicate: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM game_installations WHERE entry_id=? AND root_path_ref=? AND id<>?",
    )
    .bind(eid)
    .bind(&v.root_path_ref)
    .bind(iid.unwrap_or(""))
    .fetch_one(&mut *c)
    .await?;
    if duplicate != 0 {
        return Err(conflict(
            "This membership already has an installation record with that location",
        ));
    }
    if let Some(iid) = iid {
        let result = sqlx::query("UPDATE game_installations SET root_path_ref=?,platform=?,version=?,detection_state=?,state_origin='user' WHERE id=? AND entry_id=?").bind(v.root_path_ref).bind(v.platform).bind(v.version).bind(v.detection_state).bind(iid).bind(eid).execute(c).await?;
        if result.rows_affected() == 0 {
            return Err(missing());
        }
    } else {
        let iid = id(c).await?;
        sqlx::query("INSERT INTO game_installations(id,entry_id,root_path_ref,platform,version,detection_state,state_origin) VALUES (?,?,?,?,?,?,'user')").bind(iid).bind(eid).bind(v.root_path_ref).bind(v.platform).bind(v.version).bind(v.detection_state).execute(c).await?;
    }
    Ok(())
}
impl Database {
    pub async fn manual_details(&self, game: &str) -> Result<Details> {
        text(game, 128, true)?;
        let mut tx = self.pool.begin().await?;
        application(&mut tx).await?;
        let (revision, origin): (i64, String) =
            sqlx::query_as("SELECT revision,origin FROM games WHERE id=?")
                .bind(game)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(missing)?;
        bounded(&mut tx, game).await?;
        let metadata = sqlx::query_as(
            "SELECT title,release_date,genre,developer,favorite FROM games WHERE id=?",
        )
        .bind(game)
        .fetch_one(&mut *tx)
        .await?;
        let overridden_fields = sqlx::query_scalar(
            "SELECT field FROM game_field_overrides WHERE game_id=? ORDER BY field LIMIT 5",
        )
        .bind(game)
        .fetch_all(&mut *tx)
        .await?;
        let entries = sqlx::query_as("SELECT e.id,e.source_id,s.display_name AS source_name,s.source_type,e.completion_status FROM library_entries e JOIN library_sources s ON s.id=e.source_id WHERE game_id=? ORDER BY e.added_at_ms,e.id LIMIT 100").bind(game).fetch_all(&mut *tx).await?;
        let installations = sqlx::query_as("SELECT i.* FROM game_installations i JOIN library_entries e ON e.id=i.entry_id WHERE e.game_id=? ORDER BY i.id LIMIT 100").bind(game).fetch_all(&mut *tx).await?;
        tx.commit().await?;
        Ok(Details {
            id: game.into(),
            revision,
            origin,
            metadata,
            overridden_fields,
            entries,
            installations,
        })
    }
    pub async fn manual_create(&self, request: &CreateRequest) -> Result<String> {
        // Refuse a read-only benchmark before attempting to reserve a writer.
        application(&mut *self.pool.acquire().await?).await?;
        let value = request.metadata.validate()?;
        text(&request.source_name, 80, true)?;
        if let Some(i) = &request.installation {
            i.validate()?;
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        application(&mut tx).await?;
        let game = id(&mut tx).await?;
        let time = now(&mut tx).await?;
        sqlx::query("INSERT INTO games(id,title,sort_title,created_at_ms,updated_at_ms,origin) VALUES (?,?,?,?,?,'manual')").bind(&game).bind(&value.title).bind(value.title.to_lowercase()).bind(time).bind(time).execute(&mut *tx).await?;
        metadata(&mut tx, &game, &value).await?;
        entry(&mut tx, &game, &request.source_name).await?;
        if let Some(i) = &request.installation {
            let eid: String = sqlx::query_scalar("SELECT id FROM library_entries WHERE game_id=?")
                .bind(&game)
                .fetch_one(&mut *tx)
                .await?;
            installation(&mut tx, &game, &eid, None, i).await?;
        }
        tx.commit().await?;
        Ok(game)
    }
    pub async fn manual_mutate(&self, request: &MutationRequest) -> Result<()> {
        application(&mut *self.pool.acquire().await?).await?;
        let game = &request.game_id;
        text(game, 128, true)?;
        if request.expected_revision < 0 {
            return Err(invalid("Revision must be nonnegative"));
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        application(&mut tx).await?;
        let (revision, origin): (i64, String) =
            sqlx::query_as("SELECT revision,origin FROM games WHERE id=?")
                .bind(game)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(missing)?;
        if revision != request.expected_revision {
            return Err(conflict(
                "Game changed since this editor opened. Copy any unsaved changes, then close and reopen the editor before saving.",
            ));
        }
        bounded(&mut tx, game).await?;
        match &request.action {
            Action::Metadata { metadata: value } => metadata(&mut tx, game, value).await?,
            Action::AddEntry { source_name } => entry(&mut tx, game, source_name).await?,
            Action::UpdateEntry {
                entry_id,
                completion_status,
            } => {
                own_entry(&mut tx, game, entry_id).await?;
                if !matches!(
                    completion_status.as_str(),
                    "not_started" | "playing" | "completed" | "abandoned"
                ) {
                    return Err(invalid("Invalid completion status"));
                }
                sqlx::query("UPDATE library_entries SET completion_status=? WHERE id=?")
                    .bind(completion_status)
                    .bind(entry_id)
                    .execute(&mut *tx)
                    .await?;
            }
            Action::RemoveEntry { entry_id } => {
                own_entry(&mut tx, game, entry_id).await?;
                let count: i64 =
                    sqlx::query_scalar("SELECT count(*) FROM library_entries WHERE game_id=?")
                        .bind(game)
                        .fetch_one(&mut *tx)
                        .await?;
                if count <= 1 {
                    return Err(conflict(
                        "Keep the last membership, or explicitly remove the manual game",
                    ));
                }
                sqlx::query("DELETE FROM game_installations WHERE entry_id=?")
                    .bind(entry_id)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("DELETE FROM library_entries WHERE id=?")
                    .bind(entry_id)
                    .execute(&mut *tx)
                    .await?;
            }
            Action::SaveInstallation {
                entry_id,
                installation_id,
                installation: value,
            } => installation(&mut tx, game, entry_id, installation_id.as_deref(), value).await?,
            Action::RemoveInstallation { installation_id } => {
                text(installation_id, 128, true)?;
                let eid: String = sqlx::query_scalar("SELECT i.entry_id FROM game_installations i JOIN library_entries e ON e.id=i.entry_id WHERE i.id=? AND e.game_id=?").bind(installation_id).bind(game).fetch_optional(&mut *tx).await?.ok_or_else(missing)?;
                own_entry(&mut tx, game, &eid).await?;
                sqlx::query("DELETE FROM game_installations WHERE id=?")
                    .bind(installation_id)
                    .execute(&mut *tx)
                    .await?;
            }
            Action::RemoveGame => {
                let protected: i64 = sqlx::query_scalar("SELECT count(*) FROM library_entries e JOIN library_sources s ON s.id=e.source_id WHERE e.game_id=? AND (s.source_type<>'manual' OR e.external_item_id IS NOT NULL)").bind(game).fetch_one(&mut *tx).await?;
                if origin != "manual" || protected != 0 {
                    return Err(conflict(
                        "Only manually created games with exclusively manual memberships can be removed here",
                    ));
                }
                sqlx::query("DELETE FROM game_installations WHERE entry_id IN (SELECT id FROM library_entries WHERE game_id=?)").bind(game).execute(&mut *tx).await?;
                sqlx::query("DELETE FROM library_entries WHERE game_id=?")
                    .bind(game)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("DELETE FROM game_field_overrides WHERE game_id=?")
                    .bind(game)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("DELETE FROM games WHERE id=?")
                    .bind(game)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        bounded(&mut tx, game).await?;
        let time = now(&mut tx).await?;
        sqlx::query("UPDATE games SET revision=revision+1,updated_at_ms=? WHERE id=?")
            .bind(time)
            .bind(game)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}
