# Synthetic SQLite benchmarks

M0.5 also provides bounded synthetic artwork benchmarks. See the [M0.5 verification report](../docs/development/M0.5_VERIFICATION.md) for fixture generation/cleanup, cache ownership, limits, exact commands and measured results. The M0.2 instructions below remain the database-only baseline.

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

## Search, filter and sort scenarios (M0.4)

Use the same native benchmark build and a **new** output directory:

```powershell
./benchmarks/frontend.ps1 -DatabaseDirectory E:\dev\game-manager\benchmarks\data\m0.2-2026-10-25-01 -ResultsDirectory E:\dev\game-manager\benchmarks\results\m0.4-native-run-01 -Runs 1 -QueryScenarios
```

The query driver shares the original CDP transport in `webview.mjs`; default runner behavior still exercises the M0.3 scenarios. Query scenarios cover common/rare/no-match/empty/Unicode/punctuation text, installation complements, source and year combinations, four sorts, loaded deep filtered/sorted rows, rapid search edits, repeated filter switches and ordinary long-distance scrolling. Diagnostics record criteria-to-commit time, native metadata/page latency, backend count/page time and bounded cache/row/request counts. An explicit test-only frame/long-task observer spans criteria generations; it stores bounded samples and no FPS estimate. DOM event-driven control changes are automated inputs, not physical typing/dragging claims.

For database-only plans/timings using the locked SQLx/bundled SQLite engine:

```powershell
cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --bin library-bench --features benchmarks --release --locked
./apps/desktop/src-tauri/target/release/library-bench.exe query-measure --database E:\dev\game-manager\benchmarks\data\m0.2-2026-10-25-01\synthetic-v1-seed-42-15000.sqlite3 --iterations 50
./apps/desktop/src-tauri/target/release/library-bench.exe search-evaluate --database E:\dev\game-manager\benchmarks\data\m0.2-2026-10-25-01\synthetic-v1-seed-42-15000.sqlite3
```

`query-measure` returns counts, stable-window comparisons, count/page/combined distributions and actual plans for 14 scenarios, including deep offsets. `search-evaluate` compares LIKE and FTS5 trigram on one connection's freshly built TEMP table, discarding one warm-up before 50 samples; creation cost is excluded. Both open the main benchmark database read-only and do not run migrations. FTS TEMP data is a disposable experiment, not a maintained production index. Run at 500, 12,320, 15,000 and 25,000, with no competing compilation/tests for final measurements. Retain machine/cache state and comparison limitations with results; see [M0.4 verification](../docs/development/M0.4_VERIFICATION.md).

## M0.5 artwork and normal-build controls

Generate exclusively local procedural fixtures with the benchmark-only `artwork-fixtures` CLI. Exact generation/cleanup commands and measured results are in [M0.5 verification](../docs/development/M0.5_VERIFICATION.md). No normal application database is seeded; cache eviction never deletes originals.

The existing `frontend.ps1` runner accepts `-ArtworkDirectory` containing `artwork-500`, `artwork-12320`, `artwork-15000` and `artwork-25000` roots. `-ArtworkScenarios` runs artwork scrolling/cache checks followed by the query matrix; `-ArtworkIpcScenarios` instead saturates caches and records 50 repeated native metadata/window samples per criterion under two artwork requests. `-ArtworkCycles 20` extends repeated visits (default 3, allowed 1-50). `-ProfileArtwork` saves a separate V8 CPU profile. `-NoMemorySeries` disables periodic WMI sampling for observer-effect controls while retaining the final memory point. `-BenchmarkExecutable` can select an explicitly retained byte-identical test artifact; this introduces no portable application mode.

Build without benchmark features before `-NormalSmoke -Sizes 500 -Runs 1`: it verifies an empty isolated application library, successful Rust IPC, ignored benchmark dataset/artwork selection, no diagnostics and no alerts. It uses the same explicit source-database hashes, isolated data/profile paths, loopback transport and normal window shutdown as the benchmark runner. This is the preferred normal-build launch check.

`library-bench query-measure --artwork ABSOLUTE_FIXTURE_ROOT --database ABSOLUTE_BENCHMARK_DB --iterations 50` adds two continuous artwork workers to the unchanged SQL scenarios. Run final measurements sequentially without competing builds/tests/generation, identify cache state and keep native IPC, SQL-only, rendering and profile observations separate.

## M0.6 bounded synthetic jobs

See [M0.6 verification](../docs/development/M0.6_VERIFICATION.md) for state transitions, limits, safety, complete measurements and remaining failures. This is a feature-gated simulator, not a real importer. Build the native application with `--features benchmarks` as above; normal release builds omit job handlers and ignore the synthetic configuration.

```powershell
./benchmarks/background.ps1 -DatabaseDirectory E:\dev\game-manager\benchmarks\data\m0.2-2026-10-25-01 -ArtworkDirectory E:\dev\game-manager\benchmarks\data -ResultsDirectory E:\dev\game-manager\benchmarks\results\m0.6-new-run
```

The four datasets are 500/12,320/15,000/25,000, seed 42. Default conditions are `idle`, `cpu`, `hash`, `write`, `art1`, `art2`, `mixed0`, `mixed1`, `mixed2`, `priority`. Parsing, hashing and mixed requests use seed 20261026 and 100,000 saturated records, restarted as necessary while the query/UI probes run. No-art conditions disable thumbnail work; artwork conditions use one/two continuous requests during the isolated IPC probe. Mixed conditions share a two-worker CPU ceiling with the job; `mixed0` keeps the same job kind/seed/resize while disabling real thumbnail work. `priority` is mixed/two-art with foreground-aware admission. Record active-job duty samples; do not assume background work was active simply because requested.

Every condition gets an independently generated disposable job database under `RESULTS/benchmarks/data/jobs-SIZE-CASE`, an empty derived-art cache and a fresh profile. Known synthetic original files are hardlinked on NTFS without writing them. Canonical source databases are SHA-256 checked. Four criteria retain 20 count/page timing samples each. Native UI probes exercise text, composed source/year/installation filters, sorting, rapid jumps, tail selection and real job controls. Each run then tests exact completion, bounded admission, cancellation, rollback/recovery and validates the closed database. The runner holds process handles and requires clean native shutdown.

Options: `-Sizes @(15000,25000)`, `-Cases @('mixed0','art1','art2','mixed1','mixed2','priority')`, `-Iterations 20`, `-Cycles 5`, `-SampleMemory`, `-Trace`, `-TraceSort`, `-ClosedDiagnostics`, `-ShutdownJob -RestartCheck`. Use separate new results directories for memory/profile/shutdown controls. Default primary latency measurements have no periodic WMI sampling; a final process-memory point remains outside the observed frames. Trace captures have a 16-MiB ring and 64-MiB output cap. Whole-run trace rings can overwrite earlier events under high load; use `-TraceSort` to retain a focused transition, optionally paired with `-ClosedDiagnostics`. The frame observer separates timed IPC/UI from later safety/cycle phases, retains bounded intervals/actions/long tasks and makes no FPS claim.

The `job-fixtures.exe` benchmark-only CLI generates, audits and safely cleans an explicit root:

```powershell
$jobs = './apps/desktop/src-tauri/target/release/job-fixtures.exe'
& $jobs E:\dev\game-manager\benchmarks\data\jobs-example 15000 42
& $jobs verify E:\dev\game-manager\benchmarks\data\jobs-example
& $jobs clean E:\dev\game-manager\benchmarks\data\jobs-example
```

Root naming, marker/purpose checks, canonical paths, exclusive lease and owned-file inventory restrict cleanup. Close the app before verification/cleanup. Reconstruct by generating a new root. No automatic cleanup of failed generation, source databases, artwork originals or personal data occurs. M0.5's artwork CLI remains the original-source cleanup mechanism.

Normal-production verification uses the existing `frontend.ps1 -NormalSmoke -Sizes 500 -Runs 1` after an ordinary build. It now also verifies absent synthetic controls/rejected job IPC and ignores deliberately invalid synthetic worker/policy/root environment values.
