# ADR-011: Standalone launch execution contract

Status: **Proposed — pending review.** M1.7 authorizes isolated synthetic feasibility only. This decision does not authorize production execution or accept itself.

## Threat model, decided before implementation

An authenticated local desktop user deliberately launches a previously configured executable. Stored configuration is metadata, not execution authority. Rust resolves authoritative records; the frontend supplies IDs and revisions only. Execution gives the selected program the user's ordinary OS privileges. Malicious executable content, a compromised application/OS, privileged attackers and deliberate same-user executable replacement are outside an executable-integrity guarantee. No sandbox, content certification or exclusive filesystem confinement is promised.

Same-user filesystem changes, junctions, symlinks and other reparses follow normal Windows resolution. Lexical installation-relative containment is not object confinement. Ordinary missing/replaced/unavailable objects and stale database context fail when detectable; validation cannot bind CreateProcess to an opened file identity. There is a filesystem time-of-check/time-of-use gap. This is an execution-specific decision, not an import of ADR-008's Steam confidentiality policy. Requiring adversarial object confinement would require another design, rather than expanding this candidate into a security framework.

In-scope failures include malformed references/arguments, stale frontend state, cross-game IDs, edits/deletion before execution, unsupported provenance/kinds/sources, accidental privilege inheritance, unintended executable search or shell interpretation, process creation failures and leaked parent resources.

## Proposed contract

Preserve ADR-010's `Game → LibraryEntry → Installation → LaunchProfile` identity and schema v4. A future request has only `gameId`, `profileId`, `expectedProfileRevision`, `expectedGameRevision`; reject extra fields. No executable, working directory, raw command line or environment map comes from launch IPC. A deliberate Launch gesture authorizes one attempt for that context. Profile creation/edit/default selection/restore/startup never authorize execution. A frontend boolean is not a persistent grant or proof against compromised same-origin UI; future IPC must enforce Tauri local-window capability admission and the same backend checks.

Preparation uses bounded indexed record lookup and a coherent database snapshot. Verify application purpose/readiness, game/profile existence, immutable membership/installation/game chain, user provenance, standalone kind, exact profile/game revisions and Windows installation context. Initially admit manual-source installations only; user profiles on provider memberships remain valid stored configuration but unsupported for execution. No provider adapter is inferred. A prepared decision exists at the successful validated snapshot read; it is internal, single-use and never an executable capability returned to JavaScript.

Immediately before process creation, acquire `BEGIN IMMEDIATE`, reread and validate the entire bound context, compare it to preparation, resolve and validate paths, and spawn while the writer reservation remains held. Existing profile revision and game revision remain authoritative; installation/default edits advance game revision as in M1.6. Also compare the observed installation/membership/source/configuration fields defensively. No new revision or identity mechanism is introduced. Detectable changes fail closed. After preparation a profile edit/deletion, default change or installation edit requires a fresh user attempt. Cooperating DB writes cannot commit between final validation and spawn. Direct database tampering with ABA changes is not prevented. A future maintenance admission guard must span final readiness validation through spawn; restore/shutdown must close admission and drain admitted work. This coordination is design-only in M1.7; no production service is connected.

## References and executable types

Reuse M1.6's local drive/backslash grammar without changing stored strings. Validate each reference before using Windows path components and `PathBuf::join`. Relative references require a nonempty, supported drive-absolute recorded installation location, also validated under that grammar. Missing/empty/unsupported location fails; absolute references use their saved location without rebasing and do not require a usable installation location unless the working directory is relative. Never use CWD, PATH search, extension guessing, discovery or environment expansion to resolve targets.

Only explicit `.exe` targets, case-insensitively, are supported. Batch/script/document/shortcut/DLL/URI targets fail without fallback. Existing regular-file metadata and an attribute-only directory handle are checked immediately before spawn, following reparses. The directory open catches access denial without enumeration and permits read/write/delete sharing. No header parser, signature/trust check or executable-file identity pin is claimed. The Windows loader decides actual executable format/access/dependencies; even successful creation does not establish initialization.

An explicit working directory follows its tagged reference. Omission means the resolved executable's lexical parent directory, including when a reparse target lies elsewhere. This is launch-time policy, never a persisted edit. Fail on unavailable/invalid directories; never create one or inherit Game Manager's CWD accidentally.

## Native process boundary

Use direct `CreateProcessW`, with the full resolved `.exe` in non-NULL `lpApplicationName` and a separately encoded mutable command line. Investigation of Rust 1.99.1 found stable `Command` lacks a handle-inheritance allowlist/disable control; its relevant extensions are nightly-only. That demonstrated requirement justifies this narrow Win32 boundary. No `raw_arg`, shell wrapper, ShellExecute, UAC request, token acquisition for elevation or fallback. Refuse execution if the caller's process token is elevated or the token check fails. A selected program can independently request elevation or perform arbitrary actions after it starts; the launcher cannot prevent that without sandboxing. Production must run unelevated.

Encode each literal argument with the standard Windows C/Rust runtime convention: quote every argument, double backslashes before quotes and at the closing delimiter, and escape embedded quotes. Windows receives a command-line string rather than an argv array. Preserve M1.6 limits and additionally reject an encoded line over 32,767 UTF-16 units including quoted executable and terminator. Some valid stored arrays therefore cannot launch. Nonstandard target parsers are not certified.

Use a newly opened read/write `NUL` handle for stdin/stdout/stderr. `STARTUPINFOEXW` sets `STARTF_USESTDHANDLES` and `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` containing only that handle. `bInheritHandles=TRUE` is required for the allowlist; unrelated inheritable handles are excluded. Exact flags are `CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT | EXTENDED_STARTUPINFO_PRESENT` (`0x08080400`); no suspended, detached, breakaway or elevation flag. A first failed NULL-stream design was discarded after actual native testing; see the report. Scoped native attribute storage, NUL/directory/token handles and the primary thread handle are released on all paths. The returned process handle is owned by Rust.

Pass NULL environment to inherit the ordinary parent environment for compatibility, with no per-profile overrides or expansion. Environment may contain sensitive values and influence DLL/runtime behavior; this is not isolation. Do not put application secrets in the launching process environment. Native fixtures verify literal arguments, directory, ordinary environment and exclusion of an unrelated inheritable event handle.

## Results and lifetime

Distinguish request rejection, target-resolution failure, process-creation failure and process-created PID. Public errors contain stage/code/optional numeric OS error only, never full paths, arguments or environment. A successful spawn is irreversible authorization consumption, even if later cleanup/reporting fails; never automatically retry. Successful creation is not game initialization, and even a zero exit is not gameplay.

Recommend returning after creation and immediately dropping the Windows child handle, without a background waiter, session record or global polling. PID is a momentary diagnostic identifier subject to reuse, not authority for later attachment/termination. Synthetic tests may retain handles for bounded observation. Cancellation before spawn rejects; after spawn it never silently kills the child. App exit does not intentionally terminate authorized children; externally imposed Windows job policies can affect their lifetime. No process supervision or playtime is included.

## Review gate

The [M1.7 report](../development/M1.7_STANDALONE_LAUNCH_FEASIBILITY.md) must record native evidence, limitations, exact API behavior and recommendation A or B. M1.8 still needs separate authorization and production admission/IPC/UI design and tests. No schema, backup, production command or capability change is proposed here.
