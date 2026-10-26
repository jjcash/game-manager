# Roadmap

Status: **M0 is in progress: M0.1 Project Bootstrap and M0.2 SQLite Foundation complete; M0.1.1 CI awaits GitHub verification.** M0.3 and M1–M5 have not started. See [PROJECT_STATUS](PROJECT_STATUS.md) for verified progress and limitations; database timings do not establish frontend performance.

## M0 — Performance-validation prototype

- [x] Tauri desktop window with one dark theme — M0.1 native window and Rust IPC verified; see [evidence](development/M0.1_VERIFICATION.md).
- [x] SQLite database with 15,000 realistic synthetic game records — M0.2 also verifies 500, 2,000, 5,000, 12,320 and 25,000; see [database evidence](development/M0.2_VERIFICATION.md).
- [ ] Virtualized Details View with sortable columns.
- [ ] Search and filter commands in Rust.
- [ ] Realistic thumbnail loading and bounded caching.
- [ ] Background import simulator with progress and safe cancellation.
- [ ] Measure startup, scrolling, search, memory and interaction under load.
- [ ] Run comparable SSD and HDD measurements.

Scale fixtures through 500, 2,000, 5,000, 12,320, 15,000 and 25,000 records. Use the known Ryzen 5 4500/64 GB system as one reference, plus lower-memory testing. Record release build/version, hardware, storage placement, cache state, dataset seed/version and concurrent work. TAS contains the recovered provisional targets; no target is already achieved. Establish actual build/format/lint/test commands.

Exit: reproducible 15,000-game measurements and a documented architecture assessment; stress findings at 25,000. Keep M0 synthetic and independent of real stores/game installations.

### M0.1.1 — Continuous Integration Foundation

This CI foundation follows the verified M0.1 bootstrap. Its GitHub verification remains outstanding; the user's subsequent request separately authorized M0.2. See the [CI verification report](development/M0.1.1_CI_VERIFICATION.md) for commands, results, pinned versions, and limitations.

- [x] Prepare a Windows GitHub Actions workflow for pull requests to `main`, pushes to `main`, and manual runs.
- [x] Configure existing quality/build commands, pinned tools/actions, frozen dependency installation, dependency-only caches, least-privilege permissions, and cancellation of superseded runs.
- [x] Validate workflow structure and pass equivalent Windows local quality checks, frontend/Rust tests, and the release frontend/native build.
- [ ] Complete M0.1.1: approve commit/push, obtain a successful GitHub run, and record its URL/result. Local checks alone do not complete this milestone.

No installer publishing, deployment, signing, or benchmark gates are part of M0.1.1.

### M0.2 — SQLite Foundation and Synthetic Game Dataset

- [x] Rust-owned SQLx SQLite initialization, embedded checksummed migration, foreign keys on every connection, bounded pool and evaluated WAL configuration.
- [x] Initial canonical games, sources, library entries and installations with constraints/indexes; prototype simplifications documented.
- [x] Deterministic seed-configured generation at all six required sizes, with multi-entry/multi-copy relationships and bounded bulk transactions.
- [x] Explicit isolated benchmark paths, create-new/no-overwrite safety, purpose markers, incomplete-generation handling and Git exclusions.
- [x] Typed status/count/bounded summary services and narrow read-only commands; original app status behavior retained.
- [x] Verify initialization/reopen, constraints, pool FK policy, pagination, generation, path failures and migration/partial-batch failure preservation with disposable fixtures.
- [x] Record release database initialization/generation, closed file sizes and indexed title/release page timings at all sizes; preserve SSD environment, seed, parameters and iteration evidence.
- [x] Frontend checks, all-feature Rust formatting/Clippy/tests, release native build, isolated native smoke check, and documentation.

Complete; see [M0.2 verification](development/M0.2_VERIFICATION.md). These database-only results do not complete M0. HDD, OS-cold, lower-memory, scrolling, artwork and concurrent-background-work evidence remain outstanding. No real user/game data was used, and no Git commit or push was performed.

Next proposed scope is M0.3's virtualized Details View over bounded Rust summaries, including frontend readiness/error handling, bounded caching and measured frame time/memory. It requires separate authorization and has not started.

## M1 — Library foundation

Implement canonical Game, LibraryEntry, Installation and LaunchProfile identities, configurable grouping, manual entries, user overrides, migrations, bounded library queries, virtualized views and initial Steam import. Preserve provenance and make imports idempotent. Include basic settings and artwork storage.

Exit: records persist through restart; repeated imports do not duplicate entries or erase local edits; browsing remains responsive. The proposed detailed DSS must become tested migrations rather than being assumed production SQL.

## M2 — Launching, sessions and background work

Implement manual/standalone and supported Steam launching, multiple launch profiles, process handoff handling, confidence-aware sessions, persistent prioritized jobs, progress/cancellation and throttled notifications. Launching a process is not proof of accurate gameplay tracking.

Exit: supported launch/session workflows pass; concurrent imports do not block interaction; failures and uncertain tracking are explained.

## M3 — Universal mod and safety foundation

Implement mod inventory, immutable package store, safe archive inspection, profiles, adapter contracts, conflict planner, preview, original baselines, managed copy executor, interprocess installation lock, durable journal, verification and recovery. Include read-only detection and optional authorized adoption. Catalog unsupported mods without permitting unsafe deployment. Plan launch-time activation separately from copying.

Exit: disposable filesystem and fault-injection tests cover replacement, undeploy, profile switching, external edits, missing originals, unavailable drives and crashes at every durability boundary. No mutation of real games during tests.

## M4 — Initial game compatibility and launcher coordination

First candidate set: Half-Life 2 (Source), classic Doom II (WAD/PK3/source ports), Stardew Valley (SMAPI). Prototype feasibility determines the validated initial release subset. Record exact editions, builds, loader versions and mod formats. Launch-time activation must work without forcing every mod through file replacement.

Exit: end-to-end import, preview, activation, launch, deactivation and recovery for at least one validated game workflow; explicit support limits. All eight confirmed targets remain in the product plan, with Minecraft, Cyberpunk 2077, Skyrim, Baldur's Gate 3 and Terraria added incrementally. The listed targets do not constitute completed support or a strict implementation order.

## M5 — MVP stabilization and installed release

Complete Library + Mod Manager acceptance from PRD: polished dark theme, Details View and cover grid, keyboard/mouse support, errors/recovery UI, export/import, consistent database backup/restore, user data retention, installer/upgrade/uninstall and support documentation. Choose one installer format, signing/update policy and Windows support matrix. Portable distribution is excluded.

Exit: measured performance gates, data safety/recovery tests and supported workflow checks pass. Do not delay the MVP until all storefronts and eight ecosystems are fully integrated.

## Subsequent library integrations — exact product priority

1. Steam
2. GOG
3. Epic Games
4. Ubisoft Connect
5. EA App
6. Amazon Games
7. Emulators — RomM, RetroArch, RetroAchievements; other emulators later
8. Battle.net
9. itch.io
10. Standalone executables

This is the exact product priority order. It does not require every integration before the MVP. Manual entry and standalone executable launching remain MVP requirements despite standalone discovery being last.

For each provider, separately specify owned-library import, installed detection, metadata, launching, sessions, achievements, authentication, rate limits, rescan behavior and offline failure handling. Product priority is not a guarantee of API feasibility or identical capability coverage. RomM manages ROMs, RetroArch emulates, RetroAchievements supplies retro achievement data.

## Subsequent mod capabilities

Expand game-specific adapters, loader compatibility and dependencies; add repository adapters (Nexus Mods, Thunderstore and potentially mod.io) where APIs and permissions allow. Metadata discovery and download authorization are distinct. Add link deployment only after managed-copy/recovery stability. Virtual filesystem support is a long-term independent backend design. Collections, sharing, advanced manager interoperability and installer-script interpretation require separate designs.

## Later product expansion

- Personal notes/journal, to-do, backlog, favorites, activity statistics and achievement integration.
- Screenshot/recording organization, game-save backup policies, fuller media presentation.
- Basic controller-friendly fullscreen, then richer fullscreen with trailers, music and achievements; supported per-game fullscreen launch preferences.
- Light theme, personal website-inspired theme, richer original/custom themes, home/dashboard/widgets and game statistics pages.
- Optional Android companion for library, stats and notes, then explicitly designed authentication/synchronization. Core remains offline-capable.
- Wand-inspired Game Tools: categorized maps, guides and utilities; richer map layers/markers/progress later.
- Trainers stage 1: external registry and explicit launch; stage 2: external integrations/presets; stage 3: possible native functionality for selected games after separate product/security design. No internal engine or provider is selected.
- Community extensions and an extension marketplace only after built-in features are stable and API/security/versioning boundaries are defined.

## Scope discipline

No Playnite migration requirement. No portable mode or portable-specific testing. Initial configurable installed paths do not imply live storage relocation or cross-computer portability. Provider and trainer implementations remain future work. Update [PROJECT_STATUS](PROJECT_STATUS.md) only with verified evidence.
