# ADR-012: Bounded standalone session observation

Status: **Accepted — 2026-11-08**, under explicit user approval of the five product decisions in section 9. Originally proposed 2026-11-04; production contract revised 2026-11-08. Acceptance establishes architecture and product policy, not implementation completion or M2.2 engineering authorization. M2.2 remains separately authorized work. [M2.1 evidence](../development/M2.1_SESSION_TRACKING_FEASIBILITY.md) governs tested claims; the [revision review](../development/ADR_012_PRODUCTION_CONTRACT_REVIEW.md) records baseline verification and evidence limits.

## Context and explicit ADR-011 extension

[ADR-011](ADR-011-standalone-launch-execution-contract.md) and M1.8 release the created process handle immediately. They prove creation, not initialization, survival, a window, gameplay or a whole-game session. The accepted extension permits a restricted duplicate of that exact handle for direct-child observation under this contract. Production continues immediate release under ADR-011 until separately authorized M2.2 implementation is completed and verified.

Retain identifier/revision-only local launch IPC, manual-installation scope, privilege checks, literal arguments, NUL-only inheritance, authoritative final writer-reserved validation, launch admission and sanitized errors. Successful `CreateProcessW` irreversibly consumes launch authorization. Observation, recording, cleanup or delivery failure cannot undo that success, retry creation, terminate the child or request elevation. Lost launch delivery is an unknown outcome, never authorization to retry automatically.

M2.1 validates retained-handle waiting, stop priority, direct-child exit evidence and synthetic survival. It does **not** validate the production transfer protocol, persistence, maintenance integration or UI below. Its harness uses `Command`; M2.2 must integrate with the existing production Win32 launcher.

## 1. Admission and ownership bound

Use exactly **two shared slot records**, with stages:

`free → reserved → transfer-pending → active → terminal-pending → free`

Any occupied stage may become `unsettled` after a failed/unknown durable write. **Reservations + transfers + active observations + terminal-pending + unsettled entries must never exceed two.** Each slot includes its fixed payload, acknowledgement, cancellation bit, latest evidence and persistence state. Moving stages never allocates another entry. No independent mailbox capacity, completion queue, event history or retry backlog. Observation duplicates across all stages never exceed two; one original creation handle may additionally exist during the existing single admitted launch.

Reserve before creation and requested-record insertion, with immediate refusal, no waiting observation queue. Disabled recording, full history, busy slots, failed reconciliation/database or unavailable observer produce an observation-unavailable reason while ordinary launching remains possible under M1.8. Capacity does not enforce game single-instance behavior.

Require coupling new production observation to confirmed durable recording admission: without a confirmed requested row, launch normally but do not start an unrecorded observation. One dedicated wait owner and one serialized async persistence driver serve the same two slots. Coalesce dirty bits/latest evidence in place. No per-game thread/task/timer, SQL in native callbacks, session-lifetime SQLite connection/writer, launch/maintenance permit or occupation of the two CPU-worker permits.

## 2. Exact handle transfer and acknowledgement

1. The blocking launch owner receives the process and primary-thread handles. Release the thread promptly. With an eligible reservation/confirmed requested row, duplicate **from the creation handle** with exactly `SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION`, non-inheritable; never `DUPLICATE_SAME_ACCESS` or PID reopening. Identity reads use this duplicate. Duplication/identity failure releases acquired resources through their sole RAII owners. Release the original process handle promptly on every path.
2. The producer owns the duplicate until moving it into its reserved slot under the short service-state lock. Validate attempt/slot/run/database-generation tokens and admission there. Refusal leaves ownership with the producer for immediate drop. Successful publication is the transfer linearization point: the slot owns one `OwnedHandle`; the producer retains no owning alias or raw-handle cleanup responsibility. Wakeup/acknowledgement does not transfer ownership.
3. Between native waits, the owner takes that value once into its private fixed wait set and marks the same slot active. Only that owner removes/closes waited handles after the current wait returns. Producers, timeout handlers, persistence and UI set flags only. Borrowed raw handles are confined to the owner's native call.
4. Acknowledgement is generation-tagged state in the same slot, readable after notification/IPC loss. Notifications own no handle or additional completion payload. Dropping a receiver cannot free/cancel ownership. A sender never reclaims a transferred handle after acknowledgement loss.
5. Launch never waits for acknowledgement. Release originals and roll back final validation before publication/refusal; release the launch permit after that short step. Return process-created with the observation state known when assembling the response: `pending`, `active` if acknowledged, `ended`/`interrupted` if already terminal, or `unavailable(reason)`. Pending is not confirmed coverage. No later status can change the creation result.

Use a proposed **one-second monotonic transfer deadline** from publication. Pending at expiry means rejection/owner cleanup/terminal uncertainty. Adoption that linearized before expiry remains adopted; late acknowledgement does not reverse ownership. Adoption and deadline checks serialize under the same lock; a cancelled/stale slot cannot be resurrected. Reuse requires native release and confirmed durable settlement, and changes the occupant token.

The transfer deadline, one-second control watchdog and five-second stop-acknowledgement deadline below are proposed session protocol constants, not changes to existing performance budgets or measured guarantees. Scheduling can exceed them; evaluate expiry before adoption when the owner next executes.

## 3. Failure, wakeup recovery and maintenance

Wait on one manual-reset control event first and at most two process handles. Stop wins simultaneous stop/exit. Shared state is authoritative; wakeups are hints. Coordinate mutation/signaling and owner consumption/event reset with the same short lock to prevent reset-versus-publication lost wakeups. A finite wait timeout of at most one second rechecks stop, deadlines and pending state if notification is lost. Establish a signaled process before reading its exit code: code 259 is not a liveness test. Already-exited children skip alive evidence.

Signal failure atomically latches failure/closes observation admission; the watchdog detects it. `WAIT_FAILED`, invalid identity and owner unwind likewise fail observation closed. After waiting ends, the owner releases queued/active handles once, retaining at most two interruption summaries in their slots. The supervisor notices owner termination and disables new observation/recording. The private wait-set RAII owner releases active handles during unwind; after confirmed join, the supervisor can destroy never-adopted queued values without touching a pending wait. State-lock poisoning must allow cleanup/closure, never new adoption; no state lock spans a native wait or database call. No automatic observer replacement, handle resubmission or retry storm in that run. Unwind cleanup does not promise destructors under panic-abort/hard failure; Windows closes parent handles and durable rows cover missing delivery.

Shutdown/confirmed restore:

1. Close launch and observation admission against reservations/publication. An already-admitted launch may complete its one creation.
2. Drain short creation/rollback/publication/refusal work. Do not acquire a writer while waiting for the launch drain. Native creation retains ADR-011's lack of a guaranteed cancellation deadline.
3. Signal stop; the owner returns from its finite control wait and releases queued/active handles without waiting for game exit. Await thread-completion acknowledgement for at most five seconds, then join only a confirmed finished thread. Timeout is a lifecycle failure, not permission to close its handles or proceed with replacement. The caller holds no SQLite connection/writer during this wait/join. Reject adoption after stop, including a just-published transfer. Interruption may conservatively win an exit race.
4. Allow at most one final persistence attempt per occupied slot under existing pool/SQLite timeouts; close/drain persistence operations before closing the pool. Failed summaries may be lost at teardown, but unfinished durable rows are not deleted or declared settled. Never retry indefinitely during maintenance.
5. Invalidate the database generation before replacement; old services stay closed. Retain [ADR-009](ADR-009-offline-library-restore-lifecycle.md)'s mandatory restart, preserved originals and conservative recovery.

Validate stop/generations at adoption and database-operation admission/commit. Terminal writes committed to the old database before maintenance remain terminal there. Final maintenance writes are explicitly admitted only against that still-current old generation after native drain; other stopped producers cannot submit work. Pending exits never replay into restored data. Bounded database drain can consume existing timeouts, never a game lifetime. If an owner cannot be joined or writes cannot be drained safely, surface lifecycle failure and fail maintenance closed while preserving originals; never close another thread's waited handle or replace a live database to force progress.

Cancel, disable, failure and application exit stop observation only, never intentionally stop the child. External Windows job policy remains an OS limitation.

## 4. Durable session records

The accepted persistence design calls for schema v6: `local_session_observations` plus a singleton recording preference/revision. Rust generates independent opaque 128-bit row, attempt and observer-run IDs. Allocate attempt once per explicit admitted launch gesture; `attempt_id` is unique and reused for every persistence retry. No PID/title/path-derived identity or new frontend execution authority.

| Field group    | Contract                                                                                                                                                                          |
| -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Identity       | Immutable game, installation and LaunchProfile IDs from the authoritative same-game chain; captured game/profile revisions are provenance, not launch authority.                  |
| Lifecycle      | requested, created, direct_exited, interrupted, not_created; separate creation outcome unknown/created/not_created preserves creation evidence after interruption.                |
| Times          | Required requested UTC; nullable created, last-observed and observation-ended UTC. Observation end is not game end. Optional numeric native creation/exit FILETIME and exit code. |
| Elapsed        | Nullable checked nonnegative within-run monotonic observation-window and last-alive elapsed; last-alive ≤ observation window.                                                     |
| Interpretation | Process-only confidence; handoff unknown; whole-session outcome uncertain; constrained terminal reason; attributed playtime NULL throughout M2.2. No totals.                      |
| Concurrency    | Nonnegative revision, immutable attempt/run IDs; every operation carries an in-memory database generation that restoration cannot revive.                                         |

Requested/created alone are unfinished. Not-created requires known rejection/creation failure; requested surviving a crash cannot prove whether creation happened. Direct-exited requires signaled-handle and creation evidence. Interrupted preserves known/unknown creation, with actual game end unknown. Reasons distinguish observed exit, launch rejection, creation failure, cancellation, disable, shutdown, restore, transfer timeout, observer failure, persistence failure and restart/restore uncertainty. Missing exit evidence is NULL, not zero. Terminal records are immutable except explicit deletion; stale delivery cannot upgrade interruption into claimed exit.

### Write and settlement protocol

1. After coherent authoritative launch preparation, with a reserved slot/current ready generation, commit requested in a short transaction before final launch revalidation. Check the identity chain, history and unfinished capacity atomically. This optional step is outside ADR-011's read-only final-validation transaction. Failure cannot indefinitely block launching or authorize another spawn.
2. Execute unchanged final validation/creation under its writer reservation. Duplicate if eligible, drop originals, roll back, then publish/refuse transfer. No post-creation database acknowledgement gates launch success.
3. Release launch resources before brief created/latest/terminal writes. Requested can move directly to direct-exited with creation evidence, or to not-created after known rejection. A crash around spawn leaves explicit ambiguity, never replayable launch authority.
4. Conditional updates match row/attempt/run, expected revision and permitted prior lifecycle. Check generation both at admission and under the short lifecycle guard spanning commit. Zero affected rows requires a bounded read: identical committed evidence is idempotent success; missing/stale/conflicting state is surfaced, never recreated under a new ID or counted twice.

The async driver reads the two slot snapshots, not a copied event queue. Permit one ordinary write attempt and at most one automatic retry per pending write; replacing evidence cannot reset that budget. Exhaustion closes new recording/observation admission for that run, stops affected native coverage as uncertain and retains unsettled slots. Explicit recovery may retry these same at most two debts; confirmed settlement/validation can reopen admission without reattaching to old processes. Core launching stays available.

**Unfinished durable rows consume capacity even after handles and undelivered summaries disappear.** Unknown insert/commit outcome pins its slot until a read proves absence or terminal settlement. Never release failed slots and insert fresh requested rows repeatedly, or delete/reinsert to retry. In-memory terminal-pending is a delivery state; durable interrupted is a committed coverage fact. Teardown can lose richer evidence while leaving requested/created for reconciliation.

Enforce at most two unfinished rows in schema-v6 write guards as well as service admission. Release requires proven absence or confirmed terminal commit. Each new unfinished row therefore requires a slot, and failed persistence never frees its debt: repeated failures cannot accumulate an unbounded restart backlog.

### Restart and restore

Before recording admission, read the partial unfinished index with `LIMIT 3`: zero through two normal, third means invariant violation. In one brief transaction conditionally mark old requested/created rows interrupted for restart/restore uncertainty, advancing revisions and preserving last known evidence and known creation. Actual end/playtime stay unknown. Detection time can locate an interruption report but cannot extend observation past the last known sample. The new runtime does not adopt the old run ID.

Failed reconciliation/excess keeps observation/recording closed and shows recovery status. No fresh rows, unbounded repair, PID reopening, descendant discovery or charging downtime. Library/launch remains usable where existing database-readiness rules allow it; games may still run externally. Exact snapshot validation rejects excess unfinished rows. Restore replaces, never merges/replays, history; reconcile only the new active copy before observation readiness. v2–v5 upgrades create empty history.

## 5. History retention and deletion

Require a **10,000-row global ceiling with pause on capacity and explicit deletion**, counting all requested/created/terminal rows. This session-storage policy does not change library performance budgets. Insert guards enforce the cap transactionally. No silent FIFO pruning, archiving, overwriting oldest records, dropping terminal updates or treating personal history as cache.

At capacity, launch succeeds with history-full observation unavailability. Existing terminal updates, disable and restart/restore reconciliation remain possible because they allocate no rows; bypass the new-record gate for them. Deletion remains available at capacity/off. Resume only future explicit launches once enabled, capacity exists and all recovery/debt is cleared. Skipped launches are never retrospectively observed.

**Minimal explicit deletion is required before production persistence ships.** Provide a global keyset-paged history view, optional selected-game filter, at most 100 records per page, capacity/recovery status and visible unknowns. Permit selection of at most 100 terminal records per operation. A narrow local-main-window command prepares an ID/revision/count-bound confirmation; retain at most one pending confirmation, replacing/cancelling it on a new selection or lifecycle close. Confirmation consumes it once and validates that exact set/database generation transactionally. Reject stale selection atomically. Refuse unfinished, active, terminal-pending or unsettled records. Cancellation changes nothing. Explain permanent active-library deletion, copies retained in backups/recovery snapshots and possible return on restore. No secure erasure/backup purge. Dedicated history export, bulk retention and attribution editing are deferred; do not silently create an export/backup as a deletion side effect.

Session foreign keys to game, installation and LaunchProfile use `ON DELETE RESTRICT`. Profile edits preserve history/captured revisions. Profile, installation, membership and game deletion explain the dependent history and require its separate reviewed deletion first; active/unsettled history must settle first. Existing provenance/default/profile deletion rules still apply afterward. No personal-history cascade, detachment, retargeting, copied path/name snapshot or hidden composite delete. Group deletion is unaffected (no history reference). Explain this restriction in affected editors. No game files/artwork originals are touched.

Tradeoff: bounded storage/validation and simple identity integrity, at the cost of deliberate record removal for heavy users and temporary restrictions on deleting referenced metadata. Opt-in disclosure, visible capacity, actionable refusals and minimal deletion are necessary parts of the accepted policy. Unlimited history loses the bound; deferring deletion creates a recording dead end. Detached historical identities need a separate design/migration.

## 6. Schema-v6 and backup contract

No DDL/migration is added here. Once separately authorized, M2.2 must add one forward application migration after v5:

- Preserve v1–v5 migration SQL/checksums byte-for-byte. Add the table/preference, constrained fields, unique attempt key, immutable-chain/terminal/capacity guards; global `(requested_at_utc_ms, id)`, selected-game `(game_id, requested_at_utc_ms, id)` keyset indexes, partial unfinished index and indexed reference checks. Default disabled; no fabricated history or changes to existing IDs/revisions/overrides/artwork.
- Preserve exact v2/v3/v4/v5 validators and historical embedded migration prefixes/schema SQL. Add exact v6 table/index/trigger validation and enum/NULL/identity/preference/capacity/unfinished checks. Integrity, foreign keys, purpose, overrides, profiles and artwork checks remain mandatory.
- Read **at least seven ordered migration rows** (six plus overflow sentinel): retaining the current six-row limit while admitting v6 could hide unsupported newer history. Reject seventh/later rows, gaps, wrong versions/checksums/order, failed migrations, extra/modified objects and invalid data. Active initialization likewise rejects unsupported newer histories before upgrade. Never zip only a matching truncated prefix. If schema-object bounds become insufficient, detect overflow rather than omitting objects.
- Validate selected/private/preserved snapshots read-only at their exact version; never migrate them. Upgrade only the installed owned active copy, transactionally before readiness. Preserve ADR-009 recovery snapshots/failed components; failed migration/reconciliation cannot mark restore complete or discard originals. Completion markers never replay snapshots over later edits.
- New backups use v6; old v2–v5 selected/preserved bytes remain unchanged. Old builds cannot read v6; no downgrade. Benchmark generator-v1 stays isolated at schema v1; all six frozen fixtures/checksums and existing budgets remain unchanged.
- Preserve M1.9 asset metadata, assignments, originals and missing/corrupt-artwork behavior. Database backups include references, **exclude artwork originals**. Restore replaces assignment metadata, not image files. No artwork collection through session cleanup.

Backups contain **unencrypted activity metadata**, including IDs/timestamps/evidence even after recording is disabled. Disclose this and artwork-original exclusion in backup creation, selected-backup review and replacement confirmation. Restore replaces history/preferences; unfinished rows become uncertain on the new active copy, never attachment/launch authority. v6 recording preference replaces the old preference on confirmed restore, visibly disclosed; restoring v2–v5 starts disabled. A snapshot containing up to two unfinished rows remains valid and does not stop games.

## 7. Privacy and product meaning

Recording defaults **off**, including upgrades. Explicit enable discloses process-only evidence, limits/cap/deletion and unencrypted backups; applies only to future explicit supported launches. Disable closes admission first, requests interruption/handle release, then saves disabled preference in a short transaction. Never kill children or erase history. If saving fails, stay paused this run and explain that the preference was not saved and may revert on restart. Re-enable never attaches to running games.

Persist IDs/revisions, bounded enum/reason fields and numeric/UTC evidence only. No persisted PID in M2.2; optional native timestamps are facts, not authority. No paths, arguments, environment, window titles, input, process lists, background enumeration, telemetry, cloud requirement or automatic totals. Sanitize operational errors/logging; no unbounded diagnostic attempt history.

- **Process created** is M1.8 success independently of observation; never claim initialization/gameplay or failed launch because tracking failed.
- **Observation pending/active/ended/interrupted/unavailable(reason)** describes direct-child coverage. Explain disabled/full/capacity/persistence/observer/recovery causes without paths/raw SQL errors. UI loss cannot change backend ownership.
- **Direct child exited** needs a signaled handle, including code 259. Zero/nonzero does not prove success/crash; fast exit does not prove handoff. Generic observation stops here, handoff unknown and whole-session outcome uncertain.
- **Process lifetime** is optional native child creation/exit endpoints. **Observation window** is checked `Instant` adoption-to-last/terminal observation, including detection lag, optionally last-alive sampled evidence. Wall UTC/FILETIME can jump; no inferred monotonic duration from them. Sleep/resume/idle does not establish gameplay.
- **Game end/user-attributed playtime** remains unknown; NULL is not zero. No observation duration, app uptime, restart downtime or provider total arithmetic creates playtime.

History/status remain visible while off; history read failure is explicit, never an empty-history claim. Frontend retains one keyset page and at most two current summaries, replacing on refresh. Coalesce backend state; no UI-disconnected event backlog. Reconnect reads current slots/service status plus actual history. Unavailable launch replies are not retained in an unbounded log; their later delivery is not guaranteed.

## 8. M2.2 scope and outstanding implementation verification

M2.2 is **not implemented or authorized** here. The accepted architecture requires one bounded slice: production native integration, admission/transfer, v6 persistence/debt/reconciliation, opt-in status/history and minimal deletion/disclosures together. If separately authorized implementation cannot meet that complete scope, return for a scope decision; no silent persistence-free substitute.

Sequence: define narrow Rust/IPC states and fault seams → integrate creation-handle duplication/publication → v6 migration/exact validation/conditional writes → lifecycle generations/stop/reconciliation → minimal settings/status/history/deletion → verification. No substantial dependency or generic process/scheduler framework.

| Gate                  | Required concrete tests/evidence                                                                                                                                                                                                                                                                                                                                                                                        |
| --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Admission             | Interleave reservation/requested insert/transfer/adoption/exit/settlement; mixed occupancy ≤2. Third observation refused while a third sequential launch succeeds. No mailbox/owner double occupancy. Race 9,999/10,000 history and disable/maintenance with reservation.                                                                                                                                               |
| Ownership/delivery    | Exact handle recovery on duplicate/identity/publish failure, pending timeout, adoption-versus-deadline, lost/late/stale acknowledgement. Drop IPC before/after creation, status receiver before/after adoption, persistence receiver after exit; no double-close, leak, reclaimed handle or freed debt.                                                                                                                 |
| Native security       | Unelevated production launcher: exact original-child identity, restricted non-inheritable duplicate, unchanged NUL-only inheritance/flags, original/thread cleanup, no terminate/PID reopening. Immediate/nonzero/259 exit and known descendant survival without adoption.                                                                                                                                              |
| Wakeup/failure        | Lost/reset-coincident notification, signal failure, safe fault seam for WAIT_FAILED, unwind, hard parent failure and dropped controller. Watchdog recovers work/stop; failure closes admission, no observer restart or close during wait. Never inject by unsafe close of a waited handle.                                                                                                                              |
| Maintenance           | Race stop/transfer/adoption/exit/terminal commit/restore. Stop priority, bounded join/drain without writer or game-exit wait; old generations rejected even when restored IDs match. Failed restore preserves originals. Independent fixture handles prove child survival after disable/stop/manager exit.                                                                                                              |
| Persistence           | Requested commit ambiguity, disk full, pool timeout/contention and repeated created/terminal failure across more than two launches. Unfinished rows ≤2/debt pinned; later launches succeed unobserved; bounded retries, duplicate no-op/conflict visible. Crash before/after spawn/terminal commit never invents success/relaunch.                                                                                      |
| Reconciliation        | Zero/one/two unfinished rows, failed reconciliation, invalid third, full history, restored duplicate completion, v2–v5 empty-history upgrade. At most three indexed candidates; retain known creation/last sample and unknown actual end/playtime; no PID/descendant recovery.                                                                                                                                          |
| Product/deletion      | Off/enable/disable-during-observation/failed preference save; created-plus-unavailable results; 100-row page/selection bounds; stale/generation confirmation rejection; active/unsettled refusal; delete at cap; reference-restricted metadata deletion, no cascade. Disclosures and unchanged existing backup bytes after history deletion.                                                                            |
| Migration/backups     | Exact v2–v5 compatibility/unchanged SQL/checksums; exact v6 empty/populated/full/unfinished. Reject seventh/later history, wrong prefix/checksum/state/chain, extra objects, over-cap data. Selected/preserved bytes unchanged, active-only upgrade, repeated restore/rollback; stable IDs/NULL overrides/artwork; six unchanged v1 fixtures.                                                                           |
| Resources/performance | One wait owner/driver and fixed payloads; no growth with disconnected UI, blocked writes, many launches/long idle. Measure handle recovery, CPU/wakeups, memory, launch overhead, resource release and maintenance with two long-lived fixtures. Compare 15k/25k query/cache/virtualization when production query/scheduling changes require it; preserve ADR-007 E1–E5 and all budgets. No new M2.1 performance claim. |

For implementation run relevant repository formatting, TypeScript/frontend tests, default/all-feature Rust suites, warnings-denied all-target Clippy and ordinary/benchmark-feature builds. Preserve historical assertions and execute complete **12/12 + 9/9** launch native gates, existing **10/10** observation target and new production tests. Follow [native policy](../development/M1.7_CI_AND_NATIVE_VERIFICATION_POLICY.md) for attributable exact-tree local security/lifecycle evidence and cleanup, plus required hosted Windows checks on the intended revision. Hosted compile-only coverage is not native execution; documentation checks do not satisfy M2.2.

Exclude universal tracking, gameplay attribution/totals, generic handoff/descendants, providers, job supervision, mod activation, new frontend filesystem/shell authority and changes to historical evidence/assertions/budgets.

## 9. Rationale, alternatives and accepted product decisions

Retaining the creation handle binds exact object identity without scanning. One ledger bounds transfer/completion/durable debt; finite control waits recover lost notifications without per-game polling workers. Requested records expose the non-atomic spawn/database boundary. Restrictive references and explicit deletion protect personal history without detached-identity machinery. Opt-in/unavailable states preserve privacy and launch availability.

Rejected alternatives:

- Separate transfer/completion capacities: combined occupancy violates the bound.
- Free failed slots and reconcile unlimited rows: repeated persistence failure breaks bounded restart.
- Synchronous acknowledgement or session-lifetime writer/launch permit: couples launch/maintenance to observation and encourages unsafe handle reclamation.
- Durable post-spawn acknowledgement required for launch success: irreversible creation cannot be rolled back.
- PID polling/reopening, global snapshots/ETW/WMI, debugger/job supervision or generic descendants: identity/privacy/lifecycle cost without gameplay certainty.
- Registered callbacks/per-game waiters: callback/unregister or scaling complexity for two entries; no comparative benchmark claim.
- Unlimited history, FIFO, automatic export-and-delete, cascading/detaching history: loses bounds/personal data or adds identity semantics. A future explicit design may revisit retention.
- Default-on recording/lifetime-derived playtime: insufficient privacy choice/gameplay evidence.

The user explicitly approved all five product decisions on **2026-11-08**:

1. Recording is **off by default** and requires explicit opt-in.
2. No native observation starts without **confirmed durable recording admission**.
3. A **10,000-record history ceiling** pauses new recording without automatic pruning.
4. **Minimal, explicit terminal-history deletion is mandatory before M2.2 production persistence ships**.
5. Deletion of **games, installations and LaunchProfiles is restricted while referenced session history remains**; separate reviewed history deletion and settlement come first.

Acceptance establishes architecture and product policy only. Completed M2.1 feasibility does not establish production completion. M2.2 requires separate engineering authorization and all section 8 implementation verification; no migration, production session IPC or launch behavior change is made by this decision.

Export format, detached history identities, configurable retention and attribution editing are deferred; gameplay/handoff is excluded, not an unresolved M2.2 promise.

Stop adoption if bounded ownership/debt, recoverable control, conservative lifecycle or complete persistence/deletion scope cannot be preserved. Return for review rather than expanding into universal process management.
