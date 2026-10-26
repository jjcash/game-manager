// SPDX-License-Identifier: GPL-3.0-or-later
// Development-only comparison on one connection's TEMP tables. Main DB is read-only.
use super::{Database, Result};
use serde::Serialize;
use sqlx::Row;
use std::time::Instant;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Evaluation {
    term: String,
    strategy: &'static str,
    count: i64,
    median_ms: f64,
    p95_ms: f64,
    plan: Vec<String>,
}

impl Database {
    pub async fn evaluate_search(&self) -> Result<Vec<Evaluation>> {
        let mut c = self.pool.acquire().await?;
        sqlx::query("CREATE VIRTUAL TABLE temp.search_evaluation USING fts5(game_id UNINDEXED, title, tokenize='trigram')").execute(&mut *c).await?;
        sqlx::query("INSERT INTO temp.search_evaluation SELECT id,title FROM games")
            .execute(&mut *c)
            .await?;
        let mut results = Vec::new();
        for term in [
            "chron",
            "Afterlight 2",
            "no-such-title-xyz",
            "\u{661f}\u{306e}",
        ] {
            for strategy in ["literalLike", "trigramMatch"] {
                let (predicate, bound) = if strategy == "literalLike" {
                    ("title LIKE ? ESCAPE '\\'", format!("%{term}%"))
                } else {
                    (
                        "id IN (SELECT game_id FROM temp.search_evaluation WHERE search_evaluation MATCH ?)",
                        format!("\"{term}\""),
                    )
                };
                let count_sql = format!("SELECT count(*) FROM games WHERE {predicate}");
                let page_sql = format!(
                    "SELECT id FROM games WHERE {predicate} ORDER BY sort_title,id LIMIT 100"
                );
                let mut samples = Vec::new();
                let mut count = 0;
                for _ in 0..51 {
                    let start = Instant::now();
                    count = sqlx::query_scalar(sqlx::AssertSqlSafe(count_sql.as_str()))
                        .bind(&bound)
                        .fetch_one(&mut *c)
                        .await?;
                    let rows: Vec<String> =
                        sqlx::query_scalar(sqlx::AssertSqlSafe(page_sql.as_str()))
                            .bind(&bound)
                            .fetch_all(&mut *c)
                            .await?;
                    std::hint::black_box(rows);
                    samples.push(start.elapsed().as_secs_f64() * 1000.0);
                }
                samples.remove(0); // same warm, OS-cache-retained methodology for each strategy
                samples.sort_by(f64::total_cmp);
                let mut plan = Vec::new();
                for sql in [&count_sql, &page_sql] {
                    let explain = format!("EXPLAIN QUERY PLAN {sql}");
                    plan.extend(
                        sqlx::query(sqlx::AssertSqlSafe(explain))
                            .bind(&bound)
                            .fetch_all(&mut *c)
                            .await?
                            .iter()
                            .map(|r| r.get::<String, _>(3)),
                    );
                }
                results.push(Evaluation {
                    term: term.into(),
                    strategy,
                    count,
                    median_ms: (samples[24] + samples[25]) / 2.0,
                    p95_ms: samples[47],
                    plan,
                });
            }
        }
        sqlx::query("DROP TABLE temp.search_evaluation")
            .execute(&mut *c)
            .await?;
        Ok(results)
    }
}
