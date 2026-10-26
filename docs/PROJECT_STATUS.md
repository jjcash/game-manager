# Project Status

Last verification: 2026-10-26. See [M0.1 evidence](development/M0.1_VERIFICATION.md), [M0.1.1 CI verification](development/M0.1.1_CI_VERIFICATION.md), [M0.2 database evidence](development/M0.2_VERIFICATION.md), [M0.3 frontend evidence](development/M0.3_VERIFICATION.md), and [M0.4 search/filter/sort verification](development/M0.4_VERIFICATION.md).

## Actual state

**M0.1 through M0.4 are complete within their verified scope.** The native virtualized Details View now supports Rust/SQLite title search, installation/source/year filters and stable title/release sorting. All 72 native query scenarios passed across 500, 12,320, 15,000 and 25,000 games, alongside the original browsing comparison. M0 remains incomplete: artwork, background load, HDD/lower-memory and long-session validation are pending. Stress-count costs and occasional long frames remain measured limitations. M0.5 has not started.

Historical prototype checkmarks were accidental and explicitly withdrawn by the user. They remain invalid; the progress below comes from the current implementation and checks. Specifications and ADRs retain their planning baseline; this file governs current completion claims.

| Area                                                     | Current status                                                                                                       |
| -------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| PRD                                                      | Original detailed baseline recovered and reconciled with later decisions                                             |
| TAS                                                      | Accepted architectural baseline recovered; implementation sketches still need validation                             |
| DSS                                                      | Original design retained; initial four-entity prototype translated into tested migration version 1                   |
| MDRS                                                     | Original detailed protocol recovered; architectural safety direction accepted, protocol remains proposed             |
| Repository ADRs                                          | ADR-001–005 reconciled; historical labels mapped; ADR-006 accepts GPL-3.0-or-later                                   |
| Software license                                         | GPL-3.0-or-later accepted; root LICENSE, manifest metadata and first-party SPDX notices added                        |
| Desktop app and UI                                       | Dark Tauri shell and virtualized Details View with search/filter/sort; Mods/Settings remain placeholders             |
| Frontend-backend communication                           | Original probes/page commands retained; typed criteria/count/window IPC and runtime validation verified              |
| Development quality checks                               | Frozen install, TypeScript, lint, formatting, 24 frontend/23 Rust tests and production frontend/native builds passed |
| SQLite database/migrations                               | M0.2 complete: SQLx/SQLite, versioned migration, purpose separation, WAL/FK bounded pool, status/count/page services |
| Details View/grid and artwork pipeline                   | M0.4 searchable/filterable/sortable Details View verified; cover grid/artwork not implemented                        |
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

## Recommended next separately authorized task

Review the M0.4 stress-count and long-task findings, then separately authorize M0.5's bounded synthetic-thumbnail prototype with cache/decode/corrupt-image and query-under-artwork-load checks. Preserve the current query/cancellation/bounds tests. HDD, lower-memory, physical scaling and sustained-memory evidence remain open. Do not begin automatically. No new GitHub CI run or branch-protection enforcement is claimed by this task.

## Genuine remaining work

Validate provisional performance budgets on reference and lower-memory hardware. Choose supported Windows versions/architectures, installer/update/signing policy and defaults. The software license is settled as GPL-3.0-or-later; confirm copyright attribution and review third-party license compatibility and distribution compliance before release. Validate the concrete schema, root map, exact game editions/builds/loaders/mod samples and release adapter subset. Scope provider capabilities and authentication during their milestones. Core library/mod features, game launching, cloud/mobile/Game Tools, and installer delivery are intentionally unimplemented. See the [decision register](SOURCE_AND_OPEN_DECISIONS.md).

## Maintenance rule

Update this file only after verified milestones. Report the change, evidence, limitations and next task. Writing a specification or checking a planning box does not establish implementation completion.
