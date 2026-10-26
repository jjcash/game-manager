# Project Status

Last verification: 2026-10-25. See [M0.1 evidence](development/M0.1_VERIFICATION.md), [M0.1.1 CI verification](development/M0.1.1_CI_VERIFICATION.md), and [M0.2 database verification and measurements](development/M0.2_VERIFICATION.md).

## Actual state

**M0.1 Project Bootstrap and M0.2 SQLite Foundation are complete.** The native shell and original Rust IPC probe work in the rebuilt release. Rust-owned SQLite initialization/migrations, bounded summary reads and deterministic synthetic generation are verified. Database-only release measurements exist at all six required sizes. M0 as a whole remains incomplete: no virtualized view, scrolling, artwork, search/filtering or background-import performance has been validated.

Historical prototype checkmarks were accidental and explicitly withdrawn by the user. They remain invalid; the progress below comes from the current implementation and checks. Specifications and ADRs retain their planning baseline; this file governs current completion claims.

| Area                                                     | Current status                                                                                                                     |
| -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| PRD                                                      | Original detailed baseline recovered and reconciled with later decisions                                                           |
| TAS                                                      | Accepted architectural baseline recovered; implementation sketches still need validation                                           |
| DSS                                                      | Original design retained; initial four-entity prototype translated into tested migration version 1                                 |
| MDRS                                                     | Original detailed protocol recovered; architectural safety direction accepted, protocol remains proposed                           |
| Repository ADRs                                          | ADR-001–005 reconciled; historical labels mapped; ADR-006 accepts GPL-3.0-or-later                                                 |
| Software license                                         | GPL-3.0-or-later accepted; root LICENSE, manifest metadata and first-party SPDX notices added                                      |
| Desktop app and UI                                       | Tauri 2/React/TypeScript/Vite/Tailwind dark shell built and launched; Library/Mods/Settings placeholders only                      |
| Frontend-backend communication                           | Typed read-only `app_get_status`; runtime response validation; native success observed                                             |
| Development quality checks                               | Frontend build/typecheck, ESLint, Prettier/rustfmt, all-feature Clippy, 8 frontend and 16 Rust tests, release desktop build passed |
| SQLite database/migrations                               | M0.2 complete: SQLx/SQLite, versioned migration, purpose separation, WAL/FK bounded pool, status/count/page services               |
| Details View/grid and artwork pipeline                   | Not implemented                                                                                                                    |
| M0 prototype and benchmarks                              | Shell and SQLite complete; all six synthetic database sizes measured; remaining six prototype tasks pending                        |
| Store imports, launching and activity                    | Not implemented                                                                                                                    |
| Mod adapters, deployment and recovery                    | Not implemented or tested                                                                                                          |
| Continuous integration                                   | M0.1.1 workflow valid; all equivalent local checks passed; first successful GitHub run still required                              |
| Installer, updater and release pipeline                  | Not established                                                                                                                    |
| Game Tools, companion, dashboards, extension marketplace | Deferred                                                                                                                           |

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

## Recommended next separately authorized task

M0.3: connect typed frontend IPC to the bounded Rust summary service, add a virtualized Details View with bounded data/row caching, readiness/error handling and title/release sorting, and measure frame time and memory at reference/release/stress sizes. Do not begin automatically. Search/filtering, artwork and import simulation remain separately scoped later prototype work. M0.1.1 still needs an authorized commit/push and first successful GitHub run; no run or branch-protection enforcement is claimed. Do not expand M0 into store integrations, full production migrations, deployment, Game Tools or an extension framework.

## Genuine remaining work

Validate provisional performance budgets on reference and lower-memory hardware. Choose supported Windows versions/architectures, installer/update/signing policy and defaults. The software license is settled as GPL-3.0-or-later; confirm copyright attribution and review third-party license compatibility and distribution compliance before release. Validate the concrete schema, root map, exact game editions/builds/loaders/mod samples and release adapter subset. Scope provider capabilities and authentication during their milestones. Core library/mod features, game launching, cloud/mobile/Game Tools, and installer delivery are intentionally unimplemented. See the [decision register](SOURCE_AND_OPEN_DECISIONS.md).

## Maintenance rule

Update this file only after verified milestones. Report the change, evidence, limitations and next task. Writing a specification or checking a planning box does not establish implementation completion.
