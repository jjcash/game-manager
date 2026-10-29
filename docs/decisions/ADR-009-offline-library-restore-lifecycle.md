# ADR-009: Offline library restore lifecycle

Status: M1.4 implementation decision under the user's explicit backup/restore authorization. Verification and limitations are recorded in the [M1.4 report](../development/M1.4_BACKUP_RESTORE_VERIFICATION.md); this record does not establish a wider Windows or physical power-loss certification.

## Decision

Keep the existing application database path and schema. Use SQLite `VACUUM INTO` for consistent committed snapshots, with validation, file flushes and publication that refuses existing destinations. The initial format is an ordinary standalone SQLite database. Migration checksums, exact current schema, application purpose, integrity, foreign keys and effective overrides form the compatibility contract. Older/newer or modified schemas are rejected without migration of the selected file.

An OS-backed lock beside the application database prevents two Game Manager processes from opening the same root. SQLx pool size and query behavior remain unchanged. Benchmark readers and synthetic job fixtures cannot invoke application maintenance. The lock protects cooperating application processes; it is not a sandbox against arbitrary same-user database manipulation.

Restore requires native file selection, a validated private candidate, an opaque confirmation token and explicit replacement confirmation. Confirming promotes the candidate directory outside unconfirmed cleanup scope before capturing any originals, closes admission to application database commands, drains/closes the pool and captures a verified pre-restore snapshot while retaining the process lock. A unique recovery directory preserves both snapshots, including interrupted preparation. Durable confirmed intent precedes replacement. Controlled restart checks that the stopped pool has zero connections and releases its lock before Tauri spawns the new process; old services remain permanently gated.

The next startup applies restoration before publishing database readiness. Original main/WAL/SHM/journal components are moved individually into a preserved directory. A verified copy of the candidate becomes active; the recovery snapshots are never opened as the writable application database. Normal initialization, full validation and count verification precede a completion marker. Markers publish a fully written, flushed payload through a temporary file without overwriting existing evidence; an empty or partial marker is not completion. Before completion, interrupted attempts or failed replacement/reopening recover a fresh copy of the original. Durable recovering intent precedes rollback and overrides an earlier completion marker until recovered completion verifies. Failure to verify recovery fails closed and preserves evidence.

A verified completed/recovered marker commits the data change. Subsequent failures may retry control-file finalization but must never replay a snapshot over committed data or later edits. Before services become ready, preserve the pending reference as archived intent, update a non-authoritative last-status reference while preserving its predecessor, and retire only the redundant active pending reference. Each boundary is repeatable. A reappearing pending reference with valid completion opens and validates the current library before retrying metadata finalization; it does not compare current counts to an old snapshot. An invalid completion marker combined with retirement evidence fails closed without changing active data. With no pending intent, historical status never authorizes replacement or rollback.

There is no atomic multi-file replacement claim. Unconfirmed copies have a separate marked staging scope; confirmed recovery directories are excluded from cleanup. Recovery snapshots, failed/displaced components and prior intent records are never silently overwritten or automatically garbage-collected. Their eventual cleanup needs a separately designed, explicit retention workflow.

## Consequences

- Replacement cannot occur under live SQLx services. The frontend cannot close or replace arbitrary files, pass SQL or choose backend paths through IPC.
- A restart is mandatory; records replace the current library and are never merged by title or regenerated.
- Preparation failures leave the active original in place, with a closed pool that requires restart. Incomplete preparation without published intent cannot trigger replacement.
- Current-schema standalone backups up to 1 GiB are supported initially. Local fixed/removable drive paths are allowed; network/device/alternate-stream paths are refused. Publication and installation require hard-link support. NTFS is the verified filesystem; removable/FAT/ReFS support is not certified.
- Existing data can require several times its database size during preparation/recovery. Disk-space failures preserve the active library or recovery copy rather than deleting originals to make room.
- File flushes and process-interruption tests do not prove behavior under arbitrary media failure, lost OS writes or actual power cuts. Missing/corrupt recovery data is a surfaced recovery-required condition.

No cloud service, encryption, compression, backup schedule, provider, game-file access, database migration, alternative framework or new package version is introduced. `tempfile` and the already locked Windows bindings gain narrowly scoped runtime uses.
