# Game Manager — Agent Instructions

## Project overview

Game Manager is a free and open-source, performance-first Windows desktop application for managing game libraries, launching games, tracking activity, and managing mods.

The application is local-first and must remain usable without cloud services.

Original Game Manager software code is licensed under `GPL-3.0-or-later`: GNU GPL version 3 or, at the recipient's option, any later version. Preserve third-party notices and licenses; the project license does not automatically relicense external content. See `LICENSE` and `docs/decisions/ADR-006-software-license.md`.

## Authoritative project documentation

Before making architectural or significant implementation decisions, consult:

- [docs/specifications/PRD.md](docs/specifications/PRD.md) — Product requirements
- [docs/specifications/TAS.md](docs/specifications/TAS.md) — Technical architecture
- [docs/specifications/DSS.md](docs/specifications/DSS.md) — Database and storage
- [docs/specifications/MDRS.md](docs/specifications/MDRS.md) — Mod deployment and recovery
- [docs/decisions/](docs/decisions/) — Accepted architecture decisions
- [docs/ROADMAP.md](docs/ROADMAP.md) — Implementation roadmap
- [docs/PROJECT_STATUS.md](docs/PROJECT_STATUS.md) — Actual implementation status
- [docs/SOURCE_AND_OPEN_DECISIONS.md](docs/SOURCE_AND_OPEN_DECISIONS.md) — Evidence, agreed decisions, and remaining implementation choices

Distinguish proposed specifications from accepted decisions. Do not assume a feature exists simply because it appears in the documentation.

## Technology stack

- Windows desktop: Tauri 2
- Frontend: React + TypeScript
- Styling: Tailwind CSS
- Native backend: Rust
- Database: SQLite via SQLx
- Library virtualization: TanStack Virtual
- UI state: Zustand
- Async frontend state: TanStack Query
- Background processing: Rust job scheduler

Use pnpm for JavaScript package management.

Do not introduce alternative frameworks or major dependencies without explaining the reason and requesting approval.

## Architecture rules

1. React owns presentation and temporary UI state.
2. Rust owns persistent business logic, filesystem operations, processes, and database access.
3. SQLite is the local source of truth.
4. All external integrations use clearly defined adapters.
5. Mod deployment uses the accepted hybrid architecture.
6. Game installations and externally managed files must not be treated as application-owned.
7. Long-running work must not block the UI.
8. Use typed, narrow Tauri commands instead of unrestricted filesystem, SQL, or shell execution.
9. Prefer simple, maintainable implementations over unnecessary abstractions.
10. Do not add cloud dependencies to core functionality.

## Performance requirements

The application must be designed for large libraries.

- Primary reference size: 12,320 games
- Release-scale benchmark: 15,000 games
- Stress test: 25,000 games
- Primary interface: Virtualized Details View

Never load entire libraries, large mod manifests, or full-resolution artwork into frontend state unless explicitly justified.

Use indexed database queries, bounded caching, lazy loading, and background processing.

Treat performance targets as goals until verified by benchmarks.

## Mod-management safety

Never modify real game installations during tests.

Use disposable fixtures and temporary directories.

Never:

- Delete files whose ownership is uncertain.
- Overwrite externally modified files silently.
- Execute untrusted mod archive contents automatically.
- Apply deployment operations without validation.
- Mark deployments successful before verification.
- Assume SQLite transactions make filesystem operations atomic.

Follow the deployment journal and recovery rules described in `docs/specifications/MDRS.md`. The original detailed protocol is recovered in MDRS; concrete implementation mechanics remain subject to validation.

## Development workflow

Before implementing a task:

1. Read the relevant documentation and existing code.
2. Identify the task scope and affected modules.
3. Explain the proposed approach for nontrivial changes.
4. Implement the smallest complete, testable change.
5. Add or update appropriate tests.
6. Run the relevant checks.
7. Report results, limitations, and remaining work.

Do not invent successful test results.

Do not modify unrelated files without justification.

Ask for approval before destructive operations, major architectural changes, or adding substantial dependencies.

## Code quality

- Prefer strongly typed interfaces.
- Use explicit error handling.
- Keep modules focused.
- Avoid unnecessary duplication.
- Document non-obvious decisions.
- Use parameterized SQL queries.
- Use structured logging where appropriate.
- Follow Rust and TypeScript formatting and linting conventions.
- Do not introduce placeholder implementations and present them as completed functionality.

## Git and task management

- Keep changes small and reviewable.
- Never force-push, rewrite history, or delete branches without explicit approval.
- Never commit credentials, generated databases, benchmark datasets, build outputs, or personal game files.
- Commit appropriate dependency lockfiles and database migrations.
- Maintain `docs/PROJECT_STATUS.md` after verified milestones.
- Record significant architecture changes in `docs/decisions/`.
- Do not mark tasks complete without verification.

## Verification

Use the repository's documented commands for formatting, linting, testing, and building.

If a command cannot run, report why.

For performance-sensitive changes, run the appropriate benchmark when available.

For filesystem-changing features, include failure and recovery tests.

## Current project status

Planning specifications exist, but no application development has started.

The first implementation milestone is the performance-validation prototype.

Do not begin implementing advanced game integrations, mod deployment, mobile applications, trainer functionality, or extension marketplaces before their scheduled milestones.

## Completion report

After each task, provide:

- Summary of changes
- Files changed
- Tests and checks run
- Results
- Unresolved problems
- Suggested next task

## Confirmed product scope and integration priorities

Read the reconciled PRD, TAS, DSS and MDRS. The original detailed specifications have been recovered; do not reclassify the game list or integration order as unresolved.

The MVP is Library + Mod Manager with limited validated game-specific support. It includes manual/standalone launching, configurable grouping of multiple installations, explicit metadata overrides, backup/export, read-only existing-mod detection and optional authorized adoption. No Playnite migration is required. One polished dark theme comes first.

Integration priority order: Steam → GOG → Epic Games → Ubisoft Connect → EA App → Amazon Games → emulators (RomM, RetroArch, RetroAchievements) → Battle.net → itch.io → standalone executables. This discovery priority does not defer manual entry or standalone launching beyond the MVP.

Confirmed mod targets: Half-Life 2; classic Doom II (not the reboot); Stardew Valley; Minecraft; Cyberpunk 2077; Skyrim; Baldur's Gate 3; Terraria. First adapter candidates are Half-Life 2, classic Doom II and Stardew Valley. Exact supported editions/builds/loaders and initial release subset still require validation. Product targets do not establish working support or a strict eight-game implementation order.

Use a universal mod core with capability-based game adapters and interchangeable deployment backends. Managed copy comes first; launch-time activation can avoid file writes; links follow stability; virtual filesystem support is long-term. Adapters describe compatibility and activation; the executor owns writes. Existing-mod detection never transfers ownership. Adoption needs an explicit reviewed, preserved and verified file set.

Installed Windows only. Portable mode, portable release variants, automatic relocation and portable-specific testing were dropped. Retain centralized path resolution and configurable installed data locations with mixed SSD/HDD storage.

## Evidence, status and deferred work

Current explicit user instructions take precedence. Accepted ADRs and the decision register establish direction; detailed SQL, DTOs, traits, job internals, path layouts, installer/update choices and provisional budgets require implementation validation. PROJECT_STATUS governs completion claims. The original ADR-001 through ADR-005 filenames remain stable; historical chat numbering is mapped in the source register. ADR-006 records the later software-license decision.

The reference system is Ryzen 5 4500 with 64 GB RAM, with HDD games and optional SSD application data. This is not a minimum requirement. Test 500, 2,000, 5,000, 12,320, 15,000 and 25,000 entries; validate lower-memory configurations too. Original latency/frame targets are recorded in TAS as provisional, unmeasured goals.

Built-in features come first. Android library/stats/notes access, dashboards, richer themes, fuller fullscreen presentation, achievements, media organization, save backups and optional synchronization follow core stability. Community extensions and an extension marketplace are much later.

Game Tools include future maps, guides, utilities and progressive trainer support: external tools first, deeper external integrations/presets, then possible native functionality for selected games after separate design. No native trainer engine, provider, automatic executable downloads or execution is approved. No cloud service is required for core offline use.

No application development has started; all prototype tasks are pending. Accidental historical checkmarks are explicitly invalid. For this documentation correction, do not scaffold code, install application dependencies, commit or push. The next separately authorized engineering milestone is roadmap M0.
