# Source Record and Open Decisions

Reconciled: 2026-10-25. This register replaces the earlier partial-recovery account.

## Source coverage and method

Primary source: [Game Manager Annotations Review](https://chatgpt.com/c/6add59c2-80bc-83ea-a64e-e890c3f606f6), conversation `6add59c2-80bc-83ea-a64e-e890c3f606f6`.

The chat-reader initially exposed five recent user turns and assistant placeholders. The signed-in browser view subsequently loaded the earlier conversation and the actual assistant responses. The original PRD, detailed TAS, DSS and MDRS were recovered from rendered conversation content. The original eight-question user answers and later corrections were also read. This package no longer treats those available specifications as missing.

Source HTML/text captures were kept outside the repository-ready hierarchy for preparation. They are not part of the ZIP. Documents are reconciled editions, not verbatim chat exports: navigation/UI artifacts were removed, Markdown formatting restored, and later user decisions applied to earlier drafts. Existing sound requirements were retained as integrated content or clarifications.

## Evidence and authority

| Class | Meaning |
| --- | --- |
| Agreed | Explicit user selection/correction or architectural baseline accepted in the conversation |
| Recovered design | Actual detailed source design; concrete implementation remains subject to validation |
| Proposed | New clarification or an explicitly proposed choice in the recovered design |
| Open | A remaining choice, not a claim that prior agreed content is unavailable |

Current user instructions take precedence. Later explicit corrections override earlier drafts. Accepted architecture decisions govern direction. This register explains evidence and remaining choices; PROJECT_STATUS governs implementation claims; specifications describe intended behavior; ROADMAP sequences work. No design text establishes working functionality.

## Recovered source anchors

| Source turn / passage | Recovered evidence | Applied to |
| --- | --- | --- |
| Initial annotation review and clarifications (`bbb21d4e…`, `4efbe4c3…`) | FOSS, metadata, personal organization, imported community ratings, fullscreen meanings, Android library/stats/notes scope | PRD future vision and data model |
| Performance concerns (`c3c8a135…`) | Startup, scrolling, search/filter/sort, import responsiveness | TAS budgets and M0 |
| Eight-question answers (`f48a20d5-4773-4720-aa19-8e5608a80602`) | Exact eight games, ten integration priorities, no migration requirement, hybrid UX, adoption, grouping, MVP, dark theme, hardware | PRD/AGENTS/ROADMAP/ADRs |
| PRD draft following those answers | Full product requirements and provisional targets | PRD reconciled v0.2 |
| Detailed TAS (`1b75bfe2-1eec-4d26-b538-3639601c4bad`) | Modules, schema identities, interfaces, data flows, jobs, media, tests, benchmark budgets | TAS reconciled v0.2 |
| Subsequent architecture agreement; withdrawn checkmarks (`7c2ab638…`, `84b0d525…`) | Architecture baseline accepted; development checkmarks explicitly accidental | TAS authority and zero-progress status |
| Game Tools addendum (`9aeb889e…`) and trainer choice (`16c6a5ed…`) | Wand-inspired later tools; both trainer approaches progressively, external first | PRD/TAS/DSS/ROADMAP/ADR-005 |
| DSS response (`16c6a5ed-51b4-42ce-8b5a-96cf635f4585`) | Original 19-section storage design and reference SQL; positively reviewed in next user turn | DSS reconciled v0.2 |
| Portable complexity discussion, then “Okay, let's drop the portable version. You can proceed now.” | Portable distribution dropped; installed configurable storage retained | All four specs and ADR-002 |
| MDRS response after dropping portable | Original 28-section ownership/deployment/recovery design; no development started | MDRS reconciled v0.2 |
| Repository preparation and final annotations review | Permanent AGENTS, five-file ADR package, correction of false unresolved claims | All 15 documents |

The source dates are October 24–25, 2026 in the user's conversation. This package's revision date is October 25; it is not a fabricated original acceptance timestamp for every record.

## Agreed decision register

| ID | Established decision | Scope and qualification |
| --- | --- | --- |
| C-01 | Application development has not started; progress 0% | Accidental checkmarks withdrawn; no benchmark results |
| C-02 | Installed Windows only; portable dropped | Keep configurable paths, code/data separation and mixed SSD/HDD use |
| C-03 | Tauri 2 + React/TypeScript + Rust + SQLite; local-first/FOSS | Core independent of accounts/network/cloud |
| C-04 | Virtualized Details View; large-library performance first | 12,320 actual reference, 15,000 release benchmark, 25,000 stress |
| C-05 | Hybrid universal mod core and game adapters | Managed copy first, launch-time activation, links later, VFS long-term |
| C-06 | Progressive future trainers, external first | Native features require a later design; no provider or engine selected |
| C-07 | Later maps/guides/Game Tools | Wand is a product inspiration, not a committed provider integration |
| C-08 | Intended repository E:\dev\game-manager | Documentation package prepared separately; no runtime path dependency |
| C-09 | Eight exact mod targets | Half-Life 2, classic Doom II (not reboot), Stardew Valley, Minecraft, Cyberpunk 2077, Skyrim, Baldur's Gate 3, Terraria |
| C-10 | Exact integration priorities | Steam, GOG, Epic Games, Ubisoft Connect, EA App, Amazon Games, emulators (RomM/RetroArch/RetroAchievements), Battle.net, itch.io, standalone |
| C-11 | Library + Mod Manager MVP | Limited validated adapters; standalone launching still MVP |
| C-12 | Configurable installation grouping | Canonical game/entry/installation/launch profile remain distinct |
| C-13 | No Playnite migration requirement | General export/import and backup retained |
| C-14 | Hybrid UX; read-only existing mod detection; optional adoption | Explicit authorization and verified ownership transition |
| C-15 | One polished dark theme initially | Richer themes, dashboards and fullscreen later |
| C-16 | Built-in features first; extensions/marketplace much later | Stable interfaces now do not authorize arbitrary third-party code |
| C-17 | Optional later Android companion | Library, stats, notes; remote launching not agreed |
| C-18 | Imported community/critic ratings | No requirement to operate a new public rating service |

The exact game list and integration priority are settled. They must not be reintroduced as unresolved questions. The game list is a target set; only the storefront list was explicitly ranked. First adapter candidates are Half-Life 2, classic Doom II and Stardew Valley; their exact supported subset and versions need validation.

## Supporting stack and detailed design

The recovered TAS and original AGENTS include Tailwind CSS, SQLx, TanStack Virtual, Zustand for UI state, TanStack Query with bounded caches, persistent Rust jobs, pnpm, and proposed Tokio/FTS5/WAL configuration. Preserve these as the implementation baseline, while validating versions, concurrency settings and performance during development. Flutter and optional Supabase are future candidate technologies, not required core infrastructure.

DSS contains the real proposed entities, indexes, paths, transactions and SQL sketches. MDRS contains the real proposed state machine, journal, execution/recovery/adoption workflows and interface sketches. Their details are no longer labeled invented reconstructions; they remain unimplemented designs. UUIDv7, physical root mapping, persistence encoding, cleanup limits and recovery primitives must be validated before production use.

## Historical ADR numbering reconciliation

The original chat TAS used eight preliminary labels, while the supplied package had five differently grouped ADR filenames. Keep the supplied filenames stable and use this mapping; do not refer to nonexistent ADR-006/007/008 files.

| Historical TAS label | Historical subject | Current package authority |
| --- | --- | --- |
| ADR-001 | Hybrid backends | ADR-001 |
| ADR-002 | Tauri/React/TS/Rust | ADR-003 |
| ADR-003 | SQLite source of truth | ADR-003 |
| ADR-004 | Local-first/offline | ADR-003 |
| ADR-005 | Virtualized Details View/performance | ADR-004 |
| ADR-006 | Configurable installation grouping | C-12 and PRD |
| ADR-007 | Integrated mods in MVP | C-11 and PRD |
| ADR-008 | Internal functionality first, extensions later | C-16 and PRD |

Current ADR-002 records the later installed-only decision. Current ADR-005 records external-first trainer progression. Reconciliation corrects the supplied records and does not claim any implementation milestone or new architectural approval.

## Open implementation choices

| ID | Remaining subject | Resolution stage |
| --- | --- | --- |
| O-01 | Exact editions, builds, loaders, mod samples and final initial adapter subset | M4 prototype/compatibility validation; eight game names are already confirmed |
| O-02 | Provider capabilities, API/auth/download rights and operational limits | Each integration milestone; priority order is already confirmed |
| O-03 | Detailed design validation and common persisted contracts | Translate recovered specs into tested migrations/interfaces; no historical-spec recovery task remains |
| O-04 | Exact per-game recipes, path rules, backend primitives and journal durability | M3/M4 fixtures and fault injection |
| O-05 | Ratification/tuning of provisional performance budgets, lower-memory baseline and cache limits | M0; reference Ryzen hardware and initial numerical targets already exist |
| O-06 | Minimum Windows versions and CPU architectures | Before release support commitment |
| O-07 | Installer format, signing and update policy | M5; one format initially, NSIS suggested but not finalized |
| O-08 | Concrete DDL, types, physical root map, filenames and retention defaults | DSS implementation; conceptual schema/layout are recovered |
| O-09 | Default metadata provider and artwork policy | Integration/media design |
| O-10 | Specific FOSS license | Before publication/release |
| O-11 | Detailed map/trainer providers, compatibility and native features | Future Game Tools milestones |
| O-12 | Final layouts, accessibility target and personal visual reference | UI design; dark-first/Details View direction is confirmed |

## Corrections made to the supplied package

- Replaced false claims of missing game names and storefront order with recovered exact decisions.
- Restored the original detailed specifications; kept design proposals distinct from accepted architecture and measured implementation.
- Applied portable removal throughout; retained configurable installed storage.
- Restored no-migration, grouping, dark-first, limited-adapter MVP, read-only detection/adoption, emulator roles, and future scope.
- Recovered original performance goals/reference hardware without reporting them as achieved.
- Preserved safety, journal/recovery, bounded-query/cache and backend ownership requirements.
- Reconciled ADR numbering and restored M0–M5 milestone terminology.
- Maintained zero application progress and all prototype checklist items pending.

## Changed-file inventory

All 15 files below were updated; no application files were created:

1. `AGENTS.md`
2. `README.md`
3. `docs/PROJECT_STATUS.md`
4. `docs/ROADMAP.md`
5. `docs/SOURCE_AND_OPEN_DECISIONS.md`
6. `docs/specifications/PRD.md`
7. `docs/specifications/TAS.md`
8. `docs/specifications/DSS.md`
9. `docs/specifications/MDRS.md`
10. `docs/decisions/README.md`
11. `docs/decisions/ADR-001-hybrid-mod-deployment.md`
12. `docs/decisions/ADR-002-installed-only-windows.md`
13. `docs/decisions/ADR-003-core-stack-and-local-first.md`
14. `docs/decisions/ADR-004-virtualized-details-and-benchmark.md`
15. `docs/decisions/ADR-005-external-first-trainers.md`

The corrected ZIP contains only these files, with their repository-relative hierarchy under `game-manager/`. A separate verification report beside the ZIP records entry count, byte/hash comparison, CRC check, links and content checks. No application tests are claimed. No separate E: repository edits, commits or pushes are included.
