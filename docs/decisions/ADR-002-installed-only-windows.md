# ADR-002: Installed-Only Windows Distribution

Implementation status: **Not started**. This record documents an agreed direction; it does not claim completed code, tests, or benchmarks.

Status: **Accepted**. Reconciled 2026-10-25.

## Context

After the DSS draft, the user asked about portable complexity, then explicitly decided: “Okay, let's drop the portable version. You can proceed now.” The later MDRS removed portable distribution from the roadmap.

## Decision

Build an installed Windows application. Remove portable distribution, mode detection, portable path overrides, relocation workflows and portable-specific testing from the current roadmap. Do not maintain a parallel portable release variant.

Retain centralized path management, separate binaries/user data, configurable installed data locations, SSD/HDD placement and verification of installation identity before changes. Configurable initial roots do not imply live relocation or moving active deployments between computers.

## Consequences

Use one installer format initially; NSIS was suggested, not finalized. Minimum Windows/architecture support, signing, updater, exact directories and uninstall retention remain implementation/release choices. Upgrades must preserve databases, packages and recovery assets; uninstall must not treat external game files as app-owned.

This decision supersedes portable commitments in the original PRD/TAS/DSS drafts. [Reconciled DSS](../specifications/DSS.md) and [TAS](../specifications/TAS.md) apply it throughout. A future portable proposal would need a new decision; none is scheduled or required.
