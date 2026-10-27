# Roadmap

Status: **M0 is in progress: M0.1 through M0.6 and M0.7A/M0.7B complete within their verified scope; M0.1.1 CI awaits documented GitHub verification.** M0.7C and M1-M5 have not started. See [PROJECT_STATUS](PROJECT_STATUS.md) for measured limitations; the wider performance gate remains open.

## M0 — Performance-validation prototype

- [x] Tauri desktop window with one dark theme — M0.1 native window and Rust IPC verified; see [evidence](development/M0.1_VERIFICATION.md).
- [x] SQLite database with 15,000 realistic synthetic game records — M0.2 also verifies 500, 2,000, 5,000, 12,320 and 25,000; see [database evidence](development/M0.2_VERIFICATION.md).
- [x] Virtualized Details View over actual Rust/SQLite summaries, bounded caching and keyboard navigation — M0.3.
- [x] Title/release sorting UI in both directions, stable ID ties and explicit NULL handling - M0.4.
- [x] Rust title substring search plus installation/source/year filters, bounded counts/windows - M0.4.
- [x] Real local synthetic thumbnails with bounded jobs/queue/result/disk caches - M0.5; see [artwork evidence](development/M0.5_VERIFICATION.md).
- [x] Synthetic background-load simulator with bounded progress, scheduling and safe cancellation — M0.6; isolated fixtures only.
- [ ] Complete wider startup/scroll/search/memory/interaction gate — M0.6 records SSD contention and component readiness; controlled process-cold, HDD/lower-memory and longer-session evidence remain open.
- [x] Record initial Details View readiness, scrolling/frame intervals, IPC/query durations and memory point samples at 500, 12,320, 15,000 and 25,000 — M0.3; wider startup/load/long-session work remains open.
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

M0.3 subsequently connects this service to the actual frontend; see the verified slice below.

### M0.3 — Virtualized Details View and Frontend Performance Validation

- [x] Narrow typed info/window IPC, structured errors, existing 200-row server limit and deterministic canonical ordering.
- [x] TanStack Virtual fixed-height Details View, bounded TanStack Query pages, one active native page invocation and Zustand UI selection/position.
- [x] Loading, empty, partial failure/retry, refresh generations, stale-response cleanup, keyboard navigation and stable selection tests.
- [x] Explicit read-only synthetic dataset configuration, purpose/migration/completion/count validation and development diagnostics.
- [x] Pass existing/new checks (19 frontend and 18 Rust tests), production frontend and native Tauri builds.
- [x] Real native runs twice each at 500, 12,320, 15,000 and 25,000; actual end rows, wheel/rapid/distributed scrolling, selection, return navigation and emulated viewport/scaling.
- [x] Record timing/frame/memory evidence and limitations; inspect native 15,000-game UI and missing-database failure without creating a database.

Complete within this scope; see [M0.3 verification](development/M0.3_VERIFICATION.md). Initial usable Library commits measured 113.9–179.6 ms from component creation. Peak mounted rows were 26, retained page entries six and active native page invocations one. IPC tail latency and occasional long frames remain findings to investigate; no FPS, controlled cold-start or sustained-memory target is claimed. Physical Windows scaling, HDD and lower-memory validation remain outstanding. No commit/push or M0.4 work was performed during M0.3. M0.4 was subsequently authorized and is recorded below.

### M0.4 - Search, Filtering and Sorting

- [x] Typed validated criteria, parameterized Rust/SQLite predicates, correct canonical counts, bounded stable windows and allowlisted sort directions.
- [x] Literal substring/case/Unicode/punctuation semantics; benchmark FTS5 trigram against LIKE and document the selected scan-based strategy without a production index/migration.
- [x] Installation/source/year composition, global canonical summaries, inclusive years and explicit NULL/tie ordering.
- [x] Debounced accessible controls, reset/clear, distinct empty/no-match/error states, no stale results, focus/selection/scroll handling and bounded cancellation/cache behavior.
- [x] Pass 24 frontend and 23 Rust tests plus all local quality checks, production frontend/native builds.
- [x] Fifty-iteration database/plans scenarios at four sizes, 72 native query scenarios, and two 15,000-game M0.3 browsing comparisons.
- [x] Investigate correlated-count regressions and long frames; retain measured stress limitations rather than claiming every provisional budget achieved.

Complete within this scope; see [M0.4 verification](development/M0.4_VERIFICATION.md). Membership queries reduced the installed count-plus-page p95 from 295.17 to 101.24 ms at 15,000, while 25,000 still reached 262.10 ms. Native bounds remain 26 rows/six pages/one library request. Occasional long query-workload frames, HDD/lower-memory and sustained-memory checks remain open. No artwork/import/job work, commit/push or M0.5 implementation was performed. This describes the M0.4 handoff; the separately authorized M0.5 follows below.

### M0.5 - Bounded synthetic artwork and thumbnail pipeline

Complete within the synthetic prototype scope; see [M0.5 verification](development/M0.5_VERIFICATION.md). Real local thumbnails appear in the virtualized Details View through ID-only Rust access. Source byte/pixel limits, two jobs, a 32-entry frontend queue, 128-entry result caches and 256-file/4-MiB derived disk storage are enforced. Originals, thumbnails and temporary files have separate ownership; missing/corrupt assets preserve library interaction. No provider, importer, real-game scan or mod feature was added.

All local quality checks, 29 frontend/32 Rust tests and production builds passed; normal optimized startup and disabled benchmark selection were verified in an isolated root. Eight UI runs at 500/12,320/15,000/25,000, saturated-cache IPC/SQL probes, matched browsing controls, a CPU profile and a 20-pass memory run are documented. Primary release-scale installed SQL p95 reached 140.69 ms under continuous artwork; stress reached 299.79 ms. Long frames and shared-resource contention remain open. The longer short-run memory observation does not complete all M0 memory/hardware goals.

M0.6 was subsequently authorized and is recorded below. Query architecture/index changes remain a separate proposal.

### M0.6 - Synthetic background load and responsiveness

Complete within the synthetic prototype scope; see [M0.6 verification](development/M0.6_VERIFICATION.md). Tokio scheduling enforces one active/two queued jobs, a two-worker shared resource ceiling, bounded progress/history, typed state/failure and cooperative cancellation. Hashing/parsing and short workload write batches use independently generated disposable databases with the existing four-connection pool. Canonical benchmarks and ordinary application databases remain isolated. Normal production binaries omit job handlers and controls.

All applicable local quality checks, 36 frontend/44 Rust tests (28 default), production/benchmark builds and normal-launch isolation passed. Forty primary conditions at 500/12,320/15,000/25,000 retain 3,200 metadata and 3,200 page samples, with worker/backoff repeats, matched no-thumbnail mixed work, existing native regression drivers, paired M0.5/M0.6 artifacts, memory cycles, renderer traces and graceful shutdown/restart. Cancellation/failure preserves exact valid partial counts, not false completion.

The 150-ms filter/sort goal is not generally met, including some 15,000 cases; stress installed/not-installed/combined costs remain open. One worker reduces some query tails at slower artwork readiness; default stays two pending broader evidence. Reproduced long tasks are predominantly renderer Layout in focused traces, but the CSS trigger and unprofiled 235-ms outlier remain unknown. Memory is measured, not proven strictly bounded. HDD/lower-memory and long-session M0 gates remain open. M0.7A and M0.7B were subsequently authorized and are recorded below; M0.7C has not started. No real importer, commit or push is included.

### M0.7A — Renderer layout profiling and optimization

Complete within this investigation scope; see [M0.7A verification](development/M0.7A_VERIFICATION.md).

- [x] Reproduce the installed-complement → all/release-descending transition with deterministic 15,000/25,000 fixtures and inspect renderer Layout wall/CPU/object/frame evidence.
- [x] Control CSS/DOM/diagnostics/artwork/order effects; isolate first-use Japanese font work and retain failed reproduction attempts.
- [x] Scope an optional explicit local fallback to titles; reject the broader global-font candidate and preserve placeholder/interface fonts, appearance and virtualization behavior.
- [x] Repeat alternating native artifact comparisons, warm trials, hidden diagnostics/no-artwork controls and matched untraced runs; record mixed commit/script/scroll findings without claiming every latency target passed.
- [x] Verify regression/accessibility semantics, real narrow-window resizing, existing native browsing/query scenarios, local checks/builds and normal-production isolation.
- [x] Preserve historical reports/outliers, correct process-memory ancestry sampling and document remaining font/frame/memory/layout limitations.

Median largest Layout is 58.822 → 31.838 ms at 15,000 and 78.020 → 20.271 ms at 25,000. No long task was observed in the 12 final-candidate first-use conditions, while some frame p99, script/commit and later-scroll measurements remain higher. Bounds stay 26 mounted rows/six pages/one active library request. The 235-ms historical task, legacy 718-MiB aggregate point, existing header offset/focus contrast and broader M0 hardware/startup/long-session gate remain open. No SQL/index/migration, worker-default, integration, mod, cloud or mobile work is included.

M0.7B was subsequently authorized and is recorded below. M0.7C requires separate authorization.

### M0.7B — SQLite query and latency optimization

Complete within this investigation scope; see [M0.7B verification](development/M0.7B_VERIFICATION.md).

- [x] Establish a clean merged M0.7A baseline and measure foreground SQL before changing query behavior.
- [x] Capture actual bound SQL/plans and split count/page/ID selection/projection/acquisition; evaluate seven query shapes and two disposable covering-index candidates.
- [x] Select present-entry sets with indexed canonical membership, using existing indexes and no migration/count cache/denormalization; preserve exact M0.4 semantics.
- [x] Repeat 500/12,320/15,000/25,000 baseline/candidate SQL comparisons, including all sorts, complements, composed criteria and deep pages, with exact row/count checks and retained tails.
- [x] Verify presence/membership writes, concurrent WAL snapshots, existing cancellation/generations, all local checks/builds and ordinary production isolation.
- [x] Run native artwork/background controls, reverse-order repeats, direct filtered-window arrival probes, existing browsing/query scenarios, M0.7A font/layout controls and paired memory cycles; document mixed results.

Installed SQL count+page p95 improves 110.67/132.80 → 53.35/53.31 ms at 15,000 and 215.79/194.86 → 67.43/70.42 ms at 25,000. Native two-artwork-worker installed IPC p95 improves 149.0 → 79.5 and 307.3 → 139.5 ms. Forty untraced load conditions, four focused trace/four memory controls, 144 query scenarios and 320 filtered-window arrivals preserve 26 rows/six pages/one library request. Some not-installed/deep/write-load paths regress; stress title-tail pages still exceed 50 ms and process memory grows during the sampled cycles. These results do not complete the wider M0 gate.

M0.7C should separately address remaining deep-page/write-load variability, finer native arrival/presentation measurements and paired longer-session memory evidence. HDD/lower-memory and controlled startup remain open. No M0.7C, integration, mod, cloud/mobile work, commit or push is included.

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
