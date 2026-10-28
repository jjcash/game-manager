# Roadmap

Status: **M0 conditionally closed under accepted ADR-007 (2026-10-28), retaining exceptions E1–E5; M1.1 manual persistence and M1.2 Steam contract/fixtures are verified.** See [M1.2 evidence](development/M1.2_STEAM_DISCOVERY_VERIFICATION.md) and [PROJECT_STATUS](PROJECT_STATUS.md). Historical M0 sections record task-time decisions; original misses and release obligations remain. M1.3 and later work require separate authorization.

## M0 — Performance-validation prototype

- [x] Tauri desktop window with one dark theme — M0.1 native window and Rust IPC verified; see [evidence](development/M0.1_VERIFICATION.md).
- [x] SQLite database with 15,000 realistic synthetic game records — M0.2 also verifies 500, 2,000, 5,000, 12,320 and 25,000; see [database evidence](development/M0.2_VERIFICATION.md).
- [x] Virtualized Details View over actual Rust/SQLite summaries, bounded caching and keyboard navigation — M0.3.
- [x] Title/release sorting UI in both directions, stable ID ties and explicit NULL handling - M0.4.
- [x] Rust title substring search plus installation/source/year filters, bounded counts/windows - M0.4.
- [x] Real local synthetic thumbnails with bounded jobs/queue/result/disk caches - M0.5; see [artwork evidence](development/M0.5_VERIFICATION.md).
- [x] Synthetic background-load simulator with bounded progress, scheduling and safe cancellation — M0.6; isolated fixtures only.
- [x] Approve conditional M0 gate disposition — user accepted ADR-007 on 2026-10-28 with E1–E5; original misses and unverified release conditions remain.
- [x] Record initial Details View readiness, scrolling/frame intervals, IPC/query durations and memory point samples at 500, 12,320, 15,000 and 25,000 — M0.3; wider startup/load/long-session work remains open.
- [x] Run comparable real NVMe and HDD synthetic measurements — M0.7C reversed placement/load controls; M0.8 supplemental controls. OS-cold and lower-memory results are not implied.

Scale fixtures through 500, 2,000, 5,000, 12,320, 15,000 and 25,000 records. Use the known Ryzen 5 4500/64 GB system as one reference, plus lower-memory testing. Record release build/version, hardware, storage placement, cache state, dataset seed/version and concurrent work. TAS retains the original provisional targets; the M0.8 matrix distinguishes verified passes, misses and unverified conditions. Use the documented build/format/lint/test commands.

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

The 150-ms filter/sort goal is not generally met, including some 15,000 cases; stress installed/not-installed/combined costs remain open. One worker reduces some query tails at slower artwork readiness; default stays two pending broader evidence. Reproduced long tasks are predominantly renderer Layout in focused traces, but the CSS trigger and unprofiled 235-ms outlier remain unknown. Memory is measured, not proven strictly bounded. HDD/lower-memory and long-session M0 gates remain open. M0.7A, M0.7B and M0.7C were subsequently authorized and are recorded below. No real importer, commit or push is included.

### M0.7A — Renderer layout profiling and optimization

Complete within this investigation scope; see [M0.7A verification](development/M0.7A_VERIFICATION.md).

- [x] Reproduce the installed-complement → all/release-descending transition with deterministic 15,000/25,000 fixtures and inspect renderer Layout wall/CPU/object/frame evidence.
- [x] Control CSS/DOM/diagnostics/artwork/order effects; isolate first-use Japanese font work and retain failed reproduction attempts.
- [x] Scope an optional explicit local fallback to titles; reject the broader global-font candidate and preserve placeholder/interface fonts, appearance and virtualization behavior.
- [x] Repeat alternating native artifact comparisons, warm trials, hidden diagnostics/no-artwork controls and matched untraced runs; record mixed commit/script/scroll findings without claiming every latency target passed.
- [x] Verify regression/accessibility semantics, real narrow-window resizing, existing native browsing/query scenarios, local checks/builds and normal-production isolation.
- [x] Preserve historical reports/outliers, correct process-memory ancestry sampling and document remaining font/frame/memory/layout limitations.

Median largest Layout is 58.822 → 31.838 ms at 15,000 and 78.020 → 20.271 ms at 25,000. No long task was observed in the 12 final-candidate first-use conditions, while some frame p99, script/commit and later-scroll measurements remain higher. Bounds stay 26 mounted rows/six pages/one active library request. The 235-ms historical task, legacy 718-MiB aggregate point, existing header offset/focus contrast and broader M0 hardware/startup/long-session gate remain open. No SQL/index/migration, worker-default, integration, mod, cloud or mobile work is included.

M0.7B and M0.7C were subsequently authorized and are recorded below.

### M0.7B — SQLite query and latency optimization

Complete within this investigation scope; see [M0.7B verification](development/M0.7B_VERIFICATION.md).

- [x] Establish a clean merged M0.7A baseline and measure foreground SQL before changing query behavior.
- [x] Capture actual bound SQL/plans and split count/page/ID selection/projection/acquisition; evaluate seven query shapes and two disposable covering-index candidates.
- [x] Select present-entry sets with indexed canonical membership, using existing indexes and no migration/count cache/denormalization; preserve exact M0.4 semantics.
- [x] Repeat 500/12,320/15,000/25,000 baseline/candidate SQL comparisons, including all sorts, complements, composed criteria and deep pages, with exact row/count checks and retained tails.
- [x] Verify presence/membership writes, concurrent WAL snapshots, existing cancellation/generations, all local checks/builds and ordinary production isolation.
- [x] Run native artwork/background controls, reverse-order repeats, direct filtered-window arrival probes, existing browsing/query scenarios, M0.7A font/layout controls and paired memory cycles; document mixed results.

Installed SQL count+page p95 improves 110.67/132.80 → 53.35/53.31 ms at 15,000 and 215.79/194.86 → 67.43/70.42 ms at 25,000. Native two-artwork-worker installed IPC p95 improves 149.0 → 79.5 and 307.3 → 139.5 ms. Forty untraced load conditions, four focused trace/four memory controls, 144 query scenarios and 320 filtered-window arrivals preserve 26 rows/six pages/one library request. Some not-installed/deep/write-load paths regress; stress title-tail pages still exceed 50 ms and process memory grows during the sampled cycles. These results do not complete the wider M0 gate.

At the M0.7B handoff, deep-page/write-load variability, finer arrival timing, longer memory, HDD/lower-memory and startup were open. M0.7C's subsequent findings follow; no integrations or later product work is included.

### M0.7C — Integrated performance and resource validation

Complete within the investigation scope; see [M0.7C verification](development/M0.7C_VERIFICATION.md). **The overall M0 performance gate remains open.**

- [x] Verify the merged M0.7B baseline, preserve binaries/hashes/reference seed and previous reports, and define remaining provisional budgets.
- [x] Reproduce first/deep pages under ordinary/artwork/write/mixed work; capture actual SQL/plans, pool/service/full invoke/protocol/target/full-window stages and order-reversed historical artifacts.
- [x] Run separated sustained memory controls, real cache saturation/eviction and four 150-cycle matched combined captures, with per-PID age-validated process memory, JS heap, jobs and idle recovery.
- [x] Compare zero/one/two artwork workers, CPU/hash/write/mixed jobs and foreground backoff in repeated reversed order; add active visible-thumbnail readiness and retain defaults after tradeoff assessment.
- [x] Measure whole-process/backend/shell/Library/rows/images startup separately; identify retained OS caches and unavailable creation/presentation/cold boundaries.
- [x] Test actual HDD/NVMe, bounded HDD contention and targeted relationship distributions; explicitly leave lower-RAM/other physical scaling unverified with future procedures.
- [x] Preserve title/placeholder fonts, geometry/focus/selection, pagination/stale-response/request/cache/job bounds; run automated and applicable native regressions/builds and production-isolation smoke.
- [x] Consolidate pass/fail/unverified budgets, limitations, historical outliers and remaining-M0 recommendations without claiming a passed wider gate.

Integrated page p95 exceeds 50 ms at release/stress, some stress command pairs exceed 150 ms, and release whole-app warm startup misses one second. Long mixed captures show finite late stabilization and idle recovery, without proving leak freedom. High membership/copy fanout exposes additional page costs. No SQL/index/schema/cache/default/admission change is adopted. The 235-ms task, 718-MiB historical point, physical lower-RAM/scaling, OS-cold and actual displayed-frame evidence remain open; the 15-pixel header offset/focus contrast remain separate UI follow-ups. Any next task needs separate authorization within remaining M0; M1 is not started. No commit or push.

### M0.7D - Whole-process startup profiling and optimization

Complete as an investigation; see [M0.7D verification](development/M0.7D_VERIFICATION.md). **The overall M0 performance gate remains open.**

- [x] Preserve the merged M0.7C baseline, retained artifacts, original reports and all seed-42 fixture hashes.
- [x] Add bounded monotonic/UTC native phase marks, frontend/navigation boundaries, separate window-construction profiling and explicit legacy/buffered/off observer controls.
- [x] Run 156 ordinary timing launches across 500/12,320/15,000/25,000 and zero/two artwork workers, with release-scale reversed-order repetitions; retain faster/worse cohorts and fresh/reused profiles.
- [x] Measure the dominant synchronous window/WebView aggregate separately from small application setup and database phases; record unavailable privileged system attribution.
- [x] Retain production initialization after rejecting unproven optimizations; preserve actual loaded-row usability and initialization errors/recovery.
- [x] Pass automated checks, both native builds, isolated normal launches and representative release/stress browsing/query/font/layout/artwork/job/native-resize regressions.

Final 15,000-game native-timing-off medians remain **1,034.6 / 1,128.6 ms** with zero/two workers. Window construction profiles are about **717-917 ms**, application setup **3-4 ms**, database tasks **21-39 ms** in small reused-profile controls. No WebView2-internal, AV, DLL or scheduling cause is fabricated; WPR cannot start in the current non-elevated session. No production optimization or uniform startup improvement is claimed, and worse artifact-associated differences remain unresolved. Page/load tails and prior M0 limitations remain open. M0.7E was subsequently authorized with the query scope below; further startup optimization is deferred. No commit or push.

### M0.7E — Relationship-heavy and deep-page query cost/benefit investigation

Complete as an investigation; see [M0.7E verification](development/M0.7E_VERIFICATION.md). **Accept the documented query limitation; retain M0.7B production queries. The M0 gate remains open.**

- [x] Confirm M0.7D merged baseline and preserve reference/derivative fixtures and historical reports.
- [x] Characterize exact counts, sorted IDs, bounded summaries, full pages, pool acquisition and native invoke/target/full-window costs separately.
- [x] Compare reference, sparse/dense, two justified intermediate recipes and existing extreme fanout; explicitly separate structural plausibility from unsupported prevalence.
- [x] Measure bounded query-only candidates and a representative covering-index screen, retain regressions and apply the cost-benefit stop rule.
- [x] Complete 32 matched/reversed native deep conditions and 16 continuous artwork/mixed conditions at 15k/25k; preserve one failed writer-guard attempt, safe-prefix audit and successful repeat.
- [x] Retain canonical/search/source/date/order/arbitrary-jump/resource/font behavior; pass final automated checks, native regressions, both build modes and normal-build isolation.
- [x] Revert the temporary production candidate, document zero added production DB storage/write/migration cost, remaining budgets and next-task recommendation.

Ordinary 15k benefits are modest/inconsistent; selected extreme page-p95 savings of 125–263 ms remain far over 50 ms and do not establish expected usage. Sparse and native release-scale regressions prevent a global change. No cache, flag, FTS, migration, keyset-only pagination, worker/pool increase, startup refactor or real integration is introduced. Remaining hardware/presentation/resource gates need separate disposition; M1 does not start automatically. No commit or push.

### M0.8 — Final M0 performance gate disposition

The [disposition report](development/M0.8_GATE_DISPOSITION.md) consolidates original numerical targets, technical passes/failures/unverified conditions, cost-benefit judgments and proposed exceptions. Focused tests reproduce deliberate writer-pool exhaustion and verify surfaced failure, cancellation, valid prefixes and recovery; they do not establish the historical unexpected timeout's cause. No pool/deadline/scheduler/query/startup change is selected.

**Accepted: conditional M0 closure with documented exceptions, by explicit user instruction on 2026-10-28.** [ADR-007](decisions/ADR-007-conditional-m0-performance-gate.md) records acceptance; the gate checkbox above records conditional closure. Original PRD/TAS/DSS targets stay unchanged. Missing low-RAM/cold-start/compositor evidence and feature-specific acceptance remain release/product obligations. No open-ended profiling or SQL program is proposed.

After approval, recommend a separately authorized small M1 core library-foundation slice. Do not start importing, providers, mod work or M1 automatically. Reopen only for the documented correctness/resource/representative-performance conditions, with a plausible cost-effective remedy. No commit or push.

## M1 — Library foundation

### M1.1 — Persistent Manual Library Foundation

**Complete and verified 2026-10-28.** Manual creation, explicit user metadata overrides, manual memberships and installation metadata, safe metadata-only removal, transactional persistence, restart and bounded Details View integration. Schema v2 extends application databases only; frozen generator-v1 benchmarks stay schema v1. See [verification](development/M1.1_VERIFICATION.md) for 45 frontend/37 default/60 benchmark-feature Rust tests, both native builds, persistence/isolation and 15k/25k regressions. No commit, push or later work is performed or authorized.

### M1.2 — Steam Local Discovery Contract and Fixtures

**Complete within scope, verified 2026-10-28:** local-source research, conservative installation/cache/ownership/account evidence, stable provider identity and user-override contracts, synthetic bounded parsers and adversarial fixtures. No production Steam read/import/write path or schema change. See [verification](development/M1.2_STEAM_DISCOVERY_VERIFICATION.md) for 17 contract tests, 46 frontend/54 default/77 benchmark-feature Rust tests, both native builds and persistence/isolation checks. A minimal modal Tab-boundary regression fix preserves the original native assertions. M0 E1–E5 remain accepted; M1.1 identity semantics remain intact.

Recommended next **separately authorized** slice: M1.3 read-only installed-app discovery preview with native metadata-root permissions, Windows confinement/race tests, bounded paging, warnings and cancellation. No account-file reads, binary caches, networking, catalog commit or entitlement claim. A later persistent-import milestone needs explicit provider origins/mappings/value storage and tested v2 migration. No M1.3 work is authorized or started by this recommendation.

### Broader remaining M1 scope

Extend the verified Game, LibraryEntry and Installation foundation with separately authorized LaunchProfile identity, configurable grouping, initial Steam import, basic settings and artwork storage. Preserve explicit user overrides, canonical identity and bounded queries; make future imports idempotent.

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
