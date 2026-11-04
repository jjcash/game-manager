# Game Manager

**M1.9 local artwork:** In **Manage selected game**, choose or replace one local PNG/JPEG for a manually created game, or clear its assignment. Rust keeps an independent original under the configured application data root at `database/artwork/originals/`; the selected file is never modified. Originals are durable user data: replacement, clearing and cache eviction preserve them. Only compact thumbnails reach the frontend. See [M1.9 verification](docs/development/M1.9_LOCAL_ARTWORK_VERIFICATION.md) for verification status and limits.

**Backups remain database-only.** Schema-v5 snapshots contain artwork metadata and assignments, but **exclude the image files**. Preserve `database/artwork/originals/` separately alongside database backups. Restore replaces assignments without restoring or deleting originals; unavailable or corrupt originals show placeholders. Generated files under `database/artwork/thumbnails/` are disposable and regenerate when their originals are available. No full-content backup or original cleanup is implemented.

A free and open-source, local-first Windows desktop game library, launcher, and integrated mod manager in early development. Large-library performance, safe mod operations, and a polished interface are core requirements.

The desktop application in `apps/desktop/` provides a persistent manual game library in the existing virtualized Details View. **Add game** creates an independent canonical game and manual membership, with optional installation metadata. Select a row and choose **Manage selected game** to save metadata overrides, add/update/remove manual memberships, and record/edit/remove installation metadata. Removal confirmations describe the affected records; no game files are accessed. A fresh normal database is empty, and changes survive restart. See [M1.1 verification](docs/development/M1.1_VERIFICATION.md) for evidence and limits. Mods remains a placeholder; Settings contains local library backup and restore.

**M1.4 offline library backup and restore is verified within its documented scope.** In Settings, **Create backupâ€¦** selects a new destination for a verified standalone SQLite snapshot. **Select backup to restoreâ€¦** validates a private copy before replacement confirmation. Confirmation closes the library and preserves its previous state; **Restart and finish** applies and verifies restoration before opening services. Restoration replaces records rather than merging them. Backups are unencrypted, include artwork assignments but exclude artwork image files and game/mod files, accept exact application schemas v2, v3, v4 and v5 (older schemas upgrade only on the restored active copy) and are limited to 1 GiB. Use a local drive with hard-link support; NTFS is the verified test filesystem. Recovery directories remain in the application database directory and are never automatically deleted. See [M1.4 verification](docs/development/M1.4_BACKUP_RESTORE_VERIFICATION.md) for lifecycle, native restart, interruption tests, screenshots and limits, and [M1.6 verification](docs/development/M1.6_LAUNCH_PROFILE_VERIFICATION.md) for historical v4 compatibility; [M1.9 verification](docs/development/M1.9_LOCAL_ARTWORK_VERIFICATION.md) covers current v5 compatibility.

**M1.5 installation grouping is locally verified.** It adds a compact **Organize installation groups** section to **Manage selected game**. Create or rename a group, select an installation and explicitly save its assignment, or choose **Ungrouped**. Groups belong to one canonical game and can span its memberships. Removing a group confirms that only organization is removed; installation records and files remain untouched. Duplicate names have independent IDs. M1.5 introduced application schema v3; frozen benchmarks stay v1. See [verification](docs/development/M1.5_INSTALLATION_GROUPING_VERIFICATION.md) and [ADR-010](docs/decisions/ADR-010-installation-grouping-and-launch-profile-identity.md).

**M1.6 adds persistent manual LaunchProfile configuration in schema v4.** Expand **LaunchProfiles** in **Manage selected game** to create, edit, duplicate or explicitly remove installation-bound user profiles. Store standalone executable/working-directory references and literal arguments as a JSON string array, and explicitly select or clear a default. References receive Windows lexical validation only; no game path is accessed or executed. Remove dependent profiles before removing installation/membership/game metadata. The editor supports at most 100 profiles per game. [M1.6 verification](docs/development/M1.6_LAUNCH_PROFILE_VERIFICATION.md) governs completion claims and limits. M1.8 adds the explicit Launch action described below.

**M1.8 adds deliberate standalone launching; verification status is recorded in the [M1.8 report](docs/development/M1.8_PRODUCTION_STANDALONE_LAUNCH_VERIFICATION.md).** In **Manage selected game**, choose a saved user-owned standalone LaunchProfile bound to a manual Windows installation, then activate **Launch**. An explicit supported default may be preselected; a sole profile is never an implicit default. One-time selection does not save a default. Only explicit `.exe` targets are supported, with literal saved arguments and an explicit or executable-parent working directory. Run Game Manager unelevated. Success means Windows created a process, not successful gameplay. The executable has your ordinary privileges and is not sandboxed. No launch occurs on startup, selection, editing, default changes or restore. Provider-associated profiles remain saved metadata but cannot launch here.

[M1.2 Steam discovery contract and fixtures](docs/development/M1.2_STEAM_DISCOVERY_VERIFICATION.md) investigate credentials-free local discovery without adding a production Steam reader or importer. Offline tests validate bounded manifest/folder parsing and conservative evidence rules: an installed or cached AppID does not prove account ownership. Accepted [ADR-008](docs/decisions/ADR-008-steam-local-discovery-threat-model-b.md) records the narrowly scoped model B policy. The [M1.3C isolated Rust feasibility candidate](docs/development/M1.3C_WINDOWS_READER_FEASIBILITY.md) did not pass its native security gate: required symlink coverage is unavailable and post-approval root replacement remains unverified. The recommendation is to defer Steam local discovery. No safe reader or production Steam capability is verified; M1.3 remains incomplete. Full uninstalled/account-owned library coverage is not claimed.

The [M0.1 verification report](docs/development/M0.1_VERIFICATION.md) records the environment, exact dependency versions, checks, file inventory, and limitations.

The [M0.2 verification report](docs/development/M0.2_VERIFICATION.md) records the SQLite schema, safety tests, dataset counts and actual database timings.

## Read the documentation

**M0 is conditionally closed with accepted exceptions E1â€“E5; M1.1 is the verified manual-library foundation.** The user confirmed M0.8 CI and merge on 2026-10-28 and accepted [ADR-007](docs/decisions/ADR-007-conditional-m0-performance-gate.md). Original targets and the [M0.8 measurements](docs/development/M0.8_GATE_DISPOSITION.md) remain unchanged. Storefront importing/discovery/launching and mod work remain future scope; manual standalone creation is the M1.8 slice.

| Document                                                        | Purpose                                                       |
| --------------------------------------------------------------- | ------------------------------------------------------------- |
| [AGENTS](AGENTS.md)                                             | Permanent agent instructions, scope and safety                |
| [Project status](docs/PROJECT_STATUS.md)                        | Actual implementation status                                  |
| [Roadmap](docs/ROADMAP.md)                                      | M0â€“M5 gates and later work                                  |
| [PRD](docs/specifications/PRD.md)                               | Full product baseline and confirmed priorities                |
| [TAS](docs/specifications/TAS.md)                               | Modules, interfaces, workflows and performance budgets        |
| [DSS](docs/specifications/DSS.md)                               | Detailed identity/schema/storage design and reference SQL     |
| [MDRS](docs/specifications/MDRS.md)                             | Profiles, ownership, deployment, journaling and recovery      |
| [ADRs](docs/decisions/README.md)                                | Accepted architecture and software-license decisions          |
| [Sources and open decisions](docs/SOURCE_AND_OPEN_DECISIONS.md) | Recovered evidence, corrections and genuine remaining choices |

## Agreed direction

- Tauri 2, React, TypeScript, Rust and SQLite; internal modular architecture and offline core.
- Windows installed application only; portable distribution dropped.
- Library + Mod Manager MVP with limited tested adapters, manual launching, and configurable installation grouping. No Playnite migration requirement.
- Virtualized Details View first, also a virtualized cover grid; 12,320-game reference, 15,000-game release benchmark, 25,000-game stress test.
- Managed copy deployment first, game-specific and launch-time activation, links later, virtual filesystem later. Detect existing mods read-only; adoption is explicit.
- One polished dark theme first. Game Tools, external-first trainers, Android companion, richer themes, dashboards and extensions are later scope.

Integration priority: Steam â†’ GOG â†’ Epic Games â†’ Ubisoft Connect â†’ EA App â†’ Amazon Games â†’ emulators (RomM, RetroArch, RetroAchievements) â†’ Battle.net â†’ itch.io â†’ standalone executables. Manual entry and standalone launching remain MVP requirements.

Mod targets: Half-Life 2, classic Doom II (not the reboot), Stardew Valley, Minecraft, Cyberpunk 2077, Skyrim, Baldur's Gate 3 and Terraria. First candidates: Half-Life 2, classic Doom II and Stardew Valley. These are intended targets, not working compatibility claims.

## Windows development setup

Install the [Tauri Windows prerequisites](https://v2.tauri.app/start/prerequisites/): Microsoft C++ Build Tools with **Desktop development with C++** and a Windows SDK, Microsoft Edge WebView2 Runtime, and stable Rust with the `x86_64-pc-windows-msvc` host. Install Node.js (22.22.2+, 24.15+, or 26+; prefer a current LTS), and pnpm 12.15.0. These versions also satisfy the unit-test environment. The repository does not depend on its checkout path.

```powershell
node --version
pnpm --version
rustc --version
cargo --version
rustup show
rustup component add rustfmt clippy

cd apps/desktop
pnpm install --frozen-lockfile
pnpm desktop:dev
```

Open a new terminal after installing prerequisites. If tools are installed but missing from an older terminal's PATH, this session-only adjustment covers the default locations used on the development machine:

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:LOCALAPPDATA\pnpm\bin;$env:Path"
```

`pnpm desktop:dev` starts Vite on `127.0.0.1:1420` and opens Tauri. The **Desktop connection** panel should show **Rust backend connected Â· Game Manager 0.1.0**. **Check again** repeats the real IPC call. Library displays SQLite data (an empty library on a fresh normal database); Settings provides offline library backup/restore and Mods remains a placeholder. Browser-only previews show an actionable IPC error. No game files are accessed.

## Development commands

Run these from `apps/desktop/`:

| Command              | Purpose                                                           |
| -------------------- | ----------------------------------------------------------------- |
| `pnpm desktop:dev`   | Launch the desktop development app with reload                    |
| `pnpm dev`           | Frontend-only Vite server; Rust IPC unavailable                   |
| `pnpm build`         | Type-check frontend/configuration and build frontend assets       |
| `pnpm desktop:build` | Build the release desktop executable without installer bundling   |
| `pnpm preview`       | Preview built frontend assets; Rust IPC unavailable               |
| `pnpm typecheck`     | TypeScript checks, including Vite/Vitest configuration            |
| `pnpm lint`          | ESLint; warnings fail the check                                   |
| `pnpm lint:rust`     | Clippy across Rust targets; warnings fail the check               |
| `pnpm format`        | Apply Prettier and rustfmt                                        |
| `pnpm format:check`  | Check Prettier and rustfmt                                        |
| `pnpm test`          | Frontend IPC, shell and virtualized library tests                 |
| `pnpm test:rust`     | Rust contract, database, migrations and synthetic generator tests |

The release executable is `apps/desktop/src-tauri/target/release/game-manager.exe`. This is a development verification artifact, not a portable distribution or installed release. Installer format, signing, updating, and the supported Windows matrix remain M5 work. First builds download Rust dependencies and can take several minutes. pnpm and Cargo lockfiles are included; generated assets and build outputs are ignored.

## Continuous integration

[CI](.github/workflows/ci.yml) retains **Windows quality checks** on `windows-2025` for pull requests targeting `main`, pushes to `main`, and manual runs. It checks TypeScript, ESLint, Prettier/rustfmt, frontend tests, all-target Clippy with warnings denied, ordinary Rust tests and documentation tests, and the production frontend/native Tauri build. It compiles the `standalone_launch` and `standalone_launch_production` targets without executing their native fixtures on the elevated hosted runner. Tauri invokes `pnpm build` before native compilation.

CI pins Node.js 26.12.1, reads pnpm 12.15.0 from `package.json`, and pins Rust 1.99.1 for Windows MSVC with rustfmt and Clippy. It installs with frozen/locked dependencies, caches hosted dependency downloads only, and cancels superseded runs. `.gitattributes` keeps text checkouts on LF for consistent formatting checks on Windows. Official actions are pinned to commit SHAs; permissions are read-only and checkout credentials are not retained.

The [M1.7 CI and native verification policy](docs/development/M1.7_CI_AND_NATIVE_VERIFICATION_POLICY.md) requires all 12 existing native security tests and all nine production native tests locally on an unelevated Windows desktop for milestone and release acceptance, with evidence tied to the intended source tree. GitHub Actions does not continuously execute those tests or enforce this manual gate. The existing ruleset requires **Windows quality checks**. `pnpm test:rust` remains the full local Rust suite.

## Source layout and boundaries

```text
apps/desktop/
  src/                 React/TypeScript presentation and temporary state
    components/        Shell icons and backend-connection panel
    features/library/  Virtualized Details View, bounded cache, UI state and diagnostics
    ipc/               Narrow IPC wrapper, runtime validation, shared fixture
  src-tauri/
    src/               Rust entry points, storage service and narrow read commands
    migrations/        Versioned SQLx SQLite migrations
    tests/             Disposable database integration tests
    capabilities/      Main-window command permission
    icons/             Template desktop icons; final branding deferred
  package.json         Development commands and frontend dependencies
  pnpm-lock.yaml       JavaScript dependency resolution
  src-tauri/Cargo.lock Rust dependency resolution
docs/                  Requirements, accepted decisions, roadmap, actual status
```

Rust owns backend logic and SQLite. React receives a validated `{ appName, version, backend }` status through the unchanged `app_get_status` command; a shared fixture is checked by Rust and TypeScript tests. The local main window can also call narrow, read-only `database_get_status` and `library_get_page` commands. There is no generic SQL, filesystem, shell, opener, or remote-origin permission. Release CSP allows local assets and IPC; development CSP additionally permits Vite reload.

The virtualized Details View, search/filter/sort, bounded synthetic artwork and benchmark-only jobs are implemented within M0. Real imports, launching games, mod operations, storefront integrations, cloud/mobile/Game Tools and installer delivery remain unimplemented. The wider M0 performance gate remains open.

## SQLite setup and synthetic benchmarks

The desktop initializes SQLite asynchronously in Rust before library reads become available. No SQLx CLI or external database server is needed: migrations are embedded in the executable. Normal storage defaults to `%LOCALAPPDATA%\io.github.game-manager.desktop\database\library.sqlite3`, resolved through Tauri's installed application path API. It is independent of the checkout. An optional absolute local `GAME_MANAGER_DATA_DIR` selects the initial installed data root; it does not move an existing database. Set it only before launching, for example to a new disposable development root:

```powershell
$env:GAME_MANAGER_DATA_DIR = 'E:\dev\game-manager\benchmarks\data\desktop-smoke-example'
pnpm --dir apps/desktop desktop:dev
Remove-Item Env:GAME_MANAGER_DATA_DIR
```

Without that override, the application creates an empty persistent library. It never seeds it. Foreign keys are enabled per connection, with a four-connection maximum, WAL, FULL synchronous durability, five-second busy timeout, and versioned/checksummed migrations. Initialization errors preserve the file and expose structured failure status; `app_get_status` remains a backend probe independently of database readiness. The current Library view displays database readiness and structured errors while preserving the independent backend probe. Startup diagnostics are written as local JSON errors, and IPC error messages omit private path/SQL details.

From the repository root, create all six synthetic sizes in a **new**, explicitly configured benchmark directory:

```powershell
./benchmarks/run.ps1 -DatabaseDirectory E:\dev\game-manager\benchmarks\data\run-01 -Seed 42 -Iterations 50
```

For a single dataset:

```powershell
cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --bin library-bench --features benchmarks --release --locked
./apps/desktop/src-tauri/target/release/library-bench.exe generate --database E:\dev\game-manager\benchmarks\data\example.sqlite3 --games 15000 --seed 42
./apps/desktop/src-tauri/target/release/library-bench.exe measure --database E:\dev\game-manager\benchmarks\data\example.sqlite3 --iterations 50
```

The generator supports 500, 2,000, 5,000, 12,320, 15,000 and 25,000 canonical games. It requires a new file, refuses existing databases/sidecars, and has no overwrite mode. A database purpose marker separates benchmark and application libraries. Repeated generation uses a new path; measurement reuses an existing complete benchmark. No real accounts, libraries, game files, or artwork are accessed. Generated databases/results are ignored by Git. See [benchmark methodology](benchmarks/README.md) and [M0.2 verification](docs/development/M0.2_VERIFICATION.md) for schema simplifications, exact counts, checks and actual database timings.

## Virtualized library and frontend benchmarks

The Details View uses 38 CSS-pixel rows and eight rows of overscan per side. Only viewport pages are requested: 100 rows per page, six retained pages, and one native library request at a time. Arrow keys, Home/End and Page Up/Down navigate results. Selection and position survive navigation within the session; criteria edits clear selection, reset scrolling and discard unrelated cached pages. Refresh starts a new generation.

Title search is a literal contiguous substring with a 150 ms debounce. Leading/trailing whitespace is trimmed; `%`, `_`, quotes and backslash are data. ASCII matching is case-insensitive; non-ASCII code points match exactly, without normalization or accent folding. Filters compose installation presence, a source membership and inclusive release years. Installation presence applies to any copy of the canonical game, including a copy from another source. Source/copy summaries still describe the whole game. Unknown releases are excluded by year bounds; release ascending puts NULLs first, descending last. Title/release ordering and ID tie-breakers run in SQLite in both directions. Clear search and Reset all are available; empty-library and no-match states are distinct.

M0.4 retains scan-based literal LIKE after measuring it against FTS5 trigram. M0.7B uses present installation entry-ID sets and ordered canonical membership using the existing indexes; counts and bounded windows share the same predicates. No count cache, production FTS index or schema migration is added. See [M0.7B verification](docs/development/M0.7B_VERIFICATION.md) for controlled query/IPC/UI measurements and [query diagnostics](benchmarks/README.md#m07b-sqlite-query-diagnostics) for commands. Search matching and pagination semantics remain unchanged.

[M0.7E verification](docs/development/M0.7E_VERIFICATION.md) retains those production queries after relationship/deep-page cost-benefit comparisons. Ordinary 15,000-game gains are modest or inconsistent, sparse and native release-scale cases can regress, and larger intermediate/extreme gains do not justify a global change or migration. Benchmark-only comparisons and two intermediate fixture recipes are added; arbitrary jumps, exact canonical summaries and worker/cache limits remain intact. One synthetic writer-acquisition timeout is preserved with a safe zero-item prefix and successful unchanged-setting repeat. **Accept the documented query limitation; the wider M0 gate remains open.** Further startup optimization is deliberately deferred. See [the reproduction procedures](benchmarks/README.md#m07e-relationship-and-deep-page-investigation).

Normal startup never seeds synthetic data. To browse a complete M0.2 benchmark database, opt into the development-only Rust `benchmarks` feature and select an **existing absolute path**:

```powershell
cd apps/desktop
$env:GAME_MANAGER_BENCHMARK_DB = 'E:\dev\game-manager\benchmarks\data\m0.2-2026-10-25-01\synthetic-v1-seed-42-15000.sqlite3'
pnpm desktop:dev --features benchmarks
# Optimized, bundled frontend/native benchmark build:
pnpm tauri build --no-bundle --features benchmarks -- --locked
./src-tauri/target/release/game-manager.exe
```

Choose any generated size: 500, 2,000, 5,000, 12,320, 15,000 or 25,000. Close and restart the application after changing the variable. Benchmark viewing opens SQLite **read-only**, verifies purpose, generator metadata, count and exact migration history, and never migrates the selected file. Missing/inaccessible/incompatible data produces an error; it does not fall back to a normal library. Ordinary builds without `benchmarks` ignore this variable. Remove it from the current terminal with `Remove-Item Env:GAME_MANAGER_BENCHMARK_DB` when finished.

Development diagnostics show dataset source/size, mounted/visible rows, active/cached pages, initial view readiness, Rust query and frontend request durations. Frame sampling starts only when requested and retains at most 1,024 samples. React Profiler samples are available in development builds; normal optimized React builds do not provide them. Heap size can be unavailable and is separate from application process memory. Measurements are not FPS estimates or controlled cold-start results.

For repeatable native scenarios, see [the frontend benchmark runner](benchmarks/README.md#native-frontend-scenarios-m03). Remote debugging is a manually enabled local testing facility, not application configuration.

## License

Game Manager's original software code is distributed under the GNU General Public License as published by the Free Software Foundation, either version 3 of the License or, at the recipient's option, any later version (**SPDX: `GPL-3.0-or-later`**). See [LICENSE](LICENSE) for the complete license text and [ADR-006](docs/decisions/ADR-006-software-license.md) for the accepted decision.

Game Manager is distributed without any warranty, including the implied warranties of merchantability or fitness for a particular purpose, as provided in the license.

This project license does not automatically relicense third-party game artwork, mods, ROMs, trainers, maps, guides, dependencies, or other external content. Those materials retain their respective licenses, copyrights, and usage restrictions; managing or launching them does not grant redistribution rights.

## Synthetic artwork prototype (M0.5)

Artwork uses a separate two-job Rust service and two-request frontend scheduler; it never holds the SQL request gate. Only visible/overscan rows request 64Ã—36 PNG thumbnails for the fixed 48Ã—27 column. Original PNG/JPEG files stay in the controlled fixture root; React receives bounded image data through an ID-only command. Missing and damaged artwork are ordinary placeholder states. This synthetic path has no filesystem URL scope or real-artwork acquisition; M1.9 adds the separate manual-library import described above.

Both result caches retain at most 128 entries; the frontend queue is capped at 32 jobs and derived disk storage at 256 files/4 MiB. Only explicit benchmark builds honor `GAME_MANAGER_BENCHMARK_ARTWORK`. Normal startup generates neither artwork nor library records. Sources, derived thumbnails and temporary writes are separate. See the [M0.5 report](docs/development/M0.5_VERIFICATION.md) for source/decode caps, invalidation, security tests, repeatable commands and limitations.

`benchmarks/frontend.ps1 -ArtworkDirectory E:\dev\game-manager\benchmarks\data -ArtworkScenarios ...` adds slow/rapid scrolling, repeated distant windows, visible-image checks, native cache/concurrency counters and process-memory time series, then repeats the M0.4 search/filter/sort matrix under artwork load. `library-bench query-measure --artwork ABSOLUTE_FIXTURE_ROOT ...` measures the unchanged SQL scenarios while two synthetic-artwork workers run. Fixture CLI generation and cleanup require an explicit disposable `benchmarks/data/artwork-NAME` root. No generated image or benchmark output belongs in Git. The subsequently completed M0.6?M0.7C investigations and remaining gates are recorded below.

## Synthetic background-load experiments

M0.6 adds a Rust-owned, feature-gated synthetic scheduler with typed progress, cooperative cancellation, one active/two queued jobs and a shared two-worker CPU budget. Its writable database is independently generated under an explicit disposable benchmark root; normal application databases and canonical benchmark references are not modified. Normal native builds omit job handlers and controls. The library remains virtualized and uses its existing query/page/cache bounds.

See [M0.6 verification](docs/development/M0.6_VERIFICATION.md) for exact architecture, scheduling limits, fixture generation/cleanup, cancellation/recovery, native contention results and unresolved latency/frame findings. [Benchmark instructions](benchmarks/README.md#m06-bounded-synthetic-jobs) provide repeatable commands. No real import, provider, directory scanning or M0.7 work is included.

## Renderer layout profiling (M0.7A)

The focused sort benchmark now supports retained artifact selection, repeated transitions and temporary CSS/font experiments. First-use and warm transitions must be reported separately: visiting the title-sorted tail first can warm Japanese font fallback and mask the reproduced stall. Game titles name optional local Noto Sans JP before generic fallbacks, preserving the observed title face without bundling or downloading a font; interface and placeholder fonts retain their existing stack. See [M0.7A verification](docs/development/M0.7A_VERIFICATION.md) for evidence, comparisons and remaining limitations, and [renderer benchmark commands](benchmarks/README.md#m07a-renderer-layout-investigation). M0.7B and M0.7C investigations are recorded below; later engineering work requires separate authorization.

## SQLite latency investigation (M0.7B)

Measured present-entry sets reduce broad installed counts and filter latency without changing schema, search semantics, artwork or worker defaults. Alternating SQL/native comparisons retain improvements and regressions: stress deep-page retrieval still exceeds the provisional 50-ms goal, some not-installed/write-load paths are slower, and paired memory cycles do not establish stable process memory. See [M0.7B verification](docs/development/M0.7B_VERIFICATION.md) for plans, p50/p95/outliers, correctness checks, native regressions and the M0.7C recommendation. The wider M0 gate remains open; no commit/push or next milestone is performed automatically.

## Integrated performance validation (M0.7C)

The [M0.7C verification report](docs/development/M0.7C_VERIFICATION.md) consolidates deep-page SQL/IPC/row-arrival timing, reversed resource-fairness controls, 100â€“150-cycle memory workloads with idle recovery, whole-process startup, actual HDD/contended-disk tests and font/native regressions. [Benchmark procedures](benchmarks/README.md#m07c-integrated-validation) describe the new independent drivers and timing limits. Historical reports, seed-42 reference fixtures and retained artifact hashes are preserved.

The investigation is complete within its tested synthetic scope; **M0 remains open**. Deep-page/load tails, relation-heavy fixtures and warm startup miss provisional targets. Captured mixed memory stabilizes late and substantially recovers at idle, without proving leak freedom. Lower-RAM hardware, other physical scaling, OS-cold storage and actual displayed-frame performance remain unverified. M0.7B queries, optional title fonts, two artwork workers and default admission are retained. No real importer or later milestone is started.

## Whole-process startup investigation (M0.7D)

M0.7D adds opt-in native startup phases and separate window-construction profiling to the retained process-launch runner, preserving real database-backed row readiness. See [verification](docs/development/M0.7D_VERIFICATION.md) and [startup commands](benchmarks/README.md#m07d-whole-process-startup-investigation). Ordinary timing runs and profiling controls remain separate; fresh WebView profiles retain OS caches. Synchronous window/WebView construction dominates the measured aggregate; no production optimization is adopted. Final release-scale timing-off medians remain above one second, and the wider M0 gate stays open. The subsequently authorized M0.7E investigation addresses query cost; further startup optimization is deferred.
