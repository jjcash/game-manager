// SPDX-License-Identifier: GPL-3.0-or-later
//! Read-only, same-engine query experiments. Never exposed through IPC.
use super::*;
use serde_json::{Value, json};
use sqlx::{Execute, Row, sqlite::SqliteRow};

const ORIGINAL: &str = "g.id IN (SELECT e.game_id FROM library_entries e WHERE EXISTS (SELECT 1 FROM game_installations i WHERE i.entry_id=e.id AND i.detection_state='present'))";
const JOIN: &str = "g.id IN (SELECT e.game_id FROM game_installations i JOIN library_entries e ON e.id=i.entry_id WHERE i.detection_state='present')";
const EXISTS: &str = "EXISTS (SELECT 1 FROM library_entries e JOIN game_installations i ON i.entry_id=e.id WHERE e.game_id=g.id AND i.detection_state='present')";
const ENTRY_SET: &str = "g.id IN (SELECT e.game_id FROM library_entries e WHERE e.id IN (SELECT i.entry_id FROM game_installations i WHERE i.detection_state='present'))";

#[derive(Clone, Copy, Debug)]
enum Strategy {
    Baseline,
    InstallationJoin,
    Correlated,
    OrderedMembership,
    OrderedJoin,
    EntrySet,
    OrderedEntrySet,
    // Experiments only: keep M0.7B counts and vary bounded page selection.
    PageMembership,
    PageCorrelated,
}
impl Strategy {
    fn sql(self, sql: &str) -> String {
        // Both historical and selected production shapes can be diagnosed.
        let canonical = sql
            .replace(&format!("({ENTRY_SET}) IS TRUE"), ORIGINAL)
            .replace(ENTRY_SET, ORIGINAL)
            .replace(&format!("({JOIN}) IS TRUE"), ORIGINAL)
            .replace(JOIN, ORIGINAL)
            .replace(&format!("({ORIGINAL}) IS TRUE"), ORIGINAL);
        match self {
            Self::Baseline => canonical,
            Self::InstallationJoin => canonical.replace(ORIGINAL, JOIN),
            Self::Correlated => canonical.replace(ORIGINAL, EXISTS),
            Self::OrderedMembership => {
                canonical.replace(ORIGINAL, &format!("({ORIGINAL}) IS TRUE"))
            }
            Self::OrderedJoin => canonical.replace(ORIGINAL, &format!("({JOIN}) IS TRUE")),
            Self::EntrySet => canonical.replace(ORIGINAL, ENTRY_SET),
            Self::OrderedEntrySet => canonical.replace(ORIGINAL, &format!("({ENTRY_SET}) IS TRUE")),
            Self::PageMembership | Self::PageCorrelated => {
                let predicate = if sql.starts_with("SELECT count(*)") {
                    format!("({ENTRY_SET}) IS TRUE")
                } else if matches!(self, Self::PageMembership) {
                    format!("({ORIGINAL}) IS TRUE")
                } else {
                    EXISTS.to_string()
                };
                canonical.replace(ORIGINAL, &predicate)
            }
        }
    }
}

fn distribution(samples: &[f64]) -> Value {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    json!({"p50": sorted[(sorted.len()*50).div_ceil(100)-1],
        "p95": sorted[(sorted.len()*95).div_ceil(100)-1], "max": sorted[sorted.len()-1],
        "samples": samples})
}

async fn execute(
    connection: &mut sqlx::SqliteConnection,
    mut builder: QueryBuilder<Sqlite>,
    strategy: Strategy,
    explain: bool,
) -> Result<(Vec<SqliteRow>, f64, String)> {
    let sql = strategy.sql(builder.sql().as_str());
    let mut query = builder.build();
    let arguments = query
        .take_arguments()
        .map_err(|e| StorageError::new(ErrorCode::Database, "diagnose", e))?
        .expect("builder arguments");
    let actual = if explain {
        format!("EXPLAIN QUERY PLAN {sql}")
    } else {
        sql.clone()
    };
    let start = Instant::now();
    let rows = sqlx::query_with(sqlx::AssertSqlSafe(actual.as_str()), arguments)
        .fetch_all(connection)
        .await?;
    Ok((rows, start.elapsed().as_secs_f64() * 1000.0, sql))
}

fn count_builder(criteria: &Criteria) -> QueryBuilder<Sqlite> {
    let mut query = QueryBuilder::new(if criteria.unfiltered() {
        "SELECT count(*) FROM games"
    } else {
        "SELECT count(*) FROM games g WHERE 1=1"
    });
    if !criteria.unfiltered() {
        criteria.predicate(&mut query);
    }
    query
}
fn selection(request: &QueryRequest) -> QueryBuilder<Sqlite> {
    let mut query = QueryBuilder::new("SELECT g.id FROM games g WHERE 1=1");
    request.criteria.predicate(&mut query);
    query
        .push(" ORDER BY ")
        .push(request.criteria.order())
        .push(" LIMIT ")
        .push_bind(request.limit)
        .push(" OFFSET ")
        .push_bind(request.offset);
    query
}
fn projection(request: &QueryRequest, ids: &[String]) -> QueryBuilder<Sqlite> {
    let mut query = QueryBuilder::new("WITH page(id) AS MATERIALIZED (");
    if ids.is_empty() {
        query.push("SELECT NULL WHERE 0");
    } else {
        query.push("VALUES ");
        let mut separated = query.separated(",");
        for id in ids {
            separated
                .push("(")
                .push_bind_unseparated(id.clone())
                .push_unseparated(")");
        }
    }
    query
        .push(") ")
        .push(super::super::summary_projection(request.criteria.order()));
    query
}

pub fn scenarios(source: Option<String>) -> Vec<(String, Criteria, bool)> {
    let mut cases = vec![("default".into(), Criteria::default(), false)];
    for (name, search) in [
        ("searchCommon", "chron"),
        ("searchRare", "Afterlight 2"),
        ("noMatch", "no-such-title-xyz"),
        ("unicode", "\u{661f}\u{306e}"),
        ("punctuation", "%_\\\"'"),
    ] {
        cases.push((
            name.into(),
            Criteria {
                search: search.into(),
                ..Default::default()
            },
            false,
        ));
    }
    cases.push((
        "source".into(),
        Criteria {
            source_id: source.clone(),
            ..Default::default()
        },
        false,
    ));
    cases.push((
        "year".into(),
        Criteria {
            year_from: Some(2000),
            year_to: Some(2001),
            ..Default::default()
        },
        false,
    ));
    for (name, installed) in [
        ("all", InstallationFilter::All),
        ("installed", InstallationFilter::Installed),
        ("notInstalled", InstallationFilter::NotInstalled),
    ] {
        for (sort_name, sort, direction) in [
            ("titleAsc", SummarySort::Title, Direction::Asc),
            ("titleDesc", SummarySort::Title, Direction::Desc),
            ("releaseAsc", SummarySort::ReleaseDate, Direction::Asc),
            ("releaseDesc", SummarySort::ReleaseDate, Direction::Desc),
        ] {
            for deep in [false, true] {
                cases.push((
                    format!("{name}-{sort_name}-{}", if deep { "deep" } else { "first" }),
                    Criteria {
                        installed,
                        sort,
                        direction,
                        ..Default::default()
                    },
                    deep,
                ));
            }
        }
    }
    for installed in [
        InstallationFilter::Installed,
        InstallationFilter::NotInstalled,
    ] {
        cases.push((
            format!("combined-{installed:?}"),
            Criteria {
                search: "chron".into(),
                installed,
                source_id: source.clone(),
                year_from: Some(1990),
                year_to: Some(2020),
                sort: SummarySort::ReleaseDate,
                direction: Direction::Desc,
            },
            true,
        ));
    }
    cases
}

impl Database {
    pub async fn diagnose_queries(&self, iterations: u32, all_strategies: bool) -> Result<Value> {
        self.diagnose_query_set(iterations, if all_strategies { "all" } else { "finalists" })
            .await
    }

    pub async fn diagnose_relationship_queries(&self, iterations: u32) -> Result<Value> {
        self.diagnose_query_set(iterations, "relationships").await
    }

    async fn diagnose_query_set(&self, iterations: u32, set: &str) -> Result<Value> {
        if !(1..=10000).contains(&iterations) {
            return Err(invalid("Invalid diagnostic iteration count"));
        }
        let mut connection = self.pool.acquire().await?;
        let mut results = Vec::new();
        let source = self.source_options().await?.first().map(|s| s.id.clone());
        let mut cases = scenarios(source.clone());
        if set == "relationships" {
            for installed in [
                InstallationFilter::Installed,
                InstallationFilter::NotInstalled,
            ] {
                for (sort, direction) in [
                    (SummarySort::Title, Direction::Asc),
                    (SummarySort::ReleaseDate, Direction::Desc),
                ] {
                    for deep in [false, true] {
                        cases.push((
                            format!(
                                "source-{installed:?}-{sort:?}-{}",
                                if deep { "deep" } else { "first" }
                            ),
                            Criteria {
                                installed,
                                sort,
                                direction,
                                source_id: source.clone(),
                                ..Default::default()
                            },
                            deep,
                        ));
                    }
                }
            }
        }
        for (name, criteria, deep) in cases {
            let count: i64 = execute(
                &mut connection,
                count_builder(&criteria),
                Strategy::Baseline,
                false,
            )
            .await?
            .0[0]
                .get(0);
            let request = QueryRequest {
                limit: 100,
                offset: if deep {
                    count.saturating_sub(100).max(0) as u32
                } else {
                    0
                },
                criteria,
            };
            let expected = execute(
                &mut connection,
                request.builder(false),
                Strategy::Baseline,
                false,
            )
            .await?
            .0;
            let expected = expected.iter().map(row_value).collect::<Vec<_>>();
            let strategies: &[Strategy] = if set == "relationships" {
                &[
                    Strategy::Baseline,
                    Strategy::OrderedEntrySet,
                    Strategy::PageMembership,
                    Strategy::PageCorrelated,
                ]
            } else if set == "all" {
                &[
                    Strategy::Baseline,
                    Strategy::InstallationJoin,
                    Strategy::Correlated,
                    Strategy::OrderedMembership,
                    Strategy::OrderedJoin,
                    Strategy::EntrySet,
                    Strategy::OrderedEntrySet,
                ]
            } else {
                &[Strategy::Baseline, Strategy::OrderedEntrySet]
            };
            let mut measurements = Vec::new();
            for &strategy in strategies {
                // Warm every shape before the alternating timed rounds.
                execute(
                    &mut connection,
                    count_builder(&request.criteria),
                    strategy,
                    false,
                )
                .await?;
                execute(&mut connection, request.builder(false), strategy, false).await?;
                measurements.push((
                    strategy,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                ));
            }
            drop(connection);
            for round in 0..iterations {
                for index in 0..strategies.len() {
                    let index = if round % 2 == 0 {
                        index
                    } else {
                        strategies.len() - 1 - index
                    };
                    let (strategy, counts, pages, selections, projections, pairs, waits) =
                        &mut measurements[index];
                    let waiting = Instant::now();
                    connection = self.pool.acquire().await?;
                    waits.push(waiting.elapsed().as_secs_f64() * 1000.0);
                    let (found, count_ms, _) = execute(
                        &mut connection,
                        count_builder(&request.criteria),
                        *strategy,
                        false,
                    )
                    .await?;
                    let (rows, page_ms, _) =
                        execute(&mut connection, request.builder(false), *strategy, false).await?;
                    if found[0].get::<i64, _>(0) != count
                        || rows.iter().map(row_value).collect::<Vec<_>>() != expected
                    {
                        return Err(invalid("Diagnostic strategy changed count/rows"));
                    }
                    let (ids, select_ms, _) =
                        execute(&mut connection, selection(&request), *strategy, false).await?;
                    let ids = ids
                        .iter()
                        .map(|r| r.get::<String, _>(0))
                        .collect::<Vec<_>>();
                    let (projected, project_ms, _) = execute(
                        &mut connection,
                        projection(&request, &ids),
                        *strategy,
                        false,
                    )
                    .await?;
                    if projected.iter().map(row_value).collect::<Vec<_>>() != expected {
                        return Err(invalid("Split selection/projection changed rows"));
                    }
                    counts.push(count_ms);
                    pages.push(page_ms);
                    pairs.push(count_ms + page_ms);
                    selections.push(select_ms);
                    projections.push(project_ms);
                    drop(connection);
                }
            }
            connection = self.pool.acquire().await?;
            for (strategy, counts, pages, selections, projections, pairs, waits) in measurements {
                let mut plans = Vec::new();
                for (phase, builder) in [
                    ("count", count_builder(&request.criteria)),
                    ("page", request.builder(false)),
                    ("selection", selection(&request)),
                ] {
                    let (rows, _, sql) = execute(&mut connection, builder, strategy, true).await?;
                    plans.push(json!({"phase":phase,"sql":sql,"plan":rows.iter().map(|r|r.get::<String,_>(3)).collect::<Vec<_>>() }));
                }
                results.push(json!({"name": name, "strategy":format!("{strategy:?}"), "request":request,"count":count,"countMs":distribution(&counts),"pageMs":distribution(&pages),"combinedMs":distribution(&pairs),"selectionMs":distribution(&selections),"projectionMs":distribution(&projections),"poolMs":distribution(&waits),"plans":plans}));
            }
        }
        let sizes: Vec<(String, i64)> =
            sqlx::query_as("SELECT name,sum(pgsize) FROM dbstat GROUP BY name ORDER BY name")
                .fetch_all(&mut *connection)
                .await?;
        let version: String = sqlx::query_scalar("SELECT sqlite_version()")
            .fetch_one(&mut *connection)
            .await?;
        Ok(
            json!({"sqlite":version,"iterations":iterations,"cache":"Warm each shape; alternate forward/reverse strategy order each round; OS cache retained. Same SQLx connection per paired sample. Split selection/projection are diagnostic, not additive to page.","parameterDescription":"Synthetic criteria in request; escaped LIKE pattern then source/year bounds then limit/offset, all bound. No user data.","sizes":sizes,"scenarios":results}),
        )
    }
}

fn row_value(row: &SqliteRow) -> Value {
    json!([
        row.get::<String, _>(0),
        row.get::<String, _>(1),
        row.get::<Option<String>, _>(2),
        row.get::<Option<String>, _>(3),
        row.get::<Option<String>, _>(4),
        row.get::<i64, _>(5),
        row.get::<String, _>(6),
        row.get::<i64, _>(7),
        row.get::<i64, _>(8),
        row.get::<i64, _>(9)
    ])
}
