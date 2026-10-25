# ADR-003: Core Stack and Local-First Boundaries

Implementation status: **Not started**. This record documents an agreed direction; it does not claim completed code, tests, or benchmarks.

Status: **Accepted architectural baseline**. Reconciled 2026-10-25.

## Context

The original TAS and subsequent user architecture agreement establish the performance-first Windows architecture. The supplied package incorrectly reduced local-first and supporting stack evidence to an unrecovered baseline.

## Decision

Use Tauri 2, React + TypeScript, Rust and SQLite in a modular desktop application. React owns presentation and temporary UI state. Rust owns durable business logic, files, processes, database access and background work. SQLite is the local source of truth. Expose narrow typed Tauri commands, with compact events/channels for progress. External integrations use adapters.

Core library, supported launching and local mod functionality work without accounts, internet or cloud services. No microservices, mandatory local HTTP server or hosted infrastructure is required for the MVP. Provider failures cannot block unrelated local workflows.

## Supporting choices and consequences

Preserve Tailwind CSS, SQLx, TanStack Virtual, Zustand for UI state, TanStack Query with bounded caches, pnpm and persistent Rust jobs. The recovered TAS proposes Tokio, FTS5, WAL/coordinated writes and a bounded read pool. Pin compatible versions and validate exact settings during engineering; no package installation or working implementation is claimed.

Keep Game, LibraryEntry, Installation, LaunchProfile and ModProfile distinct. Grouping is configurable; it does not erase installation identity. Built-in features come first; internal adapters do not require a public plugin ABI. Avoid prematurely creating many empty crates. Flutter and optional Supabase remain future candidate technologies, independent of the core.

See [TAS](../specifications/TAS.md), [DSS](../specifications/DSS.md) and the [source register](../SOURCE_AND_OPEN_DECISIONS.md) for recovered designs, remaining choices and historical ADR mapping.
