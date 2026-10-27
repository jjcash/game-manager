# Game Manager

A free and open-source, local-first Windows desktop game library, launcher, and integrated mod manager in early development. Large-library performance, safe mod operations, and a polished interface are core requirements.

The desktop application in `apps/desktop/` has a Rust-backed virtualized Details View with title search, installation/source/year filters and title/release sorting. Pages and caching remain bounded, with keyboard selection, loading/error/retry states and opt-in synthetic diagnostics. M0.5 adds a compact thumbnail column with a bounded Rust synthetic-artwork service. M0.6 adds isolated synthetic background jobs, progress/cancellation and measured SQL/artwork contention; normal libraries remain unseeded. Mods and Settings remain placeholders. See [project status](docs/PROJECT_STATUS.md) and [M0.4 verification](docs/development/M0.4_VERIFICATION.md) for checks, measurements and limitations. See [M0.5 artwork verification](docs/development/M0.5_VERIFICATION.md) for ownership, cache/decode limits, deterministic fixture generation/cleanup and native artwork measurements.

The [M0.1 verification report](docs/development/M0.1_VERIFICATION.md) records the environment, exact dependency versions, checks, file inventory, and limitations.

The [M0.2 verification report](docs/development/M0.2_VERIFICATION.md) records the SQLite schema, safety tests, dataset counts and actual database timings.

## Read the documentation

| Document                                                        | Purpose                                                       |
| --------------------------------------------------------------- | ------------------------------------------------------------- |
| [AGENTS](AGENTS.md)                                             | Permanent agent instructions, scope and safety                |
| [Project status](docs/PROJECT_STATUS.md)                        | Actual implementation status                                  |
| [Roadmap](docs/ROADMAP.md)                                      | M0–M5 gates and later work                                    |
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

Integration priority: Steam → GOG → Epic Games → Ubisoft Connect → EA App → Amazon Games → emulators (RomM, RetroArch, RetroAchievements) → Battle.net → itch.io → standalone executables. Manual entry and standalone launching remain MVP requirements.

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

`pnpm desktop:dev` starts Vite on `127.0.0.1:1420` and opens Tauri. The **Desktop connection** panel should show **Rust backend connected · Game Manager 0.1.0**. **Check again** repeats the real IPC call. Library displays SQLite data (an empty library on a fresh normal database); Mods and Settings remain placeholders. Browser-only previews show an actionable IPC error. No game files are accessed.

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

[CI](.github/workflows/ci.yml) runs one **Windows quality checks** job on `windows-2025` for pull requests targeting `main`, pushes to `main`, and manual runs. It checks TypeScript, ESLint, Prettier/rustfmt, frontend tests, Clippy with warnings denied, Rust tests, and the production frontend/native Tauri build. Tauri invokes `pnpm build` before native compilation; CI does not build the frontend separately.

CI pins Node.js 26.12.1, reads pnpm 12.15.0 from `package.json`, and pins Rust 1.99.1 for Windows MSVC with rustfmt and Clippy. It installs with frozen/locked dependencies, caches dependency downloads only, and cancels superseded runs. `.gitattributes` keeps text checkouts on LF for consistent formatting checks on Windows. Official actions are pinned to commit SHAs; permissions are read-only, checkout credentials are not retained, and no secrets, installer uploads, publishing, or deployment are configured.

**M0.1.1 awaits its first successful GitHub run.** Local evidence and the approval-dependent commit/push instructions are in the [CI verification report](docs/development/M0.1.1_CI_VERIFICATION.md). After the check appears on GitHub, configure the `main` branch rules to require **Windows quality checks** before merging; the workflow alone does not enforce branch protection.

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

Virtualization, imports, search/filtering, launching games, mod operations, integrations, jobs, cloud/mobile/Game Tools, and installer delivery remain unimplemented. M0 as a whole remains incomplete. M0.3 requires separate authorization.

## SQLite setup and synthetic benchmarks

The desktop initializes SQLite asynchronously in Rust before library reads become available. No SQLx CLI or external database server is needed: migrations are embedded in the executable. Normal storage defaults to `%LOCALAPPDATA%\io.github.game-manager.desktop\database\library.sqlite3`, resolved through Tauri's installed application path API. It is independent of the checkout. An optional absolute local `GAME_MANAGER_DATA_DIR` selects the initial installed data root; it does not move an existing database. Set it only before launching, for example to a new disposable development root:

```powershell
$env:GAME_MANAGER_DATA_DIR = 'E:\dev\game-manager\benchmarks\data\desktop-smoke-example'
pnpm --dir apps/desktop desktop:dev
Remove-Item Env:GAME_MANAGER_DATA_DIR
```

Without that override, the application creates an empty persistent library. It never seeds it. Foreign keys are enabled per connection, with a four-connection maximum, WAL, FULL synchronous durability, five-second busy timeout, and versioned/checksummed migrations. Initialization errors preserve the file and expose structured failure status; `app_get_status` remains a backend probe independently of database readiness. The current shell does not display database state; new commands are ready for the next library milestone. Startup diagnostics are written as local JSON errors, and IPC error messages omit private path/SQL details.

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

M0.4 retains scan-based literal LIKE after measuring it against FTS5 trigram, with existing indexed ordering and relation probes. No production FTS index or schema migration is added. See the verification report for measured costs, query plans, Unicode limits and the reasons to revisit FTS at later scale.

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

Artwork uses a separate two-job Rust service and two-request frontend scheduler; it never holds the SQL request gate. Only visible/overscan rows request 64×36 PNG thumbnails for the fixed 48×27 column. Original PNG/JPEG files stay in the controlled fixture root; React receives bounded image data through an ID-only command. Missing and damaged artwork are ordinary placeholder states. There is no filesystem URL scope or real-artwork acquisition.

Both result caches retain at most 128 entries; the frontend queue is capped at 32 jobs and derived disk storage at 256 files/4 MiB. Only explicit benchmark builds honor `GAME_MANAGER_BENCHMARK_ARTWORK`. Normal startup generates neither artwork nor library records. Sources, derived thumbnails and temporary writes are separate. See the [M0.5 report](docs/development/M0.5_VERIFICATION.md) for source/decode caps, invalidation, security tests, repeatable commands and limitations.

`benchmarks/frontend.ps1 -ArtworkDirectory E:\dev\game-manager\benchmarks\data -ArtworkScenarios ...` adds slow/rapid scrolling, repeated distant windows, visible-image checks, native cache/concurrency counters and process-memory time series, then repeats the M0.4 search/filter/sort matrix under artwork load. `library-bench query-measure --artwork ABSOLUTE_FIXTURE_ROOT ...` measures the unchanged SQL scenarios while two synthetic-artwork workers run. Fixture CLI generation and cleanup require an explicit disposable `benchmarks/data/artwork-NAME` root. No generated image or benchmark output belongs in Git. M0.6 requires separate authorization.

## Synthetic background-load experiments

M0.6 adds a Rust-owned, feature-gated synthetic scheduler with typed progress, cooperative cancellation, one active/two queued jobs and a shared two-worker CPU budget. Its writable database is independently generated under an explicit disposable benchmark root; normal application databases and canonical benchmark references are not modified. Normal native builds omit job handlers and controls. The library remains virtualized and uses its existing query/page/cache bounds.

See [M0.6 verification](docs/development/M0.6_VERIFICATION.md) for exact architecture, scheduling limits, fixture generation/cleanup, cancellation/recovery, native contention results and unresolved latency/frame findings. [Benchmark instructions](benchmarks/README.md#m06-bounded-synthetic-jobs) provide repeatable commands. No real import, provider, directory scanning or M0.7 work is included.
