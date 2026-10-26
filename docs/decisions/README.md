# Architecture Decision Records

Reconciled 2026-10-25. Accepted architecture describes direction, not implemented functionality or approval of every interface/schema sketch. [Source register](../SOURCE_AND_OPEN_DECISIONS.md) records recovered evidence and maps the original chat's preliminary eight ADR labels to ADR-001 through ADR-005. ADR-006 records the later accepted software-license decision.

| Record | Status | Subject |
| --- | --- | --- |
| [ADR-001](ADR-001-hybrid-mod-deployment.md) | Accepted architecture | Universal mod core; copy first; launch-time activation; links/VFS later; ownership and recovery |
| [ADR-002](ADR-002-installed-only-windows.md) | Accepted | Installed Windows only; portable dropped; configurable installed roots retained |
| [ADR-003](ADR-003-core-stack-and-local-first.md) | Accepted architectural baseline | Core stack, SQLite source of truth, offline core, module ownership |
| [ADR-004](ADR-004-virtualized-details-and-benchmark.md) | Accepted direction; budgets provisional | Details View, known reference hardware, release/stress validation |
| [ADR-005](ADR-005-external-first-trainers.md) | Accepted future direction | Game Tools and progressive trainers, external first |
| [ADR-006](ADR-006-software-license.md) | Accepted | GPL-3.0-or-later for original software code; third-party licensing boundaries |

Exact mod target names and integration priorities are confirmed in [PRD](../specifications/PRD.md); they are not open decisions. Configurable grouping, Library + Mod Manager MVP, no Playnite migration, dark-first UI and built-in functionality before extensions are agreed product requirements recorded there and in the source register.

Use new ADRs for future material architectural changes and link superseded records. Correcting inaccurate source claims is documented reconciliation, not a new feature approval. See [PROJECT_STATUS](../PROJECT_STATUS.md) for actual implementation progress.

## Review and documentation alignment

ADR-001 through ADR-005 were reviewed against the recovered conversation and reconciled specifications. That review corrected the existing records; it did not accept a new architecture, implement features, or validate performance. ADR-006 is a later explicit user decision to adopt GPL-3.0-or-later.

The original chat used preliminary ADR numbers that differ from this package. Its stack/SQLite/local-first decisions are consolidated in current ADR-003; its performance decision is current ADR-004. Configurable installation grouping, integrated mods in the MVP, and built-in functionality before extensions are agreed product requirements. Current ADR-002 records the later removal of portable distribution, and current ADR-005 records external-first trainers. Historical ADR-006/007/008 labels do not designate repository records; the current ADR-006 is the software-license decision, not the historical grouping proposal.

The exact integration priority is Steam, GOG, Epic Games, Ubisoft Connect, EA App, Amazon Games, emulators (RomM, RetroArch, RetroAchievements), Battle.net, itch.io, then standalone executables. Manual entry and standalone launching remain MVP requirements. The eight confirmed mod targets are recorded in ADR-001; exact editions/builds/loaders and the validated first release subset remain implementation work.

**Historical documentation placement verified.** The original 15-document reconciliation placed AGENTS.md and README.md at the root; status, roadmap and source register under docs/; the four specifications under docs/specifications/; and the index plus five original ADRs under docs/decisions/. Later development reports and ADR-006 extend that inventory; the original count is not a current repository file count.

Source: [Game Manager Annotations Review](https://chatgpt.com/c/6add59c2-80bc-83ea-a64e-e890c3f606f6), recovered and reconciled in the preceding documentation correction.
