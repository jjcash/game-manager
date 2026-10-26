# Project Status

Last verification: 2026-10-26. See [M0.1 evidence](development/M0.1_VERIFICATION.md), [M0.1.1 CI verification](development/M0.1.1_CI_VERIFICATION.md), [M0.2 database evidence](development/M0.2_VERIFICATION.md), [M0.3 frontend evidence](development/M0.3_VERIFICATION.md), [M0.4 search/filter/sort verification](development/M0.4_VERIFICATION.md), and [M0.5 artwork verification](development/M0.5_VERIFICATION.md).

## Actual state

**M0.1 through M0.5 are complete within their verified scope.** The Rust/SQLite-backed virtualized Details View supports search/filter/sort and a compact synthetic-thumbnail column with separate bounded artwork admission, decoding and caches. All eight final UI runs passed across 500, 12,320, 15,000 and 25,000 games, plus cache-saturation IPC/SQL probes and browsing/profile controls. M0 remains incomplete: background-load simulation, HDD/lower-memory, controlled process startup and longer-session validation remain pending. Stress-count costs, artwork contention and occasional long frames remain measured limitations. M0.6 has not started.

Historical prototype checkmarks were accidental and explicitly withdrawn by the user. They remain invalid; the progress below comes from the current implementation and checks. Specifications and ADRs retain their planning baseline; this file governs current completion claims.

| Area                                                     | Current status                                                                                                       |
| -------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| PRD                                                      | Original detailed baseline recovered and reconciled with later decisions                                             |
| TAS                                                      | Accepted architectural baseline recovered; implementation sketches still need validation                             |
| DSS                                                      | Original design retained; initial four-entity prototype translated into tested migration version 1                   |
| MDRS                                                     | Original detailed protocol recovered; architectural safety direction accepted, protocol remains proposed             |
| Repository ADRs                                          | ADR-001–005 reconciled; historical labels mapped; ADR-006 accepts GPL-3.0-or-later                                   |
| Software license                                         | GPL-3.0-or-later accepted; root LICENSE, manifest metadata and first-party SPDX notices added                        |
| Desktop app and UI                                       | Dark Tauri shell and virtualized Details View with search/filter/sort/thumbnails; Mods/Settings remain placeholders             |
| Frontend-backend communication                           | Original probes/page commands retained; typed criteria/count/window IPC and runtime validation verified              |
| Development quality checks                               | Frozen install, TypeScript, lint, formatting, 29 frontend/32 Rust tests and production frontend/native builds passed |
| SQLite database/migrations                               | M0.2 complete: SQLx/SQLite, versioned migration, purpose separation, WAL/FK bounded pool, status/count/page services |
| Details View/grid and artwork pipeline                   | M0.5 compact synthetic thumbnails, bounded jobs/queue/caches verified; cover grid/providers not implemented                        |
| M0 prototype and benchmarks                              | Shell/SQLite/Details View/query prototype complete; broader M0 gate and stress limitations remain open               |
| Store imports, launching and activity                    | Not implemented                                                                                                      |
| Mod adapters, deployment and recovery                    | Not implemented or tested                                                                                            |
| Continuous integration                                   | M0.1.1 workflow valid; all equivalent local checks passed; first successful GitHub run still required                |
| Installer, updater and release pipeline                  | Not established                                                                                                      |
| Game Tools, companion, dashboards, extension marketplace | Deferred                                                                                                             |

## M0.1 implementation and evidence

The desktop package is under `apps/desktop/`, with presentation in `src/` and Rust/Tauri code in `src-tauri/`. The shell has one dark theme, accessible navigation, clearly labeled placeholders, a connection status panel and error/retry handling. At M0.1 the only exposed command returned application identity/version/backend; M0.2 extends the backend as described below while preserving that probe's behavior.

`pnpm install --frozen-lockfile`, frontend build/typecheck/lint/tests, Rust tests, formatting checks, and Clippy passed. `pnpm desktop:dev` and `pnpm desktop:build` built debug and optimized release executables. Visual and accessibility inspection of both native windows showed **Rust backend connected · Game Manager 0.1.0**. The release executable loaded bundled assets at `http://tauri.localhost/` with Vite stopped. Native pointer navigation was blocked by an automation target/overlay error; navigation and retry behavior passed DOM unit tests. No unperformed native interaction check is claimed.

No prerequisite installation was needed. The inherited terminal PATH needed a session-only adjustment for existing Rust and pnpm installations. The README contains actual setup/build/check commands, and the verification report lists exact dependency versions and source files. Build outputs/dependencies are ignored; pnpm and Cargo lockfiles are retained. Existing files were already untracked; no changes were staged, committed, or pushed.

The earlier documentation reconciliation remains the product/architecture baseline. ADR-001 through ADR-005 retain their accepted decisions; the later software-license decision is recorded in ADR-006.

## Software-license adoption

The user selected **GPL-3.0-or-later** for original Game Manager software code: GNU GPL version 3 or, at the recipient's option, any later version. See [LICENSE](../LICENSE) and [ADR-006](decisions/ADR-006-software-license.md). Both application manifests carry the exact SPDX identifier, and authored UI/IPC and Rust source files carry SPDX headers. Third-party material retains its own terms; this is not a dependency-license compatibility audit or release-compliance certification. The copyright-holder name remains for the user to confirm. No application behavior, dependency versions, or M0.2 work was changed.

License-text hash/completeness checks, both manifest metadata checks, local Markdown links, frozen dependency installation, Prettier/rustfmt, TypeScript, ESLint, and 8 frontend tests passed. LICENSE is unchanged from matching GNU Emacs/Coreutils repository copies; the direct GNU website download was unavailable. ADR-006 records the source/hash, exact checks, and that limitation.

## M0.1.1 CI foundation

The Windows-only workflow validates pull requests targeting `main`, pushes to `main`, and manual runs. It reuses the existing desktop quality scripts and Tauri's production build hook. Node.js 26.12.1, pnpm 12.15.0 from the package-manager declaration, Rust 1.99.1, and official action commit SHAs are pinned. Dependency-download caches exclude native build outputs and installers; permissions are read-only and no publishing or deployment is configured. The existing `.gitattributes` now enforces LF text checkouts; disposable checkout checks verified LF with Windows Git conversion enabled and unchanged binary icon bytes.

All equivalent local checks passed: frozen pnpm installation, TypeScript, ESLint, Prettier/rustfmt, 8 frontend tests, release frontend/native Windows build, Clippy with warnings denied, and 1 Rust test. Workflow structure/expressions passed actionlint v1.7.12. A first release-build attempt was blocked by the earlier running bootstrap executable; after closing that window normally, the build passed. Local results and exact commands are recorded in the [CI verification report](development/M0.1.1_CI_VERIFICATION.md). **M0.1.1 is not complete until the workflow passes on GitHub.** No commit or push was authorized or performed for this task, so no run of the new workflow is claimed. No application code, dependency lockfile, architecture decision, or M0.2 scope was changed.

## M0.2 implementation and evidence

Rust now initializes a local SQLx/SQLite database asynchronously, with embedded migration version 1, per-connection foreign keys, a four-connection maximum and WAL/FULL durability. Canonical games, source memberships and installation copies have separate stable identities, constraints and measured indexes. The local window has narrow read-only database-status/page commands; no raw SQL or arbitrary path execution is exposed. Normal application data is never automatically seeded. Explicit benchmark paths, purpose markers, create-new reservation and preserved failure states protect normal/existing files.

All six synthetic sizes passed deterministic-generation, exact relational-count, integrity and bounded-query verification. The 15,000-game fixture has 17,143 entries and 10,233 copies; 25,000 games have 28,572 entries and 17,054 copies. Release generation took 1,850.885 ms and 4,846.899 ms respectively on the Ryzen 5 4500/64 GB reference system's E: NVMe SSD. Across 36 title/release page scenarios, each with 50 warm iterations and 100 summaries, the largest warm p95 was 18.521 ms. These are database-only, OS-cache-retained measurements; no scrolling or full-app target is established.

Eight existing frontend tests, six Rust unit tests and ten disposable-database integration tests passed, as did TypeScript/build, ESLint, Prettier/rustfmt, all-feature warnings-denied Clippy, frozen/locked dependency checks and the production native build. The rebuilt native shell showed the original successful Rust IPC connection. A fresh isolated application root contained an empty, healthy version-1 WAL database; real user/game data was untouched. No changes were staged, committed or pushed. Schema simplifications, exact counts, all timing scenarios, earlier failed attempts, source inventory and limits are recorded in [M0.2 verification](development/M0.2_VERIFICATION.md).

## M0.3 implementation and evidence

The Details View queries 100-row windows from the existing Rust service, retains at most six page entries after pruning, mounts only viewport rows plus eight overscan rows per side, and serializes native page invocations. Stable canonical IDs preserve selection across eviction; keyboard navigation and in-session return position are tested. A native test exposed an immediate-navigation scroll-event race, now fixed by capturing the DOM offset during cleanup and protected by a regression test. Refresh generations and cancellation discard obsolete responses. Normal libraries remain unseeded; explicit benchmark builds open complete synthetic databases read-only without migration.

All 19 frontend and 18 Rust tests passed, as did frozen installation, TypeScript, ESLint, Prettier/rustfmt, warnings-denied Clippy, frontend production build and native Tauri builds. Eight final release scenarios passed (two process runs each at 500, 12,320, 15,000 and 25,000 games), with actual loaded end rows, selection, return navigation, wheel/rapid/distributed scrolling and emulated viewport/scaling. Maximum mounted rows were 26; retained pages were at most six; one page invocation was active. Initial usable Library commits took 113.9-179.6 ms from component creation, not native process creation. Frontend request p95 reached 82.5 ms and occasional frame intervals exceeded 50 ms; no FPS or overall cold-start target is claimed. Memory point samples do not establish long-session stability.

Native accessibility/visual inspection showed the real 15,000-game grid and successful original Rust probe. A separate missing-database launch displayed a structured database error, retained the backend probe and created no database; the native retry click could not be completed because automation reported unavailable coordinate geometry. Frontend retry tests pass. Physical Windows scale changes, controlled OS-cold startup, HDD/lower-memory hardware and sustained memory trends are untested. Exact environment, measurements, earlier failed driver attempts and file inventory are in [M0.3 verification](development/M0.3_VERIFICATION.md). No commit or push was performed.

## M0.4 implementation and evidence

The existing Details View now uses typed, bounded Rust/SQLite title substring queries with composable installation/source/inclusive-year filters and title/release ordering in both directions. Canonical IDs remain distinct from memberships/copies; relations do not duplicate rows. Search debounces 150 ms, controls retain focus, criteria edits clear selection/reset scrolling, placeholder metadata preserves source options while old results stay hidden, and count/page work shares one request gate. Cache occupancy remains bounded to six page entries.

FTS5 trigram was measured against literal LIKE using bundled SQLite at all four required sizes. M0.4 retains LIKE for consistent short/punctuation substrings and documents ASCII-only folding/non-ASCII exact matching. No dependency, schema, migration or index was added. The first correlated installation-count plan was slow; a measured membership-query rewrite reduced count-plus-page p95 at 15,000 from 295.17 to 101.24 ms and at 25,000 from 592.42 to 262.10 ms, preserving counts in all 56 database cases. Stress installed/not-installed/combined costs still exceed provisional targets; no universal latency guarantee is claimed.

All 24 frontend and 23 Rust tests, frozen installation, TypeScript, ESLint, Prettier/rustfmt, warnings-denied Clippy, production frontend and native builds passed. The final native matrix passed 72 scenarios with at most 26 mounted rows, six cached pages and one active native library request. At 15,000, text criteria commits including debounce reached about 203 ms; non-text commits about 137 ms. Two ordinary-browsing comparison runs retained frame p95 11.1 ms and had no >50 ms intervals, but broader query runs still recorded long intervals overlapping JavaScript long tasks. The issue is not declared resolved. Memory results are short point samples, not leak validation.

See [M0.4 verification](development/M0.4_VERIFICATION.md) for exact semantics, IPC, plans, all timing tables, M0.3 comparison, source-option/virtualizer regression fixes, earlier failures and limits. Normal user databases and real game files were untouched. No commit/push or M0.5 implementation was performed.

## M0.5 implementation and evidence

Synthetic PNG/JPEG originals, derived 64x36 PNG thumbnails and temporary writes are separate. Only mounted visible/overscan rows subscribe; a two-request/32-queued frontend scheduler feeds an ID-only Rust service with two active source jobs, no backend work queue and bounded duplicate waiters. Frontend/native result caches retain 128 entries each; derived disk storage is capped at 256 files/4 MiB. Original byte/pixel limits and cache containment checks reject unsafe inputs. React gains no filesystem URL or raw filesystem capability. Normal startup generates no artwork or library data; fixture selection requires an explicit benchmark build.

All 29 frontend and 32 Rust tests, frozen pnpm installation, TypeScript, ESLint, formatting, default/all-feature warnings-denied Clippy, production frontend and normal native Windows builds passed. The normal optimized app launched through the existing isolated hash-checking runner, showed the empty library and successful Rust connection, ignored selected nonempty benchmark databases/artwork, and rendered no diagnostics. A default-build-only fixture-constant reference was corrected and its values checked against the SQL generator.

Eight final process runs passed 208 UI scenarios with at most 27 mounted rows (including a partial row), six cached data pages, one library request, two artwork jobs/decodes and 26 queued artwork IDs. Library usability took 94-163 ms from component creation; visible artwork settled in 270-406 ms. Ordinal saturation probes filled both cache capacities at all four sizes and completed 800 repeated metadata plus 800 window samples without unstable results. Paired SQL runs retained 50 samples in each of 14 unchanged cases; all counts match M0.4. No SQL/index/schema/search architecture changed.

Artwork contention remains material: installed count-plus-page p95 at 15,000 rose from 99.41 ms idle to 140.69 ms with continuous artwork work; at 25,000 it rose from 204.55 to 299.79 ms. Stress installed/not-installed/combined costs still exceed provisional goals. Matched browsing controls without periodic memory sampling retained page p95 in M0.4's observed range, but occasional long frames persist. A separate CPU profile did not attribute the reproduced long task to a specific function or subsystem; no FPS/root-cause claim is made.

Process memory initially grew across short passes. A 20-pass release-scale run then stayed around 312-320 MiB in its last ten scrolling passes, reaching about 344 MiB after the query matrix. This is limited short-run stabilization evidence, not a strict process-memory bound or long-session leak proof. The report records full timings, memory points, cold/retained caches, observer effects, exact commands, security tests, corrected failures and limitations. Generated assets/results remain ignored. No real installation/personal library, commit, push or M0.6 work was involved.

## Recommended next separately authorized task

Separately authorize M0.6: a synthetic background-load/import simulator with progress, bounded scheduling and safe cancellation, measuring competition with interactive SQL and artwork. Include worker-count/backoff experiments and preserve all current bounds/cancellation/selection tests. Keep stress-query optimization and deeper renderer/native profiling explicitly scoped; no new index/count cache/FTS switch is approved by this task. HDD, lower-memory, physical scaling, controlled OS-cold process startup and longer-session memory evidence remain open. No new GitHub CI run or branch-protection enforcement is claimed. Do not begin M0.6 automatically.

## Genuine remaining work

Validate provisional performance budgets on reference and lower-memory hardware. Choose supported Windows versions/architectures, installer/update/signing policy and defaults. The software license is settled as GPL-3.0-or-later; confirm copyright attribution and review third-party license compatibility and distribution compliance before release. Validate the concrete schema, root map, exact game editions/builds/loaders/mod samples and release adapter subset. Scope provider capabilities and authentication during their milestones. Core library/mod features, game launching, cloud/mobile/Game Tools, and installer delivery are intentionally unimplemented. See the [decision register](SOURCE_AND_OPEN_DECISIONS.md).

## Maintenance rule

Update this file only after verified milestones. Report the change, evidence, limitations and next task. Writing a specification or checking a planning box does not establish implementation completion.
