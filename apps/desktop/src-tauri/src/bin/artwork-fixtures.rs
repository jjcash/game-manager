// SPDX-License-Identifier: GPL-3.0-or-later
use game_manager_lib::artwork::fixtures::{self, Config};
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [action, root, games, seed] if action == "generate" => games
            .parse::<u32>()
            .map_err(|e| e.to_string())
            .and_then(|games| {
                seed.parse::<u64>()
                    .map_err(|e| e.to_string())
                    .and_then(|seed| {
                        fixtures::generate(
                            std::path::Path::new(root),
                            Config {
                                version: 1,
                                games,
                                seed,
                            },
                        )
                    })
            }),
        [action, root] if action == "clean" => fixtures::clean(std::path::Path::new(root)),
        _ => Err(
            "Usage: artwork-fixtures generate ABSOLUTE_ROOT GAMES SEED | clean ABSOLUTE_ROOT"
                .into(),
        ),
    };
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
