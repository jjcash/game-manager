# Project Status

Last verification: 2026-10-25. See [M0.1 evidence and limitations](development/M0.1_VERIFICATION.md).

## Actual state

**M0.1 — Project Bootstrap is complete.** Both debug and optimized release applications have been built and launched, and real frontend-backend communication has been observed in their native windows. M0 as a whole remains incomplete: only the shell exists, and no performance measurements or acceptance targets have been validated.

Historical prototype checkmarks were accidental and explicitly withdrawn by the user. They remain invalid; the progress below comes from the current implementation and checks. Specifications and ADRs retain their planning baseline; this file governs current completion claims.

| Area                                                     | Current status                                                                                                |
| -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| PRD                                                      | Original detailed baseline recovered and reconciled with later decisions                                      |
| TAS                                                      | Accepted architectural baseline recovered; implementation sketches still need validation                      |
| DSS                                                      | Original detailed draft recovered, positively reviewed; concrete migrations not implemented                   |
| MDRS                                                     | Original detailed protocol recovered; architectural safety direction accepted, protocol remains proposed      |
| Repository ADRs                                          | ADR-001–005 reconciled; historical labels mapped; ADR-006 accepts GPL-3.0-or-later                               |
| Software license                                        | GPL-3.0-or-later accepted; root LICENSE, manifest metadata and first-party SPDX notices added                  |
| Desktop app and UI                                       | Tauri 2/React/TypeScript/Vite/Tailwind dark shell built and launched; Library/Mods/Settings placeholders only |
| Frontend-backend communication                           | Typed read-only `app_get_status`; runtime response validation; native success observed                        |
| Development quality checks                               | Frontend build/typecheck, ESLint, Prettier/rustfmt, Clippy, 8 frontend tests and 1 Rust contract test passed  |
| SQLite database/migrations                               | Not implemented                                                                                               |
| Details View/grid and artwork pipeline                   | Not implemented                                                                                               |
| M0 prototype and benchmarks                              | In progress: desktop shell only; remaining seven prototype tasks pending; no measurements                     |
| Store imports, launching and activity                    | Not implemented                                                                                               |
| Mod adapters, deployment and recovery                    | Not implemented or tested                                                                                     |
| Installer, updater, CI and release pipeline              | Not established                                                                                               |
| Game Tools, companion, dashboards, extension marketplace | Deferred                                                                                                      |

## M0.1 implementation and evidence

The desktop package is under `apps/desktop/`, with presentation in `src/` and Rust/Tauri code in `src-tauri/`. The shell has one dark theme, accessible navigation, clearly labeled placeholders, a connection status panel and error/retry handling. The only exposed command returns application identity/version/backend; there is no database readiness claim or persistent application state.

`pnpm install --frozen-lockfile`, frontend build/typecheck/lint/tests, Rust tests, formatting checks, and Clippy passed. `pnpm desktop:dev` and `pnpm desktop:build` built debug and optimized release executables. Visual and accessibility inspection of both native windows showed **Rust backend connected · Game Manager 0.1.0**. The release executable loaded bundled assets at `http://tauri.localhost/` with Vite stopped. Native pointer navigation was blocked by an automation target/overlay error; navigation and retry behavior passed DOM unit tests. No unperformed native interaction check is claimed.

No prerequisite installation was needed. The inherited terminal PATH needed a session-only adjustment for existing Rust and pnpm installations. The README contains actual setup/build/check commands, and the verification report lists exact dependency versions and source files. Build outputs/dependencies are ignored; pnpm and Cargo lockfiles are retained. Existing files were already untracked; no changes were staged, committed, or pushed.

The earlier documentation reconciliation remains the product/architecture baseline. ADR-001 through ADR-005 retain their accepted decisions; the later software-license decision is recorded in ADR-006.

## Software-license adoption

The user selected **GPL-3.0-or-later** for original Game Manager software code: GNU GPL version 3 or, at the recipient's option, any later version. See [LICENSE](../LICENSE) and [ADR-006](decisions/ADR-006-software-license.md). Both application manifests carry the exact SPDX identifier, and authored UI/IPC and Rust source files carry SPDX headers. Third-party material retains its own terms; this is not a dependency-license compatibility audit or release-compliance certification. The copyright-holder name remains for the user to confirm. No application behavior, dependency versions, or M0.2 work was changed.

License-text hash/completeness checks, both manifest metadata checks, local Markdown links, frozen dependency installation, Prettier/rustfmt, TypeScript, ESLint, and 8 frontend tests passed. LICENSE is unchanged from matching GNU Emacs/Coreutils repository copies; the direct GNU website download was unavailable. ADR-006 records the source/hash, exact checks, and that limitation.

## Recommended next separately authorized task

Agree and separately authorize M0.2's SQLite performance-prototype scope, following [ROADMAP](ROADMAP.md). No M0.2 work has begun. Synthetic data, virtualized Details View, Rust search/filter queries, artwork loading, import simulation and release benchmarks remain subsequent M0 work. Do not expand M0 into store integrations, full migrations, deployment, Game Tools or an extension framework.

## Genuine remaining work

Validate provisional performance budgets on reference and lower-memory hardware. Choose supported Windows versions/architectures, installer/update/signing policy and defaults. The software license is settled as GPL-3.0-or-later; confirm copyright attribution and review third-party license compatibility and distribution compliance before release. Validate the concrete schema, root map, exact game editions/builds/loaders/mod samples and release adapter subset. Scope provider capabilities and authentication during their milestones. Core library/mod features, game launching, cloud/mobile/Game Tools, and installer delivery are intentionally unimplemented. See the [decision register](SOURCE_AND_OPEN_DECISIONS.md).

## Maintenance rule

Update this file only after verified milestones. Report the change, evidence, limitations and next task. Writing a specification or checking a planning box does not establish implementation completion.
