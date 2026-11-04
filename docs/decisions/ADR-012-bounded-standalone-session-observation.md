# ADR-012: Bounded standalone session observation

Status: **Proposed — 2026-11-04. Not accepted.** M2.1 authorizes isolated feasibility and this proposal only. Production adoption requires separate acceptance and M2.2 authorization. [M2.1 evidence](../development/M2.1_SESSION_TRACKING_FEASIBILITY.md) governs tested claims.

## Context and explicit ADR-011 extension

[ADR-011](ADR-011-standalone-launch-execution-contract.md) currently requires immediate release of the created process handle. M1.8 does exactly that. It proves creation, not initialization, a window, survival or gameplay. Reopening a returned PID later cannot safely recover the original process identity.

Propose a narrow extension: after successful creation, a Rust observation owner may retain a restricted duplicate of that exact process handle. The launch owner still releases its original handle and primary thread handle promptly. All existing authorization, privilege, argument, inheritance, path, validation, writer/admission and creation-only result contracts remain required. Observation failure after creation never relabels creation as failure, retries creation or terminates the child. Until this ADR is separately accepted, production must continue immediate handle release without observation.

## Proposed ownership and limits

- Reserve one of **two** observation slots before creation; no waiting observation queue. Launch remains available if tracking capacity is unavailable, with a distinct unavailable observation outcome in a future separately designed interface. Capacity does not establish a game single-instance policy.
- Duplicate the created process handle with only `SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION`, non-inheritable. Release the original promptly. No terminate, VM read/write, debug, token elevation or global enumeration access is required. Duplication/identity-read failure releases acquired resources and records uncertainty where possible.
- Identify an observation by an independent session ID and in-memory generation, with PID plus native creation FILETIME as diagnostic evidence. The retained handle binds the original process object during observation. PID alone, and even a persisted PID/time pair across boots/restores, are never durable authority.
- One Rust owner serializes a fixed wait set: one control event and at most two process handles. The M2.1 experiment validates this primitive and cross-thread stop. A production owner still needs a bounded two-slot transfer mailbox with wakeup/acknowledgement and race tests; no thread per game. Do not occupy the existing two CPU-worker permits, SQLite connections, launch admission or maintenance admission for a session lifetime.
- Only the owner removes/closes waited handles after a wait returns. Other threads signal the control event; they never close a handle beneath a pending wait. Reject excess work immediately. Terminal summaries have a maximum of two occupied slots, replacing their active entries until delivered/persisted or explicitly abandoned as uncertain. No unbounded event/history queue.
- Exit, cancellation and observer/native failure release process handles once. Session cancellation means stop observing, never stop the process. Ordinary ownership drop cannot promise delivery of an interruption result; persisted unfinished records must account for that gap. Native failure is an interrupted observation, not a game crash diagnosis.

## Confidence and handoffs

Keep observed facts (request admitted, process created, direct child sampled alive, direct child signaled exited, observation interrupted) separate from inferences (possible handoff, overall session uncertain). Exit code zero does not establish successful gameplay; nonzero does not establish a game crash. A very fast exit can be normal, failed initialization, an updater or a handoff; duration alone cannot choose among them.

Generic standalone observation ends at the direct child's exit. Do not enumerate descendants, attach by basename/path/PID, hook windows/input, inspect memory or adopt unrelated already-running processes. A future validated adapter may supply explicit handoff evidence under a separate contract. Without it, retain `handoff=unknown` and `session_outcome=uncertain`; do not fabricate successful gameplay. Neither process lifetime nor an observation window is user-attributed playtime. No automatic total-playtime increment follows from these facts alone.

## Shutdown, restart and maintenance

Shutdown/confirmed restore first close launch and observation admission, drain the existing short launch operation/handle transfer, signal the observer and join it after its current native wait returns. Release all process handles without waiting for games to exit. Stop wins simultaneous stop/exit signals conservatively: interruption may be recorded even if the process just exited. Brief terminal persistence must finish or be abandoned as uncertain before closing SQLite; no writer is held while joining. A failed observer must not keep maintenance admission open for a game's lifetime.

The candidate does not implement database or application lifecycle integration. Production tests must verify stop-versus-transfer, restore-versus-completion, writer contention, dropped receivers, thread failure and shutdown ordering. Existing native OS creation calls retain ADR-011's lack of a guaranteed cancellation deadline.

On restart, inspect only a bounded indexed set of unfinished session rows. Mark them interrupted with unknown end, retaining the last confirmed observation. Do not reopen PIDs or enumerate processes, and do not charge downtime/application uptime as activity. A game can still be running externally. Restored session records are historical metadata; they cannot authorize tracking or launching. A restored unfinished row receives the same conservative treatment, never invented completion or replay into another database generation.

## Persistence, clocks and privacy

The report proposes a later versioned schema; M2.1 changes no migration and retains application v5/exact backup compatibility. Session records belong to stable game/installation/LaunchProfile identities, not paths or labels. A future migration must preserve all existing IDs/revisions/overrides/artwork, exact historical snapshot validation and benchmark schema v1.

Use `Instant` for within-run elapsed observation measurements; persist UTC timestamps separately. Wall-clock rollback can invert UTC endpoints, and OS creation/exit FILETIME differences do not provide a monotonic gameplay duration. Keep unknown end timestamps unknown. Sleep/suspend/idle and human attribution require separate policy before any playtime calculation.

Store only local IDs, coarse lifecycle timestamps, duration evidence, confidence/reason and optional numeric native evidence. No executable path, arguments, environment, window titles, input history or process lists in session history. Default restart policy needs no persisted PID at all. Retention must be bounded without silently deleting personal history: propose a 10,000-row history ceiling that pauses new recording until explicit export/deletion is designed. This is a new session-only proposal, not a change to existing performance budgets. Backups remain unencrypted local metadata and may disclose activity; restoration replaces history without merging or double counting. No cloud dependency or telemetry.

## Alternatives and stop rule

PID polling/reopening is rejected for identity races. Global process enumeration, ETW/WMI subscriptions, debugger attachment and job-based process supervision add permissions, privacy or lifecycle costs without solving generic gameplay attribution. Registered waits could remove the dedicated thread but require callback/unregister ownership not needed for two slots. M2.1 tests one shared Win32 wait set; it does not claim comparative benchmarks of every alternative.

If production cannot preserve bounded ownership, nonblocking maintenance and explicit uncertainty within this narrow design, stop production adoption and return for a concrete scope decision. Do not expand into universal process discovery. See the report's single recommendation; this Proposed ADR grants no production authority.
