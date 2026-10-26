// SPDX-License-Identifier: GPL-3.0-or-later

use game_manager_lib::storage::{self, PageRequest, SummarySort, synthetic};
use serde::Serialize;
use std::{collections::BTreeMap, path::PathBuf, time::Instant};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Timing {
    min_ms: f64,
    median_ms: f64,
    p95_ms: f64,
    max_ms: f64,
}
fn timing(mut samples: Vec<f64>) -> Timing {
    samples.sort_by(f64::total_cmp);
    let len = samples.len();
    Timing {
        min_ms: samples[0],
        median_ms: (samples[(len - 1) / 2] + samples[len / 2]) / 2.0,
        p95_ms: samples[(len * 95).div_ceil(100) - 1],
        max_ms: samples[len - 1],
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Scenario {
    request: PageRequest,
    reopen_ms: f64,
    first_after_reopen_ms: f64,
    warm: Timing,
    query_plan: Vec<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Measurement {
    database: PathBuf,
    config: synthetic::DatasetConfig,
    iterations: u32,
    cache_state: &'static str,
    status: storage::DatabaseStatus,
    scenarios: Vec<Scenario>,
}

fn invalid(message: impl ToString) -> storage::StorageError {
    storage::StorageError::new(storage::ErrorCode::InvalidRequest, "cli", message)
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!(
            "{}",
            serde_json::to_string(&error).expect("serializable error")
        );
        std::process::exit(1);
    }
}

async fn run() -> storage::Result<()> {
    let mut args = std::env::args().skip(1);
    let action = args.next().ok_or_else(|| invalid("Usage: library-bench generate|measure --database ABSOLUTE_PATH [--games N --seed N | --iterations N]"))?;
    let mut options = BTreeMap::new();
    while let Some(key) = args.next() {
        if !key.starts_with("--") {
            return Err(invalid("Expected named option"));
        }
        let value = args.next().ok_or_else(|| invalid("Missing option value"))?;
        if options.insert(key, value).is_some() {
            return Err(invalid("Duplicate option"));
        }
    }
    let path = PathBuf::from(
        options
            .remove("--database")
            .ok_or_else(|| invalid("--database is required and must be an absolute local path"))?,
    );
    storage::validate_path(&path)?;
    match action.as_str() {
        "query-measure" => {
            let iterations: u32 = options
                .remove("--iterations")
                .unwrap_or_else(|| "50".into())
                .parse()
                .map_err(invalid)?;
            if !options.is_empty() || !(1..=10000).contains(&iterations) {
                return Err(invalid("Invalid iterations/options"));
            }
            measure_queries(path, iterations).await?;
        }
        "search-evaluate" => {
            if !options.is_empty() {
                return Err(invalid("Unknown option"));
            }
            let db = storage::Database::benchmark_view(&path).await?;
            let report = db.evaluate_search().await?;
            db.close().await;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).expect("serializable report")
            );
        }
        "generate" => {
            let games = options
                .remove("--games")
                .ok_or_else(|| invalid("--games is required"))?
                .parse()
                .map_err(invalid)?;
            let seed = options
                .remove("--seed")
                .ok_or_else(|| invalid("--seed is required"))?
                .parse()
                .map_err(invalid)?;
            if !options.is_empty() {
                return Err(invalid("Unknown option"));
            }
            let report =
                synthetic::generate(&path, synthetic::DatasetConfig { games, seed }).await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).expect("serializable report")
            );
        }
        "measure" => {
            let iterations: u32 = options
                .remove("--iterations")
                .unwrap_or_else(|| "50".into())
                .parse()
                .map_err(invalid)?;
            if !options.is_empty() || !(1..=10_000).contains(&iterations) {
                return Err(invalid("Unknown option or iterations outside 1..=10000"));
            }
            let db = synthetic::open_benchmark(&path).await?;
            let config = synthetic::dataset_config(&db).await?;
            db.validate().await?;
            let status = db.status().await?;
            if status.counts.games != i64::from(config.games) {
                return Err(invalid("Dataset count does not match metadata"));
            }
            db.close().await;
            let mut scenarios = Vec::new();
            for sort in [SummarySort::Title, SummarySort::ReleaseDate] {
                for offset in [0, config.games / 2, config.games.saturating_sub(100)] {
                    let start = Instant::now();
                    let db = synthetic::open_benchmark(&path).await?;
                    let reopen_ms = start.elapsed().as_secs_f64() * 1000.0;
                    let request = PageRequest {
                        limit: 100,
                        offset,
                        sort,
                    };
                    let start = Instant::now();
                    let first = db.page(&request).await?;
                    let first_after_reopen_ms = start.elapsed().as_secs_f64() * 1000.0;
                    if first.len() != 100 {
                        return Err(invalid("Unexpected page length"));
                    }
                    let mut samples = Vec::with_capacity(iterations as usize);
                    for _ in 0..iterations {
                        let start = Instant::now();
                        let page = db.page(&request).await?;
                        samples.push(start.elapsed().as_secs_f64() * 1000.0);
                        if page != first {
                            return Err(invalid("Unstable page contents"));
                        }
                        std::hint::black_box(page);
                    }
                    let query_plan = db.page_query_plan(&request).await?;
                    scenarios.push(Scenario {
                        request,
                        reopen_ms,
                        first_after_reopen_ms,
                        warm: timing(samples),
                        query_plan,
                    });
                    db.close().await;
                }
            }
            let report = Measurement {
                database: path,
                config,
                iterations,
                cache_state: "First query after pool reopen; OS cache retained. Warm iterations follow. No OS-cold claim.",
                status,
                scenarios,
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&report).expect("serializable report")
            );
        }
        _ => return Err(invalid("Unknown action; use generate or measure")),
    }
    Ok(())
}

async fn measure_queries(path: PathBuf, iterations: u32) -> storage::Result<()> {
    use storage::query::{Criteria, Direction, InstallationFilter, QueryRequest};
    let db = storage::Database::benchmark_view(&path).await?;
    let config = synthetic::dataset_config(&db).await?;
    let source = db.source_options().await?.first().map(|s| s.id.clone());
    let cases = vec![
        ("empty", Criteria::default(), false),
        (
            "common",
            Criteria {
                search: "chron".into(),
                ..Default::default()
            },
            false,
        ),
        (
            "rare",
            Criteria {
                search: "Afterlight 2".into(),
                ..Default::default()
            },
            false,
        ),
        (
            "noMatch",
            Criteria {
                search: "no-such-title-xyz".into(),
                ..Default::default()
            },
            false,
        ),
        (
            "unicodeShort",
            Criteria {
                search: "\u{661f}\u{306e}".into(),
                ..Default::default()
            },
            false,
        ),
        (
            "punctuation",
            Criteria {
                search: "%_\\\"'".into(),
                ..Default::default()
            },
            false,
        ),
        (
            "installed",
            Criteria {
                installed: InstallationFilter::Installed,
                ..Default::default()
            },
            false,
        ),
        (
            "notInstalled",
            Criteria {
                installed: InstallationFilter::NotInstalled,
                ..Default::default()
            },
            false,
        ),
        (
            "source",
            Criteria {
                source_id: source.clone(),
                ..Default::default()
            },
            false,
        ),
        (
            "yearRange",
            Criteria {
                year_from: Some(2000),
                year_to: Some(2001),
                ..Default::default()
            },
            false,
        ),
        (
            "titleDescDeep",
            Criteria {
                direction: Direction::Desc,
                ..Default::default()
            },
            true,
        ),
        (
            "releaseAscDeep",
            Criteria {
                sort: SummarySort::ReleaseDate,
                ..Default::default()
            },
            true,
        ),
        (
            "releaseDescDeep",
            Criteria {
                sort: SummarySort::ReleaseDate,
                direction: Direction::Desc,
                ..Default::default()
            },
            true,
        ),
        (
            "combined",
            Criteria {
                search: "chron".into(),
                installed: InstallationFilter::Installed,
                source_id: source,
                year_from: Some(1990),
                year_to: Some(2020),
                sort: SummarySort::ReleaseDate,
                direction: Direction::Desc,
            },
            true,
        ),
    ];
    let mut results = Vec::new();
    for (name, criteria, deep) in cases {
        let count = db.query_count(&criteria).await?;
        let request = QueryRequest {
            offset: if deep {
                count.saturating_sub(100).max(0) as u32
            } else {
                0
            },
            limit: 100,
            criteria,
        };
        let first = db.query_window(&request).await?;
        let mut counts = Vec::new();
        let mut pages = Vec::new();
        let mut combined = Vec::new();
        for _ in 0..iterations {
            let start = Instant::now();
            let found = db.query_count(&request.criteria).await?;
            let count_ms = start.elapsed().as_secs_f64() * 1000.0;
            let result = db.query_window(&request).await?;
            combined.push(start.elapsed().as_secs_f64() * 1000.0);
            counts.push(count_ms);
            pages.push(result.query_ms);
            if found != count
                || result.items != first.items
                || result.items.len() != (count - i64::from(request.offset)).clamp(0, 100) as usize
            {
                return Err(invalid("Unstable count/window"));
            }
        }
        results.push(serde_json::json!({"name":name,"request":request,"count":count,"countMs":timing(counts),"pageMs":timing(pages),"combinedMs":timing(combined),"plans":db.criteria_plans(&request).await?}));
    }
    db.close().await;
    println!("{}",serde_json::to_string_pretty(&serde_json::json!({"database":path,"config":config,"iterations":iterations,"scenarios":results})).unwrap());
    Ok(())
}
