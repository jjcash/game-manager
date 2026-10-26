# Synthetic SQLite benchmarks

M0.2 contains database measurements only. No storefront, game files, accounts, artwork, frontend rendering, or background imports are involved.

Use Windows PowerShell with Cargo on PATH, from the repository root:

```powershell
# Choose a NEW absolute directory on the local drive being measured.
./benchmarks/run.ps1 -DatabaseDirectory E:\dev\game-manager\benchmarks\data\run-01 -Seed 42 -Iterations 50
```

The script builds the release CLI and generates all six supported game counts: 500, 2,000, 5,000, 12,320, 15,000, and 25,000. It prints a JSON report with generation duration, initialization duration, closed database size, exact relational counts, SQLite settings/version, query parameters/plans, and timing statistics. Redirect output to an explicitly chosen new file under `benchmarks/results/` if desired. Both data and results directories are ignored; the verified baseline belongs in `docs/development/M0.2_VERIFICATION.md`.

Individual commands, also from the repository root:

```powershell
cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --bin library-bench --features benchmarks --release --locked
$bench = './apps/desktop/src-tauri/target/release/library-bench.exe'
& $bench generate --database E:\dev\game-manager\benchmarks\data\example-15000.sqlite3 --games 15000 --seed 42
& $bench measure --database E:\dev\game-manager\benchmarks\data\example-15000.sqlite3 --iterations 50
```

`--database` must be explicit, absolute and local. `generate` requires `--games` and an unsigned 64-bit `--seed`; it refuses existing files and orphan SQLite sidecars. There is no overwrite/reset switch. Select a new filename for another run. `measure` only opens an existing benchmark database with complete generator metadata. A purpose marker prevents opening benchmark data as the application library or seeding application data. Errors are structured JSON on stderr and produce a nonzero exit code. A failed generation is preserved with an incomplete marker and is never automatically resumed, erased, or reported as successful.

Generator version 1 uses SplitMix64 with explicit wrapping arithmetic. Seed and ordinal define stable internal IDs, independently of synthetic source identifiers. A bijective title composition avoids repeated canonical titles within the supported sizes. Metadata includes Unicode, missing and varied release dates (1985–2025), eight genres, 60 fictional studios, favorites, four completion states, five weighted synthetic sources, missing/present copies, and games with multiple entries/installations. Installation locations are `synthetic://` references; no installation directories are created or read. Output is logically deterministic; SQLite file bytes and SQLx migration execution times are not a reproducibility contract.

Each transaction inserts at most 250 games and their associated entries/installations using bound bulk inserts. The single CLI writer commits batches sequentially. Completion is marked only after all batches and integrity/count validation. Counts and timings exclude external data access because none occurs.

Measurements use the actual summary service: 100 rows sorted ascending by `(sort_title, id)` or `(release_date, id)`, at offsets 0, half the dataset, and the last 100 rows. Release-date NULLs sort first. Every warm result is compared with the first result. First query after pool reopen and warm min/median/p95/max are reported separately. The OS file cache is retained; neither is a controlled OS-cold measurement. Reopening also measures schema validation and WAL/pool setup. Generation time excludes initialization, post-generation integrity checks, and final checkpoint/close. Size is the main SQLite file after all connections close, not a live-WAL copy.

Record CPU, RAM, OS, physical storage/filesystem, absolute data path, seed, generator version, build/toolchain, iterations, cache state, and concurrent work alongside each baseline. Do not compare SSD/HDD or lower-memory behavior unless those configurations were actually measured. Scrolling, IPC, search/filtering, frame time, artwork and concurrent import performance remain later milestones.

## Native frontend scenarios (M0.3)

Build with `pnpm tauri build --no-bundle --features benchmarks -- --locked` from `apps/desktop`, then run from the repository root:

```powershell
./benchmarks/frontend.ps1 -DatabaseDirectory E:\dev\game-manager\benchmarks\data\m0.2-2026-10-25-01 -ResultsDirectory E:\dev\game-manager\benchmarks\results\m0.3-run-01
```

Use a new results directory; close existing Game Manager windows first. Node 26 supplies fetch/WebSocket; no driver package or data server is installed. The runner starts the real bundled native app against each existing seed-42 benchmark twice (500, 12,320, 15,000 and 25,000), using a separate WebView profile. Override `-Sizes` or `-Runs` for targeted repeats. Databases are opened read-only, and SHA-256 is checked before/after. Reports, profiles and stderr stay in ignored `benchmarks/results/`.

Remote debugging is explicitly enabled for these child processes on loopback port 9223. It is not enabled by application code or normal startup. `frontend.mjs` attaches only to a matching local WebView URL. It measures initial readiness, dispatches 25 wheel events, performs 100 successive-frame long-distance scroll jumps and 20 distributed windows waited through actual row arrival, presses End and Arrow Up, selects a row, navigates away/back, emulates a 900x650 viewport at scale 1.25, and verifies selection/scroll restoration. Missing-database launches also exercise the error/retry path; page failure/retry and transport cancellation are automated frontend tests. All ordinary pages come from actual Rust/SQLite IPC.

Viewport emulation is not physical window resizing or changing Windows display scaling. Programmatic scroll jumps exercise the arbitrary-offset data path but are not physical scrollbar dragging. Frame samples include automation/idle intervals and must not be presented as achieved FPS. Process-tree private bytes are a point-in-time aggregate, not a leak test; summed working sets can double-count shared pages. First and repeated process runs retain the OS filesystem cache. Neither is a controlled cold-disk startup. Build first and wait for other compilation/check work to finish before measuring.

For manual inspection, set `GAME_MANAGER_BENCHMARK_DB` to the existing absolute dataset path and launch a `benchmarks` build. Development diagnostics identify the source/size and offer frame sampling. For actual React Profiler durations, use `pnpm desktop:dev --features benchmarks`; optimized React builds report no render-duration samples. The verification report records native UI automation success/failure separately.
