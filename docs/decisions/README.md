# Architecture Decision Records

Reconciled 2026-10-25. Accepted architecture describes direction, not implemented functionality or approval of every interface/schema sketch. [Source register](../SOURCE_AND_OPEN_DECISIONS.md) records recovered evidence and maps the original chat's preliminary eight ADR labels to this stable five-file package.

| Record | Status | Subject |
| --- | --- | --- |
| [ADR-001](ADR-001-hybrid-mod-deployment.md) | Accepted architecture | Universal mod core; copy first; launch-time activation; links/VFS later; ownership and recovery |
| [ADR-002](ADR-002-installed-only-windows.md) | Accepted | Installed Windows only; portable dropped; configurable installed roots retained |
| [ADR-003](ADR-003-core-stack-and-local-first.md) | Accepted architectural baseline | Core stack, SQLite source of truth, offline core, module ownership |
| [ADR-004](ADR-004-virtualized-details-and-benchmark.md) | Accepted direction; budgets provisional | Details View, known reference hardware, release/stress validation |
| [ADR-005](ADR-005-external-first-trainers.md) | Accepted future direction | Game Tools and progressive trainers, external first |

Exact mod target names and integration priorities are confirmed in [PRD](../specifications/PRD.md); they are not open decisions. Configurable grouping, Library + Mod Manager MVP, no Playnite migration, dark-first UI and built-in functionality before extensions are agreed product requirements recorded there and in the source register.

Use new ADRs for future material architectural changes and link superseded records. Correcting inaccurate source claims is documented reconciliation, not a new feature approval. All application work remains not started; see [PROJECT_STATUS](../PROJECT_STATUS.md).

## Review and documentation alignment

These six records were reviewed against the recovered conversation and reconciled specifications. This review corrects the existing records; it does not accept a new architecture, implement features, or validate performance.

The original chat used preliminary ADR numbers that differ from this package. Its stack/SQLite/local-first decisions are consolidated in current ADR-003; its performance decision is current ADR-004. Configurable installation grouping, integrated mods in the MVP, and built-in functionality before extensions are agreed product requirements. Current ADR-002 records the later removal of portable distribution, and current ADR-005 records external-first trainers. Do not create or cite nonexistent ADR-006/007/008 files from those preliminary labels.

The exact integration priority is Steam, GOG, Epic Games, Ubisoft Connect, EA App, Amazon Games, emulators (RomM, RetroArch, RetroAchievements), Battle.net, itch.io, then standalone executables. Manual entry and standalone launching remain MVP requirements. The eight confirmed mod targets are recorded in ADR-001; exact editions/builds/loaders and the validated first release subset remain implementation work.

**Documentation placement verified.** All 15 Markdown documents are now in the intended repository hierarchy: AGENTS.md and README.md at the root; status, roadmap and source register under docs/; the four specifications under docs/specifications/; and these six decision files under docs/decisions/. The surrounding documents match the reconciled package, the later decision-review updates are preserved, and the duplicate docs/docs/ hierarchy is absent.

Source: [Game Manager Annotations Review](https://chatgpt.com/c/6add59c2-80bc-83ea-a64e-e890c3f606f6), recovered and reconciled in the preceding documentation correction.
