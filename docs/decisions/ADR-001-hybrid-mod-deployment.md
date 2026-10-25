# ADR-001: Hybrid Safe Mod Deployment

Implementation status: **Not started**. This record documents an agreed direction; it does not claim completed code, tests, or benchmarks.

Status: **Accepted architecture**. Reconciled 2026-10-25 from the original discussion; detailed MDRS protocol remains a proposed implementation design.

## Context

The user requires integrated mod management across a broad variety of games. One deployment mechanism cannot safely cover Source mods, WAD/PK3 launch selection, loader-based mods and complex package/load-order systems.

## Decision

Use a universal capability-based mod core, installation-bound profiles, game-specific adapters and interchangeable deployment backends. Implement safe, reversible managed copy first. Support launch-time activation where an adapter can avoid filesystem deployment. Add links later and virtual filesystem support as a long-term backend, without changing the profile model.

Adapters describe compatibility, paths, loader requirements and activation; they do not mutate installations directly. The deployment planner/executor/recovery coordinator owns authorized writes. Preserve originals, durable intent, ownership manifests, exclusive installation locks, verification and restart recovery. SQLite transactions do not make filesystem operations atomic.

Existing-mod detection is read-only. Adoption requires explicit authorization, verified membership, preservation and an ownership manifest. Never delete uncertain files, silently overwrite external edits, execute archive contents automatically or mark success before verification.

## Compatibility and consequences

Confirmed targets are Half-Life 2, classic Doom II (not the reboot), Stardew Valley, Minecraft, Cyberpunk 2077, Skyrim, Baldur's Gate 3 and Terraria. Initial candidates are Half-Life 2, classic Doom II and Stardew Valley. Exact editions/builds/loader versions and initial release subset require validation; no adapter is implemented.

Packages remain immutable. Hardlinks must not expose immutable storage to game writes. Recovery assets are durable data. Test copy/reversal/profile switching, stale plans, external changes and interruption using disposable installations. Unknown mods may be cataloged without permission to deploy. Vanilla means no Game Manager-managed mods, not certified vendor-original files.

See [MDRS](../specifications/MDRS.md), [DSS](../specifications/DSS.md), [PRD](../specifications/PRD.md) and [source register](../SOURCE_AND_OPEN_DECISIONS.md). Concrete protocol, persistence format, replacement primitives and per-game recipes require fixture validation. No deployment code or successful test is implied.
