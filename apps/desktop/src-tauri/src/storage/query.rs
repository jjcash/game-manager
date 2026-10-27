// SPDX-License-Identifier: GPL-3.0-or-later
use super::{
    Database, ErrorCode, GameSummary, LibraryRow, LibraryWindow, MAX_PAGE_SIZE, Result,
    StorageError, SummarySort,
};
use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Sqlite};
use std::time::Instant;

#[cfg(feature = "benchmarks")]
mod evaluation;

#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallationFilter {
    #[default]
    All,
    Installed,
    NotInstalled,
}
#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    #[default]
    Asc,
    Desc,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Criteria {
    pub search: String,
    pub installed: InstallationFilter,
    pub source_id: Option<String>,
    pub year_from: Option<u16>,
    pub year_to: Option<u16>,
    pub sort: SummarySort,
    pub direction: Direction,
}
impl Default for Criteria {
    fn default() -> Self {
        Self {
            search: String::new(),
            installed: InstallationFilter::All,
            source_id: None,
            year_from: None,
            year_to: None,
            sort: SummarySort::Title,
            direction: Direction::Asc,
        }
    }
}
fn invalid(message: &str) -> StorageError {
    StorageError::new(ErrorCode::InvalidRequest, "library_query", message)
}
impl Criteria {
    pub fn validate(&self) -> Result<()> {
        if self.search.chars().count() > 200 || self.search.contains('\0') {
            return Err(invalid(
                "Search must contain at most 200 characters and no NUL",
            ));
        }
        if self
            .source_id
            .as_ref()
            .is_some_and(|s| s.is_empty() || s.len() > 128 || s.chars().any(char::is_control))
        {
            return Err(invalid("Invalid source identity"));
        }
        if [self.year_from, self.year_to]
            .into_iter()
            .flatten()
            .any(|year| !(1..=9999).contains(&year))
            || matches!((self.year_from,self.year_to),(Some(a),Some(b)) if a>b)
        {
            return Err(invalid("Years must be 1..9999, with from <= to"));
        }
        Ok(())
    }
    fn unfiltered(&self) -> bool {
        self.search.trim().is_empty()
            && matches!(self.installed, InstallationFilter::All)
            && self.source_id.is_none()
            && self.year_from.is_none()
            && self.year_to.is_none()
    }
    fn order(&self) -> &'static str {
        match (self.sort, self.direction) {
            (SummarySort::Title, Direction::Asc) => "g.sort_title ASC, g.id ASC",
            (SummarySort::Title, Direction::Desc) => "g.sort_title DESC, g.id DESC",
            (SummarySort::ReleaseDate, Direction::Asc) => "g.release_date ASC, g.id ASC",
            (SummarySort::ReleaseDate, Direction::Desc) => "g.release_date DESC, g.id DESC",
        }
    }
    fn predicate(&self, builder: &mut QueryBuilder<Sqlite>) {
        let search = self.search.trim();
        if !search.is_empty() {
            // Literal substring: %, _ and backslash are data, never patterns.
            let pattern = format!(
                "%{}%",
                search
                    .replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            );
            builder
                .push(" AND g.title LIKE ")
                .push_bind(pattern)
                .push(" ESCAPE '\\'");
        }
        // Build present entry IDs once using the existing covering copy index,
        // rather than probing it for every library entry. Both IDs are NOT NULL.
        // IS TRUE keeps membership a test: SQLite can scan the canonical sort
        // index instead of driving the page by game IDs and sorting all matches.
        // See M0.7B's same-engine plans/measurements before changing this shape.
        let present = "(g.id IN (SELECT e.game_id FROM library_entries e WHERE e.id IN (SELECT i.entry_id FROM game_installations i WHERE i.detection_state='present'))) IS TRUE";
        match self.installed {
            InstallationFilter::All => {}
            InstallationFilter::Installed => {
                builder.push(" AND ").push(present);
            }
            InstallationFilter::NotInstalled => {
                builder.push(" AND NOT (").push(present).push(")");
            }
        }
        if let Some(source) = &self.source_id {
            builder
                .push(" AND g.id IN (SELECT e.game_id FROM library_entries e WHERE e.source_id=")
                .push_bind(source.clone())
                .push(")");
        }
        if let Some(year) = self.year_from {
            builder
                .push(" AND g.release_date>=")
                .push_bind(format!("{year:04}-01-01"));
        }
        if let Some(year) = self.year_to {
            builder
                .push(" AND g.release_date<=")
                .push_bind(format!("{year:04}-12-31"));
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryRequest {
    pub limit: u32,
    pub offset: u32,
    pub criteria: Criteria,
}
impl QueryRequest {
    pub fn validate(&self) -> Result<()> {
        self.criteria.validate()?;
        if self.limit == 0 || self.limit > MAX_PAGE_SIZE || self.offset > 1_000_000 {
            return Err(invalid("Limit must be 1..=200 and offset <= 1000000"));
        }
        Ok(())
    }
    fn builder(&self, explain: bool) -> QueryBuilder<Sqlite> {
        let mut query = QueryBuilder::new(if explain {
            "EXPLAIN QUERY PLAN WITH page AS MATERIALIZED (SELECT g.id FROM games g WHERE 1=1"
        } else {
            "WITH page AS MATERIALIZED (SELECT g.id FROM games g WHERE 1=1"
        });
        self.criteria.predicate(&mut query);
        query
            .push(" ORDER BY ")
            .push(self.criteria.order())
            .push(" LIMIT ")
            .push_bind(self.limit)
            .push(" OFFSET ")
            .push_bind(self.offset)
            .push(") ");
        query.push(super::summary_projection(self.criteria.order()));
        query
    }
}
#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SourceOption {
    pub id: String,
    pub name: String,
}
impl Database {
    pub async fn query_count(&self, criteria: &Criteria) -> Result<i64> {
        Ok(self.query_count_timed(criteria).await?.0)
    }
    pub async fn query_count_timed(&self, criteria: &Criteria) -> Result<(i64, f64)> {
        criteria.validate()?;
        let waiting = Instant::now();
        let mut connection = self.pool.acquire().await?;
        let pool_wait_ms = waiting.elapsed().as_secs_f64() * 1000.0;
        if criteria.unfiltered() {
            return Ok((
                sqlx::query_scalar("SELECT count(*) FROM games")
                    .fetch_one(&mut *connection)
                    .await?,
                pool_wait_ms,
            ));
        }
        let mut query = QueryBuilder::new("SELECT count(*) FROM games g WHERE 1=1");
        criteria.predicate(&mut query);
        Ok((
            query
                .build_query_scalar()
                .fetch_one(&mut *connection)
                .await?,
            pool_wait_ms,
        ))
    }
    pub async fn source_options(&self) -> Result<Vec<SourceOption>> {
        Ok(sqlx::query_as("SELECT id,display_name AS name FROM library_sources ORDER BY display_name,id LIMIT 100").fetch_all(&self.pool).await?)
    }
    pub async fn query_window(&self, request: &QueryRequest) -> Result<LibraryWindow> {
        request.validate()?;
        let start = Instant::now();
        let mut connection = self.pool.acquire().await?;
        let pool_wait_ms = start.elapsed().as_secs_f64() * 1000.0;
        let items: Vec<GameSummary> = request
            .builder(false)
            .build_query_as()
            .fetch_all(&mut *connection)
            .await?;
        Ok(LibraryWindow {
            offset: request.offset,
            items: items.into_iter().map(LibraryRow::from).collect(),
            query_ms: start.elapsed().as_secs_f64() * 1000.0,
            pool_wait_ms: Some(pool_wait_ms),
        })
    }
    #[cfg(feature = "benchmarks")]
    pub async fn criteria_plans(&self, request: &QueryRequest) -> Result<Vec<String>> {
        use sqlx::Row;
        request.validate()?;
        let mut plans = Vec::new();
        let mut count = if request.criteria.unfiltered() {
            QueryBuilder::new("EXPLAIN QUERY PLAN SELECT count(*) FROM games")
        } else {
            QueryBuilder::new("EXPLAIN QUERY PLAN SELECT count(*) FROM games g WHERE 1=1")
        };
        if !request.criteria.unfiltered() {
            request.criteria.predicate(&mut count);
        }
        plans.extend(
            count
                .build()
                .fetch_all(&self.pool)
                .await?
                .iter()
                .map(|r| r.get::<String, _>(3)),
        );
        plans.extend(
            request
                .builder(true)
                .build()
                .fetch_all(&self.pool)
                .await?
                .iter()
                .map(|r| r.get::<String, _>(3)),
        );
        Ok(plans)
    }
}
