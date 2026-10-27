// SPDX-License-Identifier: GPL-3.0-or-later
#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [action, root] if action == "verify" => {
            match game_manager_lib::jobs::fixtures::audit(std::path::Path::new(root)).await {
                Ok(report) => {
                    println!("{}", serde_json::to_string_pretty(&report).unwrap());
                    Ok(())
                }
                Err(e) => Err(e.to_string()),
            }
        }
        [action, root] if action == "clean" => {
            game_manager_lib::jobs::fixtures::clean(std::path::Path::new(root))
                .await
                .map_err(|e| e.to_string())
        }
        [root, games, seed] => match (games.parse(), seed.parse()) {
            (Ok(games), Ok(seed)) => game_manager_lib::jobs::fixtures::generate(
                std::path::Path::new(root),
                game_manager_lib::storage::synthetic::DatasetConfig { games, seed },
            )
            .await
            .map_err(|e| e.to_string()),
            _ => Err("Invalid games/seed".into()),
        },
        _ => Err("Usage: job-fixtures ABSOLUTE_BENCHMARK_ROOT GAMES SEED".into()),
    };
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
