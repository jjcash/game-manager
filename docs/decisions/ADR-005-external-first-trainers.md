# ADR-005: Progressive Trainer Support, External First

Implementation status: **Not started**. This record documents an agreed direction; it does not claim completed code, tests, or benchmarks.

Status: **Accepted future direction**. Reconciled 2026-10-25.

## Context

The user requested later Wand-inspired cheats/trainers, maps and other Game Tools, then selected “Both progressively, starting with external trainers.” This does not change the Library + Mod Manager MVP or M0 priorities.

## Decision

Plan a future Game Tools area with categorized resources and optional external utilities. Trainer progression begins with registry/explicit launching, then deeper external integrations or presets, then potentially native functionality for selected games after separate product/security design. Maps and guides have their own later data and provider requirements.

## Consequences

The DSS game_resources model leaves room for tools, maps, guides, wikis and compatibility-scoped resources. A saved executable/link is not trusted merely because it is in the database. Tool code, providers, automatic downloads, silent execution, internal trainer engine, process-injection method and driver are not approved by this direction.

Avoid building a native trainer framework during the MVP. No specific Wand integration, provider, internal capability or release date is selected. Future native work needs a separate specification and decision. Core local use must stay independent of tool services.

See [PRD](../specifications/PRD.md), [DSS](../specifications/DSS.md), [roadmap](../ROADMAP.md) and [source register](../SOURCE_AND_OPEN_DECISIONS.md).
