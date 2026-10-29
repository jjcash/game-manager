# Synthetic performance benchmarks

M0 gate evidence: [conditional closure with documented exceptions](../docs/development/M0.8_GATE_DISPOSITION.md), accepted by the user on 2026-10-28 under ADR-007. M0 is conditionally closed with E1–E5; historical investigation completion does not establish every performance target passed.

## M1.1 manual library verification

Use an ordinary optimized executable for the production manual workflow. The runner reserves a new output directory, sets a disposable application-purpose root and WebView profile, and performs four real launches (create, reopen/edit, reopen/remove, reopen/empty). All writes go through the UI and narrow Rust commands; installation locations are opaque strings. No personal database or game installation is used.

```powershell
./benchmarks/manual.ps1 -ResultsDirectory E:/dev/game-manager/benchmarks/results/manual-new
./benchmarks/manual-query.ps1 -ResultsDirectory E:/dev/game-manager/benchmarks/results/manual-query-new `
  -BaselineExecutable E:/dev/game-manager/benchmarks/results/m1.1-baseline/game-manager.exe
```

The query runner uses SQLite backup to create exclusively new application-purpose copies of the 15k/25k synthetic fixtures, runs schema-v1 baseline and schema-v2 ordinary builds in reversed order over two passes, and retains 20 warm full-summary/query/invoke samples per first/deep case after one warm-up. Exact repeated rows and page lengths are asserted. Original fixture hashes must match afterward. OS caches are retained, fresh WebView profiles are used per launch, and timings are finite service measurements without displayed-frame claims. Build/test work must finish before measuring. M0 reference databases, generator and historical migration remain v1; the application migrator adds v2 only to application databases. See [M1.1 verification](../docs/development/M1.1_VERIFICATION.md).

## M1.4 offline backup and restore verification

Build an ordinary optimized executable and close existing Game Manager windows. This interactive smoke reserves a new disposable application root and profile, exercises native save/open dialogs, purpose/collision/cancellation gates, confirmation, actual Tauri restart and a subsequent process launch. Follow the six file-picker instructions printed by the driver; use only files inside the new smoke root. The runner creates its standalone benchmark-purpose rejection fixture with SQLite online backup from the frozen 500-entry reference; do not select a live reference database with sidecars. The completed phases require pending restore authority retired and historical status preserved. It never introduces path-taking production IPC or accesses a game installation.

```powershell
./benchmarks/backup.ps1 -ResultsDirectory E:/dev/game-manager/benchmarks/results/backup-new
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --lib storage::maintenance
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --release --locked --lib storage::maintenance::tests::release_and_stress_scale_maintenance_probe -- --exact --nocapture
```

The real SQLite tests use disposable roots, including native sharing/ACL denial and child-process exits that bypass Rust destructors. Physical power loss and a genuinely full volume are separate unverified conditions; injected partial-output StorageFull is labelled in the [report](../docs/development/M1.4_BACKUP_RESTORE_VERIFICATION.md). Run timing probes after builds/tests finish. Preserve all six frozen reference fixture hashes.

## M1.2 Steam contract fixtures

Run from `apps/desktop`:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --locked --test steam_discovery
```

These are synthetic, offline parser and policy tests, not a Steam scan or import benchmark. They do not open Steam roots, account/config/cache files, registry values, game directories or a production database. The support module is compiled only by the test target; known binary headers are deferred, not parsed. The [fixture inventory](../apps/desktop/src-tauri/tests/fixtures/steam/README.md) and [M1.2 report](../docs/development/M1.2_STEAM_DISCOVERY_VERIFICATION.md) distinguish parser correctness, untrusted cached hints, ownership uncertainty and native reader work still required.

Keep all seed-42 references and M0 exceptions unchanged. Production isolation and the existing four-launch manual-library smoke remain applicable; use fresh disposable roots. M1.2 includes one small modal keyboard-boundary correction found by the existing native smoke, without weakening its assertions. No query, schema, resource policy or new storefront benchmark architecture is introduced.

## M0.7E relationship and deep-page investigation

See [M0.7E verification](../docs/development/M0.7E_VERIFICATION.md) for the cost/benefit decision, dataset relevance and separate SQL/native boundaries. Finish builds/tests/fixture generation before collection. Use retained optimized binaries and new result roots; keep OS caches and ordinary desktop activity explicit.

The completed investigation recommends **retaining M0.7B production queries and accepting the documented query limitation**. Ordinary 15,000-game gains are modest or inconsistent; intermediate/extreme gains come with sparse/native regressions. No production index, schema, cache or default changes are selected. The wider M0 gate remains open, including one preserved synthetic writer-acquisition guard failure followed by a successful unchanged-setting repeat.

`query-diagnose --strategies relationships` retains the 34 existing cases and adds eight source/presence/title/release first/deep cases. It compares historical pre-M0.7B SQL, current ordered entry sets, current counts with historical ordered page membership (`PageMembership`), and current counts with correlated page existence (`PageCorrelated`). All are benchmark-only CLI experiments, with exact count/full-summary/split-projection assertions, actual SQL/plans, raw samples and alternating strategy order. Existing `all`/`finalists` options are unchanged.

```powershell
$reference = 'E:\dev\game-manager\benchmarks\data\m0.2-2026-10-25-01'
$fixtures = 'E:\dev\game-manager\benchmarks\results\relationship-fixtures\benchmarks\data'
$cli = 'E:\dev\game-manager\apps\desktop\src-tauri\target\release\library-bench.exe'
python -B benchmarks/distributions.py "$reference\synthetic-v1-seed-42-15000.sqlite3" "$fixtures\overlap.sqlite3" overlap
python -B benchmarks/distributions.py "$reference\synthetic-v1-seed-42-15000.sqlite3" "$fixtures\copies.sqlite3" copies
python -B benchmarks/relationships.py --executable $cli `
  --results E:\dev\game-manager\benchmarks\results\relationships-new `
  --iterations 10 --passes 2 `
  --database "reference-15000=$reference\synthetic-v1-seed-42-15000.sqlite3" `
  --database "overlap=$fixtures\overlap.sqlite3" `
  --database "copies=$fixtures\copies.sqlite3"
python -B benchmarks/summarize-relationships.py benchmarks/results/relationships-new |
  Set-Content -Encoding utf8 benchmarks/results/relationships-new-summary.json
python -B benchmarks/relationships.test.py
```

Add reference 25,000 and the preserved sparse/dense/15,000-fanout paths as additional `--database label=ABSOLUTE_PATH` arguments for the seven-dataset matrix. Labels must be unique lowercase letters/digits/hyphens/underscores. The runner records input/binary hashes, sizes, Git revision, SQLite version, completed files and configuration; dataset order reverses each pass. The summarizer pools **raw** samples with nearest-rank p50/p95/max/n and signed absolute/percentage savings from current production, retaining each pass separately. Split selection/projection are additional statements, never additive components of full-page timing. At ten samples p95 equals maximum; finite repeated-process observations do not establish rare-tail guarantees.

New `overlap`/`copies` fixtures add one membership on 20% of canonical games, with one/four copies respectively. Their membership/presence sets match; only copy cardinality changes. These are bounded synthetic hypotheses, not measured usage prevalence. Original reference and sparse/dense/fanout recipes remain unchanged. Covering-index experiments belong only in new SQLite backup copies; never apply experimental DDL to source fixtures or application databases.

For native first/deep service, IPC and target/full-window arrival, use the established `background.ps1 -IntegratedScenario deep -CaptureIpc -RecoverySeconds 0` with `idle,art2,write,mixed2`, 15,000/25,000 and reversed artifact/case order. Protocol instrumentation is explicit overhead; browser-local DOM readiness/rAF are not compositor presentation. Keep ordinary browsing/query/artwork/job regression drivers and normal-build isolation separate. No startup profiling is required by this milestone.

## M0.7D whole-process startup investigation

See [M0.7D verification](../docs/development/M0.7D_VERIFICATION.md) for phase definitions, paired controls, failures and limitations. Retain the original M0.7C executable; use an optimized benchmark build for added native milestones. The ordinary configured-window path remains the low-overhead timing path:

```powershell
./benchmarks/background.ps1 -DatabaseDirectory $db -ArtworkDirectory $art `
  -ResultsDirectory E:\dev\game-manager\benchmarks\results\startup-timing-new `
  -BenchmarkExecutable $exe -Sizes @(15000,500,12320,25000) `
  -Cases @('idle','art2') -Iterations 5 -IntegratedScenario startup `
  -ReadOnlyLibrary -Launches 5 -RecoverySeconds 0
node benchmarks/startup.mjs benchmarks/results/startup-timing-new |
  Set-Content -Encoding utf8 benchmarks/results/startup-timing-new-summary.json
```

Define `$db`, `$art` and `$exe` as absolute reference database directory, synthetic artwork directory and retained executable path (as in the M0.7C example below). Always select a new results root. Record artifact/order/profile state; reverse before/after order across repeated cohorts. Launch one has a fresh profile, later launches reuse it; all retain OS caches, including preflight hashing. No OS-cold claim follows from profile freshness.

For **separate profiling runs**, add `-ProfileStartupWindow`. Only a benchmark-feature binary honors `GAME_MANAGER_STARTUP_PROFILE_WINDOW`; it constructs the configured window explicitly at setup entry to bracket the synchronous call. Internal asset-setup order differs, so do not mix these samples with ordinary-path timing distributions or call them an optimization. Native marks include monotonic elapsed and UTC timestamps. Detailed marks are buffered until a post-startup stats request or normal exit (128 pending-event ceiling). `-StartupTimingMode Legacy` (default) retains M0.7C's two synchronous timestamp writes for matched historical comparisons; `Buffered` buffers those too on new binaries, and `Off` disables native marks on both old/new artifacts. Use matched `Off` controls to measure observer effects with the same process-to-real-row boundary. Do not compare old binaries' `Buffered` mode as if they supported deferred output. Stderr writes have nonzero overhead; profiling and legacy-log removal are not production startup optimizations. Timing-enabled metadata/page marks retain attempts. The summarizer uses the first request, joins the unchanged real-row metric to process creation, and retains raw marks, visibility bounds, memory points and per-stage distributions. Missing instrumentation remains null, not zero. Post-driver CPU/private memory are collection points, not startup CPU/peak estimates.

```powershell
./benchmarks/startup-errors.ps1 -BenchmarkExecutable $exe `
  -DatabaseDirectory $db `
  -ResultsDirectory E:\dev\game-manager\benchmarks\results\startup-errors-new
node --test benchmarks/startup.test.mjs
```

Error probes use only newly owned temporary roots: empty application DB, absent/directory/corrupt/locked synthetic DB and blocked application data root. They verify structured errors, retry, preserved files and normal lifecycle. Mandatory migration, identity, WAL and foreign-key checks remain in place. Successful fixed-path restarts are covered by ordinary startup runs. Shell commit, navigation, row readiness, image readiness, window visibility and physical presentation remain separate. WPR/system traces require the corresponding Windows privilege; retain failed captures and do not alter security protections.

## M0.7C integrated validation

See [M0.7C verification](../docs/development/M0.7C_VERIFICATION.md) for retained artifacts/hashes, complete results, failed observations and the overall M0 gate assessment. Existing M0.2–M0.7B procedures/evidence below remain unchanged. Build and finish checks before timed collection; run one native driver at a time, with no competing builds/tests. Always choose new result/fixture roots.

The existing background runner now accepts an independent integrated driver:

```powershell
$db = 'E:\dev\game-manager\benchmarks\data\m0.2-2026-10-25-01'
$art = 'E:\dev\game-manager\benchmarks\data'
$exe = 'E:\dev\game-manager\benchmarks\results\m0.7c-candidate\game-manager.exe'

# Fine browser-local arrival + sequential first/deep native probes.
./benchmarks/background.ps1 -DatabaseDirectory $db -ArtworkDirectory $art `
  -ResultsDirectory E:\dev\game-manager\benchmarks\results\m0.7c-deep-example `
  -BenchmarkExecutable $exe -Sizes @(15000,25000) `
  -Cases @('idle','art2','write','mixed2') -Iterations 30 `
  -IntegratedScenario deep -RecoverySeconds 0

# Independent sustained workload, with a subsequent ordinary idle interval.
./benchmarks/background.ps1 -DatabaseDirectory $db -ArtworkDirectory $art `
  -ResultsDirectory E:\dev\game-manager\benchmarks\results\m0.7c-memory-example `
  -BenchmarkExecutable $exe -Sizes 15000 -Cases art2 -Iterations 5 `
  -IntegratedScenario combined -SustainedCycles 150 `
  -RecoverySeconds 60 -SampleMemory

# Process-start boundaries, actual native window and reused-profile warm repeats.
./benchmarks/background.ps1 -DatabaseDirectory $db -ArtworkDirectory $art `
  -ResultsDirectory E:\dev\game-manager\benchmarks\results\m0.7c-startup-example `
  -BenchmarkExecutable $exe -Sizes @(500,12320,15000,25000) `
  -Cases @('idle','art2') -Iterations 5 -IntegratedScenario startup `
  -ReadOnlyLibrary -Launches 5 -RecoverySeconds 0
```

`-IntegratedScenario` selects `deep`, `thumbnails`, `idle`, `scroll`, `transitions`, `artwork`, `artwork-saturation`, `complete`, `cancel`, `combined` or `startup`. Default sustained length is 60 cycles (allowed 1–300), default recovery 60 seconds (0–600). Use zero workers (`-Cases idle`) for isolated idle/scroll/transitions/completion/cancellation; use `art2` for artwork or combined browsing. Workload names are explicit: combined/completion use gentle 4,096-record mixed jobs, cancellation repeatedly starts/cancels active slow hashing, scrolling/artwork/combined visit 20 distributed settled rows/windows per cycle, transitions exercise search/presence/release sorting and Reset all. Every cycle has a one-second minimum including work/telemetry. `idle` with 300 cycles therefore supplies at least five minutes before recovery. These are new separated workloads, not substitutes for the older background driver's five-cycle evidence. Non-memory scenarios ignore `SustainedCycles`.

`scroll`/`artwork` rotate the same 20 evenly spaced windows and are reuse controls. In M0.7C, that artwork control reached only 73 native assets and did **not** cause eviction; it is retained, not called saturation. `artwork-saturation` and `combined` use varying positions `((cycle * 20 + step) * 7919) % (total - 10)` and assert actual evictions plus full 128-entry/256-file caches. The combined window sequence was corrected before collecting either artifact's combined runs. Preserve the unsuccessful fixed-window pressure attempt separately; do not pool different patterns as equivalent workloads.

`thumbnails` visits 40 distributed windows and records actual visible-image readiness timestamps, terminal placeholder statuses and job duty during optional saturated CPU/hash/write/mixed work. Compare `art1`, `art2`, `mixed0`, `mixed1`, `mixed2`, `priority` in both orders. Job restart checks keep work requested during each visit; measured duty still determines whether it was active. Readiness may precede the later polling observation. This supplemental probe is separate from the unchanged primary fairness workload, whose initial-thumbnail timer precedes job start.

The deep driver retains sequential metadata/page IPC and backend/pool samples for all/installed/not-installed first/tail pages, reversing offset order each round. It then observes 40 distributed windows/filter with an in-page MutationObserver. Expected generation, complete requested-window readiness, loaded target row and viewport intersection are required; a bounded native query checks the canonical ID after timing. Full-window DOM readiness, next/second rAF opportunities and a 50-ms timer quantization control are separate fields. The historical coarse probe waited only for the target row; do not conflate these endpoints. The timer control excludes CDP round trips and does not recreate historical coarse-polling results. Artwork-enabled UI visits exercise ordinary visible thumbnails; use the original ten-case background driver for continuous-artwork saturation/fairness. Neither rAF nor DOM readiness measures actual browser/compositor presentation.

`-CaptureIpc` adds a passive CDP Network timeline for local POSTs to `library_get_info`/`library_query`, with offsets, request/completion times and backend fields read from bounded response buffers. Tauri's invoke is immutable and is never replaced; command permissions/headers/production code are unchanged. No headers or complete response bodies are retained. This **protocol interval** excludes caller registration, possible preflight and JS parsing/callback delivery; full invoke durations remain separate direct probes. Buffers cap at 4 MiB total/512 KiB per response, with 2,048 retained events/readbacks and explicit missing-body/failure markers. It also records earlier first-target-row arrival independently of full-window readiness, with matching canonical IDs. Use separate instrumented output directories rather than pooling these samples with primary latency results. Per-window intersections distinguish sequential page protocol requests from the residual before/after them; they do not turn that residual into measured React CPU time. Geometry reads can force layout. Integrated scenarios reject the original driver's trace/shutdown switches; run those controls separately.

The separate `layout.ps1 -FontAudit -NoArtwork -Untraced -Repetitions 1` control records actual resolved title and placeholder faces through CDP after its timed transitions/scrolling finish. Supply the retained baseline/candidate `-Executables`, both release/stress sizes and a new results directory. No-artwork rendering ensures the placeholder glyph exists; the audit does not alter production CSS or required fonts. This is a font/geometry regression check, not a first-use Layout measurement.

Integrated reports preserve browser used/allocated heap (`totalSize`, not OS commit), DOM counters, bounded frontend queues, native cache/eviction/decode stats and job/admission snapshots. Append-only `.samples.jsonl` retains partial evidence on failure; `.memory-series.json` holds periodic complete age-validated process trees, with per-PID native/browser/renderer/GPU/utility roles. Process private bytes, working sets, JS heap and configured SQLite caches are different measures. Keep sampled memory runs separate from primary latency runs. A finite plateau or idle retention does not prove leak freedom. Existing benchmark observers/diagnostics mean the idle scenario is not an idle CPU/polling certification.

The final integrated driver installs its row observer once and passes target indices/criteria as data through `Runtime.callFunctionOn`, releasing its single remote global-object handle at exit. This avoids injecting a distinct full observer source string for every varied target. Early M0.7C fixed-window controls and primary deep captures used per-evaluation injection and remain identified in the verification report; do not use a transport change as evidence of an application memory improvement. All paired combined/historical/timeline controls use matching instrumentation within each comparison.

Startup uses native process creation, a bracketed `IsWindowVisible` observation, opt-in benchmark stderr timestamps (`GAME_MANAGER_STARTUP_TIMING`), shell commit, Library initialization/usable rows and visible thumbnail timestamps. `-ReadOnlyLibrary` is startup-only and disables the synthetic scheduler while opening the canonical fixture read-only. Launch one uses a fresh profile; later launches reuse it. All retain OS caches. The window sampler refreshes its cached handle before normal shutdown. No user data is cleared; no OS-cold or GPU-presentation claim is made.

For resource fairness, retain the original ten cases and repeat a second run with reversed `-Cases` order at both release/stress sizes. Compare IPC p50/p95/max/sample counts, initial/remounted thumbnail readiness, completed saturated-job throughput, active duty, worker waits/backoffs and memory. Use matching seed/storage/viewport/artifact and separate trace/memory runs. Worker defaults stay two/unthrottled unless controlled tradeoffs justify a separate change.

`-FixtureDirectory` selects a **new absolute disposable base**, placing generated job databases, copied synthetic originals, derived cache and profiles there. The runner still checks the canonical database supplied through `-DatabaseDirectory`; job-enabled runs query the generated writable fixture. Same-volume originals are hardlinked; cross-volume originals are copied, never written. A real HDD test must identify the physical drive and put actual fixtures there; an artificial delay is not an HDD. For read-only startup, supply canonical fixture copies on the drive being measured. Results can remain under this checkout's ignored `benchmarks/results/`. Explicit external fixture roots are not protected by this repository's Git ignore rules and must never be copied into tracked source or game installations.

`benchmarks/disk-load.ps1 -Root ABSOLUTE_NEW_BENCHMARKS_DATA_DISK_ROOT` creates one owned bounded file with write-through synthetic I/O. The root must be a new local `benchmarks/data/disk-NAME` path with no reparse ancestors. Defaults are 256 MiB maximum file length and 180 seconds; it overwrites only its own newly created payload. Creating `stop.request` inside that exact owned root requests a cooperative stop. Capture its JSON byte count/start/end/duration, verify overlap with the measured application phase, and preserve partial files/errors on failure. It never scans or deletes files. Use it only for an explicitly identified disposable disk-contention experiment, separate from ordinary storage controls.

Controlled SQL relationship distributions can be created without changing the reference seed:

```powershell
python benchmarks/distributions.py `
  E:\dev\game-manager\benchmarks\data\m0.2-2026-10-25-01\synthetic-v1-seed-42-15000.sqlite3 `
  E:\dev\game-manager\benchmarks\results\distribution-example\benchmarks\data\sparse.sqlite3 sparse
```

Original variants are `sparse` (10% installed, one copy/canonical game), `dense` (90%) and `fanout` (six extra memberships/canonical game, four copies/new membership). M0.7E adds `overlap`/`copies` as described above. The script refuses existing outputs/sidecars, requires complete benchmark input, uses SQLite backup, validates integrity/FKs and rechecks the source hash. A sidecar manifest identifies each derivative; its retained generator metadata does not make it the unchanged generator-v1 reference distribution. Run `library-bench query-diagnose --database ABSOLUTE_DERIVATIVE --iterations 10 --strategies finalists` with bundled SQLx SQLite; Python is used only for fixture creation. Exact count/full-summary equivalence remains mandatory. These distributions do not validate mostly unique artwork, every title script, real import behavior or arbitrary future provider cardinalities.

After collection, run `node --test benchmarks/arrival.test.mjs benchmarks/ipc-recorder.test.mjs benchmarks/layout.test.mjs` and `./benchmarks/process-memory.test.ps1`, in addition to the documented project checks/builds and ordinary production-isolation smoke. Lower-RAM hardware, physical scaling, OS-cold storage and actual presentation stay unverified unless explicitly measured; future procedures are in the verification report.

## M0.8 final gate disposition and bounded controls

See the [M0.8 gate disposition](../docs/development/M0.8_GATE_DISPOSITION.md) for the original-target matrix, historical failures, hardware boundaries, actual results and reopening policy. **Conditional closure is a recommendation pending user approval, not a technical pass or closed gate.** Keep M0.7B production queries and existing startup/default policies. Do not restart a speculative optimization program.

The focused acquisition tests run only under the existing synthetic benchmark feature and Rust test configuration:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --features benchmarks --locked acquisition_tests -- --nocapture --test-threads=1
```

Use serial bounded repeats, retaining every exit/log. They hold all four connections deliberately to reproduce the existing 250-ms guard, then verify zero-prefix failure, cancellation and subsequent successful jobs; another test runs concurrent actual reads and repeated writes. The pool's underlying timeout remains ten seconds. This reproduces a failure mechanism, not the unknown historical cause. Fixture roots are disposable; tracing is bounded test output. No capacity/deadline/policy change is made.

For **physical Windows display scaling**, first select 100%, 125% or 150% in Windows Settings on the monitor where the new native app launches. Build the locked benchmark executable before collection, then:

```powershell
./benchmarks/physical-scaling.ps1 `
  -DatabaseDirectory E:\dev\game-manager\benchmarks\data\m0.2-2026-10-25-01 `
  -BenchmarkExecutable E:\dev\game-manager\apps\desktop\src-tauri\target\release\game-manager.exe `
  -ExpectedDpi 120 `
  -ResultsDirectory E:\dev\game-manager\benchmarks\results\m0.8-scale-125-new
```

Use DPI 96/120/144 respectively, a new results root each time, and restore the original setting afterward. The runner uses only synthetic read-only DBs, isolated application/profile paths, source hashes and clean native shutdown. It executes the established query driver at 15k/25k, checks actual **GetDpiForWindow** against the expectation, then verifies natural WebView DPR, 38-CSS-pixel rows, bounded mounting, horizontal overflow and resolved Japanese-title/placeholder fonts. Screenshots are DOM captures, not compositor measurements. No device-metrics emulation is used; physical scaling must be established separately in Windows Settings. Existing emulated-DPR drivers remain separately labelled.

Reduced process memory is not 8–16-GB Windows validation. Fresh profiles/pool reopens are not OS-cold. rAF/LongTask/Layout/DOM readiness are not displayed FPS. Unavailable hardware/privileges leave a criterion unverified; reproducible low-RAM, compositor and cold-start procedures are in M0.8. Preserve failed writer roots and exact settings before retries. An unexpected recurrence warrants bounded acquisition-entry/outcome, occupancy and executor-heartbeat capture, not larger pools/timeouts.

## Original M0.2 database baseline

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

## M0.7A renderer layout investigation

Use the optimized `benchmarks` build and existing synthetic fixtures. See [the verification report](../docs/development/M0.7A_VERIFICATION.md) for the failed reproductions, first-use font evidence and exact retained artifacts. The original M0.6 prelude reproduces a cold Japanese font path when switching installed/not-installed back to all installations and release-date descending. Navigating to the title-sorted tail first warms this path. Report each fresh process's first transition separately from its warm repeats.

```powershell
./benchmarks/background.ps1 `
  -DatabaseDirectory E:\dev\game-manager\benchmarks\data\m0.2-2026-10-25-01 `
  -ArtworkDirectory E:\dev\game-manager\benchmarks\data `
  -ResultsDirectory E:\dev\game-manager\benchmarks\results\m0.7a-new-run `
  -Sizes @(15000,25000) -Cases mixed2 -Iterations 5 `
  -TraceSort -SortRepetitions 2 `
  -BenchmarkExecutable E:\dev\game-manager\benchmarks\results\m0.7a-title-artifact\game-manager.exe

node benchmarks/summarize-layout.mjs benchmarks/results/m0.7a-new-run
node --test benchmarks/layout.test.mjs
```

`-BenchmarkExecutable` selects an explicitly retained native test artifact; the runner records its SHA-256/configuration in `environment.json`. Alternate baseline/candidate order across fresh processes with new output directories. `-ClosedDiagnostics` hides diagnostics; `-Cases mixed0` disables thumbnail workers while retaining the same mixed job and procedural resize. Omit `-TraceSort` for matched untraced controls. Do not run tests/builds concurrently with collection. Source databases remain hash-checked; writable roots/profiles/artwork copies remain isolated and ignored.

`-LayoutVariants 'baseline,blockMain,gridStart,minHeight,fixedControls,blockControls,stableScrollbar,fixedPanel'` selects temporary CDP stylesheet experiments. Additional focused variants are `noLegibility`, `explicitCjk` (Yu Gothic UI), `explicitResolved` (the observed Noto Sans JP face), `warmLatin` and `warmCjk`. Warmups force a hidden test span's layout and record its cost/font separately; they are not shipped startup work. Experimental styles do not relax the production CSP. Multi-trial traces have `.sort-REPETITION-VARIANT.trace.json` suffixes; warm probes have separate `.warm-REPETITION-VARIANT.trace.json` files. Defaults retain the single original `.trace.json` filename.

The independent **deep-first exploratory control** uses the same loopback transport but visits End before each transition:

```powershell
./benchmarks/layout.ps1 `
  -ResultsDirectory E:\dev\game-manager\benchmarks\results\m0.7a-deep-control `
  -Executables @('E:\dev\game-manager\benchmarks\results\m0.7a-title-artifact\game-manager.exe') `
  -Repetitions 3
```

It accepts `-Sizes`, `-Variants`, `-ClosedDiagnostics`, `-NoArtwork` and `-Untraced`; fixes the emulated viewport at 1120×760/DPR 1; checks row IDs/heights/placeholders/reset/focus/End and records subsequent scroll frames separately. It is useful for order/geometry controls, not cold-path proof. Its focused traces use a 32-MiB stop-when-full buffer and 64-MiB stream cap. The background driver's original 16-MiB ring/64-MiB cap is retained; use focused capture and inspect start marks rather than assuming a whole-run ring retained every phase.

Per-trial frame arrays avoid slicing an already-full global ring. The summarizer selects the marked renderer thread, keeps Layout wall/thread CPU separate and does not add nested JavaScript spans to Layout. Missing CPU data stays unavailable. Frame distributions include automation/idle time; commit, rAF, long tasks and paint/presentation are different measures. Final process-memory points are outside frame capture. Native traces, not unit timing thresholds, establish performance evidence.

All three PowerShell runners now share `process-memory.ps1`: process creation times validate ancestry and protect against counting older orphan processes after PID reuse. Reports retain per-PID identities, creation times and private bytes. Run `./benchmarks/process-memory.test.ps1` for deterministic ancestry/missing-root checks and live serialization verification. Older aggregate-only points cannot be retroactively corrected; M0.7A retains an unexplained 718-MiB historical control point and a separate corrected-sampler repeat. Summed working sets can double-count shared pages, and point samples do not establish memory bounds.

## M0.7B SQLite query diagnostics

Build `library-bench` with the locked release/benchmark configuration. `query-diagnose` opens an existing synthetic database read-only and compares the historical predicate with six controlled query-shape candidates. It records actual SQL, bound-parameter descriptions, separate count/page/ID-selection plans, execution distributions/raw samples, pool acquisition, split bounded projection and `dbstat` table/index sizes. Every sample checks count and complete row equality; candidate order reverses each round after equal warmups. This diagnostic command is never exposed through IPC.

```powershell
./benchmarks/query.ps1 `
  -DatabaseDirectory E:\dev\game-manager\benchmarks\data\m0.2-2026-10-25-01 `
  -ResultsDirectory E:\dev\game-manager\benchmarks\results\m0.7b-diagnose-new `
  -Executables @('E:\dev\game-manager\apps\desktop\src-tauri\target\release\library-bench.exe') `
  -Diagnose -Strategies finalists -Passes 1 -Iterations 50

# Compare retained baseline/candidate CLI artifacts using the original 14 cases.
./benchmarks/query.ps1 `
  -DatabaseDirectory E:\dev\game-manager\benchmarks\data\m0.2-2026-10-25-01 `
  -ResultsDirectory E:\dev\game-manager\benchmarks\results\m0.7b-paired-new `
  -Executables @('E:\dev\game-manager\benchmarks\results\m0.7b-baseline\library-bench.exe', 'E:\dev\game-manager\benchmarks\results\m0.7b-candidate\library-bench.exe') `
  -Passes 2 -Iterations 50
```

The four default sizes are 500/12,320/15,000/25,000. `-Strategies finalists` compares the historical and selected ordered entry-set shapes; omit it to evaluate all seven strategies. Results record source hashes, binary hashes, hardware/build/cache conditions and reverse artifact order each pass. Source files are hash-checked after each invocation. Use new output directories; stop competing builds/tests before collection. OS caches remain warm. Execution timers include SQLx preparation/worker/decoding overhead; split selection/projection are additional diagnostic queries and cannot be added to the full-statement timer. Native IPC, criteria-to-visible commits, renderer frames, background duty and process memory still require the established frontend/background runners. Do not compare traced versus untraced or split-diagnostic versus ordinary page timings interchangeably.

`query-measure` retains its original timing fields and adds execution-minus-pool distributions, count/page pool waits and raw paired samples. The historical retained binary has only its original fields. See [M0.7B verification](../docs/development/M0.7B_VERIFICATION.md) for candidate selection, actual plans, correctness and native before/after evidence. No cache, schema migration or FTS behavior is implied by running diagnostics.

The existing native `-QueryScenarios` driver also records `deepWindows`: 20 distributed installed/title-descending windows and 20 not-installed/title-ascending windows, each waited through an actually loaded row intersecting the viewport. Durations use the WebView clock and include CDP/polling overhead; they are observed DOM arrival, not paint/presentation or physical input latency. Small libraries naturally reuse cached pages; larger distributed jumps exercise ordinary page eviction. Existing scenario counts, exact complements, sorts, End selection, rapid criteria checks and row/cache/request bounds remain in place. Use identical drivers/artwork/load settings for artifact comparisons.
