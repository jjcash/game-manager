# ADR-004: Virtualized Details View and Scale Validation

Implementation status: **Not started**. This record documents an agreed direction; it does not claim completed code, tests, or benchmarks.

Status: **Accepted direction**; numeric budgets are provisional, unmeasured engineering targets. Reconciled 2026-10-25.

## Context

The user reports slow startup, sluggish scrolling, slow search/filter/sort and freezes during imports. Their reference is a 12,320-game Playnite library, Details View, Ryzen 5 4500 with 64 GB RAM, and HDD storage with SSD placement available.

## Decision

Make virtualized Details View the primary library experience, with bounded rows/overscan, database queries, summary DTOs and lazy thumbnail loading. Also plan a virtualized cover grid. Never require a full-library scan or network response before local browsing. Isolate expensive work in bounded prioritized jobs.

Benchmark datasets: 500, 2,000, 5,000, 12,320, 15,000 release-scale and 25,000 stress. M0, the first engineering milestone, validates the core stack with synthetic data. Test SSD/HDD placement and a lower-memory configuration; 64 GB is not a minimum requirement.

## Consequences and acceptance evidence

The original TAS proposes warm interactivity within 1 second after window creation, cold SSD startup under 3 seconds, p95 indexed search under 100 ms, filter/sort under 150 ms, small-page retrieval under 50 ms, cached details under 150 ms, 60 FPS scrolling, and stable bounded memory. These are goals to measure/tune, not achieved guarantees. Record cache state, build/version, dataset, hardware, storage and concurrent load.

Do not load the full library, full manifests or full-resolution covers into frontend state. Measure query, IPC, artwork, frame-time and background-I/O costs together. No performance-sensitive feature is complete without release-scale validation. Historical prototype checkmarks were accidental: no prototype or measurements exist.

See [TAS performance specification](../specifications/TAS.md), [roadmap](../ROADMAP.md) and [status](../PROJECT_STATUS.md).
